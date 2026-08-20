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

/// 执行带 3DMigoto Mod 注入的完整启动流程：
/// 1. 自动同步 d3dx.ini 模式与 target 游戏路径
/// 2. 提权/安全启动 3DMigoto Loader 加载器
/// 3. 等待用户配置的注入缓冲延迟 (delay_ms)
/// 4. 提权/安全拉起游戏主程序
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

    // 2. 若存在 Loader 则先启动 Loader 进入注入准备状态 (支持提权)
    let mut loader_started = false;
    if let Some(loader) = &opts.loader_exe {
        if loader.is_file() && spawn_program_with_uac(loader, &opts.migoto_dir, None, true).is_ok()
        {
            loader_started = true;
        }
    }

    // 3. 等待注入缓冲延迟 (由用户在设置中自定义，如 500ms)
    if opts.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(opts.delay_ms));
    }

    // 4. 自动提权/安全拉起游戏主程序
    let game_dir = opts.game_exe.parent().unwrap_or_else(|| Path::new("."));
    spawn_program_with_uac(&opts.game_exe, game_dir, None, false)?;

    let msg = if loader_started {
        format!(
            "✨ 3DMigoto 注入就绪 (延时 {}ms)，已拉起游戏！",
            opts.delay_ms
        )
    } else {
        "已拉起游戏主程序".to_string()
    };

    Ok(LaunchResult {
        success: true,
        message: msg,
        pid: None,
    })
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
