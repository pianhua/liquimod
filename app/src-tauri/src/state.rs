use crate::config::Config;
use liquimod_core::library::Library;
use liquimod_core::refresh::{GameWatchdog, RefreshClient};
use liquimod_core::watch::LibraryWatcher;
use std::collections::HashSet;
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
    /// 游戏运行期拆除 Junction 后待清理的运行副本。
    pub deferred_runtime_cleanup: Arc<Mutex<HashSet<i64>>>,
}

impl AppState {
    /// 启动：读配置 → 打开（或初始化）库
    pub fn bootstrap() -> Self {
        let config_path = Config::config_path();
        let mut config = Config::load();
        match liquimod_core::migoto_sync::seed_bundled_packages(&config.data_root()) {
            Ok(count) if count > 0 => tracing::info!("seeded {count} bundled core package(s)"),
            Ok(_) => {}
            Err(error) => tracing::warn!("bundled core package seeding failed: {error}"),
        }
        if let Err(error) =
            liquimod_core::migoto_sync::init_migoto_workspace(&config.managed_migoto_dir())
        {
            tracing::warn!("managed 3Dmigoto workspace initialization failed: {error}");
        }
        liquimod_core::games::hsr::Hsr::set_asset_root(config.data_root().join("GameAssets"));
        let library = Library::open(&config.library_root)
            .or_else(|_| Library::init(&config.library_root))
            .unwrap_or_else(|preferred_error| {
                let fallback = config_path
                    .parent()
                    .expect("配置路径应有父目录")
                    .join("Library");
                config.library_root = fallback.clone();
                Library::open(&fallback)
                    .or_else(|_| Library::init(&fallback))
                    .unwrap_or_else(|_| panic!("无法打开 Mod 库：{preferred_error}"))
            });
        let _ = config.save_to(&config_path);
        Self {
            config_path,
            config: Arc::new(Mutex::new(config)),
            library: Arc::new(Mutex::new(library)),
            watcher: Mutex::new(None),
            refresh: Arc::new(Mutex::new(None)),
            game_watchdog: Mutex::new(None),
            game_running: Arc::new(AtomicBool::new(false)),
            deferred_runtime_cleanup: Arc::new(Mutex::new(HashSet::new())),
        }
    }
}
