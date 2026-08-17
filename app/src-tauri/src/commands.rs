use crate::config::Config;
use crate::state::AppState;
use liquimod_core::deploy::Deployer;
use liquimod_core::games::{CharacterInfo, Game};
use liquimod_core::library::Library;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigDto {
    pub library_root: String,
    pub mods_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CharacterSummary {
    pub internal_name: String,
    pub display_name: String,
    pub image: Option<String>,
    pub total: usize,
    pub enabled: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModDto {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub installed_at: i64,
}

pub fn config_dto(c: &Config) -> ConfigDto {
    ConfigDto {
        library_root: c.library_root.display().to_string(),
        mods_dir: c.mods_dir.as_ref().map(|p| p.display().to_string()),
    }
}

/// 角色网格汇总：游戏角色按数据顺序在前，未匹配的 Mod 归入最后的 "Others"。
pub fn character_summaries(
    lib: &Library,
    game: &dyn Game,
) -> Result<Vec<CharacterSummary>, String> {
    let mods = lib.list().map_err(|e| e.to_string())?;
    let mut out: Vec<CharacterSummary> = Vec::new();
    for c in game.characters() {
        let group: Vec<_> = mods
            .iter()
            .filter(|m| m.character == c.internal_name)
            .collect();
        out.push(summary(
            c,
            group.len(),
            group.iter().filter(|m| m.enabled).count(),
        ));
    }
    let known: Vec<&str> = game
        .characters()
        .iter()
        .map(|c| c.internal_name.as_str())
        .collect();
    let others: Vec<_> = mods
        .iter()
        .filter(|m| !known.contains(&m.character.as_str()))
        .collect();
    if !others.is_empty() {
        out.push(CharacterSummary {
            internal_name: "Others".into(),
            display_name: "其他".into(),
            image: None,
            total: others.len(),
            enabled: others.iter().filter(|m| m.enabled).count(),
        });
    }
    Ok(out)
}

fn summary(c: &CharacterInfo, total: usize, enabled: usize) -> CharacterSummary {
    CharacterSummary {
        internal_name: c.internal_name.clone(),
        display_name: c.display_name.clone(),
        image: Some(c.image.clone()),
        total,
        enabled,
    }
}

pub fn mod_list(lib: &Library, character: &str) -> Result<Vec<ModDto>, String> {
    let mut mods: Vec<ModDto> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| m.character == character)
        .map(|m| ModDto {
            id: m.id,
            name: m.name,
            enabled: m.enabled,
            installed_at: m.installed_at,
        })
        .collect();
    mods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(mods)
}

pub fn set_enabled(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录，请先选择目录")?;
    if !mods_dir.is_dir() {
        return Err(format!("Mods 目录不存在：{}", mods_dir.display()));
    }
    let deployer = Deployer::new(lib, mods_dir);
    let r = if enabled {
        deployer.enable(id)
    } else {
        deployer.disable(id)
    };
    r.map_err(|e| e.to_string())
}

pub fn set_mods_dir(c: &mut Config, path: PathBuf) -> Result<ConfigDto, String> {
    if !path.is_dir() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    c.mods_dir = Some(path);
    Ok(config_dto(c))
}

// ---- Tauri 薄命令 ----

#[tauri::command]
pub fn get_config(state: tauri::State<AppState>) -> ConfigDto {
    config_dto(&state.config.lock().unwrap())
}

#[tauri::command]
pub fn choose_mods_dir(state: tauri::State<AppState>, path: String) -> Result<ConfigDto, String> {
    let mut config = state.config.lock().unwrap();
    let dto = set_mods_dir(&mut config, PathBuf::from(path))?;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(dto)
}

#[tauri::command]
pub fn get_characters(state: tauri::State<AppState>) -> Result<Vec<CharacterSummary>, String> {
    let lib = state.library.lock().unwrap();
    character_summaries(&lib, liquimod_core::games::hsr::Hsr::shared())
}

#[tauri::command]
pub fn list_mods(state: tauri::State<AppState>, character: String) -> Result<Vec<ModDto>, String> {
    let lib = state.library.lock().unwrap();
    mod_list(&lib, &character)
}

#[tauri::command]
pub fn set_mod_enabled(
    state: tauri::State<AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let lib = state.library.lock().unwrap();
    set_enabled(&lib, mods_dir.as_deref(), id, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquimod_core::games::hsr::Hsr;
    use std::fs;

    fn temp_lib() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::init(dir.path()).unwrap();
        (dir, lib)
    }

    #[test]
    fn summaries_group_mods_and_bucket_others() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        lib.add_folder(src.path(), "Stranger", "M2").unwrap();
        let out = character_summaries(&lib, Hsr::shared()).unwrap();
        let acheron = out.iter().find(|c| c.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.total, 1);
        let others = out.iter().find(|c| c.internal_name == "Others").unwrap();
        assert_eq!(others.total, 1);
        assert_eq!(others.image, None);
    }

    #[test]
    fn mod_list_filters_and_sorts() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "B").unwrap();
        lib.add_folder(src.path(), "Acheron", "A").unwrap();
        lib.add_folder(src.path(), "Bailu", "C").unwrap();
        let mods = mod_list(&lib, "Acheron").unwrap();
        assert_eq!(
            mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn set_enabled_requires_mods_dir() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        let entry = lib.add_folder(src.path(), "Acheron", "M").unwrap();
        let err = set_enabled(&lib, None, entry.id, true).unwrap_err();
        assert!(err.contains("Mods 目录"));
    }

    #[test]
    fn set_enabled_creates_and_removes_junction() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        fs::write(src.path().join("f.txt"), "x").unwrap();
        let entry = lib.add_folder(src.path(), "Acheron", "M").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), entry.id, true).unwrap();
        assert!(mods.path().join(Deployer::link_name(&entry)).exists());
        set_enabled(&lib, Some(mods.path()), entry.id, false).unwrap();
        assert!(!mods.path().join(Deployer::link_name(&entry)).exists());
    }

    #[test]
    fn set_mods_dir_rejects_missing() {
        let mut c = Config {
            library_root: PathBuf::from("x"),
            mods_dir: None,
        };
        assert!(set_mods_dir(&mut c, PathBuf::from("C:/no/such/dir")).is_err());
        assert!(c.mods_dir.is_none());
    }
}
