use crate::types::Device;
use anyhow::{anyhow, Result};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::Duration,
};
use wasapi::*;
pub const RATE: usize = 16000;

pub fn clock_hns() -> u64 {
    let (mut counter, mut freq) = (0i64, 0i64);
    unsafe {
        let _ = windows::Win32::System::Performance::QueryPerformanceCounter(&mut counter);
        let _ = windows::Win32::System::Performance::QueryPerformanceFrequency(&mut freq);
    }
    ((counter as u128 * 10_000_000) / freq.max(1) as u128) as u64
}
pub fn devices() -> Result<Vec<Device>> {
    initialize_mta().ok()?;
    let e = DeviceEnumerator::new()?;
    let mut out = vec![];
    for (direction, kind) in [
        (Direction::Render, "system"),
        (Direction::Capture, "microphone"),
    ] {
        let default = e
            .get_default_device(&direction)
            .and_then(|d| d.get_id())
            .unwrap_or_default();
        for item in &e.get_device_collection(&direction)? {
            let d = item?;
            if !matches!(d.get_state()?, DeviceState::Active) {
                continue;
            }
            let id = d.get_id()?;
            out.push(Device {
                id: id.clone(),
                name: d.get_friendlyname()?,
                kind: kind.into(),
                default: id == default,
            });
        }
    }
    Ok(out)
}
#[derive(Clone, Debug)]
pub enum AudioEvent {
    Level(String, f32),
    Warning(String),
    Error(String, String),
    Ready(String),
}

#[derive(Debug, serde::Serialize)]
pub struct CaptureReport {
    pub duration: f64,
    pub sources: Vec<CaptureSourceReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct CaptureSourceReport {
    pub kind: String,
    pub device: String,
    pub bytes: u64,
    pub peak_rms: f32,
}

// Developer smoke test: use the same capture path as the app, with a shared epoch.
pub fn capture_test(dir: &Path, seconds: f64, source: &str) -> Result<CaptureReport> {
    anyhow::ensure!(
        seconds.is_finite() && (0.1..=3600.).contains(&seconds),
        "Thời gian thu thử phải từ 0,1 đến 3600 giây"
    );
    anyhow::ensure!(
        ["system", "microphone", "both"].contains(&source),
        "Nguồn thu thử phải là system, microphone hoặc both"
    );
    for name in ["system.pcm", "microphone.pcm", "recording.wav"] {
        anyhow::ensure!(
            !dir.join(name).exists(),
            "Thư mục thu thử đã chứa audio; hãy dùng thư mục mới"
        );
    }
    let available = devices()?;
    let kinds: &[&str] = match source {
        "both" => &["system", "microphone"],
        "microphone" => &["microphone"],
        _ => &["system"],
    };
    let selected = kinds
        .iter()
        .map(|kind| {
            available
                .iter()
                .find(|d| d.kind == *kind && d.default)
                .cloned()
                .ok_or_else(|| anyhow!("Không có thiết bị mặc định: {kind}"))
        })
        .collect::<Result<Vec<_>>>()?;
    std::fs::create_dir_all(dir)?;
    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = std::sync::mpsc::channel();
    let epoch = clock_hns();
    let timer = std::time::Instant::now();
    let mut handles = vec![];
    let mut sources = vec![];
    for device in selected {
        sources.push(CaptureSourceReport {
            kind: device.kind.clone(),
            device: device.name,
            bytes: 0,
            peak_rms: 0.,
        });
        let path = dir.join(format!("{}.pcm", device.kind));
        let flag = stop.clone();
        let sender = tx.clone();
        handles.push(std::thread::spawn(move || {
            capture_resilient(
                &device.id,
                &device.kind,
                &path,
                epoch,
                flag,
                Arc::new(AtomicBool::new(false)),
                sender,
            )
        }));
    }
    drop(tx);
    let mut ready = std::collections::HashSet::new();
    let mut warnings = vec![];
    while timer.elapsed().as_secs_f64() < seconds {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(AudioEvent::Ready(kind)) => {
                ready.insert(kind);
            }
            Ok(AudioEvent::Level(kind, level)) => {
                if let Some(s) = sources.iter_mut().find(|s| s.kind == kind) {
                    s.peak_rms = s.peak_rms.max(level);
                }
            }
            Ok(AudioEvent::Warning(w)) | Ok(AudioEvent::Error(_, w)) => warnings.push(w),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
        }
    }
    stop.store(true, Ordering::Relaxed);
    let duration = timer.elapsed().as_secs_f64();
    // Join every source even when one fails. No capture thread may outlive this test.
    let mut errors = vec![];
    for h in handles {
        match h.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(e.to_string()),
            Err(_) => errors.push("Capture panicked".into()),
        }
    }
    finalize(dir, duration)?;
    anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
    anyhow::ensure!(ready.len() == sources.len(), "Chưa mở đủ nguồn âm thanh");
    for s in &mut sources {
        s.bytes = std::fs::metadata(dir.join(format!("{}.pcm", s.kind)))?.len();
    }
    Ok(CaptureReport {
        duration,
        sources,
        warnings,
    })
}

fn open_archive(path: &Path) -> Result<File> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)?;
    file.seek(SeekFrom::End(0))?;
    Ok(file)
}

pub fn capture_resilient(
    id: &str,
    kind: &str,
    path: &Path,
    epoch: u64,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    tx: Sender<AudioEvent>,
) -> Result<()> {
    for attempt in 0..3 {
        if stop.load(Ordering::Relaxed) {
            return Ok(());
        }
        match capture(
            id,
            kind,
            path,
            epoch,
            stop.clone(),
            paused.clone(),
            tx.clone(),
        ) {
            Ok(()) => return Ok(()),
            Err(e) => {
                if stop.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let message = e.to_string().to_ascii_lowercase();
                if attempt < 2 && (message.contains("0x88890004") || message.contains("0x88890026"))
                {
                    let _=tx.send(AudioEvent::Warning("Thiết bị âm thanh tạm ngắt; Innote đang mở lại. Khoảng ngắt được giữ trên dòng thời gian.".into()));
                    std::thread::sleep(Duration::from_millis(350));
                    continue;
                }
                return Err(e);
            }
        }
    }
    Ok(())
}

pub fn capture(
    id: &str,
    kind: &str,
    path: &Path,
    epoch: u64,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    tx: Sender<AudioEvent>,
) -> Result<()> {
    initialize_mta().ok()?;
    let d = DeviceEnumerator::new()?.get_device(id)?;
    let mut client = d.get_iaudioclient()?;
    // Windows audio engine performs the anti-aliased resampling in shared mode.
    let format = WaveFormat::new(32, 32, &SampleType::Float, RATE as usize, 1, None);
    let mode = StreamMode::PollingShared {
        autoconvert: true,
        buffer_duration_hns: 2_000_000,
    };
    client.initialize_client(&format, &Direction::Capture, &mode)?;
    let input = client.get_audiocaptureclient()?;
    let mut file = open_archive(path)?;
    client.start_stream()?;
    let _ = tx.send(AudioEvent::Ready(kind.into()));
    let mut position = file.metadata()?.len() as usize / 2;
    let mut last_level = std::time::Instant::now();
    let outcome = (|| -> Result<()> {
        while !stop.load(Ordering::Relaxed) {
            while let Some(frames) = input.get_next_packet_size()? {
                if frames == 0 {
                    break;
                }
                let mut bytes = vec![0u8; frames as usize * 4];
                let (count, info) = input.read_from_device(&mut bytes)?;
                bytes.truncate(count as usize * 4);
                if paused.load(Ordering::Relaxed) {
                    continue;
                }
                let packet_hns = if info.flags.timestamp_error || info.timestamp == 0 {
                    clock_hns().saturating_sub(count as u64 * 10_000_000 / RATE as u64)
                } else {
                    info.timestamp
                };
                let start = ((packet_hns.saturating_sub(epoch) as u128 * RATE as u128) / 10_000_000)
                    as usize;
                // A discontinuity flag alone can be a harmless start/stop or endpoint
                // timestamp jitter. Warn only when the packet timeline actually skips
                // more than 100 ms, and never while the user is stopping capture.
                let gap_ms = start.saturating_sub(position) as f64 * 1000. / RATE as f64;
                if !stop.load(Ordering::Relaxed) && gap_ms >= 100. {
                    let _ = tx.send(AudioEvent::Warning(format!(
                        "Nguồn {} bị mất một đoạn âm thanh; Innote vẫn giữ dòng thời gian. Kiểm tra thiết bị nếu lặp lại.",
                        kind
                    )));
                }
                // Sparse holes read as silence and preserve the common monotonic timeline.
                if start > position + RATE / 50 {
                    position = start;
                    file.seek(SeekFrom::Start((position * 2) as u64))?;
                }
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|x| {
                        if info.flags.silent {
                            0.
                        } else {
                            f32::from_le_bytes(x.try_into().unwrap())
                        }
                    })
                    .collect();
                let pcm: Vec<u8> = samples
                    .iter()
                    .flat_map(|x| ((x.clamp(-1., 1.) * 32767.) as i16).to_le_bytes())
                    .collect();
                file.write_all(&pcm)?;
                position += samples.len();
                if last_level.elapsed() > Duration::from_millis(100) {
                    let rms = (samples.iter().map(|x| x * x).sum::<f32>()
                        / samples.len().max(1) as f32)
                        .sqrt();
                    let _ = tx.send(AudioEvent::Level(kind.into(), rms));
                    last_level = std::time::Instant::now();
                }
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        Ok(())
    })();
    let _ = client.stop_stream();
    file.sync_data()?;
    outcome
}
pub fn read_pcm(path: &Path, start: usize, len: usize) -> Result<Vec<f32>> {
    let mut out = vec![0.; len];
    if !path.exists() {
        return Ok(out);
    }
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(start as u64 * 2))?;
    let mut bytes = vec![0; len * 2];
    let mut used = 0;
    while used < bytes.len() {
        let n = f.read(&mut bytes[used..])?;
        if n == 0 {
            break;
        }
        used += n;
    }
    for (i, pair) in bytes[..used - used % 2].chunks_exact(2).enumerate() {
        out[i] = i16::from_le_bytes([pair[0], pair[1]]) as f32 / 32768.;
    }
    Ok(out)
}
pub fn mix(dir: &Path, start: f64, end: f64) -> Result<Vec<f32>> {
    if !start.is_finite() || !end.is_finite() || start < 0. || end <= start || end - start > 65. {
        return Err(anyhow!("Khoảng âm thanh không hợp lệ"));
    }
    let offset = (start * RATE as f64).round() as usize;
    let len = ((end - start) * RATE as f64).round() as usize;
    let a = read_pcm(&dir.join("system.pcm"), offset, len)?;
    let b = read_pcm(&dir.join("microphone.pcm"), offset, len)?;
    Ok(a.iter()
        .zip(&b)
        .map(|(x, y)| (x + y).clamp(-1., 1.))
        .collect())
}
pub fn wav(path: &Path, samples: &[f32]) -> Result<()> {
    let mut w = hound::WavWriter::create(
        path,
        hound::WavSpec {
            channels: 1,
            sample_rate: RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    for x in samples {
        w.write_sample((x.clamp(-1., 1.) * 32767.) as i16)?;
    }
    w.finalize()?;
    Ok(())
}
pub fn finalize(dir: &Path, duration: f64) -> Result<()> {
    anyhow::ensure!(
        duration.is_finite() && duration >= 0.,
        "Thời lượng bản thu không hợp lệ"
    );
    let pending = dir.join("recording.partial.wav");
    let mut w = hound::WavWriter::create(
        &pending,
        hound::WavSpec {
            channels: 1,
            sample_rate: RATE as u32,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )?;
    let mut t = 0.;
    while t < duration {
        let end = (t + 30.).min(duration);
        for x in mix(dir, t, end)? {
            w.write_sample((x * 32767.) as i16)?;
        }
        t = end;
    }
    w.finalize()?;
    OpenOptions::new().write(true).open(&pending)?.sync_all()?;
    std::fs::rename(pending, dir.join("recording.wav"))?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_audio_ranges_cannot_allocate_or_replace_an_existing_mix() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("recording.wav"), b"previous recording").unwrap();
        for duration in [f64::NAN, f64::INFINITY, -1.] {
            assert!(finalize(&dir, duration).is_err());
        }
        for (start, end) in [(f64::NAN, 1.), (-1., 1.), (0., f64::INFINITY), (0., 66.)] {
            assert!(mix(&dir, start, end).is_err());
        }
        assert_eq!(
            std::fs::read(dir.join("recording.wav")).unwrap(),
            b"previous recording"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn finalize_crosses_chunk_boundaries_and_keeps_silent_tail() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.pcm"), 10000i16.to_le_bytes()).unwrap();
        finalize(&dir, 75.125).unwrap();
        let mut reader = hound::WavReader::open(dir.join("recording.wav")).unwrap();
        assert_eq!(reader.duration(), 1202000);
        assert!(reader.samples::<i16>().next().unwrap().unwrap() > 9000);
        assert_eq!(reader.samples::<i16>().last().unwrap().unwrap(), 0);
        assert!(!dir.join("recording.partial.wav").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn capture_test_rejects_invalid_options_without_opening_devices() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        for seconds in [f64::NAN, f64::INFINITY, -1., 0., 3601.] {
            assert!(capture_test(&dir, seconds, "both").is_err());
        }
        assert!(capture_test(&dir, 1., "unknown").is_err());
        assert!(!dir.exists());
    }
    #[test]
    fn capture_test_preserves_existing_recording() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("recording.wav");
        std::fs::write(&path, b"existing audio").unwrap();
        assert!(capture_test(&dir, 1., "both")
            .unwrap_err()
            .to_string()
            .contains("audio"));
        assert_eq!(std::fs::read(path).unwrap(), b"existing audio");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn reopening_source_preserves_previous_audio() {
        let path = std::env::temp_dir().join(format!("{}.pcm", uuid::Uuid::new_v4()));
        std::fs::write(&path, [1u8, 2, 3, 4]).unwrap();
        let mut file = open_archive(&path).unwrap();
        file.write_all(&[5, 6]).unwrap();
        drop(file);
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3, 4, 5, 6]);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn separate_channels_and_missing_tail() {
        let d = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("system.pcm"),
            [10000i16.to_le_bytes(), 20000i16.to_le_bytes()].concat(),
        )
        .unwrap();
        std::fs::write(
            d.join("microphone.pcm"),
            [10000i16.to_le_bytes(), 20000i16.to_le_bytes()].concat(),
        )
        .unwrap();
        let x = mix(&d, 0., 3. / RATE as f64).unwrap();
        assert_eq!(x.len(), 3);
        assert_eq!(x[1], 1.);
        assert_eq!(x[2], 0.);
        std::fs::remove_dir_all(d).unwrap();
    }
}
