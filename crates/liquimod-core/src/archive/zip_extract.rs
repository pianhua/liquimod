use std::fs::File;
use std::io;
use std::path::Path;

use zip::result::ZipError;
use zip::ZipArchive;

use crate::error::{LiquiModError, Result};

fn archive_err(path: &Path, source: impl std::error::Error + Send + Sync + 'static) -> LiquiModError {
    LiquiModError::Archive {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

pub fn extract_zip(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()> {
    let file = File::open(archive_path).map_err(|e| archive_err(archive_path, e))?;
    let mut archive = ZipArchive::new(file).map_err(|e| archive_err(archive_path, e))?;

    let mut encrypted_flags = Vec::with_capacity(archive.len());
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| archive_err(archive_path, e))?;
        encrypted_flags.push(entry.encrypted());
    }
    if password.is_none() && encrypted_flags.iter().any(|&e| e) {
        return Err(LiquiModError::PasswordRequired(archive_path.to_path_buf()));
    }

    for (i, &is_encrypted) in encrypted_flags.iter().enumerate() {
        let mut entry = if is_encrypted {
            let pw = password.expect("encrypted entries require a password");
            archive
                .by_index_decrypt(i, pw.as_bytes())
                .map_err(|e| match e {
                    ZipError::InvalidPassword => {
                        LiquiModError::WrongPassword(archive_path.to_path_buf())
                    }
                    other => archive_err(archive_path, other),
                })?
        } else {
            archive
                .by_index(i)
                .map_err(|e| archive_err(archive_path, e))?
        };

        let Some(name) = entry.enclosed_name() else {
            continue;
        };
        let outpath = dest.join(name);

        if entry.is_dir() {
            std::fs::create_dir_all(&outpath).map_err(|e| archive_err(archive_path, e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(|e| archive_err(archive_path, e))?;
            }
            let mut outfile = File::create(&outpath).map_err(|e| archive_err(archive_path, e))?;
            io::copy(&mut entry, &mut outfile).map_err(|e| archive_err(archive_path, e))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LiquiModError;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use zip::write::SimpleFileOptions;
    use zip::ZipWriter;

    fn make_zip(files: &[(&str, &str)], password: Option<&str>) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.zip");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        for (name, content) in files {
            let options = match password {
                Some(pw) => {
                    SimpleFileOptions::default().with_aes_encryption(zip::AesMode::Aes256, pw)
                }
                None => SimpleFileOptions::default(),
            };
            writer.start_file(*name, options).unwrap();
            writer.write_all(content.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        (dir, path)
    }

    #[test]
    fn extracts_plain_zip() {
        let (_dir, path) = make_zip(&[("hello.txt", "hi"), ("sub/world.txt", "world")], None);
        let dest = tempfile::tempdir().unwrap();
        extract_zip(&path, dest.path(), None).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("hello.txt")).unwrap(),
            "hi"
        );
        assert_eq!(
            std::fs::read_to_string(dest.path().join("sub/world.txt")).unwrap(),
            "world"
        );
    }

    #[test]
    fn reports_password_required_for_encrypted() {
        let (_dir, path) = make_zip(&[("secret.txt", "s")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        let err = extract_zip(&path, dest.path(), None).unwrap_err();
        assert!(matches!(err, LiquiModError::PasswordRequired(_)));
    }

    #[test]
    fn wrong_password_maps_to_wrong_password() {
        let (_dir, path) = make_zip(&[("secret.txt", "s")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        let err = extract_zip(&path, dest.path(), Some("nope")).unwrap_err();
        assert!(matches!(err, LiquiModError::WrongPassword(_)));
    }

    #[test]
    fn correct_password_extracts() {
        let (_dir, path) = make_zip(&[("secret.txt", "s3cret")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        extract_zip(&path, dest.path(), Some("pw")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("secret.txt")).unwrap(),
            "s3cret"
        );
    }

    #[test]
    fn rejects_zip_slip_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evil.zip");
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("../evil.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"pwned").unwrap();
        writer.finish().unwrap();

        let dest_parent = tempfile::tempdir().unwrap();
        let dest = dest_parent.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        extract_zip(&path, &dest, None).unwrap();
        assert!(!dest_parent.path().join("evil.txt").exists());
        assert!(!dest.join("evil.txt").exists());
    }
}
