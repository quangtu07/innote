use crate::types::ModelInfo;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
pub const NEMO_ID: &str = "nemotron-3.5-q8";
pub const PHOWHISPER_ID: &str = "phowhisper-small";
pub const CATALOG: [(&str, &str, u64, &str); 4] = [
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
        PHOWHISPER_ID,
        "PhoWhisper · Tiếng Việt (VinAI)",
        487601984,
        "db8de368e822fce5dcdce7c0c4c80c4b554c27655516dea5ec8c093e22f58785",
    ),
    (
        NEMO_ID,
        "Đa ngôn ngữ · Việt/Anh/Hàn/Trung",
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
    anyhow::ensure!(CATALOG.iter().any(|x| x.0 == id), "Model không hợp lệ");
    Ok(root.join("models").join(if id == NEMO_ID {
        "nemotron-3.5-q8.gguf".into()
    } else if id == PHOWHISPER_ID {
        "ggml-PhoWhisper-small.bin".into()
    } else {
        format!("ggml-{id}.bin")
    }))
}
pub fn list(root: &Path) -> Vec<ModelInfo> {
    CATALOG
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
        .collect()
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
    } else if id == PHOWHISPER_ID {
        "https://huggingface.co/dongxiat/ggml-PhoWhisper-small/resolve/main/ggml-PhoWhisper-small.bin".into()
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
    if ["multi", "ko", "zh"].contains(&language) {
        anyhow::ensure!(model == NEMO_ID, "Chế độ này cần model đa ngôn ngữ");
    }
    if model == NEMO_ID {
        anyhow::ensure!(
            ["multi", "ko", "zh"].contains(&language),
            "Chọn chế độ đa ngôn ngữ cho model này"
        );
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
            assert!(validate_mode(lang, PHOWHISPER_ID, "local").is_err());
            assert!(validate_mode(lang, NEMO_ID, "soniox").is_err());
        }
        for lang in ["vi", "en"] {
            assert!(validate_mode(lang, PHOWHISPER_ID, "soniox").is_ok());
            assert!(validate_mode(lang, PHOWHISPER_ID, "local").is_ok());
            assert!(validate_mode(lang, NEMO_ID, "local").is_err());
        }
        assert!(validate_mode("invalid", "base-q5_1", "local").is_err());
    }
}
