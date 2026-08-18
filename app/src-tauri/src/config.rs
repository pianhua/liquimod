use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    pub mods_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_enable: bool,
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
}
