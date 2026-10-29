//! Native, offline streaming preview. The C ABI is pinned to Moonshine Voice 0.1.5.
//! This module only runs in the isolated audio worker, never in the GUI process.
use crate::{live::Update, types::Segment};
use anyhow::{Context, Result};
use std::{
    collections::HashSet,
    ffi::{c_char, c_void, CStr, CString},
    path::Path,
};

#[repr(C)]
struct OptionC {
    name: *const c_char,
    value: *const c_char,
}
#[repr(C)]
struct LineC {
    text: *const c_char,
    audio: *const f32,
    audio_count: usize,
    start: f32,
    duration: f32,
    id: u64,
    complete: i8,
    updated: i8,
    new: i8,
    text_changed: i8,
    speakers_changed: i8,
    speakers: *const c_void,
    speaker_count: u64,
    latency_ms: u32,
    words: *const c_void,
    word_count: u64,
}
#[repr(C)]
struct TranscriptC {
    lines: *const LineC,
    count: u64,
}
type StreamOp = unsafe extern "C" fn(i32, i32) -> i32;

pub struct Decoder {
    _library: libloading::Library,
    transcriber: i32,
    stream: i32,
    create: unsafe extern "C" fn(i32, u32) -> i32,
    start: StreamOp,
    stop: StreamOp,
    free_stream: StreamOp,
    free_transcriber: unsafe extern "C" fn(i32),
    add: unsafe extern "C" fn(i32, i32, *const f32, u64, i32, u32) -> i32,
    transcribe: unsafe extern "C" fn(i32, i32, u32, *mut *const TranscriptC) -> i32,
    error: unsafe extern "C" fn(i32) -> *const c_char,
    base: f64,
    cursor: f64,
    pending: usize,
    interval_samples: usize,
    pending_cost: std::time::Duration,
    completed: HashSet<u64>,
    finished: bool,
}
impl Decoder {
    pub fn new(model: &Path) -> Result<Self> {
        let dll = std::env::current_exe()?
            .parent()
            .context("Executable directory")?
            .join("moonshine.dll");
        let library: libloading::Library = unsafe {
            libloading::os::windows::Library::load_with_flags(&dll, 0x00000100 | 0x00001000)?
        }
        .into();
        unsafe {
            let load = *library.get::<unsafe extern "C" fn(
                *const c_char,
                u32,
                *const OptionC,
                u64,
                i32,
            ) -> i32>(b"moonshine_load_transcriber_from_files\0")?;
            let create = *library.get(b"moonshine_create_stream\0")?;
            let start = *library.get(b"moonshine_start_stream\0")?;
            let stop = *library.get(b"moonshine_stop_stream\0")?;
            let free_stream = *library.get(b"moonshine_free_stream\0")?;
            let free_transcriber = *library.get(b"moonshine_free_transcriber\0")?;
            let add = *library.get(b"moonshine_transcribe_add_audio_to_stream\0")?;
            let transcribe = *library.get(b"moonshine_transcribe_stream\0")?;
            let error = *library.get::<unsafe extern "C" fn(i32) -> *const c_char>(
                b"moonshine_error_to_string\0",
            )?;
            // Keep all narrow native opens relative to an ASCII model basename.
            // The child's PCM path was canonicalized before changing its cwd.
            std::env::set_current_dir(model.parent().context("Model directory")?)?;
            let name = model
                .file_name()
                .and_then(|x| x.to_str())
                .context("Model name")?;
            anyhow::ensure!(name.is_ascii(), "Tên thư mục model phải là ASCII");
            let path = CString::new(name)?;
            let options = [
                ("transcription_interval", "0.08"),
                ("vad_window_duration", "0.5"),
                ("use_speculative_decoding", "true"),
                ("identify_speakers", "false"),
                ("return_audio_data", "false"),
                ("log_output_text", "false"),
            ];
            let strings: Vec<_> = options
                .iter()
                .map(|(k, v)| (CString::new(*k).unwrap(), CString::new(*v).unwrap()))
                .collect();
            let values: Vec<_> = strings
                .iter()
                .map(|(k, v)| OptionC {
                    name: k.as_ptr(),
                    value: v.as_ptr(),
                })
                .collect();
            let handle = load(
                path.as_ptr(),
                2,
                values.as_ptr(),
                values.len() as u64,
                30000,
            );
            anyhow::ensure!(handle >= 0, "Không mở được model chép trực tiếp ({handle})");
            let mut decoder = Self {
                _library: library,
                transcriber: handle,
                stream: -1,
                create,
                start,
                stop,
                free_stream,
                free_transcriber,
                add,
                transcribe,
                error,
                base: 0.,
                cursor: 0.,
                pending: 0,
                interval_samples: 1280,
                pending_cost: std::time::Duration::ZERO,
                completed: HashSet::new(),
                finished: false,
            };
            decoder.open_stream()?;
            Ok(decoder)
        }
    }
    fn check(&self, code: i32) -> Result<()> {
        if code < 0 {
            let ptr = unsafe { (self.error)(code) };
            let message = if ptr.is_null() {
                format!("mã {code}")
            } else {
                unsafe { CStr::from_ptr(ptr) }
                    .to_string_lossy()
                    .into_owned()
            };
            anyhow::bail!("Bộ chép trực tiếp: {message}");
        }
        Ok(())
    }
    fn open_stream(&mut self) -> Result<()> {
        self.stream = unsafe { (self.create)(self.transcriber, 0) };
        self.check(self.stream)?;
        self.check(unsafe { (self.start)(self.transcriber, self.stream) })?;
        self.finished = false;
        Ok(())
    }
    pub fn reset_at(&mut self, at: f64) -> Result<()> {
        if self.stream >= 0 {
            self.check(unsafe { (self.free_stream)(self.transcriber, self.stream) })?;
            self.stream = -1;
        }
        self.base = at;
        self.cursor = at;
        self.pending = 0;
        self.interval_samples = 1280;
        self.pending_cost = std::time::Duration::ZERO;
        self.completed.clear();
        self.open_stream()
    }
    pub fn decode(&mut self, samples: &[f32], finish: bool) -> Result<Vec<Update>> {
        let started = std::time::Instant::now();
        anyhow::ensure!(!self.finished || samples.is_empty(), "Luồng chép đã dừng");
        if !samples.is_empty() {
            self.check(unsafe {
                (self.add)(
                    self.transcriber,
                    self.stream,
                    samples.as_ptr(),
                    samples.len() as u64,
                    16000,
                    0,
                )
            })?;
            self.cursor += samples.len() as f64 / 16000.;
            self.pending += samples.len();
        }
        if finish && !self.finished {
            self.check(unsafe { (self.stop)(self.transcriber, self.stream) })?;
            self.finished = true;
        } else if !finish && self.pending < self.interval_samples {
            // Feature/VAD/encoder work can happen on ANY 20 ms input frame.
            // Counting only the polling frame misses most of this cost after gaps.
            self.pending_cost += started.elapsed();
            return Ok(vec![]);
        }
        self.pending = 0;
        let mut transcript = std::ptr::null();
        // FORCE_UPDATE: integer audio counter, 80 ms floor. Adapt to actual work
        // so a >80 ms pass cannot make every subsequent pass fall further behind.
        self.check(unsafe {
            (self.transcribe)(self.transcriber, self.stream, 1, &mut transcript)
        })?;
        let mut updates = vec![];
        let mut partial = vec![];
        let mut partial_start = self.cursor;
        let mut partial_end = self.cursor;
        if !transcript.is_null() {
            let result = unsafe { &*transcript };
            anyhow::ensure!(
                result.count <= 4096 && (result.count == 0 || !result.lines.is_null()),
                "Transcript streaming không hợp lệ"
            );
            for i in 0..result.count as usize {
                let line = unsafe { &*result.lines.add(i) };
                if line.text.is_null() {
                    continue;
                }
                let text = unsafe { CStr::from_ptr(line.text) }
                    .to_string_lossy()
                    .trim()
                    .to_owned();
                anyhow::ensure!(
                    line.start.is_finite() && line.duration.is_finite() && line.duration >= 0.,
                    "Mốc thời gian streaming không hợp lệ"
                );
                let start = (self.base + line.start as f64).clamp(self.base, self.cursor);
                let end = (self.base + line.start as f64 + line.duration as f64)
                    .clamp(start, self.cursor);
                if text.is_empty() {
                    continue;
                }
                if line.complete != 0 {
                    if self.completed.insert(line.id) {
                        let words = approximate_words(&text, start, end);
                        updates.push(Update {
                            text,
                            start,
                            end,
                            speech_end: Some(end),
                            final_text: true,
                            words,
                        });
                    }
                } else {
                    partial_start = partial_start.min(start);
                    partial_end = partial_end.max(end);
                    partial.push(text);
                }
            }
        }
        let partial_text = partial.join(" ");
        updates.push(Update {
            text: partial_text,
            start: partial_start,
            end: self.cursor,
            speech_end: (partial_end > partial_start).then_some(partial_end),
            final_text: finish,
            words: vec![],
        });
        self.interval_samples =
            next_interval(self.interval_samples, self.pending_cost + started.elapsed());
        self.pending_cost = std::time::Duration::ZERO;
        Ok(updates)
    }
}
impl Drop for Decoder {
    fn drop(&mut self) {
        unsafe {
            if self.stream >= 0 {
                (self.free_stream)(self.transcriber, self.stream);
            }
            (self.free_transcriber)(self.transcriber);
        }
    }
}

/// Budget decode + feed overhead, rounded to 20 ms, with slow decay to avoid
/// oscillation. This bounds scheduling only; it is not a word latency guarantee.
fn next_interval(previous: usize, cost: std::time::Duration) -> usize {
    let work = (cost.as_secs_f64() * 16000. * 1.2 + 160.).ceil() as usize;
    let target = (work.div_ceil(320) * 320).clamp(1280, 5120);
    target.max(previous * 3 / 4).div_ceil(320) * 320
}

/// Preview timestamps are approximate. Whisper remains the timestamped refinement.
/// Do not enable the extra attention alignment decoder on the latency-critical path.
fn approximate_words(text: &str, start: f64, end: f64) -> Vec<Segment> {
    if !start.is_finite() || !end.is_finite() || end <= start {
        return vec![];
    }
    let words: Vec<_> = text.split_whitespace().collect();
    words
        .iter()
        .enumerate()
        .map(|(i, text)| Segment {
            id: uuid::Uuid::new_v4().to_string(),
            start: start + (end - start) * i as f64 / words.len() as f64,
            end: start + (end - start) * (i + 1) as f64 / words.len() as f64,
            text: (*text).into(),
            edited: false,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cadence_budgets_slow_passes_and_recovers_without_float_drift() {
        use std::time::Duration;
        assert_eq!(next_interval(1280, Duration::from_millis(20)), 1280);
        assert_eq!(next_interval(1280, Duration::from_millis(120)), 2560);
        assert_eq!(next_interval(1280, Duration::from_secs(3)), 5120);
        let mut interval = 5120;
        for _ in 0..10 {
            interval = next_interval(interval, Duration::from_millis(20));
            assert_eq!(interval % 320, 0);
        }
        assert_eq!(interval, 1280);
    }
    #[test]
    fn pinned_x64_c_abi_layout_and_preview_timestamps() {
        assert_eq!(std::mem::size_of::<OptionC>(), 16);
        assert_eq!(std::mem::size_of::<LineC>(), 88);
        assert_eq!(std::mem::offset_of!(LineC, words), 72);
        assert_eq!(std::mem::size_of::<TranscriptC>(), 16);
        let words = approximate_words("Xin chào Việt Nam", 12., 14.);
        assert_eq!(words.len(), 4);
        assert_eq!(words[0].start, 12.);
        assert_eq!(words[3].end, 14.);
        assert!(words.windows(2).all(|w| w[0].end == w[1].start));
        assert!(approximate_words("Không có audio", 4., 4.).is_empty());
        assert!(approximate_words("Invalid", f64::NAN, 4.).is_empty());
    }
}
