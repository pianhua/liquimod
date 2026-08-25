use super::*;

#[tauri::command]
pub fn get_game_status(state: tauri::State<AppState>) -> GameStatusDto {
    GameStatusDto {
        running: state.game_running.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub async fn auto_detect_game_exe() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = liquimod_core::discovery::auto_detect_game_exe();
        Ok(found.map(|p| p.display().to_string()))
    })
    .await
    .map_err(|e| format!("自动探测任务异常: {e}"))?
}

#[tauri::command]
pub async fn launch_game(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    if state
        .launch_in_progress
        .swap(true, std::sync::atomic::Ordering::AcqRel)
    {
        return Err("已有模组启动任务正在进行，请稍候".to_string());
    }
    let refresh = std::sync::Arc::clone(&state.refresh);
    let (game_path, work_mode, delay_ms, data_root) = {
        let config = lock_mutex(&state.config, "config")?;
        let game_path = config
            .game_exe
            .clone()
            .ok_or_else(|| "未配置游戏主程序路径，请在设置中配置或点击自动探测".to_string());
        let game_path = match game_path {
            Ok(path) => path,
            Err(error) => {
                state
                    .launch_in_progress
                    .store(false, std::sync::atomic::Ordering::Release);
                return Err(error);
            }
        };
        let work_mode = match config.work_mode.as_str() {
            "dev" => liquimod_core::d3d::MigotoWorkMode::Dev,
            _ => liquimod_core::d3d::MigotoWorkMode::Play,
        };
        (
            game_path,
            work_mode,
            config.injection_delay_ms,
            config.data_root(),
        )
    };
    let pin =
        match lock_mutex(&state.config, "config").and_then(|config| current_launch_pin(&config)) {
            Ok(pin) => pin,
            Err(error) => {
                state
                    .launch_in_progress
                    .store(false, std::sync::atomic::Ordering::Release);
                return Err(error);
            }
        };
    let Some(helper) = refresh_helper_path() else {
        state
            .launch_in_progress
            .store(false, std::sync::atomic::Ordering::Release);
        return Err("未找到刷新 helper，无法启动注入游戏".to_string());
    };

    let task_result = tauri::async_runtime::spawn_blocking(move || {
        // 普通权限 UI 先准备并同步托管运行时；提权 helper 只从钉扎 data-root
        // 解析运行时路径并执行 Hook 生命周期，不再在 helper 中接受任意 DLL 路径。
        liquimod_core::migoto_sync::prepare_managed_runtime(
            &data_root, &game_path, work_mode, delay_ms,
        )
        .map_err(|e| format!("准备 XXMI/SRMI 运行时失败：{e}"))?;
        let mut client = {
            let mut guard = lock_mutex(&refresh, "refresh")?;
            if guard.as_ref().is_some_and(|client| client.pin() != &pin) {
                // 配置中的游戏路径或数据根目录已经变化；旧 helper 的 argv 钉扎不能复用。
                *guard = None;
            }
            guard.take()
        };
        if client.is_none() {
            client = Some(
                RefreshClient::connect_or_launch(&helper, pin.clone())
                    .map_err(|e| format!("刷新 helper 启动失败：{e}"))?,
            );
        }
        let mut client = client.ok_or_else(|| "刷新 helper 连接未建立".to_string())?;
        let mut stage_error = None;
        let result = client.launch_game(&mut |stage| {
            tracing::info!(stage, "mod launch progress");
            if let Err(error) = app.emit("launch-progress", stage) {
                stage_error = Some(error.to_string());
            }
        });
        if result.is_ok() {
            let mut guard = lock_mutex(&refresh, "refresh")?;
            if guard.is_none() {
                *guard = Some(client);
            }
        }
        if let Some(error) = stage_error {
            return Err(format!("发送启动进度失败：{error}"));
        }
        result.map_err(|error| error.to_string())
    })
    .await;
    // 无论阻塞任务返回业务错误还是 JoinError，都必须释放忙状态；否则一次
    // worker panic 会让本次应用生命周期内的启动/F10 永久停在 busy 状态。
    state
        .launch_in_progress
        .store(false, std::sync::atomic::Ordering::Release);
    let result = task_result.map_err(|error| format!("模组启动任务异常：{error}"))?;
    if result.is_err() {
        *lock_mutex(&state.refresh, "refresh")? = None;
    }
    result
}

#[tauri::command]
pub fn launch_game_native(
    state: tauri::State<AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    let game_exe = lock_mutex(&state.config, "config")?.game_exe.clone();
    let Some(game_path) = game_exe else {
        return Err("未配置游戏主程序路径，请在设置中配置或点击自动探测".to_string());
    };
    liquimod_core::launcher::launch_native_game(&game_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launch_official_launcher(
    state: tauri::State<AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    let game_exe = lock_mutex(&state.config, "config")?.game_exe.clone();

    let launcher_path = game_exe
        .as_deref()
        .and_then(liquimod_core::discovery::find_launcher_from_game_exe)
        .or_else(liquimod_core::discovery::auto_detect_official_launcher);

    let Some(launcher) = launcher_path else {
        return Err(
            "未能在系统常见位置或游戏目录中找到官方启动器 (launcher.exe / HYP.exe)".to_string(),
        );
    };

    liquimod_core::launcher::launch_official_launcher(&launcher).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn trigger_refresh_game(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let refresh = std::sync::Arc::clone(&state.refresh);
    let (process_names, pin) = {
        let config = lock_mutex(&state.config, "config")?;
        let pin = current_launch_pin(&config)?;
        (configured_game_process_names(&config), pin)
    };
    if state
        .launch_in_progress
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Err("模组启动正在进行，暂时不能发送 F10".to_string());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let process_name_refs = process_names.iter().map(String::as_str).collect::<Vec<_>>();
        if !is_game_running(&process_name_refs) {
            return Err("未检测到游戏进程，无法发送 F10".to_string());
        }
        let process_name = process_names
            .iter()
            .find(|name| is_game_running(&[name.as_str()]))
            .map(String::as_str)
            .unwrap_or("StarRail.exe");
        tracing::info!(process = %process_name, "F10 refresh command requested");
        let result = send_refresh_game(&refresh, process_name, pin);
        match &result {
            Ok(()) => tracing::info!(process = %process_name, "F10 refresh command completed"),
            Err(error) => tracing::warn!(process = %process_name, error = %error, "F10 refresh command failed"),
        }
        result
    })
    .await
    .map_err(|e| format!("刷新任务失败：{e}"))?
}
