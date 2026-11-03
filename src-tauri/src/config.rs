//! Named AI configurations ("profiles"). A profile stores one provider, an
//! optional Base URL and one API key, obfuscated at rest like the legacy
//! key store. Nothing here logs, echoes or persists raw keys.
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};

pub const PROVIDERS: &[&str] = &[
    "soniox",
    "groq",
    "openai",
    "deepseek",
    "gemini",
    "openai-compatible",
];
pub const CAPTION_PROVIDERS: &[&str] = &["soniox", "openai", "gemini", "groq"];

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ModelChoice {
    #[serde(alias = "profile_id")]
    pub profile_id: String,
    pub model: String,
}

/// Providers whose features accept a Base URL override.
pub fn provider_accepts_base_url(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "gemini" | "openai-compatible" | "deepseek" | "groq"
    )
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiProfile {
    pub id: String,
    pub name: String,
    pub provider: String,
    /// Empty means the provider default.
    pub base_url: String,
    /// Real key in memory only; obfuscated on disk.
    pub key: String,
    #[serde(default)]
    pub created: String,
    /// Set when imported from the legacy `keys.json`.
    #[serde(default)]
    pub import_tag: String,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiProfileSummary {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub base_url: String,
    pub has_key: bool,
    /// Last 4 characters of the key, useful to recognize which key is stored.
    pub key_tail: String,
    pub created: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfileInput {
    pub id: Option<String>,
    pub name: String,
    pub provider: String,
    #[serde(alias = "base_url")]
    pub base_url: String,
    #[serde(alias = "api_key")]
    pub api_key: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestInput {
    pub provider: String,
    #[serde(alias = "api_key", deserialize_with = "null_string")]
    pub api_key: String,
    #[serde(alias = "base_url")]
    pub base_url: String,
    pub model: String,
    #[serde(default, alias = "profile_id")]
    pub profile_id: Option<String>,
}

fn null_string<'de, D: serde::Deserializer<'de>>(d: D) -> std::result::Result<String, D::Error> {
    Ok(Option::<String>::deserialize(d)?.unwrap_or_default())
}

/// Only a digest is kept in the short-lived verification cache, never the key.
pub fn verification_id(provider: &str, base_url: &str, key: &str) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(
        serde_json::to_vec(&(provider, base_url, key)).unwrap(),
    ))
}

pub fn profile_file(root: &std::path::Path) -> std::path::PathBuf {
    root.join("ai-profiles.json")
}

#[derive(Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    profiles: Vec<StoredProfile>,
}

#[derive(Serialize, Deserialize)]
struct StoredProfile {
    id: String,
    name: String,
    provider: String,
    base_url: String,
    entry: crate::secure::ObfuscatedEntry,
    created: String,
    #[serde(default)]
    import_tag: String,
}

pub fn load(root: &std::path::Path) -> Vec<AiProfile> {
    load_checked(root).unwrap_or_default()
}
pub fn load_checked(root: &std::path::Path) -> Result<Vec<AiProfile>> {
    let content = match std::fs::read(profile_file(root)) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => anyhow::bail!("Không đọc được kho cấu hình AI; dữ liệu cũ được giữ nguyên"),
    };
    let stored: Stored = serde_json::from_slice(&content)
        .context("Kho cấu hình AI bị hỏng; dữ liệu cũ được giữ nguyên, có thể khôi phục từ ai-profiles.backup.json")?;
    stored
        .profiles
        .into_iter()
        .map(|p| {
            let key = crate::secure::deobfuscate_from(&p.entry)
                .context("Không đọc được key đã lưu; dữ liệu cũ được giữ nguyên")?;
            Ok(AiProfile {
                id: p.id,
                name: p.name,
                provider: p.provider,
                base_url: p.base_url,
                key,
                created: p.created,
                import_tag: p.import_tag,
            })
        })
        .collect()
}

pub fn save(root: &std::path::Path, profiles: &[AiProfile]) -> Result<()> {
    std::fs::create_dir_all(root).context("Không tạo được thư mục dữ liệu")?;
    load_checked(root)?; // Never overwrite a corrupt or unreadable store with an empty one.
    let stored = Stored {
        profiles: profiles
            .iter()
            .map(|p| StoredProfile {
                id: p.id.clone(),
                name: p.name.clone(),
                provider: p.provider.clone(),
                base_url: p.base_url.clone(),
                entry: crate::secure::obfuscate_entry(&p.key),
                created: p.created.clone(),
                import_tag: p.import_tag.clone(),
            })
            .collect(),
    };
    std::fs::write(profile_file(root), serde_json::to_vec(&stored)?)
        .context("Không ghi được cấu hình AI")
}
pub fn normalize(provider: &str) -> Result<String> {
    let provider = provider.trim().to_ascii_lowercase();
    ensure!(
        PROVIDERS.contains(&provider.as_str()),
        "Nhà cung cấp AI không hợp lệ: {provider}"
    );
    Ok(provider)
}

pub fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    ensure!(
        !name.is_empty() && name.chars().count() <= 60,
        "Tên cấu hình phải từ 1 đến 60 ký tự"
    );
    ensure!(
        !name.chars().any(|c| c == '\r' || c == '\n'),
        "Tên cấu hình không được chứa ký tự xuống dòng"
    );
    Ok(name.to_string())
}

/// Validate and normalize an edit payload. Returns (name, provider, base_url, key).
pub fn validate_input(input: &AiProfileInput) -> Result<(String, String, String, String)> {
    let provider = normalize(&input.provider)?;
    let name = validate_name(&input.name)?;
    let base_url = input.base_url.trim().trim_end_matches('/').to_string();
    if provider_accepts_base_url(&provider) && !base_url.is_empty() {
        let parsed = reqwest::Url::parse(&base_url).context("Base URL không hợp lệ")?;
        ensure!(
            matches!(parsed.scheme(), "https" | "http"),
            "Base URL phải là http hoặc https"
        );
    }
    let key = input.api_key.trim().to_string();
    if !key.is_empty() {
        ensure!(
            key.chars().count() <= 4096,
            "API key quá dài (tối đa 4096 ký tự)"
        );
        ensure!(
            key.bytes().all(|b| !b.is_ascii_control()),
            "API key chứa ký tự không hợp lệ"
        );
    }
    Ok((name, provider, base_url, key))
}

pub fn to_summary(p: &AiProfile) -> AiProfileSummary {
    let tail = if p.key.is_empty() {
        String::new()
    } else {
        p.key
            .chars()
            .rev()
            .take(4)
            .collect::<String>()
            .chars()
            .rev()
            .collect()
    };
    AiProfileSummary {
        id: p.id.clone(),
        name: p.name.clone(),
        provider: p.provider.clone(),
        base_url: p.base_url.clone(),
        has_key: !p.key.is_empty(),
        key_tail: tail,
        created: p.created.clone(),
    }
}

/// Import the legacy `keys.json` values into named profiles once, so users who
/// stored keys in older versions keep working after the upgrade.
pub fn migrate_legacy(
    root: &std::path::Path,
    mut profiles: Vec<AiProfile>,
    soniox: &str,
    llm: &str,
) -> Vec<AiProfile> {
    if load_checked(root).is_err() {
        return profiles;
    }
    let marker = root.join("ai-legacy-migrated.json");
    let mut imported: Vec<String> = std::fs::read(&marker)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut changed = false;
    let mut add =
        |profiles: &mut Vec<AiProfile>, provider: &str, key: &str, name: &str, tag: &str| {
            if key.trim().is_empty() || imported.iter().any(|p| p == provider) {
                return;
            }
            if profiles
                .iter()
                .any(|p| p.provider == provider && p.import_tag == tag)
            {
                return;
            }
            profiles.push(AiProfile {
                id: uuid::Uuid::new_v4().to_string(),
                name: name.to_string(),
                provider: provider.to_string(),
                base_url: String::new(),
                key: key.trim().to_string(),
                created: chrono::Utc::now().to_rfc3339(),
                import_tag: tag.to_string(),
            });
            changed = true;
        };
    add(
        &mut profiles,
        "soniox",
        soniox,
        "Soniox (kế thừa)",
        "legacy-soniox",
    );
    add(
        &mut profiles,
        "openai",
        llm,
        "OpenAI (kế thừa)",
        "legacy-llm",
    );
    if changed && save(root, &profiles).is_err() {
        return profiles;
    }
    for p in &profiles {
        if !p.import_tag.is_empty() && !imported.contains(&p.provider) {
            imported.push(p.provider.clone());
        }
    }
    if !imported.is_empty() {
        let _ = std::fs::write(marker, serde_json::to_vec(&imported).unwrap());
    }
    profiles
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frontend_payloads_and_legacy_snake_case_are_accepted() {
        let input: AiTestInput = serde_json::from_value(serde_json::json!({
            "provider":"soniox", "apiKey":null, "baseUrl":"", "model":"", "profileId":"p1"
        }))
        .unwrap();
        assert_eq!(input.profile_id.as_deref(), Some("p1"));
        assert!(input.api_key.is_empty());
        let profile: AiProfileInput = serde_json::from_value(serde_json::json!({
            "name":"work", "provider":"soniox", "apiKey":"test-key", "baseUrl":""
        }))
        .unwrap();
        assert_eq!(profile.api_key, "test-key");
        let legacy: AiTestInput = serde_json::from_value(serde_json::json!({
            "provider":"openai", "api_key":"k", "base_url":"", "model":"", "profile_id":"p2"
        }))
        .unwrap();
        assert_eq!(legacy.profile_id.as_deref(), Some("p2"));
        assert_ne!(
            verification_id("openai", "", "k"),
            verification_id("gemini", "", "k")
        );
        assert_ne!(
            verification_id("openai", "", "k"),
            verification_id("openai", "https://proxy/v1", "k")
        );
        assert_ne!(
            verification_id("openai", "", "k"),
            verification_id("openai", "", "new")
        );
    }

    #[test]
    fn save_and_load_roundtrip_hides_keys() {
        let dir = std::env::temp_dir().join(format!("innote-cfg-{}", uuid::Uuid::new_v4()));
        let profiles = vec![AiProfile {
            id: "p1".into(),
            name: "Soniox chính".into(),
            provider: "soniox".into(),
            base_url: String::new(),
            key: "sk-viet-🔑-test".into(),
            created: "now".into(),
            import_tag: String::new(),
        }];
        save(&dir, &profiles).unwrap();
        let raw = std::fs::read_to_string(profile_file(&dir)).unwrap();
        assert!(
            !raw.contains("sk-viet"),
            "key must not be plaintext on disk"
        );
        let loaded = load(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].key, "sk-viet-🔑-test");
        let summary = to_summary(&loaded[0]);
        assert!(summary.has_key);
        assert_eq!(summary.key_tail, "test");
        assert!(!serde_json::to_string(&summary).unwrap().contains("sk-viet"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn validate_input_rejects_bad_provider_and_url() {
        let bad = AiProfileInput {
            id: None,
            name: "x".into(),
            provider: "mystery".into(),
            base_url: String::new(),
            api_key: "k".into(),
        };
        assert!(validate_input(&bad).is_err());
        let bad_url = AiProfileInput {
            id: None,
            name: "x".into(),
            provider: "openai".into(),
            base_url: "ftp://nope".into(),
            api_key: "k".into(),
        };
        assert!(validate_input(&bad_url).is_err());
        let ok = AiProfileInput {
            id: None,
            name: "  Cấu hình OK  ".into(),
            provider: "OPENAI".into(),
            base_url: "https://api.openai.com/v1/".into(),
            api_key: " sk-abc ".into(),
        };
        let (name, provider, base_url, key) = validate_input(&ok).unwrap();
        assert_eq!(name, "Cấu hình OK");
        assert_eq!(provider, "openai");
        assert_eq!(base_url, "https://api.openai.com/v1");
        assert_eq!(key, "sk-abc");
    }

    #[test]
    fn migrate_legacy_imports_and_is_idempotent() {
        let dir = std::env::temp_dir().join(format!("innote-cfg-mig-{}", uuid::Uuid::new_v4()));
        let imported = migrate_legacy(&dir, vec![], "soniox-k", "llm-k");
        assert_eq!(imported.len(), 2);
        assert!(imported.iter().any(|p| p.provider == "soniox"));
        assert!(imported.iter().any(|p| p.provider == "openai"));
        // A second boot must not duplicate imports.
        let again = migrate_legacy(&dir, imported, "soniox-k", "llm-k");
        assert_eq!(again.len(), 2);
        assert_eq!(load(&dir).len(), 2);
        // Deleting an imported profile must not recreate it at every boot.
        let retained: Vec<_> = again
            .into_iter()
            .filter(|p| p.provider != "soniox")
            .collect();
        save(&dir, &retained).unwrap();
        let next_boot = migrate_legacy(&dir, load(&dir), "soniox-k", "llm-k");
        assert_eq!(next_boot.len(), 1);
        assert_eq!(next_boot[0].provider, "openai");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn corrupt_store_is_not_silently_overwritten_by_save_or_migration() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(profile_file(&dir), b"broken file keep for recovery").unwrap();
        assert!(load_checked(&dir).is_err());
        assert!(save(&dir, &[]).is_err());
        assert!(migrate_legacy(&dir, vec![], "legacy-key", "").is_empty());
        assert_eq!(
            std::fs::read(profile_file(&dir)).unwrap(),
            b"broken file keep for recovery"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
