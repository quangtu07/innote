#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod audio;
mod caption;
mod caption_window;
mod config;
mod correction;
mod engine;
mod fallback;
mod groq;
mod instance;
mod live;
mod llm;
mod local_caption;
mod local_stream_caption;
mod meeting_groq;
mod meeting_store;
mod meeting_bundle;
mod meeting_file_guide;
mod meeting_soniox;
mod models;
mod moonshine;
mod mp3;
mod nemo;
mod secure;
mod session_logic;
mod soniox;
mod speech_quality;
mod storage;
mod storage_paths;
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
    library_leases: Arc<Mutex<std::collections::HashMap<String,std::fs::File>>>,
    active: Arc<Mutex<Option<Arc<Run>>>>,
    writing: Arc<Mutex<()>>,
    downloading: Arc<AtomicBool>,
    devices: Arc<Mutex<Vec<Device>>>,
    soniox_key: Arc<Mutex<String>>,
    llm_key: Arc<Mutex<String>>,
    profiles: Arc<Mutex<Vec<config::AiProfile>>>,
    caption_stop: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    caption_done: Arc<AtomicBool>,
    verified_ai: Arc<Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    corrections: Arc<Mutex<std::collections::HashMap<String, Arc<correction::Control>>>>,
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
    speech: meeting_soniox::Control,
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
    fn lock_libraries(&self) -> Result<()> {
        let mut leases=self.library_leases.lock().unwrap();
        let default=storage_paths::load_registry(&self.root)?.default_library_id;
        for l in meeting_store::libraries(&self.root)? {
            if l.root.is_dir() && !leases.contains_key(&l.id) {
                let lease=meeting_store::validate_registered(&l).and_then(|_|storage_paths::acquire_library_writer(&l.root));
                match lease {Ok(lease)=>{leases.insert(l.id,lease);},Err(e) if default.as_ref()==Some(&l.id)=>return Err(e),Err(_)=>{}}
            }
        } Ok(())
    }
    fn ensure_library(&self,id:&str)->Result<()> {
        if storage_paths::load_registry(&self.root)?.default_library_id.is_none(){return Ok(());}
        let location=meeting_store::locate(&self.root,id)?;if !location.modern{return Ok(());}
        let library=meeting_store::libraries(&self.root)?.into_iter().find(|l|l.root.join(".innote/meetings.sqlite")==location.database).context("Không tìm thấy thư viện")?;
        let mut leases=self.library_leases.lock().unwrap();if !leases.contains_key(&library.id){
            meeting_store::validate_registered(&library)?;leases.insert(library.id,storage_paths::acquire_library_writer(&library.root)?);
        } Ok(())
    }
    fn edit(&self, id: &str, f: impl FnOnce(&mut Session)) -> Result<Session> {
        let _lock = self.writing.lock().unwrap();
        self.ensure_library(id)?;
        let mut s = storage::load(&self.root, id)?;
        f(&mut s);
        storage::save(&self.root, &s)?;
        Ok(s)
    }
    fn current(&self) -> Option<Arc<Run>> {
        self.active.lock().unwrap().clone()
    }
    /// Register the caption session's stop flag; returns Err if one is running.
    fn caption_begin(&self, stop: Arc<AtomicBool>) -> Result<()> {
        let mut guard = self.caption_stop.lock().unwrap();
        anyhow::ensure!(
            self.caption_done.load(Ordering::Acquire),
            "Phiên phụ đề trước đang dừng; hãy chờ vài giây"
        );
        self.caption_done.store(false, Ordering::Release);
        *guard = Some(stop);
        Ok(())
    }
    fn caption_running(&self) -> bool {
        self.current()
            .is_some_and(|r| r.speech.enabled.load(Ordering::Acquire))
            || !self.caption_done.load(Ordering::Acquire)
    }
    /// Gracefully stop the caption session (idempotent).
    fn caption_stop_now(&self) {
        if let Some(run) = self.current() {
            run.speech.enabled.store(false, Ordering::Release);
        }
        if let Some(stop) = self.caption_stop.lock().unwrap().as_ref() {
            stop.store(true, Ordering::Release);
        }
    }
    /// Resolve a profile by id; the key stays in the backend.
    fn profile(&self, id: &str) -> Result<config::AiProfile> {
        self.profiles
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .context("Không tìm thấy cấu hình AI đã chọn")
    }
    fn correction_profile(&self, id: &str) -> Result<config::AiProfile> {
        if id == "__local__" {
            return Ok(config::AiProfile {
                id: id.into(),
                name: "Local trên máy".into(),
                provider: "local".into(),
                base_url: String::new(),
                key: String::new(),
                created: String::new(),
                import_tag: String::new(),
            });
        }
        self.profile(id)
    }
    fn correction_model(
        &self,
        profile: &config::AiProfile,
        model: &str,
        language: &str,
    ) -> Result<Option<PathBuf>> {
        if profile.provider != "local" {
            return Ok(None);
        }
        local_caption::language(model, Some(language))?;
        anyhow::ensure!(models::list(&self.root).iter().any(|m|m.id==model&&m.installed),"Model hiệu chỉnh local chưa được tải hoặc thiếu thành phần; chọn model đã có trong Mẫu cấu hình");
        Ok(Some(models::path(&self.root, model)?))
    }
    /// A selected task profile owns provider, endpoint and credential together.
    fn task_config(
        &self,
        provider: &str,
        base: &str,
        key: Option<String>,
        profile_id: Option<&str>,
    ) -> Result<(String, String, String)> {
        if let Some(id) = profile_id {
            let profile = self.profile(id)?;
            return Ok((profile.provider, profile.base_url, profile.key));
        }
        Ok((
            provider.into(),
            base.into(),
            key.filter(|k| !k.trim().is_empty())
                .unwrap_or_else(|| self.llm_key.lock().unwrap().clone()),
        ))
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

fn start_recording(may: May, app: tauri::AppHandle, options: StartOptions) -> Result<Session> {
    let library=meeting_store::default_library(&may.root)?;
    may.lock_libraries()?;
    anyhow::ensure!(storage_paths::available_bytes(&library.root)?>16*1024*1024,"Nơi lưu còn quá ít dung lượng; hãy giải phóng dung lượng hoặc chọn ổ khác");
    anyhow::ensure!(
        ["local", "soniox", "groq"].contains(&options.provider.as_str()),
        "Cách chép lời không hợp lệ"
    );
    let key = if options.transcribe && options.provider == "soniox" {
        let key = if let Some(id) = options.profile_id.as_deref() {
            let p = may.profile(id)?;
            anyhow::ensure!(
                p.provider == "soniox",
                "Cấu hình chép lời phải thuộc Soniox"
            );
            p.key
        } else {
            may.soniox_key.lock().unwrap().clone()
        };
        (!key.is_empty()).then_some(key)
    } else {
        None
    };
    anyhow::ensure!(
        !options.captions_enabled || options.transcribe,
        "Hãy bật chép lời trước khi bật phụ đề cuộc họp"
    );
    let groq_profile = if options.transcribe && options.provider == "groq" {
        let profile = may.profile(
            options
                .profile_id
                .as_deref()
                .context("Hãy chọn cấu hình Groq đã lưu")?,
        )?;
        anyhow::ensure!(
            profile.provider == "groq" && !profile.key.trim().is_empty(),
            "Cấu hình Groq chưa có key hợp lệ"
        );
        anyhow::ensure!(
            !options.local_fallback,
            "Dự phòng local hiện áp dụng cho Soniox; Groq dùng cấu hình API dự phòng đã chọn"
        );
        Some(profile)
    } else {
        None
    };
    let remote = key.is_some();
    let any_remote = remote || groq_profile.is_some();
    let transcription_backup = if let Some(choice) = options.transcription_fallback.as_ref() {
        anyhow::ensure!(
            any_remote && options.transcribe,
            "Dự phòng chép lời cần cấu hình cloud chính"
        );
        let profile = may.profile(&choice.profile_id)?;
        anyhow::ensure!(
            profile.provider == options.provider
                && !profile.key.trim().is_empty()
                && (profile.provider == "soniox" && choice.model == crate::soniox::RT_MODEL
                    || profile.provider == "groq" && groq::is_speech_model(&choice.model)),
            "Dự phòng chép lời cần model phù hợp và cùng nhà cung cấp với model chính"
        );
        let primary_model = if remote {
            crate::soniox::RT_MODEL
        } else {
            options.model.as_str()
        };
        anyhow::ensure!(
            options.profile_id.as_deref() != Some(profile.id.as_str())
                || choice.model != primary_model,
            "Dự phòng phải khác model hoặc cấu hình chính"
        );
        Some((profile, choice.model.clone()))
    } else {
        None
    };
    let resolved_profile = options.profile_id.clone().or_else(|| {
        key.as_ref().and_then(|key| {
            may.profiles
                .lock()
                .unwrap()
                .iter()
                .find(|p| p.provider == "soniox" && &p.key == key)
                .map(|p| p.id.clone())
        })
    });
    if options.transcribe && options.provider == "soniox" {
        anyhow::ensure!(
            remote,
            "Chưa chọn cấu hình Soniox hợp lệ; hãy thêm/kiểm tra key trong Cài đặt"
        );
    }
    if let Some(target) = options.translation_language.as_deref() {
        anyhow::ensure!(
            options.transcribe && remote,
            "Dịch dùng chung cần chọn Soniox để chép lời"
        );
        anyhow::ensure!(
            ["vi", "en", "ko", "zh", "ja"].contains(&target),
            "Ngôn ngữ dịch không hợp lệ"
        );
        anyhow::ensure!(
            !may.caption_running(),
            "Hãy dừng phiên phụ đề độc lập trước khi bật dịch trong cuộc họp"
        );
    }
    models::validate_mode(&options.language, &options.model, &options.provider)?;
    if options.transcribe && (!any_remote || options.local_fallback) {
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
    let title = if options.title.trim().is_empty() {
        format!("Cuộc họp {}", chrono::Local::now().format("%d/%m • %H:%M"))
    } else {
        options.title.trim().to_string()
    };
    let s = Session {
        speech_quality: options.speech_quality,
        id: id.clone(),
        title,
        created: chrono::Local::now().to_rfc3339(),
        duration: 0.,
        status: "recording".into(),
        language: options.language.clone(),
        model: options.model.clone(),
        provider: if any_remote {
            options.provider.as_str()
        } else {
            "local"
        }
        .into(),
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
        corrected: None,
        summary: None,
        translations: vec![],
        speech: any_remote.then(|| SpeechInfo {
            model: if remote {
                crate::soniox::RT_MODEL.into()
            } else {
                options.model.clone()
            },
            profile_id: resolved_profile,
            translation_language: options.translation_language.clone(),
            ..SpeechInfo::default()
        }),
    };
    let _writing = may.writing.lock().unwrap();
    let location = meeting_store::create(&may.root,&s)?;
    let dir = location.capture();
    meeting_bundle::changed(&may.root,&id,false);
    drop(_writing);
    let run = Arc::new(Run {
        id: id.clone(),
        epoch: audio::clock_hns(),
        stop: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        capture_done: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
        duration: Mutex::new(0.),
        failure: Mutex::new(None),
        speech: meeting_soniox::Control::default(),
    });
    run.speech.active.store(any_remote, Ordering::Release);
    run.speech
        .transcribing
        .store(options.transcribe, Ordering::Release);
    run.speech.set_target(options.translation_language.clone());
    run.speech
        .enabled
        .store(options.captions_enabled, Ordering::Release);
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
    let fallback_enabled = options.local_fallback;
    let live_handle = if options.transcribe {
        let live_may = may.clone();
        let live_run = run.clone();
        let live_app = app.clone();
        let live_dir = dir.clone();
        let language = options.language.clone();
        Some(std::thread::spawn(move || {
            let outcome = if let Some(profile) = groq_profile {
                let model = options.model.clone();
                let mut result = meeting_groq::run_from(
                    &live_may, &live_run, &live_app, &live_dir, &profile, &model, &language, 0,
                );
                if result.as_ref().is_err_and(fallback::eligible)
                    && !live_run.stop.load(Ordering::Acquire)
                    && !live_run.cancel.load(Ordering::Acquire)
                {
                    if let Some((backup, backup_model)) = transcription_backup {
                        let saved=live_may.edit(&live_run.id,|s|{
                            s.model=backup_model.clone();if let Some(info)=s.speech.as_mut(){info.profile_id=Some(backup.id.clone());info.model=backup_model.clone();}
                            s.warnings.push(format!("Chép lời đã chuyển sang {}/{} theo dự phòng đã chọn; bản thu và chữ đã có vẫn được giữ.",backup.name,backup_model));
                        });
                        result = match saved {
                            Ok(session) => {
                                let _ = live_app.emit("session-changed", live_run.id.clone());
                                if live_run.speech.enabled.load(Ordering::Acquire) {
                                    caption::emit_status(
                                        &live_app,
                                        "groq",
                                        "fallback",
                                        false,
                                        &format!(
                                            "Đang chuyển sang {}/{} theo dự phòng đã chọn.",
                                            backup.name, backup_model
                                        ),
                                    );
                                }
                                meeting_groq::run_from(
                                    &live_may,
                                    &live_run,
                                    &live_app,
                                    &live_dir,
                                    &backup,
                                    &backup_model,
                                    &language,
                                    (session.processed * 16000.) as usize,
                                )
                            }
                            Err(e) => Err(e),
                        };
                    }
                }
                live_run.speech.active.store(false, Ordering::Release);
                if live_run.speech.enabled.swap(false, Ordering::AcqRel) {
                    caption::emit_status(
                        &live_app,
                        "groq",
                        if result.is_err() { "error" } else { "idle" },
                        false,
                        if result.is_err() {
                            "Chép lời Groq đã ngắt; xem lỗi cuộc họp. Bản thu và chữ đã có vẫn được giữ."
                        } else {
                            "Đã dừng chép lời và phụ đề Groq."
                        },
                    );
                }
                result
            } else if remote {
                let mut result = meeting_soniox::run(
                    &live_may,
                    &live_run,
                    &live_app,
                    &live_dir,
                    key.as_deref().unwrap_or(""),
                    &language,
                );
                if result.as_ref().is_err_and(fallback::eligible)
                    && !live_run.stop.load(Ordering::Acquire)
                    && !live_run.cancel.load(Ordering::Acquire)
                {
                    if let Some((backup, _)) = transcription_backup {
                        let saved=live_may.edit(&live_run.id,|s|{
                            if let Some(info)=s.speech.as_mut(){info.profile_id=Some(backup.id.clone());}
                            s.warnings.push(format!("Chép lời đã chuyển sang {}/stt-rt-v6 theo cấu hình dự phòng. Âm thanh tiếp tục được thu; phần chưa chốt có thể được gửi lại để phục hồi.",backup.name));
                        });
                        result = match saved {
                            Ok(session) => {
                                if live_run.speech.enabled.load(Ordering::Acquire) {
                                    caption::emit_status(&live_app,"soniox","fallback",false,&format!("Đang chuyển chép lời/phụ đề sang {}/stt-rt-v6; nguồn thu vẫn chạy.",backup.name));
                                }
                                let _ = live_app.emit("session-changed", live_run.id.clone());
                                let _=live_app.emit("ai-fallback",serde_json::json!({"task":"caption","from":"Soniox chính/stt-rt-v6","to":format!("{}/stt-rt-v6",backup.name)}));
                                meeting_soniox::run_from(
                                    &live_may,
                                    &live_run,
                                    &live_app,
                                    &live_dir,
                                    &backup.key,
                                    &language,
                                    (session.processed * 16000.) as usize,
                                )
                            }
                            Err(error) => Err(error),
                        };
                    }
                }
                let had_translation = live_run.speech.enabled.swap(false, Ordering::AcqRel);
                live_run.speech.active.store(false, Ordering::Release);
                live_run.speech.set_target(None);
                if had_translation {
                    caption::emit_status(
                        &live_app,
                        "soniox",
                        if result.is_err() { "error" } else { "idle" },
                        false,
                        if result.is_err() {
                            "Dịch đã ngắt; xem lỗi trong cuộc họp. Bản thu và chữ đã có được giữ."
                        } else {
                            "Đã dừng dịch; bản gốc và bản dịch được giữ trong cuộc họp."
                        },
                    );
                    if live_run.capture_done.load(Ordering::Acquire) {
                        caption_window::hide_later(&live_app);
                    }
                }
                match result {
                    Err(error) if fallback_enabled && !live_run.cancel.load(Ordering::Acquire) => {
                        let saved = live_may.edit(&live_run.id, |s| {
                            s.provider = if s.segments.is_empty() { "local" } else { "mixed" }.into();
                            if let Some(info)=s.speech.as_mut(){ info.local_fallback=true; }
                            s.warnings.push(format!("Soniox đã ngắt: {error}. Dùng local theo tùy chọn dự phòng đã bật; dịch tạm dừng, bản thu và hai bản chữ đã có vẫn được giữ."));
                        });
                        match saved {
                            Ok(session) => stream_preview_from(
                                &live_may,
                                &live_run,
                                &live_app,
                                &live_dir,
                                &language,
                                (session.processed * 16000.) as usize,
                            ),
                            Err(error) => Err(error),
                        }
                    }
                    other => other,
                }
            } else {
                stream_preview(&live_may, &live_run, &live_app, &live_dir, &language)
            };
            live_run.speech.transcribing.store(false, Ordering::Release);
            if !any_remote && live_run.speech.enabled.swap(false, Ordering::AcqRel) {
                caption::emit_status(
                    &live_app,
                    "local",
                    "idle",
                    false,
                    "Đã dừng phụ đề; bản chép đã có vẫn được giữ.",
                );
                caption_window::hide_later(&live_app);
            }
            if let Err(e) = outcome {
                let _ = live_may.edit(&live_run.id, |s| {
                    s.error = Some(format!("Chép lời đã ngắt: {e}. Bản thu và chữ đã có vẫn được giữ; không tự đổi engine khi chưa bật dự phòng."));
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
        let failed = false;
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
            if stopped {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
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
            if options.transcribe && s.error.is_none() && !failed {
                if !s.live.is_empty() {
                    let words = s.live.clone();
                    let mut current_words = Vec::new();
                    let mut seg_start = 0.0;
                    for (i, w) in words.iter().enumerate() {
                        if current_words.is_empty() {
                            seg_start = w.start;
                        }
                        current_words.push(w.text.clone());
                        let seg_end = w.end;
                        let text_chunk = w.text.trim();
                        let is_end = text_chunk.ends_with('.')
                            || text_chunk.ends_with('?')
                            || text_chunk.ends_with('!');
                        if is_end || current_words.len() >= 14 || i == words.len() - 1 {
                            s.segments.push(Segment {
                                id: uuid::Uuid::new_v4().to_string(),
                                start: seg_start,
                                end: seg_end,
                                text: current_words.join(" "),
                                edited: false,
                            });
                            current_words.clear();
                        }
                    }
                    s.live.clear();
                }
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
        let mut active=may.active.lock().unwrap();
        if storage_paths::load_registry(&may.root).ok().and_then(|r|r.default_library_id).is_some() {
            let _writing=may.writing.lock().unwrap();
            let _=meeting_store::rename(&may.root,&id,false);
            meeting_bundle::changed(&may.root,&id,true);
        }
        active.take();drop(active);
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
    stream_preview_from(may, run, app, dir, language, 0)
}
fn stream_preview_from(
    may: &May,
    run: &Run,
    app: &tauri::AppHandle,
    dir: &std::path::Path,
    language: &str,
    start: usize,
) -> Result<()> {
    let session = storage::load(&may.root, &run.id)?;
    models::validate_mode(language, &session.model, "local")?;
    let selected_model = models::path(&may.root, &session.model)?;
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--live-worker")
        .arg(language)
        .arg(dir)
        .arg(start.to_string())
        .arg(selected_model)
        .arg(session.speech_quality.as_str());
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
                    if update.final_text && session.model == models::NEMO_ID {
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
                    if run.speech.enabled.load(Ordering::Acquire) && !update.text.trim().is_empty()
                    {
                        caption::emit_status(
                            app,
                            "local",
                            "active",
                            false,
                            "Phụ đề lời gốc đang chạy trên máy; không gửi âm thanh lên cloud.",
                        );
                        caption::emit_text(
                            app,
                            caption::CaptionText {
                                text: update.text.clone(),
                                original: String::new(),
                                final_: update.final_text,
                            },
                        );
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
                && [elapsed_ms, received_ms, emitted_ms, audio_ms]
                    .iter()
                    .all(|x| x.is_finite() && (0. ..=86_400_000.).contains(x))
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
        corrections: may
            .corrections
            .lock()
            .unwrap()
            .values()
            .map(|job| job.progress.lock().unwrap().clone())
            .collect(),
        translation_preview: may.current().and_then(|r| r.speech.preview()),
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
    persist_keys(&may);
    Ok(())
}
#[tauri::command]
fn set_soniox_key(may: State<May>, key: String) -> std::result::Result<(), String> {
    let key = soniox::validate_key(&key).map_err(err)?;
    *may.soniox_key.lock().unwrap() = key;
    persist_keys(&may);
    Ok(())
}
fn persist_keys(may: &May) {
    let soniox = may.soniox_key.lock().unwrap().clone();
    let llm = may.llm_key.lock().unwrap().clone();
    if let Err(e) = secure::save(&may.root, &soniox, &llm) {
        eprintln!("Không lưu được API key: {e:#}");
    }
}
#[tauri::command]
async fn test_llm_connection(
    may: State<'_, May>,
    provider: String,
    model: String,
    base_url: String,
    key: Option<String>,
) -> std::result::Result<String, String> {
    let m = may.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<String> {
        let api_key = key
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| m.llm_key.lock().unwrap().clone());
        anyhow::ensure!(!api_key.trim().is_empty(), "Cần nhập API key");
        llm::test_connection(&provider, &model, &base_url, &api_key)?;
        Ok("ok".into())
    })
    .await
    .map_err(err)?
    .map_err(err)
}
#[tauri::command]
async fn fetch_llm_models(
    may: State<'_, May>,
    provider: String,
    base_url: String,
    key: Option<String>,
    profile_id: Option<String>,
) -> std::result::Result<Vec<String>, String> {
    let m = may.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<String>> {
        let api_key = if let Some(key) = key.filter(|k| !k.trim().is_empty()) {
            key.trim().to_string()
        } else if let Some(id) = profile_id.as_deref() {
            let p = m.profile(id)?;
            anyhow::ensure!(
                p.provider == provider,
                "Thay nhà cung cấp cần nhập API key mới"
            );
            p.key
        } else {
            m.llm_key.lock().unwrap().clone()
        };
        anyhow::ensure!(!api_key.trim().is_empty(), "Cần nhập API key");
        llm::fetch_models(&provider, &base_url, &api_key)
    })
    .await
    .map_err(err)?
    .map_err(err)
}
#[tauri::command]
async fn summarize_transcript(
    may: State<'_, May>,
    id: String,
    provider: String,
    model: String,
    base_url: String,
    prompt: String,
    style: String,
    source: String,
    key: Option<String>,
    profile_id: Option<String>,
    fallback: Option<config::ModelChoice>,
) -> std::result::Result<Session, String> {
    let m = may.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Session> {
        let session = storage::load(&m.root, &id)?;
        anyhow::ensure!(
            !["recording", "paused", "processing"].contains(&session.status.as_str()),
            "Hãy đợi bản chép lời hoàn tất trước khi tóm tắt"
        );
        let (transcript, source_name) = if source == "corrected" {
            let corrected = session
                .corrected
                .as_ref()
                .context("Chưa có bản hiệu chỉnh để tóm tắt")?;
            (corrected.text.clone(), "corrected".to_string())
        } else if source == "smoothed" {
            match session.smoothed.as_ref() {
                Some(smoothed) => (smoothed.text.clone(), "smoothed".to_string()),
                None => anyhow::bail!("Chưa có bản làm mịn để tóm tắt"),
            }
        } else {
            let text = session
                .segments
                .iter()
                .map(|segment| segment.text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::ensure!(!text.is_empty(), "Chưa có bản chép lời để tóm tắt");
            (text, "original".to_string())
        };
        let mut hasher: u64 = 0;
        for unit in transcript.encode_utf16() {
            hasher = hasher.wrapping_mul(31).wrapping_add(unit as u64);
        }
        let source_hash = format!("{}", hasher & 0xffffffff);
        let (provider, base_url, api_key) =
            m.task_config(&provider, &base_url, key, profile_id.as_deref())?;
        anyhow::ensure!(!api_key.trim().is_empty(), "Cần nhập API key");
        let alternative = if let Some(choice) = fallback {
            anyhow::ensure!(
                profile_id.as_deref() != Some(choice.profile_id.as_str()) || model != choice.model,
                "Model dự phòng phải khác model chính"
            );
            let profile = m.profile(&choice.profile_id)?;
            anyhow::ensure!(
                ["groq", "openai", "gemini", "deepseek", "openai-compatible"]
                    .contains(&profile.provider.as_str())
                    && !groq::is_speech_model(&choice.model),
                "Model dự phòng không phù hợp tác vụ văn bản"
            );
            Some((profile, choice.model))
        } else {
            None
        };

        let (source_text, task_style, task_prompt) = (&transcript, &style, &prompt);
        let (summary_text, switched) = fallback::once(
            || {
                llm::summarize(
                    &provider,
                    &model,
                    &base_url,
                    &api_key,
                    &transcript,
                    &style,
                    &prompt,
                )
            },
            alternative.as_ref().map(|(p, m)| {
                move || {
                    llm::summarize(
                        &p.provider,
                        m,
                        &p.base_url,
                        &p.key,
                        source_text,
                        task_style,
                        task_prompt,
                    )
                }
            }),
        )?;
        let (provider, model, notice) = if switched {
            let (p, m) = alternative.unwrap();
            (
                p.provider,
                m.clone(),
                Some(format!(
                    "Tóm tắt đã chuyển sang {}/{m} theo cấu hình dự phòng.",
                    p.name
                )),
            )
        } else {
            (provider, model, None)
        };
        m.edit(&id, |s| {
            if let Some(notice) = notice {
                s.warnings.push(notice);
            }

            s.summary = Some(SummarizedTranscript {
                text: summary_text,
                provider,
                model,
                source: source_name,
                prompt,
                created: chrono::Utc::now().to_rfc3339(),
                source_hash,
            });
        })
    })
    .await
    .map_err(err)?
    .map_err(err)
}
#[tauri::command]
fn delete_summary(may: State<May>, id: String) -> std::result::Result<Session, String> {
    may.edit(&id, |s| s.summary = None).map_err(err)
}
#[tauri::command]
async fn correct_transcript(
    may: State<'_, May>,
    app: tauri::AppHandle,
    id: String,
    profile_id: String,
    model: String,
    fallback: Option<config::ModelChoice>,
) -> std::result::Result<Session, String> {
    let m = may.inner().clone();
    m.ensure_library(&id).map_err(err)?;
    if m.current().is_some_and(|r|r.id==id){return Err("Hãy đợi cuộc họp hoàn tất lưu bản thu trước khi hiệu chỉnh".into());}
    let session = storage::load(&m.root, &id).map_err(err)?;
    if ["recording", "paused", "processing"].contains(&session.status.as_str()) {
        return Err(
            "Hãy kết thúc cuộc họp và đợi file ghi âm hoàn tất trước khi hiệu chỉnh".into(),
        );
    }
    let profile = m.correction_profile(&profile_id).map_err(err)?;
    if !correction::supported(&profile.provider, &model) {
        return Err(
            "Chọn model nhận âm thanh cho Hiệu chỉnh; model chat không nghe lại bản thu được"
                .into(),
        );
    }
    let alternative = if let Some(choice) = fallback {
        let p = m.correction_profile(&choice.profile_id).map_err(err)?;
        if !correction::supported(&p.provider, &choice.model)
            || (choice.profile_id == profile_id && choice.model == model)
        {
            return Err("Model hiệu chỉnh dự phòng không phù hợp hoặc trùng model chính".into());
        }
        Some((p, choice.model))
    } else {
        None
    };
    let model_path = m
        .correction_model(&profile, &model, &session.language)
        .map_err(err)?;
    if let Some((p, model)) = &alternative {
        m.correction_model(p, model, &session.language)
            .map_err(err)?;
    }
    let control = Arc::new(correction::Control::new(
        id.clone(),
        session.duration,
        model.clone(),
    ));
    {
        let mut jobs = m.corrections.lock().unwrap();
        if !jobs.is_empty() {
            return Err("Đang có bản thu được hiệu chỉnh; hãy chờ hoặc hủy công việc đó".into());
        }
        jobs.insert(id.clone(), control.clone());
    }
    let operation = async {
        let path = storage::recording_path(&m.root, &id)?;
        let first = correction::transcribe(
            &profile,
            &model,
            &session.language,
            &path,
            &control,
            model_path.as_deref(),
        )
        .await;
        let corrected = match first {
            Err(error)
                if fallback::eligible(&error)
                    && !control.cancel.load(Ordering::Acquire)
                    && alternative.is_some() =>
            {
                let (p, model) = alternative.as_ref().unwrap();
                control.update(
                    "Đang chuyển sang model hiệu chỉnh dự phòng đã chọn",
                    0.,
                    model,
                );
                correction::transcribe(
                    p,
                    model,
                    &session.language,
                    &path,
                    &control,
                    m.correction_model(p, model, &session.language)?.as_deref(),
                )
                .await?
            }
            result => result?,
        };
        let _lock = m.writing.lock().unwrap();
        anyhow::ensure!(
            !control.cancel.load(Ordering::Acquire),
            "Đã hủy hiệu chỉnh; bản hiệu chỉnh trước được giữ nguyên"
        );
        let mut latest = storage::load(&m.root, &id)?;
        latest.corrected = Some(corrected);
        storage::save(&m.root, &latest)?;
        Ok::<Session, anyhow::Error>(latest)
    }
    .await;
    m.corrections.lock().unwrap().remove(&id);
    let _ = app.emit("session-changed", id);
    operation.map_err(err)
}
#[tauri::command]
fn cancel_correction(may: State<May>, id: String) -> std::result::Result<(), String> {
    if let Some(job) = may.corrections.lock().unwrap().get(&id) {
        job.cancel.store(true, Ordering::Release);
        let mut p = job.progress.lock().unwrap();
        p.cancelling = true;
        p.stage = "Đang hủy hiệu chỉnh".into();
    }
    Ok(())
}
#[tauri::command]
async fn test_soniox_connection(
    may: State<'_, May>,
    key: Option<String>,
) -> std::result::Result<String, String> {
    let m = may.inner().clone();
    let api_key = key
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| m.soniox_key.lock().unwrap().clone());
    if api_key.trim().is_empty() {
        return Err("Cần nhập API key Soniox".into());
    }
    if let Err(e) = soniox::test_connection(&api_key).await {
        let safe = classify_ai_error(&e, "soniox").replace(&api_key, "[redacted]");
        log_ai_test_failure(&m.root, "soniox", &safe);
        return Err(safe);
    }
    Ok("ok".into())
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
#[tauri::command]
fn list_ai_profiles(may: State<May>) -> std::result::Result<Vec<config::AiProfileSummary>, String> {
    config::load_checked(&may.root).map_err(err)?;
    Ok(may
        .profiles
        .lock()
        .unwrap()
        .iter()
        .map(config::to_summary)
        .collect())
}
#[tauri::command]
fn save_ai_profile(
    may: State<May>,
    input: config::AiProfileInput,
) -> std::result::Result<config::AiProfileSummary, String> {
    save_profile_inner(&may, input)
}
fn save_profile_inner(
    may: &May,
    input: config::AiProfileInput,
) -> std::result::Result<config::AiProfileSummary, String> {
    let (name, provider, base_url, key) = config::validate_input(&input).map_err(err)?;
    let mut stored = may.profiles.lock().unwrap();
    let mut profiles = stored.clone();
    let key = if key.is_empty() {
        let old = profiles
            .iter()
            .find(|p| Some(&p.id) == input.id.as_ref())
            .ok_or("Cần nhập API key trước khi lưu cấu hình mới")?;
        if old.provider != provider {
            return Err("Thay nhà cung cấp cần nhập key mới".into());
        }
        old.key.clone()
    } else {
        key
    };
    let fingerprint = config::verification_id(&provider, &base_url, &key);
    if !may
        .verified_ai
        .lock()
        .unwrap()
        .get(&fingerprint)
        .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(600))
    {
        return Err("Hãy kiểm tra kết nối thành công với cấu hình hiện tại trước khi lưu".into());
    }
    let profile = if let Some(id) = input.id {
        let p = profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or("Không tìm thấy cấu hình AI")?;
        p.name = name;
        p.provider = provider;
        p.base_url = base_url;
        p.key = key;
        p.clone()
    } else {
        let p = config::AiProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name,
            provider,
            base_url,
            key,
            created: chrono::Utc::now().to_rfc3339(),
            import_tag: String::new(),
        };
        profiles.push(p.clone());
        p
    };
    config::save(&may.root, &profiles).map_err(err)?;
    *stored = profiles;
    Ok(config::to_summary(&profile))
}

#[tauri::command]
fn delete_ai_profile(may: State<May>, id: String) -> std::result::Result<(), String> {
    let mut profiles = may.profiles.lock().unwrap();
    let mut next = profiles.clone();
    next.retain(|p| p.id != id);
    config::save(&may.root, &next).map_err(err)?;
    *profiles = next;
    Ok(())
}
/// Classify a provider HTTP error without ever echoing the API key.
fn classify_ai_error(e: &anyhow::Error, provider: &str) -> String {
    let msg = format!("{e:#}");
    let low = msg.to_lowercase();
    if low.contains("401")
        || low.contains("invalid_api_key")
        || low.contains("unauthorized")
        || low.contains("key không đúng")
    {
        return format!(
            "{provider}: API key không đúng hoặc đã hết hạn{}",
            if low.contains("401") { " (401)" } else { "" }
        );
    }
    if low.contains("403") || low.contains("permission") {
        return format!(
            "{provider}: key không có quyền dùng tài nguyên này{}",
            if low.contains("403") { " (403)" } else { "" }
        );
    }
    if low.contains("402") {
        return format!("{provider}: đã hết số dư hoặc chạm hạn mức chi tiêu (402)");
    }
    if low.contains("429")
        || low.contains("quota")
        || low.contains("rate limit")
        || low.contains("insufficient_quota")
    {
        return format!("{provider}: đã hết hạn mức hoặc vượt giới hạn yêu cầu");
    }
    if low.contains("404") || low.contains("model not found") || low.contains("model không") {
        return format!(
            "{provider}: model không đúng hoặc tài khoản không có quyền dùng model này"
        );
    }
    if low.contains("quá lâu") || low.contains("timed out") {
        return format!("{provider}: kiểm tra quá lâu; thử lại hoặc kiểm tra mạng/proxy");
    }
    if low.contains("kết nối")
        || low.contains("tcp")
        || low.contains("dns")
        || low.contains("connection")
        || low.contains("timeout")
    {
        return format!("{provider}: không kết nối được; kiểm tra mạng hoặc Base URL");
    }
    msg
}

/// Append the raw (key-free) failure detail to the data directory so QA can
/// diagnose provider-specific test failures without exposing the key.
fn log_ai_test_failure(root: &std::path::Path, provider: &str, detail: &str) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(root);
    let path = root.join("ai-test-errors.log");
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let line = format!(
            "[{}] provider={provider} detail={:?}\n",
            chrono::Local::now().to_rfc3339(),
            detail
        );
        let _ = file.write_all(line.as_bytes());
    }
}
#[tauri::command]
async fn test_ai_connection(
    may: State<'_, May>,
    input: config::AiTestInput,
) -> std::result::Result<String, String> {
    let m = may.inner().clone();
    let provider = config::normalize(&input.provider).map_err(err)?;
    let api_key = if input.api_key.trim().is_empty() {
        input
            .profile_id
            .as_deref()
            .and_then(|id| m.profile(id).ok())
            .filter(|p| p.provider == provider)
            .map(|p| p.key)
            .unwrap_or_default()
    } else {
        input.api_key.trim().to_string()
    };
    let base_url = if provider == "groq" {
        String::new()
    } else {
        input.base_url.trim().trim_end_matches('/').to_string()
    };
    let model = input.model.trim().to_string();
    m.verified_ai
        .lock()
        .unwrap()
        .remove(&config::verification_id(&provider, &base_url, &api_key));
    let provider_check = provider.clone();
    // Hard overall deadline so the connection test can never hang the UI.
    let outcome = async {
        if provider_check == "soniox" {
            anyhow::ensure!(
                model.is_empty() || model == soniox::RT_MODEL,
                "Model Soniox trực tiếp hiện dùng {}; model async chỉ xử lý file âm thanh",
                soniox::RT_MODEL
            );
            // Soniox is a WebSocket API: run it directly on the async runtime,
            // exactly like the legacy "Kiểm tra Soniox" button that works.
            tokio::time::timeout(
                std::time::Duration::from_secs(15),
                soniox::test_connection(&api_key),
            )
            .await
            .map_err(|_| anyhow::anyhow!("Soniox phản hồi quá lâu; kiểm tra mạng hoặc key"))??;
            return Ok::<String, anyhow::Error>("ok".to_string());
        }
        if model == "gpt-realtime-translate" || model == "gemini-3.5-live-translate-preview" {
            caption::test_connection(&provider, &api_key, &base_url, &model).await?;
            return Ok("ok".into());
        }
        // The LLM family is blocking HTTP: run it on the blocking pool without
        // nesting an async block_on inside it (that can stall the runtime).
        let provider_llm = provider_check.clone();
        let base_url_llm = base_url.clone();
        let api_key_llm = api_key.clone();
        let model_llm = model.clone();
        let job = tauri::async_runtime::spawn_blocking(move || -> Result<String> {
            // Step 1: validate the key against the provider (list models).
            llm::fetch_models(&provider_llm, &base_url_llm, &api_key_llm)
                .context("Kiểm tra key không thành công")?;
            // Step 2 (optional): verify this key may use the chosen model.
            if !model_llm.is_empty() {
                if provider_llm == "groq" && groq::is_speech_model(&model_llm) {
                    groq::test_speech(&api_key_llm, &base_url_llm, &model_llm)?;
                } else {
                    llm::test_connection(&provider_llm, &model_llm, &base_url_llm, &api_key_llm)
                        .context("Key hợp lệ nhưng chưa dùng được model này")?;
                }
            }
            Ok("ok".into())
        })
        .await
        .map_err(|e| anyhow::anyhow!("Kiểm tra kết nối bị gián đoạn: {e}"))??;
        Ok::<String, anyhow::Error>(job)
    };
    match tokio::time::timeout(std::time::Duration::from_secs(35), outcome).await {
        Ok(Ok(_)) => {
            m.verified_ai.lock().unwrap().insert(
                config::verification_id(&provider, &base_url, &api_key),
                std::time::Instant::now(),
            );
            Ok(if provider == "soniox" { "Đã xác thực Soniox và xử lý luồng PCM kiểm tra; chưa đánh giá độ chính xác bản dịch." }
               else if model.is_empty() { "Key hợp lệ qua danh sách model; chưa kiểm tra quyền sử dụng một model cụ thể." }
               else if provider == "groq" && groq::is_speech_model(&model) { "Key hợp lệ; Whisper đã xử lý audio tổng hợp kiểm tra. Chưa đánh giá độ chính xác với giọng nói thật." }
               else if model == "gpt-realtime-translate" || model == "gemini-3.5-live-translate-preview" { "Đã mở phiên model realtime thành công; chưa đánh giá bản dịch bằng giọng nói." }
               else { "Key hợp lệ và model đã trả lời yêu cầu kiểm tra." }.into())
        }
        Ok(Err(e)) => {
            let safe = classify_ai_error(&e, &provider);
            let safe = if api_key.is_empty() {
                safe
            } else {
                safe.replace(&api_key, "[redacted]")
            };
            log_ai_test_failure(&m.root, &provider, &safe);
            Err(safe)
        }
        Err(_) => {
            let message = format!(
                "{provider}: kiểm tra kết nối quá lâu. Kiểm tra mạng, proxy hoặc Base URL."
            );
            log_ai_test_failure(&m.root, &provider, &message);
            Err(message)
        }
    }
}
#[tauri::command]
async fn start_caption(
    app: tauri::AppHandle,
    may: State<'_, May>,
    options: caption::CaptionOptions,
) -> std::result::Result<(), String> {
    let m = may.inner().clone();
    if m.caption_running() {
        return Err("Đang có một phiên phụ đề chạy. Hãy dừng trước.".into());
    }
    if options.mode == "meeting" {
        let active = m.current().ok_or("Không có cuộc họp đang chạy")?;
        let session = storage::load(&m.root, &active.id).map_err(err)?;
        if session.provider == "groq" {
            if options.translate {
                return Err(
                    "Groq Whisper trong cuộc họp chỉ chép lời gốc; hãy bỏ tích Dịch sang".into(),
                );
            }
            if !active.speech.active.load(Ordering::Acquire)
                || !active.speech.transcribing.load(Ordering::Acquire)
                || active.capture_done.load(Ordering::Acquire)
            {
                return Err("Engine Groq đã dừng; xem lỗi cuộc họp".into());
            }
            let info = session
                .speech
                .as_ref()
                .ok_or("Thiếu model Groq đang chạy")?;
            if options.profile_id.as_deref() != info.profile_id.as_deref()
                || options.model != info.model
            {
                return Err(
                    "Hãy dùng cùng cấu hình/model Groq đang chép lời để tránh gửi audio hai lần"
                        .into(),
                );
            }
            active.speech.enabled.store(true, Ordering::Release);
            caption::set_translation_mode(&app, false);
            caption::emit_status(
                &app,
                "groq",
                "active",
                false,
                "Phụ đề dùng bản chép Groq của cuộc họp; không gửi audio lần hai.",
            );
            if let Some(segment) = session.segments.last() {
                caption::emit_text(
                    &app,
                    caption::CaptionText {
                        text: segment.text.clone(),
                        original: String::new(),
                        final_: true,
                    },
                );
            }
            return Ok(());
        }
    }
    if options.mode == "meeting" && !options.translate {
        let active = m.current().ok_or("Không có cuộc họp đang chạy")?;
        if !active.speech.active.load(Ordering::Acquire) {
            if !active.speech.transcribing.load(Ordering::Acquire) {
                return Err("Cuộc họp không có engine chép lời đang chạy; hãy bật chép lời khi tạo cuộc họp".into());
            }
            let session = storage::load(&m.root, &active.id).map_err(err)?;
            if !matches!(session.provider.as_str(), "local" | "mixed") {
                return Err("Chép lời đã ngắt; hãy kiểm tra lỗi cuộc họp".into());
            }
            if active.capture_done.load(Ordering::Acquire) {
                return Err("Cuộc họp đã dừng thu âm".into());
            }
            active.speech.enabled.store(true, Ordering::Release);
            caption::emit_status(
                &app,
                "local",
                "active",
                false,
                "Phụ đề lời gốc dùng bản chép local hiện có; không gửi thêm audio.",
            );
            return Ok(());
        }
    }
    if options.mode == "standalone" && options.profile_id.as_deref() == Some("__local__") {
        let mut opts = options.clone();
        opts.provider = "local".into();
        opts.api_key.clear();
        opts.base_url.clear();
        opts.fallback = None;
        opts.alternative = None;
        caption::validate_options(&opts).map_err(err)?;
        let info = models::list(&m.root)
            .into_iter()
            .find(|model| model.id == opts.model && model.installed)
            .ok_or(if opts.model.starts_with("moonshine-") {
                "Thiếu model Moonshine đi kèm; hãy cài lại Innote"
            } else {
                "Model local chưa được tải đầy đủ; mở Cài đặt để tải model trước"
            })?;
        opts.local_model = Some(models::path(&m.root, &info.id).map_err(err)?);
        let devices = m.devices.lock().unwrap();
        let device = devices
            .iter()
            .find(|d| {
                d.kind == "system"
                    && opts
                        .device
                        .as_deref()
                        .filter(|s| !s.is_empty())
                        .map_or(d.default, |id| d.id == id)
            })
            .ok_or("Không tìm thấy loa/tai nghe đã chọn")?;
        opts.device = Some(device.id.clone());
        drop(devices);
        let stop = Arc::new(AtomicBool::new(false));
        m.caption_begin(stop.clone()).map_err(err)?;
        caption::run(app, opts, stop, m.caption_done.clone());
        return Ok(());
    }
    let profile_id = options
        .profile_id
        .clone()
        .ok_or("Chưa chọn cấu hình AI cho phụ đề")?;
    let profile = m.profile(&profile_id).map_err(err)?;
    let mut opts = options.clone();
    opts.provider = profile.provider.clone();
    opts.api_key = profile.key.clone();
    opts.base_url = profile.base_url.clone();
    if opts.mode == "meeting" {
        let active = m
            .current()
            .ok_or("Không có phiên cuộc họp đang chạy để phụ đề")?;
        if active.speech.active.load(Ordering::Acquire) && profile.provider == "soniox" {
            let session = storage::load(&m.root, &active.id).map_err(err)?;
            let chosen = session
                .speech
                .as_ref()
                .and_then(|s| s.profile_id.as_deref());
            if chosen != Some(profile_id.as_str()) {
                return Err(
                    "Hãy dùng cùng cấu hình Soniox đang chép lời để tránh gửi audio hai lần".into(),
                );
            }
            caption::validate_options(&caption::CaptionOptions {
                session_dir: Some(storage::session_dir(&m.root, &active.id).map_err(err)?),
                ..opts.clone()
            })
            .map_err(err)?;
            active
                .speech
                .set_target(opts.translate.then(|| opts.target_language.clone()));
            active.speech.enabled.store(true, Ordering::Release);
            caption::emit_status(
                &app,
                "soniox",
                "connecting",
                true,
                if opts.translate {
                    "Đang bật dịch trên cùng luồng Soniox; bản chép gốc vẫn được giữ."
                } else {
                    "Đang bật phụ đề lời gốc trên luồng Soniox có sẵn, không yêu cầu dịch."
                },
            );
            return Ok(());
        }
        let dir = storage::session_dir(&m.root, &active.id).map_err(err)?;
        if !dir.join("system.pcm").exists() {
            return Err(
                "Cuộc họp chưa thu âm thanh hệ thống; hãy bật nguồn loa/tai nghe khi tạo cuộc họp"
                    .into(),
            );
        }
        opts.session_dir = Some(dir);
        opts.meeting_stop = Some(active.stop.clone());
    }
    if opts.mode == "standalone" {
        let devices = m.devices.lock().unwrap();
        let selected = opts.device.as_deref().filter(|d| !d.is_empty());
        let device = devices
            .iter()
            .find(|d| d.kind == "system" && selected.map_or(d.default, |id| d.id == id))
            .ok_or("Không tìm thấy loa/tai nghe đã chọn; hãy làm mới thiết bị trong Cài đặt")?;
        opts.device = Some(device.id.clone());
    }
    opts.display_model = format!("{}/{}", profile.name, opts.model);
    if let Some(choice) = opts.fallback.clone() {
        if choice.profile_id == profile.id && choice.model == opts.model {
            return Err("Model dự phòng phải khác model chính hoặc cấu hình chính".into());
        }
        let backup = m.profile(&choice.profile_id).map_err(err)?;
        let mut next = opts.clone();
        next.provider = backup.provider;
        next.api_key = backup.key;
        next.base_url = backup.base_url;
        next.model = choice.model;
        next.display_model = format!("{}/{}", backup.name, next.model);
        next.fallback = None;
        next.alternative = None;
        if !speech_quality::SpeechQuality::supported(&next.provider) {
            next.speech_quality = Default::default();
        }
        caption::validate_options(&next).map_err(err)?;
        opts.alternative = Some(Box::new(next));
    }
    caption::validate_options(&opts).map_err(err)?;
    if matches!(opts.provider.as_str(), "openai" | "gemini") && opts.model.trim().is_empty() {
        return Err("Hãy chọn model cho phụ đề".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    m.caption_begin(stop.clone()).map_err(err)?;
    caption::run(app, opts, stop, m.caption_done.clone());
    Ok(())
}
#[tauri::command]
fn set_meeting_translation(
    may: State<May>,
    target: Option<String>,
) -> std::result::Result<(), String> {
    let run = may.current().ok_or("Không có cuộc họp đang ghi")?;
    if storage::load(&may.root, &run.id).map_err(err)?.provider != "soniox" {
        return Err("Dịch cuộc họp hiện cần Soniox; Groq Whisper chỉ chép lời gốc".into());
    }
    if !run.speech.active.load(Ordering::Acquire) || run.capture_done.load(Ordering::Acquire) {
        return Err(
            "Dịch cuộc họp cần luồng Soniox đang hoạt động. Hãy chọn Soniox khi tạo cuộc họp."
                .into(),
        );
    }
    if target
        .as_deref()
        .is_some_and(|t| !["vi", "en", "ko", "zh", "ja"].contains(&t))
    {
        return Err("Ngôn ngữ dịch không hợp lệ".into());
    }
    may.edit(&run.id, |s| {
        if let Some(info) = &mut s.speech {
            info.translation_language = target.clone();
        }
    })
    .map_err(err)?;
    run.speech.set_target(target);
    Ok(())
}
#[tauri::command]
fn stop_caption(app: tauri::AppHandle, may: State<May>) -> std::result::Result<(), String> {
    let shared = may
        .current()
        .is_some_and(|r| r.speech.enabled.load(Ordering::Acquire));
    let provider = may
        .current()
        .and_then(|r| storage::load(&may.root, &r.id).ok())
        .map(|s| s.provider)
        .unwrap_or_else(|| "soniox".into());
    may.caption_stop_now();
    if shared {
        caption::emit_status(
            &app,
            &provider,
            "idle",
            false,
            "Đã tắt hiển thị phụ đề; bản chép và bản dịch đã chọn vẫn tiếp tục được lưu.",
        );
    }
    Ok(())
}
#[tauri::command]
fn caption_snapshot() -> caption::CaptionSnapshot {
    caption::snapshot()
}
#[tauri::command]
fn caption_running(may: State<May>) -> std::result::Result<bool, String> {
    Ok(may.caption_running())
}
#[tauri::command]
async fn open_caption_window(app: tauri::AppHandle) -> std::result::Result<(), String> {
    caption_window::show(app).await.map_err(err)
}
#[tauri::command]
fn caption_window_visible(app: tauri::AppHandle) -> bool {
    app.get_webview_window("caption")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}
#[tauri::command]
async fn hide_caption_window(app: tauri::AppHandle) -> std::result::Result<(), String> {
    caption_window::hide(app).await.map_err(err)
}
#[tauri::command]
async fn close_caption_window(app: tauri::AppHandle) -> std::result::Result<(), String> {
    // Stop/close means hide this reusable native window; ASR stop is separate.
    caption_window::hide(app).await.map_err(err)
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
    let result = may.edit(&id, |s| {
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
    .map_err(err)?;
    let defer=may.current().is_some() || !may.corrections.lock().unwrap().is_empty();
    if storage_paths::load_registry(&may.root).map_err(err)?.default_library_id.is_some() {
        meeting_store::rename(&may.root,&id,defer).map_err(err)?;
        if !defer { meeting_bundle::changed(&may.root,&id,true); }
    }
    Ok(result)
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
    let folder = storage::meeting_folder(&may.root,&id).map_err(err)?.join("exports");
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
    may: State<May>,
    id: String,
    format: String,
    path: Option<String>,
    version: Option<String>,
) -> std::result::Result<String, String> {
    let s = storage::load(&may.root, &id).map_err(err)?;
    let version = version.as_deref().unwrap_or("original");
    if let Some(path) = path.filter(|p| !p.trim().is_empty()) {
        let mut path = PathBuf::from(path);
        path.set_extension(&format);
        return Ok(storage::export_file_version(&path, &s, &format, version)
            .map_err(err)?
            .to_string_lossy()
            .into());
    }
    let folder = storage::meeting_folder(&may.root,&id).map_err(err)?.join("exports");
    Ok(storage::export_to_version(&folder, &s, &format, version)
        .map_err(err)?
        .to_string_lossy()
        .into())
}
#[tauri::command]
fn open_exports_folder(path:String) -> std::result::Result<(), String> {
    let file=PathBuf::from(path);
    if !file.is_absolute() || !file.is_file(){return Err("Không tìm thấy tệp đã xuất".into());}
    let folder=file.parent().ok_or("Không tìm thấy thư mục xuất")?;
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
        "feedback" => "https://forms.gle/hkeSofwG1YNGtwMM9",
        _ => return Err("Liên kết tác giả không hợp lệ".into()),
    };
    Command::new("explorer.exe").arg(url).spawn().map_err(err)?;
    Ok(())
}
#[tauri::command]
fn audio_path(app: tauri::AppHandle, may: State<May>, id: String) -> std::result::Result<String, String> {
    let p = storage::recording_path(&may.root, &id).map_err(err)?;
    if p.exists() {
        storage_paths::reject_reparse(&p).map_err(err)?;
        app.asset_protocol_scope().allow_file(&p).map_err(err)?;
        Ok(p.to_string_lossy().into())
    } else {
        Err("Bản thu chưa được hoàn tất".into())
    }
}

#[tauri::command]
fn meeting_storage_status(may: State<May>) -> std::result::Result<serde_json::Value,String> {
    let registry=storage_paths::load_registry(&may.root).map_err(err)?;
    let default=registry.libraries.iter().find(|l|Some(&l.id)==registry.default_library_id.as_ref());
    let mut issues=Vec::new();
    if registry.default_library_id.is_none(){issues.push("Chưa tạo được thư mục mặc định Innote Meetings trong thư mục cá nhân. Hãy chọn thư mục có quyền ghi.".to_owned());}
    for l in &registry.libraries {
        if !l.root.is_dir() { issues.push(format!("Ổ lưu chưa kết nối: {}",l.root.display())); }
        else if meeting_store::validate_registered(l).is_err() { issues.push(format!("Thư mục không khớp thư viện đã đăng ký: {}",l.root.display())); }
        else if l.root.join(".innote/meetings.sqlite").exists() {
            let readable=meeting_store::connection(&l.root.join(".innote/meetings.sqlite")).and_then(|c|Ok(c.query_row("SELECT COUNT(*) FROM sessions",[],|r|r.get::<_,i64>(0))?));
            if readable.is_err() { issues.push(format!("Chưa đọc được thư viện: {}",l.root.display())); }
        }
    }
    let free=default.and_then(|l|storage_paths::available_bytes(&l.root).ok());
    Ok(serde_json::json!({"path":default.map(|l|l.root.to_string_lossy()),"ready":default.map_or(false,|l|l.root.is_dir()),"available_bytes":free,"issues":issues,"private_path":may.root,"legacy_retained":true}))
}
#[tauri::command]
async fn choose_meetings_directory(app:tauri::AppHandle)->std::result::Result<Option<String>,String> {
    let suggested=app.path().home_dir().map_err(err)?;
    tauri::async_runtime::spawn_blocking(move ||rfd::FileDialog::new().set_title("Chọn thư mục lưu cuộc họp mới").set_directory(suggested).pick_folder().map(|p|p.to_string_lossy().into_owned())).await.map_err(err)
}
#[tauri::command]
fn set_meetings_directory(app:tauri::AppHandle,may:State<May>,path:String)->std::result::Result<(),String> {
    let active=may.active.lock().unwrap();
    let corrections=may.corrections.lock().unwrap();
    if !(active.is_none()&&corrections.is_empty()&&may.caption_done.load(Ordering::Acquire)) {return Err("Hãy kết thúc phiên/hiệu chỉnh/phụ đề trước khi đổi nơi lưu".into());}
    let _writing=may.writing.lock().unwrap();
    let runtime=app.path().resource_dir().map_err(err)?;
    let valid=storage_paths::validate_library_root(std::path::Path::new(&path),&may.root,&runtime).map_err(err)?;
    let mut registry=storage_paths::load_registry(&may.root).map_err(err)?;
    let canonical=std::fs::canonicalize(&path).map_err(err)?;
    if let Ok(bytes)=std::fs::read(canonical.join(".innote/library.json")) {
        if let Ok(marker)=serde_json::from_slice::<serde_json::Value>(&bytes) {
            if let Some(id)=marker["library_id"].as_str() {
                if registry.libraries.iter().any(|old|old.id==id && old.root!=canonical && !old.root.exists()) {
                    may.library_leases.lock().unwrap().remove(id);
                }
            }
        }
    }
    let l=if let Some(existing)=registry.libraries.iter().find(|l|l.root==canonical) {existing.clone()} else {storage_paths::open_or_create_library(&valid).map_err(err)?};
    registry.select(l.clone()).map_err(err)?;
    let mut leases=may.library_leases.lock().unwrap();
    let new_lease=if !leases.contains_key(&l.id) {Some(storage_paths::acquire_library_writer(&l.root).map_err(err)?)} else {None};
    storage_paths::save_registry(&may.root,&registry).map_err(err)?;
    if let Some(lease)=new_lease {leases.insert(l.id,lease);}
    Ok(())
}
#[tauri::command]
fn open_meetings_directory(may:State<May>)->std::result::Result<(),String> {
    let l=meeting_store::default_library(&may.root).map_err(err)?;
    storage_paths::reject_reparse(&l.root).map_err(err)?;
    Command::new("explorer.exe").arg(l.root).spawn().map_err(err)?; Ok(())
}
#[tauri::command]
fn open_meeting_directory(may:State<May>,id:String)->std::result::Result<(),String> {
    let folder=storage::meeting_folder(&may.root,&id).map_err(err)?;
    storage_paths::reject_reparse(&folder).map_err(err)?;
    Command::new("explorer.exe").arg(folder).spawn().map_err(err)?; Ok(())
}
#[tauri::command]
fn meeting_files_status(may:State<May>,id:String)->std::result::Result<serde_json::Value,String> {
    let l=meeting_store::locate(&may.root,&id).map_err(err)?;
    let (revision,published,error)=if l.modern {meeting_store::revision(&l,&id).map_err(err)?} else {(0,0,None)};
    let rename_pending=if l.modern {meeting_store::connection(&l.database).map_err(err)?.query_row::<Option<String>,_,_>("SELECT pending_folder_name FROM session_paths WHERE session_id=?1",[&id],|r|r.get(0)).map_err(err)?.is_some()} else {false};
    Ok(serde_json::json!({"path":l.folder,"legacy":!l.modern,"revision":revision,"published_revision":published,"error":error,"rename_pending":rename_pending}))
}
#[tauri::command]
fn retry_meeting_bundle(may:State<May>,id:String)->std::result::Result<(),String> {may.ensure_library(&id).map_err(err)?;meeting_bundle::publish(&may.root,&id).map_err(err)}
#[tauri::command]
fn rescan_meeting_libraries(may:State<May>)->std::result::Result<usize,String> {
    let active=may.active.lock().unwrap();let jobs=may.corrections.lock().unwrap();
    if active.is_some()||!jobs.is_empty(){return Err("Hãy đợi phiên và hiệu chỉnh kết thúc trước khi quét thư viện".into());}
    let _writing=may.writing.lock().unwrap();
    for s in storage::list(&may.root).map_err(err)? {may.ensure_library(&s.id).map_err(err)?;}
    meeting_store::rescan(&may.root).map_err(err)
}

#[tauri::command]
async fn choose_audio_export_path(
    may: State<'_, May>,
    id: String,
) -> std::result::Result<Option<String>, String> {
    let s = storage::load(&may.root, &id).map_err(err)?;
    let folder = storage::meeting_folder(&may.root,&id).map_err(err)?.join("exports");
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
    let source = storage::recording_path(&may.root, &id).map_err(err)?;
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
    let corrections = may.corrections.lock().unwrap();
    if corrections.contains_key(&id) {
        return Err("Hãy hủy hoặc chờ hiệu chỉnh xong trước khi xóa cuộc họp".into());
    }
    let active = may.active.lock().unwrap();
    if active.as_ref().map(|r| r.id == id).unwrap_or(false) {
        return Err("Không thể xóa phiên đang chạy".into());
    }
    let _lock = may.writing.lock().unwrap();
    may.ensure_library(&id).map_err(err)?;
    let dir = storage::meeting_folder(&may.root, &id).map_err(err)?;
    if dir.exists() {
        storage_paths::reject_reparse(&dir).map_err(err)?;
        std::fs::remove_dir_all(dir).map_err(err)?;
    }
    let modern = storage_paths::load_registry(&may.root).map_err(err)?.default_library_id.is_some();
    let db = if modern { meeting_store::connection(&meeting_store::locate(&may.root,&id).map_err(err)?.database) } else { storage::db(&may.root) };
    let mut db=db.map_err(err)?;let tx=db.transaction().map_err(err)?;
    tx.execute("DELETE FROM sessions WHERE id=?1", [&id]).map_err(err)?;
    if modern && meeting_store::locate(&may.root,&id).map(|l|l.modern).unwrap_or(false){tx.execute("DELETE FROM session_paths WHERE session_id=?1",[&id]).map_err(err)?;}
    tx.commit().map_err(err)?;
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
        speech: meeting_soniox::Control::default(),
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
    let library = meeting_store::default_library(&m.root).map_err(err)?;
    let dir = library.root.join(".innote/audio-tests").join(&id);
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.asset_protocol_scope().allow_file(dir.join("recording.wav")).map_err(err)?;
    let run = Arc::new(Run {
        id: id.clone(),
        epoch: audio::clock_hns(),
        stop: Arc::new(AtomicBool::new(false)),
        paused: Arc::new(AtomicBool::new(false)),
        capture_done: AtomicBool::new(false),
        cancel: AtomicBool::new(false),
        duration: Mutex::new(0.),
        failure: Mutex::new(None),
        speech: meeting_soniox::Control::default(),
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
            speech: meeting_soniox::Control::default(),
        })
    }
    fn may(root: PathBuf) -> May {
        May {
            root,
            library_leases: Arc::new(Mutex::new(std::collections::HashMap::new())),
            active: Arc::new(Mutex::new(None)),
            writing: Arc::new(Mutex::new(())),
            downloading: Arc::new(AtomicBool::new(false)),
            devices: Arc::new(Mutex::new(vec![])),
            soniox_key: Arc::new(Mutex::new(String::new())),
            llm_key: Arc::new(Mutex::new(String::new())),
            profiles: Arc::new(Mutex::new(Vec::new())),
            caption_stop: Arc::new(Mutex::new(None)),
            caption_done: Arc::new(AtomicBool::new(true)),
            verified_ai: Arc::new(Mutex::new(std::collections::HashMap::new())),
            corrections: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
    #[test]
    fn local_correction_needs_no_key_but_requires_installed_matching_model() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let m = may(dir);
        let local = m.correction_profile("__local__").unwrap();
        assert_eq!(local.provider, "local");
        assert!(local.key.is_empty());
        assert!(m.profiles.lock().unwrap().is_empty());
        assert!(m.correction_profile("unknown-profile").is_err());
        assert!(m.correction_model(&local, "moonshine-en", "vi").is_err());
        assert!(m.correction_model(&local, models::NEMO_ID, "ja").is_err());
        assert!(m.correction_model(&local, models::NEMO_ID, "en").is_err());
    }
    #[test]
    fn profiles_require_fresh_matching_verification_and_survive_restart() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let m = may(dir.clone());
        let input =
            |provider: &str, base: &str, key: &str, id: Option<String>| config::AiProfileInput {
                id,
                name: "Work API".into(),
                provider: provider.into(),
                base_url: base.into(),
                api_key: key.into(),
            };
        assert!(save_profile_inner(&m, input("soniox", "", "fake-key", None)).is_err());
        m.verified_ai.lock().unwrap().insert(
            config::verification_id("soniox", "", "fake-key"),
            std::time::Instant::now(),
        );
        let first = save_profile_inner(&m, input("soniox", "", "fake-key", None)).unwrap();
        let second = save_profile_inner(&m, input("soniox", "", "fake-key", None)).unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(config::load(&dir).len(), 2);
        assert!(save_profile_inner(
            &m,
            input("soniox", "", "changed-key", Some(first.id.clone()))
        )
        .is_err());
        assert!(
            save_profile_inner(&m, input("openai", "", "fake-key", Some(first.id.clone())))
                .is_err()
        );
        assert!(save_profile_inner(
            &m,
            input(
                "soniox",
                "https://proxy.test",
                "fake-key",
                Some(first.id.clone())
            )
        )
        .is_err());
        // Blank key on edit preserves it, but still requires this session's verification.
        save_profile_inner(&m, input("soniox", "", "", Some(first.id.clone()))).unwrap();
        let restarted = may(dir.clone());
        *restarted.profiles.lock().unwrap() = config::load(&dir);
        assert_eq!(restarted.profile(&first.id).unwrap().key, "fake-key");
        let task = restarted
            .task_config(
                "gemini",
                "https://wrong-endpoint.test",
                Some("wrong-key".into()),
                Some(&first.id),
            )
            .unwrap();
        assert_eq!(task, ("soniox".into(), "".into(), "fake-key".into()));
        assert!(restarted
            .task_config("openai", "", None, Some("deleted-profile"))
            .is_err());
        assert!(save_profile_inner(&restarted, input("soniox", "", "", Some(first.id))).is_err());
        assert!(!std::fs::read_to_string(config::profile_file(&dir))
            .unwrap()
            .contains("fake-key"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn captions_cannot_restart_while_previous_session_is_draining() {
        let m = may(std::env::temp_dir());
        m.caption_begin(Arc::new(AtomicBool::new(false))).unwrap();
        m.caption_stop_now();
        assert!(m.caption_running());
        assert!(m.caption_begin(Arc::new(AtomicBool::new(false))).is_err());
        m.caption_done.store(true, Ordering::Release);
        assert!(!m.caption_running());
        m.caption_begin(Arc::new(AtomicBool::new(false))).unwrap();
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
            speech_quality: Default::default(),
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
            corrected: None,
            summary: None,
            translations: vec![],
            speech: None,
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
    if args.get(1).map(String::as_str) == Some("--ai-probe") {
        // Explicit QA entry point: uses existing keys without echoing them or
        // modifying profiles. Captures no microphone or system audio.
        let result = (|| -> Result<serde_json::Value> {
            let provider = config::normalize(args.get(2).context("Usage: --ai-probe PROVIDER")?)?;
            let root = storage::root();
            let profiles = config::load(&root);
            let legacy = secure::load(&root);
            let key = profiles
                .iter()
                .find(|p| p.provider == provider)
                .map(|p| p.key.clone())
                .unwrap_or_else(|| match provider.as_str() {
                    "soniox" => legacy.0,
                    "openai" => legacy.1,
                    _ => String::new(),
                });
            if key.is_empty() {
                return Ok(serde_json::json!({"provider":provider,"available":false}));
            }
            let base = profiles
                .iter()
                .find(|p| p.provider == provider)
                .map(|p| p.base_url.as_str())
                .unwrap_or("");
            let started = std::time::Instant::now();
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let outcome = if provider == "soniox" {
                runtime.block_on(soniox::test_connection(&key))
            } else if let Some(model) = args.get(3) {
                runtime.block_on(caption::test_connection(&provider, &key, base, model))
            } else {
                llm::fetch_models(&provider, base, &key).map(|_| ())
            };
            let error = outcome
                .err()
                .map(|e| classify_ai_error(&e, &provider).replace(&key, "[redacted]"));
            Ok(
                serde_json::json!({"provider":provider,"available":true,"success":error.is_none(),"elapsed_ms":started.elapsed().as_millis(),"error":error}),
            )
        })();
        println!("{}", serde_json::to_string(&result.map_err(err)).unwrap());
        return;
    }
    if matches!(
        args.get(1).map(String::as_str),
        Some("--live-worker" | "--stream-test")
    ) {
        // Set before loading the native library or starting any child threads.
        // Avoid a BLAS thread pool competing with capture, Zoom and Whisper.
        let selected_nemo = args.get(5).is_some_and(|p| {
            std::path::Path::new(p)
                .extension()
                .is_some_and(|s| s == "gguf")
        }) || args
            .get(2)
            .is_some_and(|language| ["multi", "ko", "zh"].contains(&language.as_str()));
        if !selected_nemo {
            std::env::set_var("OPENBLAS_NUM_THREADS", "1");
            std::env::set_var("OMP_NUM_THREADS", "1");
        }
        let result = (|| -> Result<()> {
            let language = args.get(2).context("Cần ngôn ngữ vi/en/ko/zh/multi")?;
            let path = std::path::Path::new(args.get(3).context("Cần đường dẫn audio")?);
            if args[1] == "--stream-test" {
                live::benchmark(language, path)
            } else {
                live::worker_with_model(
                    language,
                    path,
                    args.get(4).and_then(|s| s.parse().ok()).unwrap_or(0),
                    args.get(5).map(|s| std::path::Path::new(s)),
                    speech_quality::SpeechQuality::parse(args.get(6).map(String::as_str))?,
                )
            }
        })();
        if let Err(e) = result {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--caption-stream-worker") {
        if args.get(5).map(String::as_str) == Some("file-correction") {
            if let Err(error) = correction::limit_local_cpu() {
                println!("{}", serde_json::json!({"error":error.to_string()}));
                return;
            }
        }
        let result = speech_quality::SpeechQuality::parse(args.get(4).map(String::as_str))
            .and_then(|quality| {
                local_stream_caption::worker(
                    std::path::Path::new(args.get(2).map(String::as_str).unwrap_or("")),
                    args.get(3).map(String::as_str).unwrap_or("multi"),
                    quality,
                )
            });
        if let Err(error) = result {
            println!("{}", serde_json::json!({"error":format!("{error:#}")}));
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
                speech_quality: Default::default(),
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
                corrected: None,
                summary: None,
                translations: vec![],
                speech: None,
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
    let available = std::thread::spawn(audio::devices)
        .join()
        .ok()
        .and_then(|r| r.ok())
        .unwrap_or_default();
    let may = May {
        root,
        library_leases: Arc::new(Mutex::new(std::collections::HashMap::new())),
        active: Arc::new(Mutex::new(None)),
        writing: Arc::new(Mutex::new(())),
        downloading: Arc::new(AtomicBool::new(false)),
        devices: Arc::new(Mutex::new(available)),
        soniox_key: Arc::new(Mutex::new(String::new())),
        llm_key: Arc::new(Mutex::new(String::new())),
        profiles: Arc::new(Mutex::new(Vec::new())),
        caption_stop: Arc::new(Mutex::new(None)),
        caption_done: Arc::new(AtomicBool::new(true)),
        verified_ai: Arc::new(Mutex::new(std::collections::HashMap::new())),
        corrections: Arc::new(Mutex::new(std::collections::HashMap::new())),
    };
    match may.lock_libraries() {
        Ok(()) => {if let Err(e)=storage::recover_where(&may.root,|id|may.ensure_library(id).is_ok()) {eprintln!("Recovery: {e:#}");}}
        Err(_) => eprintln!("Một thư viện đang bận; bỏ qua recovery để giữ nguyên dữ liệu"),
    }
    if let Ok(sessions)=storage::list(&may.root) {
        for s in sessions {
            if may.ensure_library(&s.id).is_err(){continue;}
            if let Ok(l)=meeting_store::locate(&may.root,&s.id) {
                if l.modern {
                    if let Some(root)=l.folder.parent(){let _=meeting_file_guide::ensure_library_guide(root);}
                    let _=meeting_file_guide::ensure_meeting_guide(&l.folder,&s);
                    let pending=meeting_store::connection(&l.database).and_then(|c|Ok(c.query_row::<Option<String>,_,_>("SELECT pending_folder_name FROM session_paths WHERE session_id=?1",[&s.id],|r|r.get(0))?));
                    if matches!(pending,Ok(Some(_))){let _=meeting_store::rename(&may.root,&s.id,false);}
                    if meeting_store::revision(&l,&s.id).is_ok_and(|(r,p,e)|r>p||e.is_some()){meeting_bundle::changed(&may.root,&s.id,false);}
                }
            }
        }
    }
    // Restore previously saved API keys so in-app updates / restarts do not
    // require the user to re-enter or reconnect them.
    {
        let (soniox, llm) = secure::load(&may.root);
        *may.soniox_key.lock().unwrap() = soniox.clone();
        *may.llm_key.lock().unwrap() = llm.clone();
        // Multi-profile store: load saved profiles and import the legacy keys
        // once, preserving compatibility with versions stored before 0.9.
        let mut profiles = config::load(&may.root);
        profiles = config::migrate_legacy(&may.root, profiles, &soniox, &llm);
        *may.profiles.lock().unwrap() = profiles;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(may)
        .setup(|app| {
            let state=app.state::<May>();
            let initialized=(|| -> Result<()> {
                storage_paths::initialize_default_library(&state.root,&app.path().home_dir()?,&app.path().resource_dir()?)?;
                state.lock_libraries()?;
                Ok(())
            })();
            if initialized.is_err(){eprintln!("Chưa khởi tạo được nơi lưu mặc định; có thể chọn nơi khác trong Cài đặt");}
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            meeting_storage_status, choose_meetings_directory, set_meetings_directory, open_meetings_directory, open_meeting_directory, meeting_files_status, retry_meeting_bundle, rescan_meeting_libraries,
            report_live_paint,
            set_soniox_key,
            test_soniox_connection,
            set_llm_key,
            test_llm_connection,
            fetch_llm_models,
            list_ai_profiles,
            save_ai_profile,
            delete_ai_profile,
            test_ai_connection,
            start_caption,
            stop_caption,
            set_meeting_translation,
            caption_running,
            caption_snapshot,
            open_caption_window,
            caption_window_visible,
            hide_caption_window,
            close_caption_window,
            correct_transcript,
            cancel_correction,
            summarize_transcript,
            delete_summary,
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
            if window.label() == "caption" && matches!(event, tauri::WindowEvent::Destroyed) {
                let _ = window.app_handle().emit("caption-window-visibility", false);
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<May>();
                if window.label() == "caption" {
                    api.prevent_close();
                    caption_window::hide_later(window.app_handle());
                    let shared = state
                        .current()
                        .is_some_and(|r| r.speech.enabled.load(Ordering::Acquire));
                    state.caption_stop_now();
                    if shared {
                        caption::emit_status(
                            window.app_handle(),
                            "soniox",
                            "idle",
                            false,
                            "Đã đóng phụ đề; Soniox vẫn tiếp tục chép lời cuộc họp.",
                        );
                    }
                    return;
                }
                if state.current().is_some() {
                    api.prevent_close();
                    let _ = window.emit("close-blocked", ());
                } else {
                    state.caption_stop_now();
                    // Do not synchronously destroy another native WebView from
                    // this window's close callback. Let the main window close,
                    // drain the standalone worker, then exit hidden windows too.
                    let app = window.app_handle().clone();
                    let done = state.caption_done.clone();
                    tauri::async_runtime::spawn(async move {
                        let until = std::time::Instant::now() + std::time::Duration::from_secs(15);
                        while !done.load(Ordering::Acquire) && std::time::Instant::now() < until {
                            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                        }
                        app.exit(0);
                    });
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("Không mở được Innote");
}
