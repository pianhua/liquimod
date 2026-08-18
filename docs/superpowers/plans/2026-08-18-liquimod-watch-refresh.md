# LiquiMod 里程碑 5：文件监控对账 + F10 刷新 Helper 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 监听 Library/ 与 Mods/ 外部变动自动对账并推送 UI 提示（绝不静默改动用户文件）；游戏运行时启用/禁用/安装 Mod 后通过 UAC 提权 helper 经命名管道发送 F10 刷新 3Dmigoto。

**Architecture:** core 新增 `watch`（notify + 500ms 防抖）与 `refresh`（sysinfo 进程检测 + 命名管道客户端）模块；独立 `liquimod-refresh-helper` bin crate（管道服务端 + SendInput F10，客户端断开即退出）；app 层 setup 启动 watcher、变更后 scan+reconcile 并 emit `library-changed`，启停/安装成功后若游戏在运行则 poke helper；前端 Toast 组件 + 事件监听刷新网格。

**Tech Stack:** notify 8、sysinfo 0.33、windows crate（ShellExecuteW runas 提权 / CreateNamedPipeW / SendInput）、Tauri 2 event emit/listen、Svelte 5。

---

## 既有事实（实现者必读，勿重新探索）

- `crates/liquimod-core/src/lib.rs` 已声明 `pub mod archive/db/deploy/error/games/library/models/paths;`。
- `Game` trait（`crates/liquimod-core/src/games/mod.rs:13`）现有 `fn id()` 与 `fn characters()`；`Hsr::shared() -> &'static Hsr`（无 Clone）。
- `Library::scan(&self) -> Result<Vec<ModEntry>>`（library.rs:38）：对账 DB ↔ Library 目录；`Library::list(&self) -> Result<Vec<ModEntry>>`（:34）。
- `Deployer::new(library: &'a Library, mods_dir: &Path)`（deploy.rs:12）；`Deployer::reconcile(&self) -> Result<()>`（deploy.rs:91）：清理指向仓库内的孤儿 Junction，忽略外来内容。
- `AppState`（app/src-tauri/src/state.rs）：`config: Arc<Mutex<Config>>`（Config{library_root, mods_dir: Option<PathBuf>}）、`config_path: PathBuf`、`library: Arc<Mutex<Library>>`。
- IPC（app/src-tauri/src/commands.rs）：`get_characters/list_mods/set_mod_enabled/install_mod/uninstall_mod` 均 async + spawn_blocking；`set_enabled(lib, mods_dir: Option<&Path>, id, enabled)` 内部建 Deployer。commands 目前未接收 `tauri::AppHandle`，本里程碑会为 `set_mod_enabled`/`install_mod`/`uninstall_mod` 增加。
- 前端 `app/src/routes/+page.svelte`：`refresh()` 重载角色网格；onMount 内已有 `getCurrentWebviewWindow().onDragDropEvent` 监听模式可参照。`app/src/routes/+layout.svelte` 挂载全局组件。
- UI tokens（app.css）：`.glass`、`.radius-pill`、`.radius-card`、`.radius-panel`；文字 `--text`/`--text-dim`。
- 前端测试 vitest + @testing-library/svelte；mock 层在 `api.ts` 的 `isTauri()`。
- helper exe 与 app exe 同在 `target/release/`（workspace 共享 target），运行时经 `std::env::current_exe().parent().join("liquimod-refresh-helper.exe")` 定位。

## 明确裁剪（YAGNI，勿超范围）

- 对账只 toast 汇总数字（+N / -M），不做逐项 diff 列表。
- F10 为全局 SendInput（不定位游戏窗口句柄）；游戏不在前台时按键落空属可接受。
- helper 无 manifest，提权完全靠 ShellExecuteW `runas` 动词；UAC 拒绝 → toast 提示，不阻断。
- 无 watcher 开关设置项；mods_dir 未配置时只监听 Library/。
- 无打包/安装器（helper 与 app 同目录由构建命令保证）。

---

### Task 1: core `watch` 模块（notify + 防抖）

**Files:**
- Create: `crates/liquimod-core/src/watch.rs`
- Modify: `crates/liquimod-core/src/lib.rs`（加 `pub mod watch;`）
- Modify: `crates/liquimod-core/Cargo.toml`（加 notify）
- Test: 同文件 `#[cfg(test)]`

- [ ] **Step 1: 加依赖**

`crates/liquimod-core/Cargo.toml` `[dependencies]` 追加：

```toml
notify = "8"
```

- [ ] **Step 2: 写失败测试**

`crates/liquimod-core/src/watch.rs`：

```rust
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
    let mut watcher = notify::recommended_watcher(move |res| {
        if let Ok(event) = res {
            use notify::EventKind::*;
            if matches!(event.kind, Create(_) | Modify(_) | Remove(_) | Any) {
                let _ = tx.send(());
            }
        }
    })
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
```

- [ ] **Step 3: 注册模块并跑测试**

lib.rs 末尾加 `pub mod watch;`。dev-dependencies 已有 tempfile。

Run: `cargo test -p liquimod-core watch`
Expected: PASS（触发 1 次）。Windows ReadDirectoryChangesW 可靠；若偶发超时属环境抖动，重跑一次确认。

- [ ] **Step 4: Commit**

```bash
git add crates/liquimod-core/src/watch.rs crates/liquimod-core/src/lib.rs crates/liquimod-core/Cargo.toml Cargo.lock
git commit -m "feat(core): watch 模块（notify 监听 + 500ms 防抖）"
```

---

### Task 2: core `refresh` 客户端 + 游戏进程检测

**Files:**
- Create: `crates/liquimod-core/src/refresh.rs`
- Modify: `crates/liquimod-core/src/lib.rs`、`crates/liquimod-core/src/games/mod.rs`（trait 加 `process_names`）、`crates/liquimod-core/src/games/hsr.rs`
- Modify: `crates/liquimod-core/Cargo.toml`（加 sysinfo、windows）
- Test: refresh.rs `#[cfg(test)]`

- [ ] **Step 1: 依赖**

```toml
sysinfo = "0.33"
windows = { version = "0.61", features = ["Win32_UI_Shell", "Win32_Foundation"] }
```

（windows crate 主版本以 crates.io 最新为准，0.58+ 均可，API 一致。）

- [ ] **Step 2: Game trait 加进程名**

games/mod.rs trait 内加：

```rust
    /// 游戏主进程可执行文件名（小写，含 .exe）。
    fn process_names(&self) -> &'static [&'static str];
```

hsr.rs `impl Game for Hsr` 加：

```rust
    fn process_names(&self) -> &'static [&'static str] {
        &["starrail.exe"]
    }
```

- [ ] **Step 3: refresh.rs 完整实现（含测试）**

```rust
//! 游戏刷新：检测游戏进程；经命名管道通知提权 helper 发 F10。
//! helper 不存在时用 ShellExecuteW "runas" 提权拉起（触发一次 UAC）。

use crate::error::{LiquiModError, Result};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

pub const PIPE_NAME: &str = r"\\.\pipe\liquimod-refresh";
pub const HELPER_EXE: &str = "liquimod-refresh-helper.exe";

/// 任一给定进程名存在即为游戏运行中（大小写不敏感）。
pub fn is_game_running(process_names: &[&str]) -> bool {
    let mut sys = sysinfo::System::new();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
    sys.processes().values().any(|p| {
        let name = p.name().to_string_lossy().to_lowercase();
        process_names.iter().any(|n| name == *n)
    })
}

/// 持有管道写端 = app 生命周期；Drop 即断开，helper 随之退出。
pub struct RefreshClient {
    pipe: File,
}

impl RefreshClient {
    /// 连接已运行的 helper；否则 runas 提权拉起并等待管道就绪（最多 5s）。
    pub fn connect_or_launch(helper_exe: &Path) -> Result<Self> {
        if let Ok(pipe) = Self::try_connect() {
            return Ok(Self { pipe });
        }
        launch_elevated(helper_exe)?;
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if let Ok(pipe) = Self::try_connect() {
                return Ok(Self { pipe });
            }
        }
        Err(LiquiModError::Io(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "helper 管道等待超时",
        )))
    }

    fn try_connect() -> std::io::Result<File> {
        OpenOptions::new().write(true).open(PIPE_NAME)
    }

    /// 通知 helper 发一次 F10。
    pub fn poke(&mut self) -> Result<()> {
        self.pipe.write_all(b"1")?;
        self.pipe.flush()?;
        Ok(())
    }
}

/// ShellExecuteW(runas) 提权启动 helper（UAC 拒绝返回 SE_ERR_ACCESSDENIED=5）。
#[cfg(windows)]
fn launch_elevated(exe: &Path) -> Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    let runas: Vec<u16> = "runas\0".encode_utf16().collect();
    let path: Vec<u16> = format!("{}\0", exe.display()).encode_utf16().collect();
    let r = unsafe {
        ShellExecuteW(
            HWND::default(),
            PCWSTR(runas.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            1, // SW_SHOWNORMAL
        )
    };
    if r.0 as usize > 32 {
        Ok(())
    } else {
        Err(LiquiModError::Io(std::io::Error::other(format!(
            "helper 启动失败（可能拒绝了 UAC），code {}",
            r.0 as usize
        ))))
    }
}

#[cfg(not(windows))]
fn launch_elevated(_exe: &Path) -> Result<()> {
    Err(LiquiModError::Io(std::io::Error::other(
        "仅 Windows 支持刷新 helper",
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_running_process_case_insensitive() {
        let own = std::env::current_exe()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_lowercase();
        // 测试 harness 进程名形如 liquimod_core-xxxx.exe，取不含 hash 的前缀不可行，
        // 直接断言：不存在的进程 false；当前进程 true。
        assert!(!is_game_running(&["definitely-not-running-zzz.exe"]));
        assert!(is_game_running(&[&own]));
    }
}
```

注意：sysinfo 0.33 `refresh_processes` 签名为 `(&mut self, ProcessesToUpdate, bool)`；若实际版本签名不同，以实现时 docs.rs 为准调整一行。

- [ ] **Step 4: 注册 + 测试**

lib.rs 加 `pub mod refresh;`。

Run: `cargo test -p liquimod-core refresh games`
Expected: 全 PASS（含 hsr 既有测试不受影响——trait 加方法后 hsr impl 必须同步补，否则编译错即提示）。

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core Cargo.lock
git commit -m "feat(core): refresh 管道客户端 + 游戏进程检测"
```

---

### Task 3: `liquimod-refresh-helper` 提权 bin crate

**Files:**
- Create: `crates/liquimod-refresh-helper/Cargo.toml`
- Create: `crates/liquimod-refresh-helper/src/main.rs`
- Modify: `Cargo.toml`（workspace members 加 `crates/liquimod-refresh-helper`）

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "liquimod-refresh-helper"
version = "0.1.0"
edition = "2021"

[dependencies]
windows = { version = "0.61", features = [
    "Win32_Foundation",
    "Win32_System_Pipes",
    "Win32_Storage_FileSystem",
    "Win32_UI_Input_KeyboardAndMouse",
] }
```

- [ ] **Step 2: main.rs 完整实现（含管道回环测试）**

```rust
//! F10 刷新提权 helper：监听命名管道，收到 "1" 向系统注入一次 F10。
//! 客户端（主 app）断开管道即退出，随 app 生命周期。
//! 由主 app 以 ShellExecuteW runas 提权启动（无清单，无键盘监听，无网络）。

use std::io::Read;

const PIPE: &str = r"\\.\pipe\liquimod-refresh";

/// 从字节流读数据，每批含 b'1' 即触发一次 on_poke。EOF/错误时返回（=退出）。
fn serve(mut read: impl Read, mut on_poke: impl FnMut()) {
    let mut buf = [0u8; 64];
    loop {
        match read.read(&mut buf) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                if buf[..n].contains(&b'1') {
                    on_poke();
                }
            }
        }
    }
}

#[cfg(windows)]
fn send_f10() {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VK_F10,
    };
    let down = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_F10,
                wScan: 0,
                dwFlags: KEYBD_EVENT_FLAGS(0),
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let mut up = down;
    up.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    unsafe { SendInput(&[down, up], std::mem::size_of::<INPUT>() as i32) };
}

#[cfg(not(windows))]
fn send_f10() {}

#[cfg(windows)]
fn main() {
    use std::os::windows::io::FromRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND};
    use windows::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_TYPE_BYTE, PIPE_WAIT,
    };
    let wide: Vec<u16> = PIPE.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let handle = CreateNamedPipeW(
            windows::core::PCWSTR(wide.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1, // 单实例
            0,
            0,
            0,
            None,
        );
        let handle: HANDLE = match handle {
            Ok(h) => h,
            Err(_) => return, // 已在运行或创建失败：直接退出
        };
        if ConnectNamedPipe(handle, None).is_err() {
            return;
        }
        let file = std::fs::File::from_raw_handle(handle.0 as *mut _);
        serve(file, send_f10);
        // file drop → 句柄关闭 → 进程退出
    }
}

#[cfg(not(windows))]
fn main() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_batch_with_one_triggers_once_and_eof_stops() {
        let data = b"111xx1"; // 两批读：第一模拟一次 read 返回 "111"，第二批 "xx1"？
        // Cursor 一次 read 尽量填满缓冲——64 > 6，故只 read 一次，应触发 1 次。
        let mut count = 0;
        serve(std::io::Cursor::new(data.to_vec()), |_| count += 1);
        assert_eq!(count, 1);
    }

    #[test]
    fn batch_without_one_does_not_trigger() {
        let mut count = 0;
        serve(std::io::Cursor::new(b"hello".to_vec()), |_| count += 1);
        assert_eq!(count, 0);
    }

    #[test]
    fn split_batches_each_trigger() {
        // 模拟分两次到达：用按块迭代的 reader
        struct Chunked(Vec<std::io::Cursor<Vec<u8>>>);
        impl std::io::Read for Chunked {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                if self.0.is_empty() {
                    return Ok(0);
                }
                let n = self.0[0].read(buf)?;
                if n == 0 {
                    self.0.remove(0);
                    return self.read(buf);
                }
                Ok(n)
            }
        }
        let chunks = Chunked(vec![
            std::io::Cursor::new(b"1".to_vec()),
            std::io::Cursor::new(b"zz".to_vec()),
            std::io::Cursor::new(b"1".to_vec()),
        ]);
        let mut count = 0;
        serve(chunks, |_| count += 1);
        assert_eq!(count, 2);
    }
}
```

（`each_batch_with_one_triggers_once_and_eof_stops` 里的注释保留——Cursor 行为即如此。）

- [ ] **Step 3: workspace 注册 + 测试**

根 `Cargo.toml` members 数组加 `"crates/liquimod-refresh-helper"`。

Run: `cargo test -p liquimod-refresh-helper` 与 `cargo build --release -p liquimod-refresh-helper`
Expected: 3 tests PASS；`target/release/liquimod-refresh-helper.exe` 生成。

- [ ] **Step 4: Commit**

```bash
git add crates/liquimod-refresh-helper Cargo.toml Cargo.lock
git commit -m "feat(helper): F10 提权 helper（管道服务端 + SendInput）"
```

---

### Task 4: app 接线（watcher 启动 + 对账推送 + 自动刷新）

**Files:**
- Modify: `app/src-tauri/src/state.rs`（加 watcher/refresh 字段）
- Modify: `app/src-tauri/src/lib.rs`（setup 启动 watcher；回调对账并 emit）
- Modify: `app/src-tauri/src/commands.rs`（三命令加 AppHandle；成功后 maybe_refresh_game；choose_mods_dir 重启 watcher）
- Test: app 新增 `refresh.rs` 逻辑难单测的部分以函数抽出后测试（见下）

- [ ] **Step 1: state.rs 加字段**

```rust
use liquimod_core::refresh::RefreshClient;
use liquimod_core::watch::LibraryWatcher;

pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub config_path: PathBuf,
    pub library: Arc<Mutex<Library>>,
    /// 目录监控；mods_dir 变更时整体替换（旧实例 Drop 即停）。
    pub watcher: Mutex<Option<LibraryWatcher>>,
    /// 提权 helper 管道；持有即 helper 存活。
    pub refresh: Mutex<Option<RefreshClient>>,
}
```

`bootstrap()` 返回结构体加 `watcher: Mutex::new(None), refresh: Mutex::new(None),`。

- [ ] **Step 2: lib.rs 抽 `start_watcher` + setup 调用**

lib.rs 新增（供 setup 与 choose_mods_dir 复用）：

```rust
use liquimod_core::deploy::Deployer;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

/// （重）启动目录监控：变动 → 对账 → emit library-changed（added/removed 为 Mod 数增量）。
/// 绝不改动用户文件：scan 只对账 DB，reconcile 只清指向仓库内的孤儿链接。
pub fn start_watcher(app: &tauri::AppHandle, state: &AppState) {
    let (root, mods_dir) = {
        let cfg = state.config.lock().unwrap();
        (cfg.library_root.clone(), cfg.mods_dir.clone())
    };
    let library = Arc::clone(&state.library);
    let app2 = app.clone();
    let mods_dir2 = mods_dir.clone();
    let watcher = liquimod_core::watch::start(root, mods_dir, move || {
        let lib = library.lock().unwrap();
        let before = lib.list().map(|m| m.len()).unwrap_or(0);
        if lib.scan().is_err() {
            return;
        }
        if let Some(dir) = &mods_dir2 {
            let _ = Deployer::new(&lib, dir).reconcile();
        }
        let after = lib.list().map(|m| m.len()).unwrap_or(0);
        drop(lib);
        let added = after.saturating_sub(before);
        let removed = before.saturating_sub(after);
        let _ = app2.emit(
            "library-changed",
            serde_json::json!({ "added": added, "removed": removed }),
        );
    });
    if let Ok(w) = watcher {
        *state.watcher.lock().unwrap() = Some(w);
    }
}
```

setup 闭包里 `.manage(state)` 之后加：

```rust
            let app_handle = app.handle().clone();
            start_watcher(&app_handle, app.state::<AppState>().inner());
```

（若 setup 现有结构不同，按现状最小改动接入；要点是 manage 后调用。）

lib.rs 需 `use serde_json;`（app Cargo.toml 应已有 serde_json——若无则加 `serde_json = "1"`）。

- [ ] **Step 3: commands.rs 自动刷新**

commands.rs 顶部加：

```rust
use liquimod_core::games::hsr::Hsr;
use liquimod_core::games::Game;
use liquimod_core::refresh::{is_game_running, RefreshClient, HELPER_EXE};
use tauri::Emitter;

/// 游戏运行中则通知 helper 发 F10；失败只 toast 不阻断。
fn maybe_refresh_game(app: &tauri::AppHandle, state: &AppState) {
    if !is_game_running(Hsr::shared().process_names()) {
        return;
    }
    let helper = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(HELPER_EXE)));
    let Some(helper) = helper.filter(|p| p.exists()) else {
        let _ = app.emit("liquimod-toast", "未找到刷新 helper，跳过游戏内刷新".to_string());
        return;
    };
    let mut guard = state.refresh.lock().unwrap();
    if guard.is_none() {
        match RefreshClient::connect_or_launch(&helper) {
            Ok(c) => *guard = Some(c),
            Err(e) => {
                let _ = app.emit("liquimod-toast", format!("刷新 helper 启动失败：{e}"));
                return;
            }
        }
    }
    if let Some(client) = guard.as_mut() {
        if client.poke().is_err() {
            *guard = None; // helper 死了，下次重连
            let _ = app.emit("liquimod-toast", "刷新 helper 连接断开，下次操作将重试".to_string());
        }
    }
}
```

给 `set_mod_enabled`/`install_mod`/`uninstall_mod` 三个命令函数加参数 `app: tauri::AppHandle,`（Tauri 自动注入），在 spawn_blocking 成功返回 `Ok(...)` 之后、函数返回前调用 `maybe_refresh_game(&app, &state)`（注意在 await 之外，state 是 `tauri::State` 直接可用）。`install_mod` 只对 `InstallOutcome::Installed` 调用（`NeedsPassword` 不调）。

`choose_mods_dir` 命令同样加 `app: tauri::AppHandle,`，保存配置后调用 `crate::start_watcher(&app, state.inner())` 重启监控。

- [ ] **Step 4: 测试**

app 层为薄胶水。加一个小测试锁定 emit 载荷形状（json! key 名）即可，放 commands.rs tests：

```rust
    #[test]
    fn library_changed_payload_shape() {
        let v = serde_json::json!({ "added": 2usize, "removed": 1usize });
        assert_eq!(v["added"], 2);
        assert_eq!(v["removed"], 1);
        assert!(v.get("count").is_none());
    }
```

- [ ] **Step 5: 全量验证**

Run: `cargo test -p liquimod-app`、`cargo clippy --workspace --all-targets`、`cargo fmt --all -- --check`
Expected: 全绿。

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri
git commit -m "feat(app): watcher 对账推送 + 游戏运行时自动 F10"
```

---

### Task 5: 前端 Toast + 事件监听

**Files:**
- Create: `app/src/lib/toast.ts`
- Create: `app/src/lib/components/Toast.svelte`
- Modify: `app/src/routes/+layout.svelte`（挂载 Toast）
- Modify: `app/src/routes/+page.svelte`（listen library-changed / liquimod-toast）
- Test: `app/src/lib/toast.test.ts`、`app/src/lib/components/Toast.test.ts`

**UI 定稿（主模型设计，逐字实现）：**

- [ ] **Step 1: toast.ts**

```ts
export interface ToastItem {
  id: number;
  message: string;
}

let nextId = 1;
export const toasts = $state<ToastItem[]>([]);

export function toast(message: string, durationMs = 4000): void {
  const id = nextId++;
  toasts.push({ id, message });
  setTimeout(() => {
    const i = toasts.findIndex((t) => t.id === id);
    if (i >= 0) toasts.splice(i, 1);
  }, durationMs);
}
```

- [ ] **Step 2: Toast.svelte**

右下角纵向堆叠，液态玻璃 pill，淡入上浮：

```svelte
<script lang="ts">
  import { fly } from "svelte/transition";
  import { toasts } from "$lib/toast";
</script>

<div class="fixed bottom-4 right-4 z-[60] flex flex-col items-end gap-2 pointer-events-none">
  {#each toasts as t (t.id)}
    <div
      transition:fly={{ y: 12, duration: 200 }}
      class="glass radius-pill px-4 h-9 flex items-center text-sm"
      role="status"
    >
      {t.message}
    </div>
  {/each}
</div>
```

- [ ] **Step 3: +layout.svelte 挂载**

`<script>` 加 `import Toast from "$lib/components/Toast.svelte";`，`{@render children()}` 之后加 `<Toast />`。

- [ ] **Step 4: +page.svelte 事件监听**

script 顶部加：

```ts
  import { listen } from "@tauri-apps/api/event";
  import { toast } from "$lib/toast";
```

onMount 内（参照现有 onDragDropEvent 的注册/清理模式，注意 cancelled 守卫）加：

```ts
    let unlistenChanged: (() => void) | undefined;
    let unlistenToast: (() => void) | undefined;
    listen<{ added: number; removed: number }>("library-changed", (e) => {
      const { added, removed } = e.payload;
      if (added > 0 || removed > 0) toast(`检测到仓库变动：+${added} / -${removed}`);
      refresh();
    }).then((u) => {
      if (cancelled) u();
      else unlistenChanged = u;
    });
    listen<string>("liquimod-toast", (e) => toast(e.payload)).then((u) => {
      if (cancelled) u();
      else unlistenToast = u;
    });
```

onMount 清理函数里在现有清理后加 `unlistenChanged?.(); unlistenToast?.();`。

- [ ] **Step 5: 测试**

`toast.test.ts`（vi.useFakeTimers）：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { toast, toasts } from "./toast";

describe("toast store", () => {
  beforeEach(() => {
    toasts.length = 0;
    vi.useFakeTimers();
    return () => vi.useRealTimers();
  });

  it("push 后到时自动移除", () => {
    toast("hello");
    expect(toasts).toHaveLength(1);
    vi.advanceTimersByTime(4000);
    expect(toasts).toHaveLength(0);
  });

  it("多条独立计时", () => {
    toast("a", 1000);
    toast("b", 5000);
    vi.advanceTimersByTime(1000);
    expect(toasts.map((t) => t.message)).toEqual(["b"]);
  });
});
```

`Toast.test.ts`：

```ts
import { render, screen } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import Toast from "./Toast.svelte";
import { toast, toasts } from "$lib/toast";

describe("Toast 组件", () => {
  it("渲染 store 中的消息", () => {
    toasts.length = 0;
    render(Toast);
    toast("检测到仓库变动：+1 / -0");
    expect(screen.getByRole("status")).toHaveTextContent("检测到仓库变动：+1 / -0");
    toasts.length = 0;
  });
});
```

（toast 的 setTimeout 在组件测试里真实计时无碍——断言后立即清空。）

- [ ] **Step 6: 验证 + Commit**

Run: `npm test`、`npm run check`
Expected: 全绿。

```bash
git add app/src
git commit -m "feat(app): Toast 组件 + 仓库变动/提示事件监听"
```

---

### Task 6: E2E 验证 + 收尾

- [ ] **Step 1: 全量**

Run: `cargo test --workspace`、`npm test`（app/）
Expected: 全绿。

- [ ] **Step 2: 构建**

先杀运行中的 `liquimod-app` / `liquimod-refresh-helper` 进程，然后 app/ 下：

```bash
npm run build
cargo build --release --workspace --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml
```

确认 `target/release/liquimod-refresh-helper.exe` 与 `liquimod-app.exe` 并存；读 exe 字节验证前端 hash 内嵌（参照里程碑 4 做法）。

- [ ] **Step 3: 手动验收（交主人）**

1. 应用运行中用资源管理器往 `Library/<角色>/` 拷入一个文件夹 → 数秒内 toast「检测到仓库变动：+1 / -0」且网格计数 +1（scan 会收编新目录）。
2. 从 Library 删掉一个 Mod 目录 → toast「-1」且网格回落。
3. Mods/ 下手动删掉一个启用的 Junction → 对账清理/修复，UI 状态一致。
4. （可选，需游戏运行）切换 Mod → 首次弹 UAC → 游戏内 3Dmigoto 刷新生效；此后不再弹。

- [ ] **Step 4: 最终审查**

双审查（spec 合规 + 代码质量）子代理对 `9c29993..HEAD` 全量 diff，修复闭环。

---

## Self-Review 记录

- Spec 覆盖：§4.4 watcher 对账→Task 1/4；§4.5 helper→Task 2/3/4；§4.3 启停后自动刷新→Task 4 Step 3；推送 UI→Task 4 emit + Task 5 前端。MVP 清单「游戏进程监控 + 自动 F10」→Task 2/4。「绝不静默改动用户文件」→对账仅 scan+reconcile 既有语义（只清孤儿链接），toast 明示。
- 类型一致性：`RefreshClient`/`LibraryWatcher`/`start_watcher`/`maybe_refresh_game`/`toast()` 全文一致；emit 载荷 `{added, removed}` 前后端一致。
- 无占位符；所有代码步骤含完整代码。
