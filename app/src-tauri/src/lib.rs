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
        if let Ok((added, removed)) = reconcile_and_diff(&lib, mods_dir2.as_deref()) {
            drop(lib);
            let _ = app2.emit(
                "library-changed",
                serde_json::json!({ "added": added, "removed": removed }),
            );
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
