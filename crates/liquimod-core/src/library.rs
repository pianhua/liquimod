use crate::db::Database;
use crate::error::Result;
use crate::models::ModEntry;
use crate::paths::{is_valid_segment, LibraryLayout};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

pub(crate) const INSTALLING_MARKER: &str = ".liquimod-installing";
pub(crate) static INSTALL_LOCK: Mutex<()> = Mutex::new(());

pub struct Library {
    pub layout: LibraryLayout,
    pub db: Database,
}

impl Library {
    pub fn init(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        std::fs::create_dir_all(layout.mods_root())?;
        let db = Database::open(&layout.db_path())?;
        clean_temp_dirs(&layout.root.join("tmp"))?;
        recover_pending_installs(&layout, &db)?;
        Ok(Self { layout, db })
    }

    pub fn open(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        let db = Database::open(&layout.db_path())?;
        clean_temp_dirs(&layout.root.join("tmp"))?;
        recover_pending_installs(&layout, &db)?;
        Ok(Self { layout, db })
    }

    pub fn list(&self) -> Result<Vec<ModEntry>> {
        self.db.list_mods()
    }

    pub fn scan(&self) -> Result<Vec<ModEntry>> {
        recover_pending_installs(&self.layout, &self.db)?;
        let mut seen: Vec<(String, String)> = Vec::new();
        let mods_root = self.layout.mods_root();
        if !mods_root.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("library mods root missing: {}", mods_root.display()),
            )
            .into());
        }
        for char_entry in std::fs::read_dir(&mods_root)? {
            let char_entry = char_entry?;
            let character = char_entry.file_name().to_string_lossy().into_owned();
            let ft = char_entry.file_type()?;
            if !ft.is_dir() || ft.is_symlink() || !is_valid_segment(&character) {
                continue;
            }
            for mod_entry in std::fs::read_dir(char_entry.path())? {
                let mod_entry = mod_entry?;
                let name = mod_entry.file_name().to_string_lossy().into_owned();
                let ft = mod_entry.file_type()?;
                if !ft.is_dir() || ft.is_symlink() || !is_valid_segment(&name) {
                    continue;
                }
                let rel = format!("mods/{}/{}", character, name);
                let id = self.db.upsert_mod(&character, &name, &rel)?;
                refresh_stats(&self.db, id, &mod_entry.path())?;
                seen.push((character.clone(), name));
            }
        }
        for m in self.db.list_mods()? {
            if !seen.contains(&(m.character.clone(), m.name.clone())) {
                self.db.remove_mod(m.id)?;
                crate::thumbs::remove_thumbnail(&self.layout.root, m.id);
            }
        }
        self.db.list_mods()
    }

    /// 把外部文件夹复制进仓库并收录索引。已存在同名 mod 则覆盖式合并。
    pub fn add_folder(&self, src: &Path, character: &str, name: &str) -> Result<ModEntry> {
        if !is_valid_segment(character) {
            return Err(crate::error::LiquiModError::InvalidName(character.into()));
        }
        if !is_valid_segment(name) {
            return Err(crate::error::LiquiModError::InvalidName(name.into()));
        }
        let dest = self.layout.mod_dir(character, name);
        std::fs::create_dir_all(&dest)?;
        let src_canon = src.canonicalize()?;
        let dest_canon = dest.canonicalize()?;
        if src_canon.starts_with(&dest_canon) || dest_canon.starts_with(&src_canon) {
            return Err(crate::error::LiquiModError::InvalidName(name.into()));
        }
        copy_dir_recursive(&src_canon, &dest_canon)?;
        let rel = format!("mods/{}/{}", character, name);
        let id = self.db.upsert_mod(character, name, &rel)?;
        refresh_stats(&self.db, id, &dest)?;
        self.db.get_mod(id)
    }

    /// 重命名仓库内 Mod（只动文件系统与 DB；Junction 重建由调用方负责）。
    /// 校验失败/冲突时目录保持原样。
    pub fn rename_mod(&self, id: i64, new_name: &str) -> Result<ModEntry> {
        if !is_valid_segment(new_name) {
            return Err(crate::error::LiquiModError::InvalidName(new_name.into()));
        }
        let entry = self.db.get_mod(id)?;
        if entry.name == new_name {
            return Ok(entry);
        }
        if self.db.name_taken(&entry.character, new_name, id)? {
            return Err(crate::error::LiquiModError::DestinationExists {
                character: entry.character.clone(),
                name: new_name.into(),
            });
        }
        let old_dir = self.layout.root.join(&entry.rel_path);
        let new_rel = format!("mods/{}/{}", entry.character, new_name);
        let new_dir = self.layout.root.join(&new_rel);
        std::fs::rename(&old_dir, &new_dir)?;
        if let Err(e) = self.db.rename_mod(id, new_name, &new_rel) {
            let _ = std::fs::rename(&new_dir, &old_dir); // DB 失败回滚目录
            return Err(e);
        }
        self.db.get_mod(id)
    }
}

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let ft = entry.file_type()?;
        // 跳过符号链接，与 scan 策略一致
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// 递归统计目录（总字节, 文件数）；任何一级读不了就返回 (-1, -1)（前端显示 "—"）。
fn dir_stats(dir: &std::path::Path) -> (i64, i64) {
    let mut stack = vec![dir.to_path_buf()];
    let (mut size, mut count) = (0i64, 0i64);
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => return (-1, -1),
        };
        for e in rd.flatten() {
            let ft = match e.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() && !ft.is_symlink() {
                stack.push(e.path());
            } else if ft.is_file() {
                count += 1;
                size += e.metadata().map(|m| m.len()).unwrap_or(0) as i64;
            }
        }
    }
    (size, count)
}

/// 刷新 mod 统计：仅统计成功（非 -1）时更新 DB，失败保留旧值。
fn refresh_stats(db: &Database, id: i64, path: &Path) -> Result<()> {
    let (size, count) = dir_stats(path);
    if (size, count) != (-1, -1) {
        db.update_stats(id, size, count)?;
    }
    Ok(())
}

fn recover_pending_installs(layout: &LibraryLayout, db: &Database) -> Result<()> {
    let _install_lock = INSTALL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for (op_id, op, payload) in db.pending_ops()? {
        if op != "install" {
            continue;
        }
        if let Some(destination) = install_destination(layout, &payload) {
            let marker = destination.join(INSTALLING_MARKER);
            if matches!(
                std::fs::symlink_metadata(marker),
                Ok(metadata) if metadata.file_type().is_file()
            ) {
                remove_path_if_present(&destination)?;
            }
        }
        db.remove_op(op_id)?;
    }
    Ok(())
}

fn clean_temp_dirs(tmp_root: &Path) -> Result<()> {
    let entries = match std::fs::read_dir(tmp_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("liquimod-") {
            continue;
        }
        if entry.file_type()?.is_dir() {
            std::fs::remove_dir_all(entry.path())?;
        }
    }
    Ok(())
}

fn install_destination(layout: &LibraryLayout, payload: &str) -> Option<PathBuf> {
    let payload_path = Path::new(payload);
    let relative = if payload_path.is_absolute() {
        payload_path.strip_prefix(&layout.root).ok()?
    } else {
        payload_path
    };
    let mut components = relative.components();
    let Component::Normal(root) = components.next()? else {
        return None;
    };
    let Component::Normal(character) = components.next()? else {
        return None;
    };
    let Component::Normal(name) = components.next()? else {
        return None;
    };
    if components.next().is_some() {
        return None;
    }
    let root = root.to_str()?;
    let character = character.to_str()?;
    let name = name.to_str()?;
    if root != "mods" || !is_valid_segment(character) || !is_valid_segment(name) {
        return None;
    }
    Some(layout.mod_dir(character, name))
}

fn remove_path_if_present(path: &Path) -> Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    if metadata.is_dir() && !metadata.file_type().is_symlink() {
        std::fs::remove_dir_all(path)?;
    } else {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_layout_and_scan_reconciles() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        assert!(lib.layout.mods_root().is_dir());
        assert!(lib.layout.db_path().is_file());

        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        assert_eq!(mods.len(), 2);

        fs::remove_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].character, "Firefly");

        let lib2 = Library::open(tmp.path()).unwrap();
        assert_eq!(lib2.list().unwrap().len(), 1);
    }

    #[test]
    fn add_folder_copies_and_indexes() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();

        let src = tmp.path().join("download/MyMod");
        fs::create_dir_all(src.join("textures")).unwrap();
        fs::write(src.join("mod.ini"), b"[Constants]").unwrap();
        fs::write(src.join("textures/a.dds"), b"dds").unwrap();

        let entry = lib.add_folder(&src, "Firefly", "MyMod").unwrap();
        assert_eq!(entry.character, "Firefly");
        assert!(lib
            .layout
            .mod_dir("Firefly", "MyMod")
            .join("mod.ini")
            .is_file());
        assert!(lib
            .layout
            .mod_dir("Firefly", "MyMod")
            .join("textures/a.dds")
            .is_file());

        assert!(lib.add_folder(&src, "bad/name", "x").is_err());
        assert!(lib.add_folder(&src, "Firefly", "..").is_err());
    }

    #[test]
    fn add_folder_rejects_overlapping_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "MyMod")).unwrap();
        fs::write(lib.layout.mod_dir("Firefly", "MyMod").join("mod.ini"), b"x").unwrap();

        // src 就是 dest
        assert!(lib
            .add_folder(&lib.layout.mod_dir("Firefly", "MyMod"), "Firefly", "MyMod")
            .is_err());
        // src 是 dest 的祖先
        assert!(lib
            .add_folder(&lib.layout.character_dir("Firefly"), "Firefly", "Sub")
            .is_err());
    }

    #[cfg(windows)]
    #[ignore = "需要创建符号链接的特权（开发者模式/管理员），有权限时用 --ignored 运行"]
    #[test]
    fn add_folder_skips_symlinks() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();

        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("secret.txt"), b"secret").unwrap();

        let src = tmp.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("mod.ini"), b"[Constants]").unwrap();
        std::os::windows::fs::symlink_file(outside.join("secret.txt"), src.join("link.txt"))
            .unwrap();

        let entry = lib.add_folder(&src, "Firefly", "LinkTest").unwrap();
        assert_eq!(entry.name, "LinkTest");
        let dest = lib.layout.mod_dir("Firefly", "LinkTest");
        assert!(dest.join("mod.ini").is_file());
        assert!(!dest.join("link.txt").exists());
    }

    #[test]
    fn scan_skips_files_and_invalid_names() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();

        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        fs::write(lib.layout.mods_root().join("loose.txt"), b"x").unwrap();
        fs::write(lib.layout.character_dir("Firefly").join("note.txt"), b"x").unwrap();

        let mods = lib.scan().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].character, "Firefly");
        assert_eq!(mods[0].name, "Summer");
    }

    #[test]
    fn scan_errors_when_mods_root_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        lib.scan().unwrap();
        assert_eq!(lib.list().unwrap().len(), 1);

        fs::remove_dir_all(lib.layout.mods_root()).unwrap();
        assert!(lib.scan().is_err());
        assert_eq!(lib.list().unwrap().len(), 1);
    }

    #[test]
    fn init_cleans_temp_dirs_on_startup() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let temp_dir = root.join("tmp/liquimod-startup");
        fs::create_dir_all(&temp_dir).unwrap();

        Library::init(&root).unwrap();

        assert!(!temp_dir.exists());
    }

    #[test]
    fn scan_does_not_clean_active_temp_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let temp_dir = tmp.path().join("tmp/liquimod-active");
        fs::create_dir_all(&temp_dir).unwrap();

        lib.scan().unwrap();

        assert!(temp_dir.exists());
    }

    #[test]
    fn open_recovers_interrupted_install() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let lib = Library::init(&root).unwrap();
        let destination = lib.layout.mod_dir("Firefly", "CrashedMod");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("partial.txt"), b"partial").unwrap();
        std::fs::write(destination.join(".liquimod-installing"), b"").unwrap();
        let temp_dir = root.join("tmp/liquimod-crashed");
        std::fs::create_dir_all(&temp_dir).unwrap();
        std::fs::write(temp_dir.join("partial.txt"), b"partial").unwrap();
        lib.db
            .op_begin("install", "mods/Firefly/CrashedMod")
            .unwrap();
        drop(lib);

        let recovered = Library::open(&root).unwrap();

        assert!(!destination.exists());
        assert!(!temp_dir.exists());
        assert!(recovered.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn open_preserves_unmarked_interrupted_install_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let lib = Library::init(&root).unwrap();
        let destination = lib.layout.mod_dir("Firefly", "ManualMod");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("partial.txt"), b"partial").unwrap();
        lib.db
            .op_begin("install", "mods/Firefly/ManualMod")
            .unwrap();
        drop(lib);

        let recovered = Library::open(&root).unwrap();

        assert!(destination.exists());
        assert!(destination.join("partial.txt").is_file());
        assert!(recovered.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn dir_stats_counts_files_and_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("a.bin"), vec![0u8; 100]).unwrap();
        std::fs::create_dir(tmp.path().join("sub")).unwrap();
        std::fs::write(tmp.path().join("sub/b.bin"), vec![0u8; 50]).unwrap();
        assert_eq!(dir_stats(tmp.path()), (150, 2));
    }

    #[test]
    fn dir_stats_missing_dir_returns_minus_one() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(dir_stats(&tmp.path().join("nope")), (-1, -1));
    }

    #[test]
    fn rename_mod_moves_dir_and_updates_db() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "A", "old").unwrap();
        let renamed = lib.rename_mod(m.id, "new").unwrap();
        assert_eq!(renamed.name, "new");
        assert!(lib.layout.mod_dir("A", "new").is_dir());
        assert!(!lib.layout.mod_dir("A", "old").exists());
    }

    #[test]
    fn rename_mod_rejects_conflict_and_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m1 = lib.add_folder(src.path(), "A", "m1").unwrap();
        lib.add_folder(src.path(), "A", "m2").unwrap();
        assert!(matches!(
            lib.rename_mod(m1.id, "m2"),
            Err(crate::error::LiquiModError::DestinationExists { .. })
        ));
        assert!(matches!(
            lib.rename_mod(m1.id, "a/b"),
            Err(crate::error::LiquiModError::InvalidName(_))
        ));
        // 冲突失败后目录原样
        assert!(lib.layout.mod_dir("A", "m1").is_dir());
    }

    #[test]
    fn scan_updates_stats() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let dir = lib.layout.mod_dir("A", "m1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.bin"), vec![0u8; 42]).unwrap();
        lib.scan().unwrap();
        let m = &lib.list().unwrap()[0];
        assert_eq!((m.size_bytes, m.file_count), (42, 1));
    }

    #[test]
    fn refresh_stats_preserves_old_values_on_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let id = lib.db.upsert_mod("A", "m", "mods/A/m").unwrap();
        lib.db.update_stats(id, 999, 9).unwrap();
        // 路径不存在 → 统计失败，保留旧值
        refresh_stats(&lib.db, id, &tmp.path().join("nope")).unwrap();
        let m = lib.db.get_mod(id).unwrap();
        assert_eq!((m.size_bytes, m.file_count), (999, 9));
    }

    #[test]
    fn refresh_stats_overwrites_on_success() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let id = lib.db.upsert_mod("A", "m", "mods/A/m").unwrap();
        lib.db.update_stats(id, 999, 9).unwrap();
        let dir = lib.layout.mod_dir("A", "m");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("f.bin"), vec![0u8; 42]).unwrap();
        // 统计成功 → 覆盖旧值
        refresh_stats(&lib.db, id, &dir).unwrap();
        let m = lib.db.get_mod(id).unwrap();
        assert_eq!((m.size_bytes, m.file_count), (42, 1));
    }

    #[test]
    fn add_folder_sets_stats_immediately() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), vec![0u8; 7]).unwrap();
        let m = lib.add_folder(src.path(), "A", "m1").unwrap();
        assert!((m.size_bytes, m.file_count) != (-1, -1));
        let got = lib.db.get_mod(m.id).unwrap();
        assert!((got.size_bytes, got.file_count) != (-1, -1));
    }
}
