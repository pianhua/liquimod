use crate::db::Database;
use crate::error::Result;
use crate::models::ModEntry;
use crate::paths::{is_valid_segment, LibraryLayout};
use std::path::{Component, Path, PathBuf};

pub struct Library {
    pub layout: LibraryLayout,
    pub db: Database,
}

impl Library {
    pub fn init(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        std::fs::create_dir_all(layout.mods_root())?;
        let db = Database::open(&layout.db_path())?;
        recover_pending_installs(&layout, &db)?;
        Ok(Self { layout, db })
    }

    pub fn open(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        let db = Database::open(&layout.db_path())?;
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
                self.db.upsert_mod(&character, &name, &rel)?;
                seen.push((character.clone(), name));
            }
        }
        for m in self.db.list_mods()? {
            if !seen.contains(&(m.character.clone(), m.name.clone())) {
                self.db.remove_mod(m.id)?;
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

fn recover_pending_installs(layout: &LibraryLayout, db: &Database) -> Result<()> {
    clean_temp_dirs(&layout.root.join("tmp"))?;
    for (op_id, op, payload) in db.pending_ops()? {
        if op != "install" {
            continue;
        }
        if let Some(destination) = install_destination(layout, &payload) {
            remove_path_if_present(&destination)?;
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
    fn open_recovers_interrupted_install() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("library");
        let lib = Library::init(&root).unwrap();
        let destination = lib.layout.mod_dir("Firefly", "CrashedMod");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("partial.txt"), b"partial").unwrap();
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
}
