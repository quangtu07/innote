use crate::types::Session;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::{Path, PathBuf};

pub const DATA_DIR_NAME: &str = "Innote";
const LEGACY_DATA_DIR_NAME: &str = "Thư ký Mây";

fn migrate_local_root(local: &Path) -> PathBuf {
    let next = local.join(DATA_DIR_NAME);
    if next.exists() {
        return next;
    }
    let legacy = local.join(LEGACY_DATA_DIR_NAME);
    if legacy.exists() {
        // Keep the original folder as a fallback if another instance still has
        // files open; the next launch can retry the fast same-volume rename.
        if std::fs::rename(&legacy, &next).is_ok() {
            return next;
        }
        return legacy;
    }
    next
}

pub fn root() -> PathBuf {
    std::env::var_os("MAY_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
            migrate_local_root(&local)
        })
}
pub fn db(root: &Path) -> Result<Connection> {
    std::fs::create_dir_all(root)?;
    let c = Connection::open(root.join("meetings.sqlite"))?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    c.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS sessions(id TEXT PRIMARY KEY, body TEXT NOT NULL);")?;
    Ok(c)
}
pub fn save(root: &Path, s: &Session) -> Result<()> {
    db(root)?.execute("INSERT INTO sessions(id,body) VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET body=excluded.body", params![s.id, serde_json::to_string(s)?])?;
    Ok(())
}
fn drop_stale_discontinuity_warnings(s: &mut Session) {
    // The legacy message did not say whether samples were actually missing and was
    // raised for harmless WASAPI endpoint flags. Keep it only on unfinished
    // sessions, where the user may still need to resume/check the device.
    if s.status == "completed" && s.processed + 0.01 >= s.duration {
        s.warnings.retain(|w| {
            !(w.starts_with("Nguồn system báo gián đoạn dữ liệu âm thanh.")
                || w.starts_with("Nguồn microphone báo gián đoạn dữ liệu âm thanh."))
        });
    }
}
fn normalize_legacy_resume_text(s: &mut Session) {
    if let Some(error) = s.error.as_mut() {
        if error.to_lowercase().contains("chép tiếp") {
            *error = "Phiên đã dừng. Bản thu và bản chép hiện có được giữ trên máy.".into();
        }
    }
}
pub fn load(root: &Path, id: &str) -> Result<Session> {
    ensure_id(id)?;
    let text: String = db(root)?
        .query_row("SELECT body FROM sessions WHERE id=?1", [id], |r| r.get(0))
        .context("Không tìm thấy cuộc họp")?;
    let mut s: Session = serde_json::from_str(&text)?;
    drop_stale_discontinuity_warnings(&mut s);
    normalize_legacy_resume_text(&mut s);
    Ok(s)
}
pub fn list(root: &Path) -> Result<Vec<Session>> {
    let c = db(root)?;
    let mut q = c.prepare("SELECT body FROM sessions")?;
    let rows = q.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let mut s: Session = serde_json::from_str(&row?)?;
        drop_stale_discontinuity_warnings(&mut s);
        normalize_legacy_resume_text(&mut s);
        out.push(s);
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(out)
}
pub fn ensure_id(id: &str) -> Result<()> {
    uuid::Uuid::parse_str(id).context("Mã cuộc họp không hợp lệ")?;
    Ok(())
}
pub fn session_dir(root: &Path, id: &str) -> Result<PathBuf> {
    ensure_id(id)?;
    Ok(root.join("sessions").join(id))
}
pub fn recover(root: &Path) -> Result<()> {
    for mut s in list(root)? {
        if ["recording", "paused", "processing", "stopping"].contains(&s.status.as_str()) {
            s.status = "interrupted".into();
            if let Ok(dir) = session_dir(root, &s.id) {
                for name in ["system.pcm", "microphone.pcm"] {
                    if let Ok(meta) = std::fs::metadata(dir.join(name)) {
                        s.duration = s.duration.max(meta.len() as f64 / 32000.);
                    }
                }
                if dir.join("system.pcm").exists() || dir.join("microphone.pcm").exists() {
                    if let Err(e) = crate::audio::finalize(&dir, s.duration) {
                        s.warnings.push(format!(
                            "Chưa phục hồi được file nghe lại: {e}. PCM vẫn được giữ."
                        ));
                    }
                }
            }
            s.error =
                Some("Phiên trước bị gián đoạn. Bản thu và bản chép hiện có vẫn được giữ.".into());
            save(root, &s)?;
        }
    }
    Ok(())
}

pub fn stamp(seconds: f64, srt: bool) -> String {
    let ms = (seconds.max(0.) * 1000.).round() as u64;
    if srt {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            ms / 3600000,
            ms / 60000 % 60,
            ms / 1000 % 60,
            ms % 1000
        )
    } else {
        format!(
            "{:02}:{:02}:{:02}",
            ms / 3600000,
            ms / 60000 % 60,
            ms / 1000 % 60
        )
    }
}
pub fn export(s: &Session, format: &str) -> String {
    // Include durable streaming words if refinement failed or is still behind.
    let mut effective = s.clone();
    let words: Vec<_> = s
        .live
        .iter()
        .filter(|w| w.end > s.processed + 0.00001)
        .collect();
    if let (Some(first), Some(last)) = (words.first(), words.last()) {
        effective.segments.push(crate::types::Segment {
            id: "stream-preview".into(),
            start: first.start,
            end: last.end,
            text: format!(
                "[Chép trực tiếp, chưa hiệu chỉnh] {}",
                words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            edited: false,
        });
    }
    let s = &effective;
    if format == "srt" {
        return s
            .segments
            .iter()
            .enumerate()
            .map(|(i, x)| {
                format!(
                    "{}\n{} --> {}\n{}\n\n",
                    i + 1,
                    stamp(x.start, true),
                    stamp(x.end, true),
                    x.text
                )
            })
            .collect();
    }
    let mut out = if format == "md" {
        format!("# {}\n\n", s.title)
    } else {
        format!("{}\n\n", s.title)
    };
    if s.status != "completed" {
        out.push_str("BẢN CHƯA HOÀN TẤT — có thể còn nội dung chưa chép.\n\n");
    }
    for x in &s.segments {
        out.push_str(&format!("[{}] {}\n\n", stamp(x.start, false), x.text));
    }
    if !s.notes.is_empty() {
        out.push_str(&format!("\nGhi chú\n{}\n", s.notes));
    }
    if format == "md" {
        out.push_str(
            "\n---\n\n## Liên hệ tư vấn, hỗ trợ & lập trình\n\n\
**Alex Linh**\n\n\
- Zalo: **0962 239 818** — [zalo.me/0962239818](https://zalo.me/0962239818)\n\
- Website: [alexlinh.io.vn](https://alexlinh.io.vn/)\n",
        );
    } else {
        out.push_str(
            "\n----------------------------------------\n\
LIÊN HỆ TƯ VẤN, HỖ TRỢ & LẬP TRÌNH\n\
Alex Linh\n\
Zalo: 0962 239 818\n\
Website: https://alexlinh.io.vn/\n",
        );
    }
    out
}
pub fn export_file(path: &Path, s: &Session, format: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        ["md", "txt", "srt"].contains(&format),
        "Định dạng xuất không hợp lệ"
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(path)?;
    use std::io::Write;
    file.write_all(export(s, format).as_bytes())?;
    file.sync_all()?;
    Ok(path.to_path_buf())
}
pub fn export_to(directory: &Path, s: &Session, format: &str) -> Result<PathBuf> {
    anyhow::ensure!(
        ["md", "txt", "srt"].contains(&format),
        "Định dạng xuất không hợp lệ"
    );
    std::fs::create_dir_all(directory)?;
    let clean: String = s
        .title
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(80)
        .collect();
    let clean = clean.trim().trim_end_matches(['.', ' ']);
    let clean = if clean.is_empty() { "Cuoc-hop" } else { clean };
    let partial = if s.status == "completed" {
        ""
    } else {
        "-chua-hoan-tat"
    };
    let name = format!(
        "{}-{}-{}{}.{}",
        clean,
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        &uuid::Uuid::new_v4().to_string()[..6],
        partial,
        format
    );
    let path = directory.join(name);
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    file.write_all(export(s, format).as_bytes())?;
    file.sync_all()?;
    Ok(path)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn migrates_legacy_data_directory_into_innote() {
        let base = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let legacy = base.join(LEGACY_DATA_DIR_NAME);
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join("marker.txt"), b"keep").unwrap();
        let migrated = migrate_local_root(&base);
        assert_eq!(migrated, base.join(DATA_DIR_NAME));
        assert_eq!(std::fs::read(migrated.join("marker.txt")).unwrap(), b"keep");
        assert!(!legacy.exists());
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn existing_innote_directory_wins_over_legacy_folder() {
        let base = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(base.join(DATA_DIR_NAME)).unwrap();
        std::fs::create_dir_all(base.join(LEGACY_DATA_DIR_NAME)).unwrap();
        assert_eq!(migrate_local_root(&base), base.join(DATA_DIR_NAME));
        std::fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn unrefined_live_words_survive_recovery_and_all_export_formats() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let s:Session=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"title":"Live","created":"now","duration":5.,"status":"interrupted","language":"vi","model":"base-q5_1","processed":2.,"segments":[{"id":"final","start":0.,"end":2.,"text":"Đã sửa","edited":true}],"live":[{"id":"old","start":1.,"end":2.,"text":"Không lặp"},{"id":"new","start":3.,"end":4.,"text":"Giữ trực tiếp"}],"notes":"","error":null})).unwrap();
        save(&dir, &s).unwrap();
        let recovered = load(&dir, &s.id).unwrap();
        for format in ["md", "txt", "srt"] {
            let text = export(&recovered, format);
            assert!(text.contains("Đã sửa"));
            assert!(text.contains("Giữ trực tiếp"));
            assert!(!text.contains("Không lặp"));
            assert!(text.contains("chưa hiệu chỉnh"));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn markdown_and_txt_exports_end_with_author_contact() {
        let s:Session=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"title":"Liên hệ","created":"2026-10-29","duration":1.,"status":"completed","language":"vi","model":"base-q5_1","processed":1.,"segments":[{"id":"one","start":0.,"end":1.,"text":"Nội dung.","edited":true}],"notes":"Ghi chú cuối.","error":null,"warnings":[]})).unwrap();
        let md = export(&s, "md");
        assert!(md.contains("## Liên hệ tư vấn, hỗ trợ & lập trình"));
        assert!(md.contains("Alex Linh"));
        assert!(md.contains("Zalo: **0962 239 818**"));
        assert!(md.contains("[alexlinh.io.vn](https://alexlinh.io.vn/)"));
        assert!(md.trim_end().ends_with("[alexlinh.io.vn](https://alexlinh.io.vn/)"));
        assert!(md.find("Ghi chú cuối.").unwrap() < md.find("## Liên hệ").unwrap());
        let txt = export(&s, "txt");
        assert!(txt.contains("LIÊN HỆ TƯ VẤN, HỖ TRỢ & LẬP TRÌNH"));
        assert!(txt.contains("Alex Linh"));
        assert!(txt.contains("Zalo: 0962 239 818"));
        assert!(txt.trim_end().ends_with("Website: https://alexlinh.io.vn/"));
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let path = dir.join("lien-he.md");
        export_file(&path, &s, "md").unwrap();
        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.trim_end().ends_with("[alexlinh.io.vn](https://alexlinh.io.vn/)"));
        std::fs::remove_dir_all(dir).unwrap();
        assert!(!export(&s, "srt").contains("0962 239 818"));
    }
    #[test]
    fn time_export_crosses_hour() {
        assert_eq!(stamp(3661.234, true), "01:01:01,234");
        assert_eq!(stamp(-1., false), "00:00:00");
    }
    #[test]
    fn rejects_path_traversal() {
        assert!(ensure_id("../../models").is_err());
    }
    #[test]
    fn exports_unicode_without_overwriting_previous_files() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let s:Session=serde_json::from_value(serde_json::json!({"id":uuid::Uuid::new_v4().to_string(),"title":"Mây: cuộc họp/nháp","created":"2026-10-24","duration":1.,"status":"interrupted","language":"vi","model":"base-q5_1","processed":1.,"segments":[{"id":"one","start":0.,"end":1.,"text":"Đã chốt đầu việc.","edited":true}],"notes":"Gửi tài liệu.","error":null,"warnings":[]})).unwrap();
        let first = export_to(&dir, &s, "md").unwrap();
        let second = export_to(&dir, &s, "md").unwrap();
        assert_ne!(first, second);
        assert_eq!(first.parent(), Some(dir.as_path()));
        let text = std::fs::read_to_string(first).unwrap();
        assert!(text.contains("Đã chốt đầu việc."));
        assert!(text.contains("CHƯA HOÀN TẤT"));
        assert!(export_to(&dir, &s, "exe").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn unicode_roundtrip() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let s = Session {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Họp với Mây".into(),
            created: "now".into(),
            duration: 2.,
            status: "completed".into(),
            language: "multi".into(),
            model: crate::models::NEMO_ID.into(),
            provider: "local".into(),
            processed: 2.,
            segments: vec![],
            live: vec![],
            notes: "Đã chốt · Agreed · 합의했습니다 · 已确认".into(),
            error: None,
            warnings: vec![],
            smoothed: None,
        };
        save(&dir, &s).unwrap();
        let mut loaded = load(&dir, &s.id).unwrap();
        assert_eq!(loaded.notes, s.notes);
        loaded.segments.push(crate::types::Segment {
            id: "four-scripts".into(),
            start: 0.,
            end: 2.,
            text: "Xin chào. Hello. 안녕하세요. 大家好。".into(),
            edited: true,
        });
        save(&dir, &loaded).unwrap();
        let restored = load(&dir, &s.id).unwrap();
        for format in ["txt", "md", "srt"] {
            let path = export_to(&dir, &restored, format).unwrap();
            let text = std::fs::read_to_string(path).unwrap();
            assert!(text.contains("안녕하세요. 大家好。"));
        }
        assert!(restored.segments[0].edited);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn recovers_audio_after_abrupt_exit_without_losing_edits() {
        let dir = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let id = uuid::Uuid::new_v4().to_string();
        let audio = session_dir(&dir, &id).unwrap();
        std::fs::create_dir_all(&audio).unwrap();
        std::fs::File::create(audio.join("system.pcm"))
            .unwrap()
            .set_len(96000)
            .unwrap();
        let s = Session {
            id: id.clone(),
            title: "Phiên gián đoạn".into(),
            created: "2026-10-24".into(),
            duration: 1.,
            status: "recording".into(),
            language: "vi".into(),
            model: "base-q5_1".into(),
            provider: "local".into(),
            processed: 1.,
            segments: vec![crate::types::Segment {
                id: "segment".into(),
                start: 0.,
                end: 1.,
                text: "Đã sửa đúng".into(),
                edited: true,
            }],
            live: vec![],
            notes: "Giữ ghi chú".into(),
            error: None,
            warnings: vec![],
            smoothed: None,
        };
        save(&dir, &s).unwrap();
        recover(&dir).unwrap();
        let restored = load(&dir, &id).unwrap();
        assert_eq!(restored.status, "interrupted");
        assert_eq!(restored.duration, 3.);
        assert_eq!(restored.processed, 1.);
        assert!(restored.segments[0].edited);
        assert_eq!(restored.notes, "Giữ ghi chú");
        assert_eq!(
            hound::WavReader::open(audio.join("recording.wav"))
                .unwrap()
                .duration(),
            48000
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
#[test]
    fn completed_session_drops_only_legacy_discontinuity_warnings() {
        let mut s: Session = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "title": "QA", "created": "now",
            "duration": 10., "status": "completed", "language": "vi", "model": "base-q5_1",
            "processed": 10., "segments": [], "notes": "", "error": null,
            "warnings": [
                "Nguồn system báo gián đoạn dữ liệu âm thanh.",
                "Nguồn microphone báo gián đoạn dữ liệu âm thanh.",
                "Thiết bị âm thanh tạm ngắt; đã mở lại."
            ]
        }))
        .unwrap();
        drop_stale_discontinuity_warnings(&mut s);
        assert_eq!(s.warnings, vec!["Thiết bị âm thanh tạm ngắt; đã mở lại."]);
        s.status = "interrupted".into();
        s.processed = 5.;
        s.warnings.push("Nguồn system báo gián đoạn dữ liệu âm thanh.".into());
        drop_stale_discontinuity_warnings(&mut s);
        assert_eq!(s.warnings.len(), 2);
    }
}
