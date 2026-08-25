use super::*;

#[tauri::command]
pub fn read_log() -> Result<String, String> {
    read_log_tail(&crate::config::Config::log_dir(), 64 * 1024)
}

#[tauri::command]
pub async fn rescan_library(state: tauri::State<'_, AppState>) -> Result<RescanResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    let game_running = std::sync::Arc::clone(&state.game_running);
    tauri::async_runtime::spawn_blocking(move || {
        let (mods_dir, sources) = {
            let cfg = lock_mutex(&config, "config")?;
            (cfg.mods_dir.clone(), cfg.mod_sources.clone())
        };
        let lib = lock_mutex(&library, "library")?;
        let deploy = !game_running.load(std::sync::atomic::Ordering::Relaxed);
        let (added, removed) =
            crate::reconcile_and_diff_with_sources(&lib, mods_dir.as_deref(), &sources, deploy)
                .map_err(|e| format!("全库重新扫描失败：{e}"))?;
        Ok(RescanResultDto { added, removed })
    })
    .await
    .map_err(|e| format!("重新扫描任务失败：{e}"))?
}

#[tauri::command]
pub async fn clean_cache(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let mods = lib.list().map_err(|e| e.to_string())?;
        let valid: std::collections::HashSet<i64> = mods.into_iter().map(|m| m.id).collect();
        let thumb_dir = lib.layout.root.join("thumbs");
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(&thumb_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = stem.parse::<i64>() {
                        if !valid.contains(&id) && std::fs::remove_file(&path).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    })
    .await
    .map_err(|e| format!("清理缓存任务失败：{e}"))?
}

#[derive(Debug, serde::Serialize)]
pub struct DiagnosticStatusDto {
    pub helper_ready: bool,
    pub game_configured: bool,
    pub loader_configured: bool,
    pub mods_dir_configured: bool,
    pub checks: Vec<liquimod_core::diagnostics::DiagnosticCheck>,
    pub filesystem: Option<String>,
    pub deploy_strategy: Option<String>,
    pub defender_command: Option<String>,
}

#[tauri::command]
pub fn get_diagnostic_status(state: tauri::State<AppState>) -> Result<DiagnosticStatusDto, String> {
    let (library_root, mods_dir, game_exe) = {
        let config = lock_mutex(&state.config, "config")?;
        (
            config.library_root.clone(),
            config.mods_dir.clone(),
            config.game_exe.clone(),
        )
    };
    let helper_ready = refresh_helper_path().is_some();
    let checks = liquimod_core::diagnostics::collect_checks(
        &library_root,
        mods_dir.as_deref(),
        game_exe.as_deref(),
        None,
        helper_ready,
    );
    let filesystem = mods_dir
        .as_deref()
        .and_then(|mods| liquimod_core::filesystem::same_volume_filesystem(&library_root, mods));
    let deploy_strategy = if let Some(mods) = mods_dir.as_deref() {
        let lib = lock_mutex(&state.library, "library")?;
        Some(
            liquimod_core::deploy::Deployer::new(&lib, mods)
                .strategy_label()
                .to_owned(),
        )
    } else {
        None
    };
    let mut exclusion_paths = vec![library_root.as_path()];
    if let Some(mods) = mods_dir.as_deref() {
        exclusion_paths.push(mods);
        if let Some(parent) = mods.parent() {
            exclusion_paths.push(parent);
        }
    }

    Ok(DiagnosticStatusDto {
        helper_ready,
        game_configured: game_exe.is_some_and(|p| !p.as_os_str().is_empty()),
        // 保留 DTO 字段供旧前端兼容；当前原生 Hook 流程不再配置 Loader.exe。
        loader_configured: false,
        mods_dir_configured: mods_dir.as_ref().is_some_and(|p| !p.as_os_str().is_empty()),
        checks,
        filesystem,
        deploy_strategy,
        defender_command: liquimod_core::diagnostics::defender_exclusion_command(&exclusion_paths),
    })
}

#[tauri::command]
pub async fn repair_deployment(state: tauri::State<'_, AppState>) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "修复 Mod 部署").map_err(|e| e.to_string())?;
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    tauri::async_runtime::spawn_blocking(move || {
        let mods_dir = lock_mutex(&config, "config")?
            .mods_dir
            .clone()
            .ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法修复部署".to_string())?;
        let lib = lock_mutex(&library, "library")?;
        Deployer::new(&lib, &mods_dir)
            .reconcile()
            .map_err(|e| format!("部署对账失败：{e}"))
    })
    .await
    .map_err(|e| format!("修复部署任务失败：{e}"))?
}

#[tauri::command]
pub fn open_webview2_download(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            liquimod_core::diagnostics::WEBVIEW2_DOWNLOAD_URL,
            None::<String>,
        )
        .map_err(|e| format!("无法打开 WebView2 下载页面：{e}"))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AssetUpdateCheckResultDto {
    pub has_update: bool,
    pub remote_version: Option<String>,
    pub local_version: Option<String>,
}

#[tauri::command]
pub async fn get_local_asset_version(
    state: tauri::State<'_, AppState>,
) -> Result<Option<String>, String> {
    let asset_root = lock_mutex(&state.config, "config")?
        .data_root()
        .join("GameAssets");
    liquimod_core::games::hsr::Hsr::set_asset_root(asset_root.clone());
    let service = liquimod_core::assets_sync::AssetSyncService::with_root(asset_root);
    Ok(service.get_local_version().await)
}

#[tauri::command]
pub async fn check_game_assets_update(
    state: tauri::State<'_, AppState>,
    game: Option<String>,
) -> Result<AssetUpdateCheckResultDto, String> {
    let asset_root = lock_mutex(&state.config, "config")?
        .data_root()
        .join("GameAssets");
    liquimod_core::games::hsr::Hsr::set_asset_root(asset_root.clone());
    let service = liquimod_core::assets_sync::AssetSyncService::with_root(asset_root);
    let local = service.get_local_version().await;
    let filter = game.as_deref().or(Some("Honkai"));
    match service.check_for_updates(filter).await {
        Ok(remote_opt) => {
            let has_update = remote_opt.is_some();
            Ok(AssetUpdateCheckResultDto {
                has_update,
                remote_version: remote_opt,
                local_version: local,
            })
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn sync_game_assets(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    game: Option<String>,
) -> Result<liquimod_core::assets_sync::AssetSyncResult, String> {
    let asset_root = lock_mutex(&state.config, "config")?
        .data_root()
        .join("GameAssets");
    liquimod_core::games::hsr::Hsr::set_asset_root(asset_root.clone());
    let service = liquimod_core::assets_sync::AssetSyncService::with_root(asset_root);
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    let app_clone = app.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app_clone.emit("asset-sync-progress", progress);
        }
    });

    let filter = game.as_deref().or(Some("Honkai"));
    let result = service.sync(filter, Some(tx)).await;
    let _ = forward_task.await;

    match result {
        Ok(res) => {
            Hsr::shared().reload();
            let _ = app.emit("game-assets-updated", ());
            Ok(res)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_character_image_data(
    state: tauri::State<'_, AppState>,
    game: Option<String>,
    filename: String,
) -> Result<Option<String>, String> {
    let raw_game = game.as_deref().unwrap_or("Honkai");
    // 安全防御 (LM-P1-003): 游戏目录白名单校验
    let game_name = match raw_game.to_lowercase().as_str() {
        "honkai" | "hsr" => "Honkai",
        "genshin" => "Genshin",
        "zenless" | "zzz" => "Zenless",
        _ => return Ok(None),
    };

    // 安全防御: 净化文件名相对路径，严禁 .. 逃逸
    let Ok(safe_file) = liquimod_core::safe_path::sanitize_relative_path(Path::new(&filename))
    else {
        return Ok(None);
    };

    let asset_root = state
        .config
        .lock()
        .unwrap()
        .data_root()
        .join("GameAssets")
        .join(game_name);

    // 尝试多个可能路径（支持大小写与子目录）
    let candidates = [
        asset_root
            .join("Images")
            .join("Characters")
            .join(&safe_file),
        asset_root
            .join("images")
            .join("Characters")
            .join(&safe_file),
        asset_root
            .join("Images")
            .join("characters")
            .join(&safe_file),
        asset_root
            .join("images")
            .join("characters")
            .join(&safe_file),
        asset_root.join("Images").join(&safe_file),
        asset_root.join("images").join(&safe_file),
        asset_root.join(&safe_file),
    ];

    for path in &candidates {
        if path.is_file() {
            if let Ok(meta) = tokio::fs::metadata(path).await {
                if meta.len() > 15 * 1024 * 1024 {
                    continue;
                }
            }
            if let Ok(bytes) = tokio::fs::read(path).await {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let mime = if filename.to_lowercase().ends_with(".webp") {
                    "image/webp"
                } else if filename.to_lowercase().ends_with(".gif") {
                    "image/gif"
                } else if filename.to_lowercase().ends_with(".jpg")
                    || filename.to_lowercase().ends_with(".jpeg")
                {
                    "image/jpeg"
                } else {
                    "image/png"
                };
                return Ok(Some(format!("data:{};base64,{}", mime, b64)));
            }
        }
    }

    Ok(None)
}
