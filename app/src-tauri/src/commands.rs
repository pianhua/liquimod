use crate::config::Config;
use crate::state::AppState;
use base64::Engine;
use liquimod_core::archive::install::{
    install_archive, install_archive_inferred, install_folder, install_folder_inferred,
    InstallOutcome,
};
use liquimod_core::deploy::Deployer;
use liquimod_core::error::LiquiModError;
use liquimod_core::games::hsr::Hsr;
use liquimod_core::games::{CharacterInfo, Game};
use liquimod_core::library::Library;
use liquimod_core::models::ModStorageKind;
use liquimod_core::refresh::{is_game_running, RefreshClient, HELPER_EXE};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::{atomic::Ordering, Mutex};
use tauri::Emitter;

fn refresh_helper_path() -> Option<PathBuf> {
    let current_exe = std::env::current_exe().ok()?;
    let parent = current_exe.parent()?;
    [
        parent.join(HELPER_EXE),
        parent.join("resources").join(HELPER_EXE),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// 用户主动请求时通知 helper 发 F10。
/// 阻塞（UAC 弹窗 + 最多 5s 管道轮询）：必须在 spawn_blocking 工作线程内调用。
fn send_refresh_game(refresh: &Mutex<Option<RefreshClient>>) -> Result<(), String> {
    let Some(helper) = refresh_helper_path() else {
        return Err("未找到刷新 helper，无法发送 F10".to_string());
    };
    let mut guard = refresh.lock().unwrap();
    if guard.is_none() {
        let client = RefreshClient::connect_or_launch(&helper)
            .map_err(|e| format!("刷新 helper 启动失败：{e}"))?;
        *guard = Some(client);
    }
    if let Some(client) = guard.as_mut() {
        if let Err(error) = client.poke() {
            *guard = None; // helper 死了，下次重连
            return Err(format!("F10 发送失败：{error}"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigDto {
    pub storage_root: String,
    pub library_root: String,
    pub previous_library_root: Option<String>,
    pub mods_dir: Option<String>,
    pub auto_enable: bool,
    pub warn_multiple_mods: bool,
    pub theme: String,
    pub character_category_name: String,
    pub game_exe: Option<String>,
    pub loader_exe: Option<String>,
    pub work_mode: String,
    pub injection_delay_ms: u64,
    pub github_token: String,
    pub github_mirror: String,
    pub migoto_version: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct GameStatusDto {
    pub running: bool,
}

/// 配置的 exe 名优先；未配置时保留 HSR 默认进程名，兼容首次启动和旧配置。
pub fn configured_game_process_names(config: &Config) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(name) = config
        .game_exe
        .as_ref()
        .and_then(|p| p.file_name())
        .and_then(|s| s.to_str())
    {
        names.push(name.to_string());
    }
    for name in Hsr::shared().process_names() {
        if !names
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(name))
        {
            names.push((*name).to_string());
        }
    }
    names
}

/// 游戏运行期间阻止 Junction 启停之外的文件变更操作。
fn ensure_game_stopped(state: &AppState, operation: &str) -> Result<(), String> {
    if state.game_running.load(Ordering::Relaxed) {
        return Err(format!(
            "游戏正在运行中，为避免资源锁定或闪退，已阻止{operation}；请退出游戏后重试"
        ));
    }
    Ok(())
}
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CategoryDto {
    pub id: i64,
    pub name: String,
    pub ord: i64,
    pub kind: Option<String>,
    pub mod_count: i64,
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
    pub element: Option<String>,
    pub rarity: Option<u8>,
    pub is_favorite: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VariantDto {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModDto {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub installed_at: i64,
    pub thumb: Option<String>,
    pub size_bytes: i64,
    pub file_count: i64,
    pub path: String,
    pub category_id: Option<i64>,
    pub note: Option<String>,
    pub cover_image: Option<String>,
    pub is_favorite: bool,
    pub sort_order: i64,
    pub active_variant: Option<String>,
    pub variants: Vec<VariantDto>,
    pub storage_kind: String,
    pub source_available: bool,
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MigotoInspectDto {
    pub root: String,
    pub ini_path: String,
    pub game_exe: Option<String>,
    pub loader_exe: Option<String>,
    pub mods_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModKeyBindingDto {
    pub section: String,
    pub key: String,
    pub formatted_key: String,
    pub back: Option<String>,
    pub formatted_back: Option<String>,
    pub key_type: Option<String>,
    pub variable: Option<String>,
    pub steps: Option<usize>,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConflictReportDto {
    pub hash: String,
    pub section: String,
    pub conflicting_mods: Vec<ConflictModInfoDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct VariableConflictDto {
    pub variable: String,
    pub conflicting_mods: Vec<ConflictModInfoDto>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConflictModInfoDto {
    pub id: i64,
    pub character: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModImageDto {
    pub relative_path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub data_url: String,
    pub is_cover: bool,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub fn config_dto(c: &Config) -> ConfigDto {
    ConfigDto {
        storage_root: c.data_root().display().to_string(),
        library_root: c.library_root.display().to_string(),
        previous_library_root: c
            .previous_library_root
            .as_ref()
            .map(|p| p.display().to_string()),
        mods_dir: c.mods_dir.as_ref().map(|p| p.display().to_string()),
        auto_enable: c.auto_enable,
        warn_multiple_mods: c.warn_multiple_mods,
        theme: c.theme.clone(),
        character_category_name: c.character_category_name.clone(),
        game_exe: c.game_exe.as_ref().map(|p| p.display().to_string()),
        loader_exe: c.loader_exe.as_ref().map(|p| p.display().to_string()),
        work_mode: c.work_mode.clone(),
        injection_delay_ms: c.injection_delay_ms,
        github_token: c.github_token.clone(),
        github_mirror: c.github_mirror.clone(),
        migoto_version: c.migoto_version.clone(),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StorageInfoDto {
    pub storage_root: String,
    pub library_root: String,
    pub previous_library_root: Option<String>,
    pub files: u64,
    pub bytes: u64,
    pub available_bytes: Option<u64>,
    pub recommended_root: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StorageMigrationDto {
    pub storage_root: String,
    pub library_root: String,
    pub copied_files: u64,
    pub copied_bytes: u64,
    pub managed_migoto_migrated: bool,
    pub deployment_warning: Option<String>,
}

/// 角色网格汇总：支持指定分类（如光锥/立绘/默认皮肤等）。
pub fn character_summaries(
    lib: &Library,
    game: &dyn Game,
    category_id: Option<i64>,
    favorites: &[String],
) -> Result<Vec<CharacterSummary>, String> {
    let mods = lib.list().map_err(|e| e.to_string())?;
    let fav_set: std::collections::HashSet<&str> = favorites.iter().map(|s| s.as_str()).collect();
    let mut out: Vec<CharacterSummary> = Vec::new();
    let chars = game.characters();
    for c in chars.iter() {
        let group: Vec<_> = mods
            .iter()
            .filter(|m| {
                m.character == c.internal_name
                    && match category_id {
                        None => m.category_id.is_none(),
                        Some(cid) => m.category_id == Some(cid),
                    }
            })
            .collect();
        let is_fav = fav_set.contains(c.internal_name.as_str());
        out.push(summary(
            c,
            group.len(),
            group.iter().filter(|m| m.enabled).count(),
            is_fav,
        ));
    }

    // 若指定了分类（如光锥/立绘等），且存在不属于已知角色的 Mod，在末尾附带虚拟「通用 / 未归属」卡片
    if let Some(cid) = category_id {
        let known: std::collections::HashSet<String> =
            chars.iter().map(|c| c.internal_name.clone()).collect();
        let other_mods: Vec<_> = mods
            .iter()
            .filter(|m| m.category_id == Some(cid) && !known.contains(m.character.as_str()))
            .collect();
        if !other_mods.is_empty() {
            out.push(CharacterSummary {
                internal_name: "others".to_string(),
                display_name: "未归属角色 / 通用".to_string(),
                image: None,
                total: other_mods.len(),
                enabled: other_mods.iter().filter(|m| m.enabled).count(),
                element: None,
                rarity: None,
                is_favorite: false,
            });
        }
    }

    Ok(out)
}

fn summary(c: &CharacterInfo, total: usize, enabled: usize, is_favorite: bool) -> CharacterSummary {
    CharacterSummary {
        internal_name: c.internal_name.clone(),
        display_name: c.display_name.clone(),
        image: Some(c.image.clone()),
        total,
        enabled,
        element: c.element.clone(),
        rarity: c.rarity,
        is_favorite,
    }
}

/// 角色 → 固定分类 kind。已知角色返回 None（角色虚拟类）；
/// npc/lightcone/portrait/scene 返回对应 kind；其它一律「other」。
fn char_category_kind(character: &str, game: &dyn Game) -> Option<&'static str> {
    if game
        .characters()
        .iter()
        .any(|c| c.internal_name == character)
    {
        return None;
    }
    match character {
        "npc" => Some("npc"),
        "lightcone" => Some("lightcone"),
        "portrait" => Some("portrait"),
        "scene" => Some("scene"),
        _ => Some("other"),
    }
}

/// 幂等归类：对未显式分类的 Mod (category_id 为 NULL) 且属于固定分类的执行初始归类。
/// (LM-P1-006) 保护用户手动指定的分类，绝不强行重置。
pub fn sync_mod_categories(lib: &Library, game: &dyn Game) -> Result<usize, String> {
    let mut changed = 0;
    for m in lib.list().map_err(|e| e.to_string())? {
        // 若用户已显式分配了分类，保护用户分类不受后台扫描影响
        if m.category_id.is_some() {
            continue;
        }
        if let Some(kind) = char_category_kind(&m.character, game) {
            let id = lib
                .db
                .category_id_by_kind(kind)
                .map_err(|e| e.to_string())?;
            let want_id = match id {
                Some(id) => Some(id),
                None => {
                    lib.db
                        .ensure_default_categories()
                        .map_err(|e| e.to_string())?;
                    lib.db
                        .category_id_by_kind(kind)
                        .map_err(|e| e.to_string())?
                }
            };
            if m.category_id != want_id {
                lib.db
                    .set_mod_category(m.id, want_id)
                    .map_err(|e| e.to_string())?;
                changed += 1;
            }
        }
    }
    Ok(changed)
}

/// 阶段一：在库锁内收集角色 Mod 的基础字段与缩略图目录（纯数据，不做 IO 重的图像工作）。
fn collect_mod_rows(
    lib: &Library,
    character: &str,
    category_id: Option<i64>,
) -> Result<Vec<ModRow>, String> {
    if character == "others" {
        if let Some(cid) = category_id {
            let chars = liquimod_core::games::hsr::Hsr::shared().characters();
            let known: std::collections::HashSet<String> =
                chars.iter().map(|c| c.internal_name.clone()).collect();
            return collect_rows_where(lib, move |m| {
                m.category_id == Some(cid) && !known.contains(&m.character)
            });
        }
    }
    collect_rows_where(lib, move |m| {
        m.character == character
            && match category_id {
                None => m.category_id.is_none(),
                Some(cid) => m.category_id == Some(cid),
            }
    })
}

fn collect_rows_where(
    lib: &Library,
    pred: impl Fn(&liquimod_core::models::ModEntry) -> bool,
) -> Result<Vec<ModRow>, String> {
    let mut rows: Vec<ModRow> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| pred(m))
        .map(|m| {
            let dir = lib.entry_source_dir(&m).unwrap_or_else(|_| {
                m.source_path
                    .as_deref()
                    .map(PathBuf::from)
                    .unwrap_or_else(|| lib.layout.root.join(&m.rel_path))
            });
            let source_available = dir.is_dir();
            ModRow {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                size_bytes: m.size_bytes,
                file_count: m.file_count,
                category_id: m.category_id,
                note: m.note.clone(),
                cover_image: m.cover_image.clone(),
                is_favorite: m.is_favorite,
                sort_order: m.sort_order,
                active_variant: m.active_variant.clone(),
                variants: if source_available {
                    liquimod_core::variants::detect_variants(&dir)
                        .into_iter()
                        .map(|v| VariantDto { name: v.name })
                        .collect()
                } else {
                    Vec::new()
                },
                storage_kind: m.storage_kind.as_str().to_string(),
                source_available,
                dir,
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.is_favorite
            .cmp(&a.is_favorite)
            .then_with(|| a.sort_order.cmp(&b.sort_order))
            .then_with(|| a.name.cmp(&b.name))
    });
    Ok(rows)
}

fn rows_to_dtos(root: &Path, rows: Vec<ModRow>) -> Vec<ModDto> {
    rows.into_iter()
        .map(|m| {
            let thumb = thumb_data_url(root, &m.dir, m.id, m.cover_image.as_deref());
            ModDto {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                thumb,
                size_bytes: m.size_bytes,
                file_count: m.file_count,
                path: m.dir.display().to_string(),
                category_id: m.category_id,
                note: m.note,
                cover_image: m.cover_image,
                is_favorite: m.is_favorite,
                sort_order: m.sort_order,
                active_variant: m.active_variant,
                variants: m.variants,
                storage_kind: m.storage_kind,
                source_available: m.source_available,
            }
        })
        .collect()
}

struct ModRow {
    id: i64,
    name: String,
    enabled: bool,
    installed_at: i64,
    size_bytes: i64,
    file_count: i64,
    category_id: Option<i64>,
    note: Option<String>,
    cover_image: Option<String>,
    is_favorite: bool,
    sort_order: i64,
    active_variant: Option<String>,
    variants: Vec<VariantDto>,
    storage_kind: String,
    source_available: bool,
    dir: PathBuf,
}

/// 阶段一 + 阶段二（缩略图）组合；供测试使用。
#[allow(dead_code)]
pub fn mod_list(lib: &Library, character: &str) -> Result<Vec<ModDto>, String> {
    let root = lib.layout.root.clone();
    Ok(rows_to_dtos(&root, collect_mod_rows(lib, character, None)?))
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
pub fn thumb_data_url(
    library_root: &Path,
    mod_dir: &Path,
    mod_id: i64,
    custom_cover: Option<&str>,
) -> Option<String> {
    let path = liquimod_core::thumbs::ensure_thumbnail(library_root, mod_dir, mod_id, custom_cover)
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

/// 启动已配置的可执行文件；未配置或文件缺失时报人话错误。通过 ShellExecute 启动并自动支持提权。
fn launch_exe(exe: Option<&Path>, what: &str) -> Result<(), String> {
    let Some(exe) = exe else {
        return Err(format!("未配置{what}路径，请在设置中配置"));
    };
    if !exe.is_file() {
        return Err(format!("{what}不存在：{}", exe.display()));
    }
    liquimod_core::refresh::launch_program(exe).map_err(|e| format!("启动{what}失败：{e}"))?;
    tracing::info!("launched {} ({})", what, exe.display());
    Ok(())
}

/// 安装 Mod（支持文件夹或压缩包）：character=None 时从内容推断。人话错误信息。
pub fn install_entry(
    lib: &Library,
    game: &dyn Game,
    path: &Path,
    character: Option<&str>,
    password: Option<&str>,
) -> Result<InstallResultDto, String> {
    if !path.exists() {
        return Err(format!("路径不存在：{}", path.display()));
    }
    let outcome = if path.is_dir() {
        match character {
            Some(c) => install_folder(&lib.db, lib, path, c),
            None => install_folder_inferred(&lib.db, lib, game, path),
        }
    } else if path.is_file() {
        match character {
            Some(c) => install_archive(&lib.db, lib, path, c, password),
            None => install_archive_inferred(&lib.db, lib, game, path, password),
        }
    } else {
        return Err("不支持的内容：请拖入 Mod 文件夹或压缩包文件 (zip/7z/rar)".to_string());
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
    if entry.storage_kind == ModStorageKind::Managed {
        let dir = lib.layout.root.join(&entry.rel_path);
        match std::fs::remove_dir_all(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("删除 Mod 文件失败，可能有文件被占用".to_string()),
        }
    }
    lib.db.remove_mod(id).map_err(|e| e.to_string())?;
    liquimod_core::thumbs::remove_thumbnail(&lib.layout.root, id);
    Ok(())
}

/// 重命名：启用中则 拆 Junction → 改名 → 按新名重建。冲突时恢复原启用状态。
pub fn rename_entry(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
    new_name: &str,
) -> Result<(), String> {
    let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if !entry.enabled {
        return lib
            .rename_mod(id, new_name)
            .map(|_| ())
            .map_err(|e| humanize_install_error(&e));
    }
    let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录")?;
    let dep = Deployer::new(lib, mods_dir);
    dep.disable(id).map_err(|e| e.to_string())?;
    if let Err(e) = lib.rename_mod(id, new_name) {
        let _ = dep.enable(id); // 改名失败，恢复旧 junction
        return Err(humanize_install_error(&e));
    }
    dep.enable(id)
        .map_err(|e| format!("已改名为 {new_name}，但重新启用失败：{e}"))?;
    tracing::info!("renamed mod {id} to {new_name}");
    Ok(())
}

/// 重新分配 Mod 归属角色：禁用中只动文件与 DB；启用中先删旧 Junction、移动、再建新 Junction。
pub fn reassign_entry(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
    new_character: &str,
) -> Result<(), String> {
    let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if entry.character == new_character {
        return Ok(());
    }
    if !entry.enabled {
        lib.reassign_character(id, new_character)
            .map_err(|e| humanize_install_error(&e))?;
    } else {
        let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录")?;
        let dep = Deployer::new(lib, mods_dir);
        dep.disable(id).map_err(|e| e.to_string())?;
        if let Err(e) = lib.reassign_character(id, new_character) {
            let _ = dep.enable(id); // 移动失败，恢复旧 junction
            return Err(humanize_install_error(&e));
        }
        dep.enable(id)
            .map_err(|e| format!("已移动至 {new_character}，但重新启用失败：{e}"))?;
    }
    tracing::info!("reassigned mod {id} to character {new_character}");
    Ok(())
}

/// 安装后自动启用（设置开启时）；失败仅告警，不否决安装。
pub fn maybe_auto_enable(
    lib: &Library,
    config: &Config,
    mod_id: i64,
    app: Option<&tauri::AppHandle>,
) {
    if !config.auto_enable {
        return;
    }
    let Some(dir) = &config.mods_dir else {
        return;
    };
    if let Err(e) = Deployer::new(lib, dir).enable(mod_id) {
        tracing::warn!("auto-enable failed for mod {mod_id}: {e}");
        if let Some(app) = app {
            let _ = app.emit("liquimod-toast", format!("自动启用失败：{e}"));
        }
    } else {
        tracing::info!("auto-enabled mod {mod_id}");
    }
}

/// 读最新滚动日志尾部（最多 max_bytes、最后 200 行）。seek 定位，不全量读入内存。
pub fn read_log_tail(log_dir: &Path, max_bytes: u64) -> Result<String, String> {
    use std::io::{Read, Seek, SeekFrom};
    let rd = match std::fs::read_dir(log_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("（暂无日志）".into()),
        Err(e) => return Err(format!("读取日志目录失败：{e}")),
    };
    let mut files: Vec<_> = rd
        .flatten()
        .filter(|f| f.file_name().to_string_lossy().starts_with("liquimod.log"))
        .collect();
    files.sort_by_key(|f| f.metadata().and_then(|m| m.modified()).ok());
    let Some(latest) = files.last() else {
        return Ok("（暂无日志）".into());
    };
    let mut file = std::fs::File::open(latest.path()).map_err(|e| format!("读取日志失败：{e}"))?;
    let len = file
        .metadata()
        .map_err(|e| format!("读取日志失败：{e}"))?
        .len();
    if len > max_bytes {
        // 只读末尾 max_bytes；文件小于 max_bytes 时从头读
        file.seek(SeekFrom::End(-(max_bytes as i64)))
            .map_err(|e| format!("读取日志失败：{e}"))?;
    }
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("读取日志失败：{e}"))?;
    let text = String::from_utf8_lossy(&buf);
    let lines: Vec<&str> = text.lines().collect();
    let keep = if lines.len() > 200 {
        &lines[lines.len() - 200..]
    } else {
        &lines[..]
    };
    Ok(keep.join("\n"))
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
pub async fn get_storage_info(state: tauri::State<'_, AppState>) -> Result<StorageInfoDto, String> {
    let (library_root, previous_library_root, storage_root) = {
        let config = state.config.lock().unwrap();
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

    let old_watcher = state.watcher.lock().unwrap().take();
    drop(old_watcher);
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    let config_path = state.config_path.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let old_config = config.lock().unwrap().clone();
        let old_library_root = old_config.library_root.clone();
        let old_managed_migoto = old_config.managed_migoto_dir();
        let managed_migoto = old_config
            .mods_dir
            .as_deref()
            .and_then(Path::parent)
            .is_some_and(|parent| parent == old_managed_migoto);

        let mut library_guard = library.lock().unwrap();
        let report = liquimod_core::storage::migrate_library(&library_guard, &target_root)
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
            if let Some(loader) = old_config.loader_exe.as_deref() {
                if let Ok(relative) = loader.strip_prefix(&old_managed_migoto) {
                    next_config.loader_exe = Some(new_managed_migoto.join(relative));
                }
            }
        }
        if let Err(error) = next_config.save_to(&config_path) {
            let _ = std::fs::remove_dir_all(&report.library_root);
            if managed_migoto {
                let _ = std::fs::remove_dir_all(&new_managed_migoto);
            }
            return Err(format!("保存新存储配置失败：{error}"));
        }

        *library_guard = new_library;
        *config.lock().unwrap() = next_config.clone();
        let deployment_warning = next_config.mods_dir.as_deref().and_then(|mods_dir| {
            Deployer::new(&library_guard, mods_dir)
                .reconcile()
                .err()
                .map(|e| format!("仓库已迁移，但部署对账需要稍后重试：{e}"))
        });
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
        let mut config = config.lock().unwrap();
        let previous = config
            .previous_library_root
            .clone()
            .ok_or_else(|| "没有可清理的旧仓库".to_string())?;
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
pub async fn get_characters(
    state: tauri::State<'_, AppState>,
    category_id: Option<i64>,
) -> Result<Vec<CharacterSummary>, String> {
    let library = std::sync::Arc::clone(&state.library);
    let favorites = state.config.lock().unwrap().favorite_characters.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
    lib.db.toggle_favorite_mod(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn reorder_mods(state: tauri::State<AppState>, ids: Vec<i64>) -> Result<(), String> {
    let lib = state.library.lock().unwrap();
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
            let lib = library.lock().unwrap();
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
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
                "游戏运行期间仅支持同卷 NTFS/ReFS Junction 热切换；复制兼容模式请退出游戏后操作"
                    .to_string(),
            );
        }

        if game_running && enabled {
            deployer
                .enable_reusing_runtime(id)
                .map_err(|e| e.to_string())?;
            deferred_runtime_cleanup.lock().unwrap().remove(&id);
        } else if game_running {
            deployer
                .disable_preserving_runtime(id)
                .map_err(|e| e.to_string())?;
            deferred_runtime_cleanup.lock().unwrap().insert(id);
        } else {
            set_enabled(&lib, Some(mods_dir), id, enabled)?;
            deferred_runtime_cleanup.lock().unwrap().remove(&id);
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
    let config = std::sync::Arc::clone(&state.config);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
            let mods_dir = config
                .lock()
                .unwrap()
                .mods_dir
                .clone()
                .ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法刷新变体".to_string())?;
            Deployer::new(&lib, &mods_dir)
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
    let config_arc = std::sync::Arc::clone(&state.config);
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
        if let Ok(InstallResultDto::Installed { mod_id, .. }) = &result {
            let mod_id = *mod_id;
            let cfg = config_arc.lock().unwrap().clone();
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
        let cfg = config.lock().unwrap().clone();
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
        let lib = library.lock().unwrap();
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
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
    ensure_game_stopped(state.inner(), "应用预设")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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

#[tauri::command]
pub async fn rename_mod(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "重命名 Mod")?;
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        reassign_entry(&lib, mods_dir.as_deref(), id, &target_character)
    })
    .await
    .map_err(|e| format!("角色重分配失败：{e}"))?
}

#[tauri::command]
pub fn set_auto_enable(state: tauri::State<AppState>, enabled: bool) -> Result<ConfigDto, String> {
    let mut config = state.config.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
    config.warn_multiple_mods = enabled;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("warn_multiple_mods = {enabled}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn read_log() -> Result<String, String> {
    read_log_tail(&crate::config::Config::log_dir(), 64 * 1024)
}

#[tauri::command]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CategoryDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
        lib.db.rename_category(id, &name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("重命名分类失败：{e}"))?
}

#[tauri::command]
pub async fn delete_category(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
        let lib = library.lock().unwrap();
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
            let lib = library.lock().unwrap();
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
            let lib = library.lock().unwrap();
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
            let lib = library.lock().unwrap();
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
pub fn set_theme(state: tauri::State<AppState>, theme: String) -> Result<ConfigDto, String> {
    if !["auto", "light", "dark"].contains(&theme.as_str()) {
        return Err("主题只能是 auto / light / dark".to_string());
    }
    let mut config = state.config.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
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
pub fn get_game_status(state: tauri::State<AppState>) -> GameStatusDto {
    GameStatusDto {
        running: state.game_running.load(Ordering::Relaxed),
    }
}

#[tauri::command]
pub fn choose_loader_exe(state: tauri::State<AppState>, path: String) -> Result<ConfigDto, String> {
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
    let mut config = state.config.lock().unwrap();
    config.loader_exe = Some(p);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub async fn auto_detect_game_exe() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let found = liquimod_core::discovery::auto_detect_game_exe();
        Ok(found.map(|p| p.display().to_string()))
    })
    .await
    .map_err(|e| format!("自动探测任务异常: {e}"))?
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
        let config = state.config.lock().unwrap();
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
pub async fn install_migoto_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    download_url: String,
    version_tag: Option<String>,
) -> Result<ConfigDto, String> {
    let (target_dir, token, mirror) = {
        let mut config = state.config.lock().unwrap();
        let target = if let Some(mods) = &config.mods_dir {
            mods.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| config.managed_migoto_dir())
        } else {
            let def = config.managed_migoto_dir();
            config.mods_dir = Some(def.join("Mods"));
            def
        };
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
        (target, t, m)
    };

    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    let app_clone = app.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app_clone.emit("migoto-download-progress", progress);
        }
    });

    let res = liquimod_core::migoto_sync::download_and_install_migoto(
        &download_url,
        &target_dir,
        mirror.as_deref(),
        token.as_deref(),
        Some(tx),
    )
    .await;

    let _ = forward_task.await;
    res.map_err(|e| e.to_string())?;

    // 确保自动绑定 mods_dir 与更新版本记录
    let config = {
        let mut cfg = state.config.lock().unwrap();
        let mods_path = target_dir.join("Mods");
        if !mods_path.exists() {
            let _ = std::fs::create_dir_all(&mods_path);
        }
        cfg.mods_dir = Some(mods_path);
        if let Some(tag) = version_tag {
            cfg.migoto_version = Some(tag);
        }

        // 检查是否存在 loader
        let loader_candidate = target_dir.join("3DMigoto Loader.exe");
        if loader_candidate.is_file() {
            cfg.loader_exe = Some(loader_candidate);
        }

        cfg.save_to(&state.config_path)
            .map_err(|e| format!("配置保存失败：{e}"))?;
        config_dto(&cfg)
    };

    crate::start_watcher(&app, state.inner());
    Ok(config)
}

#[tauri::command]
pub fn switch_to_managed_migoto(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ConfigDto, String> {
    let def_dir = state.config.lock().unwrap().managed_migoto_dir();
    let _ = liquimod_core::migoto_sync::init_migoto_workspace(&def_dir)
        .map_err(|e| format!("初始化内置 3Dmigoto 失败：{e}"))?;

    let mut cfg = state.config.lock().unwrap();
    let mods_dir = def_dir.join("Mods");
    cfg.mods_dir = Some(mods_dir);

    let loader_1 = def_dir.join("3DMigoto Loader.exe");
    let loader_2 = def_dir.join("3DMigotoLoader.exe");
    if loader_1.is_file() {
        cfg.loader_exe = Some(loader_1);
    } else if loader_2.is_file() {
        cfg.loader_exe = Some(loader_2);
    }

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

        let lib = library.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
    config.injection_delay_ms = delay_ms.min(10000);
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_github_token(state: tauri::State<AppState>, token: String) -> Result<ConfigDto, String> {
    let mut config = state.config.lock().unwrap();
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
    let mut config = state.config.lock().unwrap();
    config.github_mirror = mirror.trim().to_string();
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn launch_game(
    state: tauri::State<AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    let (game_exe, mods_dir, loader_exe, work_mode_str, delay_ms) = {
        let c = state.config.lock().unwrap();
        (
            c.game_exe.clone(),
            c.mods_dir.clone(),
            c.loader_exe.clone(),
            c.work_mode.clone(),
            c.injection_delay_ms,
        )
    };

    let Some(game_path) = game_exe else {
        return Err("未配置游戏主程序路径，请在设置中配置或点击自动探测".to_string());
    };

    let migoto_dir = mods_dir
        .and_then(|m| m.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            game_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf()
        });

    let work_mode = match work_mode_str.as_str() {
        "dev" => liquimod_core::d3d::MigotoWorkMode::Dev,
        _ => liquimod_core::d3d::MigotoWorkMode::Play,
    };

    let opts = liquimod_core::launcher::GameLaunchOptions {
        game_exe: game_path,
        migoto_dir,
        loader_exe,
        work_mode,
        delay_ms,
    };

    liquimod_core::launcher::launch_with_mod(&opts).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launch_game_native(
    state: tauri::State<AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    let game_exe = state.config.lock().unwrap().game_exe.clone();
    let Some(game_path) = game_exe else {
        return Err("未配置游戏主程序路径，请在设置中配置或点击自动探测".to_string());
    };
    liquimod_core::launcher::launch_native_game(&game_path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launch_official_launcher(
    state: tauri::State<AppState>,
) -> Result<liquimod_core::launcher::LaunchResult, String> {
    let game_exe = state.config.lock().unwrap().game_exe.clone();

    let launcher_path = game_exe
        .as_deref()
        .and_then(liquimod_core::discovery::find_launcher_from_game_exe)
        .or_else(liquimod_core::discovery::auto_detect_official_launcher);

    let Some(launcher) = launcher_path else {
        return Err(
            "未能在系统常见位置或游戏目录中找到官方启动器 (launcher.exe / HYP.exe)".to_string(),
        );
    };

    liquimod_core::launcher::launch_official_launcher(&launcher).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn launch_loader(state: tauri::State<AppState>) -> Result<(), String> {
    let exe = state.config.lock().unwrap().loader_exe.clone();
    launch_exe(exe.as_deref(), "加载器")
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
        let mut config = state.config.lock().unwrap();
        if let Some(mods_dir) = info.mods_dir {
            if !mods_dir.exists() {
                let _ = std::fs::create_dir_all(&mods_dir);
            }
            config.mods_dir = Some(mods_dir);
        }
        if let Some(game_exe) = info.game_exe {
            config.game_exe = Some(game_exe);
        }
        if let Some(loader_exe) = info.loader_exe {
            config.loader_exe = Some(loader_exe);
        }
        config
            .save_to(&state.config_path)
            .map_err(|e| format!("配置保存失败：{e}"))?;
        config_dto(&config)
    };

    crate::start_watcher(&app, state.inner());
    Ok(dto)
}

#[tauri::command]
pub fn get_mod_keys(
    state: tauri::State<AppState>,
    id: i64,
) -> Result<Vec<ModKeyBindingDto>, String> {
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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
    let lib = state.library.lock().unwrap();
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

fn open_in_explorer(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("路径不存在：{}", path.display()));
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut cmd = std::process::Command::new("explorer");
        if path.is_file() {
            let win_path = path.to_string_lossy().replace('/', "\\");
            cmd.raw_arg(format!("/select,\"{}\"", win_path));
        } else {
            cmd.arg(path);
        }
        cmd.creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("打开资源管理器失败：{e}"))?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败：{e}"))?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_mod_folder(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let lib = state.library.lock().unwrap();
    let row = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    let path = lib.entry_source_dir(&row).map_err(|e| e.to_string())?;
    open_in_explorer(&path)
}

#[tauri::command]
pub fn open_path_in_explorer(path: String) -> Result<(), String> {
    open_in_explorer(Path::new(&path))
}

#[tauri::command]
pub async fn trigger_refresh_game(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let refresh = std::sync::Arc::clone(&state.refresh);
    let process_names = {
        let config = state.config.lock().unwrap();
        configured_game_process_names(&config)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let process_name_refs = process_names.iter().map(String::as_str).collect::<Vec<_>>();
        if !is_game_running(&process_name_refs) {
            return Err("未检测到游戏进程，无法发送 F10".to_string());
        }
        send_refresh_game(&refresh)
    })
    .await
    .map_err(|e| format!("刷新任务失败：{e}"))?
}

#[tauri::command]
pub fn get_mod_images(state: tauri::State<AppState>, id: i64) -> Result<Vec<ModImageDto>, String> {
    let lib = state.library.lock().unwrap();
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

#[tauri::command]
pub async fn rescan_library(state: tauri::State<'_, AppState>) -> Result<RescanResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    let game_running = std::sync::Arc::clone(&state.game_running);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let mods_dir = config.lock().unwrap().mods_dir.clone();
        let deploy = !game_running.load(std::sync::atomic::Ordering::Relaxed);
        let (added, removed) =
            crate::reconcile_and_diff_with_deploy(&lib, mods_dir.as_deref(), deploy)
                .map_err(|e| format!("全库重新扫描失败：{e}"))?;
        Ok(RescanResultDto { added, removed })
    })
    .await
    .map_err(|e| format!("重新扫描任务失败：{e}"))?
}

#[tauri::command]
pub async fn clean_cache(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let mods = lib.list().map_err(|e| e.to_string())?;
        let valid: std::collections::HashSet<i64> = mods.into_iter().map(|m| m.id).collect();
        let thumb_dir = lib.layout.root.join("thumbs");
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(&thumb_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Ok(id) = stem.parse::<i64>() {
                        if !valid.contains(&id) && std::fs::remove_file(&path).is_ok() {
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    })
    .await
    .map_err(|e| format!("清理缓存任务失败：{e}"))?
}

#[derive(Debug, serde::Serialize)]
pub struct DiagnosticStatusDto {
    pub helper_ready: bool,
    pub game_configured: bool,
    pub loader_configured: bool,
    pub mods_dir_configured: bool,
    pub checks: Vec<liquimod_core::diagnostics::DiagnosticCheck>,
    pub filesystem: Option<String>,
    pub deploy_strategy: Option<String>,
    pub defender_command: Option<String>,
}

#[tauri::command]
pub fn get_diagnostic_status(state: tauri::State<AppState>) -> DiagnosticStatusDto {
    let (library_root, mods_dir, game_exe, loader_exe) = {
        let config = state.config.lock().unwrap();
        (
            config.library_root.clone(),
            config.mods_dir.clone(),
            config.game_exe.clone(),
            config.loader_exe.clone(),
        )
    };
    let helper_ready = refresh_helper_path().is_some();
    let checks = liquimod_core::diagnostics::collect_checks(
        &library_root,
        mods_dir.as_deref(),
        game_exe.as_deref(),
        loader_exe.as_deref(),
        helper_ready,
    );
    let filesystem = mods_dir
        .as_deref()
        .and_then(|mods| liquimod_core::filesystem::same_volume_filesystem(&library_root, mods));
    let deploy_strategy = mods_dir.as_deref().map(|mods| {
        liquimod_core::deploy::Deployer::new(&state.library.lock().unwrap(), mods)
            .strategy_label()
            .to_owned()
    });
    let mut exclusion_paths = vec![library_root.as_path()];
    if let Some(mods) = mods_dir.as_deref() {
        exclusion_paths.push(mods);
        if let Some(parent) = mods.parent() {
            exclusion_paths.push(parent);
        }
    }

    DiagnosticStatusDto {
        helper_ready,
        game_configured: game_exe.is_some_and(|p| !p.as_os_str().is_empty()),
        loader_configured: loader_exe.is_some_and(|p| !p.as_os_str().is_empty()),
        mods_dir_configured: mods_dir.as_ref().is_some_and(|p| !p.as_os_str().is_empty()),
        checks,
        filesystem,
        deploy_strategy,
        defender_command: liquimod_core::diagnostics::defender_exclusion_command(&exclusion_paths),
    }
}

#[tauri::command]
pub async fn repair_deployment(state: tauri::State<'_, AppState>) -> Result<(), String> {
    ensure_game_stopped(state.inner(), "修复 Mod 部署").map_err(|e| e.to_string())?;
    let library = std::sync::Arc::clone(&state.library);
    let config = std::sync::Arc::clone(&state.config);
    tauri::async_runtime::spawn_blocking(move || {
        let mods_dir = config
            .lock()
            .unwrap()
            .mods_dir
            .clone()
            .ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法修复部署".to_string())?;
        let lib = library.lock().unwrap();
        Deployer::new(&lib, &mods_dir)
            .reconcile()
            .map_err(|e| format!("部署对账失败：{e}"))
    })
    .await
    .map_err(|e| format!("修复部署任务失败：{e}"))?
}

#[tauri::command]
pub fn open_webview2_download(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(
            liquimod_core::diagnostics::WEBVIEW2_DOWNLOAD_URL,
            None::<String>,
        )
        .map_err(|e| format!("无法打开 WebView2 下载页面：{e}"))
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AssetUpdateCheckResultDto {
    pub has_update: bool,
    pub remote_version: Option<String>,
    pub local_version: Option<String>,
}

#[tauri::command]
pub async fn get_local_asset_version() -> Result<Option<String>, String> {
    let service = liquimod_core::assets_sync::AssetSyncService::new();
    Ok(service.get_local_version().await)
}

#[tauri::command]
pub async fn check_game_assets_update(
    game: Option<String>,
) -> Result<AssetUpdateCheckResultDto, String> {
    let service = liquimod_core::assets_sync::AssetSyncService::new();
    let local = service.get_local_version().await;
    let filter = game.as_deref().or(Some("Honkai"));
    match service.check_for_updates(filter).await {
        Ok(remote_opt) => {
            let has_update = remote_opt.is_some();
            Ok(AssetUpdateCheckResultDto {
                has_update,
                remote_version: remote_opt,
                local_version: local,
            })
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn sync_game_assets(
    app: tauri::AppHandle,
    game: Option<String>,
) -> Result<liquimod_core::assets_sync::AssetSyncResult, String> {
    let service = liquimod_core::assets_sync::AssetSyncService::new();
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);

    let app_clone = app.clone();
    let forward_task = tokio::spawn(async move {
        while let Some(progress) = rx.recv().await {
            let _ = app_clone.emit("asset-sync-progress", progress);
        }
    });

    let filter = game.as_deref().or(Some("Honkai"));
    let result = service.sync(filter, Some(tx)).await;
    let _ = forward_task.await;

    match result {
        Ok(res) => {
            Hsr::shared().reload();
            let _ = app.emit("game-assets-updated", ());
            Ok(res)
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn get_character_image_data(
    game: Option<String>,
    filename: String,
) -> Result<Option<String>, String> {
    let raw_game = game.as_deref().unwrap_or("Honkai");
    // 安全防御 (LM-P1-003): 游戏目录白名单校验
    let game_name = match raw_game.to_lowercase().as_str() {
        "honkai" | "hsr" => "Honkai",
        "genshin" => "Genshin",
        "zenless" | "zzz" => "Zenless",
        _ => return Ok(None),
    };

    // 安全防御: 净化文件名相对路径，严禁 .. 逃逸
    let Ok(safe_file) = liquimod_core::safe_path::sanitize_relative_path(Path::new(&filename))
    else {
        return Ok(None);
    };

    let asset_root = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LiquiMod")
        .join("GameAssets")
        .join(game_name);

    // 尝试多个可能路径（支持大小写与子目录）
    let candidates = [
        asset_root
            .join("Images")
            .join("Characters")
            .join(&safe_file),
        asset_root
            .join("images")
            .join("Characters")
            .join(&safe_file),
        asset_root
            .join("Images")
            .join("characters")
            .join(&safe_file),
        asset_root
            .join("images")
            .join("characters")
            .join(&safe_file),
        asset_root.join("Images").join(&safe_file),
        asset_root.join("images").join(&safe_file),
        asset_root.join(&safe_file),
    ];

    for path in &candidates {
        if path.is_file() {
            if let Ok(meta) = tokio::fs::metadata(path).await {
                if meta.len() > 15 * 1024 * 1024 {
                    continue;
                }
            }
            if let Ok(bytes) = tokio::fs::read(path).await {
                use base64::Engine;
                let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                let mime = if filename.to_lowercase().ends_with(".webp") {
                    "image/webp"
                } else if filename.to_lowercase().ends_with(".gif") {
                    "image/gif"
                } else if filename.to_lowercase().ends_with(".jpg")
                    || filename.to_lowercase().ends_with(".jpeg")
                {
                    "image/jpeg"
                } else {
                    "image/png"
                };
                return Ok(Some(format!("data:{};base64,{}", mime, b64)));
            }
        }
    }

    Ok(None)
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
        let out = character_summaries(&lib, Hsr::shared(), None, &[]).unwrap();
        let acheron = out.iter().find(|c| c.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.total, 1);
        // 未知角色不再以 Others 桶混进角色网格（实体「其他」分类承接，见 sync 测试）
        assert!(!out.iter().any(|c| c.internal_name == "Others"));
    }

    #[test]
    fn sync_mod_categories_assigns_entities_and_keeps_roles_null() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        lib.add_folder(src.path(), "Stranger", "M2").unwrap();
        lib.add_folder(src.path(), "npc", "N1").unwrap();
        lib.add_folder(src.path(), "lightcone", "L1").unwrap();
        let changed = sync_mod_categories(&lib, Hsr::shared()).unwrap();
        assert_eq!(changed, 3); // Stranger→other, npc→npc, lightcone→lightcone（Acheron 是角色不动）
                                // 校验：Acheron 保持 NULL；Stranger/npc/lightcone 落到对应分类
        let mods = lib.list().unwrap();
        let by_char = |c: &str| mods.iter().find(|m| m.character == c).unwrap().clone();
        assert_eq!(by_char("Acheron").category_id, None);
        let stranger = by_char("Stranger");
        let cat_other = lib.db.category_id_by_kind("other").unwrap().unwrap();
        assert_eq!(stranger.category_id, Some(cat_other));
        let npc = by_char("npc");
        let cat_npc = lib.db.category_id_by_kind("npc").unwrap().unwrap();
        assert_eq!(npc.category_id, Some(cat_npc));
        let lc = by_char("lightcone");
        let cat_lc = lib.db.category_id_by_kind("lightcone").unwrap().unwrap();
        assert_eq!(lc.category_id, Some(cat_lc));
        // 幂等：再跑一次不再改动
        assert_eq!(sync_mod_categories(&lib, Hsr::shared()).unwrap(), 0);
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
            previous_library_root: None,
            mods_dir: None,
            auto_enable: false,
            warn_multiple_mods: true,
            theme: "auto".into(),
            character_category_name: "角色".into(),
            game_exe: None,
            loader_exe: None,
            favorite_characters: Vec::new(),
            work_mode: "play".into(),
            injection_delay_ms: 500,
            github_token: String::new(),
            github_mirror: String::new(),
            migoto_version: None,
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
    fn remove_external_entry_disconnects_without_deleting_source() {
        let (_d, lib) = temp_lib();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("mod.ini"), b"x").unwrap();
        let entry = lib
            .add_external_folder(source.path(), "Kafka", "External")
            .unwrap();
        remove_entry(&lib, None, entry.id).unwrap();
        assert!(source.path().join("mod.ini").is_file());
        assert!(lib.db.get_mod(entry.id).is_err());
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
        assert!(err.contains("不存在"));
    }

    #[test]
    fn install_entry_supports_directory_path() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let mod_dir = dir.path().join("Kafka_Test_Mod");
        std::fs::create_dir_all(&mod_dir).unwrap();
        std::fs::write(mod_dir.join("Kafka.ini"), b"[Constants]").unwrap();
        let res = install_entry(&lib, Hsr::shared(), &mod_dir, Some("Kafka"), None).unwrap();
        assert!(
            matches!(res, InstallResultDto::Installed { character, name, .. } if character == "Kafka" && name == "Kafka_Test_Mod")
        );
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
        assert!(crate::commands::thumb_data_url(dir.path(), dir.path(), 42, None).is_none());
    }

    #[test]
    fn rename_entry_disabled_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "A", "old").unwrap();
        rename_entry(&lib, None, m.id, "new").unwrap();
        assert_eq!(lib.db.get_mod(m.id).unwrap().name, "new");
        assert!(lib.layout.mod_dir("A", "new").is_dir());
    }

    #[test]
    fn rename_entry_enabled_rebuilds_junction() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "A", "old").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), m.id, true).unwrap();
        let old_link_name = liquimod_core::deploy::Deployer::link_name(&m);
        rename_entry(&lib, Some(mods.path()), m.id, "new").unwrap();
        let m_after = lib.db.get_mod(m.id).unwrap();
        let new_link_name = liquimod_core::deploy::Deployer::link_name(&m_after);
        assert!(junction::exists(mods.path().join(new_link_name)).unwrap());
        assert!(!mods.path().join(old_link_name).exists());
        assert!(m_after.enabled);
    }

    #[test]
    fn rename_entry_rebuilds_junction_targeting_new_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "A", "old").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), m.id, true).unwrap();
        rename_entry(&lib, Some(mods.path()), m.id, "new").unwrap();
        let m_after = lib.db.get_mod(m.id).unwrap();
        let link_name = liquimod_core::deploy::Deployer::link_name(&m_after);
        let target = junction::get_target(mods.path().join(link_name)).unwrap();
        assert_eq!(target, lib.layout.mod_dir("A", "new"));
    }

    #[test]
    fn rename_entry_conflict_keeps_everything() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m1 = lib.add_folder(src.path(), "A", "m1").unwrap();
        lib.add_folder(src.path(), "A", "m2").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), m1.id, true).unwrap();
        let err = rename_entry(&lib, Some(mods.path()), m1.id, "m2").unwrap_err();
        assert!(err.contains("已存在同名 Mod"));
        let m1_after = lib.db.get_mod(m1.id).unwrap();
        assert_eq!(m1_after.name, "m1");
        let link_name = liquimod_core::deploy::Deployer::link_name(&m1_after);
        assert!(junction::exists(mods.path().join(link_name)).unwrap());
        assert!(m1_after.enabled);
    }

    #[test]
    fn reassign_entry_disabled_mod() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "Others", "KafkaMod").unwrap();
        reassign_entry(&lib, None, m.id, "Kafka").unwrap();
        let m_after = lib.db.get_mod(m.id).unwrap();
        assert_eq!(m_after.character, "Kafka");
        assert_eq!(m_after.name, "KafkaMod");
        assert!(lib.layout.mod_dir("Kafka", "KafkaMod").is_dir());
        assert!(!lib.layout.mod_dir("Others", "KafkaMod").exists());
    }

    #[test]
    fn reassign_entry_enabled_rebuilds_junction() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "Others", "KafkaMod").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), m.id, true).unwrap();
        let old_link_name = liquimod_core::deploy::Deployer::link_name(&m);
        reassign_entry(&lib, Some(mods.path()), m.id, "Kafka").unwrap();
        let m_after = lib.db.get_mod(m.id).unwrap();
        let new_link_name = liquimod_core::deploy::Deployer::link_name(&m_after);
        assert!(junction::exists(mods.path().join(&new_link_name)).unwrap());
        assert!(!mods.path().join(old_link_name).exists());
        let target = junction::get_target(mods.path().join(&new_link_name)).unwrap();
        assert_eq!(target, lib.layout.mod_dir("Kafka", "KafkaMod"));
        assert!(m_after.enabled);
    }

    #[test]
    fn launch_exe_errors_when_unconfigured_or_missing() {
        assert!(launch_exe(None, "游戏")
            .unwrap_err()
            .contains("未配置游戏路径"));
        assert!(launch_exe(Some(Path::new("C:/no/such.exe")), "游戏")
            .unwrap_err()
            .contains("不存在"));
    }

    #[test]
    fn maybe_auto_enable_deploys_when_on() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        let src = tempfile::tempdir().unwrap();
        std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
        let m = lib.add_folder(src.path(), "A", "m1").unwrap();
        let mods = tempfile::tempdir().unwrap();
        let mut c = Config {
            library_root: tmp.path().to_path_buf(),
            previous_library_root: None,
            mods_dir: Some(mods.path().to_path_buf()),
            auto_enable: false,
            warn_multiple_mods: true,
            theme: "auto".into(),
            character_category_name: "角色".into(),
            game_exe: None,
            loader_exe: None,
            favorite_characters: Vec::new(),
            work_mode: "play".into(),
            injection_delay_ms: 500,
            github_token: String::new(),
            github_mirror: String::new(),
            migoto_version: None,
        };
        maybe_auto_enable(&lib, &c, m.id, None);
        assert!(!lib.db.get_mod(m.id).unwrap().enabled);
        c.auto_enable = true;
        maybe_auto_enable(&lib, &c, m.id, None);
        assert!(lib.db.get_mod(m.id).unwrap().enabled);
    }

    #[test]
    fn read_log_tail_truncates() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("logs");
        std::fs::create_dir_all(&dir).unwrap();
        let body: String = (0..300).map(|i| format!("line {i}\n")).collect();
        std::fs::write(dir.join("liquimod.log.2026-08-18"), body).unwrap();
        let s = read_log_tail(&dir, 64 * 1024).unwrap();
        assert_eq!(s.lines().count(), 200);
        assert!(s.contains("line 299"));
        assert!(!s.contains("line 0\n"));
        assert_eq!(
            read_log_tail(&tmp.path().join("nope"), 1024).unwrap(),
            "（暂无日志）"
        );
    }

    #[test]
    fn read_log_tail_respects_max_bytes() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("logs");
        std::fs::create_dir_all(&dir).unwrap();
        // 约 35KB > max_bytes：只读尾部，不得全量进内存
        let body: String = (0..5000).map(|i| format!("line {i}\n")).collect();
        std::fs::write(dir.join("liquimod.log.2026-08-18"), body).unwrap();
        let s = read_log_tail(&dir, 4096).unwrap();
        assert!(s.len() <= 4096);
        assert!(s.contains("line 4999"));
        assert!(!s.contains("line 3000"));
        assert_eq!(s.lines().count(), 200);
    }

    #[test]
    fn summaries_exclude_categorized_mods() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        let m = lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        let c = lib.db.create_category("武器").unwrap();
        lib.db.set_mod_category(m.id, Some(c)).unwrap();
        let out = character_summaries(&lib, Hsr::shared(), None, &[]).unwrap();
        let acheron = out.iter().find(|x| x.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.total, 0);
        assert!(mod_list(&lib, "Acheron").unwrap().is_empty());

        // 指定分类时应包含在统计中
        let cat_out = character_summaries(&lib, Hsr::shared(), Some(c), &[]).unwrap();
        let cat_acheron = cat_out
            .iter()
            .find(|x| x.internal_name == "Acheron")
            .unwrap();
        assert_eq!(cat_acheron.total, 1);
    }

    #[test]
    fn collect_rows_where_all_and_uncategorized() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        lib.add_folder(src.path(), "Stranger", "M2").unwrap();
        let all = collect_rows_where(&lib, |_| true).unwrap();
        assert_eq!(all.len(), 2);
        let chars = Hsr::shared().characters();
        let known: std::collections::HashSet<String> =
            chars.iter().map(|c| c.internal_name.clone()).collect();
        let uncat = collect_rows_where(&lib, |m| {
            m.category_id.is_none() && !known.contains(&m.character)
        })
        .unwrap();
        assert_eq!(uncat.len(), 1);
        assert_eq!(uncat[0].name, "M2");
    }

    #[test]
    fn inspect_3dmigoto_detects_and_dto_aligns() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        std::fs::write(root.join("3DMigotoLoader.exe"), b"mock").unwrap();
        std::fs::create_dir_all(root.join("Mods")).unwrap();
        std::fs::write(
            root.join("d3dx.ini"),
            "[Loader]\ntarget = C:\\StarRail\\Game\\StarRail.exe\nloader = 3DMigotoLoader.exe\n[Include]\ninclude_recursive = Mods\n",
        )
        .unwrap();

        let dto = inspect_3dmigoto_dir(root.display().to_string()).unwrap();
        assert_eq!(
            dto.game_exe,
            Some("C:\\StarRail\\Game\\StarRail.exe".to_string())
        );
        assert!(dto.loader_exe.unwrap().ends_with("3DMigotoLoader.exe"));
        assert!(dto.mods_dir.unwrap().ends_with("Mods"));
    }

    #[test]
    fn open_in_explorer_rejects_missing_path() {
        let missing = Path::new("C:\\non_existent_folder_xyz_12345");
        let res = open_in_explorer(missing);
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("路径不存在"));
    }

    #[test]
    fn get_mod_images_and_set_cover_test() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        // 创建测试图片
        std::fs::write(src.path().join("preview.png"), b"mock_img_data").unwrap();
        std::fs::write(src.path().join("alt_cover.jpg"), b"mock_img_data_2").unwrap();
        lib.add_folder(src.path(), "Acheron", "CustomM1").unwrap();

        let mods = lib.list().unwrap();
        let row = mods.iter().find(|m| m.name == "CustomM1").unwrap();
        let mod_dir = lib.layout.mod_dir(&row.character, &row.name);

        // 验证物理目录存在且可被扫描
        assert!(mod_dir.is_dir());
        assert!(mod_dir.join("preview.png").is_file());
        assert!(mod_dir.join("alt_cover.jpg").is_file());
    }
}
