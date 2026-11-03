use crate::types::ModelInfo;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const NEMO_ID: &str = "nemotron-3.5-q8";
pub const MOONSHINE_VI: &str = "moonshine-vi";
pub const MOONSHINE_EN: &str = "moonshine-en";
pub const CATALOG: [(&str, &str, u64, &str); 3] = [
    (
        "base-q5_1",
        "Tiết kiệm tài nguyên",
        59707625,
        "422f1ae452ade6f30a004d7e5c6a43195e4433bc370bf23fac9cc591f01a8898",
    ),
    (
        "small-q5_1",
        "Cân bằng",
        190085487,
        "ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb",
    ),
    (
        NEMO_ID,
        "Nemotron · Chép lời local đa ngôn ngữ",
        741548352,
        "a5c435f294eea8f88ce68dd27b8c3bfea7f777cb2fbba04fcd30eaa555f429ae",
    ),
];
pub fn prepare_vad(directory: &Path) -> Result<PathBuf> {
    std::fs::create_dir_all(directory)?;
    let path = directory.join("ggml-silero-v5.1.2.bin");
    let bytes = include_bytes!("../resources/ggml-silero-v5.1.2.bin");
    if std::fs::read(&path)
        .map(|b| b.as_slice() == bytes)
        .unwrap_or(false)
    {
        return Ok(path);
    }
    std::fs::write(&path, bytes)?;
    Ok(path)
}
pub fn path(root: &Path, id: &str) -> Result<PathBuf> {
    if let Some(language) = moonshine_language(id) {
        return Ok(std::env::current_exe()?.parent().context("Executable directory")?
            .join("live-models").join(language));
    }
    anyhow::ensure!(CATALOG.iter().any(|x| x.0 == id), "Model không hợp lệ");
    let name = if id == NEMO_ID {
        "nemotron-3.5-q8.gguf".to_string()
    } else {
        format!("ggml-{id}.bin")
    };
    let local = root.join("models").join(&name);
    if local.exists() {
        return Ok(local);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundled = parent.join("models").join(&name);
            if bundled.exists() {
                return Ok(bundled);
            }
        }
    }
    Ok(local)
}
pub fn list(root: &Path) -> Vec<ModelInfo> {
    let mut models: Vec<_> = CATALOG
        .iter()
        .map(|(id, name, bytes, _)| ModelInfo {
            id: id.to_string(),
            name: name.to_string(),
            bytes: *bytes,
            installed: path(root, id)
                .ok()
                .and_then(|p| std::fs::metadata(p).ok())
                .map(|m| m.len() == *bytes)
                .unwrap_or(false),
        })
        .collect();
    for (id, name) in [(MOONSHINE_VI, "Moonshine · Tiếng Việt · Có sẵn"), (MOONSHINE_EN, "Moonshine · English · Có sẵn")] {
        let directory = path(root, id).ok();
        let ready = directory.as_ref().is_some_and(|p| moonshine_ready(p));
        let bytes = directory.as_ref().and_then(|p|std::fs::read_dir(p).ok())
            .map(|entries|entries.flatten().filter_map(|e|e.metadata().ok()).map(|m|m.len()).sum()).unwrap_or(0);
        models.push(ModelInfo { id: id.into(), name: name.into(), bytes, installed: ready });
    }
    models
}
pub fn moonshine_language(id: &str) -> Option<&'static str> {
    match id { MOONSHINE_VI => Some("vi"), MOONSHINE_EN => Some("en"), _ => None }
}
fn moonshine_ready(directory: &Path) -> bool {
    let files = ["streaming_config.json", "tokenizer.bin", "adapter.ort", "cross_kv.ort",
        "decoder_kv.ort", "encoder.ort", "frontend.model.ort", "frontend.weights.ort"];
    let valid = |p: PathBuf| std::fs::metadata(p).is_ok_and(|m|m.is_file() && m.len()>0);
    files.iter().all(|f|valid(directory.join(f))) && directory.parent().and_then(Path::parent)
        .is_some_and(|root|valid(root.join("moonshine.dll")) && valid(root.join("onnxruntime.dll")))
}
pub fn download(root: &Path, id: &str, mut progress: impl FnMut(u64, u64)) -> Result<()> {
    let (_, _, size, hash) = CATALOG
        .iter()
        .find(|x| x.0 == id)
        .context("Model không hợp lệ")?;
    let dest = path(root, id)?;
    std::fs::create_dir_all(dest.parent().unwrap())?;
    let temp = dest.with_extension("partial");
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(1800))
        .build()?;
    let url = if id == NEMO_ID {
        "https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b/resolve/1c8deaecc64b91f034d73e08dd8b64625eb3395d/nemotron-3.5-asr-streaming-0.6b.q8_0.gguf".into()
    } else {
        format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{id}.bin")
    };
    let mut response = client.get(url).send()?.error_for_status()?;
    let mut file = std::fs::File::create(&temp)?;
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut buf = [0u8; 65536];
    let mut last = std::time::Instant::now();
    loop {
        let n = response.read(&mut buf)?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])?;
        digest.update(&buf[..n]);
        count += n as u64;
        if last.elapsed().as_millis() > 150 {
            progress(count, *size);
            last = std::time::Instant::now();
        }
    }
    file.sync_all()?;
    drop(file);
    anyhow::ensure!(
        count == *size && hex::encode(digest.finalize()) == *hash,
        "File model không khớp checksum. Hãy tải lại."
    );
    std::fs::rename(temp, dest)?;
    progress(count, *size);
    Ok(())
}

/// Keep the request's recognizer and language consistent before capture begins.
pub fn validate_mode(language: &str, model: &str, provider: &str) -> Result<()> {
    anyhow::ensure!(
        ["vi", "en", "ko", "zh", "multi", "auto"].contains(&language),
        "Ngôn ngữ không hợp lệ"
    );
    if provider == "soniox" {
        return Ok(());
    }
    anyhow::ensure!(model == NEMO_ID || moonshine_language(model).is_some(), "Model local không còn hỗ trợ; hãy chọn Moonshine hoặc Nemotron");
    if let Some(expected) = moonshine_language(model) {
        anyhow::ensure!(language == expected, "Model Moonshine không khớp ngôn ngữ đã chọn");
    }
    if ["multi", "ko", "zh"].contains(&language) {
        anyhow::ensure!(model == NEMO_ID, "Chế độ này cần model đa ngôn ngữ");
    }
    if model == NEMO_ID {
        anyhow::ensure!(
            provider == "local",
            "Chế độ đa ngôn ngữ hiện dùng local miễn phí"
        );
    }
    Ok(())
}
#[cfg(test)]
mod mode_tests {
    use super::*;
    #[test]
    fn mode_selects_an_actual_compatible_recognizer() {
        for lang in ["multi", "ko", "zh"] {
            assert!(validate_mode(lang, NEMO_ID, "local").is_ok());
            assert!(validate_mode(lang, MOONSHINE_VI, "local").is_err());
            assert!(validate_mode(lang, NEMO_ID, "soniox").is_ok());
        }
        for lang in ["vi", "en"] {
            let id = if lang == "vi" { MOONSHINE_VI } else { MOONSHINE_EN };
            assert!(validate_mode(lang, id, "soniox").is_ok());
            assert!(validate_mode(lang, id, "local").is_ok());
            assert!(validate_mode(lang, NEMO_ID, "local").is_ok());
        }
        assert!(validate_mode("invalid", "base-q5_1", "local").is_err());
        assert!(validate_mode("en", MOONSHINE_VI, "local").is_err());
        assert!(validate_mode("vi", "phowhisper-small", "local").is_err());
    }
    #[test]
    fn partial_bundles_are_not_reported_ready() {
        let root=std::env::temp_dir().join(format!("innote-bundle-{}",uuid::Uuid::new_v4()));
        let directory=root.join("live-models/vi");std::fs::create_dir_all(&directory).unwrap();
        assert!(!moonshine_ready(&directory));
        for file in ["streaming_config.json","tokenizer.bin","adapter.ort","cross_kv.ort","decoder_kv.ort","encoder.ort","frontend.model.ort","frontend.weights.ort"] { std::fs::write(directory.join(file),b"fixture").unwrap(); }
        assert!(!moonshine_ready(&directory));
        std::fs::write(root.join("moonshine.dll"),b"fixture").unwrap();std::fs::write(root.join("onnxruntime.dll"),b"fixture").unwrap();
        assert!(moonshine_ready(&directory));std::fs::remove_dir_all(root).unwrap();
    }
}
