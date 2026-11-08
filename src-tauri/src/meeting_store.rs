//! Location routing: the private root is never a destination for new libraries.
use crate::{storage_paths as paths, types::Session};
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Location {
    pub database: PathBuf,
    pub folder: PathBuf,
    pub modern: bool,
}
impl Location {
    pub fn capture(&self) -> PathBuf {
        if self.modern {
            self.folder.join(".innote/work")
        } else {
            self.folder.clone()
        }
    }
}
pub fn libraries(root: &Path) -> Result<Vec<paths::LibraryRef>> {
    Ok(paths::load_registry(root)?.libraries)
}
pub fn default_library(root: &Path) -> Result<paths::LibraryRef> {
    let r = paths::load_registry(root)?;
    let library = r
        .libraries
        .into_iter()
        .find(|l| Some(&l.id) == r.default_library_id.as_ref())
        .context("Hãy chọn nơi lưu cuộc họp mới trong Cài đặt → Lưu trữ")?;
    validate_registered(&library)?;
    Ok(library)
}
pub fn validate_registered(l: &paths::LibraryRef) -> Result<()> {
    let marker = l.root.join(".innote/library.json");
    paths::reject_reparse(&marker)?;
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        v["library_id"].as_str() == Some(l.id.as_str()) && v["schema_version"] == 1,
        "Ổ lưu không còn chứa đúng thư viện đã đăng ký; hãy chọn lại nơi lưu"
    );
    Ok(())
}
pub fn connection(path: &Path) -> Result<Connection> {
    paths::reject_reparse(path)?;
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(c)
}
fn roots(root: &Path) -> Result<Vec<(PathBuf, Option<PathBuf>)>> {
    let mut out = vec![(root.join("meetings.sqlite"), None)];
    for l in libraries(root)? {
        if validate_registered(&l).is_ok() {
            out.push((l.root.join(".innote/meetings.sqlite"), Some(l.root)));
        }
    }
    Ok(out)
}
pub fn locate(root: &Path, id: &str) -> Result<Location> {
    crate::storage::ensure_id(id)?;
    let mut found = None;
    for (db, library) in roots(root)? {
        if !db.try_exists().unwrap_or(false) {
            continue;
        }
        let Ok(c) = connection(&db) else {
            continue;
        };
        let exists: bool = c.query_row(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id=?1)",
            [id],
            |r| r.get(0),
        )?;
        if !exists {
            continue;
        }
        anyhow::ensure!(
            found.is_none(),
            "Trùng mã cuộc họp trong nhiều thư viện; không tự chọn bản dữ liệu"
        );
        let modern = library.is_some();
        let folder = if let Some(base) = library {
            let name: String = c.query_row(
                "SELECT relative_folder FROM session_paths WHERE session_id=?1",
                [id],
                |r| r.get(0),
            )?;
            anyhow::ensure!(
                Path::new(&name).components().count() == 1
                    && !name.contains(['/', '\\'])
                    && name != "."
                    && name != "..",
                "Đường dẫn cuộc họp không hợp lệ"
            );
            recover_rename(&c, &base, id, &name)?
        } else {
            root.join("sessions").join(id)
        };
        if folder.exists() {
            paths::reject_reparse(&folder)?;
        }
        found = Some(Location {
            database: db,
            folder,
            modern,
        });
    }
    found.context("Không tìm thấy cuộc họp hoặc ổ lưu đang mất kết nối")
}
pub fn list(root: &Path) -> Result<Vec<Session>> {
    let mut out = Vec::new();
    let mut ids = std::collections::HashSet::new();
    for (db, _) in roots(root)? {
        if !db.try_exists().unwrap_or(false) {
            continue;
        }
        let Ok(c) = connection(&db) else {
            continue;
        };
        let Ok(mut q) = c.prepare("SELECT body FROM sessions") else {
            continue;
        };
        for row in q.query_map([], |r| r.get::<_, String>(0))? {
            let s: Session = serde_json::from_str(&row?)?;
            anyhow::ensure!(
                ids.insert(s.id.clone()),
                "Trùng mã cuộc họp giữa các thư viện"
            );
            out.push(s);
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(out)
}
pub fn folder_name(s: &Session, root: &Path) -> Result<String> {
    let time = chrono::DateTime::parse_from_rfc3339(&s.created)?
        .format("%Y-%m-%d_%H-%M-%S")
        .to_string();
    let mut title = String::new();
    let budget =
        240usize.saturating_sub(root.as_os_str().to_string_lossy().encode_utf16().count() + 80);
    anyhow::ensure!(budget >= 10, "Đường dẫn nơi lưu quá dài");
    for ch in s.title.chars().filter(|c| !c.is_control()) {
        let ch = if "<>:\"/\\|?*".contains(ch) { '_' } else { ch };
        if title.encode_utf16().count() + ch.len_utf16() > budget {
            break;
        }
        title.push(ch);
    }
    let title = title.trim().trim_end_matches(['.', ' ']);
    Ok(format!(
        "{time} - {}",
        if title.is_empty() {
            "Cuộc họp"
        } else {
            title
        }
    ))
}
pub fn create(root: &Path, s: &Session) -> Result<Location> {
    crate::storage::ensure_id(&s.id)?;
    let library = default_library(root)?;
    anyhow::ensure!(
        paths::available_bytes(&library.root)? > 16 * 1024 * 1024,
        "Nơi lưu còn quá ít dung lượng"
    );
    paths::reject_reparse(&library.root)?;
    let internal = library.root.join(".innote");
    paths::reject_reparse(&internal)?;
    let database = internal.join("meetings.sqlite");
    let mut c = Connection::open(&database)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY,body TEXT NOT NULL); CREATE TABLE IF NOT EXISTS session_paths(session_id TEXT PRIMARY KEY,relative_folder TEXT NOT NULL UNIQUE,revision INTEGER NOT NULL DEFAULT 0,bundle_revision INTEGER NOT NULL DEFAULT 0,bundle_error TEXT,pending_folder_name TEXT);")?;
    let base = folder_name(s, &library.root)?;
    let folder = (1..10000)
        .find_map(|n| {
            let name = if n == 1 {
                base.clone()
            } else {
                format!("{base} ({n})")
            };
            let p = library.root.join(name);
            match std::fs::create_dir(&p) {
                Ok(()) => Some(Ok(p)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => None,
                Err(e) => Some(Err(e)),
            }
        })
        .context("Không tạo được tên thư mục duy nhất")??;
    let location = Location {
        database,
        folder,
        modern: true,
    };
    // Persist identity before the DB transaction: an interrupted create remains discoverable.
    paths::atomic_write(
        &location.folder.join(".innote-identity.json"),
        &serde_json::to_vec(&serde_json::json!({"id":s.id,"library_id":library.id}))?,
    )?;
    std::fs::create_dir_all(location.capture())?;
    let tx = c.transaction()?;
    tx.execute(
        "INSERT INTO sessions VALUES(?1,?2)",
        rusqlite::params![s.id, serde_json::to_string(s)?],
    )?;
    tx.execute(
        "INSERT INTO session_paths(session_id,relative_folder,revision) VALUES(?1,?2,1)",
        rusqlite::params![s.id, location.folder.file_name().unwrap().to_string_lossy()],
    )?;
    tx.commit()?;
    Ok(location)
}
pub fn load(location: &Location, id: &str) -> Result<Session> {
    let text: String = connection(&location.database)?.query_row(
        "SELECT body FROM sessions WHERE id=?1",
        [id],
        |r| r.get(0),
    )?;
    Ok(serde_json::from_str(&text)?)
}
pub fn save(location: &Location, s: &Session) -> Result<()> {
    let mut c = connection(&location.database)?;
    let tx = c.transaction()?;
    tx.execute(
        "UPDATE sessions SET body=?2 WHERE id=?1",
        rusqlite::params![s.id, serde_json::to_string(s)?],
    )?;
    if location.modern {
        tx.execute(
            "UPDATE session_paths SET revision=revision+1 WHERE session_id=?1",
            [&s.id],
        )?;
    }
    tx.commit()?;
    Ok(())
}
pub fn revision(location: &Location, id: &str) -> Result<(u64, u64, Option<String>)> {
    Ok(connection(&location.database)?.query_row(
        "SELECT revision,bundle_revision,bundle_error FROM session_paths WHERE session_id=?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?)
}
fn recover_rename(_c: &Connection, base: &Path, id: &str, current: &str) -> Result<PathBuf> {
    let journal = base.join(".innote/operations").join(format!("{id}.json"));
    if journal.try_exists()? {
        paths::reject_reparse(&journal)?;
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(&journal)?)?;
        let next = v["next"].as_str().context("Journal đổi tên không hợp lệ")?;
        anyhow::ensure!(
            next != "." && next != ".." && !next.contains(['/', '\\', ':']) && !next.is_empty(),
            "Journal đường dẫn không hợp lệ"
        );
        let target = base.join(next);
        if !base.join(current).exists() && target.exists() {
            paths::reject_reparse(&target)?;
            let identity: serde_json::Value =
                serde_json::from_slice(&std::fs::read(target.join(".innote-identity.json"))?)?;
            anyhow::ensure!(
                identity["id"].as_str() == Some(id),
                "Thư mục đổi tên không đúng cuộc họp"
            );
            return Ok(target); // Resolution is read-only; a writer completes the journal.
        }
    }
    Ok(base.join(current))
}
pub fn rename(root: &Path, id: &str, defer: bool) -> Result<()> {
    let l = locate(root, id)?;
    if !l.modern {
        return Ok(());
    }
    let s = load(&l, id)?;
    let base = l.folder.parent().context("Thiếu thư viện")?;
    let name = folder_name(&s, base)?;
    let current = l.folder.file_name().unwrap().to_string_lossy();
    let c = connection(&l.database)?;
    c.execute(
        "UPDATE session_paths SET relative_folder=?2 WHERE session_id=?1",
        rusqlite::params![id, current],
    )?;
    if current == name || current.starts_with(&format!("{name} (")) {
        return Ok(());
    }
    c.execute(
        "UPDATE session_paths SET pending_folder_name=?2 WHERE session_id=?1",
        rusqlite::params![id, name],
    )?;
    if defer {
        return Ok(());
    }
    for n in 1..10000 {
        let next = if n == 1 {
            name.clone()
        } else {
            format!("{name} ({n})")
        };
        let target = base.join(&next);
        if target.exists() {
            continue;
        }
        let ops = base.join(".innote/operations");
        std::fs::create_dir_all(&ops)?;
        let journal = ops.join(format!("{id}.json"));
        paths::atomic_write(
            &journal,
            &serde_json::to_vec(&serde_json::json!({"id":id,"next":next}))?,
        )?;
        if std::fs::rename(&l.folder, &target).is_err() {
            return Ok(());
        } // Pending, e.g. Windows player holds WAV.
        c.execute("UPDATE session_paths SET relative_folder=?2,pending_folder_name=NULL WHERE session_id=?1",rusqlite::params![id,next])?;
        std::fs::remove_file(journal)?;
        return Ok(());
    }
    anyhow::bail!("Không có tên thư mục trống")
}
pub fn rescan(root: &Path) -> Result<usize> {
    let mut restored = 0;
    for library in libraries(root)? {
        if validate_registered(&library).is_err() {
            continue;
        }
        let db = library.root.join(".innote/meetings.sqlite");
        if !db.exists() {
            continue;
        }
        let c = connection(&db)?;
        let mut identities = std::collections::HashMap::new();
        for entry in std::fs::read_dir(&library.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() || entry.file_name() == ".innote" {
                continue;
            }
            paths::reject_reparse(&entry.path())?;
            let marker = entry.path().join(".innote-identity.json");
            if !marker.exists() {
                continue;
            }
            paths::reject_reparse(&marker)?;
            let identity: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
            if identity["library_id"].as_str() != Some(&library.id) {
                continue;
            }
            let id = identity["id"]
                .as_str()
                .context("Nhận diện cuộc họp không hợp lệ")?;
            crate::storage::ensure_id(id)?;
            anyhow::ensure!(
                identities
                    .insert(id.to_owned(), entry.file_name())
                    .is_none(),
                "Có nhiều thư mục cùng mã cuộc họp; hãy bỏ bản sao hoặc chọn đúng thư viện"
            );
        }
        for (id, name) in identities {
            let old: Option<String> = c
                .query_row(
                    "SELECT relative_folder FROM session_paths WHERE session_id=?1",
                    [&id],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(old) = old {
                if !library.root.join(old).exists() {
                    restored += c.execute(
                        "UPDATE session_paths SET relative_folder=?2 WHERE session_id=?1",
                        rusqlite::params![id, name.to_string_lossy()],
                    )?;
                }
            }
        }
    }
    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub struct Fixture {
        pub base: PathBuf,
        pub private: PathBuf,
    }
    impl Fixture {
        pub fn new() -> Self {
            let base =
                std::env::temp_dir().join(format!("innote-library-test-{}", uuid::Uuid::new_v4()));
            for name in ["private", "runtime", "a", "b"] {
                std::fs::create_dir_all(base.join(name)).unwrap();
            }
            let base = base.canonicalize().unwrap();
            let private = base.join("private");
            Self { base, private }
        }
        pub fn select(&self, name: &str) -> paths::LibraryRef {
            let valid = paths::validate_library_root(
                &self.base.join(name),
                &self.private,
                &self.base.join("runtime"),
            )
            .unwrap();
            let l = paths::open_or_create_library(&valid).unwrap();
            let mut registry = paths::load_registry(&self.private).unwrap();
            registry.select(l.clone()).unwrap();
            paths::save_registry(&self.private, &registry).unwrap();
            l
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }
    pub fn session(title: &str) -> Session {
        serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"title":title,"created":"2026-11-07T09:30:00+07:00","duration":1.,"status":"completed","language":"vi","model":"moonshine-vi","provider":"local","processed":1.,"segments":[{"id":"word1","start":0.,"end":1.,"text":"Ngân sách là 15 triệu, không phải 50 triệu.","edited":false}],"notes":"Ghi chú","error":null})).unwrap()
    }
    #[test]
    fn legacy_and_two_libraries_remain_independent_after_restart() {
        let f = Fixture::new();
        let old = session("Cũ");
        crate::storage::save(&f.private, &old).unwrap();
        let old_body: String = crate::storage::db(&f.private)
            .unwrap()
            .query_row("SELECT body FROM sessions", [], |r| r.get(0))
            .unwrap();
        std::fs::write(f.private.join("ai-profiles.json"), b"SECRET").unwrap();
        f.select("a");
        let a = session("Họp khách 中文");
        let la = create(&f.private, &a).unwrap();
        f.select("b");
        let b = session("Họp B");
        let lb = create(&f.private, &b).unwrap();
        assert!(la.folder.starts_with(f.base.join("a")));
        assert!(lb.folder.starts_with(f.base.join("b")));
        assert!(la
            .folder
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains("Họp khách 中文"));
        assert_eq!(list(&f.private).unwrap().len(), 3);
        let mut changed = a.clone();
        changed.notes = "Sửa tại A".into();
        crate::storage::save(&f.private, &changed).unwrap();
        assert_eq!(
            crate::storage::load(&f.private, &a.id).unwrap().notes,
            "Sửa tại A"
        );
        assert_eq!(
            old_body,
            crate::storage::db(&f.private)
                .unwrap()
                .query_row::<String, _, _>("SELECT body FROM sessions WHERE id=?1", [old.id], |r| r
                    .get(0))
                .unwrap()
        );
        assert_eq!(
            std::fs::read(f.private.join("ai-profiles.json")).unwrap(),
            b"SECRET"
        );
        assert!(!f.private.join("sessions").join(&a.id).exists());
        let gone = f.base.join("unplugged-a");
        std::fs::rename(f.base.join("a"), &gone).unwrap();
        assert_eq!(list(&f.private).unwrap().len(), 2);
        assert!(locate(&f.private, &a.id).is_err());
        assert!(!f.base.join("a").exists());
        std::fs::rename(gone, f.base.join("a")).unwrap();
    }
    #[test]
    fn names_collision_and_rename_crash_keep_uuid_and_audio() {
        let f = Fixture::new();
        f.select("a");
        let mut a = session("CON / 客户: Họp...");
        let first = create(&f.private, &a).unwrap();
        let b = session(&a.title);
        let second = create(&f.private, &b).unwrap();
        assert_ne!(first.folder, second.folder);
        assert!(second.folder.to_string_lossy().ends_with(" (2)"));
        std::fs::write(first.folder.join("recording.wav"), b"audio").unwrap();
        a.title = "Tên mới".into();
        save(&first, &a).unwrap();
        rename(&f.private, &a.id, true).unwrap();
        // A notes save during another job must not apply a pending physical rename.
        a.notes="Ghi chú khi đang xử lý".into();crate::storage::save(&f.private,&a).unwrap();
        assert!(first.folder.exists());
        rename(&f.private, &a.id, false).unwrap();
        let renamed = locate(&f.private, &a.id).unwrap();
        assert!(!first.folder.exists());
        assert_eq!(
            std::fs::read(renamed.folder.join("recording.wav")).unwrap(),
            b"audio"
        );
        let next = "2026-11-07_09-30-00 - Sau crash";
        let base = renamed.folder.parent().unwrap();
        let ops = base.join(".innote/operations");
        std::fs::create_dir_all(&ops).unwrap();
        paths::atomic_write(
            &ops.join(format!("{}.json", a.id)),
            &serde_json::to_vec(&serde_json::json!({"next":next})).unwrap(),
        )
        .unwrap();
        std::fs::rename(&renamed.folder, base.join(next)).unwrap();
        assert_eq!(locate(&f.private, &a.id).unwrap().folder, base.join(next));
        assert_eq!(
            load(&locate(&f.private, &a.id).unwrap(), &a.id).unwrap().id,
            a.id
        );
    }
    #[test]
    fn new_audio_finalizes_outside_private_root_and_recovers() {
        let f = Fixture::new();
        f.select("a");
        let mut s = session("Bản thu");
        s.status = "recording".into();
        let l = create(&f.private, &s).unwrap();
        let data: Vec<u8> = (0..16000)
            .flat_map(|i| ((i % 200) as i16 * 100).to_le_bytes())
            .collect();
        std::fs::write(l.capture().join("system.pcm"), data).unwrap();
        crate::storage::recover(&f.private).unwrap();
        let stored = crate::storage::load(&f.private, &s.id).unwrap();
        assert_eq!(stored.status, "interrupted");
        let wav = crate::storage::recording_path(&f.private, &s.id).unwrap();
        assert!(wav.is_file());
        assert!(!wav.starts_with(&f.private));
        assert_eq!(hound::WavReader::open(wav).unwrap().duration(), 16000);
        assert!(!l.capture().join("recording.wav").exists());
    }
    #[test]
    fn agent_bundle_separates_versions_excludes_private_fields_and_retries_locked_file() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        f.select("a");
        let mut s = session("Phân tích cuộc họp");
        s.speech = Some(crate::types::SpeechInfo {
            profile_id: Some("PROFILE_SENTINEL".into()),
            ..Default::default()
        });
        s.corrected = Some(crate::types::CorrectedTranscript {
            text: "Bản hiệu chỉnh".into(),
            segments: s.segments.clone(),
            provider: "local".into(),
            profile_id: "PROFILE_SENTINEL".into(),
            model: "moonshine-vi".into(),
            created: s.created.clone(),
            audio_sha256: "AUDIOHASH".into(),
        });
        s.summary = Some(crate::types::SummarizedTranscript {
            text: "Tóm tắt".into(),
            provider: "groq".into(),
            model: "test".into(),
            source: "original".into(),
            prompt: "PRIVATE_PROMPT_SENTINEL".into(),
            created: s.created.clone(),
            source_hash: "HASH".into(),
        });
        s.translations.push(crate::types::TranslationSegment {
            id: "t1".into(),
            source_id: "word1".into(),
            source_ids: vec!["word1".into()],
            start: 0.,
            end: 1.,
            original: s.segments[0].text.clone(),
            text: "The budget is 15 million, not 50 million.".into(),
            language: "en".into(),
            provider: "soniox".into(),
        });
        let l = create(&f.private, &s).unwrap();
        crate::meeting_bundle::publish(&f.private, &s.id).unwrap();
        let file = l.folder.join("meeting.json");
        let before = std::fs::read(&file).unwrap();
        let text = String::from_utf8(before.clone()).unwrap();
        for secret in [
            "PROFILE_SENTINEL",
            "PRIVATE_PROMPT_SENTINEL",
            "profile_id",
            "base_url",
        ] {
            assert!(!text.contains(secret));
        }
        let json: serde_json::Value = serde_json::from_slice(&before).unwrap();
        assert_eq!(json["original"]["segments"][0]["text"], s.segments[0].text);
        assert_eq!(json["translations"][0]["language"], "en");
        assert!(l.folder.join("original.md").exists());
        assert!(l.folder.join("corrected.md").exists());
        assert!(l.folder.join("translations/en.md").exists());
        let guide=std::fs::read_to_string(l.folder.join(crate::meeting_file_guide::MEETING_GUIDE)).unwrap();
        for label in ["BẢN CHÉP GỐC","BẢN HIỆU CHỈNH","BẢN GHI ÂM","CÁC BẢN DỊCH","Nguồn tóm tắt: Bản gốc","[ĐÃ CÓ] corrected.md","[CHƯA CÓ] recording.wav"]{assert!(guide.contains(label),"Missing guide explanation: {label}");}
        for secret in ["PROFILE_SENTINEL","PRIVATE_PROMPT_SENTINEL"]{assert!(!guide.contains(secret));}
        assert!(std::fs::read_to_string(l.folder.join("original.md")).unwrap().starts_with("> BẢN CHÉP GỐC"));
        assert!(std::fs::read_to_string(l.folder.join("corrected.md")).unwrap().starts_with("> BẢN HIỆU CHỈNH"));
        std::fs::write(l.folder.join("agent-analysis.md"), b"keep agent work").unwrap();
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&file)
            .unwrap();
        s.notes = "Cập nhật".into();
        save(&l, &s).unwrap();
        assert!(crate::meeting_bundle::publish(&f.private, &s.id).is_err());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        assert!(revision(&l, &s.id).unwrap().2.is_some());
        drop(locked);
        crate::meeting_bundle::publish(&f.private, &s.id).unwrap();
        let (r, p, e) = revision(&l, &s.id).unwrap();
        assert_eq!(r, p);
        assert!(e.is_none());
        assert_eq!(
            std::fs::read(l.folder.join("agent-analysis.md")).unwrap(),
            b"keep agent work"
        );
        assert_eq!(
            load(&l, &s.id).unwrap().segments[0].text,
            s.segments[0].text
        );
        s.summary = None;
        s.corrected = None;
        s.translations.clear();
        save(&l, &s).unwrap();
        crate::meeting_bundle::publish(&f.private, &s.id).unwrap();
        assert!(!l.folder.join("summary.md").exists());
        assert!(!l.folder.join("corrected.md").exists());
        assert!(!l.folder.join("translations/en.md").exists());
        let guide=std::fs::read_to_string(l.folder.join(crate::meeting_file_guide::MEETING_GUIDE)).unwrap();
        assert!(guide.contains("[CHƯA CÓ] corrected.md"));assert!(guide.contains("[CHƯA CÓ] summary.md"));
    }
    #[test]
    fn empty_meeting_guide_never_claims_missing_outputs_or_creates_fake_transcript() {
        let f=Fixture::new();f.select("a");let mut s=session("Chưa có lời nói");s.segments.clear();s.notes.clear();s.status="recording".into();
        let l=create(&f.private,&s).unwrap();crate::meeting_bundle::publish(&f.private,&s.id).unwrap();
        let guide=std::fs::read_to_string(l.folder.join(crate::meeting_file_guide::MEETING_GUIDE)).unwrap();
        for name in ["original.md","original.srt","corrected.md","summary.md","notes.md","recording.wav"]{assert!(guide.contains(&format!("[CHƯA CÓ] {name}")));assert!(!l.folder.join(name).exists());}
        assert!(guide.contains("Trạng thái trong Innote: Đang ghi"));
        let manifest:serde_json::Value=serde_json::from_slice(&std::fs::read(l.folder.join("meeting.json")).unwrap()).unwrap();assert!(manifest["manifest"][crate::meeting_file_guide::MEETING_GUIDE].is_string());
    }
    #[test]
    fn installing_missing_guide_does_not_rewrite_existing_meeting_files() {
        let f=Fixture::new();f.select("a");let s=session("Cũ có file người dùng sửa");let l=create(&f.private,&s).unwrap();
        std::fs::write(l.folder.join("original.md"),"Bản người dùng đã sửa riêng").unwrap();
        crate::meeting_file_guide::ensure_meeting_guide(&l.folder,&s).unwrap();
        assert_eq!(std::fs::read_to_string(l.folder.join("original.md")).unwrap(),"Bản người dùng đã sửa riêng");
        assert!(l.folder.join(crate::meeting_file_guide::MEETING_GUIDE).is_file());
    }
    #[test]
    fn external_folder_rename_rescans_without_importing_agent_content() {
        let f = Fixture::new();
        f.select("a");
        let s = session("Ban đầu");
        let l = create(&f.private, &s).unwrap();
        let moved = l.folder.parent().unwrap().join("Đổi tên bằng Explorer");
        std::fs::rename(&l.folder, &moved).unwrap();
        std::fs::write(
            moved.join("original.md"),
            b"agent changed this file, not canonical",
        )
        .unwrap();
        assert_eq!(rescan(&f.private).unwrap(), 1);
        assert_eq!(locate(&f.private, &s.id).unwrap().folder, moved);
        assert_eq!(
            crate::storage::load(&f.private, &s.id).unwrap().segments[0].text,
            s.segments[0].text
        );
    }
    #[test]
    fn substituted_drive_marker_cannot_create_or_overwrite_wrong_library() {
        let f = Fixture::new();
        let l = f.select("a");
        let s = session("Cũ");
        create(&f.private, &s).unwrap();
        paths::atomic_write(&l.root.join(".innote/library.json"),&serde_json::to_vec(&serde_json::json!({"schema_version":1,"library_id":uuid::Uuid::new_v4().to_string()})).unwrap()).unwrap();
        assert!(create(&f.private, &session("Không được ghi")).is_err());
        assert!(locate(&f.private, &s.id).is_err());
        assert!(list(&f.private).unwrap().is_empty());
    }
}
