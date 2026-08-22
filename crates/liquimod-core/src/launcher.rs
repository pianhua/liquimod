//! Rust 原生 3Dmigoto 与崩铁启动管理引擎（支持挂起注入、延迟控制与双工作模式切换）

use crate::d3d::{update_d3dx_ini_mode, update_d3dx_ini_target, MigotoWorkMode};
use crate::error::{LiquiModError, Result};
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLaunchOptions {
    /// 游戏主程序绝对路径 (如 D:\Games\Star Rail Games\StarRail.exe)
    pub game_exe: PathBuf,
    /// 3Dmigoto 根目录路径 (包含 d3dx.ini)
    pub migoto_dir: PathBuf,
    /// XXMI 包中的 3dmloader.dll。
    pub injector_dll: PathBuf,
    /// 目标进程文件名，例如 StarRail.exe。
    pub process_name: String,
    /// 工作模式 (Play / Dev)
    pub work_mode: MigotoWorkMode,
    /// 3Dmigoto DLL 初始化延迟 (毫秒，写入 d3dx.ini 的 [System])。
    pub delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchResult {
    pub success: bool,
    pub message: String,
    pub pid: Option<u32>,
}

/// 安全启动程序，支持普通启动与 Windows UAC 提权自动回退（彻底解决 os error 740）
pub fn spawn_program_with_uac(
    exe_path: &Path,
    work_dir: &Path,
    args: Option<&str>,
    hide_window: bool,
) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        // 1. 先尝试标准进程创建
        let mut cmd = std::process::Command::new(exe_path);
        cmd.current_dir(work_dir);
        if hide_window {
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        if let Some(a) = args {
            if !a.is_empty() {
                cmd.arg(a);
            }
        }

        match cmd.spawn() {
            Ok(_) => Ok(()),
            Err(e)
                if e.raw_os_error() == Some(740)
                    || e.kind() == std::io::ErrorKind::PermissionDenied =>
            {
                // 2. 捕获 740 提权请求，通过 Windows Shell 机制唤起系统 UAC 授权弹窗
                let escaped_path = exe_path.to_string_lossy().replace('\'', "''");
                let escaped_dir = work_dir.to_string_lossy().replace('\'', "''");
                let mut ps_cmd = format!(
                    "Start-Process -FilePath '{}' -WorkingDirectory '{}'",
                    escaped_path, escaped_dir
                );
                if let Some(a) = args {
                    if !a.is_empty() {
                        ps_cmd.push_str(&format!(" -ArgumentList '{}'", a.replace('\'', "''")));
                    }
                }
                let output = std::process::Command::new("powershell")
                    .args(["-NoProfile", "-NonInteractive", "-Command", &ps_cmd])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output()
                    .map_err(|pe| {
                        LiquiModError::Io(std::io::Error::other(format!(
                            "提权启动程序「{}」失败: {pe}",
                            exe_path.display()
                        )))
                    })?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(LiquiModError::Io(std::io::Error::other(format!(
                        "提权启动「{}」失败: {stderr}",
                        exe_path.display()
                    ))));
                }
                Ok(())
            }
            Err(e) => Err(LiquiModError::Io(e)),
        }
    }

    #[cfg(not(windows))]
    {
        let mut cmd = std::process::Command::new(exe_path);
        cmd.current_dir(work_dir);
        if let Some(a) = args {
            if !a.is_empty() {
                cmd.arg(a);
            }
        }
        cmd.spawn().map_err(|e| LiquiModError::Io(e))?;
        Ok(())
    }
}

/// 执行与 XXMI 一致的原生 Hook 启动流程：
/// 1. 自动同步 d3dx.ini 模式与 target 游戏路径
/// 2. Hook `d3d11.dll`，再直接拉起游戏主程序
/// 3. 早期/晚期各校验一次 3Dmigoto 注入结果
/// 4. 始终释放全局 Hook，不启动外部 Loader.exe
pub fn launch_with_mod(opts: &GameLaunchOptions) -> Result<LaunchResult> {
    if !opts.game_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("游戏执行文件不存在: {}", opts.game_exe.display()),
        )));
    }

    if !opts.migoto_dir.is_dir() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("3DMigoto 工作区不存在: {}", opts.migoto_dir.display()),
        )));
    }
    let d3dx_ini = opts.migoto_dir.join("d3dx.ini");
    let d3d11 = opts.migoto_dir.join("d3d11.dll");
    if !d3dx_ini.is_file() || !d3d11.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "标准 3DMigoto 工作区不完整，请先安装/修复 XXMI 与 SRMI 核心",
        )));
    }
    if d3dx_ini.is_file() {
        update_d3dx_ini_mode(&d3dx_ini, opts.work_mode)?;
        let target_name = opts
            .game_exe
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| opts.game_exe.clone());
        update_d3dx_ini_target(&d3dx_ini, &target_name)?;
    }

    #[cfg(windows)]
    {
        if !opts.injector_dll.is_file() {
            return Err(LiquiModError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!(
                    "未找到 XXMI 原生注入器：{}，请先安装 XXMI 核心",
                    opts.injector_dll.display()
                ),
            )));
        }
        let process_name = if opts.process_name.trim().is_empty() {
            opts.game_exe
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("StarRail.exe")
                .to_string()
        } else {
            opts.process_name.clone()
        };
        tracing::info!(
            game = %opts.game_exe.display(),
            work_dir = %opts.migoto_dir.display(),
            injector = %opts.injector_dll.display(),
            d3d11 = %d3d11.display(),
            process = %process_name,
            "starting game with XXMI native Hook"
        );
        let hook = HookSession::new(&opts.injector_dll, &d3d11, &process_name)?;
        let game_dir = opts.game_exe.parent().unwrap_or_else(|| Path::new("."));
        let child = spawn_game_process(&opts.game_exe, game_dir)?;

        // 与 XXMI 一致：先做早期 Hook 校验，再等待目标进程稳定，最后做一次晚期校验。
        let (early_hooked, early_result) = hook.wait_for_injection(5)?;
        tracing::info!(process = %process_name, hooked = early_hooked, "XXMI early Hook verification completed");
        let window_seen = wait_for_window(&process_name, Duration::from_secs(15));
        tracing::info!(process = %process_name, window_seen, "XXMI game window detection completed");
        let (late_hooked, late_result) = if window_seen {
            hook.wait_for_injection(5)?
        } else {
            (false, -1)
        };
        tracing::info!(process = %process_name, hooked = late_hooked, "XXMI late Hook verification completed");
        if !early_hooked && !late_hooked {
            return Err(LiquiModError::Io(std::io::Error::other(format!(
                "XXMI Hook 未能注入 {process_name}（早期返回码 {early_result}，晚期返回码 {late_result}），请检查游戏权限、核心文件与 d3dx.ini"
            ))));
        }
        let pid = child.as_ref().map(std::process::Child::id);
        Ok(LaunchResult {
            success: true,
            message: format!("XXMI Hook 已完成，3DMigoto 已加载到 {process_name}"),
            pid,
        })
    }

    #[cfg(not(windows))]
    {
        let game_dir = opts.game_exe.parent().unwrap_or_else(|| Path::new("."));
        spawn_program_with_uac(&opts.game_exe, game_dir, None, false)?;
        Ok(LaunchResult {
            success: true,
            message: "已拉起游戏主程序（非 Windows 未执行原生 Hook）".to_string(),
            pid: None,
        })
    }
}

#[cfg(windows)]
type HookLibraryFn =
    unsafe extern "system" fn(*const u16, *mut *mut c_void, *mut *mut c_void) -> i32;

#[cfg(windows)]
type WaitForInjectionFn = unsafe extern "system" fn(*const u16, *const u16, i32) -> i32;

#[cfg(windows)]
type UnhookLibraryFn = unsafe extern "system" fn(*mut *mut c_void, *mut *mut c_void) -> i32;

#[cfg(windows)]
struct HookSession {
    library: libloading::Library,
    hook: *mut c_void,
    mutex: *mut c_void,
    d3d11_path: Vec<u16>,
    process_name: Vec<u16>,
}

#[cfg(windows)]
impl HookSession {
    fn new(injector_dll: &Path, d3d11_path: &Path, process_name: &str) -> Result<Self> {
        tracing::info!(
            injector = %injector_dll.display(),
            d3d11 = %d3d11_path.display(),
            process = %process_name,
            "loading XXMI 3dmloader.dll"
        );
        let library = unsafe { libloading::Library::new(injector_dll) }.map_err(|e| {
            LiquiModError::Io(std::io::Error::other(format!(
                "加载 3dmloader.dll 失败：{e}"
            )))
        })?;
        let d3d11_path = to_wide_path(d3d11_path);
        let process_name = to_wide_str(process_name);
        let mut session = Self {
            library,
            hook: std::ptr::null_mut(),
            mutex: std::ptr::null_mut(),
            d3d11_path,
            process_name,
        };
        let result = unsafe {
            let hook: libloading::Symbol<HookLibraryFn> = session
                .library
                .get(b"HookLibrary\0")
                .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
            hook(
                session.d3d11_path.as_ptr(),
                &mut session.hook,
                &mut session.mutex,
            )
        };
        tracing::info!(
            result,
            hook_null = session.hook.is_null(),
            "XXMI HookLibrary returned"
        );
        if result != 0 || session.hook.is_null() {
            return Err(LiquiModError::Io(std::io::Error::other(format!(
                "3dmloader.dll HookLibrary 失败，错误码 {result}"
            ))));
        }
        Ok(session)
    }

    fn wait_for_injection(&self, timeout_seconds: i32) -> Result<(bool, i32)> {
        let result = unsafe {
            let wait: libloading::Symbol<WaitForInjectionFn> = self
                .library
                .get(b"WaitForInjection\0")
                .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
            wait(
                self.d3d11_path.as_ptr(),
                self.process_name.as_ptr(),
                timeout_seconds,
            )
        };
        tracing::info!(result, timeout_seconds, "XXMI WaitForInjection returned");
        Ok((result == 0, result))
    }

    fn unhook(&mut self) {
        if self.hook.is_null() && self.mutex.is_null() {
            return;
        }
        let _ = unsafe {
            self.library
                .get::<UnhookLibraryFn>(b"UnhookLibrary\0")
                .map(|unhook| unhook(&mut self.hook, &mut self.mutex))
        };
        self.hook = std::ptr::null_mut();
        self.mutex = std::ptr::null_mut();
    }
}

#[cfg(windows)]
impl Drop for HookSession {
    fn drop(&mut self) {
        self.unhook();
    }
}

#[cfg(windows)]
fn to_wide_path(path: &Path) -> Vec<u16> {
    path.to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect()
}

#[cfg(windows)]
fn to_wide_str(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(windows)]
fn spawn_game_process(exe: &Path, work_dir: &Path) -> Result<Option<std::process::Child>> {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_CONSOLE: u32 = 0x00000010;
    const CREATE_DEFAULT_ERROR_MODE: u32 = 0x04000000;

    let mut command = std::process::Command::new(exe);
    command
        .current_dir(work_dir)
        // 与 XXMI 的 Native 启动上下文一致；GUI 游戏不会因此显示命令行窗口。
        .creation_flags(CREATE_NEW_CONSOLE | CREATE_DEFAULT_ERROR_MODE);

    match command.spawn() {
        Ok(child) => Ok(Some(child)),
        Err(error)
            if error.raw_os_error() == Some(740)
                || error.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            spawn_program_with_uac(exe, work_dir, None, false)?;
            Ok(None)
        }
        Err(error) => Err(LiquiModError::Io(error)),
    }
}

#[cfg(windows)]
fn wait_for_window(name: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        let mut system = sysinfo::System::new();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let pids: Vec<u32> = system
            .processes()
            .values()
            .filter(|process| process.name().eq_ignore_ascii_case(name))
            .map(|process| process.pid().as_u32())
            .collect();
        if pids.iter().copied().any(has_visible_window) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

#[cfg(windows)]
fn has_visible_window(pid: u32) -> bool {
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible,
    };

    struct SearchState {
        pid: u32,
        found: bool,
    }

    unsafe extern "system" fn callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let state = &mut *(lparam.0 as *mut SearchState);
        if IsWindowVisible(hwnd).as_bool() {
            let mut window_pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
            if window_pid == state.pid {
                state.found = true;
                return BOOL(0);
            }
        }
        BOOL(1)
    }

    let mut state = SearchState { pid, found: false };
    unsafe {
        let _ = EnumWindows(Some(callback), LPARAM(&mut state as *mut _ as isize));
    }
    state.found
}

#[cfg(not(windows))]
fn has_visible_window(_pid: u32) -> bool {
    true
}

/// 兼容老接口：默认执行带 Mod 启动流程
pub fn launch_game(opts: &GameLaunchOptions) -> Result<LaunchResult> {
    launch_with_mod(opts)
}

/// 原生直接启动游戏主程序（不加载 3DMigoto，纯净游戏模式）
pub fn launch_native_game(game_exe: &Path) -> Result<LaunchResult> {
    if !game_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("游戏主程序不存在: {}", game_exe.display()),
        )));
    }

    let game_dir = game_exe.parent().unwrap_or_else(|| Path::new("."));
    spawn_program_with_uac(game_exe, game_dir, None, false)?;

    Ok(LaunchResult {
        success: true,
        message: "🕹️ 已启动原生纯净游戏".to_string(),
        pid: None,
    })
}

/// 启动官方启动器（如 HoYoPlay / launcher.exe）
pub fn launch_official_launcher(launcher_exe: &Path) -> Result<LaunchResult> {
    if !launcher_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("官方启动器不存在: {}", launcher_exe.display()),
        )));
    }

    let launcher_dir = launcher_exe.parent().unwrap_or_else(|| Path::new("."));
    spawn_program_with_uac(launcher_exe, launcher_dir, None, false)?;

    Ok(LaunchResult {
        success: true,
        message: "🌐 已打开官方启动器".to_string(),
        pid: None,
    })
}
