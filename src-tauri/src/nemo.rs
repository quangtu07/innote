//! Isolated native multilingual recognition, pinned to NeMo-Speech.cpp 0.2.0.
use crate::{
    live::Update,
    types::{Segment, WorkerJob, WorkerResult},
};
use anyhow::{Context, Result};
use std::{
    ffi::{c_char, c_void, CStr, CString},
    io::{BufRead, Write},
    path::Path,
    time::Instant,
};
#[repr(C)]
struct Backend {
    size: usize,
    gpu: i32,
}
#[repr(C)]
struct Model {
    size: usize,
    path: *const c_char,
    name: *const c_char,
}
#[repr(C)]
struct Streaming {
    size: usize,
    chunk: f32,
    left: f32,
    right: f32,
    context: i32,
}
#[repr(C)]
struct Endpoint {
    size: usize,
    enable: bool,
    vad_based: bool,
    silence_ms: i32,
}
#[repr(C)]
struct Vad {
    size: usize,
    path: *const c_char,
    masking: bool,
    onset: f32,
    offset: f32,
}
#[repr(C)]
struct Config {
    size: usize,
    backend: *const Backend,
    model: *const Model,
    streaming: *const Streaming,
    decoder: *const c_void,
    vad: *const c_void,
    endpoint: *const Endpoint,
    postproc: *const c_void,
    diar: *const c_void,
    batching: *const c_void,
}
#[repr(C)]
struct Options {
    size: usize,
    request: *const c_char,
    language: *const c_char,
    interim: bool,
    word_times: bool,
    punctuation: bool,
    verbatim: bool,
    profanity: bool,
    eou: i32,
    contexts: *const c_void,
    context_count: usize,
    max_alternatives: i32,
    diarize: bool,
    max_speakers: i32,
}
type Ptr = *mut c_void;
type StreamStart = unsafe extern "C" fn(Ptr, *const Options, *mut Ptr) -> i32;
type Release = unsafe extern "C" fn(Ptr);
pub fn language(code: &str) -> Result<&'static [u8]> {
    Ok(match code {
        "multi" | "auto" => b"auto\0",
        "vi" => b"vi-VN\0",
        "en" => b"en-US\0",
        "ko" => b"ko-KR\0",
        "zh" => b"zh-CN\0",
        _ => anyhow::bail!("Ngôn ngữ đa ngôn ngữ không hợp lệ"),
    })
}
pub struct Decoder {
    // ggml/VCOMP keep process-wide backend state after recognizer destruction.
    // This isolated child pins the DLL until process exit; unloading it while
    // those backend threads/statics exist can crash during teardown.
    _library: std::mem::ManuallyDrop<libloading::Library>,
    recognizer: Ptr,
    stream: Ptr,
    defaults: unsafe extern "C" fn() -> Options,
    start: StreamStart,
    push: unsafe extern "C" fn(Ptr, *const f32, usize, i32) -> i32,
    next: unsafe extern "C" fn(Ptr, *mut Ptr) -> i32,
    finish_stream: unsafe extern "C" fn(Ptr) -> i32,
    close_stream: Release,
    destroy: Release,
    free_result: Release,
    text: unsafe extern "C" fn(Ptr, usize) -> *const c_char,
    is_final: unsafe extern "C" fn(Ptr) -> bool,
    error: unsafe extern "C" fn() -> *const c_char,
    word_count: unsafe extern "C" fn(Ptr, usize) -> usize,
    word_start: unsafe extern "C" fn(Ptr, usize, usize) -> i32,
    word_end: unsafe extern "C" fn(Ptr, usize, usize) -> i32,
    code: String,
    base: f64,
    stream_origin: f64,
    cursor: f64,
    tail: std::collections::VecDeque<f32>,
    history: std::collections::VecDeque<f32>,
    buffered_audio: bool,
    finished: bool,
}
impl Decoder {
    pub fn new(model: &Path, code: &str) -> Result<Self> {
        anyhow::ensure!(
            std::is_x86_feature_detected!("avx2"),
            "Model đa ngôn ngữ này cần CPU AVX2"
        );
        language(code)?;
        let model = std::fs::canonicalize(model)
            .context("Chưa có model đa ngôn ngữ; hãy tải trong ứng dụng")?;
        let runtime = std::env::current_exe()?
            .parent()
            .context("Executable directory")?
            .join("nemo-runtime");
        let library: libloading::Library = unsafe {
            libloading::os::windows::Library::load_with_flags(
                runtime.join("nemo_speech_asr_c.dll"),
                0x100 | 0x1000,
            )?
        }
        .into();
        macro_rules! bind {
            ($name:literal,$ty:ty) => {
                *library.get::<$ty>(concat!("nemo_speech_asr_", $name, "\0").as_bytes())?
            };
        }
        unsafe {
            let create = bind!(
                "create",
                unsafe extern "C" fn(*const Config, *mut Ptr) -> i32
            );
            let defaults = bind!(
                "recognition_options_default",
                unsafe extern "C" fn() -> Options
            );
            let start = bind!("streaming_recognize", StreamStart);
            let push = bind!(
                "stream_push_f32",
                unsafe extern "C" fn(Ptr, *const f32, usize, i32) -> i32
            );
            let next = bind!("stream_next", unsafe extern "C" fn(Ptr, *mut Ptr) -> i32);
            let finish_stream = bind!("stream_finish", unsafe extern "C" fn(Ptr) -> i32);
            let close_stream = bind!("stream_close", Release);
            let destroy = bind!("destroy", Release);
            let free_result = bind!("result_destroy", Release);
            let text = bind!(
                "result_transcript",
                unsafe extern "C" fn(Ptr, usize) -> *const c_char
            );
            let is_final = bind!("result_is_final", unsafe extern "C" fn(Ptr) -> bool);
            let error = bind!("last_error", unsafe extern "C" fn() -> *const c_char);
            let word_count = bind!(
                "result_word_count",
                unsafe extern "C" fn(Ptr, usize) -> usize
            );
            let word_start = bind!(
                "result_word_start_time",
                unsafe extern "C" fn(Ptr, usize, usize) -> i32
            );
            let word_end = bind!(
                "result_word_end_time",
                unsafe extern "C" fn(Ptr, usize, usize) -> i32
            );
            // Native narrow opens remain ASCII; only this child changes its working dir.
            std::env::set_current_dir(model.parent().context("Model parent")?)?;
            let name = model
                .file_name()
                .and_then(|s| s.to_str())
                .context("Model name")?;
            anyhow::ensure!(name.is_ascii(), "Tên file model phải là ASCII");
            let name = CString::new(name)?;
            let backend = Backend {
                size: std::mem::size_of::<Backend>(),
                gpu: -1,
            };
            let model = Model {
                size: std::mem::size_of::<Model>(),
                path: name.as_ptr(),
                name: std::ptr::null(),
            };
            let streaming = Streaming {
                size: std::mem::size_of::<Streaming>(),
                chunk: 0.16,
                left: 1.92,
                right: 1.92,
                context: 1,
            };
            // GGUF's narrow file open cannot resolve a Unicode Windows path.
            // Rust reads the bundled sidecar and publishes it in the writable
            // model directory, then native code opens an ASCII relative name.
            let vad_name = "silero-v5.1.2.gguf";
            let vad_bytes = std::fs::read(runtime.join(vad_name))?;
            let vad_dest = std::path::PathBuf::from(vad_name);
            if std::fs::read(&vad_dest).ok().as_deref() != Some(vad_bytes.as_slice()) {
                let partial = format!("silero-{}.partial", uuid::Uuid::new_v4());
                std::fs::write(&partial, &vad_bytes)?;
                if let Err(e) = std::fs::rename(&partial, &vad_dest) {
                    let _ = std::fs::remove_file(&partial);
                    anyhow::ensure!(
                        std::fs::read(&vad_dest).ok().as_deref() == Some(vad_bytes.as_slice()),
                        "Không chuẩn bị được VAD: {e}"
                    );
                }
            }
            let vad_path = CString::new(vad_name)?;
            let vad = Vad {
                size: std::mem::size_of::<Vad>(),
                path: vad_path.as_ptr(),
                masking: false,
                onset: 0.5,
                offset: 0.35,
            };
            let endpoint = Endpoint {
                size: std::mem::size_of::<Endpoint>(),
                enable: true,
                vad_based: true,
                silence_ms: 400,
            };
            let cfg = Config {
                size: std::mem::size_of::<Config>(),
                backend: &backend,
                model: &model,
                streaming: &streaming,
                decoder: std::ptr::null(),
                vad: (&vad as *const Vad).cast(),
                endpoint: &endpoint,
                postproc: std::ptr::null(),
                diar: std::ptr::null(),
                batching: std::ptr::null(),
            };
            let mut recognizer = std::ptr::null_mut();
            let status = create(&cfg, &mut recognizer);
            if status != 0 {
                let msg = error();
                anyhow::bail!(
                    "Không mở được model đa ngôn ngữ: {}",
                    if msg.is_null() {
                        "native error".into()
                    } else {
                        CStr::from_ptr(msg).to_string_lossy()
                    }
                );
            }
            let mut d = Self {
                _library: std::mem::ManuallyDrop::new(library),
                recognizer,
                stream: std::ptr::null_mut(),
                defaults,
                start,
                push,
                next,
                finish_stream,
                close_stream,
                destroy,
                free_result,
                text,
                is_final,
                error,
                word_count,
                word_start,
                word_end,
                code: code.into(),
                base: 0.,
                stream_origin: 0.,
                cursor: 0.,
                tail: std::collections::VecDeque::with_capacity(5120),
                history: std::collections::VecDeque::new(),
                buffered_audio: false,
                finished: false,
            };
            d.reset_language(code, 0.)?;
            Ok(d)
        }
    }
    fn check(&self, status: i32) -> Result<()> {
        if status != 0 {
            let p = unsafe { (self.error)() };
            anyhow::bail!(
                "Nhận dạng đa ngôn ngữ lỗi: {}",
                if p.is_null() {
                    "native error".into()
                } else {
                    unsafe { CStr::from_ptr(p) }.to_string_lossy()
                }
            );
        }
        Ok(())
    }
    pub fn reset_language(&mut self, code: &str, at: f64) -> Result<()> {
        let language = language(code)?;
        if !self.stream.is_null() {
            unsafe { (self.close_stream)(self.stream) };
            self.stream = std::ptr::null_mut();
        }
        let mut options = unsafe { (self.defaults)() };
        anyhow::ensure!(
            options.size == std::mem::size_of::<Options>(),
            "SDK đa ngôn ngữ không khớp ABI"
        );
        options.language = language.as_ptr().cast();
        options.interim = true;
        options.word_times = true;
        let status = unsafe { (self.start)(self.recognizer, &options, &mut self.stream) };
        self.check(status)?;
        self.code = code.into();
        self.base = at;
        self.stream_origin = at;
        self.cursor = at;
        self.tail.clear();
        self.history.clear();
        self.buffered_audio = false;
        self.finished = false;
        Ok(())
    }
    pub fn reset_at(&mut self, at: f64) -> Result<()> {
        let code = self.code.clone();
        self.reset_language(&code, at)
    }
    pub fn decode(&mut self, samples: &[f32], finish: bool) -> Result<Vec<Update>> {
        if self.finished && self.stream.is_null() && samples.is_empty() {
            return Ok(vec![]);
        }
        anyhow::ensure!(
            !self.finished || samples.is_empty(),
            "Luồng đa ngôn ngữ đã dừng"
        );
        // Fresh streams need no neural inference for digital silence. Keep the
        // absolute clock, including paused ranges, and preserve every nonzero
        // sample (even very quiet speech/noise) when the first audio arrives.
        if !self.buffered_audio && samples.iter().all(|x| *x == 0.) {
            self.cursor += samples.len() as f64 / 16000.;
            self.base = self.cursor;
            self.stream_origin = self.cursor;
            if finish {
                self.finished = true;
                return Ok(vec![Update {
                    text: String::new(),
                    start: self.cursor,
                    end: self.cursor,
                    speech_end: None,
                    final_text: true,
                    words: vec![],
                }]);
            }
            return Ok(vec![]);
        }
        if !samples.is_empty() {
            self.check(unsafe {
                (self.push)(self.stream, samples.as_ptr(), samples.len(), 16000)
            })?;
            self.cursor += samples.len() as f64 / 16000.;
            self.buffered_audio = true;
            self.tail.extend(samples.iter().copied());
            let excess = self.tail.len().saturating_sub(5120);
            self.tail.drain(..excess);
            self.history.extend(samples.iter().copied());
            let excess = self.history.len().saturating_sub(32 * 16000);
            self.history.drain(..excess);
        }
        if finish && !self.finished {
            self.check(unsafe { (self.finish_stream)(self.stream) })?;
            self.finished = true;
        }
        let mut updates = vec![];
        let mut repairs = vec![];
        let mut ended = false;
        for _ in 0..1000 {
            let mut result = std::ptr::null_mut();
            self.check(unsafe { (self.next)(self.stream, &mut result) })?;
            if result.is_null() {
                break;
            }
            let p = unsafe { (self.text)(result, 0) };
            let text = if p.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(p) }
                    .to_string_lossy()
                    .trim()
                    .to_owned()
            };
            let complete = unsafe { (self.is_final)(result) };
            let times = (0..unsafe { (self.word_count)(result, 0) }.min(10000)).map(|i| unsafe {
                (
                    (self.word_start)(result, 0, i),
                    (self.word_end)(result, 0, i),
                )
            });
            let bounds = timing_bounds(self.stream_origin, self.base, self.cursor, times);
            unsafe { (self.free_result)(result) };
            // A known auto-LID failure emits a long repeated Thai character
            // inside an otherwise Chinese utterance. Retry only this anomaly
            // locally with a Chinese prompt; retain the original on any failure.
            if complete
                && ["multi", "auto", "zh"].contains(&self.code.as_str())
                && repeated_thai_in_chinese(&text)
            {
                repairs.push((
                    updates.len(),
                    self.stream_origin.max(self.base),
                    self.cursor,
                ));
            }
            let words = if complete && !text.is_empty() && self.cursor > self.base {
                vec![Segment {
                    id: uuid::Uuid::new_v4().to_string(),
                    start: bounds.0,
                    end: bounds.1,
                    text: text.clone(),
                    edited: false,
                }]
            } else {
                vec![]
            };
            if complete && !text.is_empty() {
                self.base = bounds.1;
            }
            updates.push(Update {
                text,
                start: bounds.0,
                end: self.cursor,
                speech_end: bounds.2,
                final_text: complete,
                words,
            });
            ended |= complete;
        }
        if !repairs.is_empty() {
            // Drain the old result queue before closing its handle. Repair runs
            // after that stream has released its native predictor/cache state.
            unsafe { (self.close_stream)(self.stream) };
            self.stream = std::ptr::null_mut();
            for (index, start, end) in repairs {
                if let Some(text) = self.retry_chinese(start, end) {
                    updates[index].text = text.clone();
                    for segment in &mut updates[index].words {
                        segment.text = text.clone();
                    }
                }
            }
        }
        // EOU clears text but keeps the encoder's old-language context. A fresh
        // stream is essential when the next participant speaks a different language.
        if ended && !finish {
            if ["multi", "auto"].contains(&self.code.as_str()) || self.stream.is_null() {
                // The native encoder keeps lookahead beyond an EOU. Closing it
                // without replay discards the next turn's onset. Retain 320 ms,
                // including real samples, and keep the absolute audio clock.
                let cursor = self.cursor;
                let committed = self.base;
                let tail: Vec<_> = self.tail.iter().copied().collect();
                self.reset_at(cursor - tail.len() as f64 / 16000.)?;
                if !tail.is_empty() {
                    self.check(unsafe {
                        (self.push)(self.stream, tail.as_ptr(), tail.len(), 16000)
                    })?;
                    self.cursor = cursor;
                    self.buffered_audio = true;
                    self.tail.extend(tail);
                    self.history.extend(self.tail.iter().copied());
                }
                self.base = committed;
            }
        }
        if finish && updates.is_empty() {
            updates.push(Update {
                text: String::new(),
                start: self.cursor,
                end: self.cursor,
                speech_end: None,
                final_text: true,
                words: vec![],
            });
        }
        Ok(updates)
    }
    fn retry_chinese(&self, start: f64, end: f64) -> Option<String> {
        let origin = self.cursor - self.history.len() as f64 / 16000.;
        if start + 1. / 16000. < origin {
            return None;
        }
        let from = (((start - 0.16).max(origin) - origin) * 16000.).floor() as usize;
        let to = (((end + 0.16).min(self.cursor) - origin) * 16000.).ceil() as usize;
        let pcm: Vec<_> = self
            .history
            .iter()
            .skip(from)
            .take(to.saturating_sub(from))
            .copied()
            .collect();
        if std::env::var("MAY_LATENCY_AUDIT").as_deref() == Ok("1") {
            let mut run = 0;
            let mut gap = None;
            for (i, x) in pcm.iter().enumerate() {
                run = if *x == 0. { run + 1 } else { 0 };
                if run >= 2560 {
                    gap = Some(i + 1 - run);
                    break;
                }
            }
            eprintln!("nemo repair-window: cursor={:.3} origin={:.3} start={:.3} end={:.3} from={} to={} samples={} first_gap={:?}",self.cursor,origin,start,end,from,to,pcm.len(),gap);
        }
        // Added digital silence changes the encoder's initial frame phase and
        // can reproduce the same hallucination. Trim only exact zeros, never
        // a volume threshold: even quiet nonzero speech must be retained.
        let pcm = repair_window(&pcm)?;
        let mut options = unsafe { (self.defaults)() };
        options.language = b"zh-CN\0".as_ptr().cast();
        options.interim = false;
        options.word_times = false;
        let mut stream = std::ptr::null_mut();
        if unsafe { (self.start)(self.recognizer, &options, &mut stream) } != 0 || stream.is_null()
        {
            return None;
        }
        // Keep the same streaming execution policy. The SDK's offline runner
        // chooses a different attention window and did not repair this case.
        let repaired = (|| {
            let mut text = String::new();
            let collect = |text: &mut String| -> Option<()> {
                for _ in 0..1000 {
                    let mut result = std::ptr::null_mut();
                    if unsafe { (self.next)(stream, &mut result) } != 0 {
                        return None;
                    }
                    if result.is_null() {
                        return Some(());
                    }
                    if unsafe { (self.is_final)(result) } {
                        let p = unsafe { (self.text)(result, 0) };
                        if !p.is_null() {
                            text.push_str(unsafe { CStr::from_ptr(p) }.to_string_lossy().trim());
                        }
                    }
                    unsafe { (self.free_result)(result) };
                }
                None
            };
            for frame in pcm.chunks(320) {
                if unsafe { (self.push)(stream, frame.as_ptr(), frame.len(), 16000) } != 0 {
                    return None;
                }
                collect(&mut text)?;
            }
            if unsafe { (self.finish_stream)(stream) } != 0 {
                return None;
            }
            collect(&mut text)?;
            Some(text)
        })();
        unsafe { (self.close_stream)(stream) };
        let repaired = repaired?;
        if std::env::var("MAY_LATENCY_AUDIT").as_deref() == Ok("1") {
            eprintln!(
                "nemo repair: samples={} result_chars={} repeated_thai={}",
                pcm.len(),
                repaired.chars().count(),
                repeated_thai_in_chinese(&repaired)
            );
        }
        (!repaired.is_empty()
            && repaired
                .chars()
                .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c))
            && !repaired
                .chars()
                .any(|c| ('\u{0e00}'..='\u{0e7f}').contains(&c)))
        .then_some(repaired)
    }
}
fn repeated_thai_in_chinese(text: &str) -> bool {
    if text
        .chars()
        .any(|c| c.is_ascii_alphabetic() || ('\u{ac00}'..='\u{d7af}').contains(&c))
    {
        return false;
    }
    if !text.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return false;
    }
    let mut previous = '\0';
    let mut run = 0;
    for c in text.chars() {
        run = if c == previous { run + 1 } else { 1 };
        previous = c;
        if run >= 2 && ('\u{0e00}'..='\u{0e7f}').contains(&c) {
            return true;
        }
    }
    false
}
fn trim_digital_silence(samples: &[f32]) -> Option<&[f32]> {
    let start = samples.iter().position(|x| *x != 0.)?;
    let end = samples.iter().rposition(|x| *x != 0.)? + 1;
    Some(&samples[start..end])
}
fn repair_window(samples: &[f32]) -> Option<&[f32]> {
    // A replayed prefix can contain the previous participant's final phoneme,
    // followed by a digital gap and then the new speaker. Discard that prefix
    // only inside replay320 + native lookahead160 ms, across >=160 ms exact zeros.
    // Never seek a later pause in the current utterance or threshold quiet speech.
    let mut zero_start = None;
    let mut from = 0;
    for (i, sample) in samples.iter().enumerate() {
        if *sample == 0. {
            if zero_start.is_none() {
                if i > 7680 {
                    break;
                }
                zero_start = Some(i);
            }
        } else if let Some(start) = zero_start.take() {
            if i - start >= 2560 {
                from = i;
                break;
            }
        }
    }
    trim_digital_silence(&samples[from..])
}
fn timing_bounds(
    origin: f64,
    floor: f64,
    cursor: f64,
    times: impl Iterator<Item = (i32, i32)>,
) -> (f64, f64, Option<f64>) {
    let mut start = cursor;
    let mut end = floor;
    let mut speech_end: Option<f64> = None;
    for (a, b) in times {
        if a >= 0 && b > a && origin + b as f64 / 1000. > floor {
            start = start.min((origin + a as f64 / 1000.).max(floor));
            let word_end = (origin + b as f64 / 1000.).min(cursor);
            end = end.max(word_end);
            speech_end = Some(speech_end.map_or(word_end, |previous: f64| previous.max(word_end)));
        }
    }
    if end > start {
        (start, end, speech_end)
    } else {
        (floor, cursor, None)
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            if !self.stream.is_null() {
                (self.close_stream)(self.stream)
            };
            (self.destroy)(self.recognizer);
        }
    }
}
pub fn worker(model: &Path) -> Result<()> {
    let mut decoder = Decoder::new(model, "multi")?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let timer = Instant::now();
        let result = (|| -> Result<Vec<Segment>> {
            let job: WorkerJob = serde_json::from_str(&line)?;
            anyhow::ensure!(
                job.offset.is_finite() && job.offset >= 0.,
                "Mốc audio không hợp lệ"
            );
            decoder.reset_language(&job.language, job.offset)?;
            let mut wav = hound::WavReader::open(&job.path)?;
            let spec = wav.spec();
            anyhow::ensure!(
                spec.channels == 1 && spec.sample_rate == 16000 && spec.bits_per_sample == 16,
                "Cần WAV PCM16 mono16kHz"
            );
            let mut segments = vec![];
            let mut samples = wav.samples::<i16>();
            loop {
                let frame = samples
                    .by_ref()
                    .take(320)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                if frame.is_empty() {
                    break;
                }
                let x: Vec<_> = frame.iter().map(|v| *v as f32 / 32768.).collect();
                for u in decoder.decode(&x, false)? {
                    if u.final_text {
                        segments.extend(u.words);
                    }
                }
            }
            for u in decoder.decode(&[], true)? {
                segments.extend(u.words);
            }
            Ok(segments)
        })();
        let (segments, error) = match result {
            Ok(s) => (s, None),
            Err(e) => (vec![], Some(e.to_string())),
        };
        let response = WorkerResult {
            segments,
            error,
            elapsed_ms: timer.elapsed().as_millis() as u64,
            vad_windows: 0,
            inference_windows: 1,
        };
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn abi_and_language_prompts() {
        assert_eq!(std::mem::size_of::<Backend>(), 16);
        assert_eq!(std::mem::size_of::<Model>(), 24);
        assert_eq!(std::mem::size_of::<Streaming>(), 24);
        assert_eq!(std::mem::size_of::<Endpoint>(), 16);
        assert_eq!(std::mem::size_of::<Vad>(), 32);
        assert_eq!(std::mem::size_of::<Config>(), 80);
        assert_eq!(std::mem::size_of::<Options>(), 72);
        for l in ["multi", "vi", "en", "ko", "zh"] {
            assert!(language(l).is_ok());
        }
        assert!(language("invalid").is_err());
    }
    #[test]
    fn native_times_keep_stream_offset_and_exclude_endpoint_silence() {
        assert_eq!(
            timing_bounds(10., 12., 16., [(500, 900), (2300, 3100)].into_iter()),
            (12.3, 13.1, Some(13.1))
        );
        assert_eq!(
            timing_bounds(10., 12., 16., [(-1, 900), (3000, 9000)].into_iter()),
            (13., 16., Some(16.))
        );
        assert_eq!(
            timing_bounds(10., 12., 16., [(0, 0)].into_iter()),
            (12., 16., None)
        );
    }
    #[test]
    fn repair_is_limited_to_observed_repeated_thai_chinese_anomaly() {
        assert!(repeated_thai_in_chinese("因为远离大陆นนนนนนนน来"));
        assert!(repeated_thai_in_chinese("因为远离大陆นน动物"));
        assert!(!repeated_thai_in_chinese("因为 ภาษาไทย"));
        assert!(!repeated_thai_in_chinese("Hello 안녕하세요 你好"));
        assert!(!repeated_thai_in_chinese("นนนนนนนน"));
    }
    #[test]
    fn repair_padding_trim_keeps_quiet_speech_and_internal_pauses() {
        assert_eq!(
            trim_digital_silence(&[0., 0., 0.00001, 0., -0.00001, 0.]),
            Some(&[0.00001, 0., -0.00001][..])
        );
        assert_eq!(trim_digital_silence(&[0., 0.]), None);
    }
    #[test]
    fn repair_discards_only_previous_replay_fragment_before_a_digital_gap() {
        let mut pcm = vec![0.00001; 6400];
        pcm.extend(vec![0.; 12800]);
        pcm.extend([0.00002, 0., 0.00003]);
        assert_eq!(repair_window(&pcm), Some(&[0.00002, 0., 0.00003][..]));
        let mut late = vec![0.00001; 9600];
        late.extend(vec![0.; 3000]);
        late.push(0.00002);
        assert_eq!(repair_window(&late), Some(late.as_slice()));
    }
}
