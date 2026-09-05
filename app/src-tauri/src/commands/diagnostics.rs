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

#[derive(Debug, Clone, serde::Serialize)]
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

#[derive(Debug, Clone, serde::Serialize)]
pub struct DeploymentOverviewDto {
    pub configured: bool,
    pub strategy: Option<String>,
    pub filesystem: Option<String>,
    pub total_mods: usize,
    pub enabled_mods: usize,
    pub healthy_mods: usize,
    pub attention_mods: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModDiagnosticDto {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub enabled: bool,
    pub storage_kind: String,
    pub source_available: bool,
    pub deployment_state: String,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticsCenterDto {
    pub environment: DiagnosticStatusDto,
    pub deployment: DeploymentOverviewDto,
    pub mods: Vec<ModDiagnosticDto>,
    pub hash_conflicts: Vec<ConflictReportDto>,
    pub variable_conflicts: Vec<VariableConflictDto>,
}

fn collect_diagnostic_status(
    config: &crate::config::Config,
    library: &Library,
) -> DiagnosticStatusDto {
    let helper_ready = refresh_helper_path().is_some();
    let mods_dir = config
        .mods_dir
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty());
    let checks = liquimod_core::diagnostics::collect_checks(
        &config.library_root,
        mods_dir,
        config.game_exe.as_deref(),
        None,
        helper_ready,
    );
    let filesystem = mods_dir.and_then(|mods| {
        liquimod_core::filesystem::same_volume_filesystem(&config.library_root, mods)
    });
    let deploy_strategy = mods_dir.map(|mods| {
        liquimod_core::deploy::Deployer::new(library, mods)
            .strategy_label()
            .to_owned()
    });
    let mut exclusion_paths = vec![config.library_root.as_path()];
    if let Some(mods) = mods_dir {
        exclusion_paths.push(mods);
        if let Some(parent) = mods.parent() {
            exclusion_paths.push(parent);
        }
    }

    DiagnosticStatusDto {
        helper_ready,
        game_configured: config
            .game_exe
            .as_ref()
            .is_some_and(|path| !path.as_os_str().is_empty()),
        // 保留 DTO 字段供旧前端兼容；当前原生 Hook 流程不再配置 Loader.exe。
        loader_configured: false,
        mods_dir_configured: mods_dir.is_some_and(|path| !path.as_os_str().is_empty()),
        checks,
        filesystem,
        deploy_strategy,
        defender_command: liquimod_core::diagnostics::defender_exclusion_command(&exclusion_paths),
    }
}

fn conflict_report_dtos(conflicts: Vec<liquimod_core::d3d::ModConflict>) -> Vec<ConflictReportDto> {
    conflicts
        .into_iter()
        .map(|conflict| ConflictReportDto {
            hash: conflict.hash,
            section: conflict.section,
            conflicting_mods: conflict
                .conflicting_mods
                .into_iter()
                .map(|mod_info| ConflictModInfoDto {
                    id: mod_info.id,
                    character: mod_info.character,
                    name: mod_info.name,
                })
                .collect(),
        })
        .collect()
}

fn variable_conflict_dtos(
    conflicts: Vec<liquimod_core::d3d::VariableConflict>,
) -> Vec<VariableConflictDto> {
    conflicts
        .into_iter()
        .map(|conflict| VariableConflictDto {
            variable: conflict.variable,
            conflicting_mods: conflict
                .conflicting_mods
                .into_iter()
                .map(|mod_info| ConflictModInfoDto {
                    id: mod_info.id,
                    character: mod_info.character,
                    name: mod_info.name,
                })
                .collect(),
        })
        .collect()
}

fn deployment_state_label(kind: liquimod_core::deploy::DeploymentStatusKind) -> &'static str {
    match kind {
        liquimod_core::deploy::DeploymentStatusKind::Disabled => "disabled",
        liquimod_core::deploy::DeploymentStatusKind::Deployed => "deployed",
        liquimod_core::deploy::DeploymentStatusKind::Missing => "missing",
        liquimod_core::deploy::DeploymentStatusKind::Mismatched => "mismatched",
        liquimod_core::deploy::DeploymentStatusKind::Unexpected => "unexpected",
        liquimod_core::deploy::DeploymentStatusKind::SourceUnavailable => "source_unavailable",
        liquimod_core::deploy::DeploymentStatusKind::Unsupported => "unsupported",
    }
}

fn deployment_state_detail(state: &str) -> &'static str {
    match state {
        "disabled" => "Mod 已禁用，未检查到活动部署",
        "deployed" => "数据库状态与磁盘 Junction 部署一致",
        "missing" => "数据库标记为启用，但 Mods 目录中没有正确的 Junction",
        "mismatched" => "数据库标记为启用，但 Junction 指向了错误目标",
        "unexpected" => "数据库标记为禁用，但 Mods 目录仍存在部署入口",
        "source_unavailable" => "源目录不可用，无法验证或恢复部署",
        "unsupported" => "当前路径不满足同卷 NTFS/ReFS Junction，部署不可用",
        "not_configured" => "尚未配置 3Dmigoto Mods 目录",
        _ => "状态未知，请刷新诊断",
    }
}

fn mod_diagnostic_detail(state: &str, source_available: bool) -> String {
    let detail = deployment_state_detail(state);
    if !source_available && state != "source_unavailable" {
        format!("{detail}；源目录不可用，依赖源文件的操作不可执行")
    } else {
        detail.to_owned()
    }
}

fn collect_mod_diagnostics(
    config: &crate::config::Config,
    library: &Library,
    environment: &DiagnosticStatusDto,
) -> Result<(DeploymentOverviewDto, Vec<ModDiagnosticDto>), String> {
    let entries = library.list().map_err(|error| error.to_string())?;
    let mods_dir = config
        .mods_dir
        .as_deref()
        .filter(|path| !path.as_os_str().is_empty());
    let configured = mods_dir.is_some();
    let mut rows = Vec::with_capacity(entries.len());

    if let Some(mods_dir) = mods_dir {
        let deployer = liquimod_core::deploy::Deployer::new(library, mods_dir);
        let statuses = deployer
            .inspect_status()
            .map_err(|error| format!("读取 Mod 部署状态失败：{error}"))?;
        for status in statuses {
            let source_available = library.entry_source_dir(&status.entry).is_ok();
            let state = deployment_state_label(status.kind).to_owned();
            rows.push(ModDiagnosticDto {
                id: status.entry.id,
                character: status.entry.character,
                name: status.entry.name,
                enabled: status.entry.enabled,
                storage_kind: status.entry.storage_kind.as_str().to_owned(),
                source_available,
                detail: mod_diagnostic_detail(&state, source_available),
                deployment_state: state,
            });
        }
    } else {
        for entry in entries {
            let source_available = library.entry_source_dir(&entry).is_ok();
            rows.push(ModDiagnosticDto {
                id: entry.id,
                character: entry.character,
                name: entry.name,
                enabled: entry.enabled,
                storage_kind: entry.storage_kind.as_str().to_owned(),
                source_available,
                deployment_state: "not_configured".to_owned(),
                detail: mod_diagnostic_detail("not_configured", source_available),
            });
        }
    }

    let enabled_mods = rows.iter().filter(|row| row.enabled).count();
    let healthy_mods = rows
        .iter()
        .filter(|row| {
            row.source_available && matches!(row.deployment_state.as_str(), "disabled" | "deployed")
        })
        .count();
    let attention_mods = rows.len().saturating_sub(healthy_mods);

    Ok((
        DeploymentOverviewDto {
            configured,
            strategy: environment.deploy_strategy.clone(),
            filesystem: environment.filesystem.clone(),
            total_mods: rows.len(),
            enabled_mods,
            healthy_mods,
            attention_mods,
        },
        rows,
    ))
}

#[tauri::command]
pub async fn get_diagnostic_status(
    state: tauri::State<'_, AppState>,
) -> Result<DiagnosticStatusDto, String> {
    let config = lock_mutex(&state.config, "config")?.clone();
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        Ok(collect_diagnostic_status(&config, &lib))
    })
    .await
    .map_err(|error| format!("诊断任务失败：{error}"))?
}

/// Read one coherent snapshot for the diagnostics workbench. All filesystem and INI inspection
/// runs off the Tauri/UI thread, and this command never repairs or mutates deployment state.
#[tauri::command]
pub async fn get_diagnostics_center(
    state: tauri::State<'_, AppState>,
) -> Result<DiagnosticsCenterDto, String> {
    let config = lock_mutex(&state.config, "config")?.clone();
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let environment = collect_diagnostic_status(&config, &lib);
        let (deployment, mods) = collect_mod_diagnostics(&config, &lib, &environment)?;
        let hash_conflicts = liquimod_core::d3d::detect_conflicts(&lib)
            .map(conflict_report_dtos)
            .map_err(|error| format!("读取 Hash 冲突失败：{error}"))?;
        let variable_conflicts = liquimod_core::d3d::detect_variable_conflicts(&lib)
            .map(variable_conflict_dtos)
            .map_err(|error| format!("读取变量冲突失败：{error}"))?;
        Ok(DiagnosticsCenterDto {
            environment,
            deployment,
            mods,
            hash_conflicts,
            variable_conflicts,
        })
    })
    .await
    .map_err(|error| format!("诊断中心任务失败：{error}"))?
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
