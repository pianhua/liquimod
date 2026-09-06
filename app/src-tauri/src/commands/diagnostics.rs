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
    pub deployment_root: Option<String>,
    pub total_mods: usize,
    pub enabled_mods: usize,
    pub healthy_mods: usize,
    pub attention_mods: usize,
    pub pending_operations: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ModDiagnosticDto {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub enabled: bool,
    pub storage_kind: String,
    pub source_available: bool,
    pub source_path: Option<String>,
    pub deployment_path: Option<String>,
    pub deployment_state: String,
    pub detail: String,
    pub remediation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct PendingOperationDto {
    pub id: i64,
    pub operation: String,
    pub payload: String,
    pub mod_id: Option<i64>,
    pub target: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticsCenterDto {
    pub environment: DiagnosticStatusDto,
    pub deployment: DeploymentOverviewDto,
    pub mods: Vec<ModDiagnosticDto>,
    pub pending_operations: Vec<PendingOperationDto>,
    pub hash_conflicts: Vec<ConflictReportDto>,
    pub variable_conflicts: Vec<VariableConflictDto>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RepairDeploymentResultDto {
    pub attempted_mods: usize,
    pub repaired_mods: usize,
    pub remaining_attention: usize,
    pub pending_operations: usize,
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

fn mod_diagnostic_remediation(state: &str, source_available: bool, storage_kind: &str) -> String {
    if !source_available {
        return if storage_kind == "external" {
            "恢复外部源目录后点击“重新检查”；源离线期间不能启用、打开或修复此 Mod，LiquiMod 不会复制或接管源文件。".to_owned()
        } else {
            "恢复托管 Mod 目录后点击“重新检查”；源目录不可用期间不会重建部署。".to_owned()
        };
    }

    match state {
        "disabled" => "无需处理；需要使用时从资源库启用此 Mod。".to_owned(),
        "deployed" => "无需处理；数据库状态与实际 Junction 一致。".to_owned(),
        "missing" => "确认 Mods 目录可写且源目录在线，然后点击“修复部署”。".to_owned(),
        "mismatched" => "先检查部署入口是否被其他工具占用；修复只会删除可验证属于 LiquiMod 的 Junction，未知路径会保留并报告。".to_owned(),
        "unexpected" => "数据库已禁用但仍有部署入口；修复只清理可验证属于 LiquiMod 的入口，不会删除未知目录或链接。".to_owned(),
        "unsupported" => "将 Library 与 3DMigoto Mods 移到同一 NTFS/ReFS 卷后再修复；当前不会创建复制副本。".to_owned(),
        "not_configured" => "在设置中配置 3Dmigoto Mods 目录，然后重新检查。".to_owned(),
        _ => "重新检查诊断；不要手动删除无法确认归属的目录或 Junction。".to_owned(),
    }
}

fn mod_source_path(entry: &liquimod_core::models::ModEntry, library: &Library) -> Option<String> {
    match entry.storage_kind {
        liquimod_core::models::ModStorageKind::Managed => Some(
            library
                .layout
                .root
                .join(&entry.rel_path)
                .display()
                .to_string(),
        ),
        liquimod_core::models::ModStorageKind::External => entry.source_path.clone(),
    }
}

fn pending_operation_detail(operation: &str, target: Option<&str>, payload: &str) -> String {
    let target = target
        .map(|value| format!("（{value}）"))
        .unwrap_or_default();
    match operation {
        "install" => format!("安装事务{target}尚未完成；重新打开应用时会清理临时安装标记。"),
        "enable" => format!("启用事务{target}尚未完成；重新检查后会按数据库状态尝试重建部署。"),
        "disable" => format!("禁用事务{target}尚未完成；重新检查后会尝试移除可验证的部署入口。"),
        "refresh" => format!("刷新事务{target}尚未完成；源目录在线且路径安全时可重新修复。"),
        _ => format!("操作 {operation}{target} 尚未完成；请重新检查。记录：{payload}"),
    }
}

fn collect_pending_operations(library: &Library) -> Result<Vec<PendingOperationDto>, String> {
    let entries = library.list().map_err(|error| error.to_string())?;
    let pending = library
        .db
        .pending_ops()
        .map_err(|error| error.to_string())?;
    Ok(pending
        .into_iter()
        .map(|(id, operation, payload)| {
            let mod_id = payload
                .parse::<i64>()
                .ok()
                .filter(|mod_id| entries.iter().any(|entry| entry.id == *mod_id));
            let entry = mod_id.and_then(|mod_id| entries.iter().find(|entry| entry.id == mod_id));
            let target = entry
                .map(|entry| format!("{}/{}", entry.character, entry.name))
                .or_else(|| (!payload.is_empty()).then(|| payload.clone()));
            let detail = pending_operation_detail(&operation, target.as_deref(), &payload);
            PendingOperationDto {
                id,
                operation,
                payload,
                mod_id,
                target,
                detail,
            }
        })
        .collect())
}

fn collect_mod_diagnostics(
    config: &crate::config::Config,
    library: &Library,
    environment: &DiagnosticStatusDto,
    pending_operation_count: usize,
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
            let storage_kind = status.entry.storage_kind.as_str().to_owned();
            rows.push(ModDiagnosticDto {
                id: status.entry.id,
                character: status.entry.character.clone(),
                name: status.entry.name.clone(),
                enabled: status.entry.enabled,
                storage_kind: storage_kind.clone(),
                source_available,
                source_path: mod_source_path(&status.entry, library),
                deployment_path: Some(
                    mods_dir
                        .join(Deployer::link_name(&status.entry))
                        .display()
                        .to_string(),
                ),
                detail: mod_diagnostic_detail(&state, source_available),
                remediation: mod_diagnostic_remediation(&state, source_available, &storage_kind),
                deployment_state: state,
            });
        }
    } else {
        for entry in entries {
            let source_available = library.entry_source_dir(&entry).is_ok();
            let storage_kind = entry.storage_kind.as_str().to_owned();
            let source_path = mod_source_path(&entry, library);
            rows.push(ModDiagnosticDto {
                id: entry.id,
                character: entry.character,
                name: entry.name,
                enabled: entry.enabled,
                storage_kind: storage_kind.clone(),
                source_available,
                source_path,
                deployment_path: None,
                deployment_state: "not_configured".to_owned(),
                detail: mod_diagnostic_detail("not_configured", source_available),
                remediation: mod_diagnostic_remediation(
                    "not_configured",
                    source_available,
                    &storage_kind,
                ),
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
            deployment_root: mods_dir.map(|path| path.display().to_string()),
            total_mods: rows.len(),
            enabled_mods,
            healthy_mods,
            attention_mods,
            pending_operations: pending_operation_count,
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
        let pending_operations = collect_pending_operations(&lib)?;
        let (deployment, mods) =
            collect_mod_diagnostics(&config, &lib, &environment, pending_operations.len())?;
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
            pending_operations,
            hash_conflicts,
            variable_conflicts,
        })
    })
    .await
    .map_err(|error| format!("诊断中心任务失败：{error}"))?
}

#[tauri::command]
pub async fn repair_deployment(
    state: tauri::State<'_, AppState>,
) -> Result<RepairDeploymentResultDto, String> {
    ensure_game_stopped(state.inner(), "修复 Mod 部署").map_err(|e| e.to_string())?;
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    tauri::async_runtime::spawn_blocking(move || {
        let mods_dir = lock_mutex(&config, "config")?
            .mods_dir
            .clone()
            .ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法修复部署".to_string())?;
        let lib = lock_mutex(&library, "library")?;
        let deployer = Deployer::new(&lib, &mods_dir);
        let before = deployer
            .inspect_status()
            .map_err(|e| format!("读取修复前部署状态失败：{e}"))?;
        let attempted_mods = before
            .iter()
            .filter(|status| {
                !matches!(
                    status.kind,
                    liquimod_core::deploy::DeploymentStatusKind::Disabled
                        | liquimod_core::deploy::DeploymentStatusKind::Deployed
                )
            })
            .count();
        let pending_before = lib
            .db
            .pending_ops()
            .map_err(|e| format!("读取待恢复操作失败：{e}"))?;
        if pending_before.is_empty() {
            deployer
                .reconcile()
                .map_err(|e| format!("部署对账失败：{e}"))?;
        } else {
            deployer
                .recover()
                .map_err(|e| format!("恢复未完成部署事务失败：{e}"))?;
        }
        let after = deployer
            .inspect_status()
            .map_err(|e| format!("读取修复后部署状态失败：{e}"))?;
        let remaining_attention = after
            .iter()
            .filter(|status| {
                !matches!(
                    status.kind,
                    liquimod_core::deploy::DeploymentStatusKind::Disabled
                        | liquimod_core::deploy::DeploymentStatusKind::Deployed
                )
            })
            .count();
        let pending_operations = lib
            .db
            .pending_ops()
            .map_err(|e| format!("读取待恢复操作失败：{e}"))?
            .len();
        Ok(RepairDeploymentResultDto {
            attempted_mods,
            repaired_mods: attempted_mods.saturating_sub(remaining_attention),
            remaining_attention,
            pending_operations,
        })
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

#[cfg(test)]
mod diagnostics_tests {
    use super::*;
    use liquimod_core::library::Library;
    use std::fs;

    fn temp_library() -> (tempfile::TempDir, Library) {
        let temp = tempfile::tempdir().expect("create temp library root");
        let library = Library::init(temp.path()).expect("initialize library");
        (temp, library)
    }

    #[test]
    fn remediation_is_present_for_every_supported_deployment_state() {
        for state in [
            "disabled",
            "deployed",
            "missing",
            "mismatched",
            "unexpected",
            "source_unavailable",
            "unsupported",
            "not_configured",
        ] {
            let remediation = mod_diagnostic_remediation(state, true, "managed");
            assert!(
                !remediation.trim().is_empty(),
                "missing remediation for {state}"
            );
        }
    }

    #[test]
    fn remediation_preserves_external_source_safety_boundary() {
        let remediation = mod_diagnostic_remediation("source_unavailable", false, "external");
        assert!(remediation.contains("恢复外部源目录"));
        assert!(remediation.contains("不能启用、打开或修复"));
        assert!(remediation.contains("不会复制或接管源文件"));
    }

    #[test]
    fn unsupported_remediation_requires_same_volume_ntfs_or_refs() {
        let remediation = mod_diagnostic_remediation("unsupported", true, "managed");
        assert!(remediation.contains("同一 NTFS/ReFS 卷"));
        assert!(remediation.contains("不会创建复制副本"));
    }

    #[test]
    fn source_paths_distinguish_managed_and_external_mods() {
        let (temp, library) = temp_library();
        let source = temp.path().join("input");
        fs::create_dir_all(&source).expect("create source");
        let managed = library
            .add_folder(&source, "Firefly", "Managed")
            .expect("add managed mod");

        let external_source = temp
            .path()
            .parent()
            .expect("temp parent")
            .join(format!("liquimod-external-{}", managed.id));
        fs::create_dir_all(&external_source).expect("create external source");
        let external = library
            .add_external_folder(&external_source, "Acheron", "External")
            .expect("add external mod");

        assert_eq!(
            mod_source_path(&managed, &library),
            Some(
                library
                    .layout
                    .root
                    .join(&managed.rel_path)
                    .display()
                    .to_string()
            )
        );
        assert_eq!(
            mod_source_path(&external, &library),
            external.source_path.clone()
        );

        let _ = fs::remove_dir_all(external_source);
    }

    #[test]
    fn pending_operations_are_mapped_to_targets_and_recovery_guidance() {
        let (temp, library) = temp_library();
        let source = temp.path().join("input");
        fs::create_dir_all(&source).expect("create source");
        let entry = library
            .add_folder(&source, "Acheron", "Offline Source")
            .expect("add mod");
        let refresh_id = library
            .db
            .op_begin("refresh", &entry.id.to_string())
            .expect("begin refresh operation");
        let unknown_id = library
            .db
            .op_begin("custom", "unmapped-payload")
            .expect("begin unknown operation");

        let pending = collect_pending_operations(&library).expect("collect pending operations");
        assert_eq!(pending.len(), 2);

        let refresh = pending
            .iter()
            .find(|operation| operation.id == refresh_id)
            .expect("refresh operation");
        assert_eq!(refresh.operation, "refresh");
        assert_eq!(refresh.payload, entry.id.to_string());
        assert_eq!(refresh.mod_id, Some(entry.id));
        assert_eq!(refresh.target.as_deref(), Some("Acheron/Offline Source"));
        assert!(refresh.detail.contains("Acheron/Offline Source"));
        assert!(refresh.detail.contains("源目录在线"));

        let unknown = pending
            .iter()
            .find(|operation| operation.id == unknown_id)
            .expect("unknown operation");
        assert_eq!(unknown.operation, "custom");
        assert_eq!(unknown.payload, "unmapped-payload");
        assert_eq!(unknown.mod_id, None);
        assert_eq!(unknown.target.as_deref(), Some("unmapped-payload"));
        assert!(unknown.detail.contains("操作 custom"));
        assert!(unknown.detail.contains("unmapped-payload"));
    }
}
