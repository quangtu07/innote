//! Local streaming preview, independent of the slower Whisper refinement worker.
//! Native Moonshine (or legacy Vosk) runs in a child: failure cannot stop capture.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

/// Keep archive handles open while capture extends them. Avoid reopening both
/// endpoints fifty times a second and query live size through the open handle.
struct PcmTail {
    sources: [(std::path::PathBuf, Option<File>); 2],
    bytes: Vec<u8>,
}
impl PcmTail {
    fn new(dir: &Path) -> Self {
        Self {
            sources: ["system.pcm", "microphone.pcm"].map(|name| (dir.join(name), None)),
            bytes: vec![],
        }
    }
    fn available(&mut self) -> Result<usize> {
        let mut available = 0;
        for (path, file) in &mut self.sources {
            if file.is_none() {
                match File::open(path.as_path()) {
                    Ok(opened) => *file = Some(opened),
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => return Err(e.into()),
                }
            }
            available = available.max(file.as_ref().unwrap().metadata()?.len() as usize / 2);
        }
        Ok(available)
    }
    fn read(&mut self, start: usize, end: usize) -> Result<Vec<f32>> {
        let len = end - start;
        self.bytes.resize(len * 2, 0);
        let mut output = vec![0_f32; len];
        for (_, file) in &mut self.sources {
            let Some(file) = file else { continue };
            file.seek(SeekFrom::Start(start as u64 * 2))?;
            let mut used = 0;
            while used < self.bytes.len() {
                let read = file.read(&mut self.bytes[used..])?;
                if read == 0 {
                    break;
                }
                used += read;
            }
            for (sample, bytes) in output
                .iter_mut()
                .zip(self.bytes[..used - used % 2].chunks_exact(2))
            {
                *sample += i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32768.;
            }
        }
        output.iter_mut().for_each(|x| *x = x.clamp(-1., 1.));
        Ok(output)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Update {
    pub text: String,
    pub start: f64,
    /// Absolute audio clock at the end of the fed frames. This is not a word end.
    pub end: f64,
    /// Last word end reported by the native decoder, when it exposes word times.
    /// QA latency must use this anchor instead of `end`/frame size.
    #[serde(default)]
    pub speech_end: Option<f64>,
    pub final_text: bool,
    pub words: Vec<crate::types::Segment>,
}
type Ptr = *mut c_void;
type Release = unsafe extern "C" fn(Ptr);
type ReadResult = unsafe extern "C" fn(Ptr) -> *const c_char;
struct VoskDecoder {
    // Keep the library loaded until both native objects have been freed.
    _library: libloading::Library,
    model: Ptr,
    recognizer: Ptr,
    free_model: Release,
    free_recognizer: Release,
    accept: unsafe extern "C" fn(Ptr, *const f32, i32) -> i32,
    result: ReadResult,
    partial: ReadResult,
    finish: ReadResult,
    new_recognizer: unsafe extern "C" fn(Ptr, f32) -> Ptr,
    set_words: unsafe extern "C" fn(Ptr, i32),
    base: f64,
    cursor: f64,
    utterance_start: f64,
}
impl VoskDecoder {
    fn new(model: &Path) -> Result<Self> {
        let dll = std::env::current_exe()?
            .parent()
            .context("Executable directory")?
            .join("libvosk.dll");
        // Absolute DLL path and safe dependency search; never search the working directory.
        let library: libloading::Library = unsafe {
            libloading::os::windows::Library::load_with_flags(&dll, 0x00000100 | 0x00001000)?
        }
        .into();
        unsafe {
            let load =
                *library.get::<unsafe extern "C" fn(*const c_char) -> Ptr>(b"vosk_model_new\0")?;
            let new =
                *library.get::<unsafe extern "C" fn(Ptr, f32) -> Ptr>(b"vosk_recognizer_new\0")?;
            let free_model = *library.get::<Release>(b"vosk_model_free\0")?;
            let free_recognizer = *library.get::<Release>(b"vosk_recognizer_free\0")?;
            let accept = *library.get(b"vosk_recognizer_accept_waveform_f\0")?;
            let result = *library.get(b"vosk_recognizer_result\0")?;
            let partial = *library.get(b"vosk_recognizer_partial_result\0")?;
            let finish = *library.get(b"vosk_recognizer_final_result\0")?;
            let words =
                *library.get::<unsafe extern "C" fn(Ptr, i32)>(b"vosk_recognizer_set_words\0")?;
            let log = *library.get::<unsafe extern "C" fn(i32)>(b"vosk_set_log_level\0")?;
            log(-1);
            // Vosk's MinGW narrow fopen cannot open an absolute UTF-8 Windows
            // path. The isolated child uses a Unicode-safe working directory and
            // an ASCII relative model name; Rust keeps capture paths absolute.
            std::env::set_current_dir(model.parent().context("Streaming model directory")?)?;
            let name = model
                .file_name()
                .and_then(|n| n.to_str())
                .context("Streaming model name")?;
            anyhow::ensure!(name.is_ascii(), "Tên model streaming phải là ASCII");
            let path = CString::new(name)?;
            let model = load(path.as_ptr());
            anyhow::ensure!(!model.is_null(), "Không mở được model streaming local");
            let recognizer = new(model, 16000.);
            if recognizer.is_null() {
                free_model(model);
                anyhow::bail!("Không tạo được bộ chép trực tiếp");
            }
            words(recognizer, 1);
            Ok(Self {
                _library: library,
                model,
                recognizer,
                free_model,
                free_recognizer,
                accept,
                result,
                partial,
                finish,
                new_recognizer: new,
                set_words: words,
                base: 0.,
                cursor: 0.,
                utterance_start: 0.,
            })
        }
    }
    fn decode(&mut self, samples: &[f32], finish: bool) -> Result<Update> {
        // Vosk's float API expects PCM amplitude [-32768,32767], not normalized audio.
        let pcm: Vec<f32> = samples.iter().map(|x| x * 32768.).collect();
        let status = if finish {
            1
        } else {
            unsafe { (self.accept)(self.recognizer, pcm.as_ptr(), pcm.len() as i32) }
        };
        anyhow::ensure!(status >= 0, "Bộ chép trực tiếp không giải mã được audio");
        self.cursor += samples.len() as f64 / 16000.;
        let reader = if finish {
            self.finish
        } else if status == 1 {
            self.result
        } else {
            self.partial
        };
        let ptr = unsafe { reader(self.recognizer) };
        anyhow::ensure!(!ptr.is_null(), "Bộ chép trực tiếp trả kết quả rỗng");
        let value: serde_json::Value =
            serde_json::from_slice(unsafe { CStr::from_ptr(ptr).to_bytes() })?;
        let final_text = finish || status == 1;
        let text = value[if final_text { "text" } else { "partial" }]
            .as_str()
            .unwrap_or("")
            .trim()
            .to_owned();
        let start = value["result"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["start"].as_f64())
            .map(|t| t + self.base)
            .unwrap_or(self.utterance_start);
        let end = value["result"]
            .as_array()
            .and_then(|a| a.last())
            .and_then(|v| v["end"].as_f64())
            .map(|t| t + self.base)
            .unwrap_or(self.cursor);
        if final_text {
            self.utterance_start = self.cursor;
        }
        let words: Vec<crate::types::Segment> = {
            value[if final_text {
                "result"
            } else {
                "partial_result"
            }]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .filter_map(|word| {
                        let start = (word["start"].as_f64()? + self.base).max(self.base);
                        let end = (word["end"].as_f64()? + self.base).min(self.cursor);
                        let text = word["word"].as_str()?.trim();
                        (start.is_finite() && end.is_finite() && end > start && !text.is_empty())
                            .then(|| crate::types::Segment {
                                id: uuid::Uuid::new_v4().to_string(),
                                start,
                                end,
                                text: text.into(),
                                edited: false,
                            })
                    })
                    .collect()
            })
            .unwrap_or_default()
        };
        let speech_end = words.last().map(|w| w.end);
        Ok(Update {
            text,
            start,
            end,
            speech_end,
            final_text,
            words,
        })
    }
    fn reset_at(&mut self, at: f64) -> Result<()> {
        // Vosk reset() preserves its feature-pipeline clock. Create a fresh
        // recognizer sharing the loaded model before applying a timeline offset.
        unsafe {
            let next = (self.new_recognizer)(self.model, 16000.);
            anyhow::ensure!(!next.is_null(), "Không khởi tạo lại được streaming");
            (self.set_words)(next, 1);
            (self.free_recognizer)(self.recognizer);
            self.recognizer = next;
        }
        self.cursor = at;
        self.base = at;
        self.utterance_start = at;
        Ok(())
    }
}
impl Drop for VoskDecoder {
    fn drop(&mut self) {
        unsafe {
            (self.free_recognizer)(self.recognizer);
            (self.free_model)(self.model);
        }
    }
}
enum Decoder {
    Moonshine(crate::moonshine::Decoder),
    Multilingual(crate::nemo::Decoder),
    Legacy(VoskDecoder),
}
impl Decoder {
    fn new(model: &Path, language: &str) -> Result<Self> {
        if model.extension().is_some_and(|s| s == "gguf") {
            return Ok(Self::Multilingual(crate::nemo::Decoder::new(
                model, language,
            )?));
        }
        if model.join("streaming_config.json").exists() {
            Ok(Self::Moonshine(crate::moonshine::Decoder::new(model)?))
        } else {
            Ok(Self::Legacy(VoskDecoder::new(model)?))
        }
    }
    fn decode(&mut self, samples: &[f32], finish: bool) -> Result<Vec<Update>> {
        match self {
            Self::Moonshine(d) => d.decode(samples, finish),
            Self::Multilingual(d) => d.decode(samples, finish),
            Self::Legacy(d) => Ok(vec![d.decode(samples, finish)?]),
        }
    }
    fn reset_at(&mut self, at: f64) -> Result<()> {
        match self {
            Self::Moonshine(d) => d.reset_at(at),
            Self::Multilingual(d) => d.reset_at(at),
            Self::Legacy(d) => d.reset_at(at),
        }
    }
}
fn emit_updates(updates: Vec<Update>, previous: &mut String) -> Result<()> {
    for update in updates {
        if update.final_text || update.text != *previous {
            *previous = if update.final_text {
                String::new()
            } else {
                update.text.clone()
            };
            let mut value = serde_json::to_value(&update)?;
            if std::env::var("MAY_LATENCY_AUDIT").as_deref() == Ok("1") {
                value["worker_unix_ms"] = serde_json::json!(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs_f64()
                        * 1000.
                );
            }
            println!("{}", value);
            std::io::stdout().flush()?;
        }
    }
    Ok(())
}
pub fn model_path(language: &str) -> Result<std::path::PathBuf> {
    anyhow::ensure!(
        ["vi", "en", "ko", "zh", "multi"].contains(&language),
        "Chép trực tiếp hỗ trợ Việt, Anh, Hàn, Trung hoặc đa ngôn ngữ"
    );
    if ["multi", "ko", "zh"].contains(&language) {
        return crate::models::path(&crate::storage::root(), crate::models::NEMO_ID);
    }
    let exe = std::env::current_exe()?;
    let modern = exe
        .parent()
        .context("Executable directory")?
        .join("live-models")
        .join(language);
    if modern.is_dir() {
        return Ok(modern);
    }
    let name = match language {
        "vi" => "vosk-model-small-vn-0.4",
        "en" => "vosk-model-small-en-us-0.15",
        _ => anyhow::bail!("Chép trực tiếp hỗ trợ Việt, Anh, Hàn, Trung hoặc đa ngôn ngữ"),
    };
    Ok(std::env::current_exe()?
        .parent()
        .context("Executable directory")?
        .join("streaming-models")
        .join(name))
}
/// Child tails the durable PCM, so no unbounded queue and no waiting for Whisper.
/// EOF on stdin means capture has joined; drain the remaining samples, then flush.
pub fn worker(language: &str, dir: &Path) -> Result<()> {
    let dir = std::fs::canonicalize(dir)?;
    let mut decoder = Decoder::new(&model_path(language)?, language)?;
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = done.clone();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = std::io::stdin().read_line(&mut line);
        signal.store(true, std::sync::atomic::Ordering::Release);
    });
    let mut cursor = 0usize;
    let mut previous = String::new();
    let mut zeros = 0usize;
    let mut skipped_silence = false;
    let multilingual = ["multi", "ko", "zh"].contains(&language);
    let boundary_samples = 16000 * if multilingual { 30 } else { 20 };
    let mut next_boundary = boundary_samples;
    let mut pcm = PcmTail::new(&dir);
    println!("{}", serde_json::json!({"ready":true}));
    std::io::stdout().flush()?;
    loop {
        let stopped = done.load(std::sync::atomic::Ordering::Acquire);
        let available = pcm.available()?;
        // Capture writes every ~10 ms. Keep one 20 ms frame for endpoint skew.
        let available = if stopped {
            available
        } else {
            available.saturating_sub(320)
        };
        if available > cursor && (stopped || available - cursor >= 320) {
            let end = available.min(cursor + 320);
            let samples = pcm.read(cursor, end)?;
            // Capture pads paused/disconnected ranges with exact zeros. Flush once,
            // then skip padded silence without decoding minutes of it on resume.
            zeros = if samples.iter().all(|x| *x == 0.) {
                zeros + samples.len()
            } else {
                0
            };
            if skipped_silence && zeros > 0 {
                cursor = end;
                continue;
            }
            if skipped_silence {
                decoder.reset_at(cursor as f64 / 16000.)?;
                if multilingual {
                    next_boundary = cursor + boundary_samples;
                }
                skipped_silence = false;
            }
            let updates = decoder.decode(&samples, false)?;
            cursor = end;
            if multilingual && updates.iter().any(|u| u.final_text) {
                next_boundary = cursor + boundary_samples;
            }
            emit_updates(updates, &mut previous)?;
            // Align forced endpoints with Whisper's 20-second watermark. This
            // prevents a long partial from repeating an already refined prefix.
            if zeros >= 16000 || cursor >= next_boundary {
                emit_updates(decoder.decode(&[], true)?, &mut previous)?;
                decoder.reset_at(cursor as f64 / 16000.)?;
                skipped_silence = zeros >= 16000;
                next_boundary = if multilingual {
                    cursor + boundary_samples
                } else {
                    (cursor / boundary_samples + 1) * boundary_samples
                };
                previous.clear();
            }
        } else if stopped {
            emit_updates(decoder.decode(&[], true)?, &mut previous)?;
            break;
        } else {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    Ok(())
}

/// Deterministic fixture driver, paced by the audio clock rather than fed in a burst.
pub fn benchmark(language: &str, path: &Path) -> Result<()> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    anyhow::ensure!(
        spec.sample_rate == 16000 && spec.channels == 1 && spec.bits_per_sample == 16,
        "Cần WAV PCM16 mono 16kHz"
    );
    let mut decoder = Decoder::new(&model_path(language)?, language)?;
    let start = std::time::Instant::now();
    let samples = reader
        .samples::<i16>()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut count = 0usize;
    let mut previous = String::new();
    for frame in samples.chunks(320) {
        count += frame.len();
        let target = std::time::Duration::from_secs_f64(count as f64 / 16000.);
        if let Some(wait) = target.checked_sub(start.elapsed()) {
            std::thread::sleep(wait);
        }
        let normalized: Vec<_> = frame.iter().map(|x| *x as f32 / 32768.).collect();
        let tick = std::time::Instant::now();
        let updates = decoder.decode(&normalized, false)?;
        for update in updates {
            if update.final_text || update.text != previous {
                previous = if update.final_text {
                    String::new()
                } else {
                    update.text.clone()
                };
                println!(
                    "{}",
                    serde_json::json!({"wall_ms":start.elapsed().as_millis(),"audio_ms":count as u64 * 1000 /16000,"decode_ms":tick.elapsed().as_millis(),"update":update})
                );
                std::io::stdout().flush()?;
            }
        }
    }
    for update in decoder.decode(&[], true)? {
        println!(
            "{}",
            serde_json::json!({"wall_ms":start.elapsed().as_millis(),"audio_ms":count as u64 *1000/16000,"update":update})
        );
    }
    Ok(())
}

#[cfg(test)]
mod tail_tests {
    use super::*;
    #[test]
    fn open_writer_growth_late_endpoint_padding_and_mix_are_visible() {
        let dir = std::env::temp_dir().join(format!("may-tail-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        let mut tail = PcmTail::new(&dir);
        assert_eq!(tail.available().unwrap(), 0);
        let mut system = File::create(dir.join("system.pcm")).unwrap();
        let encode = |v: &[i16]| v.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<_>>();
        system.write_all(&encode(&[32000, -32000, 1000])).unwrap();
        assert_eq!(tail.available().unwrap(), 3);
        assert_eq!(&tail.read(0, 6).unwrap()[3..], &[0., 0., 0.]);
        let mut microphone = File::create(dir.join("microphone.pcm")).unwrap();
        microphone.write_all(&encode(&[32000, -32000])).unwrap();
        system.write_all(&encode(&[2000, 3000, 4000])).unwrap();
        assert_eq!(tail.available().unwrap(), 6);
        assert_eq!(
            tail.read(0, 6).unwrap(),
            crate::audio::mix(&dir, 0., 6. / 16000.).unwrap()
        );
        assert_eq!(
            tail.read(1, 6).unwrap(),
            crate::audio::mix(&dir, 1. / 16000., 6. / 16000.).unwrap()
        );
        drop(tail);
        drop(system);
        drop(microphone);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
