mod commands;
mod config;
mod state;

use state::AppState;
use std::sync::Arc;
use std::time::Duration;
use tauri::{Emitter, Manager};

/// 对账并求增量（纯函数，便于单测）：返回 (added, removed) 为 (character, name) 集合差集大小。
pub fn reconcile_and_diff(
    lib: &liquimod_core::library::Library,
    mods_dir: Option<&std::path::Path>,
) -> Result<(usize, usize), String> {
    reconcile_and_diff_with_deploy(lib, mods_dir, true)
}

/// 扫描库并可选地对齐物理部署。游戏运行期间只允许更新索引，避免外部文件监控
/// 绕过命令层防呆而重建 Junction 或复制部署目录。
pub fn reconcile_and_diff_with_deploy(
    lib: &liquimod_core::library::Library,
    mods_dir: Option<&std::path::Path>,
    deploy: bool,
) -> Result<(usize, usize), String> {
    reconcile_and_diff_with_sources(lib, mods_dir, &[], deploy)
}

/// 扫描托管库与配置的外部源，并按需重建 3Dmigoto 运行入口。
pub fn reconcile_and_diff_with_sources(
    lib: &liquimod_core::library::Library,
    mods_dir: Option<&std::path::Path>,
    external_sources: &[std::path::PathBuf],
    deploy: bool,
) -> Result<(usize, usize), String> {
    use std::collections::HashSet;
    let key = |m: &liquimod_core::models::ModEntry| (m.character.clone(), m.name.clone());
    let before: HashSet<_> = lib
        .list()
        .map_err(|e| e.to_string())?
        .iter()
        .map(key)
        .collect();
    lib.scan_external_sources(external_sources, liquimod_core::games::hsr::Hsr::shared())
        .map_err(|e| format!("外部 Mod 源扫描失败：{e}"))?;
    lib.scan().map_err(|e| e.to_string())?;
    // 扫描后统一归类（仅对未分类 Mod 赋初始分类）
    commands::sync_mod_categories(lib, liquimod_core::games::hsr::Hsr::shared())
        .map_err(|e| format!("分类对齐失败：{e}"))?;
    if deploy {
        if let Some(dir) = mods_dir {
            liquimod_core::deploy::Deployer::new(lib, dir)
                .reconcile()
                .map_err(|e| format!("部署状态对齐失败：{e}"))?;
        }
    }
    let after: HashSet<_> = lib
        .list()
        .map_err(|e| e.to_string())?
        .iter()
        .map(key)
        .collect();
    Ok((
        after.difference(&before).count(),
        before.difference(&after).count(),
    ))
}

/// （重）启动目录监控：变动 → 对账 → emit library-changed（added/removed 为 Mod 增量）。
/// 绝不改动用户文件：scan 只对账 DB，reconcile 只清指向仓库内的孤儿链接。
pub fn start_watcher(app: &tauri::AppHandle, state: &AppState) {
    let (root, mods_dir, configured_sources) = {
        let cfg = state.config.lock().unwrap();
        (
            cfg.library_root.clone(),
            cfg.mods_dir.clone(),
            cfg.mod_sources.clone(),
        )
    };
    let library = Arc::clone(&state.library);
    let mut external_sources = configured_sources.clone();
    external_sources.extend(
        library
            .lock()
            .unwrap()
            .list()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|entry| entry.source_path.map(std::path::PathBuf::from))
            .filter(|path| path.is_dir()),
    );
    let game_running = Arc::clone(&state.game_running);
    let app2 = app.clone();
    let mods_dir2 = mods_dir.clone();
    let external_sources2 = configured_sources.clone();
    let watcher = liquimod_core::watch::start(root, mods_dir, external_sources, move || {
        let lib = library.lock().unwrap();
        let deploy = !game_running.load(std::sync::atomic::Ordering::Relaxed);
        match reconcile_and_diff_with_sources(
            &lib,
            mods_dir2.as_deref(),
            &external_sources2,
            deploy,
        ) {
            Ok((added, removed)) => {
                drop(lib);
                tracing::info!("reconcile: +{added} / -{removed}");
                let _ = app2.emit(
                    "library-changed",
                    serde_json::json!({ "added": added, "removed": removed }),
                );
            }
            Err(e) => {
                drop(lib);
                let _ = app2.emit("liquimod-toast", format!("仓库对账失败：{e}"));
            }
        }
    });
    if let Ok(w) = watcher {
        let old = state.watcher.lock().unwrap().take();
        drop(old); // join 防抖线程在锁外进行
        *state.watcher.lock().unwrap() = Some(w);
    } else {
        let _ = app.emit(
            "liquimod-toast",
            "目录监控启动失败，本次改动不会被自动侦测".to_string(),
        );
    }
}

/// 启动或重启游戏进程看门狗，只在状态变化时广播事件。
pub fn start_game_watchdog(app: &tauri::AppHandle, state: &AppState) {
    let process_names = {
        let config = state.config.lock().unwrap();
        commands::configured_game_process_names(&config)
    };
    let running = Arc::clone(&state.game_running);
    let library = Arc::clone(&state.library);
    let config = Arc::clone(&state.config);
    let deferred_runtime_cleanup = Arc::clone(&state.deferred_runtime_cleanup);
    let app2 = app.clone();
    let watchdog = liquimod_core::refresh::GameWatchdog::start(
        process_names,
        Duration::from_secs(2),
        move |is_running| {
            running.store(is_running, std::sync::atomic::Ordering::Relaxed);
            let _ = app2.emit(
                "game-status-changed",
                commands::GameStatusDto {
                    running: is_running,
                },
            );
            if !is_running {
                let library = Arc::clone(&library);
                let config = Arc::clone(&config);
                let deferred_runtime_cleanup = Arc::clone(&deferred_runtime_cleanup);
                let app3 = app2.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let (mods_dir, sources) = {
                        let cfg = config.lock().unwrap();
                        (cfg.mods_dir.clone(), cfg.mod_sources.clone())
                    };
                    let lib = library.lock().unwrap();
                    if let Some(dir) = mods_dir.as_deref() {
                        if let Err(error) =
                            liquimod_core::deploy::Deployer::new(&lib, dir).recover()
                        {
                            let _ = app3
                                .emit("liquimod-toast", format!("游戏退出后事务恢复失败：{error}"));
                        }
                    }
                    match reconcile_and_diff_with_sources(&lib, mods_dir.as_deref(), &sources, true)
                    {
                        Ok((added, removed)) if added > 0 || removed > 0 => {
                            deferred_runtime_cleanup.lock().unwrap().clear();
                            let _ = app3.emit(
                                "library-changed",
                                serde_json::json!({ "added": added, "removed": removed }),
                            );
                        }
                        Ok(_) => {
                            deferred_runtime_cleanup.lock().unwrap().clear();
                        }
                        Err(error) => {
                            let _ = app3
                                .emit("liquimod-toast", format!("游戏退出后部署对账失败：{error}"));
                        }
                    }
                });
            }
        },
    );
    let old = state.game_watchdog.lock().unwrap().replace(watchdog);
    drop(old);
}
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let log_dir = config::Config::log_dir();
    std::fs::create_dir_all(&log_dir).ok();
    let appender = tracing_appender::rolling::daily(&log_dir, "liquimod.log");
    let (nb, guard) = tracing_appender::non_blocking(appender);
    // guard 需活到 run() 返回：作为局部变量绑定，进程退出时 natural flush 缓冲日志落盘。
    // 勿 mem::forget——那会跳过 flush。
    let _guard = guard;
    tracing_subscriber::fmt()
        .with_writer(nb)
        .with_ansi(false)
        .with_max_level(tracing::Level::INFO)
        .init();
    tracing::info!("LiquiMod starting");
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(state::AppState::bootstrap())
        .setup(|app| {
            // 启动恢复：完成上次崩溃遗留的启停事务（op_log）
            let state = app.state::<AppState>();
            let (mods_dir, process_names) = {
                let config = state.config.lock().unwrap();
                (
                    config.mods_dir.clone(),
                    commands::configured_game_process_names(&config),
                )
            };
            let running = {
                let names: Vec<&str> = process_names.iter().map(String::as_str).collect();
                liquimod_core::refresh::is_game_running(&names)
            };
            state
                .game_running
                .store(running, std::sync::atomic::Ordering::Relaxed);
            if let Some(dir) = mods_dir.as_deref() {
                if running {
                    tracing::warn!("game is running; deferred startup deployment recovery");
                } else {
                    let lib = state.library.lock().unwrap();
                    if let Err(e) = liquimod_core::deploy::Deployer::new(&lib, dir).recover() {
                        tracing::warn!("startup recover failed: {e}");
                    }
                }
            }
            // 启动对账：索引库目录、统计大小/文件数、对齐 junction（含应用关闭期间的外部变动）
            {
                let lib = state.library.lock().unwrap();
                let deploy = !running;
                let sources = state.config.lock().unwrap().mod_sources.clone();
                match reconcile_and_diff_with_sources(&lib, mods_dir.as_deref(), &sources, deploy) {
                    Ok((added, removed)) => tracing::info!("startup scan: +{added} / -{removed}"),
                    Err(e) => tracing::warn!("startup scan failed: {e}"),
                }
            }
            let app_handle = app.handle().clone();
            start_watcher(&app_handle, app.state::<AppState>().inner());
            start_game_watchdog(&app_handle, app.state::<AppState>().inner());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::choose_mods_dir,
            commands::add_mod_source,
            commands::remove_mod_source,
            commands::get_storage_info,
            commands::migrate_storage,
            commands::cleanup_previous_library,
            commands::get_characters,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::set_mod_variant,
            commands::install_mod,
            commands::connect_external_mod,
            commands::uninstall_mod,
            commands::rename_mod,
            commands::reassign_mod,
            commands::set_auto_enable,
            commands::set_warn_multiple_mods,
            commands::read_log,
            commands::list_presets,
            commands::save_preset,
            commands::apply_preset,
            commands::delete_preset,
            commands::list_passwords,
            commands::add_password,
            commands::remove_password,
            commands::list_categories,
            commands::create_category,
            commands::rename_category,
            commands::delete_category,
            commands::move_category,
            commands::set_mod_category,
            commands::list_category_mods,
            commands::list_all_mods,
            commands::list_uncategorized_mods,
            commands::set_theme,
            commands::set_character_category_name,
            commands::choose_game_exe,
            commands::get_game_status,
            commands::launch_game,
            commands::launch_game_native,
            commands::launch_official_launcher,
            commands::inspect_3dmigoto_dir,
            commands::import_3dmigoto_dir,
            commands::get_mod_keys,
            commands::set_mod_custom_cover,
            commands::get_active_conflicts,
            commands::get_active_variable_conflicts,
            commands::open_mod_folder,
            commands::open_path_in_explorer,
            commands::trigger_refresh_game,
            commands::get_mod_images,
            commands::set_mod_cover_from_internal,
            commands::reset_mod_cover,
            commands::get_mod_cover_image,
            commands::rescan_library,
            commands::clean_cache,
            commands::get_diagnostic_status,
            commands::repair_deployment,
            commands::open_webview2_download,
            commands::get_local_asset_version,
            commands::check_game_assets_update,
            commands::sync_game_assets,
            commands::get_character_image_data,
            commands::set_mod_note,
            commands::toggle_favorite_character,
            commands::toggle_favorite_mod,
            commands::reorder_mods,
            commands::auto_detect_game_exe,
            commands::init_migoto_workspace,
            commands::check_migoto_update,
            commands::check_xxmi_update,
            commands::get_core_package_status,
            commands::install_migoto_update,
            commands::install_srmi_update,
            commands::install_xxmi_update,
            commands::switch_to_managed_migoto,
            commands::migrate_mods_from_old_migoto,
            commands::set_work_mode,
            commands::set_injection_delay,
            commands::set_github_token,
            commands::set_github_mirror,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::reconcile_and_diff;

    #[test]
    fn reconcile_diff_detects_replace_same_count() {
        // 计数法会给 (0,0)：len 都是 2 —— 这正是被修复的 bug。
        use std::collections::HashSet;
        let before: HashSet<(String, String)> =
            [("A".into(), "m1".into()), ("A".into(), "m2".into())]
                .into_iter()
                .collect();
        let after: HashSet<(String, String)> =
            [("A".into(), "m1".into()), ("B".into(), "m2".into())]
                .into_iter()
                .collect();
        assert_eq!(after.difference(&before).count(), 1);
        assert_eq!(before.difference(&after).count(), 1);
        assert_eq!(before.len(), after.len());
    }

    #[test]
    fn reconcile_and_diff_reports_rename_as_add_remove() {
        // 模拟一个防抖窗口内的 删除+重建：同一 (character,name) 位置换成新名字，
        // 数量不变但集合差集应报告 (1,1)，而不是旧计数法的 (0,0)。
        let tmp = tempfile::tempdir().unwrap();
        let lib = liquimod_core::library::Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        lib.add_folder(src.path(), "A", "m1").unwrap();
        assert_eq!(reconcile_and_diff(&lib, None).unwrap(), (0, 0));

        std::fs::remove_dir_all(lib.layout.mod_dir("A", "m1")).unwrap();
        std::fs::create_dir_all(lib.layout.mod_dir("A", "m2")).unwrap();
        std::fs::write(lib.layout.mod_dir("A", "m2").join("mod.ini"), b"x").unwrap();

        let (added, removed) = reconcile_and_diff(&lib, None).unwrap();
        assert_eq!((added, removed), (1, 1));
    }

    #[test]
    fn reconcile_and_diff_cleans_orphan_link_with_mods_dir() {
        // 传入 mods_dir 时应走 reconcile 分支（清孤儿链接），不报错。
        let tmp = tempfile::tempdir().unwrap();
        let lib = liquimod_core::library::Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        lib.add_folder(src.path(), "A", "m1").unwrap();

        let mods = tempfile::tempdir().unwrap();
        assert_eq!(reconcile_and_diff(&lib, Some(mods.path())).unwrap(), (0, 0));
    }
}
