use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub default: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Segment {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub text: String,
    #[serde(default)]
    pub edited: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SmoothedTranscript {
    pub text: String,
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub prompt: String,
    pub created: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CorrectedTranscript {
    pub text: String,
    pub segments: Vec<Segment>,
    pub provider: String,
    pub profile_id: String,
    pub model: String,
    pub created: String,
    pub audio_sha256: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SummarizedTranscript {
    pub text: String,
    pub provider: String,
    pub model: String,
    pub source: String,
    #[serde(default)]
    pub prompt: String,
    pub created: String,
    #[serde(default)]
    pub source_hash: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TranslationSegment {
    pub id: String,
    pub source_id: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    pub start: f64,
    pub end: f64,
    pub original: String,
    pub text: String,
    pub language: String,
    pub provider: String,
}
#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SpeechInfo {
    pub model: String,
    pub profile_id: Option<String>,
    pub translation_language: Option<String>,
    pub connections: u64,
    pub uploaded_samples: u64,
    pub local_fallback: bool,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Session {
    #[serde(default)]
    pub speech_quality: crate::speech_quality::SpeechQuality,
    pub id: String,
    pub title: String,
    pub created: String,
    pub duration: f64,
    pub status: String,
    pub language: String,
    pub model: String,
    #[serde(default = "local_provider")]
    pub provider: String,
    pub processed: f64,
    pub segments: Vec<Segment>,
    /// Streaming words remain available until Whisper has refined their audio range.
    #[serde(default)]
    pub live: Vec<Segment>,
    pub notes: String,
    pub error: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// AI-smoothed copy. The original `segments` are never replaced.
    #[serde(default)]
    pub smoothed: Option<SmoothedTranscript>,
    #[serde(default)]
    pub corrected: Option<CorrectedTranscript>,
    /// AI summary, stored separately from the transcript.
    #[serde(default)]
    pub summary: Option<SummarizedTranscript>,
    #[serde(default)]
    pub translations: Vec<TranslationSegment>,
    #[serde(default)]
    pub speech: Option<SpeechInfo>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub installed: bool,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    #[serde(default)]
    pub corrections: Vec<crate::correction::Progress>,
    #[serde(default)]
    pub translation_preview: Option<crate::meeting_soniox::TranslationPreview>,
    pub sessions: Vec<Session>,
    pub devices: Vec<Device>,
    pub models: Vec<ModelInfo>,
    pub data_dir: String,
    pub active_id: Option<String>,
    pub soniox_configured: bool,
    #[serde(default)]
    pub llm_configured: bool,
}
#[derive(Serialize)]
pub struct AudioTestResult {
    pub path: String,
    pub peaks: std::collections::BTreeMap<String, f32>,
    pub warnings: Vec<String>,
}
#[derive(Deserialize, Clone)]
pub struct StartOptions {
    #[serde(default, alias = "speechQuality")]
    pub speech_quality: crate::speech_quality::SpeechQuality,
    pub title: String,
    pub system: Option<String>,
    pub microphone: Option<String>,
    pub language: String,
    pub model: String,
    pub transcribe: bool,
    #[serde(default = "local_provider")]
    pub provider: String,
    /// Named AI profile used for the transcription provider (key stays local).
    #[serde(default)]
    #[serde(alias = "profileId")]
    pub profile_id: Option<String>,
    #[serde(default, alias = "translationLanguage")]
    pub translation_language: Option<String>,
    #[serde(default, alias = "captionsEnabled")]
    pub captions_enabled: bool,
    #[serde(default, alias = "transcriptionFallback")]
    pub transcription_fallback: Option<crate::config::ModelChoice>,
    #[serde(default, alias = "localFallback")]
    pub local_fallback: bool,
}
fn local_provider() -> String {
    "local".into()
}
#[derive(Serialize, Deserialize)]
pub struct WorkerJob {
    pub path: String,
    pub offset: f64,
    pub language: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct WorkerResult {
    pub segments: Vec<Segment>,
    pub error: Option<String>,
    pub elapsed_ms: u64,
    #[serde(default)]
    pub vad_windows: usize,
    #[serde(default)]
    pub inference_windows: usize,
}
