mod commands;
mod config;
mod state;

use state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// 对账并求增量（纯函数，便于单测）：返回 (added, removed) 为 (character, name) 集合差集大小。
pub fn reconcile_and_diff(
    lib: &liquimod_core::library::Library,
    mods_dir: Option<&std::path::Path>,
) -> Result<(usize, usize), String> {
    use std::collections::HashSet;
    let key = |m: &liquimod_core::models::ModEntry| (m.character.clone(), m.name.clone());
    let before: HashSet<_> = lib
        .list()
        .map_err(|e| e.to_string())?
        .iter()
        .map(key)
        .collect();
    lib.scan().map_err(|e| e.to_string())?;
    // 扫描后统一归类（角色→NULL；非角色→对应固定分类）
    let _ = commands::sync_mod_categories(lib, liquimod_core::games::hsr::Hsr::shared());
    if let Some(dir) = mods_dir {
        let _ = liquimod_core::deploy::Deployer::new(lib, dir).reconcile();
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
    let (root, mods_dir) = {
        let cfg = state.config.lock().unwrap();
        (cfg.library_root.clone(), cfg.mods_dir.clone())
    };
    let library = Arc::clone(&state.library);
    let app2 = app.clone();
    let mods_dir2 = mods_dir.clone();
    let watcher = liquimod_core::watch::start(root, mods_dir, move || {
        let lib = library.lock().unwrap();
        match reconcile_and_diff(&lib, mods_dir2.as_deref()) {
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
            let mods_dir = state.config.lock().unwrap().mods_dir.clone();
            if let Some(dir) = mods_dir {
                let lib = state.library.lock().unwrap();
                if let Err(e) = liquimod_core::deploy::Deployer::new(&lib, &dir).recover() {
                    tracing::warn!("startup recover failed: {e}");
                }
            }
            // 启动对账：索引库目录、统计大小/文件数、对齐 junction（含应用关闭期间的外部变动）
            {
                let lib = state.library.lock().unwrap();
                let mods_dir = state.config.lock().unwrap().mods_dir.clone();
                match reconcile_and_diff(&lib, mods_dir.as_deref()) {
                    Ok((added, removed)) => tracing::info!("startup scan: +{added} / -{removed}"),
                    Err(e) => tracing::warn!("startup scan failed: {e}"),
                }
            }
            let app_handle = app.handle().clone();
            start_watcher(&app_handle, app.state::<AppState>().inner());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::choose_mods_dir,
            commands::get_characters,
            commands::list_mods,
            commands::set_mod_enabled,
            commands::install_mod,
            commands::uninstall_mod,
            commands::rename_mod,
            commands::reassign_mod,
            commands::set_auto_enable,
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
            commands::choose_loader_exe,
            commands::launch_game,
            commands::launch_game_native,
            commands::launch_official_launcher,
            commands::launch_loader,
            commands::inspect_3dmigoto_dir,
            commands::import_3dmigoto_dir,
            commands::get_mod_keys,
            commands::set_mod_custom_cover,
            commands::get_active_conflicts,
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
            commands::install_migoto_update,
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
