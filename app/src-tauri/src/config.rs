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
    500
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
    /// 平台配置路径：%APPDATA%/LiquiMod/config.json
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .expect("无法定位用户配置目录")
            .join("LiquiMod")
            .join("config.json")
    }

    /// 日志跟随大文件数据根；迁移后在下次启动时切换。
    pub fn log_dir() -> PathBuf {
        Self::load().data_root().join("Logs")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let existed = path.is_file() || Self::backup_path(&path).is_file();
        let mut config = Self::load_from(&path);
        if !existed {
            config.library_root = Self::preferred_data_root().join("Library");
        }
        config
    }

    /// 默认大文件数据根：优先使用程序所在盘，无法定位时才回退用户配置目录。
    pub fn preferred_data_root() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|exe| liquimod_core::filesystem::volume_root(&exe))
            .map(|root| root.join("LiquiModData"))
            .unwrap_or_else(|| {
                Self::config_path()
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf()
            })
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

    /// 文件缺失或损坏时回退默认（library_root = config 文件同目录 Library/）。
    pub fn load_from(path: &Path) -> Self {
        let backup = Self::backup_path(path);
        match fs::read_to_string(path)
            .or_else(|_| fs::read_to_string(&backup))
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        {
            Some(c) => c,
            None => Self {
                library_root: path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("Library"),
                previous_library_root: None,
                mods_dir: None,
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
        fs::write(
            &temp,
            serde_json::to_string_pretty(self).expect("Config 序列化"),
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
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        let c = Config {
            library_root: PathBuf::from("C:/lib"),
            previous_library_root: None,
            mods_dir: Some(PathBuf::from("C:/game/Mods")),
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
