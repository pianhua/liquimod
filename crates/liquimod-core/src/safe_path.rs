use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SafePathError {
    #[error("路径包含父级目录跳转 (..): {0}")]
    ParentDirTraversal(String),
    #[error("路径包含绝对根目录或盘符前缀: {0}")]
    AbsoluteOrPrefixed(String),
    #[error("路径为空或包含非法组件: {0}")]
    InvalidComponent(String),
    #[error("目标路径逃逸出安全根目录 '{root}': '{target}'")]
    EscapesRoot { root: String, target: String },
}

/// 净化相对路径：仅允许 Component::Normal，严禁 ParentDir (..)、RootDir、Prefix (C:)、UNC
pub fn sanitize_relative_path(rel: &Path) -> Result<PathBuf, SafePathError> {
    let mut out = PathBuf::new();
    let mut has_component = false;

    for comp in rel.components() {
        match comp {
            Component::Normal(os_str) => {
                has_component = true;
                out.push(os_str);
            }
            Component::ParentDir => {
                return Err(SafePathError::ParentDirTraversal(rel.display().to_string()));
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(SafePathError::AbsoluteOrPrefixed(rel.display().to_string()));
            }
            Component::CurDir => {
                // 忽略 '.'
            }
        }
    }

    if !has_component {
        return Err(SafePathError::InvalidComponent(rel.display().to_string()));
    }

    Ok(out)
}

/// 确保 relative_path 在 base_root 目录树之内，并返回拼接后的安全路径
pub fn ensure_contained(base_root: &Path, relative: &Path) -> Result<PathBuf, SafePathError> {
    let clean_rel = sanitize_relative_path(relative)?;
    let joined = base_root.join(&clean_rel);

    // 双重校验：通过相对前缀检查确认 containment
    if let Ok(canon_base) = base_root.canonicalize() {
        if let Ok(canon_joined) = joined.canonicalize() {
            if !canon_joined.starts_with(&canon_base) {
                return Err(SafePathError::EscapesRoot {
                    root: base_root.display().to_string(),
                    target: joined.display().to_string(),
                });
            }
        }
    }

    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_clean_normal_relative_path() {
        let p = Path::new("textures/diffuse.dds");
        let safe = sanitize_relative_path(p).unwrap();
        assert_eq!(safe, PathBuf::from("textures/diffuse.dds"));
    }

    #[test]
    fn rejects_parent_dir_traversal() {
        let p = Path::new("../outside.txt");
        assert!(matches!(
            sanitize_relative_path(p),
            Err(SafePathError::ParentDirTraversal(_))
        ));

        let p2 = Path::new("a/b/../../outside.txt");
        assert!(matches!(
            sanitize_relative_path(p2),
            Err(SafePathError::ParentDirTraversal(_))
        ));
    }

    #[test]
    fn rejects_absolute_and_drive_prefixed_paths() {
        let p = Path::new("/etc/passwd");
        assert!(matches!(
            sanitize_relative_path(p),
            Err(SafePathError::AbsoluteOrPrefixed(_))
        ));

        let p_win = Path::new("C:\\Windows\\System32");
        assert!(matches!(
            sanitize_relative_path(p_win),
            Err(SafePathError::AbsoluteOrPrefixed(_))
        ));
    }
}
