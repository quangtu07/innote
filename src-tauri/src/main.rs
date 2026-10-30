#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod engine;
mod instance;
mod live;
mod llm;
mod models;
mod moonshine;
mod mp3;
mod nemo;
mod session_logic;
mod soniox;
mod storage;
mod types;

use anyhow::{Context, Result};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager, State};
use types::*;

#[derive(Clone)]
struct May {
    root: PathBuf,
    active: Arc<Mutex<Option<Arc<Run>>>>,
    writing: Arc<Mutex<()>>,
    downloading: Arc<AtomicBool>,
    devices: Arc<Mutex<Vec<Device>>>,
    soniox_key: Arc<Mutex<String>>,
    llm_key: Arc<Mutex<String>>,
}
struct Run {
    id: String,
    epoch: u64,
    stop: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    capture_done: AtomicBool,
    cancel: AtomicBool,
    duration: Mutex<f64>,
    failure: Mutex<Option<String>>,
}
impl Run {
    fn stop_now(&self) {
        let mut duration = self.duration.lock().unwrap();
        if !self.stop.load(Ordering::Acquire) {
            *duration = audio::clock_hns().saturating_sub(self.epoch) as f64 / 10_000_000.;
            self.stop.store(true, Ordering::Release);
        }
    }
    fn fail_storage(&self, message: String) {
        *self.failure.lock().unwrap() = Some(message);
        self.stop_now();
        self.cancel.store(true, Ordering::Release);
    }
}
impl May {
    fn edit(&self, id: &str, f: impl FnOnce(&mut Session)) -> Result<Session> {
        let _lock = self.writing.lock().unwrap();
        let mut s = storage::load(&self.root, id)?;
        f(&mut s);
        storage::save(&self.root, &s)?;
        Ok(s)
    }
    fn current(&self) -> Option<Arc<Run>> {
        self.active.lock().unwrap().clone()
    }
    fn reserve_resume(&self, run: Arc<Run>) -> Result<()> {
        let mut active = self.active.lock().unwrap();
        anyhow::ensure!(active.is_none(), "Một phiên khác đang chạy");
        let dir = storage::session_dir(&self.root, &run.id)?;
        let mut audio_duration: Option<f64> = None;
        for name in ["system.pcm", "microphone.pcm"] {
            match std::fs::metadata(dir.join(name)) {
                Ok(meta) => {
                    audio_duration = Some(
                        audio_duration
                            .unwrap_or(0.)
                            .max((meta.len() / 2) as f64 / 16000.),
                    )
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        let audio_duration = audio_duration
            .context("Không tìm thấy audio PCM để chép tiếp. Bản nghe lại hiện có vẫn được giữ.")?;
        self.edit(&run.id, |s| {
            s.duration = s.duration.max(audio_duration);
            s.status = "processing".into();
            s.error = None;
            if s.provider != "local" {
                s.provider = if s.processed > 0. { "mixed" } else { "local" }.into();
                s.warnings.push(
                    "Chép tiếp dùng local miễn phí; không tự gửi lại bản thu lên Soniox.".into(),
                );
            }
        })?;
        *active = Some(run);
        Ok(())
    }
}
struct Worker {
    child: Child,
    input: Option<ChildStdin>,
    output: std::sync::mpsc::Receiver<String>,
}
impl Worker {
    fn new(model: &std::path::Path) -> Result<Self> {
        let mut c = Command::new(std::env::current_exe()?);
        c.arg("--asr-worker").arg(model);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // Keep CPU recognition below capture and the user's meeting app.
            c.creation_flags(0x08000000 | 0x00004000);
        }
        let mut child = c
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let input = child.stdin.take();
        let stdout = child.stdout.take().context("Worker output")?;
        let (tx, output) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => {
                        if tx.send(line).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            child,
            input,
            output,
        })
    }
    fn process(&mut self, job: &WorkerJob, cancel: &AtomicBool) -> Result<WorkerResult> {
        writeln!(
            self.input.as_mut().context("Worker input")?,
            "{}",
            serde_json::to_string(job)?
        )?;
        self.input.as_mut().unwrap().flush()?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(180);
        let line = loop {
            anyhow::ensure!(
                !cancel.load(Ordering::Relaxed),
                "Đã dừng chép theo yêu cầu. Có thể chép tiếp từ bản thu"
            );
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "Worker không trả kết quả trong 180 giây. Bản thu vẫn được giữ"
            );
            match self
                .output
                .recv_timeout(std::time::Duration::from_millis(200))
            {
                Ok(line) => break line,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => anyhow::bail!("Worker dừng bất thường. Bản thu vẫn được giữ"),
            }
        };
        Ok(serde_json::from_str(&line)?)
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn remote_then_local<T>(
    remote: &mut bool,
    cancel: &AtomicBool,
    cloud: impl FnOnce() -> Result<T>,
    local: impl FnOnce() -> Result<T>,
    fallback: impl FnOnce(&str) -> Result<()>,
) -> Result<T> {
    anyhow::ensure!(!cancel.load(Ordering::Acquire), "Đã dừng chép theo yêu cầu");
    if *remote {
        match cloud() {
            Ok(result) => return Ok(result),
            Err(e) => {
                anyhow::ensure!(!cancel.load(Ordering::Acquire), "Đã dừng chép theo yêu cầu");
                *remote = false;
                fallback(&format!("{e}. Innote đã chuyển sang local miễn phí đến hết phiên; bản thu và phần đã chép vẫn được giữ."))?;
            }
        }
    }
    local()
}

fn process_job(
    worker: &mut Option<Worker>,
    model: &std::path::Path,
    key: &Option<String>,
    remote: &mut bool,
    job: &WorkerJob,
    cancel: &AtomicBool,
    fallback: impl FnOnce(&str) -> Result<()>,
) -> Result<WorkerResult> {
    remote_then_local(
        remote,
        cancel,
        || {
            tauri::async_runtime::block_on(soniox::transcribe(
                key.as_deref().unwrap_or(""),
                job,
                cancel,
            ))
        },
        || {
            if worker.is_none() {
                *worker = Some(Worker::new(model)?);
            }
            worker.as_mut().unwrap().process(job, cancel)
        },
        fallback,
    )
}

fn start_recording(may: May, app: tauri::AppHandle, options: StartOptions) -> Result<Session> {
    anyhow::ensure!(
        ["local", "soniox"].contains(&options.provider.as_str()),
        "Cách chép lời không hợp lệ"
    );
    let key = if options.transcribe && options.provider == "soniox" {
        let key = may.soniox_key.lock().unwrap().clone();
        (!key.is_empty()).then_some(key)
    } else {
        None
    };
    let mut remote = key.is_some();
    models::validate_mode(&options.language, &options.model, &options.provider)?;
    let model = models::path(&may.root, &options.model)?;
    if options.transcribe {
        anyhow::ensure!(
            models::list(&may.root)
                .iter()
                .any(|m| m.id == options.model && m.installed),
            "Hãy tải model trước hoặc chọn Chỉ thu âm"
        );
    }
    let mut active = may.active.lock().unwrap();
    anyhow::ensure!(
        active.is_none(),
        "Một phiên khác đang chạy. Hãy kết thúc trước."
    );
    anyhow::ensure!(
        options.system.is_some() || options.microphone.is_some(),
        "Chọn ít nhất một nguồn âm thanh"
    );
    let id = uuid::Uuid::new_v4().to_string();
    let dir = storage::session_dir(&may.root, &id)?;
    std::fs::create_dir_all(&dir)?;
    let title = if options.title.trim().is_empty() {
        format!("Cuộc họp {}", chrono::Local::now().format("%d/%m • %H:%M"))
    } else {
        options.title.trim().to_string()
    };
    let s = Session {
        id: id.clone(),
        title,
        created: chrono::Local::now().to_rfc3339(),
        duration: 0.,
        status: "recording".into(),
        language: options.language.clone(),
        model: options.model.clone(),
        provider: if remote { "soniox" } else { "local" }.into(),
        processed: 0.,
        segments: vec![],
        live: vec![],
        notes: String::new(),
        error: None,
        warnings: if options.transcribe && options.provider == "soniox" && !remote {
            vec!["Chưa có key Soniox. Phiên này dùng model local miễn phí; không gửi audio lên Soniox.".into()]
        } else {
            vec![]
        },
        smoothed: None,
    };
    storage::save(&may.root, &s)?;
    let run = Arc::new(Run {
        id: id.clone(),
        epoch: audio::clock_hns(),
        stop: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        capture_done: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
        duration: Mutex::new(0.),
        failure: Mutex::new(None),
    });
    let (tx, rx) = std::sync::mpsc::channel();
    let mut handles = vec![];
    for (kind, device) in [
        ("system", options.system),
        ("microphone", options.microphone),
    ] {
        if let Some(device) = device {
            let run = run.clone();
            let path = dir.join(format!("{kind}.pcm"));
            let tx = tx.clone();
            handles.push(std::thread::spawn(move || {
                if let Err(e) = audio::capture_resilient(
                    &device,
                    kind,
                    &path,
                    run.epoch,
                    run.stop.clone(),
                    run.paused.clone(),
                    tx.clone(),
                ) {
                    let _ = tx.send(audio::AudioEvent::Error(kind.into(), format!(
                        "Nguồn âm thanh đã ngắt. Kiểm tra tai nghe/micro và chọn lại thiết bị. Chi tiết ({kind}): {e}"
                    )));
                }
            }));
        }
    }
    // Do not claim to record until every requested source has opened successfully.
    let mut ready = std::collections::HashSet::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    let mut startup_error = None;
    while ready.len() < handles.len() {
        if std::time::Instant::now() > deadline {
            startup_error = Some("Mở thiết bị âm thanh quá lâu".to_string());
            break;
        }
        match rx.recv_timeout(std::time::Duration::from_millis(200)) {
            Ok(audio::AudioEvent::Ready(kind)) => {
                ready.insert(kind);
            }
            Ok(audio::AudioEvent::Error(_, e)) => {
                startup_error = Some(e);
                break;
            }
            _ => {}
        }
    }
    if let Some(e) = startup_error {
        run.stop.store(true, Ordering::Relaxed);
        for h in handles {
            let _ = h.join();
        }
        let _ = may.edit(&id, |s| {
            s.status = "interrupted".into();
            s.error = Some(e.clone());
        });
        return Err(anyhow::anyhow!(e));
    }
    *active = Some(run.clone());
    drop(active);
    let live_handle = if options.transcribe {
        let live_may = may.clone();
        let live_run = run.clone();
        let live_app = app.clone();
        let live_dir = dir.clone();
        let language = options.language.clone();
        Some(std::thread::spawn(move || {
            if let Err(e) = stream_preview(&live_may, &live_run, &live_app, &live_dir, &language) {
                let _ = live_may.edit(&live_run.id, |s| {
                    s.warnings.push(format!(
                        "Chép trực tiếp local đã ngắt: {e}. Bản thu vẫn được giữ."
                    ));
                    if s.model == models::NEMO_ID {
                        s.error = Some(
                            "Nhận dạng đa ngôn ngữ đã ngắt; bản thu và bản chép hiện có vẫn được giữ.".into(),
                        );
                    }
                });
                let _ = live_app.emit(
                    "live-transcript",
                    serde_json::json!({"id":live_run.id,"unavailable":true}),
                );
            }
        }))
    } else {
        None
    };
    let monitor_may = may.clone();
    let monitor_run = run.clone();
    let monitor_id = id.clone();
    let monitor_app = app.clone();
    let monitor = std::thread::spawn(move || {
        let mut healthy = session_logic::Sources::new(ready);
        while !monitor_run.capture_done.load(Ordering::Relaxed) {
            for event in rx.try_iter() {
                match event {
                    audio::AudioEvent::Level(kind, value) => {
                        let _ = monitor_app.emit(
                            "audio-level",
                            serde_json::json!({"kind":kind,"value":value}),
                        );
                    }
                    audio::AudioEvent::Warning(message) => {
                        let _ = monitor_may.edit(&monitor_id, |s| {
                            if !s.warnings.contains(&message) {
                                s.warnings.push(message);
                            }
                        });
                    }
                    audio::AudioEvent::Error(kind, message) => {
                        let all_lost = healthy.failed(&kind);
                        if all_lost {
                            monitor_run.stop_now();
                        }
                        let _ = monitor_may.edit(&monitor_id, |s| {
                            s.error = Some(message);
                            if !all_lost {
                                s.error = s
                                    .error
                                    .take()
                                    .map(|e| format!("{e}. Nguồn còn lại vẫn đang được thu."));
                            }
                            if all_lost {
                                s.status = "processing".into();
                            }
                        });
                    }
                    _ => {}
                }
            }
            if !monitor_run.stop.load(Ordering::Relaxed) {
                let duration =
                    audio::clock_hns().saturating_sub(monitor_run.epoch) as f64 / 10_000_000.;
                let saved = monitor_may.edit(&monitor_id, |s| {
                    if monitor_run.stop.load(Ordering::Acquire) {
                        return;
                    }
                    s.duration = duration;
                    s.status = if monitor_run.paused.load(Ordering::Relaxed) {
                        "paused"
                    } else {
                        "recording"
                    }
                    .into();
                });
                if let Err(e) = saved {
                    monitor_run.fail_storage(format!("Không lưu được trạng thái: {e}. Kiểm tra dung lượng và quyền thư mục dữ liệu."));
                    let _ = monitor_app.emit("storage-error", format!("Không lưu được trạng thái cuộc họp: {e}. Innote đã dừng ghi; hãy kiểm tra dung lượng và quyền thư mục dữ liệu. Audio đã ghi vẫn được giữ."));
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(150));
        }
    });
    std::thread::spawn(move || {
        let mut worker = None;
        let mut failed = false;
        let mut captures_joined = false;
        let mut threads = Some(handles);
        let mut last_save = std::time::Instant::now();
        loop {
            let stopped = run.stop.load(Ordering::Relaxed);
            let duration = if stopped {
                *run.duration.lock().unwrap()
            } else {
                audio::clock_hns().saturating_sub(run.epoch) as f64 / 10_000_000.
            };
            if stopped && !captures_joined {
                if let Some(threads) = threads.take() {
                    for h in threads {
                        let _ = h.join();
                    }
                }
                captures_joined = true;
                run.capture_done.store(true, Ordering::Relaxed);
            }
            if last_save.elapsed().as_millis() > 700 || stopped {
                let _ = may.edit(&id, |s| {
                    let stopped_now = run.stop.load(Ordering::Acquire);
                    s.duration = if stopped_now {
                        *run.duration.lock().unwrap()
                    } else {
                        duration
                    };
                    if stopped_now {
                        s.status = "processing".into();
                    } else {
                        s.status = if run.paused.load(Ordering::Relaxed) {
                            "paused"
                        } else {
                            "recording"
                        }
                        .into();
                    }
                });
                last_save = std::time::Instant::now();
            }
            if !options.transcribe || failed || options.model == models::NEMO_ID {
                if stopped {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
                continue;
            }
            let s = match storage::load(&may.root, &id) {
                Ok(s) => s,
                Err(_) => break,
            };
            if let Some((start, end)) = session_logic::next_job(
                s.processed,
                duration,
                stopped,
                run.paused.load(Ordering::Acquire),
            ) {
                let result = (|| -> Result<WorkerResult> {
                    let path = engine::write_job(&dir, start, end)?;
                    process_job(
                        &mut worker,
                        &model,
                        &key,
                        &mut remote,
                        &WorkerJob {
                            path: path.to_string_lossy().into(),
                            offset: start,
                            language: s.language.clone(),
                        },
                        &run.cancel,
                        |message| {
                            may.edit(&id, |s| {
                                s.provider =
                                    if s.processed > 0. { "mixed" } else { "local" }.into();
                                s.warnings.push(message.into());
                            })?;
                            Ok(())
                        },
                    )
                })();
                match result {
                    Ok(result) if result.error.is_none() => {
                        let saved = may.edit(&id, |s| {
                            session_logic::commit(s, result.segments, end);
                        });
                        if let Err(e) = saved {
                            failed = true;
                            worker = None;
                            run.fail_storage(format!(
                                "Không lưu được bản chép: {e}. Kiểm tra thư mục dữ liệu."
                            ));
                            let _ = app.emit("storage-error", format!("Không lưu được bản chép: {e}. Innote đã dừng ghi để giữ dữ liệu hiện có."));
                        }
                    }
                    other => {
                        let error = match other {
                            Ok(r) => r.error.unwrap_or_default(),
                            Err(e) => e.to_string(),
                        };
                        failed = true;
                        worker = None;
                        let _ = may.edit(&id, |s| {
                            s.error =
                                Some(format!("Chép lời tạm dừng: {error}. Bản thu vẫn được lưu."))
                        });
                    }
                }
            } else if stopped {
                break;
            } else {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        if let Some(threads) = threads.take() {
            for h in threads {
                let _ = h.join();
            }
        }
        run.capture_done.store(true, Ordering::Relaxed);
        if let Some(handle) = live_handle {
            let _ = handle.join();
        }
        let _ = monitor.join();
        let duration = *run.duration.lock().unwrap();
        let final_audio = audio::finalize(&dir, duration);
        let _ = may.edit(&id, |s| {
            if let Some(error) = run.failure.lock().unwrap().clone() {
                s.error = Some(error);
            }
            s.duration = duration;
            if options.model == models::NEMO_ID
                && options.transcribe
                && s.error.is_none()
                && !failed
            {
                s.processed = duration;
            }
            s.status = if final_audio.is_err() || failed || s.error.is_some() {
                "interrupted"
            } else if options.transcribe {
                "completed"
            } else {
                "recorded"
            }
            .into();
            if let Err(e) = final_audio {
                s.error = Some(format!("Chưa tạo được file nghe lại: {e}"));
            }
        });
        may.active.lock().unwrap().take();
        let _ = app.emit("session-changed", id);
    });
    Ok(s)
}

/// A separate worker tails PCM while the accuracy worker may be busy for minutes.
fn stream_preview(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    language: &str,
) -> Result<()> {
    let mut command = Command::new(std::env::current_exe()?);
    command.arg("--live-worker").arg(language).arg(dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let result = (|| -> Result<()> {
        let mut input = child.stdin.take();
        let output = child.stdout.take().context("Streaming output")?;
        let (tx, rx) = std::sync::mpsc::sync_channel(8);
        std::thread::spawn(move || {
            for line in BufReader::new(output).lines() {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut drain_deadline = None;
        let ready_deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut ready = false;
        loop {
            if run.cancel.load(Ordering::Acquire) {
                break;
            }
            if run.capture_done.load(Ordering::Acquire) && input.is_some() {
                input.take(); // EOF tells the child no more PCM can arrive.
                drain_deadline =
                    Some(std::time::Instant::now() + std::time::Duration::from_secs(15));
            }
            anyhow::ensure!(
                ready || std::time::Instant::now() < ready_deadline,
                "Mở model streaming quá lâu"
            );
            anyhow::ensure!(
                drain_deadline.is_none_or(|d| std::time::Instant::now() < d),
                "Bộ chép trực tiếp chưa xả hết audio; bản thu và bản chép hiện có vẫn được giữ"
            );
            match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(line) => {
                    let value: serde_json::Value = serde_json::from_str(&line?)?;
                    if value["ready"] == true {
                        ready = true;
                        continue;
                    }
                    let worker_unix_ms = value["worker_unix_ms"].as_f64();
                    let update: live::Update = serde_json::from_value(value)?;
                    if update.final_text && ["multi", "ko", "zh"].contains(&language) {
                        may.edit(&run.id, |s| {
                            s.segments.extend(update.words.iter().cloned());
                            s.processed = s.processed.max(update.end);
                        })?;
                        let _ = app.emit("session-changed", run.id.clone());
                    } else if update.final_text && !update.words.is_empty() {
                        may.edit(&run.id, |s| {
                            s.live.extend(
                                update
                                    .words
                                    .iter()
                                    .filter(|w| w.end > s.processed + 0.00001)
                                    .cloned(),
                            );
                        })?;
                    }
                    let mut event = serde_json::json!({"id":run.id,"update":update});
                    if std::env::var("MAY_LATENCY_AUDIT").as_deref() == Ok("1") {
                        let elapsed = audio::clock_hns().saturating_sub(run.epoch) as f64 / 10_000.;
                        let unix = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs_f64()
                            * 1000.;
                        event["audit_epoch_unix_ms"] = serde_json::json!(unix - elapsed);
                        event["audit_emit_ms"] = serde_json::json!(elapsed);
                        // Explicit local QA: distinguish decoder lag from archive
                        // persistence/IPC work in the parent. Never trace normally.
                        let path = dir.join("backend-latency-audit.jsonl");
                        if std::fs::metadata(&path)
                            .map(|m| m.len() < 8 * 1024 * 1024)
                            .unwrap_or(true)
                        {
                            if let Ok(mut file) = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(path)
                            {
                                let line = format!(
                                    "{}\n",
                                    serde_json::json!({
                                        "worker_unix_ms":worker_unix_ms,
                                        "emitted_unix_ms":unix,
                                        "audio_ms":event["update"]["end"].as_f64().unwrap_or(0.)*1000.,
                                        "speech_end_ms":event["update"]["speech_end"].as_f64().map(|x| x*1000.),
                                        "elapsed_ms":elapsed
                                    })
                                );
                                let _ = file.write_all(line.as_bytes());
                            }
                        }
                    }
                    app.emit("live-transcript", event)?;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let status = child.wait()?;
                    anyhow::ensure!(
                        status.success() && run.capture_done.load(Ordering::Acquire),
                        "Tiến trình streaming dừng bất thường ({status})"
                    );
                    break;
                }
            }
        }
        Ok(())
    })();
    let _ = child.kill();
    let _ = child.wait();
    result
}

// Explicit local QA only. No transcript tracing in normal launches.
#[tauri::command]
fn report_live_paint(
    may: State<May>,
    id: String,
    text: String,
    elapsed_ms: f64,
    received_ms: f64,
    emitted_ms: f64,
    audio_ms: f64,
    speech_end_ms: Option<f64>,
    final_text: bool,
) -> std::result::Result<(), String> {
    if std::env::var("MAY_LATENCY_AUDIT").as_deref() != Ok("1") {
        return Ok(());
    }
    (|| -> Result<()> {
        let run = may.current().context("Không có phiên audit đang chạy")?;
        anyhow::ensure!(
            run.id == id
                && text.len() <= 8192
                && [elapsed_ms, received_ms, emitted_ms, audio_ms].iter().all(|x| x.is_finite() && (0. ..=86_400_000.).contains(x))
                && speech_end_ms.is_none_or(|x| x.is_finite() && (0. ..=86_400_000.).contains(&x)),
            "Dữ liệu audit không hợp lệ"
        );
        let path = storage::session_dir(&may.root, &id)?.join("live-paint-audit.jsonl");
        if std::fs::metadata(&path)
            .map(|m| m.len() > 8 * 1024 * 1024)
            .unwrap_or(false)
        {
            return Ok(());
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        let line = format!(
            "{}\n",
            serde_json::json!({
                "wall_ms":elapsed_ms,
                "received_ms":received_ms,
                "emitted_ms":emitted_ms,
                "audio_ms":audio_ms,
                "word_end_ms":speech_end_ms,
                "final":final_text,
                "text":text
            })
        );
        file.write_all(line.as_bytes())?;
        Ok(())
    })()
    .map_err(err)
}

#[tauri::command]
fn snapshot(may: State<May>) -> std::result::Result<Snapshot, String> {
    let may = may.inner();
    let active_id = may.current().map(|r| r.id.clone());
    let mut sessions = storage::list(&may.root).map_err(err)?;
    for s in &mut sessions {
        session_logic::reconcile_visible_state(s, active_id.as_deref());
    }
    Ok(Snapshot {
        sessions,
        devices: may.devices.lock().unwrap().clone(),
        models: models::list(&may.root),
        data_dir: may.root.to_string_lossy().into(),
        active_id,
        soniox_configured: !may.soniox_key.lock().unwrap().is_empty(),
        llm_configured: !may.llm_key.lock().unwrap().is_empty(),
    })
}
#[tauri::command]
fn set_llm_key(may: State<May>, key: String) -> std::result::Result<(), String> {
    let key = key.trim().to_string();
    if key.chars().count() > 4096 {
        return Err("API key quá dài".into());
    }
    *may.llm_key.lock().unwrap() = key;
    Ok(())
}
#[tauri::command]
async fn smooth_transcript(
    may: State<'_, May>,
    id: String,
    provider: String,
    model: String,
    base_url: String,
    prompt: String,
) -> std::result::Result<Session, String> {
    let m = may.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Session> {
        let session = storage::load(&m.root, &id)?;
        anyhow::ensure!(
            !["recording", "paused", "processing"].contains(&session.status.as_str()),
            "Hãy đợi bản chép lời hoàn tất trước khi làm mịn"
        );
        let original = session
            .segments
            .iter()
            .map(|segment| segment.text.trim())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        anyhow::ensure!(!original.is_empty(), "Chưa có bản chép lời để làm mịn");
        let api_key = m.llm_key.lock().unwrap().clone();
        let smoothed = llm::smooth(&provider, &model, &base_url, &api_key, &original, &prompt)?;
        m.edit(&id, |s| {
            s.smoothed = Some(SmoothedTranscript {
                text: smoothed,
                provider,
                model,
                prompt,
                created: chrono::Utc::now().to_rfc3339(),
            });
        })
    })
    .await
    .map_err(err)?
    .map_err(err)
}
#[tauri::command]
fn set_soniox_key(may: State<May>, key: String) -> std::result::Result<(), String> {
    let key = soniox::validate_key(&key).map_err(err)?;
    *may.soniox_key.lock().unwrap() = key;
    Ok(())
}
#[tauri::command]
async fn refresh_devices(may: State<'_, May>) -> std::result::Result<(), String> {
    let m = may.inner().clone();
    let devices = tauri::async_runtime::spawn_blocking(audio::devices)
        .await
        .map_err(err)?
        .map_err(err)?;
    *m.devices.lock().unwrap() = devices;
    Ok(())
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
#[tauri::command]
async fn start(
    app: tauri::AppHandle,
    may: State<'_, May>,
    options: StartOptions,
) -> std::result::Result<Session, String> {
    let m = may.inner().clone();
    tauri::async_runtime::spawn_blocking(move || start_recording(m, app, options))
        .await
        .map_err(err)?
        .map_err(err)
}
#[tauri::command]
fn pause(may: State<May>, paused: bool) -> std::result::Result<(), String> {
    let r = may.current().ok_or("Không có phiên đang ghi")?;
    if r.stop.load(Ordering::Relaxed) {
        return Err("Phiên đã kết thúc".into());
    }
    r.paused.store(paused, Ordering::Relaxed);
    may.edit(&r.id, |s| {
        s.status = if paused { "paused" } else { "recording" }.into()
    })
    .map_err(err)?;
    Ok(())
}
#[tauri::command]
fn stop(may: State<May>) -> std::result::Result<(), String> {
    let r = may.current().ok_or("Không có phiên đang ghi")?;
    r.stop_now();
    may.edit(&r.id, |s| s.status = "processing".into())
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
fn cancel_processing(may: State<May>) -> std::result::Result<(), String> {
    let run = may.current().ok_or("Không có phiên đang xử lý")?;
    if !run.stop.load(Ordering::Relaxed) {
        return Err("Hãy kết thúc ghi âm trước khi dừng chép".into());
    }
    run.cancel.store(true, Ordering::Relaxed);
    Ok(())
}
#[tauri::command]
async fn download_model(
    app: tauri::AppHandle,
    may: State<'_, May>,
    id: String,
) -> std::result::Result<(), String> {
    let m = may.inner().clone();
    if m.downloading.swap(true, Ordering::SeqCst) {
        return Err("Đang tải một model khác".into());
    }
    let result = tauri::async_runtime::spawn_blocking(move || {
        let result = models::download(&m.root, &id, |bytes, total| {
            let _ = app.emit(
                "model-progress",
                serde_json::json!({"id":id,"bytes":bytes,"total":total}),
            );
        });
        m.downloading.store(false, Ordering::SeqCst);
        result
    })
    .await
    .map_err(err)?;
    result.map_err(err)
}
#[tauri::command]
fn edit_session(
    may: State<May>,
    id: String,
    title: Option<String>,
    notes: Option<String>,
    segment_id: Option<String>,
    text: Option<String>,
) -> std::result::Result<Session, String> {
    may.edit(&id, |s| {
        if let Some(title) = title {
            if !title.trim().is_empty() {
                s.title = title.trim().into();
            }
        }
        if let Some(notes) = notes {
            s.notes = notes;
        }
        if let (Some(id), Some(text)) = (segment_id, text) {
            if let Some(x) = s.segments.iter_mut().find(|x| x.id == id) {
                x.text = text;
                x.edited = true;
            }
        }
    })
    .map_err(err)
}
fn export_filename(title: &str, format: &str) -> String {
    let clean: String = title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(80)
        .collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']);
    let clean = if clean.is_empty() { "Cuoc-hop" } else { clean };
    format!(
        "{}-{}.{}",
        clean,
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        format
    )
}
#[tauri::command]
async fn choose_export_path(
    app: tauri::AppHandle,
    may: State<'_, May>,
    id: String,
    format: String,
) -> std::result::Result<Option<String>, String> {
    let s = storage::load(&may.root, &id).map_err(err)?;
    let (label, extension) = match format.as_str() {
        "md" => ("Markdown", "md"),
        "txt" => ("Văn bản TXT", "txt"),
        "srt" => ("Phụ đề SRT", "srt"),
        _ => return Err("Định dạng xuất không hợp lệ".into()),
    };
    let folder = app.path().download_dir().map_err(err)?.join("Innote");
    let name = export_filename(&s.title, extension);
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>> {
        std::fs::create_dir_all(&folder)?;
        Ok(rfd::FileDialog::new()
            .set_directory(&folder)
            .set_file_name(&name)
            .add_filter(label, &[extension])
            .save_file()
            .map(|path| path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(err)
}
#[tauri::command]
fn export_session(
    app: tauri::AppHandle,
    may: State<May>,
    id: String,
    format: String,
    path: Option<String>,
) -> std::result::Result<String, String> {
    let s = storage::load(&may.root, &id).map_err(err)?;
    if let Some(path) = path.filter(|p| !p.trim().is_empty()) {
        let mut path = PathBuf::from(path);
        path.set_extension(&format);
        return Ok(storage::export_file(&path, &s, &format)
            .map_err(err)?
            .to_string_lossy()
            .into());
    }
    let folder = app.path().download_dir().map_err(err)?.join("Innote");
    Ok(storage::export_to(&folder, &s, &format)
        .map_err(err)?
        .to_string_lossy()
        .into())
}
#[tauri::command]
fn open_exports_folder(app: tauri::AppHandle) -> std::result::Result<(), String> {
    let folder = app.path().download_dir().map_err(err)?.join("Innote");
    std::fs::create_dir_all(&folder).map_err(err)?;
    Command::new("explorer.exe")
        .arg(folder)
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
fn open_author_link(kind: String) -> std::result::Result<(), String> {
    let url = match kind.as_str() {
        "zalo" => "https://zalo.me/0962239818",
        "linkedin" => "https://www.linkedin.com/in/alex-linh-790794237/",
        "website" => "https://alexlinh.io.vn/",
        _ => return Err("Liên kết tác giả không hợp lệ".into()),
    };
    Command::new("explorer.exe")
        .arg(url)
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
fn audio_path(may: State<May>, id: String) -> std::result::Result<String, String> {
    let p = storage::session_dir(&may.root, &id)
        .map_err(err)?
        .join("recording.wav");
    if p.exists() {
        Ok(p.to_string_lossy().into())
    } else {
        Err("Bản thu chưa được hoàn tất".into())
    }
}

#[tauri::command]
async fn choose_audio_export_path(
    app: tauri::AppHandle,
    may: State<'_, May>,
    id: String,
) -> std::result::Result<Option<String>, String> {
    let s = storage::load(&may.root, &id).map_err(err)?;
    let folder = app.path().download_dir().map_err(err)?.join("Innote");
    let name = export_filename(&s.title, "mp3");
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>> {
        std::fs::create_dir_all(&folder)?;
        Ok(rfd::FileDialog::new()
            .set_directory(&folder)
            .set_file_name(&name)
            .add_filter("Âm thanh MP3", &["mp3"])
            .save_file()
            .map(|path| path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(err)
}

#[tauri::command]
async fn export_audio(
    may: State<'_, May>,
    id: String,
    path: String,
) -> std::result::Result<String, String> {
    let source = storage::session_dir(&may.root, &id)
        .map_err(err)?
        .join("recording.wav");
    if !source.exists() {
        return Err("Bản thu chưa được hoàn tất".into());
    }
    let mut output = PathBuf::from(path);
    output.set_extension("mp3");
    tauri::async_runtime::spawn_blocking(move || -> std::result::Result<String, String> {
        mp3::encode(&source, &output).map_err(err)?;
        Ok(output.to_string_lossy().into())
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
fn open_folder(may: State<May>) -> std::result::Result<(), String> {
    Command::new("explorer.exe")
        .arg(&may.root)
        .spawn()
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
fn delete_session(may: State<May>, id: String) -> std::result::Result<(), String> {
    let active = may.active.lock().unwrap();
    if active.as_ref().map(|r| r.id == id).unwrap_or(false) {
        return Err("Không thể xóa phiên đang chạy".into());
    }
    let _lock = may.writing.lock().unwrap();
    let dir = storage::session_dir(&may.root, &id).map_err(err)?;
    if dir.exists() {
        std::fs::remove_dir_all(dir).map_err(err)?;
    }
    storage::db(&may.root)
        .map_err(err)?
        .execute("DELETE FROM sessions WHERE id=?1", [id])
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
async fn resume_transcript(
    app: tauri::AppHandle,
    may: State<'_, May>,
    id: String,
) -> std::result::Result<(), String> {
    let m = may.inner().clone();
    let s = storage::load(&m.root, &id).map_err(err)?;
    let model = models::path(&m.root, &s.model).map_err(err)?;
    if !model.exists() {
        return Err("Tải model đã chọn trước khi chép tiếp".into());
    }
    let run = Arc::new(Run {
        id: id.clone(),
        epoch: audio::clock_hns(),
        stop: Arc::new(AtomicBool::new(true)),
        paused: Arc::new(AtomicBool::new(false)),
        capture_done: AtomicBool::new(true),
        cancel: AtomicBool::new(false),
        duration: Mutex::new(s.duration),
        failure: Mutex::new(None),
    });
    m.reserve_resume(run.clone()).map_err(err)?;
    std::thread::spawn(move || {
        let result = (|| -> Result<()> {
            let dir = storage::session_dir(&m.root, &id)?;
            let mut worker = Worker::new(&model)?;
            loop {
                let s = storage::load(&m.root, &id)?;
                if s.processed >= s.duration - 0.00001 {
                    break;
                }
                let start = (s.processed - 1.5).max(0.);
                let end = (s.processed + 20.).min(s.duration);
                let path = engine::write_job(&dir, start, end)?;
                let r = worker.process(
                    &WorkerJob {
                        path: path.to_string_lossy().into(),
                        offset: start,
                        language: s.language.clone(),
                    },
                    &run.cancel,
                )?;
                if let Some(e) = r.error {
                    anyhow::bail!(e);
                }
                m.edit(&id, |s| {
                    session_logic::commit(s, r.segments, end);
                })?;
            }
            audio::finalize(&dir, storage::load(&m.root, &id)?.duration)?;
            Ok(())
        })();
        let _ = m.edit(&id, |s| match result {
            Ok(()) => {
                s.status = "completed".into();
                s.error = None;
            }
            Err(e) => {
                s.status = "interrupted".into();
                s.error = Some(e.to_string());
            }
        });
        m.active.lock().unwrap().take();
        let _ = app.emit("session-changed", id);
    });
    Ok(())
}

#[tauri::command]
async fn test_audio(
    app: tauri::AppHandle,
    may: State<'_, May>,
    system: Option<String>,
    microphone: Option<String>,
) -> std::result::Result<AudioTestResult, String> {
    if system.is_none() && microphone.is_none() {
        return Err("Chọn ít nhất một nguồn âm thanh để thu thử".into());
    }
    let m = may.inner().clone();
    let id = uuid::Uuid::new_v4().to_string();
    let dir = storage::session_dir(&m.root, &id).map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    let run = Arc::new(Run {
        id: id.clone(),
        epoch: audio::clock_hns(),
        stop: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        capture_done: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
        duration: Mutex::new(0.),
        failure: Mutex::new(None),
    });
    {
        let mut active = m.active.lock().unwrap();
        if active.is_some() {
            return Err("Hãy kết thúc phiên đang chạy trước khi kiểm tra".into());
        }
        *active = Some(run.clone());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<AudioTestResult> {
            let (tx, rx) = std::sync::mpsc::channel();
            let mut handles = vec![];
            let mut peaks = std::collections::BTreeMap::<String, f32>::new();
            let mut warnings = vec![];
            for (kind, device) in [("system", system), ("microphone", microphone)] {
                if let Some(id) = device {
                    peaks.insert(kind.into(), 0.);
                    let tx = tx.clone();
                    let r = run.clone();
                    let path = dir.join(format!("{kind}.pcm"));
                    handles.push(std::thread::spawn(move || {
                        audio::capture_resilient(
                            &id,
                            kind,
                            &path,
                            r.epoch,
                            r.stop.clone(),
                            r.paused.clone(),
                            tx,
                        )
                    }));
                }
            }
            let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
            drop(tx);
            while std::time::Instant::now() < until {
                match rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(audio::AudioEvent::Level(kind, value)) => {
                        if let Some(peak) = peaks.get_mut(&kind) {
                            *peak = peak.max(value);
                        }
                        let _ = app.emit(
                            "audio-level",
                            serde_json::json!({"kind":kind,"value":value}),
                        );
                    }
                    Ok(audio::AudioEvent::Warning(message)) => warnings.push(message),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    _ => {}
                }
            }
            run.stop.store(true, Ordering::Relaxed);
            let mut failures = vec![];
            for h in handles {
                match h.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => failures.push(e.to_string()),
                    Err(_) => failures.push("Lỗi luồng thu âm".into()),
                }
            }
            anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
            audio::finalize(&dir, 5.)?;
            Ok(AudioTestResult {
                path: dir.join("recording.wav").to_string_lossy().into(),
                peaks,
                warnings,
            })
        })();
        m.active.lock().unwrap().take();
        result.map_err(err)
    })
    .await
    .map_err(err)?
}

#[cfg(test)]
mod controller_tests {
    use super::*;
    #[test]
    fn provider_defaults_local_and_errors_fall_back_once_without_reuploading() {
        let o: StartOptions = serde_json::from_value(serde_json::json!({"title":"","system":null,"microphone":"mic","language":"vi","model":"base-q5_1","transcribe":true})).unwrap();
        assert_eq!(o.provider, "local");
        for code in [401, 402, 403, 429, 503] {
            let mut remote = true;
            let cancel = AtomicBool::new(false);
            let warnings = Mutex::new(vec![]);
            let result = remote_then_local(
                &mut remote,
                &cancel,
                || anyhow::bail!("Soniox {code}"),
                || Ok("local result"),
                |m| {
                    warnings.lock().unwrap().push(m.to_string());
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(result, "local result");
            assert!(!remote);
            assert_eq!(warnings.lock().unwrap().len(), 1);
            assert_eq!(
                remote_then_local(
                    &mut remote,
                    &cancel,
                    || panic!("must not retry remote"),
                    || Ok("next local"),
                    |_| panic!("only warn once")
                )
                .unwrap(),
                "next local"
            );
        }
    }
    #[test]
    fn remote_success_does_not_run_local_and_cancel_never_triggers_fallback() {
        let mut remote = true;
        let cancel = AtomicBool::new(false);
        assert_eq!(
            remote_then_local(
                &mut remote,
                &cancel,
                || Ok(1),
                || panic!("local must not run"),
                |_| panic!("no warning")
            )
            .unwrap(),
            1
        );
        let result: Result<()> = remote_then_local(
            &mut remote,
            &cancel,
            || {
                cancel.store(true, Ordering::Release);
                anyhow::bail!("cancelled")
            },
            || panic!("cancel must not start local"),
            |_| panic!("cancel is not fallback"),
        );
        assert!(result.is_err());
    }
    fn run(id: String) -> Arc<Run> {
        Arc::new(Run {
            id,
            epoch: audio::clock_hns(),
            stop: Arc::new(AtomicBool::new(false)),
            paused: Arc::new(AtomicBool::new(false)),
            capture_done: AtomicBool::new(false),
            cancel: AtomicBool::new(false),
            duration: Mutex::new(0.),
            failure: Mutex::new(None),
        })
    }
    fn may(root: PathBuf) -> May {
        May {
            root,
            active: Arc::new(Mutex::new(None)),
            writing: Arc::new(Mutex::new(())),
            downloading: Arc::new(AtomicBool::new(false)),
            devices: Arc::new(Mutex::new(vec![])),
            soniox_key: Arc::new(Mutex::new(String::new())),
            llm_key: Arc::new(Mutex::new(String::new())),
        }
    }
    #[test]
    fn repeated_stop_freezes_duration_and_storage_failure_cancels_asr() {
        let run = run(uuid::Uuid::new_v4().to_string());
        run.stop_now();
        let first = *run.duration.lock().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(3));
        run.stop_now();
        assert_eq!(*run.duration.lock().unwrap(), first);
        run.fail_storage("Disk unavailable".into());
        assert!(run.cancel.load(Ordering::Acquire));
        assert!(run.failure.lock().unwrap().is_some());
        assert_eq!(*run.duration.lock().unwrap(), first);
    }
    #[test]
    fn failed_resume_cannot_leave_the_application_permanently_busy() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.join("meetings.sqlite")).unwrap();
        let run = run(uuid::Uuid::new_v4().to_string());
        let dir = storage::session_dir(&root, &run.id).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("system.pcm"), [0u8; 32]).unwrap();
        let may = may(root.clone());
        assert!(may.reserve_resume(run).is_err());
        assert!(may.current().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resume_requires_pcm_and_recovers_unsaved_tail_without_losing_edits() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let run = run(uuid::Uuid::new_v4().to_string());
        let session = Session {
            id: run.id.clone(),
            title: "Nháp".into(),
            created: "now".into(),
            duration: 1.,
            status: "interrupted".into(),
            language: "vi".into(),
            model: "base-q5_1".into(),
            provider: "local".into(),
            processed: 1.,
            segments: vec![Segment {
                id: "edited".into(),
                start: 0.,
                end: 1.,
                text: "Đã sửa".into(),
                edited: true,
            }],
            live: vec![],
            notes: "Giữ ghi chú".into(),
            error: None,
            warnings: vec![],
            smoothed: None,
        };
        storage::save(&root, &session).unwrap();
        let may = may(root.clone());
        assert!(may.reserve_resume(run.clone()).is_err());
        assert!(may.current().is_none());
        assert_eq!(storage::load(&root, &run.id).unwrap().status, "interrupted");
        let dir = storage::session_dir(&root, &run.id).unwrap();
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::File::create(dir.join("system.pcm"))
            .unwrap()
            .set_len(96000)
            .unwrap();
        may.reserve_resume(run.clone()).unwrap();
        let saved = storage::load(&root, &run.id).unwrap();
        assert_eq!(saved.duration, 3.);
        assert_eq!(saved.processed, 1.);
        assert_eq!(saved.notes, session.notes);
        assert_eq!(saved.segments[0].text, "Đã sửa");
        assert!(saved.segments[0].edited);
        assert!(may.reserve_resume(run).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if matches!(
        args.get(1).map(String::as_str),
        Some("--live-worker" | "--stream-test")
    ) {
        // Set before loading the native library or starting any child threads.
        // Avoid a BLAS thread pool competing with capture, Zoom and Whisper.
        std::env::set_var("OPENBLAS_NUM_THREADS", "1");
        std::env::set_var("OMP_NUM_THREADS", "1");
        let result = (|| -> Result<()> {
            let language = args.get(2).context("Cần ngôn ngữ vi/en/ko/zh/multi")?;
            let path = std::path::Path::new(args.get(3).context("Cần đường dẫn audio")?);
            if args[1] == "--stream-test" {
                live::benchmark(language, path)
            } else {
                live::worker(language, path)
            }
        })();
        if let Err(e) = result {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--asr-worker") {
        if let Err(e) = engine::worker(args.get(2).map(String::as_str).unwrap_or("")) {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--devices") {
        let result = audio::devices();
        let failed = result.is_err();
        println!("{}", serde_json::to_string(&result.map_err(err)).unwrap());
        if failed {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--capture-test") {
        let result = (|| -> Result<audio::CaptureReport> {
            let dir = PathBuf::from(
                args.get(2)
                    .context("Usage: --capture-test DIR [SECONDS] [system|microphone|both]")?,
            );
            let seconds = args
                .get(3)
                .map(|s| s.parse::<f64>())
                .transpose()?
                .unwrap_or(5.);
            audio::capture_test(
                &dir,
                seconds,
                args.get(4).map(String::as_str).unwrap_or("system"),
            )
        })();
        let failed = result.is_err();
        println!("{}", serde_json::to_string(&result.map_err(err)).unwrap());
        if failed {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--resume-test") {
        // QA-only: run the exact resume window/look-back/commit loop on a PCM
        // folder without opening the GUI. No user data root is touched.
        std::env::set_var("OPENBLAS_NUM_THREADS", "1");
        std::env::set_var("OMP_NUM_THREADS", "1");
        let result = (|| -> Result<serde_json::Value> {
            let dir = std::fs::canonicalize(
                args.get(2)
                    .context("Usage: --resume-test DIR MODEL LANGUAGE [EDITED_TEXT]")?,
            )?;
            let model = PathBuf::from(
                args.get(3)
                    .context("Usage: --resume-test DIR MODEL LANGUAGE [EDITED_TEXT]")?,
            );
            anyhow::ensure!(model.is_file(), "Không tìm thấy model");
            let language = args
                .get(4)
                .context("Usage: --resume-test DIR MODEL LANGUAGE [EDITED_TEXT]")?
                .clone();
            let mut duration = 0_f64;
            for name in ["system.pcm", "microphone.pcm"] {
                if let Ok(meta) = std::fs::metadata(dir.join(name)) {
                    duration = duration.max((meta.len() / 2) as f64 / 16000.);
                }
            }
            anyhow::ensure!(duration > 0., "Không tìm thấy PCM để chép tiếp");
            let mut s = Session {
                id: "resume-test".into(),
                title: "resume-test".into(),
                created: String::new(),
                duration,
                status: "processing".into(),
                language: language.clone(),
                model: model
                    .file_name()
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                provider: "local".into(),
                processed: 0.,
                segments: vec![],
                live: vec![],
                notes: String::new(),
                error: None,
                warnings: vec![],
                smoothed: None,
            };
            if let Some(text) = args.get(5).filter(|x| !x.is_empty()) {
                s.segments.push(Segment {
                    id: uuid::Uuid::new_v4().to_string(),
                    start: 0.,
                    end: 1.,
                    text: text.clone(),
                    edited: true,
                });
            }
            let mut worker = Worker::new(&model)?;
            let cancel = AtomicBool::new(false);
            let mut windows = 0_usize;
            let mut raw_segments = Vec::new();
            while s.processed < s.duration - 0.00001 {
                let start = (s.processed - 1.5).max(0.);
                let end = (s.processed + 20.).min(s.duration);
                let path = engine::write_job(&dir, start, end)?;
                let r = worker.process(
                    &WorkerJob {
                        path: path.to_string_lossy().into(),
                        offset: start,
                        language: language.clone(),
                    },
                    &cancel,
                )?;
                if let Some(e) = r.error {
                    anyhow::bail!(e);
                }
                let before = s.processed;
                raw_segments.push(r.segments.clone());
                session_logic::commit(&mut s, r.segments, end);
                anyhow::ensure!(s.processed > before + 0.00001, "Watermark không tiến");
                windows += 1;
            }
            Ok(serde_json::json!({
                "duration": s.duration,
                "processed": s.processed,
                "windows": windows,
                "segments": s.segments,
                "raw_segments": raw_segments,
                "edited_preserved": s.segments.iter().any(|x| x.edited)
            }))
        })();
        let failed = result.is_err();
        println!("{}", serde_json::to_string(&result.map_err(err)).unwrap());
        if failed {
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--transcribe-test") {
        let result = (|| -> Result<WorkerResult> {
            let model = args
                .get(2)
                .context("Usage: --transcribe-test MODEL WAV [vi|en]")?;
            let wav = args
                .get(3)
                .context("Usage: --transcribe-test MODEL WAV [vi|en]")?;
            engine::transcribe_file(
                std::path::Path::new(model),
                std::path::Path::new(wav),
                args.get(4).map(String::as_str).unwrap_or("vi"),
            )
        })();
        let failed = result.as_ref().map(|r| r.error.is_some()).unwrap_or(true);
        println!("{}", serde_json::to_string(&result.map_err(err)).unwrap());
        if failed {
            std::process::exit(1);
        }
        return;
    }
    let root = storage::root();
    let _instance = match instance::Instance::acquire(&root) {
        Ok(Some(guard)) => guard,
        Ok(None) => return,
        Err(e) => {
            eprintln!("Cannot lock data directory: {e:#}");
            return;
        }
    };
    if let Err(e) = storage::recover(&root) {
        eprintln!("Recovery: {e:#}");
    }
    let available = std::thread::spawn(audio::devices)
        .join()
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let may = May {
        root,
        active: Arc::new(Mutex::new(None)),
        writing: Arc::new(Mutex::new(())),
        downloading: Arc::new(AtomicBool::new(false)),
        devices: Arc::new(Mutex::new(available)),
        soniox_key: Arc::new(Mutex::new(String::new())),
        llm_key: Arc::new(Mutex::new(String::new())),
    };
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(may)
        .invoke_handler(tauri::generate_handler![
            snapshot,
            report_live_paint,
            set_soniox_key,
            set_llm_key,
            smooth_transcript,
            refresh_devices,
            start,
            pause,
            stop,
            cancel_processing,
            download_model,
            edit_session,
            choose_export_path,
            export_session,
            choose_audio_export_path,
            export_audio,
            open_exports_folder,
            open_author_link,
            audio_path,
            open_folder,
            delete_session,
            resume_transcript,
            test_audio
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<May>();
                if state.current().is_some() {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", ());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Không mở được Innote");
}
