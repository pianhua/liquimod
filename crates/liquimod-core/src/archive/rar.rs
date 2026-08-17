use std::path::Path;

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

fn map_err(path: &Path, password: Option<&str>, error: UnrarError) -> LiquiModError {
    match (error.code, error.when, password.is_some()) {
        (Code::MissingPassword, _, _) => LiquiModError::PasswordRequired(path.to_path_buf()),
        (Code::BadPassword, _, true) | (Code::BadData, When::Process, true) => {
            LiquiModError::WrongPassword(path.to_path_buf())
        }
        _ => archive_err(path, error),
    }
}

/// 解压 RAR 归档到 `dest`，支持加密归档（`password` 为 None 时遇加密归档返回
/// [`LiquiModError::PasswordRequired`]，错误密码返回 [`LiquiModError::WrongPassword`]）。
///
/// 失败语义：
/// - 解压非原子：中途失败时 `dest` 可能残留部分已解压文件，清理由调用方负责（Task 7 编排层处理）。
/// - `unrar` 0.5.8 将条目文件名暴露为 [`std::path::PathBuf`]，本实现不要求文件名可转换为 UTF-8。
/// - `extract_with_base` 委托原生 UnRAR 处理条目路径；Windows 实现会通过 `ConvertPath` 清理盘符、UNC、`.`
///   和 `..` 路径组件，并检查链接目标安全性，因此路径穿越会被清理而不是由 Rust 包装层显式拒绝。
pub fn extract_rar(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()> {
    let mut archive = match password {
        Some(pw) => Archive::with_password(archive_path, pw).open_for_processing(),
        None => Archive::new(archive_path).open_for_processing(),
    }
    .map_err(|error| map_err(archive_path, password, error))?;

    while let Some(header) = archive
        .read_header()
        .map_err(|error| map_err(archive_path, password, error))?
    {
        archive = if header.entry().is_file() {
            header
                .extract_with_base(dest)
                .map_err(|error| map_err(archive_path, password, error))?
        } else {
            header
                .skip()
                .map_err(|error| map_err(archive_path, password, error))?
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::extract_rar;
    use crate::error::LiquiModError;
    use std::path::PathBuf;

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
    #[ignore = "needs real rar fixture"]
    fn extracts_real_rar() {
        let dest = tempfile::tempdir().unwrap();

        extract_rar(&fixture("plain.rar"), dest.path(), None).unwrap();

        assert_eq!(
            std::fs::read_to_string(dest.path().join("hello.txt")).unwrap(),
            "hello from rar\n"
        );
    }

    #[test]
    #[ignore = "needs real rar fixture"]
    fn wrong_password_maps() {
        let dest = tempfile::tempdir().unwrap();

        let error = extract_rar(&fixture("encrypted.rar"), dest.path(), Some("wrong")).unwrap_err();

        assert!(matches!(error, LiquiModError::WrongPassword(_)));
    }

    #[test]
    #[ignore = "needs real rar fixture"]
    fn missing_password_maps() {
        let dest = tempfile::tempdir().unwrap();

        let error = extract_rar(&fixture("encrypted.rar"), dest.path(), None).unwrap_err();

        assert!(matches!(error, LiquiModError::PasswordRequired(_)));
    }
}
