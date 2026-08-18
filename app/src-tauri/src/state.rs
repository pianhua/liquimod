use crate::config::Config;
use liquimod_core::library::Library;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub config_path: PathBuf,
    pub library: Arc<Mutex<Library>>,
}

impl AppState {
    /// 启动：读配置 → 打开（或初始化）库
    pub fn bootstrap() -> Self {
        let config = Config::load();
        let library = Library::open(&config.library_root)
            .or_else(|_| Library::init(&config.library_root))
            .expect("无法打开 Mod 库");
        Self {
            config_path: Config::config_path(),
            config: Arc::new(Mutex::new(config)),
            library: Arc::new(Mutex::new(library)),
        }
    }
}
