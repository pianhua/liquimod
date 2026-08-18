use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_theme() -> String {
    "auto".into()
}
fn default_character_category_name() -> String {
    "角色".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    pub mods_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_enable: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_character_category_name")]
    pub character_category_name: String,
    #[serde(default)]
    pub game_exe: Option<PathBuf>,
    #[serde(default)]
    pub loader_exe: Option<PathBuf>,
}

impl Config {
    /// 平台配置路径：%APPDATA%/LiquiMod/config.json
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .expect("无法定位用户配置目录")
            .join("LiquiMod")
            .join("config.json")
    }

    /// 日志目录：%APPDATA%/LiquiMod/logs
    pub fn log_dir() -> PathBuf {
        Self::config_path()
            .parent()
            .expect("配置路径应有父目录")
            .join("logs")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    /// 文件缺失或损坏时回退默认（library_root = config 文件同目录 Library/）。
    pub fn load_from(path: &Path) -> Self {
        match fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        {
            Some(c) => c,
            None => Self {
                library_root: path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("Library"),
                mods_dir: None,
                auto_enable: false,
                theme: default_theme(),
                character_category_name: default_character_category_name(),
                game_exe: None,
                loader_exe: None,
            },
        }
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(self).expect("Config 序列化"),
        )
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
            mods_dir: Some(PathBuf::from("C:/game/Mods")),
            auto_enable: true,
            theme: "auto".into(),
            character_category_name: "角色".into(),
            game_exe: None,
            loader_exe: None,
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
