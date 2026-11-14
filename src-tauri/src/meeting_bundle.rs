//! Agent-readable snapshots, deliberately independent of private profile/config serialization.
use crate::{meeting_store as store, storage_paths, types::Session};
use anyhow::Result;
use sha2::{Digest, Sha256};
use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};
static PUBLISH: Mutex<()> = Mutex::new(());
pub fn changed(root: &Path, id: &str, flush: bool) {
    if flush {
        let _ = publish(root, id);
        return;
    }
    static QUEUE: OnceLock<std::sync::mpsc::Sender<(std::path::PathBuf, String)>> = OnceLock::new();
    let sender = QUEUE.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<(std::path::PathBuf, String)>();
        std::thread::spawn(move || loop {
            let Ok(first) = rx.recv() else {
                break;
            };
            let mut batch = std::collections::HashSet::from([first]);
            std::thread::sleep(std::time::Duration::from_secs(1));
            batch.extend(rx.try_iter());
            for (root, id) in batch {
                let _ = publish(&root, &id);
            }
        });
        tx
    });
    let _ = sender.send((root.to_owned(), id.to_owned()));
}
pub fn snapshot(s: &Session, revision: u64, audio: bool, speakers: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"schema_version":2,"revision":revision,"id":s.id,"title":s.title,"created":s.created,"status":s.status,"duration":s.duration,"language":s.language,
        "original":{"segments":s.segments,"live":s.live}, "translations":s.translations,
        "corrected":s.corrected.as_ref().map(|c|serde_json::json!({"text":c.text,"segments":c.segments,"provider":c.provider,"model":c.model,"created":c.created})),
        "summary":s.summary.as_ref().map(|c|serde_json::json!({"text":c.text,"source":c.source,"provider":c.provider,"model":c.model,"created":c.created})),
        "speakers":speakers,
        "notes":s.notes,"audio":{"available":audio,"path":if audio {Some("recording.wav")} else {None}}})
}
pub fn publish(root: &Path, id: &str) -> Result<()> {
    let _lock = PUBLISH.lock().unwrap();
    let location = store::locate(root, id)?;
    if !location.modern {
        return Ok(());
    }
    let (revision, _, _) = store::revision(&location, id)?;
    let s = store::load(&location, id)?;
    let speakers = ["original", "corrected"]
        .iter()
        .find_map(|version| {
            crate::speaker::load_analysis(root, id, version)
                .ok()
                .filter(|a| a.status == "ready")
        })
        .map(|a| serde_json::json!({
            "transcript_version": a.transcript_version,
            "revision": a.revision,
            "participants": a.participants,
            "segments": a.segments,
        }))
        .unwrap_or(serde_json::Value::Null);
    let original_labels = crate::speaker::export_labels(root, id, "original").unwrap_or_default();
    let corrected_labels = crate::speaker::export_labels(root, id, "corrected").unwrap_or_default();
    let result = (|| -> Result<()> {
        storage_paths::reject_reparse(&location.folder)?;
        crate::meeting_file_guide::ensure_library_guide(location.folder.parent().ok_or_else(||anyhow::anyhow!("Thiếu thư mục thư viện"))?)?;
        let mut outputs = std::collections::BTreeMap::new();
        if !s.segments.is_empty() || !s.live.is_empty() {
            outputs.insert(
                "original.md".to_owned(),
                format!("> BẢN CHÉP GỐC — lời nhận dạng trong phiên ghi và sửa tay đã lưu; hãy nghe audio để đối chiếu khi cần.\n\n{}",crate::storage::export_version_with_labels(&s,"md","original",&original_labels)),
            );
            outputs.insert(
                "original.srt".to_owned(),
                crate::storage::export_version_with_labels(&s, "srt", "original", &original_labels),
            );
        }
        if s.corrected.is_some() {
            outputs.insert(
                "corrected.md".into(),
                format!("> BẢN HIỆU CHỈNH — kết quả model nghe lại bản ghi âm; bản chép gốc được giữ riêng.\n\n{}",crate::storage::export_version_with_labels(&s, "md", "corrected", &corrected_labels)),
            );
            outputs.insert(
                "corrected.srt".into(),
                crate::storage::export_version_with_labels(&s, "srt", "corrected", &corrected_labels),
            );
        }
        if let Some(summary) = &s.summary {
            outputs.insert(
                "summary.md".into(),
                format!("# Tóm tắt — {}\n\n{}\n", s.title, summary.text),
            );
        }
        if !s.notes.is_empty() {
            outputs.insert("notes.md".into(), format!("# Ghi chú — {}\n\n{}\n",s.title,s.notes));
        }
        let langs: std::collections::BTreeSet<_> =
            s.translations.iter().map(|t| t.language.as_str()).collect();
        for lang in langs {
            anyhow::ensure!(
                !lang.is_empty()
                    && lang.len() <= 32
                    && lang.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'),
                "Mã ngôn ngữ không hợp lệ"
            );
            outputs.insert(
                format!("translations/{lang}.md"),
                crate::storage::export_version(&s, "md", &format!("translation:{lang}")),
            );
            // Translation timestamps refer to source ranges, not separately aligned translated words.
        }
        let mut manifest = std::collections::BTreeMap::new();
        let previous = std::fs::read(location.folder.join("meeting.json"))
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok());
        if let Some(old) = previous.as_ref().and_then(|v| v["manifest"].as_object()) {
            for (name, digest) in old {
                if outputs.contains_key(name) {
                    continue;
                }
                let known = [
                    "original.md",
                    "original.srt",
                    "corrected.md",
                    "corrected.srt",
                    "summary.md",
                    "notes.md",
                ]
                .contains(&name.as_str())
                    || name
                        .strip_prefix("translations/")
                        .and_then(|n| n.strip_suffix(".md"))
                        .is_some_and(|n| {
                            !n.is_empty()
                                && n.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
                        });
                if !known {
                    continue;
                }
                let target = location.folder.join(name);
                if target.exists() {
                    storage_paths::reject_reparse(&target)?;
                    let bytes = std::fs::read(&target)?;
                    if digest.as_str() == Some(hex::encode(Sha256::digest(&bytes)).as_str()) {
                        std::fs::remove_file(target)?;
                    } else {
                        std::fs::rename(
                            &target,
                            target.with_extension(format!("user-copy-{}.md", uuid::Uuid::new_v4())),
                        )?;
                    }
                }
            }
        }
        for (name, text) in &outputs {
            let path = location.folder.join(name);
            std::fs::create_dir_all(path.parent().unwrap())?;
            storage_paths::atomic_write(&path, text.as_bytes())?;
            manifest.insert(name.clone(), hex::encode(Sha256::digest(text.as_bytes())));
        }
        let guide=crate::meeting_file_guide::meeting_guide(&s,&location.folder);
        storage_paths::atomic_write(&location.folder.join(crate::meeting_file_guide::MEETING_GUIDE),guide.as_bytes())?;
        manifest.insert(crate::meeting_file_guide::MEETING_GUIDE.to_owned(),hex::encode(Sha256::digest(guide.as_bytes())));
        let mut value = snapshot(
            &s,
            revision,
            location.folder.join("recording.wav").is_file(),
            speakers,
        );
        value["manifest"] = serde_json::to_value(&manifest)?;
        storage_paths::atomic_write(
            &location.folder.join("meeting.json"),
            &serde_json::to_vec_pretty(&value)?,
        )?;
        Ok(())
    })();
    let c = store::connection(&location.database)?;
    match &result {
        Ok(()) => {
            c.execute(
                "UPDATE session_paths SET bundle_revision=?2,bundle_error=NULL WHERE session_id=?1",
                rusqlite::params![id, revision],
            )?;
        }
        Err(_) => {
            c.execute("UPDATE session_paths SET bundle_error='Đã lưu trong Innote, chưa cập nhật xong tệp cuộc họp' WHERE session_id=?1",[id])?;
        }
    }
    result
}
