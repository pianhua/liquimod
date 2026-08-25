use super::*;

#[tauri::command]
pub fn get_config(state: tauri::State<AppState>) -> Result<ConfigDto, String> {
    let config = lock_mutex(&state.config, "config")?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn choose_mods_dir(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    let dto = {
        let mut config = lock_mutex(&state.config, "config")?;
        let dto = set_mods_dir(&mut config, PathBuf::from(path))?;
        config
            .save_to(&state.config_path)
            .map_err(|e| format!("配置保存失败：{e}"))?;
        dto
    };
    crate::start_watcher(&app, state.inner());
    Ok(dto)
}

#[tauri::command]
pub async fn add_mod_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    ensure_game_stopped(state.inner(), "添加外部 Mod 源")?;
    let config = std::sync::Arc::clone(&state.config);
    let library = std::sync::Arc::clone(&state.library);
    let config_path = state.config_path.clone();
    let game_running = state.game_running.load(Ordering::Relaxed);
    let source = PathBuf::from(path);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut next = lock_mutex(&config, "config")?.clone();
        let source = validate_mod_source(&next, &source)?;
        let config_changed = if !next
            .mod_sources
            .iter()
            .map(|value| normalized_for_compare(value))
            .any(|value| value == source)
        {
            next.mod_sources.push(source);
            next.mod_sources
                .sort_by_key(|value| value.to_string_lossy().to_lowercase());
            true
        } else {
            false
        };
        let (mods_dir, sources) = (next.mods_dir.clone(), next.mod_sources.clone());
        if config_changed {
            next.save_to(&config_path)
                .map_err(|e| format!("保存外部 Mod 源配置失败：{e}"))?;
            *lock_mutex(&config, "config")? = next.clone();
        }
        let lib = lock_mutex(&library, "library")?;
        crate::reconcile_and_diff_with_sources(&lib, mods_dir.as_deref(), &sources, !game_running)
            .map_err(|e| format!("扫描外部 Mod 源失败：{e}"))?;
        Ok::<ConfigDto, String>(config_dto(&next))
    })
    .await
    .map_err(|e| format!("添加外部 Mod 源任务失败：{e}"))??;
    crate::start_watcher(&app, state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn remove_mod_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    ensure_game_stopped(state.inner(), "移除外部 Mod 源")?;
    let config = std::sync::Arc::clone(&state.config);
    let library = std::sync::Arc::clone(&state.library);
    let config_path = state.config_path.clone();
    let requested = normalized_for_compare(Path::new(&path));
    let result = tauri::async_runtime::spawn_blocking(move || {
        let mut next = lock_mutex(&config, "config")?.clone();
        let before = next.mod_sources.len();
        next.mod_sources
            .retain(|value| normalized_for_compare(value) != requested);
        if next.mod_sources.len() == before {
            return Err("未找到要移除的外部 Mod 源".to_string());
        }

        // 解除连接只删除索引与 LiquiMod 自己创建的 Junction，不触碰源目录中的任何文件。
        let mods_dir = next.mods_dir.clone();
        let lib = lock_mutex(&library, "library")?;
        let external_entries = lib
            .db
            .list_mods()
            .map_err(|e| format!("读取外部 Mod 索引失败：{e}"))?
            .into_iter()
            .filter(|entry| {
                entry.storage_kind == ModStorageKind::External
                    && entry
                        .source_path
                        .as_deref()
                        .map(|source| {
                            let source = normalized_for_compare(Path::new(source));
                            source == requested || source.starts_with(&requested)
                        })
                        .unwrap_or(false)
            })
            .collect::<Vec<_>>();
        if let Some(mods_dir) = mods_dir.as_deref() {
            let deployer = Deployer::new(&lib, mods_dir);
            for entry in &external_entries {
                if entry.enabled {
                    deployer
                        .disable(entry.id)
                        .map_err(|e| format!("移除外部 Mod 部署失败：{e}"))?;
                }
            }
        }
        for entry in external_entries {
            lib.db
                .remove_mod(entry.id)
                .map_err(|e| format!("移除外部 Mod 索引失败：{e}"))?;
            liquimod_core::thumbs::remove_thumbnail(&lib.layout.root, entry.id);
        }
        drop(lib);
        next.save_to(&config_path)
            .map_err(|e| format!("保存外部 Mod 源配置失败：{e}"))?;
        *lock_mutex(&config, "config")? = next.clone();
        Ok(config_dto(&next))
    })
    .await
    .map_err(|e| format!("移除外部 Mod 源任务失败：{e}"))??;
    crate::start_watcher(&app, state.inner());
    Ok(result)
}

#[tauri::command]
pub async fn get_storage_info(state: tauri::State<'_, AppState>) -> Result<StorageInfoDto, String> {
    let (library_root, previous_library_root, storage_root) = {
        let config = lock_mutex(&state.config, "config")?;
        (
            config.library_root.clone(),
            config.previous_library_root.clone(),
            config.data_root(),
        )
    };
    tauri::async_runtime::spawn_blocking(move || {
        let stats = liquimod_core::storage::storage_stats(&library_root)
            .map_err(|e| format!("统计仓库失败：{e}"))?;
        Ok(StorageInfoDto {
            storage_root: storage_root.display().to_string(),
            library_root: library_root.display().to_string(),
            previous_library_root: previous_library_root.map(|p| p.display().to_string()),
            files: stats.files,
            bytes: stats.bytes,
            available_bytes: stats.available_bytes,
            recommended_root: Config::preferred_data_root().display().to_string(),
        })
    })
    .await
    .map_err(|e| format!("统计仓库任务失败：{e}"))?
}

#[tauri::command]
pub async fn migrate_storage(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    target_root: String,
) -> Result<StorageMigrationDto, String> {
    ensure_game_stopped(state.inner(), "迁移数据仓库")?;
    let target_root = PathBuf::from(target_root.trim());
    if target_root.as_os_str().is_empty() {
        return Err("请选择新的数据存储目录".to_string());
    }

    let old_watcher = lock_mutex(&state.watcher, "watcher")?.take();
    drop(old_watcher);
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    let config_path = state.config_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let old_config = lock_mutex(&config, "config")?.clone();
        let old_library_root = old_config.library_root.clone();
        let old_managed_migoto = old_config.managed_migoto_dir();
        let managed_migoto = old_config
            .mods_dir
            .as_deref()
            .and_then(Path::parent)
            .is_some_and(|parent| parent == old_managed_migoto);

        let report = {
            let library_guard = lock_mutex(&library, "library")?;
            liquimod_core::storage::migrate_library(&library_guard, &target_root)
        }
        .map_err(|e| format!("仓库迁移失败：{e}"))?;
        let new_managed_migoto = target_root.join("3DMigoto");
        if managed_migoto {
            if let Err(error) = liquimod_core::storage::copy_managed_directory(
                &old_managed_migoto,
                &new_managed_migoto,
            ) {
                let _ = std::fs::remove_dir_all(&report.library_root);
                return Err(format!("3DMigoto 迁移失败：{error}"));
            }
            liquimod_core::migoto_sync::init_migoto_workspace(&new_managed_migoto)
                .map_err(|e| format!("初始化迁移后的 3DMigoto 失败：{e}"))?;
        }

        let new_library = Library::open(&report.library_root)
            .map_err(|e| format!("无法打开迁移后的仓库：{e}"))?;
        new_library
            .db
            .verify_integrity()
            .map_err(|e| format!("迁移后的数据库校验失败：{e}"))?;

        let mut next_config = old_config.clone();
        next_config.previous_library_root = Some(old_library_root);
        next_config.library_root = report.library_root.clone();
        if managed_migoto {
            next_config.mods_dir = Some(new_managed_migoto.join("Mods"));
            next_config.loader_exe = None;
        }
        if let Err(error) = next_config.save_to(&config_path) {
            let _ = std::fs::remove_dir_all(&report.library_root);
            if managed_migoto {
                let _ = std::fs::remove_dir_all(&new_managed_migoto);
            }
            return Err(format!("保存新存储配置失败：{error}"));
        }

        *lock_mutex(&config, "config")? = next_config.clone();
        let deployment_warning = {
            let mut library_guard = lock_mutex(&library, "library")?;
            *library_guard = new_library;
            liquimod_core::games::hsr::Hsr::set_asset_root(
                next_config.data_root().join("GameAssets"),
            );
            next_config.mods_dir.as_deref().and_then(|mods_dir| {
                Deployer::new(&library_guard, mods_dir)
                    .reconcile()
                    .err()
                    .map(|e| format!("仓库已迁移，但部署对账需要稍后重试：{e}"))
            })
        };
        Ok(StorageMigrationDto {
            storage_root: target_root.display().to_string(),
            library_root: report.library_root.display().to_string(),
            copied_files: report.copied_files,
            copied_bytes: report.copied_bytes,
            managed_migoto_migrated: managed_migoto,
            deployment_warning,
        })
    })
    .await
    .map_err(|e| format!("仓库迁移任务失败：{e}"))?;
    crate::start_watcher(&app, state.inner());
    result
}

#[tauri::command]
pub async fn cleanup_previous_library(state: tauri::State<'_, AppState>) -> Result<u64, String> {
    ensure_game_stopped(state.inner(), "清理旧仓库")?;
    let config = std::sync::Arc::clone(&state.config);
    let config_path = state.config_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut config = lock_mutex(&config, "config")?;
        let previous = config
            .previous_library_root
            .clone()
            .ok_or_else(|| "没有可清理的旧仓库".to_string())?;
        if !previous.exists() {
            // 迁移记录可能指向已经被用户手动移走的旧仓库。这里只清理陈旧元数据，
            // 不尝试删除不存在的路径，也不放宽后续真实目录的安全校验。
            config.previous_library_root = None;
            config
                .save_to(&config_path)
                .map_err(|e| format!("保存清理状态失败：{e}"))?;
            return Ok(0);
        }
        if previous == config.library_root
            || previous.file_name().and_then(|name| name.to_str()) != Some("Library")
            || !previous.join("liquimod.db").is_file()
        {
            return Err("旧仓库路径校验失败，已阻止删除".to_string());
        }
        let stats = liquimod_core::storage::storage_stats(&previous)
            .map_err(|e| format!("统计旧仓库失败：{e}"))?;
        std::fs::remove_dir_all(&previous)
            .map_err(|e| format!("删除旧仓库失败，可能仍有文件被占用：{e}"))?;
        config.previous_library_root = None;
        config
            .save_to(&config_path)
            .map_err(|e| format!("保存清理状态失败：{e}"))?;
        Ok(stats.bytes)
    })
    .await
    .map_err(|e| format!("旧仓库清理任务失败：{e}"))?
}

#[tauri::command]
pub fn set_auto_enable(state: tauri::State<AppState>, enabled: bool) -> Result<ConfigDto, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    config.auto_enable = enabled;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("auto_enable = {enabled}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_warn_multiple_mods(
    state: tauri::State<AppState>,
    enabled: bool,
) -> Result<ConfigDto, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    config.warn_multiple_mods = enabled;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("warn_multiple_mods = {enabled}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_theme(state: tauri::State<AppState>, theme: String) -> Result<ConfigDto, String> {
    if !["auto", "light", "dark"].contains(&theme.as_str()) {
        return Err("主题只能是 auto / light / dark".to_string());
    }
    let mut config = lock_mutex(&state.config, "config")?;
    config.theme = theme.clone();
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("theme = {theme}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_character_category_name(
    state: tauri::State<AppState>,
    name: String,
) -> Result<ConfigDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let mut config = lock_mutex(&state.config, "config")?;
    config.character_category_name = name;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn choose_game_exe(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    let p = PathBuf::from(path);
    if !p.is_file() {
        return Err(format!("文件不存在：{}", p.display()));
    }
    if !p
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("exe"))
        .unwrap_or(false)
    {
        return Err("请选择 .exe 可执行文件".to_string());
    }
    let mut config = lock_mutex(&state.config, "config")?;
    config.game_exe = Some(p);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    let dto = config_dto(&config);
    drop(config);
    crate::start_game_watchdog(&app, state.inner());
    Ok(dto)
}

#[tauri::command]
pub fn init_migoto_workspace(target_dir: String) -> Result<String, String> {
    let p = PathBuf::from(target_dir);
    let ini = liquimod_core::migoto_sync::init_migoto_workspace(&p).map_err(|e| e.to_string())?;
    Ok(ini.display().to_string())
}

#[tauri::command]
pub async fn check_migoto_update(
    state: tauri::State<'_, AppState>,
) -> Result<liquimod_core::migoto_sync::MigotoReleaseInfo, String> {
    let (token, mirror) = {
        let config = lock_mutex(&state.config, "config")?;
        let t = if config.github_token.is_empty() {
            None
        } else {
            Some(config.github_token.clone())
        };
        let m = if config.github_mirror.is_empty() {
            None
        } else {
            Some(config.github_mirror.clone())
        };
        (t, m)
    };

    liquimod_core::migoto_sync::check_latest_srmi_release(token.as_deref(), mirror.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_xxmi_update(
    state: tauri::State<'_, AppState>,
) -> Result<liquimod_core::migoto_sync::MigotoReleaseInfo, String> {
    let (token, mirror) = {
        let config = lock_mutex(&state.config, "config")?;
        (
            (!config.github_token.is_empty()).then(|| config.github_token.clone()),
            (!config.github_mirror.is_empty()).then(|| config.github_mirror.clone()),
        )
    };
    liquimod_core::migoto_sync::check_latest_xxmi_release(token.as_deref(), mirror.as_deref())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_core_package_status(
    state: tauri::State<AppState>,
) -> Result<Vec<liquimod_core::migoto_sync::PackageStatus>, String> {
    let data_root = lock_mutex(&state.config, "config")?.data_root();
    Ok(liquimod_core::migoto_sync::package_statuses(&data_root))
}

async fn install_latest_core_package(
    app: &tauri::AppHandle,
    state: &AppState,
    package: liquimod_core::migoto_sync::PackageKind,
) -> Result<ConfigDto, String> {
    let (data_root, token, mirror, config_path) = {
        let config = lock_mutex(&state.config, "config")?;
        (
            config.data_root(),
            (!config.github_token.is_empty()).then(|| config.github_token.clone()),
            (!config.github_mirror.is_empty()).then(|| config.github_mirror.clone()),
            state.config_path.clone(),
        )
    };
    let release = match package {
        liquimod_core::migoto_sync::PackageKind::Srmi => {
            liquimod_core::migoto_sync::check_latest_srmi_release(
                token.as_deref(),
                mirror.as_deref(),
            )
            .await
        }
        liquimod_core::migoto_sync::PackageKind::Xxmi => {
            liquimod_core::migoto_sync::check_latest_xxmi_release(
                token.as_deref(),
                mirror.as_deref(),
            )
            .await
        }
    }
    .map_err(|e| e.to_string())?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let app2 = app.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app2.emit("migoto-download-progress", progress);
        }
    });
    let result = liquimod_core::migoto_sync::install_official_package(
        &release,
        &data_root,
        mirror.as_deref(),
        token.as_deref(),
        Some(tx),
    )
    .await
    .map_err(|e| e.to_string());
    let _ = forward_task.await;
    result?;

    let mut config = lock_mutex(&state.config, "config")?;
    config.mods_dir = Some(liquimod_core::migoto_sync::runtime_paths(&data_root).mods_dir);
    if package == liquimod_core::migoto_sync::PackageKind::Srmi {
        config.migoto_version = Some(release.tag_name.clone());
    }
    config
        .save_to(&config_path)
        .map_err(|e| format!("保存核心版本配置失败：{e}"))?;
    let dto = config_dto(&config);
    drop(config);
    crate::start_watcher(app, state);
    Ok(dto)
}

#[tauri::command]
pub async fn install_xxmi_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConfigDto, String> {
    ensure_game_stopped(state.inner(), "更新 XXMI 核心")?;
    install_latest_core_package(
        &app,
        state.inner(),
        liquimod_core::migoto_sync::PackageKind::Xxmi,
    )
    .await
}

#[tauri::command]
pub async fn install_srmi_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConfigDto, String> {
    ensure_game_stopped(state.inner(), "更新 SRMI 核心")?;
    install_latest_core_package(
        &app,
        state.inner(),
        liquimod_core::migoto_sync::PackageKind::Srmi,
    )
    .await
}

#[tauri::command]
pub async fn install_migoto_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    _download_url: String,
    _version_tag: Option<String>,
) -> Result<ConfigDto, String> {
    ensure_game_stopped(state.inner(), "更新 SRMI 核心")?;
    install_latest_core_package(
        &app,
        state.inner(),
        liquimod_core::migoto_sync::PackageKind::Srmi,
    )
    .await
}

#[tauri::command]
pub fn switch_to_managed_migoto(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConfigDto, String> {
    let def_dir = lock_mutex(&state.config, "config")?.managed_migoto_dir();
    let _ = liquimod_core::migoto_sync::init_migoto_workspace(&def_dir)
        .map_err(|e| format!("初始化内置 3Dmigoto 失败：{e}"))?;

    let mut cfg = lock_mutex(&state.config, "config")?;
    let mods_dir = def_dir.join("Mods");
    cfg.mods_dir = Some(mods_dir);
    cfg.loader_exe = None;

    cfg.save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    let dto = config_dto(&cfg);
    drop(cfg);

    crate::start_watcher(&app, state.inner());
    Ok(dto)
}

#[derive(Debug, serde::Serialize)]
pub struct MigrateResultDto {
    pub total_found: usize,
    pub migrated_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn migrate_mods_from_old_migoto(
    state: tauri::State<'_, AppState>,
    old_dir: String,
) -> Result<MigrateResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let p = PathBuf::from(old_dir);
        let mods_path = if p.join("Mods").is_dir() {
            p.join("Mods")
        } else {
            p
        };

        if !mods_path.is_dir() {
            return Err("指定的旧目录不存在或不包含 Mods 文件夹".to_string());
        }

        let lib = lock_mutex(&library, "library")?;
        let mut total_found = 0;
        let mut migrated_count = 0;
        let mut failed_count = 0;
        let mut errors = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&mods_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                // 安全防御 (LM-P2-014): 使用 symlink_metadata 严禁 follow symlink 误把软链接当实体复制
                if let Ok(ft) = entry.file_type() {
                    if ft.is_symlink() || !ft.is_dir() {
                        continue;
                    }
                } else if let Ok(meta) = std::fs::symlink_metadata(&path) {
                    if meta.file_type().is_symlink() || !meta.is_dir() {
                        continue;
                    }
                } else {
                    continue;
                }

                let folder_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if folder_name.is_empty()
                    || folder_name.starts_with('.')
                    || folder_name.eq_ignore_ascii_case("disabled")
                {
                    continue;
                }

                total_found += 1;

                // 1. 尝试推断角色
                let character = liquimod_core::games::infer_character(&path, Hsr::shared())
                    .unwrap_or_else(|| "others".to_string());

                // 2. 清洗 Mod 名
                let mod_name = folder_name.to_string();

                // 3. 复制并收录到 Library
                match lib.add_folder(&path, &character, &mod_name) {
                    Ok(_) => {
                        migrated_count += 1;
                    }
                    Err(e) => {
                        failed_count += 1;
                        errors.push(format!("{}: {}", folder_name, e));
                    }
                }
            }
        }

        // 统一自动归类
        let _ = sync_mod_categories(&lib, Hsr::shared());

        Ok(MigrateResultDto {
            total_found,
            migrated_count,
            failed_count,
            errors,
        })
    })
    .await
    .map_err(|e| format!("迁移任务失败：{e}"))?
}

#[tauri::command]
pub fn set_work_mode(state: tauri::State<AppState>, mode: String) -> Result<ConfigDto, String> {
    if !["play", "dev"].contains(&mode.as_str()) {
        return Err("工作模式只能是 play 或 dev".to_string());
    }
    let mut config = lock_mutex(&state.config, "config")?;
    config.work_mode = mode.clone();

    // 若已配置 3Dmigoto 目录，顺带实时更新 d3dx.ini
    if let Some(mods_dir) = &config.mods_dir {
        if let Some(parent) = mods_dir.parent() {
            let ini = parent.join("d3dx.ini");
            if ini.is_file() {
                let m = match mode.as_str() {
                    "dev" => liquimod_core::d3d::MigotoWorkMode::Dev,
                    _ => liquimod_core::d3d::MigotoWorkMode::Play,
                };
                let _ = liquimod_core::d3d::update_d3dx_ini_mode(&ini, m);
            }
        }
    }

    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_injection_delay(
    state: tauri::State<AppState>,
    delay_ms: u64,
) -> Result<ConfigDto, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    config.injection_delay_ms = delay_ms.min(10000);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_github_token(state: tauri::State<AppState>, token: String) -> Result<ConfigDto, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    config.github_token = token.trim().to_string();
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_github_mirror(
    state: tauri::State<AppState>,
    mirror: String,
) -> Result<ConfigDto, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    config.github_mirror = mirror.trim().to_string();
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn inspect_3dmigoto_dir(path: String) -> Result<MigotoInspectDto, String> {
    let p = PathBuf::from(path);
    let info = liquimod_core::d3d::inspect_migoto_dir(&p).map_err(|e| e.to_string())?;
    Ok(MigotoInspectDto {
        root: info.root.display().to_string(),
        ini_path: info.ini_path.display().to_string(),
        game_exe: info.game_exe.map(|p| p.display().to_string()),
        loader_exe: info.loader_exe.map(|p| p.display().to_string()),
        mods_dir: info.mods_dir.map(|p| p.display().to_string()),
    })
}

#[tauri::command]
pub fn import_3dmigoto_dir(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    let p = PathBuf::from(path);
    let info = liquimod_core::d3d::inspect_migoto_dir(&p).map_err(|e| e.to_string())?;

    let dto = {
        let mut config = lock_mutex(&state.config, "config")?;
        if let Some(game_exe) = info.game_exe {
            config.game_exe = Some(game_exe);
        }
        // 导入旧配置只读取游戏路径；运行时始终使用 LiquiMod 自己的
        // 3DMigoto 工作区，旧 Loader.exe 不再进入启动链。
        config.mods_dir = Some(config.managed_mods_dir());
        config.loader_exe = None;
        config
            .save_to(&state.config_path)
            .map_err(|e| format!("配置保存失败：{e}"))?;
        config_dto(&config)
    };

    crate::start_watcher(&app, state.inner());
    Ok(dto)
}
