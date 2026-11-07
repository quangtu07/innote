//! Private registry and validated meeting library roots. This module never loads credentials.
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct LibraryRef {
    pub id: String,
    pub root: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageRegistry {
    pub schema_version: u32,
    pub default_library_id: Option<String>,
    pub libraries: Vec<LibraryRef>,
}
impl Default for StorageRegistry {
    fn default() -> Self {
        Self {
            schema_version: 1,
            default_library_id: None,
            libraries: vec![],
        }
    }
}
pub struct ValidatedRoot {
    path: PathBuf,
}
pub fn available_bytes(root: &Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    let wide: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0;
    unsafe {
        windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            windows::core::PCWSTR(wide.as_ptr()),
            Some(&mut free),
            None,
            None,
        )?;
    }
    Ok(free)
}

pub fn load_registry(private_root: &Path) -> Result<StorageRegistry> {
    let path = private_root.join("meeting-storage.json");
    if !path
        .try_exists()
        .context("Không kiểm tra được cấu hình nơi lưu cuộc họp")?
    {
        return Ok(StorageRegistry::default());
    }
    let registry: StorageRegistry = serde_json::from_slice(&std::fs::read(path)?)
        .context("Không đọc được cấu hình nơi lưu cuộc họp; giữ nguyên tệp để phục hồi")?;
    registry.validate()?;
    Ok(registry)
}
impl StorageRegistry {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.schema_version == 1,
            "Phiên bản cấu hình lưu trữ chưa được hỗ trợ"
        );
        let mut ids = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for library in &self.libraries {
            uuid::Uuid::parse_str(&library.id).context("Mã thư viện không hợp lệ")?;
            anyhow::ensure!(
                library.root.is_absolute(),
                "Đường dẫn thư viện phải tuyệt đối"
            );
            anyhow::ensure!(
                ids.insert(&library.id) && paths.insert(path_key(&library.root)),
                "Thư viện bị trùng; không tự gộp dữ liệu"
            );
        }
        anyhow::ensure!(
            self.default_library_id
                .as_ref()
                .map_or(true, |id| ids.contains(id)),
            "Không tìm thấy thư viện mặc định trong cấu hình"
        );
        Ok(())
    }
    pub fn select(&mut self, library: LibraryRef) -> Result<()> {
        let mut next = self.clone();
        if let Some(old) = next
            .libraries
            .iter_mut()
            .find(|r| r.id == library.id || path_key(&r.root) == path_key(&library.root))
        {
            if old.id == library.id
                && path_key(&old.root) != path_key(&library.root)
                && !old.root.try_exists()?
            {
                old.root = library.root.clone();
            }
            anyhow::ensure!(
                old.id == library.id && path_key(&old.root) == path_key(&library.root),
                "Thư viện trùng ID hoặc đường dẫn; cần liên kết lại có xác nhận"
            );
        } else {
            anyhow::ensure!(
                !next.libraries.iter().any(|l| contains_path(
                    &path_key(&l.root),
                    &path_key(&library.root)
                ) || contains_path(
                    &path_key(&library.root),
                    &path_key(&l.root)
                )),
                "Các thư viện phải ở những thư mục riêng, không nằm lồng nhau"
            );
            next.libraries.push(library.clone());
        }
        next.default_library_id = Some(library.id);
        next.validate()?;
        *self = next;
        Ok(())
    }
}
pub fn save_registry(private_root: &Path, registry: &StorageRegistry) -> Result<()> {
    load_registry(private_root)?; // A corrupt registry must never be replaced with defaults.
    registry.validate()?;
    atomic_write(
        &private_root.join("meeting-storage.json"),
        &serde_json::to_vec_pretty(registry)?,
    )
}
pub fn validate_library_root(
    candidate: &Path,
    private_root: &Path,
    runtime_root: &Path,
) -> Result<ValidatedRoot> {
    anyhow::ensure!(
        candidate.is_absolute() && candidate.is_dir(),
        "Thư mục lưu không tồn tại hoặc ổ đĩa chưa kết nối"
    );
    reject_reparse(candidate)?;
    let path = candidate.canonicalize()?;
    for protected in [private_root, runtime_root] {
        let protected = protected
            .canonicalize()
            .context("Không xác định được thư mục riêng của ứng dụng")?;
        let a = path_key(&path);
        let b = path_key(&protected);
        anyhow::ensure!(
            !contains_path(&a, &b) && !contains_path(&b, &a),
            "Chọn thư mục cuộc họp riêng, không nằm trong hoặc chứa cấu hình/bộ cài Innote"
        );
    }
    let text = path.to_string_lossy();
    let network =
        text.starts_with(r"\\?\UNC\") || (text.starts_with(r"\\") && !text.starts_with(r"\\?\"));
    anyhow::ensure!(!network, "Chưa hỗ trợ thư viện trên ổ mạng");
    let prefix = path
        .components()
        .next()
        .context("Thiếu ổ đĩa")?
        .as_os_str()
        .to_string_lossy()
        .to_string();
    let drive = format!("{}\\", prefix.trim_end_matches('\\'));
    let wide: Vec<u16> = drive.encode_utf16().chain(Some(0)).collect();
    let kind = unsafe {
        windows::Win32::Storage::FileSystem::GetDriveTypeW(windows::core::PCWSTR(wide.as_ptr()))
    };
    anyhow::ensure!(
        kind == 2 || kind == 3,
        "Chỉ hỗ trợ ổ local hoặc ổ rời đang kết nối"
    );
    anyhow::ensure!(
        text.encode_utf16().count() + 100 <= 240,
        "Đường dẫn nơi lưu quá dài; hãy chọn thư mục ngắn hơn"
    );
    let probe = path.join(format!(".innote-probe-{}", uuid::Uuid::new_v4()));
    let moved = probe.with_extension("renamed");
    let result = (|| -> Result<()> {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)?;
        file.write_all(b"Innote storage probe")?;
        file.sync_all()?;
        drop(file);
        anyhow::ensure!(
            std::fs::read(&probe)? == b"Innote storage probe",
            "Không đọc được dữ liệu thử"
        );
        std::fs::rename(&probe, &moved)?;
        std::fs::remove_file(&moved)?;
        Ok(())
    })();
    let _ = std::fs::remove_file(&probe);
    let _ = std::fs::remove_file(&moved);
    result.context("Không thể tạo/đọc/đổi tên tệp tại nơi lưu đã chọn")?;
    Ok(ValidatedRoot { path })
}
pub fn open_or_create_library(root: &ValidatedRoot) -> Result<LibraryRef> {
    reject_reparse(&root.path)?;
    let internal = root.path.join(".innote");
    std::fs::create_dir_all(&internal)?;
    reject_reparse(&internal)?;
    let _lease = acquire_library_writer(&root.path)?;
    let marker = internal.join("library.json");
    if marker.try_exists()? {
        reject_reparse(&marker)?;
    }
    let metadata: LibraryMarker = if marker.try_exists()? {
        serde_json::from_slice(&std::fs::read(&marker)?)
            .context("Không đọc được nhận diện thư viện; không tạo đè")?
    } else {
        anyhow::ensure!(
            !internal.join("meetings.sqlite").try_exists()?,
            "Thư viện thiếu nhận diện; cần phục hồi, không tạo đè"
        );
        let metadata = LibraryMarker {
            schema_version: 1,
            library_id: uuid::Uuid::new_v4().to_string(),
        };
        atomic_write(&marker, &serde_json::to_vec_pretty(&metadata)?)?;
        metadata
    };
    anyhow::ensure!(
        metadata.schema_version == 1,
        "Phiên bản thư viện chưa được hỗ trợ"
    );
    uuid::Uuid::parse_str(&metadata.library_id)?;
    Ok(LibraryRef {
        id: metadata.library_id,
        root: root.path.clone(),
    })
}

#[derive(Serialize, Deserialize)]
struct LibraryMarker {
    schema_version: u32,
    library_id: String,
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy()
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase()
}
fn contains_path(parent: &str, child: &str) -> bool {
    parent == child || child.starts_with(&format!("{parent}\\"))
}

pub fn reject_reparse(path: &Path) -> Result<()> {
    use std::os::windows::fs::MetadataExt;
    for ancestor in path.ancestors() {
        let metadata = std::fs::symlink_metadata(ancestor)?;
        anyhow::ensure!(
            metadata.file_attributes() & 0x400 == 0,
            "Không dùng symlink/junction cho nơi lưu cuộc họp"
        );
    }
    Ok(())
}

/// OS-held file handle: released on process exit, including crashes.
pub fn acquire_library_writer(root: &Path) -> Result<std::fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    let internal = root.join(".innote");
    reject_reparse(&internal)?;
    if internal.join("writer.lock").exists() {
        reject_reparse(&internal.join("writer.lock"))?;
    }
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(internal.join("writer.lock"))
        .context("Thư viện đang được một tiến trình khác sử dụng hoặc không thể ghi")
}

pub fn atomic_write(target: &Path, bytes: &[u8]) -> Result<()> {
    use std::{io::Write, os::windows::ffi::OsStrExt};
    use windows::{
        core::PCWSTR,
        Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        },
    };
    let parent = target.parent().context("Thiếu thư mục đích")?;
    reject_reparse(parent)?;
    let temporary = parent.join(format!(".innote-write-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
        unsafe {
            MoveFileExW(
                PCWSTR(from.as_ptr()),
                PCWSTR(to.as_ptr()),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("innote-storage-{}", uuid::Uuid::new_v4()));
            for name in ["private", "runtime", "a", "b"] {
                std::fs::create_dir_all(path.join(name)).unwrap();
            }
            Self(path)
        }
        fn root(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn library(&self, name: &str) -> LibraryRef {
            open_or_create_library(
                &validate_library_root(
                    &self.root(name),
                    &self.root("private"),
                    &self.root("runtime"),
                )
                .unwrap(),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn registry_restart_keeps_default_and_previous_roots() {
        let f = Fixture::new();
        let mut registry = StorageRegistry::default();
        let a = f.library("a");
        let b = f.library("b");
        registry.select(a.clone()).unwrap();
        registry.select(b.clone()).unwrap();
        save_registry(&f.root("private"), &registry).unwrap();
        let read = load_registry(&f.root("private")).unwrap();
        assert_eq!(read.default_library_id, Some(b.id));
        assert_eq!(read.libraries.len(), 2);
        assert!(read.libraries.contains(&a));
    }
    #[test]
    fn missing_root_does_not_create_database() {
        let f = Fixture::new();
        let missing = f.root("missing");
        assert!(validate_library_root(&missing, &f.root("private"), &f.root("runtime")).is_err());
        assert!(!missing.exists());
        assert!(!f.root("a/meetings.sqlite").exists());
        assert_eq!(
            load_registry(&f.root("private")).unwrap(),
            StorageRegistry::default()
        );
    }
    #[test]
    fn reject_private_overlap_and_runtime() {
        let f = Fixture::new();
        for candidate in [&f.0, &f.root("private"), &f.root("runtime")] {
            assert!(
                validate_library_root(candidate, &f.root("private"), &f.root("runtime")).is_err()
            );
        }
    }
    #[test]
    fn duplicate_library_identity_is_not_merged() {
        let f = Fixture::new();
        let a = f.library("a");
        let mut registry = StorageRegistry::default();
        registry.select(a.clone()).unwrap();
        let before = registry.clone();
        assert!(registry
            .select(LibraryRef {
                id: a.id,
                root: f.root("b")
            })
            .is_err());
        assert_eq!(before, registry);
    }
    #[test]
    fn corrupt_registry_is_never_overwritten() {
        let f = Fixture::new();
        let path = f.root("private/meeting-storage.json");
        std::fs::write(&path, b"broken keep me").unwrap();
        assert!(load_registry(&f.root("private")).is_err());
        assert!(save_registry(&f.root("private"), &StorageRegistry::default()).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken keep me");
    }
    #[test]
    fn marker_is_stable_and_not_a_database() {
        let f = Fixture::new();
        let first = f.library("a");
        assert_eq!(first, f.library("a"));
        assert!(!f.root("a/.innote/meetings.sqlite").exists());
        assert!(!f.root("a/ai-profiles.json").exists());
    }
    #[test]
    fn unreadable_registry_does_not_become_empty_default() {
        let f = Fixture::new();
        // A directory at the registry filename is an I/O error, not a missing registry.
        let path = f.root("private/meeting-storage.json");
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("keep"), b"recovery evidence").unwrap();
        assert!(load_registry(&f.root("private")).is_err());
        assert!(save_registry(&f.root("private"), &StorageRegistry::default()).is_err());
        assert_eq!(
            std::fs::read(path.join("keep")).unwrap(),
            b"recovery evidence"
        );
    }
    #[test]
    fn rejected_long_root_creates_no_probe_or_library() {
        let f = Fixture::new();
        let path = f.root("a").join("x".repeat(120));
        std::fs::create_dir(&path).unwrap();
        assert!(validate_library_root(&path, &f.root("private"), &f.root("runtime")).is_err());
        assert_eq!(std::fs::read_dir(&path).unwrap().count(), 0);
    }
    #[test]
    fn unicode_root_preserves_name_and_does_not_copy_private_files() {
        let f = Fixture::new();
        let name = "Họp khách hàng 中文";
        std::fs::create_dir(f.root(name)).unwrap();
        std::fs::write(f.root("private/ai-profiles.json"), b"PRIVATE SENTINEL").unwrap();
        let library = f.library(name);
        assert_eq!(
            library.root.file_name().unwrap(),
            std::ffi::OsStr::new(name)
        );
        assert!(!library.root.join("ai-profiles.json").exists());
        assert_eq!(
            std::fs::read(f.root("private/ai-profiles.json")).unwrap(),
            b"PRIVATE SENTINEL"
        );
        assert_eq!(std::fs::read_dir(library.root).unwrap().count(), 1);
    }
    #[test]
    fn writer_lease_is_exclusive_and_released() {
        if let Some(root) = std::env::var_os("INNOTE_STORAGE_LEASE_TEST_ROOT") {
            let expected = std::env::var("INNOTE_STORAGE_LEASE_TEST_BUSY").unwrap() == "1";
            assert_eq!(acquire_library_writer(Path::new(&root)).is_err(), expected);
            return;
        }
        let f = Fixture::new();
        let library = f.library("a");
        let first = acquire_library_writer(&library.root).unwrap();
        assert!(acquire_library_writer(&library.root).is_err());
        assert!(acquire_library_writer(&f.library("b").root).is_ok());
        let child = |busy: bool| {
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "storage_paths::tests::writer_lease_is_exclusive_and_released",
                ])
                .env("INNOTE_STORAGE_LEASE_TEST_ROOT", &library.root)
                .env(
                    "INNOTE_STORAGE_LEASE_TEST_BUSY",
                    if busy { "1" } else { "0" },
                )
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
        };
        child(true);
        drop(first);
        child(false);
        assert!(acquire_library_writer(&library.root).is_ok());
    }
    #[test]
    fn locked_registry_preserves_bytes_on_failed_replace() {
        use std::os::windows::fs::OpenOptionsExt;
        let f = Fixture::new();
        let mut registry = StorageRegistry::default();
        registry.select(f.library("a")).unwrap();
        save_registry(&f.root("private"), &registry).unwrap();
        let path = f.root("private/meeting-storage.json");
        let before = std::fs::read(&path).unwrap();
        let locked = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&path)
            .unwrap();
        registry.select(f.library("b")).unwrap();
        assert!(save_registry(&f.root("private"), &registry).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        drop(locked);
        save_registry(&f.root("private"), &registry).unwrap();
        assert_eq!(load_registry(&f.root("private")).unwrap(), registry);
        assert_eq!(std::fs::read_dir(f.root("private")).unwrap().count(), 1);
    }
    #[test]
    fn unknown_schema_and_dangling_default_rejected() {
        let mut registry = StorageRegistry::default();
        registry.schema_version = 2;
        assert!(registry.validate().is_err());
        registry.schema_version = 1;
        registry.default_library_id = Some(uuid::Uuid::new_v4().to_string());
        assert!(registry.validate().is_err());
    }
    #[test]
    fn corrupted_marker_and_orphan_database_are_not_recreated() {
        let f = Fixture::new();
        f.library("a");
        let marker = f.root("a/.innote/library.json");
        std::fs::write(&marker, b"broken").unwrap();
        let valid =
            validate_library_root(&f.root("a"), &f.root("private"), &f.root("runtime")).unwrap();
        assert!(open_or_create_library(&valid).is_err());
        assert_eq!(std::fs::read(&marker).unwrap(), b"broken");
        std::fs::remove_file(&marker).unwrap();
        std::fs::write(f.root("a/.innote/meetings.sqlite"), b"keep").unwrap();
        assert!(open_or_create_library(&valid).is_err());
        assert!(!marker.exists());
    }
    #[test]
    fn internal_junction_is_rejected() {
        let f = Fixture::new();
        let status = std::process::Command::new("cmd")
            .args(["/c", "mklink", "/J"])
            .arg(f.root("a/.innote").to_string_lossy().replace('/', "\\"))
            .arg(f.root("private"))
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "mklink: {} {}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
        let valid =
            validate_library_root(&f.root("a"), &f.root("private"), &f.root("runtime")).unwrap();
        assert!(open_or_create_library(&valid).is_err());
        assert!(!f.root("private/library.json").exists());
        std::fs::remove_dir(f.root("a/.innote")).unwrap();
    }
}
