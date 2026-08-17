use super::{extract_recursive, resolve_content_root, ExtractReport, PasswordBook};
use crate::db::Database;
use crate::error::{LiquiModError, Result};
use crate::library::Library;
use crate::paths::is_valid_segment;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

static INSTALL_LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed {
        mod_id: i64,
        name: String,
        warnings: Vec<String>,
    },
    NeedsPassword,
}

/// Installs an archive into the library. Destination ownership checking, copying, and rollback are
/// serialized within this process; callers from separate processes are not synchronized.
pub fn install_archive(
    db: &Database,
    library: &Library,
    archive_path: &Path,
    character: &str,
    explicit_password: Option<&str>,
) -> Result<InstallOutcome> {
    let name = archive_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("archive has no file stem: {}", archive_path.display()),
            )
        })?;
    let temp_dir = TempExtractionDir::new(&library.layout.root);
    let password_book = PasswordBook::new(db);
    let mut candidates = vec![None];
    if let Some(password) = explicit_password {
        add_candidate(&mut candidates, Some(password.to_owned()));
    }
    for password in password_book.candidates()? {
        add_candidate(&mut candidates, Some(password));
    }

    let mut successful_password: Option<Option<String>> = None;
    let mut report = ExtractReport {
        nested_warnings: Vec::new(),
    };
    for candidate in candidates {
        temp_dir.prepare()?;
        let mut attempt_report = ExtractReport {
            nested_warnings: Vec::new(),
        };
        match extract_recursive(
            archive_path,
            temp_dir.path(),
            candidate.as_deref(),
            &mut attempt_report,
        ) {
            Ok(()) => {
                successful_password = Some(candidate);
                report = attempt_report;
                break;
            }
            Err(LiquiModError::WrongPassword(_)) | Err(LiquiModError::PasswordRequired(_)) => {
                temp_dir.clean()?;
            }
            Err(error) => return Err(error),
        }
    }

    let Some(password) = successful_password else {
        return Ok(InstallOutcome::NeedsPassword);
    };
    if let Some(password) = password {
        password_book.learn(&password)?;
    }

    let content_root = resolve_content_root(temp_dir.path())?;
    let _install_lock = INSTALL_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let destination = if is_valid_segment(character) && is_valid_segment(&name) {
        Some(library.layout.mod_dir(character, &name))
    } else {
        None
    };
    let destination_existed = match destination.as_ref() {
        None => true,
        Some(destination) => match std::fs::symlink_metadata(destination) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => true,
        },
    };
    let entry = match library.add_folder(&content_root, character, &name) {
        Ok(entry) => entry,
        Err(error) => {
            if !destination_existed {
                if let Some(destination) = destination {
                    let _ = std::fs::remove_dir_all(destination);
                }
            }
            return Err(error);
        }
    };
    Ok(InstallOutcome::Installed {
        mod_id: entry.id,
        name: entry.name,
        warnings: report.nested_warnings,
    })
}

fn add_candidate(candidates: &mut Vec<Option<String>>, candidate: Option<String>) {
    if !candidates
        .iter()
        .any(|existing| existing.as_deref() == candidate.as_deref())
    {
        candidates.push(candidate);
    }
}

struct TempExtractionDir {
    path: PathBuf,
}

impl TempExtractionDir {
    fn new(app_data: &Path) -> Self {
        Self {
            path: app_data
                .to_path_buf()
                .join("tmp")
                .join(format!("liquimod-{}", Uuid::new_v4())),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn prepare(&self) -> Result<()> {
        self.clean()?;
        std::fs::create_dir_all(&self.path)?;
        Ok(())
    }

    fn clean(&self) -> Result<()> {
        match std::fs::remove_dir_all(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for TempExtractionDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::PasswordBook;
    use crate::error::LiquiModError;
    use crate::library::Library;
    use std::fs::File;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Barrier, Mutex};
    use std::thread;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn setup() -> (tempfile::TempDir, Library) {
        let tmp = tempfile::tempdir().unwrap();
        let library = Library::init(&tmp.path().join("library")).unwrap();
        (tmp, library)
    }

    fn write_zip(path: &Path, files: &[(&str, &[u8])], password: Option<&str>) {
        let file = File::create(path).unwrap();
        let mut writer = ZipWriter::new(file);
        for (name, contents) in files {
            let options = match password {
                Some(password) => {
                    SimpleFileOptions::default().with_aes_encryption(zip::AesMode::Aes256, password)
                }
                None => SimpleFileOptions::default(),
            };
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    fn zip_bytes(files: &[(&str, &[u8])], password: Option<&str>) -> Vec<u8> {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("nested.zip");
        write_zip(&path, files, password);
        std::fs::read(path).unwrap()
    }

    fn install_dirs(library: &Library) -> Vec<PathBuf> {
        let tmp_root = library.layout.root.join("tmp");
        let Ok(entries) = std::fs::read_dir(tmp_root) else {
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("liquimod-"))
            .map(|entry| entry.path())
            .collect()
    }

    #[test]
    fn installs_plain_zip_into_library() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("PlainMod.zip");
        write_zip(&archive, &[("mod.ini", b"[Constants]")], None);

        let outcome = install_archive(&library.db, &library, &archive, "Firefly", None).unwrap();

        let InstallOutcome::Installed {
            mod_id,
            name,
            warnings,
        } = outcome
        else {
            panic!("expected installed outcome");
        };
        assert_eq!(name, "PlainMod");
        assert!(mod_id > 0);
        assert!(warnings.is_empty());
        assert!(library
            .layout
            .mod_dir("Firefly", "PlainMod")
            .join("mod.ini")
            .is_file());
        assert_eq!(library.list().unwrap().len(), 1);
        assert!(install_dirs(&library).is_empty());
    }

    #[test]
    fn wrong_book_then_explicit_password_works() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("SecretMod.zip");
        write_zip(&archive, &[("secret.txt", b"secret")], Some("correct"));
        library.db.add_password("wrong").unwrap();

        let outcome =
            install_archive(&library.db, &library, &archive, "Firefly", Some("correct")).unwrap();
        assert!(matches!(outcome, InstallOutcome::Installed { .. }));
        assert_eq!(
            PasswordBook::new(&library.db).candidates().unwrap(),
            ["wrong", "correct"]
        );

        let second = install_archive(&library.db, &library, &archive, "Firefly", None).unwrap();
        assert!(matches!(second, InstallOutcome::Installed { .. }));
        assert_eq!(
            PasswordBook::new(&library.db).candidates().unwrap(),
            ["wrong", "correct"]
        );
    }

    #[test]
    fn all_passwords_fail_returns_needs_password() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("SecretMod.zip");
        write_zip(&archive, &[("secret.txt", b"secret")], Some("correct"));
        library.db.add_password("book-wrong").unwrap();

        let outcome = install_archive(
            &library.db,
            &library,
            &archive,
            "Others",
            Some("explicit-wrong"),
        )
        .unwrap();

        assert!(matches!(outcome, InstallOutcome::NeedsPassword));
        assert!(install_dirs(&library).is_empty());
        assert!(library.list().unwrap().is_empty());
    }

    #[test]
    fn unencrypted_archive_needs_no_password() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("PlainMod.zip");
        write_zip(&archive, &[("mod.ini", b"plain")], None);
        library.db.add_password("unused").unwrap();

        let outcome = install_archive(&library.db, &library, &archive, "Others", None).unwrap();

        assert!(matches!(outcome, InstallOutcome::Installed { .. }));
        assert_eq!(
            PasswordBook::new(&library.db).candidates().unwrap(),
            ["unused"]
        );
    }

    #[test]
    fn temp_dir_cleaned_on_failure() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("not-an-archive.bin");
        std::fs::write(&archive, b"not an archive").unwrap();

        let error = install_archive(&library.db, &library, &archive, "Others", None).unwrap_err();

        assert!(matches!(error, LiquiModError::UnsupportedArchive(_)));
        assert!(install_dirs(&library).is_empty());
    }

    #[test]
    fn failed_add_folder_rolls_back_library_destination() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("PlainMod.zip");
        write_zip(&archive, &[("mod.ini", b"plain")], None);

        let error = install_archive(&library.db, &library, &archive, "bad/name", None).unwrap_err();

        assert!(matches!(error, LiquiModError::InvalidName(_)));
        assert!(install_dirs(&library).is_empty());
        assert!(!library.layout.mod_dir("bad/name", "PlainMod").exists());
        assert!(library.layout.character_dir("bad").read_dir().is_err());
    }

    #[test]
    fn existing_destination_is_preserved_when_copy_fails() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("ExistingMod.zip");
        write_zip(
            &archive,
            &[("readme.txt", b"new"), ("conflict/file.txt", b"new")],
            None,
        );
        let destination = library.layout.mod_dir("Firefly", "ExistingMod");
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(destination.join("marker.txt"), b"keep").unwrap();
        std::fs::write(destination.join("conflict"), b"keep").unwrap();

        let error = install_archive(&library.db, &library, &archive, "Firefly", None).unwrap_err();

        assert!(matches!(error, LiquiModError::Io(_)));
        assert_eq!(
            std::fs::read(destination.join("marker.txt")).unwrap(),
            b"keep"
        );
        assert_eq!(
            std::fs::read(destination.join("conflict")).unwrap(),
            b"keep"
        );
        assert!(install_dirs(&library).is_empty());
    }

    #[test]
    fn new_destination_is_removed_when_database_write_fails() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("DbFailureMod.zip");
        write_zip(&archive, &[("mod.ini", b"plain")], None);
        let connection = rusqlite::Connection::open(library.layout.db_path()).unwrap();
        connection.execute("DROP TABLE mods", []).unwrap();

        let error = install_archive(&library.db, &library, &archive, "Others", None).unwrap_err();

        assert!(matches!(error, LiquiModError::Db(_)));
        assert!(!library.layout.mod_dir("Others", "DbFailureMod").exists());
        assert!(install_dirs(&library).is_empty());
    }

    #[test]
    fn encrypted_nested_archive_warns_and_is_kept() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("OuterMod.zip");
        let nested = zip_bytes(&[("secret.txt", b"secret")], Some("nested-password"));
        write_zip(
            &archive,
            &[("plain.txt", b"plain"), ("nested.zip", nested.as_slice())],
            None,
        );

        let outcome = install_archive(&library.db, &library, &archive, "Others", None).unwrap();

        let InstallOutcome::Installed { warnings, .. } = outcome else {
            panic!("expected installed outcome");
        };
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("nested.zip") && warning.contains("password")));
        let destination = library.layout.mod_dir("Others", "OuterMod");
        assert_eq!(
            std::fs::read_to_string(destination.join("plain.txt")).unwrap(),
            "plain"
        );
        assert!(destination.join("nested.zip").is_file());
        assert!(!std::fs::read_dir(&destination).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("__nested_")));
    }

    #[test]
    fn concurrent_install_of_different_mods_succeeds() {
        let (first_tmp, first_library) = setup();
        let (second_tmp, second_library) = setup();
        let first_archive = first_tmp.path().join("FirstMod.zip");
        let second_archive = second_tmp.path().join("SecondMod.zip");
        write_zip(&first_archive, &[("first.txt", b"first")], None);
        write_zip(&second_archive, &[("second.txt", b"second")], None);
        let first_library = Arc::new(Mutex::new(first_library));
        let second_library = Arc::new(Mutex::new(second_library));
        let barrier = Arc::new(Barrier::new(2));

        let first_handle = {
            let library = Arc::clone(&first_library);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let library = library.lock().unwrap();
                install_archive(&library.db, &library, &first_archive, "Firefly", None)
            })
        };
        let second_handle = {
            let library = Arc::clone(&second_library);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                let library = library.lock().unwrap();
                install_archive(&library.db, &library, &second_archive, "Others", None)
            })
        };

        assert!(matches!(
            first_handle.join().unwrap(),
            Ok(InstallOutcome::Installed { .. })
        ));
        assert!(matches!(
            second_handle.join().unwrap(),
            Ok(InstallOutcome::Installed { .. })
        ));
        let first_library = first_library.lock().unwrap();
        let second_library = second_library.lock().unwrap();
        assert!(first_library
            .layout
            .mod_dir("Firefly", "FirstMod")
            .join("first.txt")
            .is_file());
        assert!(second_library
            .layout
            .mod_dir("Others", "SecondMod")
            .join("second.txt")
            .is_file());
    }

    #[test]
    fn wrapper_dir_unwrapped() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("WrappedMod.zip");
        write_zip(
            &archive,
            &[("FooMod-v1/FooMod/mod.ini", b"[Constants]")],
            None,
        );

        let outcome = install_archive(&library.db, &library, &archive, "Firefly", None).unwrap();

        assert!(matches!(outcome, InstallOutcome::Installed { .. }));
        let destination = library.layout.mod_dir("Firefly", "WrappedMod");
        assert!(destination.join("mod.ini").is_file());
        assert!(!destination.join("FooMod-v1").exists());
        assert!(!destination.join("FooMod").exists());
    }
}
