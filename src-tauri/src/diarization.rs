//! Sidecar port for sherpa-onnx offline diarization. The worker gets its own
//! directory and ONNX Runtime; it never opens the meeting database.
use crate::speaker::SpeakerInterval;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub const MODEL_SEGMENTATION: &str = "segmentation.int8.onnx";
pub const MODEL_EMBEDDING: &str = "campplus.onnx";
const WORKER: &str = "sherpa-onnx-offline-speaker-diarization.exe";

pub fn resource_dir() -> Result<PathBuf> {
    let candidates = [
        std::env::var_os("INNOTE_DIARIZATION_DIR").map(PathBuf::from),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("diarization"))),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/diarization")),
    ];
    for candidate in candidates.into_iter().flatten() {
        if candidate.join(WORKER).is_file()
            && candidate.join(MODEL_SEGMENTATION).is_file()
            && candidate.join(MODEL_EMBEDDING).is_file()
            && candidate.join("sherpa-onnx-c-api.dll").is_file()
            && candidate.join("onnxruntime.dll").is_file()
        {
            return Ok(candidate);
        }
    }
    anyhow::bail!("Chưa có bộ phân tích người nói trên máy (thiếu thư mục diarization)")
}
pub fn status() -> serde_json::Value {
    match resource_dir() {
        Ok(dir) => serde_json::json!({"available": true, "path": dir.to_string_lossy()}),
        Err(e) => serde_json::json!({"available": false, "error": e.to_string()}),
    }
}
pub fn write_source_wav(pcm: &Path, wav: &Path) -> Result<(f64, String)> {
    let length = std::fs::metadata(pcm).context("Không đọc được nguồn âm thanh")?.len();
    anyhow::ensure!(length > 0 && length % 2 == 0, "Nguồn âm thanh trống hoặc sai định dạng");
    let mut reader = BufReader::new(std::fs::File::open(pcm).context("Không đọc được nguồn âm thanh")?);
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(wav, spec)?;
    let mut hasher = Sha256::new();
    let mut samples: u64 = 0;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        anyhow::ensure!(read % 2 == 0, "Nguồn âm thanh sai định dạng");
        hasher.update(&buffer[..read]);
        for pair in buffer[..read].chunks_exact(2) {
            writer.write_sample(i16::from_le_bytes([pair[0], pair[1]]))?;
        }
        samples += (read / 2) as u64;
    }
    writer.finalize()?;
    Ok((samples as f64 / 16000., hex::encode(hasher.finalize())))
}
fn parse_line(line: &str, duration: f64) -> Option<SpeakerInterval> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() != 4 || parts[1] != "--" || !parts[3].starts_with("speaker_") {
        return None;
    }
    let start: f64 = parts[0].parse().ok()?;
    let end: f64 = parts[2].parse().ok()?;
    let cluster: i32 = parts[3].trim_start_matches("speaker_").parse().ok()?;
    if !start.is_finite() || !end.is_finite() || start < 0. || end <= start || end > duration + 0.5 {
        return None;
    }
    Some(SpeakerInterval { start: start.min(duration), end: end.min(duration), cluster, confidence: None })
}
pub fn analyze_source(
    wav: &Path,
    duration: f64,
    num_clusters: Option<u32>,
    child_slot: Arc<Mutex<Option<Child>>>,
    cancel: Arc<AtomicBool>,
) -> Result<Vec<SpeakerInterval>> {
    anyhow::ensure!(!cancel.load(Ordering::Acquire), "Đã hủy phân tích người nói");
    let dir = resource_dir()?;
    let mut command = Command::new(dir.join(WORKER));
    command
        .current_dir(&dir)
        .arg(format!("--segmentation.pyannote-model={}", dir.join(MODEL_SEGMENTATION).display()))
        .arg(format!("--embedding.model={}", dir.join(MODEL_EMBEDDING).display()));
    if let Some(num_clusters) = num_clusters {
        command.arg(format!("--clustering.num-clusters={num_clusters}"));
    }
    command
        .arg(wav)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    let mut child = command.spawn().context("Không khởi động được worker phân tích người nói")?;
    let stdout = child.stdout.take().context("Worker không có stdout")?;
    let mut stderr = child.stderr.take().context("Worker không có stderr")?;
    let error_text = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    *child_slot.lock().unwrap() = Some(child);
    let mut intervals = Vec::new();
    for line in BufReader::new(stdout).lines() {
        if cancel.load(Ordering::Acquire) {
            if let Some(mut child) = child_slot.lock().unwrap().take() {
                let _ = child.kill();
            }
            anyhow::bail!("Đã hủy phân tích người nói");
        }
        let line = line?;
        if let Some(interval) = parse_line(&line, duration) {
            intervals.push(interval);
        }
    }
    let mut child = child_slot.lock().unwrap().take().context("Worker đã dừng")?;
    let status = child.wait()?;
    let error_text = error_text.join().unwrap_or_default();
    if cancel.load(Ordering::Acquire) {
        anyhow::bail!("Đã hủy phân tích người nói");
    }
    anyhow::ensure!(
        status.success(),
        "Worker phân tích người nói kết thúc với mã {}: {}",
        status.code().unwrap_or(-1),
        error_text.trim()
    );
    intervals.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.cluster.cmp(&b.cluster)));
    Ok(intervals)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_lines_are_strictly_parsed_and_clamped() {
        assert_eq!(parse_line("0.638 -- 6.848 speaker_00", 10.).unwrap().cluster, 0);
        assert_eq!(parse_line("0.638 -- 6.848 speaker_00", 10.).unwrap().start, 0.638);
        assert!(parse_line("Started", 10.).is_none());
        assert!(parse_line("bad -- text speaker_1", 10.).is_none());
        assert!(parse_line("1 -- 2 speaker_x", 10.).is_none());
        assert!(parse_line("1 -- 12 speaker_1", 10.).is_none());
        assert!(parse_line("1 -- 2 speaker_1 extra", 10.).is_none());
    }
    #[test]
    fn source_wav_preserves_duration_and_hash() {
        let dir = std::env::temp_dir().join(format!("innote-diar-wav-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pcm = dir.join("microphone.pcm");
        let mut bytes = Vec::new();
        for sample in [0i16, 1000, -1000, 2000] {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(&pcm, &bytes).unwrap();
        let wav = dir.join("source.wav");
        let (duration, hash) = write_source_wav(&pcm, &wav).unwrap();
        assert_eq!(duration, 4. / 16000.);
        assert_eq!(hash.len(), 64);
        let reader = hound::WavReader::open(&wav).unwrap();
        assert_eq!(reader.duration(), 4);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn public_multispeaker_sample_runs_when_fixture_exists() {
        let sample = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../test-results/diarization-spike/four-speakers-zh.wav");
        if !sample.is_file() || resource_dir().is_err() {
            return;
        }
        let intervals = analyze_source(
            &sample,
            56.861,
            None,
            Arc::new(Mutex::new(None)),
            Arc::new(AtomicBool::new(false)),
        )
        .unwrap();
        assert!(!intervals.is_empty());
        assert!(intervals.iter().all(|i| i.start >= 0. && i.end > i.start));
    }
}