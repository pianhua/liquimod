use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeployStrategy {
    Junction,
    /// The selected paths cannot use a Junction. Copy deployment is not implemented.
    CopyFallback,
}

impl DeployStrategy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Junction => "NTFS 极速软链接模式",
            Self::CopyFallback => "暂不支持：需要同卷 NTFS/ReFS Junction",
        }
    }
}

/// 返回路径所在卷的文件系统名称。Windows 上使用 GetVolumeInformationW，
/// 其它平台返回 None，让调用方保持原有 Junction 行为。
pub fn filesystem_name(path: &Path) -> Option<String> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::{GetVolumeInformationW, GetVolumePathNameW};

        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let input = absolute
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut root = [0u16; 260];
        let root_ok = unsafe { GetVolumePathNameW(PCWSTR(input.as_ptr()), &mut root).is_ok() };
        if !root_ok {
            return None;
        }
        let mut fs_name = [0u16; 64];
        let ok = unsafe {
            GetVolumeInformationW(
                PCWSTR(root.as_ptr()),
                None,
                None,
                None,
                None,
                Some(&mut fs_name),
            )
            .is_ok()
        };
        if !ok {
            return None;
        }
        let len = fs_name
            .iter()
            .position(|v| *v == 0)
            .unwrap_or(fs_name.len());
        Some(String::from_utf16_lossy(&fs_name[..len]))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

pub fn same_volume_filesystem(a: &Path, b: &Path) -> Option<String> {
    let left = filesystem_name(a)?;
    let right = filesystem_name(b)?;
    if left.eq_ignore_ascii_case(&right) {
        Some(left)
    } else {
        Some(format!("{} -> {}", left, right))
    }
}

pub fn choose_strategy(library_root: &Path, mods_dir: &Path) -> DeployStrategy {
    #[cfg(windows)]
    {
        let library_fs = filesystem_name(library_root);
        let mods_fs = filesystem_name(mods_dir);
        let junction_safe = matches!(library_fs.as_deref(), Some("NTFS") | Some("ReFS"))
            && matches!(mods_fs.as_deref(), Some("NTFS") | Some("ReFS"))
            && library_fs
                .as_deref()
                .zip(mods_fs.as_deref())
                .map(|(a, b)| a.eq_ignore_ascii_case(b))
                .unwrap_or(false);
        if junction_safe {
            DeployStrategy::Junction
        } else {
            DeployStrategy::CopyFallback
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (library_root, mods_dir);
        DeployStrategy::Junction
    }
}

pub fn volume_root(path: &Path) -> Option<PathBuf> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::core::PCWSTR;
        use windows::Win32::Storage::FileSystem::GetVolumePathNameW;
        let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        let input = absolute
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut root = [0u16; 260];
        unsafe {
            GetVolumePathNameW(PCWSTR(input.as_ptr()), &mut root).ok()?;
        }
        let len = root.iter().position(|v| *v == 0).unwrap_or(root.len());
        Some(PathBuf::from(String::from_utf16_lossy(&root[..len])))
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strategy_has_user_facing_labels() {
        assert!(DeployStrategy::Junction.label().contains("软链接"));
        assert!(DeployStrategy::CopyFallback.label().contains("暂不支持"));
    }
}
