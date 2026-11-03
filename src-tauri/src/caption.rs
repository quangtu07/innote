//! Real-time subtitle translation. Captures the chosen output device (or tails
//! the meeting's shared system.pcm so captions never double-capture audio a
//! meeting already records), streams PCM chunks to a cloud provider and emits
//! `caption-text` / `caption-status` events to every window.
//!
//! Providers (per official docs — Soniox real-time translation, OpenAI
//! Realtime Translation GA, Gemini Live API): Soniox transcribe-websocket with
//! a `translation` block (16 kHz PCM16); OpenAI `/v1/realtime/translations`
//! (24 kHz PCM16, resampled); Gemini Live `BidiGenerateContent` (16 kHz).
//! Keys and PCM never enter logs. Meeting final text is saved with the session.
use anyhow::{bail, ensure, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    io::Read,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tauri::Emitter;
use tokio_tungstenite::{
    connect_async_with_config,
    tungstenite::{client::IntoClientRequest, http::HeaderValue, Message},
};

const SONIOX_ENDPOINT: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
const OPENAI_TRANSLATE_ENDPOINT: &str = "wss://api.openai.com/v1/realtime/translations";
const GEMINI_ENDPOINT: &str =
    "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct CaptionText {
    /// Translated text; empty while there is nothing yet.
    pub text: String,
    /// Source-language text when the provider returns it.
    pub original: String,
    /// True when the provider finalized the phrase; false for live partials.
    pub final_: bool,
}

#[derive(Serialize, Clone)]
pub struct CaptionStatus {
    pub state: String,
    pub provider: String,
    /// Whether audio is currently being sent to the provider.
    pub sending: bool,
    pub message: String,
}

#[derive(Serialize, Clone)]
pub struct CaptionSnapshot {
    pub status: CaptionStatus,
    pub caption: CaptionText,
    pub translating: bool,
    pub paired: Option<CaptionText>,
}
impl CaptionSnapshot {
    fn accept(&mut self, text: &CaptionText) -> bool {
        if !text.final_ && self.caption == *text {
            return false;
        }
        self.caption = text.clone();
        if self.translating
            && text.final_
            && !text.text.trim().is_empty()
            && !text.original.trim().is_empty()
        {
            self.paired = Some(text.clone());
        }
        true
    }
}
static SNAPSHOT: std::sync::OnceLock<std::sync::Mutex<CaptionSnapshot>> =
    std::sync::OnceLock::new();
fn latest() -> &'static std::sync::Mutex<CaptionSnapshot> {
    SNAPSHOT.get_or_init(|| {
        std::sync::Mutex::new(CaptionSnapshot {
            translating: false,
            paired: None,
            status: CaptionStatus {
                state: "idle".into(),
                provider: String::new(),
                sending: false,
                message: String::new(),
            },
            caption: CaptionText {
                text: String::new(),
                original: String::new(),
                final_: true,
            },
        })
    })
}
pub fn snapshot() -> CaptionSnapshot {
    latest().lock().unwrap().clone()
}
pub(crate) fn set_translation_mode(app: &tauri::AppHandle, translating: bool) {
    let changed = {
        let mut current = latest().lock().unwrap();
        if current.translating == translating {
            false
        } else {
            current.translating = translating;
            current.paired = None;
            current.caption = CaptionText {
                text: String::new(),
                original: String::new(),
                final_: true,
            };
            true
        }
    };
    if changed {
        let _ = app.emit("caption-mode", translating);
    }
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionOptions {
    #[serde(default, alias = "speechQuality")]
    pub speech_quality: crate::speech_quality::SpeechQuality,
    pub provider: String,
    #[serde(alias = "api_key")]
    pub api_key: String,
    #[serde(alias = "base_url")]
    pub base_url: String,
    pub model: String,
    #[serde(alias = "target_language")]
    pub target_language: String,
    /// Omitted by older clients: preserve their translation behavior.
    #[serde(default = "translation_enabled")]
    pub translate: bool,
    #[serde(default, alias = "source_language")]
    pub source_language: Option<String>,
    #[serde(default)]
    pub fallback: Option<crate::config::ModelChoice>,
    #[serde(skip)]
    pub alternative: Option<Box<CaptionOptions>>,
    #[serde(skip)]
    pub audio_source: Option<AudioSource>,
    #[serde(skip)]
    pub retry_audio: Arc<std::sync::Mutex<crate::groq::RetryAudio>>,
    #[serde(skip)]
    pub local_model: Option<PathBuf>,
    #[serde(skip)]
    pub display_model: String,
    /// Loopback output device for standalone mode.
    pub device: Option<String>,
    /// "standalone" captures its own loopback stream; "meeting" tails the
    /// shared system.pcm produced by the running meeting.
    pub mode: String,
    #[serde(alias = "session_dir")]
    pub session_dir: Option<PathBuf>,
    /// Chosen AI profile; the backend resolves the key server-side.
    #[serde(alias = "profile_id")]
    pub profile_id: Option<String>,
    #[serde(skip)]
    pub meeting_stop: Option<Arc<AtomicBool>>,
}
fn translation_enabled() -> bool {
    true
}

pub fn validate_options(o: &CaptionOptions) -> Result<()> {
    ensure!(crate::speech_quality::SpeechQuality::supported(&o.provider) || o.speech_quality == Default::default(),
        "API/model này hiện chỉ hỗ trợ mức Cân bằng (mặc định)");
    if o.provider == "local" {
        ensure!(
            !o.translate && o.mode == "standalone",
            "Local dùng cho phụ đề lời gốc độc lập; hãy bỏ tích Dịch sang"
        );
        crate::local_caption::language(&o.model, o.source_language.as_deref())?;
        return Ok(());
    }
    ensure!(
        crate::config::CAPTION_PROVIDERS.contains(&o.provider.as_str()),
        "Phụ đề trực tiếp chỉ hỗ trợ Soniox, OpenAI hoặc Gemini"
    );
    ensure!(
        !o.api_key.trim().is_empty(),
        "Cấu hình phụ đề chưa có API key"
    );
    ensure!(
        !o.translate || ["vi", "en", "ko", "zh", "ja"].contains(&o.target_language.as_str()),
        "Mã ngôn ngữ đích không hợp lệ"
    );
    ensure!(
        o.translate || matches!(o.provider.as_str(), "soniox" | "groq"),
        "Phụ đề lời gốc độc lập hiện hỗ trợ Soniox; model đã chọn chỉ dùng để dịch"
    );
    ensure!(
        o.source_language
            .as_deref()
            .is_none_or(|s| ["", "auto", "multi", "vi", "en", "ko", "zh", "ja"].contains(&s)),
        "Ngôn ngữ gốc không hợp lệ"
    );
    ensure!(
        matches!(o.mode.as_str(), "standalone" | "meeting"),
        "Chế độ phụ đề không hợp lệ"
    );
    if o.mode == "meeting" {
        ensure!(o.session_dir.is_some(), "Thiếu phiên cuộc họp để phụ đề");
    } else {
        ensure!(o.device.is_some(), "Hãy chọn thiết bị âm thanh để phụ đề");
    }
    if o.provider == "groq" {
        ensure!(!o.translate && o.mode == "standalone", "Groq Whisper dùng cho phụ đề lời gốc độc lập; chưa hỗ trợ dịch streaming hoặc engine cuộc họp");
        ensure!(
            crate::groq::is_speech_model(&o.model),
            "Hãy chọn whisper-large-v3 hoặc whisper-large-v3-turbo cho phụ đề Groq"
        );
        return Ok(());
    }
    let expected = match o.provider.as_str() {
        "soniox" => "stt-rt-v5",
        "openai" => "gpt-realtime-translate",
        _ => "gemini-3.5-live-translate-preview",
    };
    ensure!(
        o.model == expected,
        "Model không phù hợp dịch trực tiếp; hãy chọn {expected}"
    );
    Ok(())
}
// ---------------------------------------------------------------------------
// Small utilities
// ---------------------------------------------------------------------------

/// Linear-interpolation resampler 16 kHz -> 24 kHz (OpenAI needs 24 kHz PCM16).
pub fn resample_16k_to_24k(input: &[i16]) -> Vec<i16> {
    let ratio = 3.0 / 2.0;
    let out_len = ((input.len() as f64) * ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f64 / ratio;
        let a = src.floor() as usize;
        let t = src - a as f64;
        let v0 = input.get(a).copied().unwrap_or(0) as f64;
        let v1 = input.get(a + 1).map_or(v0, |x| *x as f64);
        out.push((v0 + (v1 - v0) * t).round().clamp(-32768., 32767.) as i16);
    }
    out
}

/// Minimal standard base64 encoder (keeps the caption module dependency-free).
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Audio producers: one shared 16 kHz mono f32 stream into a tokio channel.
// ---------------------------------------------------------------------------

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
pub(crate) type Sink = tokio::sync::mpsc::Sender<Result<Vec<f32>>>;
pub(crate) type AudioSource =
    Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<Vec<f32>>>>>;
pub(crate) fn audio_input(opts: &CaptionOptions, stop: Arc<AtomicBool>) -> Result<AudioSource> {
    if let Some(source) = &opts.audio_source {
        return Ok(source.clone());
    }
    let (tx, rx) = tokio::sync::mpsc::channel(3200);
    if opts.mode == "meeting" {
        spawn_tailer(
            opts.session_dir.as_deref().context("Thiếu cuộc họp")?,
            tx,
            stop,
        );
    } else {
        spawn_standalone_capture(opts.device.clone().context("Thiếu loa/tai nghe")?, tx, stop)?;
    }
    Ok(Arc::new(tokio::sync::Mutex::new(rx)))
}
async fn send(socket: &mut Socket, message: Message) -> Result<()> {
    tokio::time::timeout(Duration::from_secs(5), socket.send(message))
        .await
        .map_err(|_| anyhow::anyhow!("Gửi dữ liệu AI quá lâu; kiểm tra mạng"))?
        .map_err(|_| anyhow::anyhow!("Không gửi được dữ liệu tới AI; kiểm tra mạng"))
}

fn connection_error(provider: &str, e: tokio_tungstenite::tungstenite::Error) -> anyhow::Error {
    if let tokio_tungstenite::tungstenite::Error::Http(response) = e {
        let code = response.status().as_u16();
        let reason = match code {
            401 => "API key không đúng hoặc đã hết hạn",
            403 => "key không có quyền dùng model hoặc tài nguyên",
            402 | 429 => "đã hết số dư, hạn mức hoặc vượt giới hạn yêu cầu",
            400 | 404 => "model hoặc cấu hình không hợp lệ",
            _ => "dịch vụ chưa sẵn sàng",
        };
        return anyhow::anyhow!("{provider} {code}: {reason}");
    }
    anyhow::anyhow!("{provider}: không kết nối được; kiểm tra mạng, TLS hoặc Base URL")
}

async fn ready(socket: &mut Socket, provider: &str) -> Result<()> {
    if provider == "soniox" {
        return Ok(());
    }
    tokio::time::timeout(Duration::from_secs(8), async {
        while let Some(message) = socket.next().await {
            let bytes = match message.map_err(|e| connection_error(provider, e))? {
                Message::Text(t) => t.into_bytes(),
                Message::Binary(b) => b,
                Message::Ping(_) => {
                    socket.flush().await?;
                    continue;
                }
                Message::Close(_) => bail!("{provider}: phiên đóng trước khi xác nhận model"),
                _ => continue,
            };
            let v: Value = serde_json::from_slice(&bytes).context("AI trả JSON không hợp lệ")?;
            if provider == "openai" {
                OpenAiState::new().push(&v)?;
            } else {
                GeminiState::new().push(&v)?;
            }
            if v["type"] == "session.updated" || v.get("setupComplete").is_some() {
                return Ok(());
            }
        }
        bail!("{provider}: phiên đóng trước khi xác nhận model")
    })
    .await
    .map_err(|_| anyhow::anyhow!("{provider}: xác nhận model quá lâu"))?
}

/// Key checks and realtime model checks use different APIs. No audio is captured here.
pub async fn test_connection(provider: &str, key: &str, base: &str, model: &str) -> Result<()> {
    let opts = CaptionOptions {
        speech_quality: Default::default(),
        provider: provider.into(),
        api_key: key.into(),
        base_url: base.into(),
        model: model.into(),
        target_language: "vi".into(),
        translate: true,
        source_language: None,
        fallback: None,
        alternative: None,
        audio_source: None,
        retry_audio: Default::default(),
        local_model: None,
        display_model: String::new(),
        device: Some(String::new()),
        mode: "standalone".into(),
        session_dir: None,
        profile_id: None,
        meeting_stop: None,
    };
    validate_options(&opts)?;
    tokio::time::timeout(Duration::from_secs(15), async {
        let mut socket = opts.connect().await?;
        send(&mut socket, opts.init_message()?).await?;
        ready(&mut socket, provider).await?;
        let _ = tokio::time::timeout(Duration::from_millis(300), socket.close(None)).await;
        Ok(())
    })
    .await
    .map_err(|_| anyhow::anyhow!("{provider}: kiểm tra model realtime quá lâu"))?
}

/// Tail appended bytes of the meeting's system.pcm into the caption sink so
/// the meeting and captions share one capture stream (no double capture). The
/// tail starts where the file currently ends, so joining mid-meeting does not
/// re-send archived audio.
fn spawn_tailer(dir: &Path, sink: Sink, stop: Arc<AtomicBool>) {
    let target = dir.join("system.pcm");
    std::thread::spawn(move || {
        let mut offset = std::fs::metadata(&target)
            .map(|m| (m.len() / 2) as usize)
            .unwrap_or(0);

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let available = std::fs::metadata(&target)
                .map(|m| (m.len() / 2) as usize)
                .unwrap_or(0);
            if available > offset {
                let len = (available - offset).min(3200);
                let mut bytes = vec![0u8; len * 2];
                let mut used = 0;
                if let Ok(mut f) = std::fs::File::open(&target) {
                    use std::io::Seek;
                    let _ = f.seek(std::io::SeekFrom::Start(offset as u64 * 2));
                    while used < bytes.len() {
                        match f.read(&mut bytes[used..]) {
                            Ok(0) => break,
                            Ok(n) => used += n,
                            Err(_) => break,
                        }
                    }
                    offset += used / 2;
                    if used >= 2 {
                        let samples: Vec<f32> = bytes[..used - used % 2]
                            .chunks_exact(2)
                            .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.)
                            .collect();
                        if sink.blocking_send(Ok(samples)).is_err() {
                            break;
                        }

                        continue;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    });
}

/// Standalone mode: capture the selected loopback output device into the sink.
pub(crate) fn spawn_standalone_capture(
    device: String,
    sink: Sink,
    stop: Arc<AtomicBool>,
) -> Result<()> {
    let (tx, rx) = std::sync::mpsc::channel::<Vec<f32>>();
    let errors = sink.clone();
    std::thread::spawn(move || {
        if crate::audio::capture_frames(&device, stop.clone(), Arc::new(AtomicBool::new(false)), tx)
            .is_err()
        {
            let _ = errors.blocking_send(Err(anyhow::anyhow!(
                "Không mở được loa/tai nghe đã chọn; kiểm tra thiết bị âm thanh Windows"
            )));
        }
    });
    std::thread::spawn(move || {
        while let Ok(chunk) = rx.recv() {
            if sink.blocking_send(Ok(chunk)).is_err() {
                break;
            }
        }
    });
    Ok(())
}
// ---------------------------------------------------------------------------
// Provider token parsers (pure, unit-tested without network).
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SonioxState {
    phrase_orig: String,
    phrase_tr: String,
    originals: std::collections::VecDeque<String>,
}
impl SonioxState {
    fn new() -> Self {
        Self::default()
    }
    fn push_source(&mut self, v: &Value) -> Result<Vec<CaptionText>> {
        if v.get("error_code").is_some() || v.get("error_type").is_some() {
            bail!(crate::soniox::describe_error(
                v["error_code"].as_u64().unwrap_or(500)
            ));
        }
        ensure!(
            v["tokens"].is_array() || v["finished"] == true,
            "Soniox trả dữ liệu không hợp lệ"
        );
        let mut out = Vec::new();
        let mut interim = String::new();
        for token in v["tokens"].as_array().into_iter().flatten() {
            if token["translation_status"] == "translation" {
                continue;
            }
            let word = token["text"].as_str().context("Soniox thiếu token")?;
            if matches!(word, "<end>" | "<fin>") {
                if token["is_final"] == true && !self.phrase_orig.trim().is_empty() {
                    out.push(CaptionText {
                        text: std::mem::take(&mut self.phrase_orig).trim().into(),
                        original: String::new(),
                        final_: true,
                    });
                }
            } else if token["is_final"] == true {
                if crate::speech_quality::confirmed_chunk_boundary(&self.phrase_orig, word) {
                    out.push(CaptionText {
                        text: std::mem::take(&mut self.phrase_orig).trim().into(),
                        original: String::new(),
                        final_: true,
                    });
                }
                self.phrase_orig.push_str(word);
            } else {
                interim.push_str(word);
            }
        }
        ensure!(
            self.phrase_orig.len() + interim.len() < 64000,
            "Soniox thiếu ranh giới câu quá lâu"
        );
        if v["finished"] == true {
            if !self.phrase_orig.trim().is_empty() {
                out.push(CaptionText {
                    text: std::mem::take(&mut self.phrase_orig).trim().into(),
                    original: String::new(),
                    final_: true,
                });
            }
        } else {
            let text = format!("{}{}", self.phrase_orig, interim);
            if !text.trim().is_empty() {
                out.push(CaptionText {
                    text,
                    original: String::new(),
                    final_: false,
                });
            }
        }
        Ok(out)
    }
    fn flush(&mut self) -> Option<CaptionText> {
        let original = self
            .originals
            .pop_front()
            .unwrap_or_else(|| std::mem::take(&mut self.phrase_orig));
        let text = std::mem::take(&mut self.phrase_tr);
        if text.trim().is_empty() && original.trim().is_empty() {
            return None;
        }
        Some(CaptionText {
            text: text.trim().into(),
            original: original.trim().into(),
            final_: true,
        })
    }
    fn push(&mut self, v: &Value) -> Result<Vec<CaptionText>> {
        if v.get("error_code").is_some() || v.get("error_type").is_some() {
            bail!(crate::soniox::describe_error(
                v["error_code"].as_u64().unwrap_or(500)
            ));
        }
        ensure!(
            v.is_object() && (v["tokens"].is_array() || v["finished"] == true),
            "Soniox trả dữ liệu không hợp lệ"
        );
        let mut out = Vec::new();
        let (mut live_orig, mut live_tr) = (String::new(), String::new());
        for t in v["tokens"].as_array().into_iter().flatten() {
            let text = t["text"].as_str().context("Soniox thiếu nội dung token")?;
            let final_ = t["is_final"] == true;
            let translation = t["translation_status"] == "translation";
            if matches!(text, "<end>" | "<fin>") {
                if !final_ {
                    continue;
                }
                // Original endpoints can precede translated text. Keep them queued
                // until that translation's endpoint, never clear both streams early.
                if translation
                    || t["translation_status"] == "none"
                    || (t["translation_status"].is_null() && !self.phrase_tr.is_empty())
                {
                    if let Some(event) = self.flush() {
                        out.push(event);
                    }
                    live_tr.clear();
                    live_orig.clear();
                } else if !self.phrase_orig.trim().is_empty() {
                    self.originals
                        .push_back(std::mem::take(&mut self.phrase_orig));
                    live_orig.clear();
                }
                continue;
            }
            let target = if translation {
                if final_ {
                    &mut self.phrase_tr
                } else {
                    &mut live_tr
                }
            } else {
                if final_ {
                    &mut self.phrase_orig
                } else {
                    &mut live_orig
                }
            };
            target.push_str(text);
            // "none" means already in target language, so show it as-is.
            if t["translation_status"] == "none" {
                if final_ {
                    self.phrase_tr.push_str(text);
                } else {
                    live_tr.push_str(text);
                }
            }
            ensure!(
                self.phrase_orig.len() + self.phrase_tr.len() + live_orig.len() + live_tr.len()
                    < 64000
                    && self.originals.len() < 64,
                "Soniox thiếu ranh giới câu quá lâu; hãy bật lại phụ đề"
            );
        }
        if v["finished"] == true {
            if let Some(event) = self.flush() {
                out.push(event);
            }
            while let Some(original) = self.originals.pop_front() {
                if !original.trim().is_empty() {
                    out.push(CaptionText {
                        text: String::new(),
                        original,
                        final_: true,
                    });
                }
            }
        } else if !self.phrase_tr.is_empty()
            || !live_tr.is_empty()
            || !self.phrase_orig.is_empty()
            || !live_orig.is_empty()
        {
            out.push(CaptionText {
                text: format!("{}{}", self.phrase_tr, live_tr),
                original: format!(
                    "{}{}",
                    self.originals.front().unwrap_or(&self.phrase_orig),
                    live_orig
                ),
                final_: false,
            });
        }
        Ok(out)
    }
}

pub(crate) fn emit_text(app: &tauri::AppHandle, text: CaptionText) {
    let translating = {
        let mut snapshot = latest().lock().unwrap();
        if !snapshot.accept(&text) {
            return;
        }
        snapshot.translating
    };
    let _ = app.emit("caption-text", json!({"text":text.text,"original":text.original,"final_":text.final_,"translating":translating}));
}

pub(crate) fn emit_status(
    app: &tauri::AppHandle,
    provider: &str,
    state: &str,
    sending: bool,
    message: &str,
) {
    if provider == "local" || state == "idle" || state == "error" {
        set_translation_mode(app, false);
    }
    {
        let mut snapshot = latest().lock().unwrap();
        snapshot.status = CaptionStatus {
            state: state.into(),
            provider: provider.into(),
            sending,
            message: message.into(),
        };
        if state == "connecting" {
            snapshot.paired = None;
            snapshot.caption = CaptionText {
                text: String::new(),
                original: String::new(),
                final_: true,
            };
        }
    }
    let _ = app.emit(
        "caption-status",
        CaptionStatus {
            state: state.into(),
            provider: provider.into(),
            sending,
            message: message.into(),
        },
    );
}
fn trim_caption(text: &mut String, max: usize) {
    if let Some((offset, _)) = text.char_indices().rev().nth(max) {
        text.drain(..offset);
    }
}
struct OpenAiState {
    translation: String,
    original: String,
}

impl OpenAiState {
    fn new() -> Self {
        Self {
            translation: String::new(),
            original: String::new(),
        }
    }
    fn push(&mut self, v: &Value) -> Result<Vec<CaptionText>> {
        let event = v["type"].as_str().unwrap_or("");
        if event == "error" {
            let code = v["error"]["code"].as_str().map(String::from);
            let message = v["error"]["message"]
                .as_str()
                .unwrap_or("Lỗi không xác định");
            if matches!(
                code.as_deref(),
                Some("invalid_api_key") | Some("authentication_error")
            ) {
                bail!("OpenAI: API key không đúng hoặc đã hết hạn");
            }
            if matches!(
                code.as_deref(),
                Some("insufficient_quota") | Some("rate_limit_exceeded")
            ) {
                bail!("OpenAI: đã hết hạn mức hoặc vượt giới hạn yêu cầu");
            }
            if message.to_ascii_lowercase().contains("model not found") {
                bail!("OpenAI: model không đúng hoặc tài khoản không có quyền dùng model này");
            }
            bail!("OpenAI: lỗi yêu cầu hoặc dịch vụ; kiểm tra model và hạn mức");
        }
        let mut out = Vec::new();
        match event {
            "session.input_transcript.delta"
            | "conversation.item.input_audio_transcription.delta" => {
                if let Some(t) = v["delta"].as_str() {
                    self.original.push_str(t);
                    trim_caption(&mut self.original, 800);
                }
            }
            "session.output_transcript.delta"
            | "response.output_audio_transcript.delta"
            | "response.output_transcript.delta"
            | "response.output_text.delta" => {
                if let Some(t) = v["delta"].as_str() {
                    self.translation.push_str(t);
                }
                let text = self.translation.trim().to_string();
                if !text.is_empty() {
                    // Translation has no response.done turns. Bound the rolling caption
                    // at sentence boundaries or 240 characters.
                    if text.chars().count() >= 240
                        || text.ends_with(['.', '!', '?', '。', '！', '？'])
                    {
                        out.push(self.flush());
                    } else {
                        out.push(CaptionText {
                            text,
                            original: self.original.clone(),
                            final_: false,
                        });
                    }
                }
            }
            "response.done"
            | "response.completed"
            | "response.output_transcript.done"
            | "response.output_audio_transcript.done" => {
                // The bulk final text replaces the live deltas accumulated so
                // far (they are the same phrase, not an additional one).
                let text = v["response"]["output"]
                    .as_array()
                    .and_then(|out| {
                        out.iter().find_map(|item| {
                            item["output"].as_array().and_then(|content| {
                                content
                                    .iter()
                                    .find_map(|c| c["transcript"].as_str().or(c["text"].as_str()))
                            })
                        })
                    })
                    .map(|s| s.to_string());
                if let Some(t) = text.filter(|t| !t.trim().is_empty()) {
                    self.translation = t.trim().to_string();
                }
                out.push(self.flush());
            }
            "session.closed" => {
                let text = self.translation.trim().to_string();
                if !text.is_empty() {
                    out.push(CaptionText {
                        text,
                        original: self.original.clone(),
                        final_: true,
                    });
                }
            }
            _ => {}
        }
        Ok(out)
    }
    fn flush(&mut self) -> CaptionText {
        let translation = std::mem::take(&mut self.translation);
        let original = std::mem::take(&mut self.original);
        CaptionText {
            text: translation.trim().to_string(),
            original: original.trim().to_string(),
            final_: true,
        }
    }
}
struct GeminiState {
    translation: String,
    original: String,
}

impl GeminiState {
    fn new() -> Self {
        Self {
            translation: String::new(),
            original: String::new(),
        }
    }
    fn push(&mut self, v: &Value) -> Result<Vec<CaptionText>> {
        if let Some(err) = v.get("error") {
            let code = err["code"].as_u64().unwrap_or(500);
            let message = err["message"].as_str().unwrap_or("Lỗi không xác định");
            if code == 401 || (code == 400 && message.to_ascii_lowercase().contains("api key")) {
                bail!("Gemini: API key không đúng hoặc đã hết hạn");
            }
            if code == 403 {
                bail!("Gemini 403: key không có quyền dùng model này");
            }
            if code == 429 {
                bail!("Gemini: đã hết hạn mức hoặc vượt giới hạn yêu cầu");
            }
            if code == 404 || message.to_ascii_lowercase().contains("model not found") {
                bail!("Gemini: model không đúng hoặc tài khoản không có quyền dùng model này");
            }
            bail!("Gemini: lỗi yêu cầu hoặc dịch vụ; kiểm tra model và hạn mức");
        }
        let mut out = Vec::new();
        let content = &v["serverContent"];
        if let Some(t) = content["inputTranscription"]["text"].as_str() {
            self.original.push_str(t);
            trim_caption(&mut self.original, 800);
        }
        if let Some(t) = content["outputTranscription"]["text"].as_str() {
            self.translation.push_str(t);
        }
        let complete = content["turnComplete"] == true
            || v.get("goAway").is_some()
            || self.translation.chars().count() >= 240;
        if !self.translation.is_empty() || !self.original.is_empty() {
            out.push(CaptionText {
                text: self.translation.trim().into(),
                original: self.original.trim().into(),
                final_: complete,
            });
            if complete {
                self.translation.clear();
                self.original.clear();
            }
        }
        if v.get("goAway").is_some() {
            bail!("Gemini yêu cầu kết nối lại; hãy bật lại phụ đề");
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------------
// Session loop: connect to the provider, stream shared audio, emit events.
// ---------------------------------------------------------------------------

struct Session {
    app: tauri::AppHandle,
    opts: CaptionOptions,
    stop: Arc<AtomicBool>,
}
impl Session {
    fn publish(&self, text: CaptionText) {
        // Non-shared providers still retain the returned original and translation.
        // They have no source timestamps here: keep them explicitly unaligned.
        if text.final_ && !text.text.trim().is_empty() && self.opts.mode == "meeting" {
            if let Some(id) = self
                .opts
                .session_dir
                .as_ref()
                .and_then(|p| p.file_name())
                .and_then(|s| s.to_str())
            {
                use tauri::Manager;
                let may = self.app.state::<crate::May>();
                if let Err(_) = may.edit(id, |session| {
                    session.translations.push(crate::types::TranslationSegment {
                        id: uuid::Uuid::new_v4().to_string(),
                        source_id: String::new(),
                        source_ids: vec![],
                        start: 0.,
                        end: 0.,
                        original: text.original.clone(),
                        text: text.text.clone(),
                        language: self.opts.target_language.clone(),
                        provider: self.opts.provider.clone(),
                    })
                }) {
                    let _ = self.app.emit(
                        "storage-error",
                        "Không lưu được bản dịch; hãy kiểm tra thư mục dữ liệu.",
                    );
                }
                let _ = self.app.emit("session-changed", id);
            }
        }
        emit_text(&self.app, text);
    }
    async fn run(&self) -> Result<()> {
        set_translation_mode(&self.app, self.opts.translate);
        if self.opts.provider == "local" {
            return crate::local_caption::run(&self.app, &self.opts, self.stop.clone(), |text| {
                self.publish(text)
            })
            .await;
        }
        if self.opts.provider == "groq" {
            let result =
                crate::groq::run_captions(&self.app, &self.opts, self.stop.clone(), |text| {
                    self.publish(text)
                })
                .await;
            if result.is_ok() {
                emit_status(&self.app, "groq", "idle", false, "Đã dừng phụ đề Groq.");
            }
            return result;
        }
        let provider = self.opts.provider.as_str();
        emit_status(
            &self.app,
            provider,
            "connecting",
            false,
            "Đang kết nối nhà cung cấp…",
        );
        let setup = async {
            let mut socket = tokio::time::timeout(Duration::from_secs(10), self.opts.connect())
                .await
                .map_err(|_| anyhow::anyhow!("Kết nối AI quá lâu (10 giây)"))??;
            send(&mut socket, self.opts.init_message()?).await?;
            ready(&mut socket, provider).await?;
            Ok::<_, anyhow::Error>(socket)
        };
        let cancelled = async {
            loop {
                if self.stop.load(Ordering::Acquire)
                    || self
                        .opts
                        .meeting_stop
                        .as_ref()
                        .is_some_and(|s| s.load(Ordering::Acquire))
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        let mut socket = tokio::select! {
            result = setup => result?,
            _ = cancelled => {
                emit_status(&self.app, provider, "idle", false, "Đã dừng phụ đề.");
                return Ok(());
            }
        };
        // Capture starts only after provider setup, so a slow handshake cannot
        // build an unbounded backlog or keep recording after an auth failure.
        let source = audio_input(&self.opts, self.stop.clone())?;
        let mut audio = source.lock().await;
        let mut soniox = SonioxState::new();
        let mut openai = OpenAiState::new();
        let mut gemini = GeminiState::new();
        let mut pending: std::collections::VecDeque<f32> =
            self.opts.retry_audio.lock().unwrap().take().into();
        let mut received = pending.len();
        self.opts
            .retry_audio
            .lock()
            .unwrap()
            .append(pending.make_contiguous())?;
        let mut ticker = tokio::time::interval(Duration::from_millis(50));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut last_sound = std::time::Instant::now();
        let mut warned = false;
        let mut sending = false;
        loop {
            tokio::select! {
                biased;
                _ = ticker.tick() => {
                    if self.stop.load(Ordering::Acquire) || self.opts.meeting_stop.as_ref().is_some_and(|s| s.load(Ordering::Acquire)) { break; }
                    // Clock-paced audio, including silence. Never compress long pauses.
                    // 50 ms micro-chunks, with bounded 2x catch-up after a delayed
                    // tick. Consume queued audio before padding missing silence.
                    let len=pending.len().clamp(800,1600);
                    let mut samples = Vec::with_capacity(len);
                    for _ in 0..len { samples.push(pending.pop_front().unwrap_or(0.)); }
                    if samples.iter().any(|s: &f32| s.abs() > 0.0005) {
                        last_sound = std::time::Instant::now();
                        if warned { warned = false; emit_status(&self.app, provider, "sending", true, "Đã nhận lại âm thanh; đang gửi để tạo phụ đề."); }
                    }
                    send(&mut socket, self.opts.audio_message(&samples)).await?;
                    // Keep five seconds of sent context plus all unsent audio for
                    // the explicit alternative. Captions may repeat at handoff;
                    // never silently discard the entire pending queue.
                    self.opts.retry_audio.lock().unwrap().acknowledge(received.saturating_sub(pending.len()+16000*5));
                    if !sending { sending = true; emit_status(&self.app, provider, "sending", true, "Đang gửi âm thanh hệ thống lên nhà cung cấp để tạo phụ đề."); }
                    if !warned && last_sound.elapsed() > Duration::from_secs(10) {
                        warned = true;
                        emit_status(&self.app, provider, "sending", true, "Chưa nhận được tiếng từ loa/tai nghe trong 10 giây. Kiểm tra thiết bị phát, âm lượng hoặc trạng thái tạm dừng cuộc họp.");
                    }
                }
                chunk = audio.recv() => match chunk {
                    Some(Ok(samples)) => {
                        ensure!(pending.len() + samples.len() <= 16000 * 30, "Âm thanh tồn đọng quá 30 giây; hãy bật lại phụ đề");
                        self.opts.retry_audio.lock().unwrap().append(&samples)?;
                        received+=samples.len();
                        pending.extend(samples);
                    }
                    Some(Err(e)) => return Err(e),
                    None if self.stop.load(Ordering::Acquire) || self.opts.meeting_stop.as_ref().is_some_and(|s|s.load(Ordering::Acquire)) => break,
                    None => bail!("Nguồn âm thanh đã dừng; hãy chọn lại thiết bị phát và bật lại phụ đề"),
                },
                message = socket.next() => {
                    let message = message.context("Nhà cung cấp đóng phiên; hãy bật lại phụ đề")?
                        .map_err(|_| anyhow::anyhow!("Kết nối AI bị gián đoạn; kiểm tra mạng"))?;
                    let text = match message {
                        Message::Text(t) => t,
                        Message::Binary(b) if provider == "gemini" => String::from_utf8(b).context("Gemini trả JSON không hợp lệ")?,
                        Message::Ping(_) => { socket.flush().await?; continue; },
                        Message::Close(_) => bail!("Nhà cung cấp đóng phiên; kiểm tra hạn mức và bật lại phụ đề"),
                        _ => continue,
                    };
                    let value: Value = serde_json::from_str(&text).context("Nhà cung cấp trả JSON không hợp lệ")?;
                    let events = match provider { "soniox" => if self.opts.translate { soniox.push(&value)? } else { soniox.push_source(&value)? }, "openai" => openai.push(&value)?, _ => gemini.push(&value)? };
                    for e in events { self.publish(e); }
                    if value["finished"] == true || value["type"] == "session.closed" { bail!("Nhà cung cấp đã kết thúc phiên; hãy bật lại phụ đề"); }
                }
            }
        }
        emit_status(
            &self.app,
            provider,
            "stopping",
            false,
            "Đang hoàn tất phần phụ đề cuối…",
        );
        // Bound shutdown, while allowing provider to flush final translations.
        let finish = async {
            let mut tail: Vec<f32> = pending.drain(..).collect();
            while let Ok(chunk) = audio.try_recv() {
                if let Ok(samples) = chunk {
                    tail.extend(samples);
                }
            }
            if !tail.is_empty() {
                send(&mut socket, self.opts.audio_message(&tail)).await?;
            }
            let ending = match provider {
                "soniox" => Message::Text(String::new()),
                "openai" => Message::Text(json!({"type":"session.close"}).to_string()),
                _ => Message::Text(json!({"realtimeInput":{"audioStreamEnd":true}}).to_string()),
            };
            send(&mut socket, ending).await?;
            while let Some(Ok(message)) = socket.next().await {
                let bytes = match message {
                    Message::Text(t) => t.into_bytes(),
                    Message::Binary(b) => b,
                    Message::Close(_) => break,
                    _ => continue,
                };
                let v: Value = serde_json::from_slice(&bytes)?;
                let events = match provider {
                    "soniox" => {
                        if self.opts.translate {
                            soniox.push(&v)?
                        } else {
                            soniox.push_source(&v)?
                        }
                    }
                    "openai" => openai.push(&v)?,
                    _ => gemini.push(&v)?,
                };
                for e in events {
                    self.publish(e);
                }
                if v["finished"] == true
                    || v["type"] == "session.closed"
                    || v["serverContent"]["turnComplete"] == true
                {
                    break;
                }
            }
            Ok::<_, anyhow::Error>(())
        };
        let _ = tokio::time::timeout(Duration::from_secs(3), finish).await;
        let _ = tokio::time::timeout(Duration::from_millis(300), socket.close(None)).await;
        emit_status(&self.app, provider, "idle", false, "Đã dừng phụ đề.");
        Ok(())
    }
}
impl CaptionOptions {
    async fn connect(
        &self,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    > {
        match self.provider.as_str() {
            "soniox" => {
                let mut request = SONIOX_ENDPOINT
                    .into_client_request()
                    .map_err(|_| anyhow::anyhow!("Địa chỉ Soniox không hợp lệ"))?;
                let mut auth = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                    .map_err(|_| anyhow::anyhow!("API key không hợp lệ"))?;
                auth.set_sensitive(true);
                request.headers_mut().insert("Authorization", auth);
                let (socket, _) = connect_async_with_config(request, None, true)
                    .await
                    .map_err(crate::soniox::websocket_error)?;
                Ok(socket)
            }
            "openai" => {
                ensure!(
                    !self.model.trim().is_empty(),
                    "Cần chọn model OpenAI (ví dụ gpt-realtime-translate)"
                );
                let url = if self.base_url.trim().is_empty() {
                    OPENAI_TRANSLATE_ENDPOINT.to_string()
                } else {
                    format!(
                        "{}/realtime/translations",
                        self.base_url.trim().trim_end_matches('/')
                    )
                };
                let mut url = reqwest::Url::parse(&url).context("Base URL không hợp lệ")?;
                let scheme = if url.scheme() == "http" { "ws" } else { "wss" };
                url.set_scheme(scheme)
                    .map_err(|_| anyhow::anyhow!("Base URL không hợp lệ"))?;
                url.query_pairs_mut()
                    .append_pair("model", self.model.trim());
                let mut request = url
                    .as_str()
                    .into_client_request()
                    .map_err(|_| anyhow::anyhow!("Địa chỉ OpenAI không hợp lệ"))?;
                let mut auth = HeaderValue::from_str(&format!("Bearer {}", self.api_key))
                    .map_err(|_| anyhow::anyhow!("API key không hợp lệ"))?;
                auth.set_sensitive(true);
                request.headers_mut().insert("Authorization", auth);
                let (socket, _) = connect_async_with_config(request, None, true)
                    .await
                    .map_err(|e| connection_error("OpenAI", e))?;
                Ok(socket)
            }
            "gemini" => {
                ensure!(
                    !self.model.trim().is_empty(),
                    "Cần chọn model Gemini Live Translation"
                );
                ensure!(
                    self.base_url.trim().is_empty(),
                    "Gemini Live hiện chỉ hỗ trợ endpoint Google mặc định; bỏ Base URL"
                );
                let mut request = GEMINI_ENDPOINT.into_client_request()?;
                let mut key = HeaderValue::from_str(self.api_key.trim())
                    .map_err(|_| anyhow::anyhow!("Key Gemini không hợp lệ"))?;
                key.set_sensitive(true);
                request.headers_mut().insert("x-goog-api-key", key);
                let (socket, _) = connect_async_with_config(request, None, true)
                    .await
                    .map_err(|e| connection_error("Gemini", e))?;
                Ok(socket)
            }
            other => bail!("Nhà cung cấp phụ đề không hợp lệ: {other}"),
        }
    }
    fn init_message(&self) -> Result<Message> {
        match self.provider.as_str() {
            "soniox" => Ok(Message::Text(crate::meeting_soniox::configuration(
                self.source_language.as_deref().unwrap_or("multi"),
                self.translate.then_some(self.target_language.as_str()),
                self.speech_quality,
            ).to_string())),
            "openai" => Ok(Message::Text(
                json!({
                    "type": "session.update",
                    "session": {
                        "audio": {
                            "input": { "transcription": { "model": "gpt-realtime-whisper" }, "noise_reduction": null },
                            "output": { "language": self.target_language }
                        }
                    }
                })
                .to_string(),
            )),
            "gemini" => Ok(Message::Text(
                json!({
                    "setup": {
                        "model": format!("models/{}", self.model.trim()),
                        "generationConfig": {
                            "responseModalities": ["AUDIO"],
                            "inputAudioTranscription": {},
                            "outputAudioTranscription": {},
                            "translationConfig": {
                                "targetLanguageCode": if self.target_language == "zh" { "zh-Hans" } else { &self.target_language },
                                "echoTargetLanguage": true
                            }
                        }
                    }
                })
                .to_string(),
            )),
            other => bail!("Nhà cung cấp phụ đề không hợp lệ: {other}"),
        }
    }

    fn audio_message(&self, samples: &[f32]) -> Message {
        let pcm16: Vec<i16> = samples
            .iter()
            .map(|x| (x.clamp(-1., 1.) * 32767.) as i16)
            .collect();
        match self.provider.as_str() {
            "soniox" => {
                let bytes: Vec<u8> = pcm16.iter().flat_map(|s| s.to_le_bytes()).collect();
                Message::Binary(bytes)
            }
            "openai" => {
                let bytes: Vec<u8> = resample_16k_to_24k(&pcm16)
                    .iter()
                    .flat_map(|s| s.to_le_bytes())
                    .collect();
                Message::Text(
                    json!({ "type": "session.input_audio_buffer.append", "audio": base64_encode(&bytes) })
                        .to_string(),
                )
            }
            "gemini" => {
                let bytes: Vec<u8> = pcm16.iter().flat_map(|s| s.to_le_bytes()).collect();
                Message::Text(
                    json!({
                        "realtimeInput": {
                            "audio": { "mimeType": "audio/pcm;rate=16000", "data": base64_encode(&bytes) }
                        }
                    })
                    .to_string(),
                )
            }
            _ => Message::Binary(Vec::new()),
        }
    }
}
/// Entry point for `start_caption`. Runs the session to completion on a
/// background thread and reports errors through the status event.
pub fn run(
    app: tauri::AppHandle,
    opts: CaptionOptions,
    stop: Arc<AtomicBool>,
    done: Arc<AtomicBool>,
) {
    std::thread::spawn(move || {
        let provider = opts.provider.clone();
        let result = run_inner(app.clone(), opts, stop.clone());
        stop.store(true, Ordering::Release);
        if let Err(e) = result {
            emit_status(&app, &provider, "error", false, &format!("{e:#}"));
        }
        done.store(true, Ordering::Release);
    });
}

fn run_inner(app: tauri::AppHandle, mut opts: CaptionOptions, stop: Arc<AtomicBool>) -> Result<()> {
    validate_options(&opts)?;
    tauri::async_runtime::block_on(async {
        // The same producer/queue survives a provider failure and handshake.
        if opts.mode == "standalone" && opts.alternative.is_some() {
            opts.audio_source = Some(audio_input(&opts, stop.clone())?);
        }
        let mut alternative = opts.alternative.take();
        loop {
            if stop.load(Ordering::Acquire) {
                return Ok(());
            }
            let attempt_stop = Arc::new(AtomicBool::new(false));
            let session = Session {
                app: app.clone(),
                opts: opts.clone(),
                stop: attempt_stop.clone(),
            };
            let running = session.run();
            tokio::pin!(running);
            let stopping = async {
                while !stop.load(Ordering::Acquire) {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            };
            let result = tokio::select! {
                result=&mut running=>result,
                _=stopping=>{attempt_stop.store(true,Ordering::Release);tokio::time::timeout(Duration::from_secs(13),&mut running).await.map_err(|_|anyhow::anyhow!("Phiên phụ đề chưa dừng sau 13 giây"))?}
            };
            attempt_stop.store(true, Ordering::Release);
            match result {
                Ok(()) => return Ok(()),
                Err(error)
                    if !stop.load(Ordering::Acquire)
                        && !opts
                            .meeting_stop
                            .as_ref()
                            .is_some_and(|s| s.load(Ordering::Acquire))
                        && crate::fallback::eligible(&error)
                        && alternative.is_some() =>
                {
                    let mut next = *alternative.take().unwrap();
                    next.audio_source = opts.audio_source.clone();
                    let _=app.emit("ai-fallback",serde_json::json!({"task":"caption","from":opts.display_model,"to":next.display_model}));
                    emit_status(&app,&next.provider,"fallback",false,&format!("{} lỗi: {error}. Đang chuyển sang {} đã chọn; nguồn thu vẫn chạy, phụ đề có thể trễ khi bắt kịp.",opts.display_model,next.display_model));
                    opts = next;
                }
                Err(error) => return Err(error),
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_chunks_keep_subwords_and_do_not_commit_interim() {
        let mut parser = SonioxState::new();
        let tokens: Vec<_> = ["one", " two", " three", " four", " five", " six", "teen", ",", " seven"]
            .iter().map(|text| json!({"text":text,"is_final":true})).collect();
        let events = parser.push_source(&json!({"tokens":tokens})).unwrap();
        assert_eq!(events[0].text, "one two three four five sixteen,");
        assert!(events[0].final_);
        assert_eq!(events[1].text.trim(), "seven");
        assert!(!events[1].final_);
        let interim = parser.push_source(&json!({"tokens":[{"text":" wrong","is_final":false}]})).unwrap();
        assert!(interim.iter().all(|e| !e.final_));
        let tail = parser.push_source(&json!({"finished":true})).unwrap();
        assert_eq!(tail[0].text, "seven");
        assert!(tail[0].final_);
        assert!(parser.push_source(&json!({"finished":true})).unwrap().is_empty());
    }
    #[test]
    fn processing_levels_change_parameters_without_changing_model_or_provider() {
        let mut opts=options("soniox","stt-rt-v5");
        for (level,delay) in [(crate::speech_quality::SpeechQuality::Fast,500),(crate::speech_quality::SpeechQuality::Balanced,750),(crate::speech_quality::SpeechQuality::Accurate,1200)] {
            opts.speech_quality=level;
            let value:Value=serde_json::from_str(opts.init_message().unwrap().to_text().unwrap()).unwrap();
            assert_eq!(value["model"],"stt-rt-v5");assert_eq!(value["max_endpoint_delay_ms"],delay);
            assert_eq!(opts.provider,"soniox");assert_eq!(opts.api_key,"test-key");
        }
        let mut fixed=options("gemini","gemini-3.5-live-translate-preview");fixed.speech_quality=crate::speech_quality::SpeechQuality::Fast;
        assert!(validate_options(&fixed).is_err());fixed.speech_quality=Default::default();assert!(validate_options(&fixed).is_ok());
    }

    #[test]
    fn snapshot_skips_duplicate_partials_but_preserves_final_cues_and_last_pair() {
        let mut snapshot = latest().lock().unwrap().clone();
        snapshot.translating = true;
        let partial = CaptionText {
            text: "Hello".into(),
            original: "Xin chào".into(),
            final_: false,
        };
        assert!(snapshot.accept(&partial));
        assert!(!snapshot.accept(&partial));
        let final_cue = CaptionText {
            final_: true,
            ..partial.clone()
        };
        assert!(snapshot.accept(&final_cue));
        assert!(snapshot.accept(&final_cue)); // Repeated spoken utterances are valid.
        let next = CaptionText {
            text: "Next".into(),
            ..partial
        };
        assert!(snapshot.accept(&next));
        assert_eq!(snapshot.paired, Some(final_cue));
    }

    fn options(provider: &str, model: &str) -> CaptionOptions {
        serde_json::from_value(json!({"provider":provider,"apiKey":"test-key","baseUrl":"", "model":model,
            "targetLanguage":"vi","device":"speaker","mode":"standalone","sessionDir":null,"profileId":"p1"})).unwrap()
    }
    #[test]
    fn source_only_config_has_no_translation_and_rejects_translation_only_engines() {
        let mut o = options("soniox", "stt-rt-v5");
        assert!(o.translate);
        o.translate = false;
        o.source_language = Some("en".into());
        o.target_language = String::new();
        validate_options(&o).unwrap();
        let config: Value =
            serde_json::from_str(o.init_message().unwrap().to_text().unwrap()).unwrap();
        assert!(config.get("translation").is_none());
        assert_eq!(config["language_hints"], json!(["en"]));
        o.source_language = Some("ja".into());
        let japanese: Value =
            serde_json::from_str(o.init_message().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(japanese["language_hints"], json!(["ja"]));
        o.provider = "openai".into();
        o.model = "gpt-realtime-translate".into();
        assert!(validate_options(&o).is_err());
    }
    #[test]
    fn groq_whisper_is_source_only_and_cannot_be_mistaken_for_a_translation_model() {
        let mut o = options("groq", "whisper-large-v3-turbo");
        o.translate = false;
        validate_options(&o).unwrap();
        o.translate = true;
        assert!(validate_options(&o).is_err());
        o.translate = false;
        o.model = "openai/gpt-oss-20b".into();
        assert!(validate_options(&o).is_err());
        o.model = "whisper-large-v3".into();
        o.mode = "meeting".into();
        o.session_dir = Some("fixture".into());
        assert!(validate_options(&o).is_err());
    }
    #[test]
    fn source_only_displays_interim_without_translation_and_keeps_final_once() {
        let mut p = SonioxState::new();
        let first = p
            .push_source(&json!({"tokens":[{"text":"Wrong","is_final":false}]}))
            .unwrap();
        assert_eq!(first[0].text, "Wrong");
        assert!(!first[0].final_);
        assert!(first[0].original.is_empty());
        let next=p.push_source(&json!({"tokens":[{"text":"Right","is_final":true},{"text":" now","is_final":false}]})).unwrap();
        assert_eq!(next[0].text, "Right now");
        let final_=p.push_source(&json!({"tokens":[{"text":" here.","is_final":true},{"text":"<end>","is_final":true}]})).unwrap();
        assert_eq!(final_.len(), 1);
        assert_eq!(final_[0].text, "Right here.");
        assert!(final_[0].final_);
        assert!(p.push_source(&json!({"finished":true})).unwrap().is_empty());
    }

    #[test]
    fn provider_configs_and_audio_match_dedicated_translation_protocols() {
        let s = options("soniox", "stt-rt-v5");
        validate_options(&s).unwrap();
        let init: Value =
            serde_json::from_str(s.init_message().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(init["audio_format"], "pcm_s16le");
        assert_eq!(init["enable_endpoint_detection"], true);
        assert_eq!(
            init["translation"],
            json!({"type":"one_way","target_language":"vi"})
        );
        assert!(init.get("api_key").is_none());
        assert!(matches!(s.audio_message(&[0.;1600]), Message::Binary(b) if b.len() == 3200));
        let o = options("openai", "gpt-realtime-translate");
        let init: Value =
            serde_json::from_str(o.init_message().unwrap().to_text().unwrap()).unwrap();
        assert!(init["session"].get("model").is_none());
        assert_eq!(
            init["session"]["audio"]["input"]["transcription"]["model"],
            "gpt-realtime-whisper"
        );
        let audio: Value =
            serde_json::from_str(o.audio_message(&[0.; 1600]).to_text().unwrap()).unwrap();
        assert_eq!(audio["type"], "session.input_audio_buffer.append");
        assert_eq!(audio["audio"].as_str().unwrap().len(), 6400); // 2400 samples, PCM16 -> base64
        let g = options("gemini", "gemini-3.5-live-translate-preview");
        let init: Value =
            serde_json::from_str(g.init_message().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(
            init["setup"]["generationConfig"]["responseModalities"],
            json!(["AUDIO"])
        );
        assert_eq!(
            init["setup"]["generationConfig"]["translationConfig"]["targetLanguageCode"],
            "vi"
        );
        assert!(init["setup"].get("systemInstruction").is_none());
        let audio: Value =
            serde_json::from_str(g.audio_message(&[0.; 1600]).to_text().unwrap()).unwrap();
        assert_eq!(
            audio["realtimeInput"]["audio"]["mimeType"],
            "audio/pcm;rate=16000"
        );
        assert!(validate_options(&options("gemini", "gemini-2.5-flash")).is_err());
        assert!(validate_options(&options("openai", "gpt-4o-mini")).is_err());
    }

    #[test]
    fn soniox_replaces_interim_keeps_delayed_translation_and_multiple_boundaries() {
        let mut s = SonioxState::new();
        s.push(&json!({"tokens":[{"text":"draft","is_final":false,"translation_status":"translation"}]})).unwrap();
        let events = s.push(&json!({"tokens":[{"text":"new draft","is_final":false,"translation_status":"translation"}]})).unwrap();
        assert_eq!(events[0].text, "new draft");
        s.push(&json!({"tokens":[{"text":"Không phê duyệt 12,5 triệu.","is_final":true,"translation_status":"original"},{"text":"<end>","is_final":true,"translation_status":"original"}]})).unwrap();
        let events = s.push(&json!({"tokens":[
            {"text":"Did not approve 12.5 million.","is_final":true,"translation_status":"translation"},
            {"text":"<end>","is_final":true,"translation_status":"translation"},
            {"text":"Câu hai.","is_final":true,"translation_status":"original"},
            {"text":"<end>","is_final":true,"translation_status":"original"},
            {"text":"Second sentence.","is_final":true,"translation_status":"translation"},
            {"text":"<fin>","is_final":true,"translation_status":"translation"}
        ]})).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events
            .iter()
            .all(|e| e.final_ && !e.text.contains('<') && !e.original.contains('<')));
        assert_eq!(events[0].original, "Không phê duyệt 12,5 triệu.");
        assert_eq!(events[1].text, "Second sentence.");
        assert!(s.push(&json!({"finished":true})).unwrap().is_empty());
    }

    #[test]
    fn dedicated_openai_deltas_and_gemini_binary_json_do_not_need_chat_turns() {
        let mut o = OpenAiState::new();
        o.push(&json!({"type":"session.input_transcript.delta","delta":"No"}))
            .unwrap();
        o.push(&json!({"type":"session.output_transcript.delta","delta":"Không "}))
            .unwrap();
        let final_ = o
            .push(&json!({"type":"session.output_transcript.delta","delta":"đồng ý."}))
            .unwrap();
        assert_eq!(final_[0].text, "Không đồng ý.");
        assert!(final_[0].final_);
        assert_eq!(final_[0].original, "No");
        let mut g = GeminiState::new();
        let v: Value = serde_json::from_slice(br#"{"serverContent":{"outputTranscription":{"text":"Hello"},"inputTranscription":{"text":"Bonjour"}}}"#).unwrap();
        let events = g.push(&v).unwrap();
        assert_eq!(events[0].text, "Hello");
        assert_eq!(events[0].original, "Bonjour");
    }

    #[test]
    fn resampler_keeps_duration_and_amplitude() {
        // 16 kHz 1 second of a constant +0.244 amplitude -> 24 kHz 1 second.
        let input: Vec<i16> = vec![8000i16; 16000];
        let out = resample_16k_to_24k(&input);
        assert_eq!(out.len(), 24000);
        assert!(
            out.iter().all(|x| *x >= 7900 && *x <= 8100),
            "amplitude distorted"
        );
    }

    #[test]
    fn base64_encodes_standard() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b""), "");
    }

    #[test]
    fn soniox_translation_tokens_combine() {
        let mut s = SonioxState::new();
        // Original final + translation non-final arrive together: live partial.
        let events = s
            .push(&json!({"tokens": [
                {"text": "Xin ", "is_final": true, "translation_status": "original"},
                {"text": "chào", "is_final": true, "translation_status": "original"},
                {"text": "Hello", "is_final": false, "translation_status": "translation"}
            ]}))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(!events[0].final_);
        assert_eq!(events[0].text, "Hello");
        assert_eq!(events[0].original, "Xin chào");
        // The server confirms the translation, then the <end> boundary flushes
        // the final phrase.
        let _ = s
            .push(&json!({"tokens": [
                {"text": "Hello", "is_final": true, "translation_status": "translation"}
            ]}))
            .unwrap();
        let events = s
            .push(&json!({"tokens": [
                {"text": "!", "is_final": true, "translation_status": "translation"},
                {"text": "<end>", "is_final": true, "translation_status": "translation"}
            ]}))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].final_);
        assert_eq!(events[0].text, "Hello!");
    }

    #[test]
    fn soniox_errors_are_classified_without_secrets() {
        let mut s = SonioxState::new();
        let err = s
            .push(&json!({"error_code": 401, "error_message": "secret-key"}))
            .unwrap_err()
            .to_string();
        assert!(!err.contains("secret-key"));
        assert!(err.contains("key"));
    }

    #[test]
    fn openai_events_build_translated_text() {
        let mut o = OpenAiState::new();
        let _ = o
            .push(&json!({"type": "response.output_transcript.delta", "delta": "Xin "}))
            .unwrap();
        let _ = o
            .push(&json!({"type": "response.output_transcript.delta", "delta": "chào."}))
            .unwrap();
        let events = o
            .push(&json!({"type": "response.done", "response": {
                "output": [{"output": [{"transcript": "Xin chào."}]}]
            }}))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].final_);
        assert_eq!(events[0].text, "Xin chào.");
    }

    #[test]
    fn openai_key_errors_are_classified() {
        let mut o = OpenAiState::new();
        let err = o
            .push(&json!({"type": "error", "error": {"type": "invalid_request_error", "code": "invalid_api_key", "message": "Incorrect API key"}}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("key"));
    }

    #[test]
    fn gemini_turns_emit_partial_and_final() {
        let mut g = GeminiState::new();
        let partial = g
            .push(&json!({"serverContent": {"outputTranscription": {"text": "Chào "}, "turnComplete": false}}))
            .unwrap();
        assert_eq!(partial.len(), 1);
        assert!(!partial[0].final_);
        assert_eq!(partial[0].text, "Chào");
        let events = g
            .push(&json!({"serverContent": {"outputTranscription": {"text": "mọi người."}, "turnComplete": true}}))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert!(events[0].final_);
        assert_eq!(events[0].text, "Chào mọi người.");
    }

    #[test]
    fn gemini_model_error_is_classified() {
        let mut g = GeminiState::new();
        let err = g
            .push(&json!({"error": {"code": 404, "message": "models/xyz not found (not found)."}}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("model"));
    }
}
