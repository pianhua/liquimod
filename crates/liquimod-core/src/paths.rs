use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LibraryLayout {
    pub root: PathBuf,
}

impl LibraryLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn mods_root(&self) -> PathBuf {
        self.root.join("mods")
    }
    pub fn db_path(&self) -> PathBuf {
        self.root.join("liquimod.db")
    }
    pub fn character_dir(&self, character: &str) -> PathBuf {
        self.mods_root().join(character)
    }
    pub fn mod_dir(&self, character: &str, name: &str) -> PathBuf {
        self.character_dir(character).join(name)
    }
}

pub fn is_valid_segment(s: &str) -> bool {
    !s.is_empty() && s != ".." && !s.contains(['/', '\\'])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_paths() {
        let l = LibraryLayout::new("C:/lib");
        assert_eq!(l.mods_root(), PathBuf::from("C:/lib").join("mods"));
        assert_eq!(l.db_path(), PathBuf::from("C:/lib").join("liquimod.db"));
        assert_eq!(l.character_dir("Firefly"), PathBuf::from("C:/lib").join("mods").join("Firefly"));
        assert_eq!(l.mod_dir("Firefly", "Summer"), PathBuf::from("C:/lib").join("mods").join("Firefly").join("Summer"));
    }

    #[test]
    fn rejects_bad_segments() {
        assert!(!is_valid_segment(""));
        assert!(!is_valid_segment("a/b"));
        assert!(!is_valid_segment("a\\b"));
        assert!(!is_valid_segment(".."));
        assert!(is_valid_segment("流萤 Firefly"));
    }
}
