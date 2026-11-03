//! Source-only local captions using bundled Moonshine or optional Nemotron.
use crate::caption::{CaptionOptions, CaptionText};
use anyhow::{ensure, Result};
use std::sync::{atomic::AtomicBool, Arc};
pub fn language(model: &str, source: Option<&str>) -> Result<&'static str> {
    let source = source.unwrap_or("auto");
    if let Some(code) = crate::models::moonshine_language(model) {
        ensure!(["", "auto", code].contains(&source), "Moonshine đã chọn không khớp ngôn ngữ gốc; hãy chọn đúng model Việt/Anh");
        return Ok(code);
    }
    match model {
        crate::models::NEMO_ID => match source {
            "" | "auto" | "multi" => Ok("multi"),
            "vi" => Ok("vi"), "en" => Ok("en"), "ko" => Ok("ko"), "zh" => Ok("zh"),
            _ => anyhow::bail!("Nemotron local hỗ trợ Việt, Anh, Hàn và Trung"),
        },
        _ => anyhow::bail!("Model local không còn hỗ trợ; hãy chọn Moonshine hoặc Nemotron"),
    }
}
pub async fn run<F: FnMut(CaptionText)>(app: &tauri::AppHandle, opts: &CaptionOptions, stop: Arc<AtomicBool>, publish: F) -> Result<()> {
    crate::local_stream_caption::run(app, opts, stop, publish).await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn model_languages_are_explicit_and_old_model_is_not_selectable() {
        assert_eq!(language("moonshine-vi", Some("auto")).unwrap(), "vi");
        assert_eq!(language("moonshine-en", Some("en")).unwrap(), "en");
        assert!(language("moonshine-vi", Some("en")).is_err());
        assert!(language("moonshine-en", Some("multi")).is_err());
        assert!(language("phowhisper-small", None).is_err());
        assert_eq!(language(crate::models::NEMO_ID, Some("en")).unwrap(), "en");
        assert!(language(crate::models::NEMO_ID, Some("ja")).is_err());
    }
}
