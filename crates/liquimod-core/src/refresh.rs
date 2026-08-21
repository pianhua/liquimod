//! 游戏刷新：检测游戏进程；经命名管道通知提权 helper 发 F10。
//! helper 不存在时用 ShellExecuteW "runas" 提权拉起（触发一次 UAC）。

use crate::error::{LiquiModError, Result};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

pub const PIPE_NAME: &str = r"\\.\pipe\liquimod-refresh";
pub const HELPER_EXE: &str = "liquimod-refresh-helper.exe";

/// 任一给定进程名存在即为游戏运行中（大小写不敏感，免分配比较）。
pub fn is_game_running(process_names: &[&str]) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        process_names
            .iter()
            .any(|n| p.name().eq_ignore_ascii_case(*n))
    })
}

/// 持有管道双工端 = app 生命周期；Drop 即断开，helper 随之退出。
pub struct RefreshClient {
    pipe: File,
}

impl RefreshClient {
    /// 连接已运行的 helper；否则 runas 提权拉起并等待管道就绪（最多 5s）。
    ///
    /// # 阻塞性
    /// 本方法会阻塞调用线程：可能跨越整个 UAC 弹窗期间，外加最多 5s 的管道轮询。
    /// **必须**从阻塞/工作线程调用（如 `spawn_blocking`），切勿在 async 或主 UI 线程上调用。
    ///
    /// # 单客户端管道
    /// 管道仅支持单一客户端：第一个 app 实例持有时，第二个实例会在轮询超时后得到
    /// `TimedOut` 错误，属可接受的 fail-fast 路径。
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
        OpenOptions::new().read(true).write(true).open(PIPE_NAME)
    }

    /// 通知 helper 发一次 F10。
    ///
    /// 同一 read 批次内到达的多次 poke 会合并为单次 F10（helper 按"批次含 b'1'"触发）。
    pub fn poke(&mut self) -> Result<()> {
        self.pipe.write_all(b"1")?;
        self.pipe.flush()?;
        wait_for_pipe_reply(&self.pipe, Duration::from_secs(2))?;
        let mut ack = [0u8; 1];
        self.pipe.read_exact(&mut ack)?;
        if ack[0] == b'1' {
            Ok(())
        } else {
            Err(LiquiModError::Io(std::io::Error::other(
                "未找到或无法聚焦游戏窗口，F10 未发送",
            )))
        }
    }

    /// 通知 helper 以 3DMigoto Hook 一键启动游戏
    pub fn launch_game(
        &mut self,
        game_exe: &Path,
        work_dir: Option<&Path>,
        d3d11_dll: Option<&Path>,
        loader_dll: Option<&Path>,
    ) -> Result<()> {
        let exe_str = game_exe.to_string_lossy();
        let dir_str = work_dir.map(|p| p.to_string_lossy()).unwrap_or_default();
        let d3d_str = d3d11_dll.map(|p| p.to_string_lossy()).unwrap_or_default();
        let loader_str = loader_dll.map(|p| p.to_string_lossy()).unwrap_or_default();

        let cmd = format!(
            "LAUNCH|{}|{}|{}|{}\n",
            exe_str, dir_str, d3d_str, loader_str
        );
        self.pipe.write_all(cmd.as_bytes())?;
        self.pipe.flush()?;
        wait_for_pipe_reply(&self.pipe, Duration::from_secs(15))?;
        let mut ack = [0u8; 2];
        self.pipe.read_exact(&mut ack)?;
        if &ack == b"L1" {
            Ok(())
        } else {
            Err(LiquiModError::Io(std::io::Error::other(
                "游戏启动或 3DMigoto 注入失败",
            )))
        }
    }
}

#[cfg(windows)]
fn wait_for_pipe_reply(pipe: &File, timeout: Duration) -> std::io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Pipes::PeekNamedPipe;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        let mut available = 0;
        let handle = HANDLE(pipe.as_raw_handle());
        unsafe { PeekNamedPipe(handle, None, 0, None, Some(&mut available), None) }
            .map_err(std::io::Error::other)?;
        if available > 0 {
            return Ok(());
        }
        if std::time::Instant::now() >= deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "刷新 helper 响应超时",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(not(windows))]
fn wait_for_pipe_reply(_pipe: &File, _timeout: Duration) -> std::io::Result<()> {
    Ok(())
}

/// 启动外部可执行文件：
/// 1. 优先使用 ShellExecuteW("open") 启动，工作目录设在 exe 所在文件夹；
/// 2. 若遇 SE_ERR_ACCESSDENIED=5，自动以 "runas" 动词请求 UAC 提权；
/// 3. 若启动成功返回 Ok(())，若用户取消或文件异常返回清晰错误。
#[cfg(windows)]
pub fn launch_program(exe: &Path) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SHOW_WINDOW_CMD;

    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let open_verb: Vec<u16> = "open\0".encode_utf16().collect();
    let runas_verb: Vec<u16> = "runas\0".encode_utf16().collect();
    let file_path: Vec<u16> = format!("{}\0", exe.display()).encode_utf16().collect();
    let dir_path: Vec<u16> = format!("{}\0", dir.display()).encode_utf16().collect();

    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(open_verb.as_ptr()),
            PCWSTR(file_path.as_ptr()),
            PCWSTR::null(),
            PCWSTR(dir_path.as_ptr()),
            SHOW_WINDOW_CMD(1), // SW_SHOWNORMAL
        )
    };

    if r.0 as usize > 32 {
        return Ok(());
    }

    if r.0 as usize == 5 {
        let r_runas = unsafe {
            ShellExecuteW(
                None,
                PCWSTR(runas_verb.as_ptr()),
                PCWSTR(file_path.as_ptr()),
                PCWSTR::null(),
                PCWSTR(dir_path.as_ptr()),
                SHOW_WINDOW_CMD(1),
            )
        };
        if r_runas.0 as usize > 32 {
            return Ok(());
        }
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "程序启动被拒绝或未授权管理员权限 (code {})",
            r_runas.0 as usize
        ))));
    }

    Err(LiquiModError::Io(std::io::Error::other(format!(
        "启动程序失败「{}」(code {})",
        exe.display(),
        r.0 as usize
    ))))
}

#[cfg(not(windows))]
pub fn launch_program(exe: &Path) -> Result<()> {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    std::process::Command::new(exe)
        .current_dir(dir)
        .spawn()
        .map_err(|e| LiquiModError::Io(e))?;
    Ok(())
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

/// 游戏运行状态看门狗：低频轮询进程生命周期，只在状态变化时回调。
pub struct GameWatchdog {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}

impl GameWatchdog {
    pub fn start<F>(process_names: Vec<String>, interval: Duration, mut on_change: F) -> Self
    where
        F: FnMut(bool) + Send + 'static,
    {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_thread = Arc::clone(&stop);
        let join = thread::spawn(move || {
            let mut last = None;
            while !stop_thread.load(Ordering::Relaxed) {
                let names: Vec<&str> = process_names.iter().map(String::as_str).collect();
                let running = is_game_running(&names);
                if last != Some(running) {
                    on_change(running);
                    last = Some(running);
                }

                let mut elapsed = Duration::ZERO;
                while elapsed < interval && !stop_thread.load(Ordering::Relaxed) {
                    let slice = (interval - elapsed).min(Duration::from_millis(100));
                    thread::sleep(slice);
                    elapsed += slice;
                }
            }
        });
        Self {
            stop,
            join: Some(join),
        }
    }
}

impl Drop for GameWatchdog {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

#[cfg(test)]
mod watchdog_tests {
    use super::GameWatchdog;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[test]
    fn watchdog_reports_initial_state_and_stops_cleanly() {
        let states = Arc::new(Mutex::new(Vec::new()));
        let observed = Arc::clone(&states);
        let watchdog = GameWatchdog::start(
            vec!["liquimod-process-that-does-not-exist.exe".to_string()],
            Duration::from_millis(20),
            move |running| observed.lock().unwrap().push(running),
        );
        std::thread::sleep(Duration::from_millis(60));
        drop(watchdog);
        assert_eq!(states.lock().unwrap().as_slice(), &[false]);
    }
}
