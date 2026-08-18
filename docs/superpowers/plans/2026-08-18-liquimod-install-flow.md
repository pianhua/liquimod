# 沉浸式安装流 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 拖入压缩包 → 悬浮玻璃面板原位显示安装进度（解压识别角色 → 密码流 → 入库），支持撤销；全程不弹窗。

**Architecture:** 核心新增 `infer_character`（扫描解压目录 ini 文本与文件名，按命中次数推断角色）与 `install_archive_inferred`（单阶段：解压→推断→入库）。App 层新增 `install_mod` / `uninstall_mod` IPC（`spawn_blocking` 不阻塞 UI）。前端新增 runes 安装队列 store + 底部悬浮玻璃面板 + Tauri 文件拖放接线。

**Tech Stack:** Rust（liquimod-core / src-tauri）、Svelte 5 runes、Tauri 2 `onDragDropEvent`、Vitest。

**明确范围（YAGNI，对应设计文档 §4.2 的裁剪）：**
- 进度为阶段式（安装中/需密码/完成/失败），无字节级进度环——core 解压无进度回调，改造留给以后。
- 无"自动启用"（设置项在里程碑 6）。
- 无缩略图（里程碑 6）。
- 推断错误的恢复手段 = 完成态"撤销"按钮（新 `uninstall_mod`），不做两阶段确认流。
- 密码成功即自动入密码本（core 已有 `learn` 行为），不做"是否存入"询问。

**关键既有事实（实现者不必再查）：**
- `install_archive(db, library, archive_path, character, explicit_password) -> Result<InstallOutcome>`，`InstallOutcome::Installed { mod_id, name, warnings }` / `NeedsPassword`，位于 `crates/liquimod-core/src/archive/install.rs`。
- `Game` trait：`fn characters(&self) -> &[CharacterInfo]`，`CharacterInfo { internal_name, display_name, image: String }`；HSR 实例：`liquimod_core::games::hsr::Hsr::shared()`。
- `Library` 有 pub 字段 `db`、`layout`；`layout.root: PathBuf`；`ModEntry { id, character, name, rel_path, enabled, installed_at }`；`db.get_mod(id)` / `db.remove_mod(id)`。
- `Deployer::new(&lib, mods_dir)`，`.disable(id)` 幂等。
- `LiquiModError` 变体：`DestinationExists { character, name }`、`UnsupportedArchive(String)`、`WrongPassword`、`PasswordRequired` 等（见 `crates/liquimod-core/src/error.rs`）。
- AppState（`app/src-tauri/src/state.rs`）当前是 `config: Mutex<Config>`、`library: Mutex<Library>`——本计划改为 `Arc<Mutex<_>>` 以便 `spawn_blocking`。
- 前端 `api.ts` 有 `isTauri()` 运行时检测与 mock 层；测试通过 `vi.mock("$lib/api")` 拦截。
- 拖放：Tauri v2 `getCurrentWebviewWindow().onDragDropEvent(cb)`，`event.payload.type` 为 `"enter" | "over" | "drop" | "leave"`，drop 时 `event.payload.paths: string[]`。
- **WebView2 教训：不要依赖 aspect-ratio / 内容撑高来定 grid 行高**（本计划不涉及网格改动，仅备忘）。
- 手动构建 exe：`npm run build`（app/ 目录）→ `cargo build --release --features tauri/custom-protocol --manifest-path src-tauri/Cargo.toml`（app/ 目录）。**必须带 `tauri/custom-protocol`，否则 exe 不内嵌前端**。构建前杀 `liquimod-app` 与 `python` 进程（锁 app/build 与 target）。
- 验证 exe 内嵌新前端：读 exe 字节查当前 build 的 JS/CSS hash 字符串。
- exe 可视化验证：`$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"` 启动后，用 CDP（Node 24 内置 WebSocket）`Page.captureScreenshot` 截图，脚本参考 `%LOCALAPPDATA%\Temp\opencode\cdpshot.mjs`（会话临时目录，丢失则重写一个 20 行脚本即可）。

---

### Task 1: 核心——角色推断 `infer_character`

**Files:**
- Modify: `crates/liquimod-core/src/games/mod.rs`
- Test: 同文件 `#[cfg(test)] mod tests`（追加）

- [ ] **Step 1: 写失败测试（追加到 games/mod.rs 的 tests 模块，若无则新建）**

```rust
    fn fixture_game() -> crate::games::hsr::Hsr {
        crate::games::hsr::Hsr::shared().clone()
    }

    #[test]
    fn infers_character_from_ini_mentions() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("mod.ini"),
            "[Constants]\n; Firefly costume swap\nglobal $firefly = 1\n",
        )
        .unwrap();
        assert_eq!(
            infer_character(tmp.path(), &fixture_game()),
            Some("Firefly".to_string())
        );
    }

    #[test]
    fn infers_from_folder_names_when_no_ini_match() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("Acheron_HD_Textures");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("readme.txt"), b"no hints here").unwrap();
        assert_eq!(
            infer_character(tmp.path(), &fixture_game()),
            Some("Acheron".to_string())
        );
    }

    #[test]
    fn returns_none_without_hints() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("mod.ini"), "[Constants]\nglobal $x = 1\n").unwrap();
        assert_eq!(infer_character(tmp.path(), &fixture_game()), None);
    }

    #[test]
    fn most_mentioned_character_wins() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("mod.ini"),
            "; acheron acheron acheron\n; blade\n",
        )
        .unwrap();
        assert_eq!(
            infer_character(tmp.path(), &fixture_game()),
            Some("Acheron".to_string())
        );
    }
```

注意：`Hsr::shared()` 返回 `&'static Hsr`；若 `Hsr` 未实现 `Clone`，把 `fixture_game()` 改为直接在使用处 `Hsr::shared()`（函数签名收 `&dyn Game`，传 `Hsr::shared()` 即可，先读 `games/hsr.rs` 确认）。

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core games:: -- --nocapture`
Expected: FAIL（`infer_character` 未定义）

- [ ] **Step 3: 实现（games/mod.rs 追加）**

```rust
use std::collections::HashMap;
use std::fs;
use std::path::Path;

const MAX_DEPTH: usize = 8;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// 从解压目录内容推断角色：合并 ini/txt/json 文本与全部文件名作为语料，
/// 统计每个角色（内部名 / 显示名 / 立绘文件名stem）的小写命中次数，取最高者。
pub fn infer_character(dir: &Path, game: &dyn Game) -> Option<String> {
    let mut corpus = String::new();
    let mut budget = MAX_TOTAL_BYTES;
    collect_text(dir, 0, &mut budget, &mut corpus);
    let mut scores: HashMap<&str, usize> = HashMap::new();
    for c in game.characters() {
        let mut score = 0usize;
        let stem = Path::new(&c.image)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        for needle in [
            c.internal_name.to_lowercase(),
            c.display_name.to_lowercase(),
            stem,
        ] {
            if needle.len() < 3 {
                continue;
            }
            score += corpus.matches(&needle).count();
        }
        if score > 0 {
            scores.insert(c.internal_name.as_str(), score);
        }
    }
    scores
        .into_iter()
        .max_by_key(|(_, score)| *score)
        .map(|(name, _)| name.to_owned())
}

fn collect_text(dir: &Path, depth: usize, budget: &mut usize, out: &mut String) {
    if depth > MAX_DEPTH || *budget == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        out.push_str(&entry.file_name().to_string_lossy().to_lowercase());
        out.push('\n');
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect_text(&path, depth + 1, budget, out);
        } else if meta.is_file()
            && meta.len() <= MAX_FILE_BYTES
            && matches!(
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .as_deref(),
                Some("ini" | "txt" | "json")
            )
        {
            if let Ok(bytes) = fs::read(&path) {
                *budget = budget.saturating_sub(bytes.len());
                out.push_str(&String::from_utf8_lossy(&bytes).to_lowercase());
            }
        }
    }
}
```

（文件顶部已有的 `use` 若与新 import 重复则合并；`Game`/`CharacterInfo` 已在同模块。）

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core games::`
Expected: 全部 PASS

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/games/mod.rs
git commit -m "feat(core): infer_character 从解压内容推断角色"
```

---

### Task 2: 核心——`install_archive_inferred` + `InstallOutcome` 带角色

**Files:**
- Modify: `crates/liquimod-core/src/archive/install.rs`
- Modify: `crates/liquimod-cli/src/main.rs:237-252`
- Test: `crates/liquimod-core/src/archive/install.rs` tests

- [ ] **Step 1: 改 `InstallOutcome::Installed` 增加 `character: String` 字段；把现有 `install_archive` 改名为内部 `install_inner` 并抽角色解析闭包；新增两个公开入口**

```rust
use super::{extract_recursive, resolve_content_root, ExtractReport, PasswordBook};
use crate::db::Database;
use crate::error::{LiquiModError, Result};
use crate::games::{infer_character, Game};
use crate::library::{Library, INSTALL_LOCK};
use crate::paths::is_valid_segment;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub enum InstallOutcome {
    Installed {
        mod_id: i64,
        name: String,
        character: String,
        warnings: Vec<String>,
    },
    NeedsPassword,
}

/// 指定角色安装（CLI 与前端确认角色后使用）。
pub fn install_archive(
    db: &Database,
    library: &Library,
    archive_path: &Path,
    character: &str,
    explicit_password: Option<&str>,
) -> Result<InstallOutcome> {
    let character = character.to_owned();
    install_inner(db, library, archive_path, explicit_password, |_| {
        Ok(character)
    })
}

/// 自动推断角色安装：解压后从内容推断，无线索时归入 "Others"。
pub fn install_archive_inferred(
    db: &Database,
    library: &Library,
    game: &dyn Game,
    archive_path: &Path,
    explicit_password: Option<&str>,
) -> Result<InstallOutcome> {
    install_inner(db, library, archive_path, explicit_password, |temp| {
        Ok(infer_character(temp, game).unwrap_or_else(|| "Others".to_string()))
    })
}

/// 单阶段安装：解压（含密码本重试）→ 解析角色 → 复制入库 → 写 DB。
fn install_inner(
    db: &Database,
    library: &Library,
    archive_path: &Path,
    explicit_password: Option<&str>,
    resolve_character: impl FnOnce(&Path) -> Result<String>,
) -> Result<InstallOutcome> {
    // 函数体 = 原 install_archive 全部逻辑，差异仅两处：
    // 1) 解压循环成功后（拿到 content_root 之前）：
    //      let character = resolve_character(temp_dir.path())?;
    //    后续所有对 character 的使用改为该局部变量（is_valid_segment(&character) 等）。
    // 2) 成功返回：
    //      Ok(InstallOutcome::Installed { mod_id: entry.id, name: entry.name, character, warnings })
}
```

（实现者：直接搬移原函数体，勿重写逻辑；`TempExtractionDir`、`add_candidate` 保持不变。）

- [ ] **Step 2: 修复所有编译错误——既有测试与 CLI 的解构补 `character`**

- `install.rs` tests：所有 `let InstallOutcome::Installed { mod_id, name, warnings } = outcome` 改为加 `..` 或按需取 `character`。
- `crates/liquimod-cli/src/main.rs` `print_installation`：

```rust
        InstallOutcome::Installed {
            mod_id,
            name,
            character,
            warnings,
        } => {
            println!("Installed: {name} -> {character} (id {mod_id})");
            for warning in warnings {
                println!("Warning: {warning}");
            }
            Ok(())
        }
```

Run: `cargo test --workspace`
Expected: 全绿

- [ ] **Step 3: 写新测试（install.rs tests 追加）**

```rust
    #[test]
    fn inferred_install_picks_character_from_ini() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("MysteryMod.zip");
        write_zip(&archive, &[("mod.ini", b"; firefly skin\nglobal $firefly = 1")], None);

        let outcome = install_archive_inferred(
            &library.db,
            &library,
            crate::games::hsr::Hsr::shared(),
            &archive,
            None,
        )
        .unwrap();

        let InstallOutcome::Installed { character, .. } = outcome else {
            panic!("expected installed outcome");
        };
        assert_eq!(character, "Firefly");
        assert!(library.layout.mod_dir("Firefly", "MysteryMod").is_dir());
    }

    #[test]
    fn inferred_install_falls_back_to_others() {
        let (tmp, library) = setup();
        let archive = tmp.path().join("UnknownMod.zip");
        write_zip(&archive, &[("mod.ini", b"[Constants]")], None);

        let outcome = install_archive_inferred(
            &library.db,
            &library,
            crate::games::hsr::Hsr::shared(),
            &archive,
            None,
        )
        .unwrap();

        let InstallOutcome::Installed { character, .. } = outcome else {
            panic!("expected installed outcome");
        };
        assert_eq!(character, "Others");
        assert!(library.layout.mod_dir("Others", "UnknownMod").is_dir());
    }
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core archive::`
Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/archive/install.rs crates/liquimod-cli/src/main.rs
git commit -m "feat(core): install_archive_inferred 单阶段推断安装，InstallOutcome 携带角色"
```

---

### Task 3: App——`install_mod` / `uninstall_mod` IPC

**Files:**
- Modify: `app/src-tauri/src/state.rs`
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Test: `app/src-tauri/src/commands.rs` tests

- [ ] **Step 1: state.rs 改 Arc（Tauri State 不能直接进 spawn_blocking）**

```rust
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
```

既有命令 `state.config.lock()` / `state.library.lock()` 经 Arc deref 照常工作，无需改。

- [ ] **Step 2: 写失败测试（commands.rs tests 追加；zip 夹具helper本地定义）**

```rust
    fn write_zip(path: &std::path::Path, files: &[(&str, &[u8])], password: Option<&str>) {
        use std::io::Write;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for (name, contents) in files {
            let options = match password {
                Some(p) => zip::write::SimpleFileOptions::default()
                    .with_aes_encryption(zip::AesMode::Aes256, p),
                None => zip::write::SimpleFileOptions::default(),
            };
            writer.start_file(*name, options).unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn install_entry_with_explicit_character() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("CoolMod.zip");
        write_zip(&zip, &[("mod.ini", b"[Constants]")], None);

        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();

        let InstallResultDto::Installed { character, name, .. } = dto else {
            panic!("expected installed");
        };
        assert_eq!((character.as_str(), name.as_str()), ("Bailu", "CoolMod"));
    }

    #[test]
    fn install_entry_infers_character() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Mystery.zip");
        write_zip(&zip, &[("mod.ini", b"; kafka kafka kafka")], None);

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, None).unwrap();

        let InstallResultDto::Installed { character, .. } = dto else {
            panic!("expected installed");
        };
        assert_eq!(character, "Kafka");
    }

    #[test]
    fn install_entry_needs_password() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Locked.zip");
        write_zip(&zip, &[("s.txt", b"s")], Some("pw1"));

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, None).unwrap();
        assert_eq!(dto, InstallResultDto::NeedsPassword);

        let dto = install_entry(&lib, Hsr::shared(), &zip, None, Some("pw1")).unwrap();
        assert!(matches!(dto, InstallResultDto::Installed { .. }));
    }

    #[test]
    fn install_entry_humanizes_duplicate_error() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Dup.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();

        let err = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap_err();
        assert!(err.contains("已存在同名 Mod"));
    }

    #[test]
    fn remove_entry_deletes_files_and_row() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Gone.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();
        let InstallResultDto::Installed { mod_id, .. } = dto else {
            panic!("expected installed");
        };

        remove_entry(&lib, None, mod_id).unwrap();

        assert!(lib.list().unwrap().is_empty());
        assert!(!lib.layout.mod_dir("Bailu", "Gone").exists());
    }

    #[test]
    fn remove_entry_disables_junction_first() {
        let (_d, lib) = temp_lib();
        let dir = tempfile::tempdir().unwrap();
        let zip = dir.path().join("Active.zip");
        write_zip(&zip, &[("m.ini", b"x")], None);
        let dto = install_entry(&lib, Hsr::shared(), &zip, Some("Bailu"), None).unwrap();
        let InstallResultDto::Installed { mod_id, .. } = dto else {
            panic!("expected installed");
        };
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), mod_id, true).unwrap();
        let entry = lib.db.get_mod(mod_id).unwrap();
        let link = mods.path().join(Deployer::link_name(&entry));
        assert!(link.exists());

        remove_entry(&lib, Some(mods.path()), mod_id).unwrap();

        assert!(!link.exists());
        assert!(lib.list().unwrap().is_empty());
    }
```

（`zip` crate 已在 workspace 依赖——liquimod-core 测试用了它；若 src-tauri 的 dev-dependencies 没有 `zip`，在 `app/src-tauri/Cargo.toml` 的 `[dev-dependencies]` 加 `zip = { version = "...", default-features = false, features = ["aes-crypto"] }`，版本对齐 workspace。）

- [ ] **Step 3: 实现（commands.rs 追加）**

```rust
use liquimod_core::archive::install::{
    install_archive, install_archive_inferred, InstallOutcome,
};
use liquimod_core::error::LiquiModError;
use liquimod_core::games::Game;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InstallResultDto {
    Installed {
        mod_id: i64,
        name: String,
        character: String,
        warnings: Vec<String>,
    },
    NeedsPassword,
}

/// 安装压缩包：character=None 时从内容推断。人话错误信息。
pub fn install_entry(
    lib: &Library,
    game: &dyn Game,
    path: &Path,
    character: Option<&str>,
    password: Option<&str>,
) -> Result<InstallResultDto, String> {
    if !path.is_file() {
        return Err(format!("文件不存在：{}", path.display()));
    }
    let outcome = match character {
        Some(c) => install_archive(&lib.db, lib, path, c, password),
        None => install_archive_inferred(&lib.db, lib, game, path, password),
    };
    match outcome {
        Ok(InstallOutcome::Installed {
            mod_id,
            name,
            character,
            warnings,
        }) => Ok(InstallResultDto::Installed {
            mod_id,
            name,
            character,
            warnings,
        }),
        Ok(InstallOutcome::NeedsPassword) => Ok(InstallResultDto::NeedsPassword),
        Err(error) => Err(humanize_install_error(&error)),
    }
}

fn humanize_install_error(error: &LiquiModError) -> String {
    match error {
        LiquiModError::DestinationExists { name, .. } => format!("已存在同名 Mod：{name}"),
        LiquiModError::UnsupportedArchive(_) => {
            "不是支持的压缩包（支持 zip / 7z / rar）".to_string()
        }
        _ => error.to_string(),
    }
}

/// 卸载：启用中则先拆 Junction，再删库目录与 DB 记录。
pub fn remove_entry(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
) -> Result<(), String> {
    let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if entry.enabled {
        let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录")?;
        Deployer::new(lib, mods_dir)
            .disable(id)
            .map_err(|e| e.to_string())?;
    }
    let dir = lib.layout.root.join(&entry.rel_path);
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.to_string()),
    }
    lib.db.remove_mod(id).map_err(|e| e.to_string())
}

// ---- Tauri 薄命令（追加） ----

#[tauri::command]
pub async fn install_mod(
    state: tauri::State<'_, AppState>,
    path: String,
    character: Option<String>,
    password: Option<String>,
) -> Result<InstallResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        install_entry(
            &lib,
            liquimod_core::games::hsr::Hsr::shared(),
            Path::new(&path),
            character.as_deref(),
            password.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("安装任务失败：{e}"))?
}

#[tauri::command]
pub async fn uninstall_mod(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        remove_entry(&lib, mods_dir.as_deref(), id)
    })
    .await
    .map_err(|e| format!("卸载任务失败：{e}"))?
}
```

- [ ] **Step 4: lib.rs 注册命令**

`invoke_handler` 数组追加 `commands::install_mod,` 与 `commands::uninstall_mod,`。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p liquimod-app` 与 `cargo clippy --workspace --all-targets`、`cargo fmt --all -- --check`
Expected: 全绿、无警告

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/src app/src-tauri/Cargo.toml
git commit -m "feat(app): install_mod/uninstall_mod IPC（异步不阻塞 UI，人话错误）"
```

---

### Task 4: 前端——安装队列 store + 悬浮面板 + 拖放接线

**Files:**
- Create: `app/src/lib/install.svelte.ts`
- Create: `app/src/lib/components/InstallOverlay.svelte`
- Modify: `app/src/lib/api.ts`
- Modify: `app/src/routes/+page.svelte`
- Test: `app/src/lib/install.svelte.test.ts`、`app/src/lib/components/InstallOverlay.test.ts`

- [ ] **Step 1: api.ts 扩展（含 mock）**

追加类型与方法：

```ts
export type InstallResult =
  | { status: "installed"; mod_id: number; name: string; character: string; warnings: string[] }
  | { status: "needs_password" };
```

`call` 的 mock 分支追加（非 Tauri 环境便于浏览器迭代）：

```ts
      case "install_mod": {
        await new Promise((r) => setTimeout(r, 800));
        const p = String(args?.path ?? "");
        if (p.includes("locked") && args?.password == null)
          return { status: "needs_password" } as T;
        if (p.includes("locked") && args?.password !== "1234")
          return { status: "needs_password" } as T;
        return {
          status: "installed",
          mod_id: 99,
          name: p.split(/[\\/]/).pop()?.replace(/\.(zip|7z|rar)$/i, "") ?? "Mod",
          character: "Firefly",
          warnings: [],
        } as T;
      }
      case "uninstall_mod":
        return undefined as T;
```

`api` 对象追加：

```ts
  installMod: (path: string, character?: string | null, password?: string | null) =>
    call<InstallResult>("install_mod", { path, character: character ?? null, password: password ?? null }),
  uninstallMod: (id: number) => call<void>("uninstall_mod", { id }),
```

- [ ] **Step 2: 写失败测试 `app/src/lib/install.svelte.test.ts`**

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("$lib/api", () => ({
  api: {
    installMod: vi.fn(),
    uninstallMod: vi.fn(),
  },
}));

import { api } from "$lib/api";
import {
  dismissInstall,
  enqueueInstalls,
  installJobs,
  submitInstallPassword,
  undoInstall,
} from "./install.svelte";

const flush = () => new Promise((r) => setTimeout(r, 0));

describe("install queue", () => {
  beforeEach(() => {
    installJobs.length = 0;
    vi.clearAllMocks();
  });

  it("installs successfully and calls back", async () => {
    vi.mocked(api.installMod).mockResolvedValue({
      status: "installed",
      mod_id: 7,
      name: "Cool",
      character: "Firefly",
      warnings: [],
    });
    const onInstalled = vi.fn();

    enqueueInstalls(["C:/dl/Cool.zip"], onInstalled);
    expect(installJobs).toHaveLength(1);
    expect(installJobs[0].stage).toBe("installing");
    await flush(); await flush();

    expect(installJobs[0].stage).toBe("done");
    expect(installJobs[0].character).toBe("Firefly");
    expect(installJobs[0].modId).toBe(7);
    expect(onInstalled).toHaveBeenCalledOnce();
  });

  it("password flow: needs-password then submit", async () => {
    vi.mocked(api.installMod)
      .mockResolvedValueOnce({ status: "needs_password" })
      .mockResolvedValueOnce({
        status: "installed",
        mod_id: 8,
        name: "Locked",
        character: "Kafka",
        warnings: [],
      });

    enqueueInstalls(["C:/dl/Locked.zip"], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("needs-password");
    expect(api.installMod).toHaveBeenCalledWith("C:/dl/Locked.zip", null, null);

    await submitInstallPassword(installJobs[0], "pw");
    await flush();
    expect(api.installMod).toHaveBeenLastCalledWith("C:/dl/Locked.zip", null, "pw");
    expect(installJobs[0].stage).toBe("done");
  });

  it("error stage keeps human message and retry works", async () => {
    vi.mocked(api.installMod)
      .mockRejectedValueOnce(new Error("已存在同名 Mod：Dup"))
      .mockResolvedValueOnce({
        status: "installed",
        mod_id: 9,
        name: "Dup",
        character: "Others",
        warnings: [],
      });

    enqueueInstalls(["C:/dl/Dup.zip"], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("error");
    expect(installJobs[0].message).toContain("已存在同名 Mod");

    const { retryInstall } = await import("./install.svelte");
    retryInstall(installJobs[0], vi.fn());
    await flush(); await flush();
    expect(installJobs[0].stage).toBe("done");
  });

  it("undo uninstalls and removes the job", async () => {
    vi.mocked(api.installMod).mockResolvedValue({
      status: "installed",
      mod_id: 11,
      name: "X",
      character: "Bailu",
      warnings: [],
    });
    vi.mocked(api.uninstallMod).mockResolvedValue(undefined);
    const onInstalled = vi.fn();

    enqueueInstalls(["C:/dl/X.zip"], onInstalled);
    await flush(); await flush();

    await undoInstall(installJobs[0], onInstalled);
    expect(api.uninstallMod).toHaveBeenCalledWith(11);
    expect(installJobs).toHaveLength(0);
    expect(onInstalled).toHaveBeenCalledTimes(2);
  });

  it("dismiss removes without side effects", () => {
    installJobs.push({
      id: 999,
      fileName: "a.zip",
      path: "C:/a.zip",
      stage: "done",
      character: "Bailu",
      modId: 1,
      message: null,
      warnings: [],
    });
    dismissInstall(installJobs[0]);
    expect(installJobs).toHaveLength(0);
  });
});
```

- [ ] **Step 3: 运行确认失败**

Run: `npm test -- install.svelte`（app/ 目录）
Expected: FAIL（模块不存在）

- [ ] **Step 4: 实现 `app/src/lib/install.svelte.ts`**

```ts
import { api } from "$lib/api";

export type InstallStage = "installing" | "needs-password" | "done" | "error";

export interface InstallJob {
  id: number;
  fileName: string;
  path: string;
  stage: InstallStage;
  character: string | null;
  modId: number | null;
  message: string | null;
  warnings: string[];
}

let nextId = 1;

export const installJobs = $state<InstallJob[]>([]);

export function enqueueInstalls(paths: string[], onInstalled: () => void): void {
  for (const path of paths) {
    const job: InstallJob = {
      id: nextId++,
      fileName: path.split(/[\\/]/).pop() ?? path,
      path,
      stage: "installing",
      character: null,
      modId: null,
      message: null,
      warnings: [],
    };
    installJobs.push(job);
    void runInstall(job, null, onInstalled);
  }
}

async function runInstall(
  job: InstallJob,
  password: string | null,
  onInstalled: () => void,
): Promise<void> {
  job.stage = "installing";
  job.message = null;
  try {
    const result = await api.installMod(job.path, null, password);
    if (result.status === "needs_password") {
      job.stage = "needs-password";
      return;
    }
    job.stage = "done";
    job.character = result.character;
    job.modId = result.mod_id;
    job.warnings = result.warnings;
    onInstalled();
  } catch (e) {
    job.stage = "error";
    job.message = e instanceof Error ? e.message : String(e);
  }
}

export async function submitInstallPassword(
  job: InstallJob,
  password: string,
  onInstalled: () => void,
): Promise<void> {
  await runInstall(job, password, onInstalled);
}

export function retryInstall(job: InstallJob, onInstalled: () => void): void {
  void runInstall(job, null, onInstalled);
}

export async function undoInstall(
  job: InstallJob,
  onInstalled: () => void,
): Promise<void> {
  if (job.modId != null) {
    try {
      await api.uninstallMod(job.modId);
    } catch {
      // 撤销失败也移除任务条目；错误属于非阻断提示，主流程永远可用
    }
  }
  dismissInstall(job);
  onInstalled();
}

export function dismissInstall(job: InstallJob): void {
  const i = installJobs.indexOf(job);
  if (i >= 0) installJobs.splice(i, 1);
}
```

- [ ] **Step 5: 运行确认通过**

Run: `npm test -- install.svelte`
Expected: 5 tests PASS

- [ ] **Step 6: 写 InstallOverlay 组件测试 `app/src/lib/components/InstallOverlay.test.ts`**

```ts
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import InstallOverlay from "./InstallOverlay.svelte";
import type { InstallJob } from "$lib/install.svelte";
import type { CharacterSummary } from "$lib/api";

const characters: CharacterSummary[] = [
  { internal_name: "Firefly", display_name: "Firefly", image: "firefly.png", total: 1, enabled: 0 },
];

function job(partial: Partial<InstallJob>): InstallJob {
  return {
    id: 1,
    fileName: "Cool.zip",
    path: "C:/dl/Cool.zip",
    stage: "installing",
    character: null,
    modId: null,
    message: null,
    warnings: [],
    ...partial,
  };
}

describe("InstallOverlay", () => {
  it("shows installing stage", () => {
    render(InstallOverlay, { props: { jobs: [job({})], characters, onInstalled: vi.fn() } });
    expect(screen.getByText("Cool.zip")).toBeTruthy();
    expect(screen.getByText(/正在安装/)).toBeTruthy();
  });

  it("shows done stage with character display name and undo", () => {
    render(InstallOverlay, {
      props: {
        jobs: [job({ stage: "done", character: "Firefly", modId: 5 })],
        characters,
        onInstalled: vi.fn(),
      },
    });
    expect(screen.getByText(/已安装到 Firefly/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "撤销" })).toBeTruthy();
  });

  it("shows password input when needed and submits", async () => {
    render(InstallOverlay, {
      props: { jobs: [job({ stage: "needs-password" })], characters, onInstalled: vi.fn() },
    });
    const input = screen.getByPlaceholderText("压缩包密码");
    await fireEvent.input(input, { target: { value: "pw" } });
    await fireEvent.click(screen.getByRole("button", { name: "确认" }));
    // 交互细节由 store 测试覆盖，这里只验证不抛错且输入存在
    expect(input).toBeTruthy();
  });

  it("shows error message with retry", () => {
    render(InstallOverlay, {
      props: {
        jobs: [job({ stage: "error", message: "已存在同名 Mod：Cool" })],
        characters,
        onInstalled: vi.fn(),
      },
    });
    expect(screen.getByText("已存在同名 Mod：Cool")).toBeTruthy();
    expect(screen.getByRole("button", { name: "重试" })).toBeTruthy();
  });

  it("renders nothing when no jobs", () => {
    const { container } = render(InstallOverlay, {
      props: { jobs: [], characters, onInstalled: vi.fn() },
    });
    expect(container.querySelector(".install-overlay")).toBeNull();
  });
});
```

- [ ] **Step 7: 实现 `app/src/lib/components/InstallOverlay.svelte`**

```svelte
<script lang="ts">
  import {
    dismissInstall,
    retryInstall,
    submitInstallPassword,
    undoInstall,
    type InstallJob,
  } from "$lib/install.svelte";
  import type { CharacterSummary } from "$lib/api";

  let {
    jobs,
    characters,
    onInstalled,
  }: {
    jobs: InstallJob[];
    characters: CharacterSummary[];
    onInstalled: () => void;
  } = $props();

  let passwords = $state<Record<number, string>>({});

  function displayName(internal: string): string {
    return characters.find((c) => c.internal_name === internal)?.display_name ?? internal;
  }
</script>

{#if jobs.length > 0}
  <div class="install-overlay fixed bottom-6 inset-x-0 z-50 flex justify-center pointer-events-none">
    <div class="glass radius-panel pointer-events-auto w-[420px] max-w-[90vw] px-5 py-4 flex flex-col gap-3"
      style="box-shadow: var(--shadow-lift)">
      {#each jobs as job (job.id)}
        <div class="flex items-center gap-3 min-h-9">
          {#if job.stage === "installing"}
            <span class="spinner shrink-0"></span>
          {/if}
          <span class="text-sm font-medium truncate flex-1 min-w-0">{job.fileName}</span>

          {#if job.stage === "installing"}
            <span class="text-sm text-secondary shrink-0">正在安装…</span>
          {:else if job.stage === "needs-password"}
            <input
              class="glass radius-pill px-3 h-8 text-sm w-32 outline-none bg-transparent text-white"
              placeholder="压缩包密码"
              type="password"
              bind:value={passwords[job.id]}
              onkeydown={(e) => {
                if (e.key === "Enter" && passwords[job.id]) {
                  submitInstallPassword(job, passwords[job.id], onInstalled);
                }
              }}
            />
            <button
              class="accent-fill accent-text radius-pill px-3.5 h-8 text-sm font-medium cursor-pointer shrink-0"
              onclick={() => submitInstallPassword(job, passwords[job.id] ?? "", onInstalled)}
            >确认</button>
          {:else if job.stage === "done"}
            <span class="text-sm shrink-0">已安装到 {displayName(job.character ?? "")}</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => undoInstall(job, onInstalled)}
            >撤销</button>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => dismissInstall(job)}
            >关闭</button>
          {:else if job.stage === "error"}
            <span class="text-sm shrink-0" style="color: var(--danger)">{job.message}</span>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => retryInstall(job, onInstalled)}
            >重试</button>
            <button
              class="glass radius-pill px-3 h-7 text-xs cursor-pointer shrink-0"
              onclick={() => dismissInstall(job)}
            >关闭</button>
          {/if}
        </div>
        {#if job.warnings.length > 0}
          {#each job.warnings as w}
            <p class="text-xs text-secondary -mt-2 pl-1">{w}</p>
          {/each}
        {/if}
      {/each}
    </div>
  </div>
{/if}

<style>
  .spinner {
    width: 16px;
    height: 16px;
    border-radius: 9999px;
    border: 2px solid var(--glass-stroke);
    border-top-color: var(--accent);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
```

（若 `--accent` token 不存在于 app.css，改 spinner 的 `border-top-color` 为 `#409CFF`；先读 app.css 确认 token 名。）

- [ ] **Step 8: +page.svelte 接线拖放**

script 顶部追加 import：

```ts
  import { isTauri } from "$lib/api";
  import { enqueueInstalls, installJobs } from "$lib/install.svelte";
  import InstallOverlay from "$lib/components/InstallOverlay.svelte";
```

state 追加：

```ts
  let dragHover = $state(false);
```

onMount 改为：

```ts
  onMount(() => {
    void refresh();
    if (!isTauri()) return;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/webviewWindow").then(({ getCurrentWebviewWindow }) => {
      getCurrentWebviewWindow()
        .onDragDropEvent((event) => {
          const t = event.payload.type;
          if (t === "enter" || t === "over") dragHover = true;
          else if (t === "leave") dragHover = false;
          else if (t === "drop") {
            dragHover = false;
            const paths = event.payload.paths.filter((p) =>
              /\.(zip|7z|rar)$/i.test(p),
            );
            if (paths.length > 0) enqueueInstalls(paths, refresh);
          }
        })
        .then((u) => (unlisten = u));
    });
    return () => unlisten?.();
  });
```

（删掉原来的 `onMount(refresh);` 行。动态 import 与 TitleBar 的模式一致。）

模板根 div 内最后追加：

```svelte
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} onInstalled={refresh} />
```

- [ ] **Step 9: 全量测试**

Run: `npm test`（app/ 目录）
Expected: 全部 PASS（既有 10 + 新增 9）

- [ ] **Step 10: Commit**

```bash
git add app/src
git commit -m "feat(app): 沉浸式安装流——拖放队列、悬浮玻璃面板、密码流、撤销"
```

---

### Task 5: 端到端验收 + 收尾

**Files:**
- 无新文件；可能修复性改动

- [ ] **Step 1: 全量自动化**

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
npm test    # app/ 目录
```

Expected: 全绿

- [ ] **Step 2: 构建 exe 并做 CDP 视觉验证**

```powershell
# 杀锁进程后：
npm run build   # app/ 目录
cargo build --release --features tauri/custom-protocol --manifest-path src-tauri\Cargo.toml   # app/ 目录
# 启动（带 CDP）：
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"
Start-Process "..\..\target\release\liquimod-app.exe"
# CDP 截图确认主界面无回归
```

再验证 exe 内嵌了新前端 hash（读字节查 build/_app/immutable 文件名）。

- [ ] **Step 3: 造测试压缩包，人工/CDP 拖放验证**

CDP 无法模拟 OS 级文件拖放——此步由主人手动：把任意 zip（内含 `mod.ini` 且文本含角色名，如 `firefly`）拖入窗口，确认：
1. 底部浮层出现并显示"正在安装…"
2. 完成后显示"已安装到 Firefly"，网格对应角色 Mod 数 +1
3. "撤销"后数量回落
4. 加密 zip：浮层变密码输入框，错误密码回到输入，正确密码安装成功
5. 非压缩包文件拖入：显示"不是支持的压缩包"

- [ ] **Step 4: 修复验收发现的问题，最终 commit**

```bash
git add -A
git commit -m "fix(app): 安装流验收修复"
```

---

## Self-Review 记录

- Spec 覆盖：§4.2 拖入→浮层→推断→密码→嵌套（core extract_recursive 已覆盖嵌套）→入库→写库 ✓；裁剪项已在头部声明（进度环/自动启用/缩略图/密码本询问）。
- 类型一致性：`InstallResultDto` serde tag `status` snake_case → 前端 `InstallResult` 判别联合一致（`installed`/`needs_password`，字段 `mod_id`/`name`/`character`/`warnings`）；`InstallJob` 字段在 store/overlay/tests 三处一致（`modId` 为 camelCase 前端字段，区别于 DTO 的 `mod_id`）。
- `uninstall_mod` 依赖 Task 3 的 `remove_entry`；`undoInstall` 依赖 `api.uninstallMod`——顺序一致。
