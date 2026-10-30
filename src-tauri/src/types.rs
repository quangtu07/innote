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
pub struct Session {
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
    pub title: String,
    pub system: Option<String>,
    pub microphone: Option<String>,
    pub language: String,
    pub model: String,
    pub transcribe: bool,
    #[serde(default = "local_provider")]
    pub provider: String,
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
