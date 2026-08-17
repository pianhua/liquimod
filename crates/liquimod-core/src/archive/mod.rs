pub mod detect;
pub mod rar;
pub mod sevenz;
pub mod zip_extract;

use crate::db::Database;
use crate::error::{LiquiModError, Result};
use detect::{detect_format, ArchiveFormat};
use std::path::{Path, PathBuf};

const MAX_CONTENT_ROOT_DEPTH: u32 = 10;
const MAX_NESTED_DEPTH: u32 = 5;

/// Unwraps single-directory archive wrappers, stopping after ten directory levels.
pub fn resolve_content_root(dir: &Path) -> Result<PathBuf> {
    let mut current = dir.to_path_buf();

    for _ in 0..MAX_CONTENT_ROOT_DEPTH {
        let entries = std::fs::read_dir(&current)?.collect::<std::io::Result<Vec<_>>>()?;
        if entries.len() != 1 {
            break;
        }

        let Some(entry) = entries.into_iter().next() else {
            break;
        };
        let path = entry.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || junction::exists(&path).unwrap_or(false)
        {
            break;
        }
        current = path;
    }

    Ok(current)
}

pub struct ExtractReport {
    pub nested_warnings: Vec<String>,
}

/// Extracts an archive and recursively scans all regular files in its output tree for nested archives.
pub fn extract_recursive(
    archive_path: &Path,
    dest: &Path,
    password: Option<&str>,
    depth: u32,
    report: &mut ExtractReport,
) -> Result<()> {
    std::fs::create_dir_all(dest)?;

    match detect_format(archive_path)? {
        ArchiveFormat::Zip => zip_extract::extract_zip(archive_path, dest, password)?,
        ArchiveFormat::SevenZ => sevenz::extract_7z(archive_path, dest, password)?,
        ArchiveFormat::Rar => rar::extract_rar(archive_path, dest, password)?,
    }

    let mut files = Vec::new();
    collect_regular_files(dest, &mut files)?;
    let mut nested_index = 0;
    for path in files {
        let is_archive = match detect_format(&path) {
            Ok(_) => true,
            Err(LiquiModError::UnsupportedArchive(_)) => false,
            Err(error) => return Err(error),
        };
        if !is_archive {
            continue;
        }

        let nested_dest = next_nested_dest(dest, &mut nested_index)?;
        if depth < MAX_NESTED_DEPTH - 1 {
            extract_recursive(&path, &nested_dest, password, depth + 1, report)?;
        } else {
            report.nested_warnings.push(format!(
                "nested archive depth limit reached; skipping {}",
                path.display()
            ));
        }
    }

    Ok(())
}

fn collect_regular_files(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() || junction::exists(&path).unwrap_or(false) {
            continue;
        }
        if metadata.is_dir() {
            collect_regular_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn next_nested_dest(dest: &Path, index: &mut u32) -> Result<PathBuf> {
    loop {
        let path = dest.join(format!("__nested_{index}"));
        *index += 1;
        match std::fs::symlink_metadata(&path) {
            Ok(_) => continue,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(path),
            Err(error) => return Err(error.into()),
        }
    }
}

pub struct PasswordBook<'a> {
    db: &'a Database,
}

impl<'a> PasswordBook<'a> {
    pub fn new(db: &'a Database) -> Self {
        Self { db }
    }

    pub fn candidates(&self) -> Result<Vec<String>> {
        self.db.list_passwords()
    }

    pub fn learn(&self, password: &str) -> Result<()> {
        self.db.add_password(password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    #[test]
    fn learn_then_candidates() {
        let db = Database::open_in_memory().unwrap();
        let book = PasswordBook::new(&db);
        book.learn("x").unwrap();
        assert!(book.candidates().unwrap().contains(&"x".to_string()));
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8])]) {
        let file = File::create(path).unwrap();
        let mut writer = ZipWriter::new(file);
        for (name, contents) in files {
            writer
                .start_file(*name, SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    fn zip_bytes(files: &[(&str, &[u8])]) -> Vec<u8> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("archive.zip");
        write_zip(&path, files);
        std::fs::read(path).unwrap()
    }

    fn nested_zip(dir: &Path, layers: usize) -> PathBuf {
        let deepest_marker = format!("level{layers}.txt");
        let mut contents = zip_bytes(&[(deepest_marker.as_str(), b"marker")]);
        for level in (1..layers).rev() {
            let marker = format!("level{level}.txt");
            contents = zip_bytes(&[(marker.as_str(), b"marker"), ("inner.zip", &contents)]);
        }
        let path = dir.join("outer.zip");
        std::fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn content_root_unwraps_single_wrapper_dir() {
        let dir = tempfile::tempdir().unwrap();
        let content = dir.path().join("A").join("B");
        std::fs::create_dir_all(&content).unwrap();
        std::fs::write(content.join("file"), b"content").unwrap();

        assert_eq!(resolve_content_root(dir.path()).unwrap(), content);
    }

    #[test]
    fn content_root_stops_at_multiple_entries() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("A")).unwrap();
        std::fs::create_dir(dir.path().join("B")).unwrap();

        assert_eq!(resolve_content_root(dir.path()).unwrap(), dir.path());
    }

    #[test]
    fn content_root_depth_cap() {
        let dir = tempfile::tempdir().unwrap();
        let mut current = dir.path().to_path_buf();
        let mut expected = dir.path().to_path_buf();
        for layer in 1..=11 {
            current = current.join(format!("layer{layer}"));
            std::fs::create_dir(&current).unwrap();
            if layer <= 10 {
                expected = current.clone();
            }
        }
        std::fs::write(current.join("file"), b"content").unwrap();

        assert_eq!(resolve_content_root(dir.path()).unwrap(), expected);
    }

    #[test]
    fn nested_zip_inside_zip_is_extracted() {
        let source = tempfile::tempdir().unwrap();
        let inner = zip_bytes(&[("inner.txt", b"nested")]);
        let outer = source.path().join("outer.zip");
        write_zip(&outer, &[("inner.zip", &inner)]);
        let dest = tempfile::tempdir().unwrap();
        let mut report = ExtractReport {
            nested_warnings: Vec::new(),
        };

        extract_recursive(&outer, dest.path(), None, 0, &mut report).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest.path().join("__nested_0/inner.txt")).unwrap(),
            "nested"
        );
        assert!(report.nested_warnings.is_empty());
    }

    #[test]
    fn depth_limit_stops_recursion() {
        let source = tempfile::tempdir().unwrap();
        let archive = nested_zip(source.path(), 6);
        let dest = tempfile::tempdir().unwrap();
        let mut report = ExtractReport {
            nested_warnings: Vec::new(),
        };

        extract_recursive(&archive, dest.path(), None, 0, &mut report).unwrap();

        let mut layer_dest = dest.path().to_path_buf();
        for level in 1..=5 {
            assert!(layer_dest.join(format!("level{level}.txt")).is_file());
            if level < 5 {
                layer_dest = layer_dest.join("__nested_0");
            }
        }
        let sixth_dest = layer_dest.join("__nested_0");
        assert!(!sixth_dest.exists());
        assert!(!sixth_dest.join("level6.txt").exists());
        assert!(report
            .nested_warnings
            .iter()
            .any(|warning| warning.contains("depth limit")));
    }

    #[test]
    fn non_archive_files_are_ignored() {
        let source = tempfile::tempdir().unwrap();
        let archive = source.path().join("outer.zip");
        write_zip(&archive, &[("readme.txt", b"plain")]);
        let dest = tempfile::tempdir().unwrap();
        let mut report = ExtractReport {
            nested_warnings: Vec::new(),
        };

        extract_recursive(&archive, dest.path(), None, 0, &mut report).unwrap();

        assert!(!dest.path().join("__nested_0").exists());
        assert!(report.nested_warnings.is_empty());
    }
}
