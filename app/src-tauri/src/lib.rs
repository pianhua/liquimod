mod commands;
mod config;
mod state;

use liquimod_core::deploy::Deployer;
use state::AppState;
use std::sync::Arc;
use tauri::{Emitter, Manager};

/// （重）启动目录监控：变动 → 对账 → emit library-changed（added/removed 为 Mod 数增量）。
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
        let before = lib.list().map(|m| m.len()).unwrap_or(0);
        if lib.scan().is_err() {
            return;
        }
        if let Some(dir) = &mods_dir2 {
            let _ = Deployer::new(&lib, dir).reconcile();
        }
        let after = lib.list().map(|m| m.len()).unwrap_or(0);
        drop(lib);
        let added = after.saturating_sub(before);
        let removed = before.saturating_sub(after);
        let _ = app2.emit(
            "library-changed",
            serde_json::json!({ "added": added, "removed": removed }),
        );
    });
    if let Ok(w) = watcher {
        *state.watcher.lock().unwrap() = Some(w);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::bootstrap())
        .setup(|app| {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
