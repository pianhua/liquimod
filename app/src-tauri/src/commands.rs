use crate::config::Config;
use crate::state::AppState;
use base64::Engine;
use liquimod_core::archive::install::{install_archive, install_archive_inferred, InstallOutcome};
use liquimod_core::deploy::Deployer;
use liquimod_core::error::LiquiModError;
use liquimod_core::games::hsr::Hsr;
use liquimod_core::games::{CharacterInfo, Game};
use liquimod_core::library::Library;
use liquimod_core::refresh::{is_game_running, RefreshClient, HELPER_EXE};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Emitter;

/// 游戏运行中则通知 helper 发 F10；失败只 toast 不阻断。
/// 阻塞（UAC 弹窗 + 最多 5s 管道轮询）：必须在 spawn_blocking 工作线程内调用。
fn maybe_refresh_game(app: &tauri::AppHandle, refresh: &Mutex<Option<RefreshClient>>) {
    if !is_game_running(Hsr::shared().process_names()) {
        return;
    }
    let helper = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(HELPER_EXE)));
    let Some(helper) = helper.filter(|p| p.exists()) else {
        let _ = app.emit(
            "liquimod-toast",
            "未找到刷新 helper，跳过游戏内刷新".to_string(),
        );
        return;
    };
    let mut guard = refresh.lock().unwrap();
    if guard.is_none() {
        match RefreshClient::connect_or_launch(&helper) {
            Ok(c) => *guard = Some(c),
            Err(e) => {
                let _ = app.emit("liquimod-toast", format!("刷新 helper 启动失败：{e}"));
                return;
            }
        }
    }
    if let Some(client) = guard.as_mut() {
        if client.poke().is_err() {
            *guard = None; // helper 死了，下次重连
            let _ = app.emit(
                "liquimod-toast",
                "刷新 helper 连接断开，下次操作将重试".to_string(),
            );
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigDto {
    pub library_root: String,
    pub mods_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PresetDto {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApplyResultDto {
    pub enabled: usize,
    pub disabled: usize,
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
    pub thumb: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InstallResultDto {
    Installed {
        mod_id: i64,
        name: String,
        character: String,
        warnings: Vec<String>,
    },
    NeedsPassword,
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
    let root = lib.layout.root.clone();
    let mut mods: Vec<ModDto> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| m.character == character)
        .map(|m| {
            let dir = lib.layout.mod_dir(&m.character, &m.name);
            let thumb = thumb_data_url(&root, &dir, m.id);
            ModDto {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                thumb,
            }
        })
        .collect();
    mods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(mods)
}

pub fn save_preset_named(
    db: &liquimod_core::db::Database,
    name: &str,
    mod_ids: &[i64],
) -> Result<i64, String> {
    db.save_preset(name, mod_ids).map_err(|e| e.to_string())
}

pub fn preset_dtos(db: &liquimod_core::db::Database) -> Result<Vec<PresetDto>, String> {
    db.list_presets().map_err(|e| e.to_string()).map(|ps| {
        ps.into_iter()
            .map(|p| PresetDto {
                id: p.id,
                name: p.name,
                created_at: p.created_at,
            })
            .collect()
    })
}

pub fn apply_preset_by_id(
    lib: &Library,
    mods_dir: Option<&Path>,
    preset_id: i64,
) -> Result<ApplyResultDto, String> {
    let mods_dir = mods_dir.ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法应用预设".to_string())?;
    let (enabled, disabled) =
        liquimod_core::preset::apply_preset(lib, mods_dir, preset_id).map_err(|e| e.to_string())?;
    Ok(ApplyResultDto { enabled, disabled })
}

/// 缩略图 data URL；缓存未生成时现场生成，失败静默为 None（不阻断列表）。
pub fn thumb_data_url(library_root: &Path, mod_dir: &Path, mod_id: i64) -> Option<String> {
    let path = liquimod_core::thumbs::ensure_thumbnail(library_root, mod_dir, mod_id)
        .ok()
        .flatten()?;
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
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

/// 安装压缩包：character=None 时从内容推断。人话错误信息。
pub fn install_entry(
    lib: &Library,
    game: &dyn Game,
    path: &Path,
    character: Option<&str>,
    password: Option<&str>,
) -> Result<InstallResultDto, String> {
    if !path.exists() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    if !path.is_file() {
        return Err("不支持的内容：请拖入压缩包文件".to_string());
    }
    let outcome = match character {
        Some(c) => install_archive(&lib.db, lib, path, c, password),
        None => install_archive_inferred(&lib.db, lib, game, path, password),
    };
    match outcome {
        Ok(InstallOutcome::Installed {
            mod_id,
            name,
            character,
            warnings,
        }) => Ok(InstallResultDto::Installed {
            mod_id,
            name,
            character,
            warnings,
        }),
        Ok(InstallOutcome::NeedsPassword) => Ok(InstallResultDto::NeedsPassword),
        Err(error) => Err(humanize_install_error(&error)),
    }
}

fn humanize_install_error(error: &LiquiModError) -> String {
    match error {
        LiquiModError::DestinationExists { name, .. } => format!("已存在同名 Mod：{name}"),
        LiquiModError::InvalidName(_) => {
            "压缩包或角色名称不合法（不能含路径分隔符等特殊字符）".to_string()
        }
        LiquiModError::Io(_) => "文件读写失败（可能磁盘空间不足或文件被占用）".to_string(),
        LiquiModError::UnsupportedArchive(_) => {
            "不是支持的压缩包（支持 zip / 7z / rar）".to_string()
        }
        LiquiModError::Archive { .. } => "压缩包损坏或读取失败".to_string(),
        LiquiModError::Db(_) => "数据库错误，请重启应用".to_string(),
        _ => error.to_string(),
    }
}

/// 卸载：启用中则先拆 Junction，再删库目录与 DB 记录。
pub fn remove_entry(lib: &Library, mods_dir: Option<&Path>, id: i64) -> Result<(), String> {
    let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if entry.enabled {
        let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录")?;
        Deployer::new(lib, mods_dir)
            .disable(id)
            .map_err(|e| e.to_string())?;
    }
    let dir = lib.layout.root.join(&entry.rel_path);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("删除 Mod 文件失败，可能有文件被占用".to_string()),
    }
    lib.db.remove_mod(id).map_err(|e| e.to_string())
}

// ---- Tauri 薄命令 ----

#[tauri::command]
pub fn get_config(state: tauri::State<AppState>) -> ConfigDto {
    config_dto(&state.config.lock().unwrap())
}

#[tauri::command]
pub fn choose_mods_dir(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    path: String,
) -> Result<ConfigDto, String> {
    let dto = {
        let mut config = state.config.lock().unwrap();
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
pub async fn get_characters(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CharacterSummary>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        character_summaries(&lib, liquimod_core::games::hsr::Hsr::shared())
    })
    .await
    .map_err(|e| format!("读取角色失败：{e}"))?
}

#[tauri::command]
pub async fn list_mods(
    state: tauri::State<'_, AppState>,
    character: String,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        mod_list(&lib, &character)
    })
    .await
    .map_err(|e| format!("读取 Mod 列表失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_enabled(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = set_enabled(&lib, mods_dir.as_deref(), id, enabled);
        if result.is_ok() {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            maybe_refresh_game(&app2, &refresh);
        }
        result
    })
    .await
    .map_err(|e| format!("切换 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn install_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
    character: Option<String>,
    password: Option<String>,
) -> Result<InstallResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = install_entry(
            &lib,
            Hsr::shared(),
            Path::new(&path),
            character.as_deref(),
            password.as_deref(),
        );
        if matches!(result, Ok(InstallResultDto::Installed { .. })) {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            maybe_refresh_game(&app2, &refresh);
        }
        result
    })
    .await
    .map_err(|e| format!("安装任务失败：{e}"))?
}

#[tauri::command]
pub async fn uninstall_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = remove_entry(&lib, mods_dir.as_deref(), id);
        if result.is_ok() {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            maybe_refresh_game(&app2, &refresh);
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
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = apply_preset_by_id(&lib, mods_dir.as_deref(), id);
        if let Ok(r) = &result {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            let _ = app2.emit(
                "liquimod-toast",
                format!(
                    "已应用预设「{name}」：启用 {} / 停用 {}",
                    r.enabled, r.disabled
                ),
            );
            maybe_refresh_game(&app2, &refresh);
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
        let lib = library.lock().unwrap();
        lib.db.delete_preset(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除预设失败：{e}"))?
}

#[tauri::command]
pub async fn list_passwords(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
        lib.db.remove_password(&value).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移除密码失败：{e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquimod_core::games::hsr::Hsr;
    use std::fs;

    #[test]
    fn library_changed_payload_shape() {
        let v = serde_json::json!({ "added": 2usize, "removed": 1usize });
        assert_eq!(v["added"], 2);
        assert_eq!(v["removed"], 1);
        assert!(v.get("count").is_none());
    }

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

    fn write_zip(path: &std::path::Path, files: &[(&str, &[u8])], password: Option<&str>) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in files {
            let options = match password {
                Some(p) => zip::write::SimpleFileOptions::default()
                    .with_aes_encryption(zip::AesMode::Aes256, p),
                None => zip::write::SimpleFileOptions::default(),
            };
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn install_entry_with_explicit_character() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("CoolMod.zip");
        write_zip(&zip, &[("mod.ini", b"[Constants]")], None);

        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();

        let InstallResultDto::Installed {
            character, name, ..
        } = dto
        else {
            panic!("expected installed");
        };
        assert_eq!((character.as_str(), name.as_str()), ("Bailu", "CoolMod"));
    }

    #[test]
    fn install_entry_infers_character() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Mystery.zip");
        write_zip(&zip, &[("mod.ini", b"; kafka kafka kafka")], None);

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, None).unwrap();

        let InstallResultDto::Installed { character, .. } = dto else {
            panic!("expected installed");
        };
        assert_eq!(character, "Kafka");
    }

    #[test]
    fn install_entry_needs_password() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Locked.zip");
        write_zip(&zip, &[("s.txt", b"s")], Some("pw1"));

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, None).unwrap();
        assert_eq!(dto, InstallResultDto::NeedsPassword);

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, Some("pw1")).unwrap();
        assert!(matches!(dto, InstallResultDto::Installed { .. }));
    }

    #[test]
    fn install_entry_humanizes_duplicate_error() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Dup.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();

        let err = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap_err();
        assert!(err.contains("已存在同名 Mod"));
    }

    #[test]
    fn remove_entry_deletes_files_and_row() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Gone.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();
        let InstallResultDto::Installed { mod_id, .. } = dto else {
            panic!("expected installed");
        };

        remove_entry(&lib, None, mod_id).unwrap();

        assert!(lib.list().unwrap().is_empty());
        assert!(!lib.layout.mod_dir("Bailu", "Gone").exists());
    }

    #[test]
    fn remove_entry_disables_junction_first() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Active.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();
        let InstallResultDto::Installed { mod_id, .. } = dto else {
            panic!("expected installed");
        };
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), mod_id, true).unwrap();
        let entry = lib.db.get_mod(mod_id).unwrap();
        let link = mods.path().join(Deployer::link_name(&entry));
        assert!(link.exists());

        remove_entry(&lib, Some(mods.path()), mod_id).unwrap();

        assert!(!link.exists());
        assert!(lib.list().unwrap().is_empty());
    }

    #[test]
    fn remove_entry_missing_id_errors() {
        let (_d, lib) = temp_lib();
        assert!(remove_entry(&lib, None, 99999).is_err());
    }

    #[test]
    fn install_entry_missing_file_errors() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let err = install_entry(
            &lib,
            Hsr::shared(),
            &dir.path().join("Nope.zip"),
            None,
            None,
        )
        .unwrap_err();
        assert!(err.contains("文件不存在"));
    }

    #[test]
    fn install_entry_rejects_directory_path() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let err = install_entry(&lib, Hsr::shared(), dir.path(), None, None).unwrap_err();
        assert!(err.contains("请拖入压缩包文件"));
    }

    #[test]
    fn install_entry_rejects_non_archive() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Fake.zip");
        std::fs::write(&zip, b"not a zip").unwrap();
        let err = install_entry(&lib, Hsr::shared(), &zip, None, None).unwrap_err();
        assert!(err.contains("不是支持的压缩包"));
    }

    #[test]
    fn preset_dto_roundtrip() {
        let db = liquimod_core::db::Database::open_in_memory().unwrap();
        let m = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let id = crate::commands::save_preset_named(&db, "日常", &[m]).unwrap();
        let list = crate::commands::preset_dtos(&db).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].name, "日常");
    }

    #[test]
    fn apply_preset_requires_mods_dir() {
        let dir = tempfile::tempdir().unwrap();
        let lib = liquimod_core::library::Library::init(dir.path()).unwrap();
        let pid = lib.db.save_preset("p", &[]).unwrap();
        assert!(crate::commands::apply_preset_by_id(&lib, None, pid).is_err());
    }

    #[test]
    fn thumb_data_url_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(crate::commands::thumb_data_url(dir.path(), dir.path(), 42).is_none());
    }
}
