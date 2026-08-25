use crate::config::Config;
use liquimod_core::library::Library;
use liquimod_core::refresh::{GameWatchdog, RefreshClient};
use liquimod_core::watch::LibraryWatcher;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, Arc, Mutex, MutexGuard};

/// Lock an application-state mutex without turning poisoning into a process panic.
///
/// A poisoned state is not silently recovered: callers receive a contextual error and
/// can stop the current command or lifecycle action before using potentially partial data.
pub(crate) fn lock_mutex<'a, T>(
    mutex: &'a Mutex<T>,
    name: &str,
) -> Result<MutexGuard<'a, T>, String> {
    mutex
        .lock()
        .map_err(|_| format!("应用状态锁 {name} 已损坏，请重启应用"))
}

/// Application state lock ordering:
///
/// * atomics do not participate in lock ordering;
/// * when configuration and Library are both needed, use Config → Library;
/// * prefer copying configuration values and releasing Config before locking Library;
/// * never acquire Config while holding Library (or watcher/watchdog/refresh/deferred-cleanup);
/// * do not hold a state guard across `.await`, process/UAC, network, or long filesystem work.
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
    /// 防止启动注入期间重入模组启动或阻塞 F10 管道请求。
    pub launch_in_progress: Arc<AtomicBool>,
    /// 游戏运行期拆除 Junction 后待清理的运行副本。
    pub deferred_runtime_cleanup: Arc<Mutex<HashSet<i64>>>,
}

impl AppState {
    /// 启动：读配置 → 打开（或初始化）库。
    pub fn bootstrap() -> Result<Self, String> {
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

        let preferred_root = config.library_root.clone();
        let preferred_error =
            match Library::open(&preferred_root).or_else(|_| Library::init(&preferred_root)) {
                Ok(library) => {
                    if let Err(error) = config.save_to(&config_path) {
                        tracing::warn!("saving configuration during bootstrap failed: {error}");
                    }
                    return Ok(Self {
                        config_path,
                        config: Arc::new(Mutex::new(config)),
                        library: Arc::new(Mutex::new(library)),
                        watcher: Mutex::new(None),
                        refresh: Arc::new(Mutex::new(None)),
                        game_watchdog: Mutex::new(None),
                        game_running: Arc::new(AtomicBool::new(false)),
                        launch_in_progress: Arc::new(AtomicBool::new(false)),
                        deferred_runtime_cleanup: Arc::new(Mutex::new(HashSet::new())),
                    });
                }
                Err(error) => error,
            };

        let fallback = config_path
            .parent()
            .map(|parent| parent.join("Library"))
            .unwrap_or_else(|| PathBuf::from("Library"));
        config.library_root = fallback.clone();
        let library = Library::open(&fallback).or_else(|_| Library::init(&fallback));
        let library = match library {
            Ok(library) => library,
            Err(fallback_error) => {
                return Err(format!(
                    "无法打开 Mod 库：首选路径 {} 失败：{preferred_error}；fallback 路径 {} 失败：{fallback_error}",
                    preferred_root.display(),
                    fallback.display()
                ));
            }
        };
        if let Err(error) = config.save_to(&config_path) {
            tracing::warn!("saving fallback configuration during bootstrap failed: {error}");
        }
        Ok(Self {
            config_path,
            config: Arc::new(Mutex::new(config)),
            library: Arc::new(Mutex::new(library)),
            watcher: Mutex::new(None),
            refresh: Arc::new(Mutex::new(None)),
            game_watchdog: Mutex::new(None),
            game_running: Arc::new(AtomicBool::new(false)),
            launch_in_progress: Arc::new(AtomicBool::new(false)),
            deferred_runtime_cleanup: Arc::new(Mutex::new(HashSet::new())),
        })
    }
}
