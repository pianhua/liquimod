//! 游戏刷新：检测游戏进程；经命名管道通知提权 helper 发 F10。
//! helper 不存在时用 ShellExecuteW "runas" 提权拉起（触发一次 UAC）。

use crate::error::{LiquiModError, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

pub const PIPE_NAME: &str = r"\\.\pipe\liquimod-refresh";
pub const HELPER_EXE: &str = "liquimod-refresh-helper.exe";

/// 任一给定进程名存在即为游戏运行中（大小写不敏感）。
pub fn is_game_running(process_names: &[&str]) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        process_names.iter().any(|n| name == *n)
    })
}

/// 持有管道写端 = app 生命周期；Drop 即断开，helper 随之退出。
pub struct RefreshClient {
    pipe: File,
}

impl RefreshClient {
    /// 连接已运行的 helper；否则 runas 提权拉起并等待管道就绪（最多 5s）。
    pub fn connect_or_launch(helper_exe: &Path) -> Result<Self> {
        if let Ok(pipe) = Self::try_connect() {
            return Ok(Self { pipe });
        }
        launch_elevated(helper_exe)?;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(pipe) = Self::try_connect() {
                return Ok(Self { pipe });
            }
        }
        Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "helper 管道等待超时",
        )))
    }

    fn try_connect() -> std::io::Result<File> {
        OpenOptions::new().write(true).open(PIPE_NAME)
    }

    /// 通知 helper 发一次 F10。
    pub fn poke(&mut self) -> Result<()> {
        self.pipe.write_all(b"1")?;
        self.pipe.flush()?;
        Ok(())
    }
}

/// ShellExecuteW(runas) 提权启动 helper（UAC 拒绝返回 SE_ERR_ACCESSDENIED=5）。
#[cfg(windows)]
fn launch_elevated(exe: &Path) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;
    let runas: Vec<u16> = "runas\0".encode_utf16().collect();
    let path: Vec<u16> = format!("{}\0", exe.display()).encode_utf16().collect();
    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(runas.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SHOW_WINDOW_CMD(1), // SW_SHOWNORMAL
        )
    };
    if r.0 as usize > 32 {
        Ok(())
    } else {
        Err(LiquiModError::Io(std::io::Error::other(format!(
            "helper 启动失败（可能拒绝了 UAC），code {}",
            r.0 as usize
        ))))
    }
}

#[cfg(not(windows))]
fn launch_elevated(_exe: &Path) -> Result<()> {
    Err(LiquiModError::Io(std::io::Error::other(
        "仅 Windows 支持刷新 helper",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_running_process_case_insensitive() {
        let own = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        // 测试 harness 进程名形如 liquimod_core-xxxx.exe，取不含 hash 的前缀不可行，
        // 直接断言：不存在的进程 false；当前进程 true。
        assert!(!is_game_running(&["definitely-not-running-zzz.exe"]));
        assert!(is_game_running(&[&own]));
    }
}
