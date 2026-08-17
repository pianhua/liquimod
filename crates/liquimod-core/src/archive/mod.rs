pub mod detect;
pub mod install;
pub mod rar;
pub mod sevenz;
pub mod zip_extract;

use crate::db::Database;
use crate::error::{LiquiModError, Result};
use detect::{detect_format, ArchiveFormat};
use std::path::{Path, PathBuf};

const MAX_CONTENT_ROOT_DEPTH: u32 = 10;
const MAX_NESTED_DEPTH: u32 = 5;
const MAX_NESTED_ARCHIVES: u32 = 64;
const ERROR_NOT_A_REPARSE_POINT: i32 = 0x1126;

fn map_junction_result(result: std::io::Result<bool>) -> Result<bool> {
    match result {
        Ok(is_junction) => Ok(is_junction),
        Err(error) if error.raw_os_error() == Some(ERROR_NOT_A_REPARSE_POINT) => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn is_junction(path: &Path) -> Result<bool> {
    map_junction_result(junction::exists(path))
}

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
        if metadata.file_type().is_symlink() {
            break;
        }
        if is_junction(&path)? || !metadata.is_dir() {
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
/// Top-level password failures are returned to the caller. Password failures in nested archives are
/// recorded as warnings, leaving the nested archive in place for later handling. The caller must
/// provide a new empty directory for every extraction attempt because nested destination numbering
/// avoids existing paths.
pub fn extract_recursive(
    archive_path: &Path,
    dest: &Path,
    password: Option<&str>,
    report: &mut ExtractReport,
) -> Result<()> {
    let mut nested_count = 0;
    extract_recursive_inner(archive_path, dest, password, 0, report, &mut nested_count)
}

fn extract_recursive_inner(
    archive_path: &Path,
    dest: &Path,
    password: Option<&str>,
    depth: u32,
    report: &mut ExtractReport,
    nested_count: &mut u32,
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

        if *nested_count >= MAX_NESTED_ARCHIVES {
            report.nested_warnings.push(format!(
                "nested archive count limit reached; skipping {}",
                path.display()
            ));
            continue;
        }
        *nested_count += 1;
        let nested_dest = next_nested_dest(dest, &mut nested_index)?;
        if depth < MAX_NESTED_DEPTH - 1 {
            match extract_recursive_inner(
                &path,
                &nested_dest,
                password,
                depth + 1,
                report,
                nested_count,
            ) {
                Ok(()) => {}
                Err(LiquiModError::WrongPassword(_)) => {
                    let _ = std::fs::remove_dir_all(&nested_dest);
                    report.nested_warnings.push(format!(
                        "nested archive has a wrong password; leaving {} in place",
                        path.display()
                    ));
                }
                Err(LiquiModError::PasswordRequired(_)) => {
                    let _ = std::fs::remove_dir_all(&nested_dest);
                    report.nested_warnings.push(format!(
                        "nested archive requires a password; leaving {} in place",
                        path.display()
                    ));
                }
                Err(error) => {
                    let _ = std::fs::remove_dir_all(&nested_dest);
                    return Err(error);
                }
            }
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
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        for entry in std::fs::read_dir(current)? {
            let path = entry?.path();
            let metadata = std::fs::symlink_metadata(&path)?;
            if metadata.file_type().is_symlink() || is_junction(&path)? {
                continue;
            }
            if metadata.is_dir() {
                pending.push(path);
            } else if metadata.is_file() {
                files.push(path);
            }
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

    #[test]
    fn junction_non_reparse_point_is_ignored() {
        let result = map_junction_result(Err(std::io::Error::from_raw_os_error(
            ERROR_NOT_A_REPARSE_POINT,
        )))
        .unwrap();

        assert!(!result);
    }

    #[test]
    fn junction_io_error_is_propagated() {
        let error = map_junction_result(Err(std::io::Error::from_raw_os_error(5))).unwrap_err();

        assert!(matches!(error, LiquiModError::Io(_)));
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

        extract_recursive(&outer, dest.path(), None, &mut report).unwrap();

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

        extract_recursive(&archive, dest.path(), None, &mut report).unwrap();

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
    fn nested_archive_count_limit_skips_after_limit() {
        let source = tempfile::tempdir().unwrap();
        let archive_count = MAX_NESTED_ARCHIVES as usize + 1;
        let nested_archives: Vec<Vec<u8>> = (0..archive_count)
            .map(|index| zip_bytes(&[("marker.txt", format!("nested-{index}").as_bytes())]))
            .collect();
        let names: Vec<String> = (0..archive_count)
            .map(|index| format!("inner-{index}.zip"))
            .collect();
        let files: Vec<(&str, &[u8])> = names
            .iter()
            .zip(nested_archives.iter())
            .map(|(name, contents)| (name.as_str(), contents.as_slice()))
            .collect();
        let archive = source.path().join("outer.zip");
        write_zip(&archive, &files);
        let dest = tempfile::tempdir().unwrap();
        let mut report = ExtractReport {
            nested_warnings: Vec::new(),
        };

        extract_recursive(&archive, dest.path(), None, &mut report).unwrap();

        let extracted = (0..MAX_NESTED_ARCHIVES as usize)
            .filter(|index| dest.path().join(format!("__nested_{index}/marker.txt")).is_file())
            .count();
        assert_eq!(extracted, MAX_NESTED_ARCHIVES as usize);
        assert!(!dest
            .path()
            .join(format!("__nested_{MAX_NESTED_ARCHIVES}"))
            .exists());
        assert!(report
            .nested_warnings
            .iter()
            .any(|warning| warning.contains("archive count limit")));
    }

    #[test]
    fn deep_directory_scan_does_not_recurse() {
        let source = tempfile::tempdir().unwrap();
        let nested_path = format!("{}/marker.txt", vec!["dir"; 100].join("/"));
        let archive = source.path().join("outer.zip");
        write_zip(&archive, &[(nested_path.as_str(), b"deep")]);
        let dest = tempfile::tempdir().unwrap();
        let mut report = ExtractReport {
            nested_warnings: Vec::new(),
        };

        extract_recursive(&archive, dest.path(), None, &mut report).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest.path().join(nested_path)).unwrap(),
            "deep"
        );
        assert!(report.nested_warnings.is_empty());
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

        extract_recursive(&archive, dest.path(), None, &mut report).unwrap();

        assert!(!dest.path().join("__nested_0").exists());
        assert!(report.nested_warnings.is_empty());
    }
}
