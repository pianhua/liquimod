use crate::error::Result;
use crate::library::Library;
use crate::models::ModEntry;
use std::path::{Path, PathBuf};

pub struct Deployer<'a> {
    pub library: &'a Library,
    pub mods_dir: PathBuf,
}

impl<'a> Deployer<'a> {
    pub fn new(library: &'a Library, mods_dir: &Path) -> Self {
        Self { library, mods_dir: mods_dir.to_path_buf() }
    }

    /// 3Dmigoto Mods 目录中的链接名：角色--Mod名（确定性，避免跨角色重名冲突）
    pub fn link_name(entry: &ModEntry) -> String {
        format!("{}--{}", entry.character, entry.name)
    }

    pub fn enable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if entry.enabled {
            return Ok(());
        }
        let target = self.library.layout.root.join(&entry.rel_path);
        if !target.is_dir() {
            return Err(crate::error::LiquiModError::ModNotFound(format!(
                "library folder missing: {}",
                target.display()
            )));
        }
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        // 用 junction::exists 而非 Path::exists：后者会穿透 junction，悬空链接会被误判为不存在
        if junction::exists(&link).unwrap_or(false) {
            // 已是 junction（可能悬空）→ 先拆掉重建，保证指向正确 target
            junction::delete(&link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
            if link.exists() {
                std::fs::remove_dir(&link)
                    .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
            }
        } else if link.symlink_metadata().is_ok() {
            // 残留的非 junction 目录（崩溃残留）：仅在为空目录时移除，非空则报错（不碰用户数据）
            std::fs::remove_dir(&link).map_err(|e| {
                crate::error::LiquiModError::Junction(format!(
                    "path occupied by non-junction entry: {} ({e})",
                    link.display()
                ))
            })?;
        }
        junction::create(&target, &link)
            .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        self.library.db.set_enabled(id, true)?;
        self.library.db.op_finish(op)
    }

    pub fn disable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if !entry.enabled {
            return Ok(());
        }
        let op = self.library.db.op_begin("disable", &id.to_string())?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        if junction::exists(&link).unwrap_or(false) {
            junction::delete(&link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        if link.exists() {
            std::fs::remove_dir(&link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        self.library.db.set_enabled(id, false)?;
        self.library.db.op_finish(op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Library, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();
        let mods_dir = tmp.path().join("GameMods");
        fs::create_dir_all(&mods_dir).unwrap();
        (tmp, lib, mods_dir)
    }

    #[test]
    fn enable_creates_junction_disable_removes_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();

        let link = mods_dir.join(Deployer::link_name(&entry));
        assert!(junction::exists(&link).unwrap());
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);

        d.disable(entry.id).unwrap();
        assert!(!link.exists());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);

        d.disable(entry.id).unwrap();
    }

    #[test]
    fn enable_heals_stale_empty_dir_from_crashed_disable() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let d = Deployer::new(&lib, &mods_dir);

        // 模拟 disable 崩溃残留：junction 已删但空目录还在，且 DB 仍是 enabled=false
        fs::create_dir_all(mods_dir.join(Deployer::link_name(&entry))).unwrap();

        d.enable(entry.id).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        assert!(junction::exists(&link).unwrap());
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
    }

    #[test]
    fn enable_rejects_missing_library_folder() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        fs::remove_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        assert!(d.enable(entry.id).is_err());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
    }
}
