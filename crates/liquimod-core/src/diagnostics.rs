use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    Pass,
    Warn,
    Fail,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DiagnosticCheck {
    pub id: String,
    pub label: String,
    pub state: CheckState,
    pub detail: String,
    pub remediation: Option<String>,
}

pub const WEBVIEW2_DOWNLOAD_URL: &str =
    "https://developer.microsoft.com/microsoft-edge/webview2/#download-section";

pub fn collect_checks(
    library_root: &Path,
    mods_dir: Option<&Path>,
    game_exe: Option<&Path>,
    loader_exe: Option<&Path>,
    helper_ready: bool,
) -> Vec<DiagnosticCheck> {
    vec![
        check_path("library", "LiquiMod 仓库", library_root, true),
        check_mods_dir(mods_dir),
        check_webview2(),
        check_vc_runtime(),
        check_d3d11(mods_dir, game_exe, loader_exe),
        check_helper(helper_ready),
    ]
}

pub fn defender_exclusion_command(paths: &[&Path]) -> Option<String> {
    let valid = paths
        .iter()
        .filter(|path| path.is_dir())
        .map(|path| format!("'{}'", powershell_quote(path)))
        .collect::<Vec<_>>();
    if valid.is_empty() {
        return None;
    }
    Some(format!(
        "Add-MpPreference -ExclusionPath {}",
        valid.join(", ")
    ))
}

fn check_path(id: &str, label: &str, path: &Path, writable: bool) -> DiagnosticCheck {
    if !path.is_dir() {
        return DiagnosticCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            state: CheckState::Fail,
            detail: format!("目录不存在：{}", path.display()),
            remediation: Some("请在设置中选择或重新创建有效目录。".to_owned()),
        };
    }
    if writable && !probe_write(path) {
        return DiagnosticCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            state: CheckState::Fail,
            detail: format!("目录不可写：{}", path.display()),
            remediation: Some("请检查目录权限、只读属性或安全软件拦截。".to_owned()),
        };
    }
    DiagnosticCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        state: CheckState::Pass,
        detail: format!("目录可用：{}", path.display()),
        remediation: None,
    }
}

fn check_mods_dir(mods_dir: Option<&Path>) -> DiagnosticCheck {
    match mods_dir {
        Some(path) => check_path("mods_dir", "3Dmigoto Mods 目录", path, true),
        None => DiagnosticCheck {
            id: "mods_dir".to_owned(),
            label: "3Dmigoto Mods 目录".to_owned(),
            state: CheckState::Fail,
            detail: "尚未配置 Mods 部署目录".to_owned(),
            remediation: Some("请在设置中选择 3Dmigoto 的 Mods 目录。".to_owned()),
        },
    }
}

fn check_helper(helper_ready: bool) -> DiagnosticCheck {
    DiagnosticCheck {
        id: "refresh_helper".to_owned(),
        label: "F10 热刷新助手".to_owned(),
        state: if helper_ready {
            CheckState::Pass
        } else {
            CheckState::Warn
        },
        detail: if helper_ready {
            "刷新助手已就绪".to_owned()
        } else {
            "未找到刷新助手，启用/安装仍可使用，但不会自动发送 F10".to_owned()
        },
        remediation: (!helper_ready)
            .then_some("重新安装或修复 LiquiMod 发布包中的 refresh-helper。".to_owned()),
    }
}

fn check_webview2() -> DiagnosticCheck {
    #[cfg(windows)]
    {
        let candidates = [
            std::env::var_os("PROGRAMFILES")
                .map(|root| PathBuf::from(root).join("Microsoft/EdgeWebView/Application")),
            std::env::var_os("PROGRAMFILES(X86)")
                .map(|root| PathBuf::from(root).join("Microsoft/EdgeWebView/Application")),
            std::env::var_os("LOCALAPPDATA")
                .map(|root| PathBuf::from(root).join("Microsoft/EdgeWebView/Application")),
        ];
        if candidates
            .into_iter()
            .flatten()
            .any(|path| has_webview_executable(&path))
        {
            return pass(
                "webview2",
                "WebView2 运行时",
                "已检测到 WebView2 Evergreen Runtime",
            );
        }
        DiagnosticCheck {
            id: "webview2".to_owned(),
            label: "WebView2 运行时".to_owned(),
            state: CheckState::Fail,
            detail: "未检测到 WebView2 Runtime，应用界面或升级后的 WebView 组件可能无法启动"
                .to_owned(),
            remediation: Some(format!(
                "请安装 Microsoft WebView2 Evergreen Runtime：{}",
                WEBVIEW2_DOWNLOAD_URL
            )),
        }
    }
    #[cfg(not(windows))]
    {
        unknown(
            "webview2",
            "WebView2 运行时",
            "当前平台不适用 Windows WebView2 检查",
        )
    }
}

fn check_vc_runtime() -> DiagnosticCheck {
    #[cfg(windows)]
    {
        let system_root = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let system32 = system_root.join("System32");
        let runtime_files = ["vcruntime140.dll", "msvcp140.dll"];
        if runtime_files
            .iter()
            .all(|name| system32.join(name).is_file())
        {
            pass(
                "vc_runtime",
                "Microsoft Visual C++ 运行库",
                "已检测到 VC++ 运行库",
            )
        } else {
            DiagnosticCheck {
                id: "vc_runtime".to_owned(),
                label: "Microsoft Visual C++ 运行库".to_owned(),
                state: CheckState::Warn,
                detail: "未能确认 VC++ 2015-2022 运行库完整存在".to_owned(),
                remediation: Some(
                    "请安装或修复 Microsoft Visual C++ 2015-2022 Redistributable (x64)。"
                        .to_owned(),
                ),
            }
        }
    }
    #[cfg(not(windows))]
    {
        unknown(
            "vc_runtime",
            "Microsoft Visual C++ 运行库",
            "当前平台不适用 Windows VC++ 检查",
        )
    }
}

fn check_d3d11(
    mods_dir: Option<&Path>,
    game_exe: Option<&Path>,
    loader_exe: Option<&Path>,
) -> DiagnosticCheck {
    let mut roots = Vec::new();
    for path in [mods_dir, game_exe, loader_exe].into_iter().flatten() {
        if let Some(parent) = path.parent() {
            if !roots.iter().any(|item: &PathBuf| item == parent) {
                roots.push(parent.to_path_buf());
            }
        }
    }
    if let Some(root) = roots.iter().find(|root| root.join("d3d11.dll").is_file()) {
        let path = root.join("d3d11.dll");
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        if size > 64 * 1024 {
            return pass(
                "d3d11",
                "3Dmigoto d3d11.dll",
                &format!("已检测到 {} ({} KB)", path.display(), size / 1024),
            );
        }
        return warn(
            "d3d11",
            "3Dmigoto d3d11.dll",
            "已找到 d3d11.dll，但文件体积异常偏小",
        );
    }
    warn(
        "d3d11",
        "3Dmigoto d3d11.dll",
        "未在已配置工作目录附近找到 d3d11.dll；若使用其他注入方式可忽略",
    )
}

fn has_webview_executable(root: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        path.is_dir() && path.join("msedgewebview2.exe").is_file()
    })
}

fn probe_write(dir: &Path) -> bool {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or_default();
    let path = dir.join(format!(
        ".liquimod-write-test-{}-{}",
        std::process::id(),
        stamp
    ));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(_) => std::fs::remove_file(path).is_ok(),
        Err(_) => false,
    }
}

fn powershell_quote(path: &Path) -> String {
    path.display().to_string().replace('\'', "''")
}

fn pass(id: &str, label: &str, detail: &str) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        state: CheckState::Pass,
        detail: detail.to_owned(),
        remediation: None,
    }
}

fn warn(id: &str, label: &str, detail: &str) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        state: CheckState::Warn,
        detail: detail.to_owned(),
        remediation: None,
    }
}

#[cfg(not(windows))]
fn unknown(id: &str, label: &str, detail: &str) -> DiagnosticCheck {
    DiagnosticCheck {
        id: id.to_owned(),
        label: label.to_owned(),
        state: CheckState::Unknown,
        detail: detail.to_owned(),
        remediation: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writable_directory_check_does_not_leave_probe_file() {
        let dir = tempfile::tempdir().unwrap();
        let check = check_path("library", "库", dir.path(), true);
        assert_eq!(check.state, CheckState::Pass);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }

    #[test]
    fn defender_command_quotes_paths() {
        let dir = tempfile::tempdir().unwrap();
        let command = defender_exclusion_command(&[dir.path()]).unwrap();
        assert!(command.starts_with("Add-MpPreference -ExclusionPath"));
        assert!(command.contains("'") && command.contains(&dir.path().display().to_string()));
    }
}
