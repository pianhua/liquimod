use super::*;

#[tauri::command]
pub async fn get_characters(
    state: tauri::State<'_, AppState>,
    category_id: Option<i64>,
) -> Result<Vec<CharacterSummary>, String> {
    let library = std::sync::Arc::clone(&state.library);
    let favorites = lock_mutex(&state.config, "config")?
        .favorite_characters
        .clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        character_summaries(
            &lib,
            liquimod_core::games::hsr::Hsr::shared(),
            category_id,
            &favorites,
        )
    })
    .await
    .map_err(|e| format!("读取角色失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_note(
    state: tauri::State<'_, AppState>,
    id: i64,
    note: Option<String>,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db
            .set_mod_note(id, note.as_deref())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("更新 Mod 备注失败：{e}"))?
}

#[tauri::command]
pub fn toggle_favorite_character(
    state: tauri::State<AppState>,
    internal_name: String,
) -> Result<bool, String> {
    let mut config = lock_mutex(&state.config, "config")?;
    let is_fav = if let Some(idx) = config
        .favorite_characters
        .iter()
        .position(|s| s == &internal_name)
    {
        config.favorite_characters.remove(idx);
        false
    } else {
        config.favorite_characters.push(internal_name);
        true
    };
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("保存配置失败：{e}"))?;
    Ok(is_fav)
}

#[tauri::command]
pub fn toggle_favorite_mod(state: tauri::State<AppState>, id: i64) -> Result<bool, String> {
    let lib = lock_mutex(&state.library, "library")?;
    lib.db.toggle_favorite_mod(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_mods(state: tauri::State<AppState>, ids: Vec<i64>) -> Result<(), String> {
    let lib = lock_mutex(&state.library, "library")?;
    lib.db.reorder_mods(&ids).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_mods(
    state: tauri::State<'_, AppState>,
    character: String,
    category_id: Option<i64>,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = lock_mutex(&library, "library")?;
            let root = lib.layout.root.clone();
            let rows = collect_mod_rows(&lib, &character, category_id)?;
            (root, rows)
        }; // 释放库锁后再做缩略图生成（可能慢）
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取 Mod 列表失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_enabled(
    state: tauri::State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    let deferred_runtime_cleanup = std::sync::Arc::clone(&state.deferred_runtime_cleanup);
    let game_running = state.game_running.load(Ordering::Relaxed);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let mods_dir = mods_dir
            .as_deref()
            .ok_or("未配置 3Dmigoto Mods 目录，请先选择目录")?;
        if !mods_dir.is_dir() {
            return Err(format!("Mods 目录不存在：{}", mods_dir.display()));
        }
        let deployer = Deployer::new(&lib, mods_dir);
        if game_running
            && !matches!(
                deployer.strategy(),
                liquimod_core::filesystem::DeployStrategy::Junction
            )
        {
            return Err(
                "当前仅支持同卷 NTFS/ReFS Junction；请将应用数据根与 3Dmigoto Mods 目录放在同一卷后重试"
                    .to_string(),
            );
        }

        let cleanup_id = if game_running && enabled {
            deployer
                .enable_reusing_runtime(id)
                .map_err(|e| e.to_string())?;
            Some(false)
        } else if game_running {
            deployer
                .disable_preserving_runtime(id)
                .map_err(|e| e.to_string())?;
            Some(true)
        } else {
            set_enabled(&lib, Some(mods_dir), id, enabled)?;
            Some(false)
        };
        drop(lib);
        if let Some(insert) = cleanup_id {
            let mut cleanup = lock_mutex(&deferred_runtime_cleanup, "deferred_runtime_cleanup")?;
            if insert {
                cleanup.insert(id);
            } else {
                cleanup.remove(&id);
            }
        }
        tracing::info!("set mod {id} enabled={enabled} game_running={game_running}");
        Ok(())
    })
    .await
    .map_err(|e| format!("切换 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_variant(
    state: tauri::State<'_, AppState>,
    id: i64,
    variant: Option<String>,
) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "切换 Mod 变体")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
        let root = lib.entry_source_dir(&entry).map_err(|e| e.to_string())?;
        let available = liquimod_core::variants::detect_variants(&root);
        let requested = variant.filter(|v| !v.trim().is_empty());
        if let Some(ref value) = requested {
            if !available.iter().any(|v| v.name == *value) {
                return Err(format!("变体不存在：{value}"));
            }
        }
        lib.db
            .set_active_variant(id, requested.as_deref())
            .map_err(|e| e.to_string())?;
        if entry.enabled {
            let mods_dir = mods_dir
                .as_deref()
                .ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法刷新变体".to_string())?;
            Deployer::new(&lib, mods_dir)
                .refresh(id)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("切换变体任务失败：{e}"))?
}

#[tauri::command]
pub async fn install_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    character: Option<String>,
    password: Option<String>,
) -> Result<InstallResultDto, String> {
    ensure_game_stopped(state.inner(), "安装 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let cfg = lock_mutex(&state.config, "config")?.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let result = install_entry(
            &lib,
            Hsr::shared(),
            Path::new(&path),
            character.as_deref(),
            password.as_deref(),
        );
        if let Ok(InstallResultDto::Installed { mod_id, .. }) = &result {
            let mod_id = *mod_id;
            maybe_auto_enable(&lib, &cfg, mod_id, Some(&app2));
            tracing::info!("installed mod {mod_id}");
        }
        result
    })
    .await
    .map_err(|e| format!("安装任务失败：{e}"))?
}

#[tauri::command]
pub async fn connect_external_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    character: Option<String>,
) -> Result<InstallResultDto, String> {
    ensure_game_stopped(state.inner(), "连接外部 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    let source = PathBuf::from(path);
    let app_for_task = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if !source.is_dir() {
            return Err(format!("外部 Mod 目录不存在：{}", source.display()));
        }
        let source_canonical = source
            .canonicalize()
            .map_err(|e| format!("无法读取外部 Mod 目录：{e}"))?;
        let cfg = lock_mutex(&config, "config")?.clone();
        if let Some(mods_dir) = cfg.mods_dir.as_deref() {
            if let Ok(mods_canonical) = mods_dir.canonicalize() {
                if source_canonical.starts_with(&mods_canonical)
                    || mods_canonical.starts_with(&source_canonical)
                {
                    return Err(
                        "不能连接 3DMigoto Mods 部署目录内的文件夹；请使用旧版本迁移功能导入"
                            .to_string(),
                    );
                }
            }
        }
        let name = source
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "无法从目录名识别 Mod 名称".to_string())?
            .to_string();
        let character = character
            .filter(|value| !value.trim().is_empty())
            .or_else(|| liquimod_core::games::infer_character(&source_canonical, Hsr::shared()))
            .unwrap_or_else(|| "Others".to_string());
        let lib = lock_mutex(&library, "library")?;
        let entry = lib
            .add_external_folder(&source_canonical, &character, &name)
            .map_err(|e| humanize_install_error(&e))?;
        maybe_auto_enable(&lib, &cfg, entry.id, Some(&app_for_task));
        Ok(InstallResultDto::Installed {
            mod_id: entry.id,
            name,
            character,
            warnings: vec!["已连接外部源目录；断开连接不会删除原文件".to_string()],
        })
    })
    .await
    .map_err(|e| format!("连接外部 Mod 任务失败：{e}"))?;
    crate::start_watcher(&app, state.inner());
    result
}

#[tauri::command]
pub async fn uninstall_mod(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "卸载 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let result = remove_entry(&lib, mods_dir.as_deref(), id);
        if result.is_ok() {
            tracing::info!("uninstalled mod {id}");
        }
        result
    })
    .await
    .map_err(|e| format!("卸载任务失败：{e}"))?
}

#[tauri::command]
pub async fn list_presets(state: tauri::State<'_, AppState>) -> Result<Vec<PresetDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        preset_dtos(&lib.db)
    })
    .await
    .map_err(|e| format!("读取预设失败：{e}"))?
}

#[tauri::command]
pub async fn save_preset(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<PresetDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let ids = liquimod_core::preset::snapshot_enabled(&lib).map_err(|e| e.to_string())?;
        let id = save_preset_named(&lib.db, &name, &ids)?;
        preset_dtos(&lib.db)?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "预设保存后读取失败".to_string())
    })
    .await
    .map_err(|e| format!("保存预设失败：{e}"))?
}

#[tauri::command]
pub async fn apply_preset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<ApplyResultDto, String> {
    ensure_game_stopped(state.inner(), "应用预设")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        let result = apply_preset_by_id(&lib, mods_dir.as_deref(), id);
        if let Ok(r) = &result {
            tracing::info!("applied preset {id}（{name}）");
            drop(lib);
            let _ = app2.emit(
                "liquimod-toast",
                format!(
                    "已应用预设「{name}」：启用 {} / 停用 {}",
                    r.enabled, r.disabled
                ),
            );
        }
        result
    })
    .await
    .map_err(|e| format!("应用预设失败：{e}"))?
}

#[tauri::command]
pub async fn delete_preset(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.delete_preset(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除预设失败：{e}"))?
}

#[tauri::command]
pub async fn list_passwords(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.list_passwords().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("读取密码本失败：{e}"))?
}

#[tauri::command]
pub async fn add_password(state: tauri::State<'_, AppState>, value: String) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let v = value.trim().to_string();
        if v.is_empty() {
            return Err("密码不能为空".to_string());
        }
        let lib = lock_mutex(&library, "library")?;
        lib.db.add_password(&v).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("添加密码失败：{e}"))?
}

#[tauri::command]
pub async fn remove_password(
    state: tauri::State<'_, AppState>,
    value: String,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.remove_password(&value).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移除密码失败：{e}"))?
}

#[tauri::command]
pub async fn rename_mod(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "重命名 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        rename_entry(&lib, mods_dir.as_deref(), id, &name)
    })
    .await
    .map_err(|e| format!("重命名任务失败：{e}"))?
}

#[tauri::command]
pub async fn reassign_mod(
    state: tauri::State<'_, AppState>,
    id: i64,
    target_character: String,
) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "移动 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = lock_mutex(&state.config, "config")?.mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        reassign_entry(&lib, mods_dir.as_deref(), id, &target_character)
    })
    .await
    .map_err(|e| format!("角色重分配失败：{e}"))?
}

#[tauri::command]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CategoryDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db
            .list_categories()
            .map_err(|e| e.to_string())
            .map(|cs| {
                cs.into_iter()
                    .map(|c| CategoryDto {
                        id: c.id,
                        name: c.name,
                        ord: c.ord,
                        kind: c.kind,
                        mod_count: c.mod_count,
                    })
                    .collect()
            })
    })
    .await
    .map_err(|e| format!("读取分类失败：{e}"))?
}

#[tauri::command]
pub async fn create_category(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<i64, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.create_category(&name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("新建分类失败：{e}"))?
}

#[tauri::command]
pub async fn rename_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.rename_category(id, &name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("重命名分类失败：{e}"))?
}

#[tauri::command]
pub async fn delete_category(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.delete_category(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除分类失败：{e}"))?
}

#[tauri::command]
pub async fn move_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    delta: i64,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db.move_category(id, delta).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移动分类失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    category_id: Option<i64>,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = lock_mutex(&library, "library")?;
        lib.db
            .set_mod_category(id, category_id)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移动 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn list_category_mods(
    state: tauri::State<'_, AppState>,
    category_id: i64,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = lock_mutex(&library, "library")?;
            let root = lib.layout.root.clone();
            let rows = collect_rows_where(&lib, move |m| m.category_id == Some(category_id))?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取分类 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn list_all_mods(state: tauri::State<'_, AppState>) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = lock_mutex(&library, "library")?;
            let root = lib.layout.root.clone();
            let rows = collect_rows_where(&lib, |_| true)?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取全部 Mod 失败：{e}"))?
}

/// 未分类 = 未归类（category_id NULL）且不属于任何已知游戏角色。

#[tauri::command]
pub async fn list_uncategorized_mods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = lock_mutex(&library, "library")?;
            let root = lib.layout.root.clone();
            let chars = Hsr::shared().characters();
            let known: std::collections::HashSet<String> =
                chars.iter().map(|c| c.internal_name.clone()).collect();
            let rows = collect_rows_where(&lib, |m| {
                m.category_id.is_none() && !known.contains(&m.character)
            })?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取未分类 Mod 失败：{e}"))?
}

#[tauri::command]
pub fn get_mod_keys(
    state: tauri::State<AppState>,
    id: i64,
) -> Result<Vec<ModKeyBindingDto>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let mod_dir = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;
    let keys = liquimod_core::d3d::scan_mod_keys(&mod_dir);
    Ok(keys
        .into_iter()
        .map(|k| ModKeyBindingDto {
            section: k.section,
            key: k.key,
            formatted_key: k.formatted_key,
            back: k.back,
            formatted_back: k.formatted_back,
            key_type: k.key_type,
            variable: k.variable,
            steps: k.steps,
            comment: k.comment,
        })
        .collect())
}

#[tauri::command]
pub fn set_mod_custom_cover(
    state: tauri::State<AppState>,
    id: i64,
    image_path: String,
) -> Result<Option<String>, String> {
    let src = PathBuf::from(image_path);
    if !src.is_file() {
        return Err("选择的图片文件不存在".to_string());
    }
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if row.storage_kind == ModStorageKind::External {
        return Err("外部连接 Mod 保持源目录只读，请从其现有图片中选择封面".to_string());
    }
    let mod_dir = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;

    let ext = src
        .extension()
        .map(|e| e.to_string_lossy().to_string())
        .unwrap_or_else(|| "png".to_string());
    let dest_name = format!("custom_cover_{}.{}", uuid::Uuid::new_v4(), ext);
    let dest = mod_dir.join(&dest_name);
    std::fs::copy(&src, &dest).map_err(|e| format!("保存外部封面失败：{e}"))?;

    lib.db
        .set_mod_cover_image(id, Some(&dest_name))
        .map_err(|e| e.to_string())?;

    // 清除并重新生成缩略图
    liquimod_core::thumbs::remove_thumbnail(&lib.layout.root, id);
    let new_thumb = thumb_data_url(&lib.layout.root, &mod_dir, id, Some(&dest_name));
    Ok(new_thumb)
}

#[tauri::command]
pub fn set_mod_cover_from_internal(
    state: tauri::State<AppState>,
    id: i64,
    relative_path: String,
) -> Result<String, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let mod_dir = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;

    // 安全防御 (LM-P1-004): 严禁利用 .. 越界逃出 Mod 文件夹
    let safe_rel = liquimod_core::safe_path::sanitize_relative_path(Path::new(&relative_path))
        .map_err(|e| format!("非法图片路径: {e}"))?;
    let src = liquimod_core::safe_path::ensure_contained(&mod_dir, &safe_rel)
        .map_err(|e| format!("图片路径越界: {e}"))?;

    if !src.is_file() {
        return Err("所选图片不存在".to_string());
    }

    let rel_str = safe_rel.to_string_lossy().replace('\\', "/");

    // 绝不拷贝覆盖磁盘原文件！直接将经过净化的规范相对路径写入 DB 持久化记录
    lib.db
        .set_mod_cover_image(id, Some(&rel_str))
        .map_err(|e| e.to_string())?;

    liquimod_core::thumbs::remove_thumbnail(&lib.layout.root, id);
    let new_thumb = thumb_data_url(&lib.layout.root, &mod_dir, id, Some(&rel_str));
    new_thumb.ok_or_else(|| "生成缩略图失败".to_string())
}

#[tauri::command]
pub fn reset_mod_cover(state: tauri::State<AppState>, id: i64) -> Result<Option<String>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let mod_dir = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;

    lib.db
        .set_mod_cover_image(id, None)
        .map_err(|e| e.to_string())?;

    liquimod_core::thumbs::remove_thumbnail(&lib.layout.root, id);
    let new_thumb = thumb_data_url(&lib.layout.root, &mod_dir, id, None);
    Ok(new_thumb)
}

#[tauri::command]
pub fn get_mod_cover_image(
    state: tauri::State<AppState>,
    id: i64,
) -> Result<Option<String>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let mod_dir = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;

    let Some(src) = liquimod_core::thumbs::find_preview_image(&mod_dir, row.cover_image.as_deref())
    else {
        return Ok(None);
    };

    let bytes = std::fs::read(&src).map_err(|e| e.to_string())?;
    let lower = src.to_string_lossy().to_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else if lower.ends_with(".avif") {
        "image/avif"
    } else {
        "image/jpeg"
    };

    use base64::Engine;
    let b64 = base64::prelude::BASE64_STANDARD.encode(&bytes);
    Ok(Some(format!("data:{mime};base64,{b64}")))
}

#[tauri::command]
pub fn get_active_conflicts(
    state: tauri::State<AppState>,
) -> Result<Vec<ConflictReportDto>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let conflicts = liquimod_core::d3d::detect_conflicts(&lib).map_err(|e| e.to_string())?;
    Ok(conflicts
        .into_iter()
        .map(|c| ConflictReportDto {
            hash: c.hash,
            section: c.section,
            conflicting_mods: c
                .conflicting_mods
                .into_iter()
                .map(|m| ConflictModInfoDto {
                    id: m.id,
                    character: m.character,
                    name: m.name,
                })
                .collect(),
        })
        .collect())
}

#[tauri::command]
pub fn get_active_variable_conflicts(
    state: tauri::State<AppState>,
) -> Result<Vec<VariableConflictDto>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let conflicts =
        liquimod_core::d3d::detect_variable_conflicts(&lib).map_err(|e| e.to_string())?;
    Ok(conflicts
        .into_iter()
        .map(|c| VariableConflictDto {
            variable: c.variable,
            conflicting_mods: c
                .conflicting_mods
                .into_iter()
                .map(|m| ConflictModInfoDto {
                    id: m.id,
                    character: m.character,
                    name: m.name,
                })
                .collect(),
        })
        .collect())
}

pub(crate) fn resolve_existing_explorer_path(path: &Path) -> Result<PathBuf, String> {
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("无法获取当前工作目录：{e}"))?
            .join(path)
    };

    let canonical = resolved
        .canonicalize()
        .or_else(|_| {
            // canonicalize 要求路径已存在；若失败但路径存在，返回绝对路径。
            if resolved.exists() {
                Ok(resolved.clone())
            } else {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "path does not exist",
                ))
            }
        })
        .map_err(|e| format!("路径不存在或不可访问：{} ({e})", resolved.display()))?;

    if !canonical.exists() {
        return Err(format!("路径不存在：{}", canonical.display()));
    }
    Ok(canonical)
}

pub(crate) fn open_in_explorer(path: &Path) -> Result<(), String> {
    let canonical = resolve_existing_explorer_path(path)?;
    tauri_plugin_opener::open_path(&canonical, None::<&str>)
        .map_err(|e| format!("打开目录失败：{e}"))?;
    Ok(())
}

#[tauri::command]
pub fn get_mod_images(state: tauri::State<AppState>, id: i64) -> Result<Vec<ModImageDto>, String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let mod_dir = match lib.entry_source_dir(&row) {
        Ok(path) => path,
        Err(_) => return Ok(Vec::new()),
    };
    if !mod_dir.is_dir() {
        return Ok(Vec::new());
    }

    let active_cover =
        liquimod_core::thumbs::find_preview_image(&mod_dir, row.cover_image.as_deref());

    const MAX_IMAGES_COUNT: usize = 60;
    const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024; // 15 MB

    let mut images = Vec::new();
    fn scan_imgs(
        dir: &Path,
        base: &Path,
        depth: usize,
        active_cover: Option<&PathBuf>,
        out: &mut Vec<ModImageDto>,
    ) {
        if depth > 6 || out.len() >= MAX_IMAGES_COUNT {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if out.len() >= MAX_IMAGES_COUNT {
                    break;
                }
                let p = entry.path();
                if p.is_file() {
                    let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    let lower = fname.to_lowercase();
                    if lower.ends_with(".png")
                        || lower.ends_with(".jpg")
                        || lower.ends_with(".jpeg")
                        || lower.ends_with(".webp")
                        || lower.ends_with(".bmp")
                        || lower.ends_with(".gif")
                        || lower.ends_with(".avif")
                    {
                        if let Ok(meta) = p.metadata() {
                            if meta.len() > MAX_IMAGE_BYTES {
                                continue;
                            }
                            let rel = p
                                .strip_prefix(base)
                                .map(|r| r.to_string_lossy().replace('\\', "/"))
                                .unwrap_or_else(|_| fname.to_string());

                            if let Ok(bytes) = std::fs::read(&p) {
                                let mime = if lower.ends_with(".png") {
                                    "image/png"
                                } else if lower.ends_with(".webp") {
                                    "image/webp"
                                } else if lower.ends_with(".gif") {
                                    "image/gif"
                                } else if lower.ends_with(".bmp") {
                                    "image/bmp"
                                } else if lower.ends_with(".avif") {
                                    "image/avif"
                                } else {
                                    "image/jpeg"
                                };
                                use base64::Engine;
                                let b64 = base64::prelude::BASE64_STANDARD.encode(&bytes);

                                let (w, h) = match image::image_dimensions(&p) {
                                    Ok((w, h)) => (Some(w), Some(h)),
                                    Err(_) => (None, None),
                                };

                                let is_cover = active_cover.map(|c| c == &p).unwrap_or(false);

                                out.push(ModImageDto {
                                    relative_path: rel,
                                    filename: fname.to_string(),
                                    size_bytes: meta.len(),
                                    data_url: format!("data:{mime};base64,{b64}"),
                                    is_cover,
                                    width: w,
                                    height: h,
                                });
                            }
                        }
                    }
                } else if p.is_dir()
                    && !entry.file_type().map(|ft| ft.is_symlink()).unwrap_or(false)
                {
                    scan_imgs(&p, base, depth + 1, active_cover, out);
                }
            }
        }
    }

    scan_imgs(&mod_dir, &mod_dir, 0, active_cover.as_ref(), &mut images);
    images.sort_by(|a, b| match (a.is_cover, b.is_cover) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.relative_path.cmp(&b.relative_path),
    });

    Ok(images)
}

#[derive(Debug, serde::Serialize)]
pub struct RescanResultDto {
    pub added: usize,
    pub removed: usize,
}
