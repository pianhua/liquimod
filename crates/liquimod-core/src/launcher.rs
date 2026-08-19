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

/// 执行带 3DMigoto Mod 注入的完整启动流程：
/// 1. 自动同步 d3dx.ini 模式与 target 游戏路径
/// 2. 启动 3DMigoto Loader 加载器
/// 3. 等待用户配置的注入缓冲延迟 (delay_ms)
/// 4. 自动拉起游戏主程序
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

    // 2. 若存在 Loader 则先启动 Loader 进入注入准备状态
    let mut loader_pid = None;
    if let Some(loader) = &opts.loader_exe {
        if loader.is_file() {
            let mut cmd = std::process::Command::new(loader);
            cmd.current_dir(&opts.migoto_dir);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }
            if let Ok(child) = cmd.spawn() {
                loader_pid = Some(child.id());
            }
        }
    }

    // 3. 等待注入缓冲延迟 (由用户在设置中自定义，如 500ms)
    if opts.delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(opts.delay_ms));
    }

    // 4. 自动拉起游戏主程序
    let game_dir = opts.game_exe.parent().unwrap_or_else(|| Path::new("."));
    let mut cmd = std::process::Command::new(&opts.game_exe);
    cmd.current_dir(game_dir);
    let child = cmd.spawn().map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!("启动游戏主程序失败: {}", e)))
    })?;

    let msg = if loader_pid.is_some() {
        format!("✨ 3DMigoto 注入就绪 (延时 {}ms)，已成功拉起游戏 (PID: {})", opts.delay_ms, child.id())
    } else {
        format!("已拉起游戏主程序 (PID: {})", child.id())
    };

    Ok(LaunchResult {
        success: true,
        message: msg,
        pid: Some(child.id()),
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
    let mut cmd = std::process::Command::new(game_exe);
    cmd.current_dir(game_dir);

    let child = cmd.spawn().map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!("启动游戏主程序失败: {}", e)))
    })?;

    Ok(LaunchResult {
        success: true,
        message: format!("🕹️ 已启动原生纯净游戏 (PID: {})", child.id()),
        pid: Some(child.id()),
    })
}
