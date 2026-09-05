mod config;
mod diagnostics;
mod explorer;
mod launch;
mod library;

use crate::config::Config;
use crate::state::{lock_mutex, AppState};
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

/// 由当前配置构建 helper 钉扎:用户 SID 取自当前进程令牌;
/// 游戏路径未配置时 helper 仅提供 F10(LAUNCH 会被钉扎拒绝)。
fn current_launch_pin(config: &Config) -> Result<liquimod_core::refresh::LaunchPin, String> {
    let user_sid = liquimod_core::refresh::current_user_sid()
        .map_err(|e| format!("获取当前用户 SID 失败：{e}"))?;
    let game_exe = config.game_exe.clone();
    Ok(liquimod_core::refresh::LaunchPin {
        user_sid,
        data_root: game_exe.as_ref().map(|_| config.data_root()),
        game_exe,
    })
}

/// 用户主动请求时通知 helper 发 F10。
/// 阻塞（UAC 弹窗 + 最多 5s 管道轮询）：必须在 spawn_blocking 工作线程内调用。
fn send_refresh_game(
    refresh: &Mutex<Option<RefreshClient>>,
    process_name: &str,
    pin: liquimod_core::refresh::LaunchPin,
) -> Result<(), String> {
    let Some(helper) = refresh_helper_path() else {
        return Err("未找到刷新 helper，无法发送 F10".to_string());
    };
    let mut client = {
        let mut guard = lock_mutex(refresh, "refresh")?;
        if guard.as_ref().is_some_and(|client| client.pin() != &pin) {
            *guard = None;
        }
        guard.take()
    };
    if client.is_none() {
        client = Some(
            RefreshClient::connect_or_launch(&helper, pin.clone())
                .map_err(|e| format!("刷新 helper 启动失败：{e}"))?,
        );
    }
    let mut client = client.ok_or_else(|| "刷新 helper 连接未建立".to_string())?;
    if let Err(error) = client.poke_for_process(process_name) {
        return Err(format!("F10 发送失败：{error}"));
    }
    let mut guard = lock_mutex(refresh, "refresh")?;
    if guard.is_none() {
        *guard = Some(client);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigDto {
    pub storage_root: String,
    pub library_root: String,
    pub previous_library_root: Option<String>,
    pub mods_dir: Option<String>,
    pub mod_sources: Vec<String>,
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
        mod_sources: c
            .mod_sources
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
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
    let requested = normalized_for_compare(&path);
    let managed = normalized_for_compare(&c.managed_mods_dir());
    if requested != managed {
        return Err(format!(
            "3DMigoto Mods 运行目录由 LiquiMod 托管，不能切换到外部目录；请使用“外部 Mod 源目录”添加真实 Mod 文件夹。当前托管目录：{}",
            c.managed_mods_dir().display()
        ));
    }
    c.mods_dir = Some(c.managed_mods_dir());
    Ok(config_dto(c))
}

fn normalize_existing_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    if !path.is_dir() {
        return Err(format!("{label}不存在：{}", path.display()));
    }
    path.canonicalize()
        .map_err(|e| format!("无法读取{label}：{e}"))
}

fn normalized_for_compare(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|cwd| cwd.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn paths_overlap(left: &Path, right: &Path) -> bool {
    left == right || left.starts_with(right) || right.starts_with(left)
}

fn validate_mod_source(config: &Config, source: &Path) -> Result<PathBuf, String> {
    let source = normalize_existing_directory(source, "外部 Mod 源目录")?;
    let library = normalized_for_compare(&config.library_root);
    if paths_overlap(&source, &library) {
        return Err("外部 Mod 源不能与 LiquiMod Library 重叠".to_string());
    }
    if let Some(mods_dir) = config.mods_dir.as_deref() {
        let mods = normalized_for_compare(mods_dir);
        if paths_overlap(&source, &mods) {
            return Err("外部 Mod 源不能与 3DMigoto Mods 部署目录重叠".to_string());
        }
    }
    let managed_migoto = normalized_for_compare(&config.managed_migoto_dir());
    if paths_overlap(&source, &managed_migoto) {
        return Err("外部 Mod 源不能与 LiquiMod 托管的 3DMigoto 工作区重叠".to_string());
    }
    Ok(source)
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

pub use config::*;
pub use diagnostics::*;
pub use explorer::*;
pub use launch::*;
pub(crate) use library::open_in_explorer;
pub use library::*;

#[cfg(test)]
mod tests {
    use super::*;
    use liquimod_core::games::hsr::Hsr;
    use std::fs;

    fn app_state_for_game_guard() -> (tempfile::TempDir, AppState) {
        let temp = tempfile::tempdir().unwrap();
        let library = Library::init(temp.path()).unwrap();
        let config = Config {
            library_root: temp.path().to_path_buf(),
            previous_library_root: None,
            mods_dir: Some(temp.path().join("Mods")),
            mod_sources: Vec::new(),
            auto_enable: false,
            warn_multiple_mods: true,
            theme: "auto".into(),
            character_category_name: "角色".into(),
            game_exe: None,
            loader_exe: None,
            favorite_characters: Vec::new(),
            work_mode: "play".into(),
            injection_delay_ms: 0,
            github_token: String::new(),
            github_mirror: String::new(),
            migoto_version: None,
        };
        let state = AppState {
            config: std::sync::Arc::new(std::sync::Mutex::new(config)),
            config_path: temp.path().join("config.json"),
            library: std::sync::Arc::new(std::sync::Mutex::new(library)),
            watcher: std::sync::Mutex::new(None),
            refresh: std::sync::Arc::new(std::sync::Mutex::new(None)),
            game_watchdog: std::sync::Mutex::new(None),
            game_running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            launch_in_progress: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            deferred_runtime_cleanup: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        };
        (temp, state)
    }

    #[test]
    fn ensure_game_stopped_allows_operations_when_game_is_not_running() {
        let (_temp, state) = app_state_for_game_guard();

        assert!(ensure_game_stopped(&state, "修复 Mod 部署").is_ok());
    }

    #[test]
    fn ensure_game_stopped_rejects_operations_with_the_operation_name_when_game_is_running() {
        let (_temp, state) = app_state_for_game_guard();
        state
            .game_running
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let error = ensure_game_stopped(&state, "安装 Mod").unwrap_err();

        assert!(error.contains("游戏正在运行中"));
        assert!(error.contains("安装 Mod"));
    }

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
            mod_sources: Vec::new(),
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
            mod_sources: Vec::new(),
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
    fn resolve_existing_explorer_path_returns_absolute_path() {
        let temp = tempfile::tempdir().unwrap();
        let sub = temp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();

        // 相对路径应解析为绝对路径。
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();
        let resolved = resolve_existing_explorer_path(Path::new("sub")).unwrap();
        std::env::set_current_dir(original_dir).unwrap();

        assert!(resolved.is_absolute());
        assert!(resolved.exists());
        assert_eq!(resolved, sub.canonicalize().unwrap());
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
