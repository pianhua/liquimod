//! Library/ 与 Mods/ 目录监控：notify 监听 + 500ms 防抖，
//! 任意变动回调一次（对账由调用方触发，本模块不做任何文件改动）。

use crate::error::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(500);

/// 持有即监听；Drop 即停止（同步 join 防抖线程）。
/// 注意：Drop 时可能因队列中的最后一个信号同步再触发一次 `on_change()`，
/// 这是正确的停止语义（对账幂等，重复执行无害）。
pub struct LibraryWatcher {
    watcher: Option<RecommendedWatcher>,
    debouncer: Option<std::thread::JoinHandle<()>>,
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        drop(self.watcher.take()); // 先断 sender → 防抖线程退出
        if let Some(h) = self.debouncer.take() {
            let _ = h.join();
        }
    }
}

/// 库根内的自身写入（DB/WAL、缩略图缓存、临时区）——忽略它们的事件，
/// 防止「对账 → scan 写库 → 事件 → 再对账」的自反馈环。
fn is_self_write(library_root: &std::path::Path, p: &std::path::Path) -> bool {
    let Ok(rel) = p.strip_prefix(library_root) else {
        return false; // mods_dir 等库外路径不算
    };
    let Some(std::path::Component::Normal(first)) = rel.components().next() else {
        return false;
    };
    let first = first.to_string_lossy();
    first == "thumbs" || first == "tmp" || first.starts_with("liquimod.db")
}

/// 监听 `library_root`，以及 `mods_dir`（若已配置且存在）。
/// 变动去抖后调用 `on_change`（在后台线程，勿持锁过久）。
pub fn start(
    library_root: PathBuf,
    mods_dir: Option<PathBuf>,
    on_change: impl Fn() + Send + 'static,
) -> Result<LibraryWatcher> {
    let (tx, rx) = mpsc::channel::<()>();
    let root_for_filter = library_root.clone();
    let mut watcher = notify::recommended_watcher(
        move |res: std::result::Result<notify::Event, notify::Error>| {
            match res {
                Ok(event) => {
                    use notify::EventKind::*;
                    // Any 非通配符，用排除法表达意图
                    if matches!(event.kind, Access(_) | Other) {
                        return;
                    }
                    if !event.paths.is_empty()
                        && event
                            .paths
                            .iter()
                            .all(|p| is_self_write(&root_for_filter, p))
                    {
                        return;
                    }
                    let _ = tx.send(());
                }
                Err(_) => {
                    // 通知错误（如队列溢出）可能已丢事件 → 按变动信号处理
                    //（对账幂等，fail-safe）
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
        watcher: Some(watcher),
        debouncer: Some(debouncer),
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

    #[test]
    fn ignores_self_writes() {
        let dir = tempfile::tempdir().unwrap();
        let hits = Arc::new(AtomicUsize::new(0));
        let hits2 = Arc::clone(&hits);
        let _w = start(dir.path().to_path_buf(), None, move || {
            hits2.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(300)); // 等 watcher 就绪
                                                        // 自身写入：DB/WAL、thumbs/、tmp/ 一律不触发
        std::fs::write(dir.path().join("liquimod.db-wal"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("thumbs")).unwrap();
        std::fs::write(dir.path().join("thumbs/1.jpg"), b"x").unwrap();
        std::fs::create_dir_all(dir.path().join("tmp")).unwrap();
        std::fs::write(dir.path().join("tmp/a"), b"x").unwrap();
        std::thread::sleep(DEBOUNCE * 3);
        assert_eq!(hits.load(Ordering::SeqCst), 0, "自身写入不应触发对账");
        // 正常变动仍触发
        std::fs::create_dir_all(dir.path().join("mods/A/m1")).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while hits.load(Ordering::SeqCst) == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(hits.load(Ordering::SeqCst) >= 1, "正常变动应触发");
    }

    #[test]
    fn is_self_write_covers_db_cache_tmp() {
        let root = std::path::Path::new("C:/lib");
        assert!(is_self_write(
            root,
            std::path::Path::new("C:/lib/liquimod.db")
        ));
        assert!(is_self_write(
            root,
            std::path::Path::new("C:/lib/liquimod.db-wal")
        ));
        assert!(is_self_write(
            root,
            std::path::Path::new("C:/lib/thumbs/1.jpg")
        ));
        assert!(is_self_write(root, std::path::Path::new("C:/lib/tmp/x")));
        assert!(!is_self_write(
            root,
            std::path::Path::new("C:/lib/mods/A/m1")
        ));
        assert!(!is_self_write(root, std::path::Path::new("E:/Mods/A--m1")));
    }
}
