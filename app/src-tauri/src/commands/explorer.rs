use super::*;

use super::open_in_explorer;

#[tauri::command]
pub fn open_mod_folder(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let lib = lock_mutex(&state.library, "library")?;
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let path = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;
    tracing::info!(
        mod_id = id,
        storage_kind = ?row.storage_kind,
        path = %path.display(),
        "open_mod_folder"
    );
    open_in_explorer(&path)
}

#[tauri::command]
pub fn open_path_in_explorer(path: String) -> Result<(), String> {
    open_in_explorer(Path::new(&path))
}
