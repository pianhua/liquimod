use crate::error::{LiquiModError, Result};
use crate::library::Library;
use std::path::{Path, PathBuf};

const RESERVED_HEADROOM: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageMigrationReport {
    pub library_root: PathBuf,
    pub copied_files: u64,
    pub copied_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageStats {
    pub files: u64,
    pub bytes: u64,
    pub available_bytes: Option<u64>,
}

pub fn storage_stats(root: &Path) -> Result<StorageStats> {
    let (files, bytes) = tree_stats(root)?;
    Ok(StorageStats {
        files,
        bytes,
        available_bytes: available_space(root),
    })
}

/// Copy a library into `<data_root>/Library`, verify it, then atomically publish it.
/// The source remains untouched so callers can offer explicit cleanup after switching.
pub fn migrate_library(source: &Library, data_root: &Path) -> Result<StorageMigrationReport> {
    source.db.checkpoint()?;
    source.db.verify_integrity()?;

    std::fs::create_dir_all(data_root)?;
    let source_root = source.layout.root.canonicalize()?;
    let data_root = data_root.canonicalize()?;
    let final_root = data_root.join("Library");
    if final_root == source_root
        || final_root.starts_with(&source_root)
        || source_root.starts_with(&final_root)
    {
        return Err(invalid_input("新存储目录不能与当前仓库重叠"));
    }
    if final_root.exists() {
        let empty = final_root.is_dir() && std::fs::read_dir(&final_root)?.next().is_none();
        if empty {
            std::fs::remove_dir(&final_root)?;
        } else {
            return Err(invalid_input(format!(
                "目标仓库已存在且非空：{}",
                final_root.display()
            )));
        }
    }

    let (source_files, source_bytes) = tree_stats(&source_root)?;
    if let Some(available) = available_space(&data_root) {
        if available < source_bytes.saturating_add(RESERVED_HEADROOM) {
            return Err(invalid_input(format!(
                "目标盘空间不足：需要至少 {} 字节，当前可用 {} 字节",
                source_bytes.saturating_add(RESERVED_HEADROOM),
                available
            )));
        }
    }

    let staging_parent = data_root.join(format!(".liquimod-migrating-{}", uuid::Uuid::new_v4()));
    let staging_root = staging_parent.join("Library");
    let result = (|| {
        copy_tree(&source_root, &staging_root, true)?;
        let (copied_files, copied_bytes) = tree_stats(&staging_root)?;
        if (copied_files, copied_bytes) != (source_files, source_bytes) {
            return Err(invalid_input(format!(
                "迁移校验失败：源 {source_files} 文件/{source_bytes} 字节，目标 {copied_files} 文件/{copied_bytes} 字节"
            )));
        }
        let staged = Library::open(&staging_root)?;
        staged.db.verify_integrity()?;
        drop(staged);
        std::fs::rename(&staging_root, &final_root)?;
        Ok(StorageMigrationReport {
            library_root: final_root,
            copied_files,
            copied_bytes,
        })
    })();
    if staging_parent.exists() {
        let _ = std::fs::remove_dir_all(&staging_parent);
    }
    result
}

pub fn copy_managed_directory(source: &Path, destination: &Path) -> Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    if destination.exists() {
        let empty = destination.is_dir() && std::fs::read_dir(destination)?.next().is_none();
        if empty {
            std::fs::remove_dir(destination)?;
        } else {
            return Err(invalid_input(format!(
                "目标托管 3DMigoto 目录已存在且非空：{}",
                destination.display()
            )));
        }
    }
    let parent = destination
        .parent()
        .ok_or_else(|| invalid_input("托管目录缺少父路径"))?;
    std::fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".liquimod-copying-{}", uuid::Uuid::new_v4()));
    let result = (|| {
        copy_tree(source, &staging, false)?;
        let source_stats = tree_stats(source)?;
        let target_stats = tree_stats(&staging)?;
        if source_stats != target_stats {
            return Err(invalid_input("托管 3DMigoto 目录复制校验失败"));
        }
        std::fs::rename(&staging, destination)?;
        Ok(())
    })();
    if staging.exists() {
        let _ = std::fs::remove_dir_all(staging);
    }
    result
}

fn copy_tree(source: &Path, destination: &Path, library_root: bool) -> Result<()> {
    std::fs::create_dir_all(destination)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        if library_root && should_skip_library_entry(&name.to_string_lossy()) {
            continue;
        }
        let path = entry.path();
        let metadata = entry.file_type()?;
        if metadata.is_symlink() || junction::exists(&path).unwrap_or(false) {
            continue;
        }
        let target = destination.join(&name);
        if metadata.is_dir() {
            copy_tree(&path, &target, false)?;
        } else if metadata.is_file() {
            std::fs::copy(path, target)?;
        }
    }
    Ok(())
}

fn tree_stats(root: &Path) -> Result<(u64, u64)> {
    if !root.exists() {
        return Ok((0, 0));
    }
    let mut files = 0u64;
    let mut bytes = 0u64;
    let mut stack = vec![(root.to_path_buf(), true)];
    while let Some((dir, is_root)) = stack.pop() {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let name = entry.file_name();
            if is_root && should_skip_library_entry(&name.to_string_lossy()) {
                continue;
            }
            let path = entry.path();
            let kind = entry.file_type()?;
            if kind.is_symlink() || junction::exists(&path).unwrap_or(false) {
                continue;
            }
            if kind.is_dir() {
                stack.push((path, false));
            } else if kind.is_file() {
                files += 1;
                bytes = bytes.saturating_add(entry.metadata()?.len());
            }
        }
    }
    Ok((files, bytes))
}

fn should_skip_library_entry(name: &str) -> bool {
    name == "tmp"
        || name == ".liquimod-runtime"
        || name.ends_with(".db-wal")
        || name.ends_with(".db-shm")
}

fn invalid_input(message: impl Into<String>) -> LiquiModError {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message.into()).into()
}

pub fn available_space(path: &Path) -> Option<u64> {
    fs2::available_space(path).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migration_copies_and_verifies_library_without_ephemeral_dirs() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source = Library::init(source_dir.path()).unwrap();
        let mod_dir = source.layout.mod_dir("Firefly", "Summer");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("mod.ini"), b"[Constants]").unwrap();
        std::fs::create_dir_all(source.layout.root.join("tmp/liquimod-stale")).unwrap();
        std::fs::write(source.layout.root.join("tmp/liquimod-stale/x"), b"x").unwrap();
        source.scan().unwrap();

        let report = migrate_library(&source, target_dir.path()).unwrap();
        assert!(report
            .library_root
            .join("mods/Firefly/Summer/mod.ini")
            .is_file());
        assert!(!report.library_root.join("tmp/liquimod-stale").exists());
        assert_eq!(
            Library::open(&report.library_root)
                .unwrap()
                .list()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn migration_rejects_nonempty_destination() {
        let source_dir = tempfile::tempdir().unwrap();
        let target_dir = tempfile::tempdir().unwrap();
        let source = Library::init(source_dir.path()).unwrap();
        std::fs::create_dir_all(target_dir.path().join("Library")).unwrap();
        std::fs::write(target_dir.path().join("Library/owned.txt"), b"x").unwrap();
        assert!(migrate_library(&source, target_dir.path()).is_err());
    }
}
