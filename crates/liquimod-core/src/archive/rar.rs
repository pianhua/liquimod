use std::path::{Component, Path, PathBuf};

use unrar::{
    error::{Code, UnrarError, When},
    Archive,
};

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

fn map_err(path: &Path, encrypted: Option<bool>, error: UnrarError) -> LiquiModError {
    match (error.code, error.when, encrypted) {
        (Code::MissingPassword, _, _) => LiquiModError::PasswordRequired(path.to_path_buf()),
        (Code::BadPassword, _, _) | (Code::BadData, When::Process, Some(true)) => {
            LiquiModError::WrongPassword(path.to_path_buf())
        }
        _ => archive_err(path, error),
    }
}

fn normalize_archive_path(archive_path: &Path) -> PathBuf {
    Archive::new(archive_path)
        .as_first_part()
        .filename()
        .to_path_buf()
}

fn is_safe_entry(path: &Path) -> bool {
    !path.is_absolute()
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
}

/// 解压 RAR 归档到 `dest`，支持加密归档（`password` 为 None 时遇加密归档返回
/// [`LiquiModError::PasswordRequired`]，错误密码返回 [`LiquiModError::WrongPassword`]）。
///
/// 失败语义：
/// - 解压非原子：中途失败时 `dest` 可能残留部分已解压文件，清理由调用方负责（Task 7 编排层处理）。
/// - `unrar` 0.5.8 将条目文件名暴露为 [`std::path::PathBuf`]，本实现不要求文件名可转换为 UTF-8。
/// - Rust 层会跳过含 `..` 组件、绝对路径或盘符前缀的条目，避免将不安全文件名传给解压函数。
/// - `unrar` 0.5.8 的 Unix 后端会将原始条目路径拼接到目标目录且不执行安全连接检查；Windows 原生后端会
///   通过 `ConvertPath` 清理路径组件。本实现仍保留 Rust 层防护以保持跨平台安全语义。
pub fn extract_rar(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()> {
    let normalized_archive_path = normalize_archive_path(archive_path);
    let mut archive = match password {
        Some(pw) => Archive::with_password(&normalized_archive_path, pw).open_for_processing(),
        None => Archive::new(&normalized_archive_path).open_for_processing(),
    }
    .map_err(|error| map_err(archive_path, None, error))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|error| map_err(archive_path, None, error))?
    {
        let encrypted = header.entry().is_encrypted();
        archive = if !is_safe_entry(&header.entry().filename) {
            header
                .skip()
                .map_err(|error| map_err(archive_path, Some(encrypted), error))?
        } else if header.entry().is_file() {
            header
                .extract_with_base(dest)
                .map_err(|error| map_err(archive_path, Some(encrypted), error))?
        } else {
            header
                .skip()
                .map_err(|error| map_err(archive_path, Some(encrypted), error))?
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{extract_rar, is_safe_entry, map_err, normalize_archive_path};
    use crate::error::LiquiModError;
    use std::path::{Path, PathBuf};
    use unrar::error::{Code, UnrarError, When};

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn garbage_rar_returns_archive_error() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("garbage.rar");
        std::fs::write(&archive, b"not a rar file").unwrap();
        let dest = tempfile::tempdir().unwrap();

        let result = extract_rar(&archive, dest.path(), None);

        assert!(matches!(result, Err(LiquiModError::Archive { .. })));
    }

    #[test]
    fn normalizes_non_first_volume_to_first_part() {
        assert_eq!(
            normalize_archive_path(Path::new("foo.part2.rar")),
            PathBuf::from("foo.part1.rar")
        );
        assert_eq!(
            normalize_archive_path(Path::new("foo.r02")),
            PathBuf::from("foo.r01")
        );
        assert_eq!(
            normalize_archive_path(Path::new("foo.rar")),
            PathBuf::from("foo.rar")
        );
    }

    #[test]
    fn opens_first_part_for_non_first_volume() {
        let dir = tempfile::tempdir().unwrap();
        let part2 = dir.path().join("foo.part2.rar");
        std::fs::write(&part2, b"not a rar file").unwrap();
        let dest = tempfile::tempdir().unwrap();

        let error = extract_rar(&part2, dest.path(), None).unwrap_err();

        match error {
            LiquiModError::Archive { source, .. } => {
                assert_eq!(source.to_string(), "Could not open archive");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn bad_data_process_maps_only_encrypted_entries_to_wrong_password() {
        let path = Path::new("archive.rar");

        assert!(matches!(
            map_err(
                path,
                Some(true),
                UnrarError::from(Code::BadData, When::Process)
            ),
            LiquiModError::WrongPassword(_)
        ));
        assert!(matches!(
            map_err(
                path,
                Some(false),
                UnrarError::from(Code::BadData, When::Process)
            ),
            LiquiModError::Archive { .. }
        ));
    }

    #[test]
    fn rejects_unsafe_entry_paths() {
        for path in [
            "../evil.txt",
            "dir/../../evil.txt",
            "/absolute/evil.txt",
            r"C:\absolute\evil.txt",
            r"\server\share\evil.txt",
        ] {
            assert!(!is_safe_entry(Path::new(path)), "{path}");
        }

        for path in ["textures/a.dds", "dir/./file.txt"] {
            assert!(is_safe_entry(Path::new(path)), "{path}");
        }
    }

    #[test]
    #[ignore = "needs fixture: rar a plain.rar <dir> (WinRAR CLI)"]
    fn extracts_real_rar() {
        let dest = tempfile::tempdir().unwrap();

        extract_rar(&fixture("plain.rar"), dest.path(), None).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest.path().join("hello.txt")).unwrap(),
            "hello from rar\n"
        );
    }

    #[test]
    #[ignore = "needs fixture: rar a -p<pw> encrypted.rar <dir> (WinRAR CLI)"]
    fn wrong_password_maps() {
        let dest = tempfile::tempdir().unwrap();

        let error = extract_rar(&fixture("encrypted.rar"), dest.path(), Some("wrong")).unwrap_err();

        assert!(matches!(error, LiquiModError::WrongPassword(_)));
    }

    #[test]
    #[ignore = "needs fixture: rar a -p<pw> encrypted.rar <dir> (WinRAR CLI)"]
    fn missing_password_maps() {
        let dest = tempfile::tempdir().unwrap();

        let error = extract_rar(&fixture("encrypted.rar"), dest.path(), None).unwrap_err();

        assert!(matches!(error, LiquiModError::PasswordRequired(_)));
    }
}
