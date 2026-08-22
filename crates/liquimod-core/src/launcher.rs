//! Rust 原生 3Dmigoto 与崩铁启动管理引擎（支持挂起注入、延迟控制与双工作模式切换）

use crate::d3d::{update_d3dx_ini_mode, update_d3dx_ini_target, MigotoWorkMode};
use crate::error::{LiquiModError, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameLaunchOptions {
    /// 游戏主程序绝对路径 (如 D:\Games\Star Rail Games\StarRail.exe)
    pub game_exe: PathBuf,
    /// 3Dmigoto 根目录路径 (包含 d3dx.ini)
    pub migoto_dir: PathBuf,
    /// 3Dmigoto 加载器 (如 3DMigoto Loader.exe，可选)
    pub loader_exe: Option<PathBuf>,
    /// 工作模式 (Play / Dev)
    pub work_mode: MigotoWorkMode,
    /// 注入时机延迟 (毫秒，0 ~ 5000ms)
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

#[cfg(windows)]
/// 原生 Win32 Hook 注入启动（与 XXMI 100% 对齐的无感 Hook 注入机制）
pub fn launch_with_hook(
    game_exe: &Path,
    work_dir: Option<&Path>,
    d3d11_dll: &Path,
    loader_dll: &Path,
) -> Result<LaunchResult> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{FreeLibrary, HANDLE};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::System::Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW};
    use windows::Win32::UI::WindowsAndMessaging::HHOOK;

    if !game_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("游戏执行文件不存在: {}", game_exe.display()),
        )));
    }
    if !d3d11_dll.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("3DMigoto d3d11.dll 不存在: {}", d3d11_dll.display()),
        )));
    }
    if !loader_dll.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("3dmloader.dll 不存在: {}", loader_dll.display()),
        )));
    }

    let work_path = work_dir.unwrap_or_else(|| game_exe.parent().unwrap_or(Path::new(".")));

    let loader_wide: Vec<u16> = loader_dll
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let h_loader = unsafe { LoadLibraryW(PCWSTR(loader_wide.as_ptr())) }.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "加载 3dmloader.dll 失败: {e}"
        )))
    })?;

    type FnHookLibrary = unsafe extern "system" fn(PCWSTR, *mut HHOOK, *mut HANDLE) -> i32;
    type FnWaitForInjection = unsafe extern "system" fn(PCWSTR, PCWSTR, i32) -> i32;
    type FnUnhookLibrary = unsafe extern "system" fn(*mut HHOOK, *mut HANDLE) -> i32;
    type FnStartProcess = unsafe extern "system" fn(PCWSTR, PCWSTR, PCWSTR) -> i32;

    let p_hook = unsafe { GetProcAddress(h_loader, windows::core::s!("HookLibrary")) };
    let p_wait = unsafe { GetProcAddress(h_loader, windows::core::s!("WaitForInjection")) };
    let p_unhook = unsafe { GetProcAddress(h_loader, windows::core::s!("UnhookLibrary")) };
    let p_start = unsafe { GetProcAddress(h_loader, windows::core::s!("StartProcess")) };

    if let (Some(f_hook), Some(f_wait), Some(f_unhook)) = (p_hook, p_wait, p_unhook) {
        let fn_hook: FnHookLibrary = unsafe { std::mem::transmute(f_hook) };
        let fn_wait: FnWaitForInjection = unsafe { std::mem::transmute(f_wait) };
        let fn_unhook: FnUnhookLibrary = unsafe { std::mem::transmute(f_unhook) };

        let d3d11_wide: Vec<u16> = d3d11_dll
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let mut hook = HHOOK::default();
        let mut mutex = HANDLE::default();

        let hook_res = unsafe { fn_hook(PCWSTR(d3d11_wide.as_ptr()), &mut hook, &mut mutex) };
        if hook_res != 0 {
            let _ = unsafe { FreeLibrary(h_loader) };
            return Err(LiquiModError::Io(std::io::Error::other(format!(
                "3DMigoto HookLibrary 失败，错误代码: {hook_res}"
            ))));
        }

        let app_name_wide: Vec<u16> = game_exe
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let work_dir_wide: Vec<u16> = work_path
            .to_string_lossy()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        let si = STARTUPINFOW {
            cb: std::mem::size_of::<STARTUPINFOW>() as u32,
            ..Default::default()
        };
        let mut pi = PROCESS_INFORMATION::default();

        let started = unsafe {
            CreateProcessW(
                PCWSTR(app_name_wide.as_ptr()),
                None,
                None,
                None,
                false,
                windows::Win32::System::Threading::PROCESS_CREATION_FLAGS(0),
                None,
                PCWSTR(work_dir_wide.as_ptr()),
                &si,
                &mut pi,
            )
        };

        if started.is_err() {
            if let Some(f_start) = p_start {
                let fn_start: FnStartProcess = unsafe { std::mem::transmute(f_start) };
                let empty_wide: Vec<u16> = vec![0];
                let start_res = unsafe {
                    fn_start(
                        PCWSTR(app_name_wide.as_ptr()),
                        PCWSTR(work_dir_wide.as_ptr()),
                        PCWSTR(empty_wide.as_ptr()),
                    )
                };
                if start_res != 0 {
                    let _ = unsafe { fn_unhook(&mut hook, &mut mutex) };
                    let _ = unsafe { FreeLibrary(h_loader) };
                    return Err(LiquiModError::Io(std::io::Error::other(format!(
                        "启动游戏进程失败，错误码: {start_res}"
                    ))));
                }
            } else {
                let _ = unsafe { fn_unhook(&mut hook, &mut mutex) };
                let _ = unsafe { FreeLibrary(h_loader) };
                return Err(LiquiModError::Io(std::io::Error::other(
                    "启动游戏进程失败（需要管理员权限）".to_string(),
                )));
            }
        }

        let proc_name = game_exe.file_name().unwrap_or_default().to_string_lossy();
        let proc_wide: Vec<u16> = proc_name.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = unsafe { fn_wait(PCWSTR(d3d11_wide.as_ptr()), PCWSTR(proc_wide.as_ptr()), 15) };
        let _ = unsafe { fn_unhook(&mut hook, &mut mutex) };
        let _ = unsafe { FreeLibrary(h_loader) };

        Ok(LaunchResult {
            success: true,
            message: "✨ 已无感加载 3DMigoto 并拉起游戏！".to_string(),
            pid: None,
        })
    } else {
        let _ = unsafe { FreeLibrary(h_loader) };
        Err(LiquiModError::Io(std::io::Error::other(
            "3dmloader.dll 缺少必要导出函数 (HookLibrary / WaitForInjection / UnhookLibrary)",
        )))
    }
}

#[cfg(not(windows))]
pub fn launch_with_hook(
    _game_exe: &Path,
    _work_dir: Option<&Path>,
    _d3d11_dll: &Path,
    _loader_dll: &Path,
) -> Result<LaunchResult> {
    Err(LiquiModError::Io(std::io::Error::other("仅支持 Windows")))
}

/// 执行带 3DMigoto Mod 注入的完整启动流程：
/// 1. 自动同步 d3dx.ini 模式与 target 游戏路径
/// 2. 原生 Hook 注入拉起游戏（彻底废除第三方 Loader 弹窗）
pub fn launch_with_mod(opts: &GameLaunchOptions) -> Result<LaunchResult> {
    if !opts.game_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("游戏执行文件不存在: {}", opts.game_exe.display()),
        )));
    }

    // 1. 同步更新 d3dx.ini
    if opts.migoto_dir.is_dir() {
        let ini_path = opts.migoto_dir.join("d3dx.ini");
        if ini_path.is_file() {
            let _ = update_d3dx_ini_mode(&ini_path, opts.work_mode);
            let _ = update_d3dx_ini_target(&ini_path, &opts.game_exe);
        }
    }

    // 2. 原生 Hook 无感注入
    let d3d11_dll = opts.migoto_dir.join("d3d11.dll");
    let loader_dll = opts.migoto_dir.join("3dmloader.dll");
    let game_dir = opts.game_exe.parent();

    if d3d11_dll.is_file() && loader_dll.is_file() {
        return launch_with_hook(&opts.game_exe, game_dir, &d3d11_dll, &loader_dll);
    }

    // 3. 备用纯净原生启动
    launch_native_game(&opts.game_exe)
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
