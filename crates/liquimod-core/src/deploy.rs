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
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        let target = self.library.layout.root.join(&entry.rel_path);
        let link = self.mods_dir.join(Self::link_name(&entry));
        if !link.exists() {
            junction::create(&target, &link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
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
}
