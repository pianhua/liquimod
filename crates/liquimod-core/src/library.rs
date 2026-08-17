use crate::db::Database;
use crate::error::Result;
use crate::models::ModEntry;
use crate::paths::{is_valid_segment, LibraryLayout};
use std::path::Path;

pub struct Library {
    pub layout: LibraryLayout,
    pub db: Database,
}

impl Library {
    pub fn init(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        std::fs::create_dir_all(layout.mods_root())?;
        let db = Database::open(&layout.db_path())?;
        Ok(Self { layout, db })
    }

    pub fn open(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        let db = Database::open(&layout.db_path())?;
        Ok(Self { layout, db })
    }

    pub fn list(&self) -> Result<Vec<ModEntry>> {
        self.db.list_mods()
    }

    pub fn scan(&self) -> Result<Vec<ModEntry>> {
        let mut seen: Vec<(String, String)> = Vec::new();
        let mods_root = self.layout.mods_root();
        if mods_root.is_dir() {
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
        }
        for m in self.db.list_mods()? {
            if !seen.contains(&(m.character.clone(), m.name.clone())) {
                self.db.remove_mod(m.id)?;
            }
        }
        self.db.list_mods()
    }
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
}
