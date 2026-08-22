use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_theme() -> String {
    "auto".into()
}
fn default_character_category_name() -> String {
    "角色".into()
}
fn default_work_mode() -> String {
    "play".into()
}
fn default_injection_delay_ms() -> u64 {
    0
}
fn default_warn_multiple_mods() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    #[serde(default)]
    pub previous_library_root: Option<PathBuf>,
    pub mods_dir: Option<PathBuf>,
    /// 外部 Mod 源根目录；不会复制或接管源文件。
    #[serde(default)]
    pub mod_sources: Vec<PathBuf>,
    #[serde(default)]
    pub auto_enable: bool,
    #[serde(default = "default_warn_multiple_mods")]
    pub warn_multiple_mods: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_character_category_name")]
    pub character_category_name: String,
    #[serde(default)]
    pub game_exe: Option<PathBuf>,
    #[serde(default)]
    pub loader_exe: Option<PathBuf>,
    #[serde(default)]
    pub favorite_characters: Vec<String>,
    #[serde(default = "default_work_mode")]
    pub work_mode: String,
    #[serde(default = "default_injection_delay_ms")]
    pub injection_delay_ms: u64,
    #[serde(default)]
    pub github_token: String,
    #[serde(default)]
    pub github_mirror: String,
    #[serde(default)]
    pub migoto_version: Option<String>,
}

impl Config {
    /// 便携应用根目录。发布包中即为 XXMI Launcher.exe 所在目录。
    pub fn app_root() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// 便携配置路径：<应用根目录>/config/config.json。
    pub fn config_path() -> PathBuf {
        Self::app_root().join("config").join("config.json")
    }

    /// 旧版本配置路径，仅在首次启动迁移时读取。
    pub fn legacy_config_path() -> Option<PathBuf> {
        let path = dirs::config_dir()?.join("LiquiMod").join("config.json");
        (path != Self::config_path()).then_some(path)
    }

    /// 日志始终写入便携应用根目录，避免首次迁移前继续产生 C 盘日志。
    pub fn log_dir() -> PathBuf {
        Self::preferred_data_root().join("Logs")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.is_file() || Self::backup_path(&path).is_file() {
            let loaded = Self::load_from(&path);
            let root = Self::preferred_data_root();
            let managed_library_portable = loaded.library_root.starts_with(&root);
            let managed_migoto_portable = loaded
                .mods_dir
                .as_deref()
                .is_some_and(|path| path.starts_with(root.join("3DMigoto")));
            if managed_library_portable && managed_migoto_portable {
                return loaded;
            }
            if let Ok(migrated) = Self::migrate_legacy_to_portable(&loaded) {
                return migrated;
            }
            // 即使历史托管目录已离线，也不让新运行时继续写回旧绝对路径。
            let mut fallback = Self::default_for_root(&root);
            fallback.previous_library_root = Some(loaded.library_root);
            fallback.game_exe = loaded.game_exe;
            fallback.mod_sources = loaded.mod_sources;
            fallback.theme = loaded.theme;
            fallback.character_category_name = loaded.character_category_name;
            fallback.work_mode = loaded.work_mode;
            fallback.injection_delay_ms = loaded.injection_delay_ms;
            fallback.github_token = loaded.github_token;
            fallback.github_mirror = loaded.github_mirror;
            fallback.migoto_version = loaded.migoto_version;
            let _ = fallback.save_to(&path);
            return fallback;
        }
        if let Some(legacy_path) = Self::legacy_config_path() {
            if legacy_path.is_file() || Self::backup_path(&legacy_path).is_file() {
                let legacy = Self::load_from(&legacy_path);
                if let Ok(migrated) = Self::migrate_legacy_to_portable(&legacy) {
                    return migrated;
                }
                // 迁移失败时仍然坚持使用便携根目录；旧库保留为待处理历史路径，
                // 防止应用继续把运行数据写回 C 盘。
                let mut fallback = Self::default_for_root(&Self::preferred_data_root());
                fallback.previous_library_root = Some(legacy.library_root);
                fallback.game_exe = legacy.game_exe;
                fallback.favorite_characters = legacy.favorite_characters;
                fallback.mod_sources = legacy.mod_sources;
                fallback.theme = legacy.theme;
                fallback.character_category_name = legacy.character_category_name;
                fallback.work_mode = legacy.work_mode;
                fallback.injection_delay_ms = legacy.injection_delay_ms;
                fallback.github_token = legacy.github_token;
                fallback.github_mirror = legacy.github_mirror;
                fallback.migoto_version = legacy.migoto_version;
                let _ = fallback.save_to(&path);
                return fallback;
            }
        }
        Self::default_for_root(&Self::preferred_data_root())
    }

    /// 便携数据根：与程序同目录，便于整目录复制、压缩和恢复。
    pub fn preferred_data_root() -> PathBuf {
        Self::app_root()
    }

    pub fn data_root(&self) -> PathBuf {
        self.library_root
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    }

    pub fn managed_migoto_dir(&self) -> PathBuf {
        self.data_root().join("3DMigoto")
    }

    pub fn managed_mods_dir(&self) -> PathBuf {
        self.managed_migoto_dir().join("Mods")
    }

    fn default_for_root(root: &Path) -> Self {
        Self {
            library_root: root.join("Library"),
            previous_library_root: None,
            mods_dir: Some(root.join("3DMigoto").join("Mods")),
            mod_sources: Vec::new(),
            auto_enable: false,
            warn_multiple_mods: default_warn_multiple_mods(),
            theme: default_theme(),
            character_category_name: default_character_category_name(),
            game_exe: None,
            loader_exe: None,
            favorite_characters: Vec::new(),
            work_mode: default_work_mode(),
            injection_delay_ms: default_injection_delay_ms(),
            github_token: String::new(),
            github_mirror: String::new(),
            migoto_version: None,
        }
    }

    fn config_root(path: &Path) -> PathBuf {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        if parent.file_name().and_then(|name| name.to_str()) == Some("config") {
            parent.parent().unwrap_or(parent).to_path_buf()
        } else {
            parent.to_path_buf()
        }
    }

    fn resolve_path(root: &Path, path: PathBuf) -> PathBuf {
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    }

    fn portableize_path(root: &Path, path: &Path) -> PathBuf {
        path.strip_prefix(root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| path.to_path_buf())
    }

    fn persisted_for(&self, root: &Path) -> Self {
        let mut persisted = self.clone();
        persisted.library_root = Self::portableize_path(root, &self.library_root);
        persisted.mods_dir = self
            .mods_dir
            .as_deref()
            .map(|path| Self::portableize_path(root, path));
        persisted.loader_exe = self
            .loader_exe
            .as_deref()
            .map(|path| Self::portableize_path(root, path));
        persisted.mod_sources = self
            .mod_sources
            .iter()
            .map(|path| Self::portableize_path(root, path))
            .collect();
        persisted
    }

    /// 把旧版 `%APPDATA%/LiquiMod` 配置及其自有数据复制到便携应用目录。
    /// 外部游戏路径和外部 Mod 源只保留引用，绝不复制或删除。
    fn migrate_legacy_to_portable(legacy: &Self) -> std::io::Result<Self> {
        let root = Self::preferred_data_root();
        fs::create_dir_all(&root)?;
        let portable_library = root.join("Library");
        let legacy_library = legacy.library_root.clone();

        if legacy_library.is_dir()
            && legacy_library.canonicalize().ok() != portable_library.canonicalize().ok()
            && !portable_library.exists()
        {
            let source = liquimod_core::library::Library::open(&legacy_library)
                .or_else(|_| liquimod_core::library::Library::init(&legacy_library))
                .map_err(|error| std::io::Error::other(error.to_string()))?;
            liquimod_core::storage::migrate_library(&source, &root)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }

        let legacy_migoto = legacy.managed_migoto_dir();
        let portable_migoto = root.join("3DMigoto");
        if legacy_migoto.is_dir()
            && legacy_migoto.canonicalize().ok() != portable_migoto.canonicalize().ok()
            && !portable_migoto.exists()
        {
            liquimod_core::storage::copy_managed_directory(&legacy_migoto, &portable_migoto)
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }

        let mut migrated = legacy.clone();
        migrated.library_root = portable_library;
        migrated.mods_dir = Some(root.join("3DMigoto").join("Mods"));
        migrated.loader_exe = None;
        migrated.previous_library_root = Some(legacy_library);
        migrated.save_to(&Self::config_path())?;
        Ok(migrated)
    }

    /// 文件缺失或损坏时回退默认（library_root = config 文件同目录 Library/）。
    pub fn load_from(path: &Path) -> Self {
        let backup = Self::backup_path(path);
        match fs::read_to_string(path)
            .or_else(|_| fs::read_to_string(&backup))
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .map(|mut config| {
                let root = Self::config_root(path);
                config.library_root = Self::resolve_path(&root, config.library_root);
                config.mods_dir = config
                    .mods_dir
                    .take()
                    .map(|value| Self::resolve_path(&root, value));
                config.loader_exe = config
                    .loader_exe
                    .take()
                    .map(|value| Self::resolve_path(&root, value));
                config.mod_sources = config
                    .mod_sources
                    .into_iter()
                    .map(|value| Self::resolve_path(&root, value))
                    .collect();
                config
            }) {
            Some(c) => c,
            None => Self {
                library_root: path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("Library"),
                previous_library_root: None,
                mods_dir: None,
                mod_sources: Vec::new(),
                auto_enable: false,
                warn_multiple_mods: default_warn_multiple_mods(),
                theme: default_theme(),
                character_category_name: default_character_category_name(),
                game_exe: None,
                loader_exe: None,
                favorite_characters: Vec::new(),
                work_mode: default_work_mode(),
                injection_delay_ms: default_injection_delay_ms(),
                github_token: String::new(),
                github_mirror: String::new(),
                migoto_version: None,
            },
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("json.tmp");
        let backup = Self::backup_path(path);
        let root = Self::config_root(path);
        let persisted = self.persisted_for(&root);
        fs::write(
            &temp,
            serde_json::to_string_pretty(&persisted).expect("Config 序列化"),
        )?;
        if !path.exists() {
            return fs::rename(temp, path);
        }
        if backup.exists() {
            fs::remove_file(&backup)?;
        }
        fs::rename(path, &backup)?;
        if let Err(error) = fs::rename(&temp, path) {
            let _ = fs::rename(&backup, path);
            let _ = fs::remove_file(&temp);
            return Err(error);
        }
        fs::remove_file(backup)
    }

    fn backup_path(path: &Path) -> PathBuf {
        path.with_extension("json.bak")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_next_to_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("LiquiMod").join("config.json");
        let c = Config::load_from(&path);
        assert_eq!(c.library_root, dir.path().join("LiquiMod").join("Library"));
        assert_eq!(c.mods_dir, None);
        assert!(c.mod_sources.is_empty());
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        let c = Config {
            library_root: PathBuf::from("C:/lib"),
            previous_library_root: None,
            mods_dir: Some(PathBuf::from("C:/game/Mods")),
            mod_sources: Vec::new(),
            auto_enable: true,
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
            migoto_version: Some("v2.4.2".into()),
        };
        c.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), c);
    }

    #[test]
    fn corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        fs::write(&path, "{ not json").unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.library_root, dir.path().join("Library"));
    }

    #[test]
    fn auto_enable_defaults_false_and_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let c: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(!c.auto_enable);
        let mut c = c;
        c.auto_enable = true;
        c.save_to(&path).unwrap();
        let c2: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(c2.auto_enable);
    }

    #[test]
    fn multiple_mod_warning_defaults_true_and_roundtrips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let mut c = Config::load_from(&path);
        assert!(c.warn_multiple_mods);
        c.warn_multiple_mods = false;
        c.save_to(&path).unwrap();
        assert!(!Config::load_from(&path).warn_multiple_mods);
    }

    #[test]
    fn exe_paths_default_none_and_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.game_exe, None);
        assert_eq!(c.loader_exe, None);
        let mut c = c;
        c.game_exe = Some(PathBuf::from("C:/game/StarRail.exe"));
        c.save_to(&path).unwrap();
        assert_eq!(
            Config::load_from(&path).game_exe,
            Some(PathBuf::from("C:/game/StarRail.exe"))
        );
    }

    #[test]
    fn theme_and_category_name_default_and_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.theme, "auto");
        assert_eq!(c.character_category_name, "角色");
        let mut c = c;
        c.theme = "light".into();
        c.character_category_name = "机体".into();
        c.save_to(&path).unwrap();
        let c2 = Config::load_from(&path);
        assert_eq!(c2.theme, "light");
        assert_eq!(c2.character_category_name, "机体");
    }
}
