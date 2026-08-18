//! Library/ 与 Mods/ 目录监控：notify 监听 + 500ms 防抖，
//! 任意变动回调一次（对账由调用方触发，本模块不做任何文件改动）。

use crate::error::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(500);

/// 持有即监听；Drop 即停止。
pub struct LibraryWatcher {
    _watcher: RecommendedWatcher,
    _debouncer: std::thread::JoinHandle<()>,
}

/// 监听 `library_root`，以及 `mods_dir`（若已配置且存在）。
/// 变动去抖后调用 `on_change`（在后台线程，勿持锁过久）。
pub fn start(
    library_root: PathBuf,
    mods_dir: Option<PathBuf>,
    on_change: impl Fn() + Send + 'static,
) -> Result<LibraryWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let mut watcher = notify::recommended_watcher(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                use notify::EventKind::*;
                if matches!(event.kind, Create(_) | Modify(_) | Remove(_) | Any) {
                    let _ = tx.send(());
                }
            }
        },
    )
    .map_err(|e| crate::error::LiquiModError::Io(std::io::Error::other(e)))?;
    watcher
        .watch(&library_root, RecursiveMode::Recursive)
        .map_err(|e| crate::error::LiquiModError::Io(std::io::Error::other(e)))?;
    if let Some(dir) = mods_dir {
        if dir.is_dir() {
            watcher
                .watch(&dir, RecursiveMode::Recursive)
                .map_err(|e| crate::error::LiquiModError::Io(std::io::Error::other(e)))?;
        }
    }
    let debouncer = std::thread::spawn(move || {
        // 收第一个信号后清空 DEBOUNCE 窗口内的后续信号，回调一次。
        while rx.recv().is_ok() {
            while rx.recv_timeout(DEBOUNCE).is_ok() {}
            on_change();
        }
    });
    Ok(LibraryWatcher {
        _watcher: watcher,
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn fires_once_per_burst_after_debounce() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = Arc::clone(&hits);
        let _w = start(dir.path().to_path_buf(), None, move || {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300)); // 等 watcher 就绪
        for i in 0..3 {
            std::fs::write(dir.path().join(format!("f{i}.txt")), "x").unwrap();
            std::thread::sleep(Duration::from_millis(50));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while hits.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1, "watcher 未触发");
        std::thread::sleep(DEBOUNCE * 2);
        assert_eq!(hits.load(Ordering::SeqCst), 1, "突发写入应只回调一次");
    }
}
