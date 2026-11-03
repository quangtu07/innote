//! Obfuscated-at-rest storage for API keys so they survive app restarts and
//! in-app updates. Keys are XOR-scrambled with a random nonce before being
//! written to the local data directory. This is not encryption and does not
//! protect against a local attacker; it only avoids storing keys in plaintext
//! so a stray backup/log/editor peek does not leak them. Nothing is ever sent
//! over the network by this code.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

const FILE_NAME: &str = "keys.json";

#[derive(Default, Serialize, Deserialize)]
struct StoredKeys {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    soniox: Option<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    llm: Option<Entry>,
}

#[derive(Serialize, Deserialize)]
struct Entry {
    nonce: String,
    data: String,
}

fn derive_mask(nonce: &[u8], length: usize) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"innote-local-key-v1");
    hasher.update(nonce);
    let base = hasher.finalize();
    (0..length)
        .map(|i| base[i % base.len()])
        .collect::<Vec<u8>>()
}

fn obfuscate(text: &str) -> Entry {
    // Build a random-ish nonce from time/process data. This is obfuscation, not
    // cryptography, so a unique per-write mask is sufficient.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mut seed = Vec::new();
    seed.extend_from_slice(&now.to_le_bytes());
    seed.extend_from_slice(&pid.to_le_bytes());
    let digest = Sha256::digest(&seed);
    let mut nonce = [0u8; 16];
    nonce.copy_from_slice(&digest[..16]);
    let bytes = text.as_bytes();
    let mask = derive_mask(&nonce, bytes.len());
    let data: Vec<u8> = bytes.iter().zip(mask.iter()).map(|(a, b)| a ^ b).collect();
    Entry {
        nonce: hex::encode(nonce),
        data: hex::encode(data),
    }
}

fn deobfuscate(entry: &Entry) -> Result<String> {
    let nonce = hex::decode(&entry.nonce).context("Nonce sai")?;
    let data = hex::decode(&entry.data).context("Dữ liệu sai")?;
    let mask = derive_mask(&nonce, data.len());
    let out: Vec<u8> = data.iter().zip(mask.iter()).map(|(a, b)| a ^ b).collect();
    String::from_utf8(out).context("Không đọc được key đã lưu")
}

/// Public helpers so other modules can store obfuscated secrets with the same
/// scheme (used by the multi-profile AI configuration store).
#[derive(Default, Serialize, Deserialize)]
pub struct ObfuscatedEntry {
    pub nonce: String,
    pub data: String,
}

pub fn obfuscate_entry(text: &str) -> ObfuscatedEntry {
    let compiled = obfuscate(text);
    ObfuscatedEntry {
        nonce: compiled.nonce,
        data: compiled.data,
    }
}

pub fn deobfuscate_from(entry: &ObfuscatedEntry) -> Result<String> {
    let compiled = Entry {
        nonce: entry.nonce.clone(),
        data: entry.data.clone(),
    };
    deobfuscate(&compiled)
}

fn path(root: &Path) -> std::path::PathBuf {
    root.join(FILE_NAME)
}

/// Read both API keys from the local data directory (if present).
pub fn load(root: &Path) -> (String, String) {
    let content = match std::fs::read(path(root)) {
        Ok(c) => c,
        Err(_) => return (String::new(), String::new()),
    };
    let stored: StoredKeys = match serde_json::from_slice(&content) {
        Ok(s) => s,
        Err(_) => return (String::new(), String::new()),
    };
    let soniox = stored
        .soniox
        .as_ref()
        .and_then(|e| deobfuscate(e).ok())
        .unwrap_or_default();
    let llm = stored
        .llm
        .as_ref()
        .and_then(|e| deobfuscate(e).ok())
        .unwrap_or_default();
    (soniox, llm)
}

/// Persist the current keys. Empty values remove that key from disk.
pub fn save(root: &Path, soniox: &str, llm: &str) -> Result<()> {
    let mut stored = StoredKeys::default();
    if !soniox.trim().is_empty() {
        stored.soniox = Some(obfuscate(soniox));
    }
    if !llm.trim().is_empty() {
        stored.llm = Some(obfuscate(llm));
    }
    std::fs::create_dir_all(root).context("Không tạo được thư mục dữ liệu")?;
    let target = path(root);
    if soniox.trim().is_empty() && llm.trim().is_empty() {
        // Remove the credential file entirely when the last key is cleared.
        let _ = std::fs::remove_file(&target);
        return Ok(());
    }
    std::fs::write(&target, serde_json::to_vec(&stored)?)
        .context("Không ghi được dữ liệu key an toàn")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_preserves_unicode_keys() {
        let key = "sk-việt-nam-テスト-🔑-1234567890";
        let entry = obfuscate(key);
        assert_eq!(deobfuscate(&entry).unwrap(), key);
    }

    #[test]
    fn empty_save_removes_file_entries() {
        let dir = std::env::temp_dir().join(format!("innote-key-test-{}", uuid::Uuid::new_v4()));
        save(&dir, "abc", "").unwrap();
        let (s, l) = load(&dir);
        assert_eq!(s, "abc");
        assert_eq!(l, "");
        save(&dir, "", "").unwrap();
        assert_eq!(
            std::fs::metadata(path(&dir)).map(|m| m.len()).unwrap_or(0),
            0
        ); // empty JSON entry, key removed
        let _ = std::fs::remove_dir_all(&dir);
    }
}
