use crate::config::Config;
use liquimod_core::library::Library;
use liquimod_core::refresh::{GameWatchdog, RefreshClient};
use liquimod_core::watch::LibraryWatcher;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex};

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub config_path: PathBuf,
    pub library: Arc<Mutex<Library>>,
    /// 目录监控；mods_dir 变更时整体替换（旧实例 Drop 即停）。
    pub watcher: Mutex<Option<LibraryWatcher>>,
    /// 提权 helper 管道（Arc 便于跨线程移动）；持有即 helper 存活。
    pub refresh: Arc<Mutex<Option<RefreshClient>>>,
    /// 游戏进程状态看门狗；配置的游戏路径变化时整体替换。
    pub game_watchdog: Mutex<Option<GameWatchdog>>,
    /// 最近一次看门狗状态，供 IPC 快速读取。
    pub game_running: Arc<AtomicBool>,
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
            watcher: Mutex::new(None),
            refresh: Arc::new(Mutex::new(None)),
            game_watchdog: Mutex::new(None),
            game_running: Arc::new(AtomicBool::new(false)),
        }
    }
}
