use std::path::Path;

use sevenz_rust2::{decompress_file, decompress_file_with_password, Error, Password};

use crate::error::{LiquiModError, Result};

fn archive_err(
    path: &Path,
    source: impl std::error::Error + Send + Sync + 'static,
) -> LiquiModError {
    LiquiModError::Archive {
        path: path.to_path_buf(),
        source: Box::new(source),
    }
}

fn map_err(path: &Path, e: Error) -> LiquiModError {
    match e {
        Error::PasswordRequired => LiquiModError::PasswordRequired(path.to_path_buf()),
        Error::MaybeBadPassword(_) => LiquiModError::WrongPassword(path.to_path_buf()),
        other => archive_err(path, other),
    }
}

/// 解压 7z 归档到 `dest`，支持 AES-256 加密归档（`password` 为 None 时遇加密归档返回
/// [`LiquiModError::PasswordRequired`]，错误密码返回 [`LiquiModError::WrongPassword`]）。
///
/// 失败语义：
/// - 解压非原子：中途失败时 `dest` 可能残留部分已解压文件，清理由调用方负责（Task 7 编排层处理）。
/// - 路径穿越（如 `../evil.txt`）条目由 sevenz-rust2 的 safe_join 拒绝，冒泡为
///   [`LiquiModError::Archive`]。
pub fn extract_7z(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()> {
    let result = match password {
        Some(pw) => decompress_file_with_password(archive_path, dest, Password::from(pw)),
        None => decompress_file(archive_path, dest),
    };
    result.map_err(|e| map_err(archive_path, e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::LiquiModError;
    use std::path::PathBuf;

    fn make_7z(files: &[(&str, &str)], password: Option<&str>) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        for (name, content) in files {
            let path = src.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, content).unwrap();
        }
        let archive = dir.path().join("test.7z");
        match password {
            Some(pw) => {
                sevenz_rust2::compress_to_path_encrypted(&src, &archive, Password::from(pw))
                    .unwrap();
            }
            None => {
                sevenz_rust2::compress_to_path(&src, &archive).unwrap();
            }
        }
        (dir, archive)
    }

    #[test]
    fn extracts_plain_7z() {
        let (_dir, path) = make_7z(&[("hello.txt", "hi"), ("sub/world.txt", "world")], None);
        let dest = tempfile::tempdir().unwrap();
        extract_7z(&path, dest.path(), None).unwrap();
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
    fn reports_password_required() {
        let (_dir, path) = make_7z(&[("secret.txt", "s")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        let err = extract_7z(&path, dest.path(), None).unwrap_err();
        assert!(matches!(err, LiquiModError::PasswordRequired(_)));
    }

    #[test]
    fn wrong_password_maps() {
        let (_dir, path) = make_7z(&[("secret.txt", "s")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        let err = extract_7z(&path, dest.path(), Some("nope")).unwrap_err();
        assert!(matches!(err, LiquiModError::WrongPassword(_)));
    }

    #[test]
    fn correct_password_extracts() {
        let (_dir, path) = make_7z(&[("secret.txt", "s3cret")], Some("pw"));
        let dest = tempfile::tempdir().unwrap();
        extract_7z(&path, dest.path(), Some("pw")).unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.path().join("secret.txt")).unwrap(),
            "s3cret"
        );
    }

    #[test]
    fn rejects_non_7z_signature() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fake.7z");
        std::fs::write(&path, b"not a 7z file at all").unwrap();
        let dest = tempfile::tempdir().unwrap();
        let err = extract_7z(&path, dest.path(), None).unwrap_err();
        assert!(matches!(err, LiquiModError::Archive { .. }));
    }

    #[test]
    fn rejects_zip_slip_entry() {
        use std::io::Cursor;

        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.7z");
        let mut writer = sevenz_rust2::ArchiveWriter::create(&archive).unwrap();
        writer.set_encrypt_header(false);
        writer
            .push_archive_entry(
                sevenz_rust2::ArchiveEntry::new_file("../evil.txt"),
                Some(Cursor::new(b"pwned")),
            )
            .unwrap();
        writer.finish().unwrap();

        let dest = dir.path().join("dest");
        std::fs::create_dir(&dest).unwrap();
        let err = extract_7z(&archive, &dest, None).unwrap_err();

        assert!(matches!(err, LiquiModError::Archive { .. }));
        assert!(!dir.path().join("evil.txt").exists());
        assert!(!dest.join("evil.txt").exists());
    }
}
