mod commands;
mod config;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::bootstrap())
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
