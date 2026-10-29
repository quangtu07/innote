use anyhow::Result;
use sha2::{Digest, Sha256};
use std::path::Path;
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE},
        System::Threading::CreateMutexW,
    },
};

/// Held before recovery: another GUI must never recover a live recording.
/// Windows closes this handle on crashes, so no stale lock file survives.
pub struct Instance(HANDLE);
impl Instance {
    pub fn acquire(root: &Path) -> Result<Option<Self>> {
        std::fs::create_dir_all(root)?;
        let normalized = std::fs::canonicalize(root)?
            .to_string_lossy()
            .replace('/', "\\")
            .to_lowercase();
        let digest = hex::encode(Sha256::digest(normalized.as_bytes()));
        let name: Vec<u16> = format!("Local\\vn.thukymay.{digest}\0")
            .encode_utf16()
            .collect();
        unsafe {
            let handle = CreateMutexW(None, false, PCWSTR(name.as_ptr()))?;
            if GetLastError() == ERROR_ALREADY_EXISTS {
                let _ = CloseHandle(handle);
                return Ok(None);
            }
            Ok(Some(Self(handle)))
        }
    }
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_root_is_exclusive_and_releases_after_exit() {
        let root = std::env::temp_dir().join(uuid::Uuid::new_v4().to_string());
        let first = Instance::acquire(&root).unwrap().unwrap();
        assert!(Instance::acquire(&root.join(".")).unwrap().is_none());
        let other = root.join("other");
        assert!(Instance::acquire(&other).unwrap().is_some());
        drop(first);
        assert!(Instance::acquire(&root).unwrap().is_some());
        std::fs::remove_dir_all(root).unwrap();
    }
}
