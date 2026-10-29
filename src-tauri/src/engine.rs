use crate::{audio, types::*};
use anyhow::{Context, Result};
use std::{
    io::{BufRead, Write},
    path::Path,
    time::Instant,
};
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperVadContext,
    WhisperVadContextParams, WhisperVadParams,
};

// VAD bounds remain on the original sample timeline. Coalesce nearby speech
// without concatenating away pauses; Whisper timestamps are local to each slice.
fn speech_windows(len: usize, spans: impl IntoIterator<Item = (f32, f32)>) -> Vec<(usize, usize)> {
    let mut windows: Vec<(usize, usize)> = vec![];
    for (start, end) in spans {
        if !start.is_finite() || !end.is_finite() || end <= start {
            continue;
        }
        let start = ((start.max(0.) as f64 * 160.).floor() as usize)
            .saturating_sub(2400)
            .min(len);
        let end = ((end.max(0.) as f64 * 160.).ceil() as usize)
            .saturating_add(2400)
            .min(len);
        if end <= start {
            continue;
        }
        if let Some(last) = windows
            .last_mut()
            .filter(|last| start <= last.1.saturating_add(12800))
        {
            last.1 = last.1.max(end);
        } else {
            windows.push((start, end));
        }
    }
    windows
}

// Whisper encodes a padded 30-second context on every full() call, even for
// a one-second utterance. A live job fits in one context: retain the original
// silence between VAD regions and encode it once rather than once per turn.
fn inference_windows(len: usize, windows: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    if len <= 30 * 16000 && windows.len() > 1 {
        vec![(windows[0].0, windows.last().unwrap().1)]
    } else {
        windows
    }
}

pub fn worker(model: &str) -> Result<()> {
    if Path::new(model).extension().is_some_and(|s| s == "gguf") {
        return crate::nemo::worker(Path::new(model));
    }
    anyhow::ensure!(
        std::is_x86_feature_detected!("avx2"),
        "Bản CPU này cần bộ xử lý hỗ trợ AVX2. Bạn vẫn có thể chỉ thu âm."
    );
    let vad = crate::models::prepare_vad(Path::new(model).parent().context("Model path")?)?;
    let mut options = WhisperContextParameters::default();
    options.use_gpu(false);
    // CPU baseline; flash attention needs a separate benchmark before enabling.
    options.flash_attn(false);
    let ctx = WhisperContext::new_with_params(model, options).context("Không mở được model")?;
    let mut state = ctx.create_state()?;
    let mut detector = WhisperVadContext::new(
        vad.to_str().context("VAD path")?,
        WhisperVadContextParams::default(),
    )?;
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let timer = Instant::now();
        let mut vad_windows = 0;
        let mut encoded_windows = 0;
        let result = (|| -> Result<Vec<Segment>> {
            let job: WorkerJob = serde_json::from_str(&line)?;
            anyhow::ensure!(
                job.offset.is_finite() && job.offset >= 0.,
                "Mốc audio không hợp lệ"
            );
            anyhow::ensure!(
                ["vi", "en", "ko", "zh", "auto"].contains(&job.language.as_str()),
                "Ngôn ngữ không hợp lệ"
            );
            let r = hound::WavReader::open(&job.path)?;
            anyhow::ensure!(
                r.spec().sample_rate == 16000 && r.spec().channels == 1,
                "Worker cần WAV mono 16 kHz"
            );
            let samples: Vec<f32> = r
                .into_samples::<i16>()
                .map(|x| x.map(|v| v as f32 / 32768.))
                .collect::<std::result::Result<_, _>>()?;
            if samples.len() < 1600
                || samples.iter().map(|x| x * x).sum::<f32>() / (samples.len() as f32) < 0.000001
            {
                return Ok(vec![]);
            }
            let spans = detector.segments_from_samples(WhisperVadParams::default(), &samples)?;
            let windows = speech_windows(samples.len(), spans.map(|s| (s.start, s.end)));
            vad_windows = windows.len();
            let windows = inference_windows(samples.len(), windows);
            encoded_windows = windows.len();
            let mut out = vec![];
            for (start, end) in windows {
                let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
                params.set_n_threads(4);
                params.set_translate(false);
                params.set_language(if job.language == "auto" {
                    None
                } else {
                    Some(&job.language)
                });
                params.set_print_special(false);
                params.set_print_progress(false);
                params.set_print_realtime(false);
                params.set_print_timestamps(false);
                params.set_no_context(true);
                params.set_suppress_blank(true);
                params.set_suppress_nst(true);
                // whisper-rs 0.16 calls whisper_full_with_state, which bypasses the
                // built-in VAD preprocessing in whisper_full/whisper_full_parallel.
                params.enable_vad(false);
                state.full(params, &samples[start..end])?;
                for x in state.as_iter() {
                    let text = x.to_str_lossy()?.trim().to_string();
                    if text.is_empty() {
                        continue;
                    }
                    out.push(Segment {
                        id: uuid::Uuid::new_v4().to_string(),
                        start: job.offset
                            + start as f64 / 16000.
                            + (x.start_timestamp().max(0) as f64 / 100.)
                                .min((end - start) as f64 / 16000.),
                        end: job.offset
                            + start as f64 / 16000.
                            + (x.end_timestamp().max(0) as f64 / 100.)
                                .min((end - start) as f64 / 16000.),
                        text,
                        edited: false,
                    });
                }
            }
            out.retain(|s| s.end > s.start);
            Ok(out)
        })();
        let msg = match result {
            Ok(segments) => WorkerResult {
                segments,
                error: None,
                elapsed_ms: timer.elapsed().as_millis() as u64,
                vad_windows,
                inference_windows: encoded_windows,
            },
            Err(e) => WorkerResult {
                segments: vec![],
                error: Some(e.to_string()),
                elapsed_ms: timer.elapsed().as_millis() as u64,
                vad_windows,
                inference_windows: encoded_windows,
            },
        };
        writeln!(stdout, "{}", serde_json::to_string(&msg)?)?;
        stdout.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn short_job_encodes_once_without_concatenating_pauses_or_losing_offset() {
        let regions = vec![(189600, 210000), (250000, 260000), (280000, 300000)];
        assert_eq!(
            inference_windows(20 * 16000, regions.clone()),
            vec![(189600, 300000)]
        );
        assert_eq!(inference_windows(40 * 16000, regions.clone()), regions);
        assert!(inference_windows(16000, vec![]).is_empty());
        assert_eq!(
            inference_windows(16000, vec![(2400, 10000)]),
            vec![(2400, 10000)]
        );
    }
    #[test]
    fn vad_keeps_leading_and_internal_pause_offsets_and_clamps_padding() {
        let windows = speech_windows(40 * 16000, [(1200., 1500.), (3000., 3500.)]);
        assert_eq!(windows, vec![(189600, 242400), (477600, 562400)]);
        assert!(windows[0].0 as f64 / 16000. >= 11.8);
        assert_eq!(
            speech_windows(16000, [(0., 30.), (50., 100.)]),
            vec![(0, 16000)]
        );
        assert!(speech_windows(16000, [(f32::NAN, 30.), (20., 10.)]).is_empty());
    }
}

pub fn transcribe_file(model: &Path, path: &Path, language: &str) -> Result<WorkerResult> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.arg("--asr-worker").arg(model);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00004000);
    }
    let mut child = command
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    let job = WorkerJob {
        path: std::fs::canonicalize(path)?.to_string_lossy().into(),
        offset: 0.,
        language: language.into(),
    };
    writeln!(
        child.stdin.take().context("stdin")?,
        "{}",
        serde_json::to_string(&job)?
    )?;
    let output = child.wait_with_output()?;
    anyhow::ensure!(output.status.success(), "Worker nhận dạng dừng bất thường");
    Ok(serde_json::from_slice(&output.stdout)?)
}

pub fn write_job(dir: &Path, start: f64, end: f64) -> Result<std::path::PathBuf> {
    let path = dir.join("pending.wav");
    audio::wav(&path, &audio::mix(dir, start, end)?)?;
    Ok(path)
}
