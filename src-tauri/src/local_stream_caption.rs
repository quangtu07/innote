//! Clock-paced PCM to the existing Model local streaming decoder in an isolated child.
use crate::{
    caption::{CaptionOptions, CaptionText},
    live::Update,
};
use anyhow::{ensure, Context, Result};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
enum Notice {
    Ready,
    Text(Update),
    Progress(usize),
    Closed,
    Error(String),
}
struct StreamWorker {
    child: Child,
    input: Option<std::sync::mpsc::Sender<Vec<f32>>>,
    output: tokio::sync::mpsc::UnboundedReceiver<Notice>,
    queued: Arc<AtomicUsize>,
}
impl StreamWorker {
    fn new(model: &Path, language: &str, quality: crate::speech_quality::SpeechQuality) -> Result<Self> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("--caption-stream-worker")
            .arg(model)
            .arg(language)
            .arg(quality.as_str())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000 | 0x00004000);
        }
        let mut child = command
            .spawn()
            .context("Không khởi động được Model local streaming")?;
        let mut input = child.stdin.take().context("Model local input")?;
        let stdout = child.stdout.take().context("Model local output")?;
        let (tx, output) = tokio::sync::mpsc::unbounded_channel();
        let errors = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let notice = (|| -> Result<Notice> {
                    let line = line?;
                    ensure!(line.len() < 1_000_000, "Model local trả dữ liệu quá lớn");
                    let value: serde_json::Value = serde_json::from_str(&line)?;
                    Ok(if value["ready"] == true {
                        Notice::Ready
                    } else if let Some(processed) = value["processed"].as_u64() {
                        Notice::Progress(processed as usize)
                    } else if let Some(error) = value["error"].as_str() {
                        Notice::Error(error.into())
                    } else {
                        Notice::Text(serde_json::from_value(value["update"].clone())?)
                    })
                })();
                if tx
                    .send(notice.unwrap_or_else(|e| Notice::Error(format!("Model local worker: {e}"))))
                    .is_err()
                {
                    break;
                }
            }
            let _ = tx.send(Notice::Closed);
        });
        let (sender, receiver) = std::sync::mpsc::channel::<Vec<f32>>();
        let queued = Arc::new(AtomicUsize::new(0));
        let counter = queued.clone();
        std::thread::spawn(move || {
            while let Ok(samples) = receiver.recv() {
                let count = samples.len();
                let pcm: Vec<_> = samples
                    .iter()
                    .flat_map(|s| ((s.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes())
                    .collect();
                let result = input.write_all(&pcm).and_then(|_| input.flush());
                counter.fetch_sub(count, Ordering::AcqRel);
                if result.is_err() {
                    let _ = errors.send(Notice::Error("Model local đã dừng nhận audio".into()));
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input: Some(sender),
            output,
            queued,
        })
    }
    fn send(&self, samples: Vec<f32>) -> Result<()> {
        let count = samples.len();
        let pending = self.queued.fetch_add(count, Ordering::AcqRel);
        if pending + count > 16000 * 5 {
            self.queued.fetch_sub(count, Ordering::AcqRel);
            anyhow::bail!("Model local local không theo kịp: audio tồn đọng hơn 5 giây. Hãy giảm tải CPU hoặc chọn cấu hình khác.");
        }
        if self
            .input
            .as_ref()
            .context("Model local đã đóng")?
            .send(samples)
            .is_err()
        {
            self.queued.fetch_sub(count, Ordering::AcqRel);
            anyhow::bail!("Model local đã dừng nhận audio");
        }
        Ok(())
    }
}
impl Drop for StreamWorker {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
pub async fn run<F: FnMut(CaptionText)>(
    app: &tauri::AppHandle,
    opts: &CaptionOptions,
    stop: Arc<AtomicBool>,
    mut publish: F,
) -> Result<()> {
    let model = opts.local_model.clone().context("Thiếu model Model local")?;
    let language =
        crate::local_caption::language(&opts.model, opts.source_language.as_deref())?.to_string();
    crate::caption::emit_status(
        app,
        "local",
        "connecting",
        false,
        "Đang nạp Model local streaming trên máy; không dùng API…",
    );
    let quality = opts.speech_quality;
    let mut worker =
        tokio::task::spawn_blocking(move || StreamWorker::new(&model, &language, quality)).await??;
    let ready = async {
        loop {
            tokio::select! {
                notice = worker.output.recv() => match notice {
                    Some(Notice::Ready) => return Ok(()),
                    Some(Notice::Error(error)) => anyhow::bail!(error),
                    None | Some(Notice::Closed) => anyhow::bail!("Model local worker dừng khi nạp model"),
                    _ => {}
                },
                _ = tokio::time::sleep(Duration::from_millis(50)) => {
                    if stop.load(Ordering::Acquire) { return Ok(()); }
                }
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(30), ready)
        .await
        .context("Nạp Model local quá lâu (30 giây)")??;
    if stop.load(Ordering::Acquire) {
        crate::caption::emit_status(app, "local", "idle", false, "Đã dừng phụ đề local.");
        return Ok(());
    }
    let source = crate::caption::audio_input(opts, stop.clone())?;
    let mut audio = source.lock().await;
    let mut pending = std::collections::VecDeque::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(20));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut sound = std::time::Instant::now();
    let mut warned = false;
    let mut submitted = 0usize;
    let mut processed = 0usize;
    let clock = std::time::Instant::now();
    crate::caption::emit_status(
        app,
        "local",
        "active",
        false,
        "Model local đang nhận audio liên tục trên máy, có chữ tạm theo lời nói; không dùng API.",
    );
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                if stop.load(Ordering::Acquire) { break; }
                let count = due_samples(clock.elapsed(), submitted);
                if count > 0 {
                    ensure!(submitted.saturating_sub(processed) + count <= 16000 * 5,
                        "Model local không theo kịp âm thanh: tồn đọng hơn 5 giây. Hãy giảm tải CPU hoặc chọn model khác.");
                    let samples = (0..count).map(|_| pending.pop_front().unwrap_or(0.)).collect();
                    worker.send(samples)?;
                    submitted += count;
                }
                if !warned && sound.elapsed() > Duration::from_secs(10) {
                    warned = true;
                    crate::caption::emit_status(app, "local", "active", false,
                        "Chưa nhận được tiếng từ loa/tai nghe trong 10 giây; kiểm tra thiết bị phát và âm lượng.");
                }
            },
            input = audio.recv() => {
                if stop.load(Ordering::Acquire) { break; }
                let samples = input.context("Nguồn âm thanh đã dừng; hãy chọn lại loa/tai nghe")??;
                ensure!(pending.len() + samples.len() <= 16000 * 5,
                    "Audio local tồn đọng hơn 5 giây; CPU chưa theo kịp");
                if samples.iter().any(|s| s.abs() > 0.002) {
                    sound = std::time::Instant::now();
                    if warned {
                        warned = false;
                        crate::caption::emit_status(app, "local", "active", false,
                            "Đã nhận lại audio loa; đang chép local liên tục.");
                    }
                }
                pending.extend(samples);
            },
            notice = worker.output.recv() => match notice {
                Some(Notice::Text(update)) => {
                    if !update.text.trim().is_empty() {
                        publish(CaptionText { text: update.text, original: String::new(), final_: update.final_text });
                    }
                },
                Some(Notice::Progress(count)) => processed = count,
                Some(Notice::Error(error)) => anyhow::bail!(error),
                None | Some(Notice::Closed) => anyhow::bail!("Model local worker dừng bất thường"),
                _ => {}
            }
        }
    }
    // EOF flushes the native decoder before teardown, retaining the last words.
    // Bound shutdown so slow hardware cannot leave the session stuck stopping.
    if !pending.is_empty() {
        worker.send(pending.drain(..).collect())?;
    }
    worker.input.take();
    let drain = async {
        loop {
            match worker.output.recv().await {
                Some(Notice::Text(update)) if !update.text.trim().is_empty() => {
                    publish(CaptionText {
                        text: update.text,
                        original: String::new(),
                        final_: update.final_text,
                    })
                }
                Some(Notice::Error(error)) => anyhow::bail!(error),
                None | Some(Notice::Closed) => return Ok(()),
                _ => {}
            }
        }
    };
    tokio::time::timeout(Duration::from_secs(8), drain)
        .await
        .context("Model local chưa hoàn tất đoạn cuối sau 8 giây; CPU không theo kịp")??;
    drop(worker);
    crate::caption::emit_status(
        app,
        "local",
        "idle",
        false,
        "Đã dừng phụ đề Model local local.",
    );
    Ok(())
}
fn due_samples(elapsed: Duration, submitted: usize) -> usize {
    // Allow capture packets 80 ms to arrive. Never insert silence and then
    // speed up the same clock again merely because a packet arrived late.
    let target = (elapsed.saturating_sub(Duration::from_millis(80)).as_secs_f64() * 16000.) as usize;
    target.saturating_sub(submitted).min(640)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pacing_waits_for_capture_and_never_runs_ahead_of_wall_clock() {
        assert_eq!(due_samples(Duration::from_millis(50), 0), 0);
        assert_eq!(due_samples(Duration::from_millis(100), 0), 320);
        assert_eq!(due_samples(Duration::from_millis(100), 320), 0);
        assert_eq!(due_samples(Duration::from_millis(200), 320), 640);
        assert_eq!(due_samples(Duration::from_millis(200), 1920), 0);
    }
}
/// Private binary-pipe worker entry. Reuses the model with a caption-only CPU profile.
pub fn worker(model: &Path, language: &str, quality: crate::speech_quality::SpeechQuality) -> Result<()> {
    let mut decoder = crate::live::CaptionDecoder::captions_with_quality(model, language, quality)?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    writeln!(output, "{{\"ready\":true}}")?;
    output.flush()?;
    let mut previous = String::new();
    let mut processed = 0usize;
    loop {
        // Match the native CLI's 160 ms ingestion; the encoder retains its cache.
        let mut bytes = [0u8; 5120];
        let mut used = 0;
        while used < bytes.len() {
            let count = input.read(&mut bytes[used..])?;
            if count == 0 {
                break;
            }
            used += count;
        }
        if used == 0 {
            break;
        }
        ensure!(used % 2 == 0, "Audio PCM16 không hợp lệ");
        let samples: Vec<_> = bytes[..used]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.)
            .collect();
        for update in decoder.decode(&samples, false)? {
            if update.final_text || update.text != previous {
                previous = if update.final_text {
                    String::new()
                } else {
                    update.text.clone()
                };
                writeln!(output, "{}", serde_json::json!({"update":update}))?;
                output.flush()?;
            }
        }
        processed += samples.len();
        writeln!(output, "{}", serde_json::json!({"processed":processed}))?;
        output.flush()?;
    }
    for update in decoder.decode(&[], true)? {
        writeln!(output, "{}", serde_json::json!({"update":update}))?;
    }
    output.flush()?;
    Ok(())
}
