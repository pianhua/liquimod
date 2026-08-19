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

/// 执行游戏与 3Dmigoto 的精细化启动流程
pub fn launch_game(opts: &GameLaunchOptions) -> Result<LaunchResult> {
    if !opts.game_exe.is_file() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("游戏执行文件不存在: {}", opts.game_exe.display()),
        )));
    }
    if !opts.migoto_dir.is_dir() {
        return Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("3Dmigoto 目录不存在: {}", opts.migoto_dir.display()),
        )));
    }

    // 1. 启动前根据模式动态同步更新 d3dx.ini (工作模式与 target 路径)
    let ini_path = opts.migoto_dir.join("d3dx.ini");
    if ini_path.is_file() {
        let _ = update_d3dx_ini_mode(&ini_path, opts.work_mode);
        let _ = update_d3dx_ini_target(&ini_path, &opts.game_exe);
    }

    // 2. 根据是否存在专门的 Loader 选择启动策略
    if let Some(loader) = &opts.loader_exe {
        if loader.is_file() {
            return launch_via_loader(loader, &opts.migoto_dir, opts.delay_ms);
        }
    }

    // 3. 默认原生启动游戏主程序
    launch_native_game(&opts.game_exe, &opts.migoto_dir, opts.delay_ms)
}

/// 通过 3Dmigoto Loader 加载器启动
fn launch_via_loader(loader_path: &Path, work_dir: &Path, delay_ms: u64) -> Result<LaunchResult> {
    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }

    let mut cmd = std::process::Command::new(loader_path);
    cmd.current_dir(work_dir);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        cmd.creation_flags(0x08000000);
    }

    let child = cmd.spawn().map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "启动 3Dmigoto 加载器失败: {}",
            e
        )))
    })?;

    Ok(LaunchResult {
        success: true,
        message: format!(
            "已通过加载器「{}」启动游戏",
            loader_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ),
        pid: Some(child.id()),
    })
}

/// 原生直接启动游戏主程序
fn launch_native_game(game_exe: &Path, _work_dir: &Path, delay_ms: u64) -> Result<LaunchResult> {
    let game_dir = game_exe.parent().unwrap_or_else(|| Path::new("."));

    if delay_ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    }

    let mut cmd = std::process::Command::new(game_exe);
    cmd.current_dir(game_dir);

    let child = cmd.spawn().map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!("启动游戏主程序失败: {}", e)))
    })?;

    Ok(LaunchResult {
        success: true,
        message: format!("已启动游戏进程 (PID: {})", child.id()),
        pid: Some(child.id()),
    })
}
