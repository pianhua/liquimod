# LiquiMod 里程碑 6「预设 + 设置页 + 缩略图 + 打磨」Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 启用清单预设（快照/一键全量应用）、设置页（目录 + 密码本 + 打开 Library）、Mod 缩略图预览、CSP 等打磨。

**Architecture:** core 新增 db presets 表 + `preset.rs` 应用逻辑 + `thumbs.rs` 预览图缩略（image crate，缓存 JPEG，无 DB 迁移）；app 新增预设/密码本命令、ModDto 携带 data-URL 缩略图、注册 opener 插件；前端主模型已写定的 PresetMenu 弹层、Settings 视图、TitleBar 齿轮。预设应用走既有 Deployer 逐条 enable/disable，watcher 对账天然无副作用（DB 先行，差集为 0）。

**Tech Stack:** Rust（rusqlite 0.32 / image 0.25 / base64 0.22）、Tauri 2、Svelte 5（$state rune）。

---

## 既有事实（执行者不必再探索）

- `crates/liquimod-core/src/db.rs`：`Database { conn: Connection }`，`open/open_in_memory` → `init(conn)` 内 `execute_batch` 建表（mods/op_log/passwords），`now_unix() -> i64` 已存在，tests 在 `#[cfg(test)] mod tests`（行 175 起），用 `Database::open_in_memory()`。
- `crates/liquimod-core/src/models.rs`：`ModEntry { id, character, name, rel_path, enabled, installed_at }`。
- `crates/liquimod-core/src/library.rs`：`Library { layout: LibraryLayout, db: Database }`，`list()`；`layout.mod_dir(character, name) -> PathBuf`（绝对路径），`layout.root` 为库根。
- `crates/liquimod-core/src/deploy.rs`：`Deployer::new(&lib, mods_dir)`，`enable(id)/disable(id)` 幂等（junction + DB），返回 `Result<()>`。
- `crates/liquimod-core/src/lib.rs`：`pub mod` 列表在此登记。
- `crates/liquimod-core/src/error.rs`：`Result<T>`、`LiquiModError`。
- app 命令模式（`app/src-tauri/src/commands.rs`）：读命令 `spawn_blocking` + `library.lock().unwrap()`；写命令成功后在**锁外** `maybe_refresh_game(&app2, &refresh)`（`refresh = Arc::clone(&state.refresh)`）。`set_enabled(&lib, mods_dir.as_deref(), id, enabled)` 返回 `Result<(), String>`。
- `app/src-tauri/src/lib.rs`：`invoke_handler` 在行 86-94 登记命令；builder 链可插 `.plugin(...)`。
- `app/src-tauri/Cargo.toml` 已有 `tauri-plugin-opener = "2"` 但**未注册**（本里程碑注册后使用）。
- 前端：`api.ts` 的 `call<T>` mock 层（非 Tauri 环境返回 mock），`api` 对象集中导出方法；`toast(msg)` 来自 `$lib/toast.svelte`；CSS 工具类 `glass / radius-pill / radius-card / radius-panel / accent-fill / accent-text / text-secondary` 全局可用；`+page.svelte` 的 `refresh()` 重拉 config+characters。
- `choose_mods_dir` 命令已存在（`api.chooseModsDir(path)`），dialog 用 `import { open } from "@tauri-apps/plugin-dialog"`。
- 测试命令：`cargo test --workspace`、`cargo clippy --workspace --all-targets`、`cargo fmt --all`；`cd app; npm test; npm run check`。
- 提交风格：`feat(core): …` / `fix(app): …` 中文描述。

## 裁剪声明（不做，勿扩展）

- 预设无重命名/排序/导入导出；同名保存即覆盖。
- 缩略图不进 DB（确定性缓存路径 `thumbs/{id}.jpg`，源文件更新则重生）；无手动设置封面。
- 设置页无主题切换（跟随系统已是设计）。
- 不做打包安装器。

---

### Task 1: core 预设存储（db.rs + models.rs）

**Files:**
- Modify: `crates/liquimod-core/src/models.rs`
- Modify: `crates/liquimod-core/src/db.rs`

- [ ] **Step 1: models.rs 追加 Preset**

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}
```

- [ ] **Step 2: 写失败测试**（追加到 db.rs 的 `mod tests`）

```rust
    #[test]
    fn preset_roundtrip_and_overwrite() {
        let db = Database::open_in_memory().unwrap();
        let a = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let b = db.upsert_mod("Asta", "m2", "mods/Asta/m2").unwrap();
        let id1 = db.save_preset("日常", &[a, b]).unwrap();
        assert_eq!(db.preset_mod_ids(id1).unwrap(), vec![a, b]);
        // 同名覆盖：条目整体替换，id 复用
        let id2 = db.save_preset("日常", &[b]).unwrap();
        assert_eq!(id1, id2);
        assert_eq!(db.preset_mod_ids(id1).unwrap(), vec![b]);
        let list = db.list_presets().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "日常");
    }

    #[test]
    fn preset_delete_cascades_entries() {
        let db = Database::open_in_memory().unwrap();
        let a = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let id = db.save_preset("x", &[a]).unwrap();
        db.delete_preset(id).unwrap();
        assert!(db.list_presets().unwrap().is_empty());
        assert!(db.preset_mod_ids(id).unwrap().is_empty());
    }

    #[test]
    fn preset_rejects_empty_name() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.save_preset("  ", &[]).is_err());
    }
```

Run: `cargo test -p liquimod-core preset` — Expected: FAIL（方法不存在）

- [ ] **Step 3: db.rs 实现**

`init` 的 `execute_batch` SQL 字符串末尾追加（在 passwords 表之后）：

```sql
             CREATE TABLE IF NOT EXISTS presets (
               id INTEGER PRIMARY KEY,
               name TEXT NOT NULL UNIQUE,
               created_at INTEGER NOT NULL
             );
             CREATE TABLE IF NOT EXISTS preset_entries (
               preset_id INTEGER NOT NULL REFERENCES presets(id) ON DELETE CASCADE,
               mod_id INTEGER NOT NULL,
               PRIMARY KEY (preset_id, mod_id)
             );
```

`init` 函数体 `execute_batch(...)?;` 之后加：

```rust
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
```

`impl Database` 内追加：

```rust
    /// 同名覆盖：条目整体替换并复用 id。
    pub fn save_preset(&self, name: &str, mod_ids: &[i64]) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LiquiModError::InvalidName("预设名不能为空".into()));
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO presets (name, created_at) VALUES (?1, ?2)
             ON CONFLICT(name) DO UPDATE SET created_at = excluded.created_at",
            rusqlite::params![name, now_unix()],
        )?;
        let id: i64 = tx.query_row(
            "SELECT id FROM presets WHERE name = ?1",
            [name],
            |r| r.get(0),
        )?;
        tx.execute("DELETE FROM preset_entries WHERE preset_id = ?1", [id])?;
        for mid in mod_ids {
            tx.execute(
                "INSERT OR IGNORE INTO preset_entries (preset_id, mod_id) VALUES (?1, ?2)",
                rusqlite::params![id, mid],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }

    pub fn list_presets(&self) -> Result<Vec<Preset>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, created_at FROM presets ORDER BY created_at DESC, id DESC")?;
        let rows = stmt.query_map([], |r| {
            Ok(Preset {
                id: r.get(0)?,
                name: r.get(1)?,
                created_at: r.get(2)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn preset_mod_ids(&self, preset_id: i64) -> Result<Vec<i64>> {
        let mut stmt = self
            .conn
            .prepare("SELECT mod_id FROM preset_entries WHERE preset_id = ?1")?;
        let rows = stmt.query_map([preset_id], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn delete_preset(&self, preset_id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM presets WHERE id = ?1", [preset_id])?;
        Ok(())
    }
```

db.rs 头部 `use crate::models::ModEntry;` 改为 `use crate::models::{ModEntry, Preset};`

Run: `cargo test -p liquimod-core preset` — Expected: 3 passed

- [ ] **Step 4: 全量回归 + 提交**

Run: `cargo test --workspace; cargo clippy --workspace --all-targets; cargo fmt --all`
Expected: 全绿

```bash
git add crates/liquimod-core/src/{models.rs,db.rs}
git commit -m "feat(core): presets/preset_entries 表与同名覆盖式存储"
```

---

### Task 2: core 预设应用逻辑（preset.rs）

**Files:**
- Create: `crates/liquimod-core/src/preset.rs`
- Modify: `crates/liquimod-core/src/lib.rs`

- [ ] **Step 1: 写失败测试**（放 preset.rs 内 `#[cfg(test)] mod tests`，实现先行写下、测试先行运行）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Library, i64, i64, i64) {
        let lib_dir = tempfile::tempdir().unwrap();
        let mods_dir = tempfile::tempdir().unwrap();
        let lib = Library::init(lib_dir.path()).unwrap();
        lib.add_folder(
            &{
                let d = mods_dir.path().join("src1");
                std::fs::create_dir_all(&d).unwrap();
                d
            },
            "Asta",
            "m1",
        )
        .unwrap();
        // 更直接：用 upsert + 真实目录
        let root = lib.layout.mods_root();
        std::fs::create_dir_all(root.join("Asta/m2")).unwrap();
        std::fs::create_dir_all(root.join("Asta/m3")).unwrap();
        let a = lib.db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let b = lib.db.upsert_mod("Asta", "m2", "mods/Asta/m2").unwrap();
        let c = lib.db.upsert_mod("Asta", "m3", "mods/Asta/m3").unwrap();
        (lib_dir, mods_dir, lib, a, b, c)
    }

    #[test]
    fn snapshot_returns_enabled_ids() {
        let (_l, mdir, lib, a, b, _c) = fixture();
        let dep = Deployer::new(&lib, mdir.path());
        dep.enable(a).unwrap();
        dep.enable(b).unwrap();
        let ids = snapshot_enabled(&lib).unwrap();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn apply_enables_listed_and_disables_rest() {
        let (_l, mdir, lib, a, b, c) = fixture();
        let dep = Deployer::new(&lib, mdir.path());
        dep.enable(a).unwrap();
        dep.enable(b).unwrap();
        dep.enable(c).unwrap();
        let pid = lib.db.save_preset("p", &[b]).unwrap();
        let (en, dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!((en, dis), (0, 2)); // b 已启用，a/c 被停用
        assert!(!lib.db.get_mod(a).unwrap().enabled);
        assert!(lib.db.get_mod(b).unwrap().enabled);
        assert!(!lib.db.get_mod(c).unwrap().enabled);
        // 幂等：再应用一次零变更
        assert_eq!(apply_preset(&lib, mdir.path(), pid).unwrap(), (0, 0));
    }

    #[test]
    fn apply_ignores_stale_mod_ids() {
        let (_l, mdir, lib, a, _b, _c) = fixture();
        let pid = lib.db.save_preset("p", &[a, 99999]).unwrap();
        let (en, _dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!(en, 1);
    }
}
```

注：fixture 中 `add_folder` 只为建 m1 目录，若 m1 目录已由它建好则 upsert 幂等。若 `add_folder` 需要真实源目录外的校验失败，可直接 `std::fs::create_dir_all(root.join("Asta/m1"))` 替代。

Run: `cargo test -p liquimod-core --lib preset` — Expected: FAIL（模块不存在）

- [ ] **Step 2: 实现 preset.rs**

```rust
//! 预设 = 启用清单快照；应用 = 全量对账重建 Junction。

use crate::deploy::Deployer;
use crate::error::Result;
use crate::library::Library;
use std::path::Path;

/// 当前启用的 mod id 列表（按 id 升序，便于测试断言）。
pub fn snapshot_enabled(lib: &Library) -> Result<Vec<i64>> {
    let mut ids: Vec<i64> = lib.list()?.into_iter().filter(|m| m.enabled).map(|m| m.id).collect();
    ids.sort_unstable();
    Ok(ids)
}

/// 应用预设：启用清单内、停用清单外。返回 (启用数, 停用数)。清单内不存在的 id 静默忽略。
pub fn apply_preset(lib: &Library, mods_dir: &Path, preset_id: i64) -> Result<(usize, usize)> {
    let want: std::collections::HashSet<i64> =
        lib.db.preset_mod_ids(preset_id)?.into_iter().collect();
    let dep = Deployer::new(lib, mods_dir);
    let (mut enabled, mut disabled) = (0usize, 0usize);
    for m in lib.list()? {
        if want.contains(&m.id) && !m.enabled {
            dep.enable(m.id)?;
            enabled += 1;
        } else if !want.contains(&m.id) && m.enabled {
            dep.disable(m.id)?;
            disabled += 1;
        }
    }
    Ok((enabled, disabled))
}
```

lib.rs 加 `pub mod preset;`（按字母序插入现有 mod 列表）。

Run: `cargo test -p liquimod-core --lib preset` — Expected: 3 passed

- [ ] **Step 3: 回归 + 提交**

Run: `cargo test --workspace; cargo clippy --workspace --all-targets; cargo fmt --all`

```bash
git add crates/liquimod-core/src/{preset.rs,lib.rs}
git commit -m "feat(core): 预设快照与全量应用（幂等，忽略失效 id）"
```

---

### Task 3: core Mod 缩略图（thumbs.rs）

**Files:**
- Create: `crates/liquimod-core/src/thumbs.rs`
- Modify: `crates/liquimod-core/src/lib.rs`、`crates/liquimod-core/Cargo.toml`

设计：在 mod 目录内（递归最深 2 层）找预览图——优先文件名 stem 为 `preview`（不区分大小写），否则按路径字典序取第一张 `.png/.jpg/.jpeg/.webp`；缩放至最长边 384，JPEG q80 缓存到 `{library_root}/thumbs/{mod_id}.jpg`；源文件 mtime 新于缓存则重生。不入 DB。

- [ ] **Step 1: Cargo.toml 加依赖**

```toml
image = { version = "0.25", default-features = false, features = ["png", "jpeg", "webp"] }
```

- [ ] **Step 2: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_preview_stem_first() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.save(dir.path().join("aaa.png")).unwrap();
        img.save(dir.path().join("Preview.PNG")).unwrap();
        let found = find_preview_image(dir.path()).unwrap();
        assert_eq!(found.file_stem().unwrap().to_string_lossy(), "Preview");
    }

    #[test]
    fn falls_back_to_first_image_and_searches_subdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("textures")).unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 255, 0, 255]));
        img.save(dir.path().join("textures/b.png")).unwrap();
        let found = find_preview_image(dir.path()).unwrap();
        assert!(found.ends_with("b.png"));
    }

    #[test]
    fn no_image_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mod.ini"), "x").unwrap();
        assert!(find_preview_image(dir.path()).is_none());
    }

    #[test]
    fn ensure_thumbnail_caches_and_regenerates_when_source_newer() {
        let lib_dir = tempfile::tempdir().unwrap();
        let mod_dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(900, 300, image::Rgba([1, 2, 3, 255]));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t1 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7).unwrap().unwrap();
        assert!(t1.ends_with("7.jpg"));
        let thumb = image::open(&t1).unwrap();
        assert_eq!(thumb.width(), 384);
        let mtime1 = std::fs::metadata(&t1).unwrap().modified().unwrap();
        // 缓存新鲜：直接命中
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let t2 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7).unwrap().unwrap();
        let mtime2 = std::fs::metadata(&t2).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
        // 源更新：重生
        std::thread::sleep(std::time::Duration::from_millis(1100));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t3 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7).unwrap().unwrap();
        let mtime3 = std::fs::metadata(&t3).unwrap().modified().unwrap();
        assert!(mtime3 > mtime2);
    }
}
```

（tempfile 已在 dev-dependencies。）Run: `cargo test -p liquimod-core --lib thumbs` — Expected: FAIL

- [ ] **Step 3: 实现 thumbs.rs**

```rust
//! Mod 预览图缩略：确定性缓存路径 thumbs/{id}.jpg，源新则重生。

use std::path::{Path, PathBuf};

const IMAGE_EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const THUMB_LONG_EDGE: u32 = 384;

/// 在 mod 目录内（最深 2 层）找预览图：preview stem 优先，否则字典序第一张。
pub fn find_preview_image(mod_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    collect_images(mod_dir, 0, &mut candidates);
    candidates.sort();
    candidates
        .iter()
        .find(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().eq_ignore_ascii_case("preview"))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

fn collect_images(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 1 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() && !ft.is_symlink() {
            collect_images(&path, depth + 1, out);
        } else if ft.is_file()
            && path
                .extension()
                .map(|e| {
                    let e = e.to_string_lossy().to_lowercase();
                    IMAGE_EXTS.contains(&e.as_str())
                })
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// 生成/复用缩略图。无预览图或解码失败返回 Ok(None)。mtime 比较失败时保守重生。
pub fn ensure_thumbnail(
    library_root: &Path,
    mod_dir: &Path,
    mod_id: i64,
) -> crate::error::Result<Option<PathBuf>> {
    let Some(src) = find_preview_image(mod_dir) else {
        return Ok(None);
    };
    let thumb_dir = library_root.join("thumbs");
    let dest = thumb_dir.join(format!("{mod_id}.jpg"));
    let fresh = match (
        std::fs::metadata(&src).and_then(|m| m.modified()),
        std::fs::metadata(&dest).and_then(|m| m.modified()),
    ) {
        (Ok(s), Ok(d)) => d >= s,
        (Ok(_), Err(_)) => false,
        _ => false,
    };
    if fresh {
        return Ok(Some(dest));
    }
    std::fs::create_dir_all(&thumb_dir)?;
    let img = match image::open(&src) {
        Ok(i) => i,
        Err(_) => return Ok(None), // 损坏图片不阻断列表
    };
    let thumb = img.thumbnail(THUMB_LONG_EDGE, THUMB_LONG_EDGE);
    thumb
        .save_with_format(&dest, image::ImageFormat::Jpeg)
        .map_err(std::io::Error::other)?;
    Ok(Some(dest))
}
```

lib.rs 加 `pub mod thumbs;`。

Run: `cargo test -p liquimod-core --lib thumbs` — Expected: 4 passed（mtime 粒度若导致缓存断言抖动，把 sleep 调大到 1100ms 已覆盖 Windows 的常见粒度）

- [ ] **Step 4: 回归 + 提交**

Run: `cargo test --workspace; cargo clippy --workspace --all-targets; cargo fmt --all`

```bash
git add crates/liquimod-core
git commit -m "feat(core): Mod 预览图缩略缓存（preview 优先，源新重生）"
```

---

### Task 4: app 预设/密码本命令 + ModDto 缩略图 + opener 注册

**Files:**
- Modify: `app/src-tauri/src/commands.rs`、`app/src-tauri/src/lib.rs`、`app/src-tauri/Cargo.toml`

- [ ] **Step 1: Cargo.toml 加 `base64 = "0.22"`**

- [ ] **Step 2: 写失败测试**（commands.rs `mod tests` 追加；参考既有 tests 的 fixture 风格——用 `Library::init(tempdir)` + `AppState` 不可构造时只测纯函数）

```rust
    #[test]
    fn preset_dto_roundtrip() {
        let db = liquimod_core::db::Database::open_in_memory().unwrap();
        let m = db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let id = crate::commands::save_preset_named(&db, "日常", &[m]).unwrap();
        let list = crate::commands::preset_dtos(&db).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
        assert_eq!(list[0].name, "日常");
    }

    #[test]
    fn apply_preset_requires_mods_dir() {
        let dir = tempfile::tempdir().unwrap();
        let lib = liquimod_core::library::Library::init(dir.path()).unwrap();
        let pid = lib.db.save_preset("p", &[]).unwrap();
        assert!(crate::commands::apply_preset_by_id(&lib, None, pid).is_err());
    }

    #[test]
    fn thumb_data_url_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(crate::commands::thumb_data_url(dir.path(), 42).is_none());
    }
```

（tempfile 需在 app/src-tauri `[dev-dependencies]`，若已有则跳过。）Run: `cargo test -p liquimod-app` — Expected: FAIL

- [ ] **Step 3: 实现 commands.rs 纯函数 + 命令**

文件头部 import 区追加：

```rust
use base64::Engine;
```

DTO 定义（放在 ConfigDto 附近）：

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PresetDto {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ApplyResultDto {
    pub enabled: usize,
    pub disabled: usize,
}
```

ModDto 加字段（同时更新既有构造点 `mod_list`）：

```rust
pub struct ModDto {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub installed_at: i64,
    pub thumb: Option<String>,
}
```

纯函数：

```rust
pub fn save_preset_named(
    db: &liquimod_core::db::Database,
    name: &str,
    mod_ids: &[i64],
) -> Result<i64, String> {
    db.save_preset(name, mod_ids).map_err(|e| e.to_string())
}

pub fn preset_dtos(db: &liquimod_core::db::Database) -> Result<Vec<PresetDto>, String> {
    db.list_presets()
        .map_err(|e| e.to_string())
        .map(|ps| {
            ps.into_iter()
                .map(|p| PresetDto {
                    id: p.id,
                    name: p.name,
                    created_at: p.created_at,
                })
                .collect()
        })
}

pub fn apply_preset_by_id(
    lib: &Library,
    mods_dir: Option<&Path>,
    preset_id: i64,
) -> Result<ApplyResultDto, String> {
    let mods_dir = mods_dir.ok_or_else(|| "未配置 3Dmigoto Mods 目录，无法应用预设".to_string())?;
    let (enabled, disabled) = liquimod_core::preset::apply_preset(lib, mods_dir, preset_id)
        .map_err(|e| e.to_string())?;
    Ok(ApplyResultDto { enabled, disabled })
}

/// 缩略图 data URL；缓存未生成时现场生成，失败静默为 None（不阻断列表）。
pub fn thumb_data_url(library_root: &Path, mod_dir: &Path, mod_id: i64) -> Option<String> {
    let path =
        liquimod_core::thumbs::ensure_thumbnail(library_root, mod_dir, mod_id).ok().flatten()?;
    let bytes = std::fs::read(path).ok()?;
    Some(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}
```

`thumb_data_url` 测试签名用了两参版本——以三参为准，测试改为：

```rust
    #[test]
    fn thumb_data_url_missing_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(crate::commands::thumb_data_url(dir.path(), dir.path(), 42).is_none());
    }
```

`mod_list` 修改：map 闭包内为每个 mod 调 `thumb_data_url(&lib.layout.root, &lib.layout.mod_dir(&m.character, &m.name), m.id)`。注意闭包先消耗 `m.character/m.name` 计算 mod_dir 再 move 字段，或先 clone。参考：

```rust
pub fn mod_list(lib: &Library, character: &str) -> Result<Vec<ModDto>, String> {
    let root = lib.layout.root.clone();
    let mut mods: Vec<ModDto> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| m.character == character)
        .map(|m| {
            let dir = lib.layout.mod_dir(&m.character, &m.name);
            let thumb = thumb_data_url(&root, &dir, m.id);
            ModDto {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                thumb,
            }
        })
        .collect();
    mods.sort_by_key(|m| m.installed_at);
    Ok(mods)
}
```

（若既有实现已有排序/其他字段，保持既有行为，仅加 thumb。）

Tauri 命令（追加，跟随既有模式）：

```rust
#[tauri::command]
pub async fn list_presets(state: tauri::State<'_, AppState>) -> Result<Vec<PresetDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        preset_dtos(&lib.db)
    })
    .await
    .map_err(|e| format!("读取预设失败：{e}"))?
}

#[tauri::command]
pub async fn save_preset(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<PresetDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let ids = liquimod_core::preset::snapshot_enabled(&lib).map_err(|e| e.to_string())?;
        let id = save_preset_named(&lib.db, &name, &ids)?;
        preset_dtos(&lib.db)?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| "预设保存后读取失败".to_string())
    })
    .await
    .map_err(|e| format!("保存预设失败：{e}"))?
}

#[tauri::command]
pub async fn apply_preset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<ApplyResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = apply_preset_by_id(&lib, mods_dir.as_deref(), id);
        if let Ok(r) = &result {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            let _ = app2.emit(
                "library-changed",
                serde_json::json!({ "added": r.enabled, "removed": r.disabled }),
            );
            let _ = app2.emit(
                "liquimod-toast",
                format!("已应用预设「{name}」：启用 {} / 停用 {}", r.enabled, r.disabled),
            );
            maybe_refresh_game(&app2, &refresh);
        }
        result
    })
    .await
    .map_err(|e| format!("应用预设失败：{e}"))?
}
```

注意：`library-changed` 事件会让前端 toast 一次「检测到仓库变动」——这会造成双 toast。因此 apply_preset **不 emit library-changed**，只 emit liquimod-toast 并返回结果由前端自行 refresh。删去上面那段 library-changed emit，最终版：

```rust
#[tauri::command]
pub async fn apply_preset(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<ApplyResultDto, String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        let result = apply_preset_by_id(&lib, mods_dir.as_deref(), id);
        if let Ok(r) = &result {
            drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
            let _ = app2.emit(
                "liquimod-toast",
                format!("已应用预设「{name}」：启用 {} / 停用 {}", r.enabled, r.disabled),
            );
            maybe_refresh_game(&app2, &refresh);
        }
        result
    })
    .await
    .map_err(|e| format!("应用预设失败：{e}"))?
}
```

```rust
#[tauri::command]
pub async fn delete_preset(state: tauri::State<'_, AppState>, id: i64) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.delete_preset(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除预设失败：{e}"))?
}

#[tauri::command]
pub async fn list_passwords(state: tauri::State<'_, AppState>) -> Result<Vec<String>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.list_passwords().map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("读取密码本失败：{e}"))?
}

#[tauri::command]
pub async fn add_password(state: tauri::State<'_, AppState>, value: String) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let v = value.trim().to_string();
        if v.is_empty() {
            return Err("密码不能为空".to_string());
        }
        let lib = library.lock().unwrap();
        lib.db.add_password(&v).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("添加密码失败：{e}"))?
}

#[tauri::command]
pub async fn remove_password(
    state: tauri::State<'_, AppState>,
    value: String,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.remove_password(&value).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移除密码失败：{e}"))?
}
```

lib.rs `generate_handler!` 追加：`commands::list_presets, commands::save_preset, commands::apply_preset, commands::delete_preset, commands::list_passwords, commands::add_password, commands::remove_password`。builder 链加 `.plugin(tauri_plugin_opener::init())`。

Run: `cargo test -p liquimod-app; cargo clippy --workspace --all-targets; cargo fmt --all` — Expected: 绿

- [ ] **Step 4: 提交**

```bash
git add app/src-tauri
git commit -m "feat(app): 预设/密码本命令 + ModDto 缩略图 data URL + 注册 opener 插件"
```

---

### Task 5: 前端 api.ts + PresetMenu 组件 + 主页接入

**Files:**
- Modify: `app/src/lib/api.ts`
- Create: `app/src/lib/components/PresetMenu.svelte`、`app/src/lib/components/PresetMenu.test.ts`
- Modify: `app/src/routes/+page.svelte`

UI 代码为主模型定稿，逐字实现，勿自由发挥。

- [ ] **Step 1: api.ts 扩展**

```ts
export interface ModDto {
  id: number;
  name: string;
  enabled: boolean;
  installed_at: number;
  thumb: string | null;
}

export interface PresetDto {
  id: number;
  name: string;
  created_at: number;
}

export interface ApplyResultDto {
  enabled: number;
  disabled: number;
}
```

mockMods 三条各加 `thumb: null`。`call` 的 mock switch 追加：

```ts
      case "list_presets":
        return structuredClone(mockPresets) as T;
      case "save_preset": {
        const p = { id: mockPresets.length + 1, name: String(args?.name ?? "预设"), created_at: 1755000000 };
        const i = mockPresets.findIndex((x) => x.name === p.name);
        if (i >= 0) mockPresets[i] = { ...p, id: mockPresets[i].id };
        else mockPresets.push(p);
        return structuredClone(p) as T;
      }
      case "apply_preset":
        return { enabled: 2, disabled: 1 } as T;
      case "delete_preset": {
        const i = mockPresets.findIndex((x) => x.id === Number(args?.id));
        if (i >= 0) mockPresets.splice(i, 1);
        return undefined as T;
      }
      case "list_passwords":
        return structuredClone(mockPasswords) as T;
      case "add_password":
        mockPasswords.push(String(args?.value ?? ""));
        return undefined as T;
      case "remove_password": {
        const i = mockPasswords.indexOf(String(args?.value));
        if (i >= 0) mockPasswords.splice(i, 1);
        return undefined as T;
      }
```

mock 数据（放 mockMods 后）：

```ts
const mockPresets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1755000000 },
  { id: 2, name: "截图模式", created_at: 1755100000 },
];

const mockPasswords: string[] = ["1234"];
```

`api` 对象追加：

```ts
  listPresets: () => call<PresetDto[]>("list_presets"),
  savePreset: (name: string) => call<PresetDto>("save_preset", { name }),
  applyPreset: (id: number, name: string) =>
    call<ApplyResultDto>("apply_preset", { id, name }),
  deletePreset: (id: number) => call<void>("delete_preset", { id }),
  listPasswords: () => call<string[]>("list_passwords"),
  addPassword: (value: string) => call<void>("add_password", { value }),
  removePassword: (value: string) => call<void>("remove_password", { value }),
```

- [ ] **Step 2: 写失败测试 PresetMenu.test.ts**

```ts
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import PresetMenu from "./PresetMenu.svelte";
import { api, type PresetDto } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPresets: vi.fn(),
      savePreset: vi.fn(),
      applyPreset: vi.fn(),
      deletePreset: vi.fn(),
    },
  };
});

const presets: PresetDto[] = [
  { id: 1, name: "日常出战", created_at: 1 },
  { id: 2, name: "截图模式", created_at: 2 },
];

describe("PresetMenu", () => {
  beforeEach(() => {
    vi.mocked(api.listPresets).mockResolvedValue(presets);
    vi.mocked(api.savePreset).mockResolvedValue({ id: 3, name: "新", created_at: 3 });
    vi.mocked(api.applyPreset).mockResolvedValue({ enabled: 2, disabled: 1 });
    vi.mocked(api.deletePreset).mockResolvedValue(undefined);
  });

  it("打开时加载并列出预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => expect(screen.getByText("日常出战")).toBeTruthy());
    expect(screen.getByText("截图模式")).toBeTruthy();
  });

  it("保存当前为预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await fireEvent.input(screen.getByPlaceholderText("保存当前启用为预设…"), {
      target: { value: "新组合" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "保存" }));
    expect(api.savePreset).toHaveBeenCalledWith("新组合");
  });

  it("应用预设并回调 onapplied", async () => {
    const onapplied = vi.fn();
    render(PresetMenu, { props: { onapplied } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => screen.getByText("日常出战"));
    await fireEvent.click(screen.getByText("日常出战"));
    expect(api.applyPreset).toHaveBeenCalledWith(1, "日常出战");
    await waitFor(() => expect(onapplied).toHaveBeenCalled());
  });

  it("删除预设", async () => {
    render(PresetMenu, { props: { onapplied: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: "预设" }));
    await waitFor(() => screen.getByText("日常出战"));
    await fireEvent.click(screen.getByLabelText("删除预设 日常出战"));
    expect(api.deletePreset).toHaveBeenCalledWith(1);
  });
});
```

Run: `cd app; npx vitest run src/lib/components/PresetMenu.test.ts` — Expected: FAIL（组件不存在）

- [ ] **Step 3: 实现 PresetMenu.svelte（主模型定稿，逐字）**

```svelte
<script lang="ts">
  import { api, type PresetDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";

  let { onapplied }: { onapplied: () => void } = $props();

  let open = $state(false);
  let presets = $state<PresetDto[]>([]);
  let newName = $state("");
  let busy = $state(false);

  async function load() {
    try {
      presets = await api.listPresets();
    } catch (e) {
      toast(String(e));
    }
  }

  function toggleOpen() {
    open = !open;
    if (open) void load();
  }

  async function save() {
    const name = newName.trim();
    if (!name || busy) return;
    busy = true;
    try {
      await api.savePreset(name);
      newName = "";
      await load();
      toast(`已保存预设「${name}」`);
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function apply(p: PresetDto) {
    if (busy) return;
    busy = true;
    try {
      await api.applyPreset(p.id, p.name);
      onapplied();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function remove(p: PresetDto) {
    try {
      await api.deletePreset(p.id);
      await load();
    } catch (e) {
      toast(String(e));
    }
  }
</script>

<div class="relative">
  <button
    class="glass radius-pill h-9 px-4 text-sm flex items-center gap-1.5 cursor-pointer transition-transform hover:scale-[1.03]"
    aria-label="预设"
    onclick={toggleOpen}
  >
    <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path
        d="M3 1.5h7v10l-3.5-2.6L3 11.5v-10z"
        stroke="currentColor"
        stroke-width="1.2"
        stroke-linejoin="round"
      />
    </svg>
    预设
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭预设菜单"
      onclick={() => (open = false)}
    ></button>
    <div class="glass radius-panel absolute right-0 top-11 z-50 w-72 p-2.5 flex flex-col gap-1">
      {#each presets as p (p.id)}
        <div class="flex items-center gap-1 rounded-xl px-1.5 py-1 transition-colors hover:bg-[var(--glass-stroke)]">
          <button
            class="flex-1 text-left text-sm px-1.5 py-1 cursor-pointer truncate disabled:opacity-50"
            disabled={busy}
            onclick={() => apply(p)}
          >
            {p.name}
          </button>
          <button
            class="w-6 h-6 grid place-items-center rounded-full text-secondary cursor-pointer transition-colors hover:bg-[var(--danger)] hover:text-white"
            aria-label={`删除预设 ${p.name}`}
            onclick={() => remove(p)}
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
              <path d="M2 2l5 5M7 2L2 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      {:else}
        <p class="text-xs text-secondary px-2.5 py-2">还没有预设，保存当前启用组合试试</p>
      {/each}
      <div class="flex gap-1.5 mt-1 pt-2" style="border-top: 0.5px solid var(--glass-stroke)">
        <input
          bind:value={newName}
          placeholder="保存当前启用为预设…"
          class="flex-1 h-8 px-3 text-sm bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onkeydown={(e) => e.key === "Enter" && save()}
        />
        <button
          class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
          disabled={!newName.trim() || busy}
          onclick={save}
        >
          保存
        </button>
      </div>
    </div>
  {/if}
</div>
```

Run: `cd app; npx vitest run src/lib/components/PresetMenu.test.ts` — Expected: 4 passed

- [ ] **Step 4: +page.svelte 接入**

`import` 区加：

```svelte
  import PresetMenu from "$lib/components/PresetMenu.svelte";
```

主页 header 右端（`<SearchBar bind:value={query} />` 处）改为：

```svelte
      <div class="flex items-center gap-2.5">
        <PresetMenu onapplied={refresh} />
        <SearchBar bind:value={query} />
      </div>
```

Run: `cd app; npm test; npm run check` — Expected: 全绿

- [ ] **Step 5: 提交**

```bash
git add app/src
git commit -m "feat(ui): 预设弹层（保存快照/一键应用/删除）+ 主页接入"
```

---

### Task 6: 设置页 + TitleBar 齿轮 + CharacterDetail 缩略图

**Files:**
- Create: `app/src/lib/views/Settings.svelte`、`app/src/lib/views/Settings.test.ts`
- Modify: `app/src/lib/components/TitleBar.svelte`、`app/src/lib/views/CharacterDetail.svelte`、`app/src/routes/+page.svelte`

- [ ] **Step 1: 写失败测试 Settings.test.ts**

```ts
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { describe, it, expect, vi, beforeEach } from "vitest";
import Settings from "./Settings.svelte";
import { api } from "$lib/api";

vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPasswords: vi.fn(),
      addPassword: vi.fn(),
      removePassword: vi.fn(),
    },
    isTauri: () => false,
  };
});

const config = { library_root: "C:/mock/Library", mods_dir: "D:/game/Mods" };

describe("Settings", () => {
  beforeEach(() => {
    vi.mocked(api.listPasswords).mockResolvedValue(["1234"]);
    vi.mocked(api.addPassword).mockResolvedValue(undefined);
    vi.mocked(api.removePassword).mockResolvedValue(undefined);
  });

  it("显示目录配置", () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    expect(screen.getByText("C:/mock/Library")).toBeTruthy();
    expect(screen.getByText("D:/game/Mods")).toBeTruthy();
  });

  it("加载并展示密码本", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => expect(screen.getByText("1234")).toBeTruthy());
  });

  it("添加密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await fireEvent.input(screen.getByPlaceholderText("添加解压密码…"), {
      target: { value: "abc" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "添加" }));
    expect(api.addPassword).toHaveBeenCalledWith("abc");
  });

  it("移除密码", async () => {
    render(Settings, { props: { config, onback: () => {}, onchanged: () => {} } });
    await waitFor(() => screen.getByText("1234"));
    await fireEvent.click(screen.getByLabelText("移除密码 1234"));
    expect(api.removePassword).toHaveBeenCalledWith("1234");
  });

  it("返回回调", async () => {
    const onback = vi.fn();
    render(Settings, { props: { config, onback, onchanged: () => {} } });
    await fireEvent.click(screen.getByRole("button", { name: /返回/ }));
    expect(onback).toHaveBeenCalled();
  });
});
```

Run: `cd app; npx vitest run src/lib/views/Settings.test.ts` — Expected: FAIL

- [ ] **Step 2: 实现 Settings.svelte（主模型定稿，逐字）**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { api, isTauri, type ConfigDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    config,
    onback,
    onchanged,
  }: {
    config: ConfigDto | null;
    onback: () => void;
    onchanged: () => void;
  } = $props();

  let passwords = $state<string[]>([]);
  let newPassword = $state("");

  onMount(async () => {
    try {
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    }
  });

  async function pickModsDir() {
    try {
      const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
      if (typeof path === "string") {
        await api.chooseModsDir(path);
        toast("已更新 Mods 目录");
        onchanged();
      }
    } catch (e) {
      toast(String(e));
    }
  }

  async function openLibrary() {
    if (!isTauri() || !config) return;
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(config.library_root);
    } catch (e) {
      toast(String(e));
    }
  }

  async function addPassword() {
    const v = newPassword.trim();
    if (!v) return;
    try {
      await api.addPassword(v);
      newPassword = "";
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    }
  }

  async function removePassword(v: string) {
    try {
      await api.removePassword(v);
      passwords = await api.listPasswords();
    } catch (e) {
      toast(String(e));
    }
  }
</script>

<div class="flex flex-col h-full min-h-0">
  <div class="flex items-center gap-4 px-8 pt-3 pb-4 shrink-0">
    <button
      class="glass radius-pill pl-2.5 pr-3.5 h-8 text-sm flex items-center gap-1 cursor-pointer transition-transform hover:-translate-x-0.5"
      onclick={onback}
    >
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
        <path d="M7 1L2.5 5L7 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      返回
    </button>
    <h2 class="text-2xl font-bold tracking-tight">设置</h2>
  </div>

  <div class="flex flex-col gap-3 px-8 pb-8 overflow-y-auto flex-1 min-h-0 max-w-2xl w-full mx-auto">
    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">目录</h3>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">Mod 仓库（Library）</p>
          <p class="text-xs text-secondary truncate">{config?.library_root ?? "…"}</p>
        </div>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer"
          onclick={openLibrary}
        >
          打开
        </button>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">3Dmigoto Mods 目录</p>
          <p class="text-xs text-secondary truncate">{config?.mods_dir ?? "未配置"}</p>
        </div>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm shrink-0 cursor-pointer"
          onclick={pickModsDir}
        >
          选择…
        </button>
      </div>
    </section>

    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">解压密码本</h3>
      <p class="text-xs text-secondary">安装加密压缩包时自动逐个尝试</p>
      {#each passwords as p (p)}
        <div class="flex items-center justify-between rounded-xl px-3 py-2"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          <span class="text-sm font-mono">{p}</span>
          <button
            class="w-6 h-6 grid place-items-center rounded-full text-secondary cursor-pointer transition-colors hover:bg-[var(--danger)] hover:text-white"
            aria-label={`移除密码 ${p}`}
            onclick={() => removePassword(p)}
          >
            <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
              <path d="M2 2l5 5M7 2L2 7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" />
            </svg>
          </button>
        </div>
      {:else}
        <p class="text-xs text-secondary">空</p>
      {/each}
      <div class="flex gap-1.5 mt-1">
        <input
          bind:value={newPassword}
          placeholder="添加解压密码…"
          class="flex-1 h-8 px-3 text-sm bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
          onkeydown={(e) => e.key === "Enter" && addPassword()}
        />
        <button
          class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
          disabled={!newPassword.trim()}
          onclick={addPassword}
        >
          添加
        </button>
      </div>
    </section>
  </div>
</div>
```

- [ ] **Step 3: TitleBar.svelte 加齿轮按钮**

`<script>` 加 prop：

```svelte
  let { onsettings }: { onsettings: () => void } = $props();
```

右侧按钮组**最前**（minimize 之前）插入：

```svelte
    <button
      aria-label="设置"
      class="w-8 h-8 grid place-items-center rounded-full transition-colors hover:bg-[var(--glass-stroke)]"
      onclick={onsettings}
    >
      <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
        <path
          d="M7 4.8a2.2 2.2 0 1 0 0 4.4 2.2 2.2 0 0 0 0-4.4zM11.9 5.5l-.8-.5c.1-.5.1-1-.1-1.5l.5-.8-1-1-.8.5c-.4-.3-.9-.5-1.4-.5L8.9.8H7.1l-.1.9c-.5.1-1 .3-1.4.6l-.8-.5-1 1 .5.8c-.2.4-.3.9-.2 1.4l-.8.5.4 1.1.9-.2c.3.4.7.8 1.2 1l-.2.9h1.1l.5-.8c.5 0 1-.1 1.4-.4l.8.5 1-1-.5-.8c.2-.5.3-1 .2-1.5l.8-.5-.4-1.3z"
          stroke="currentColor"
          stroke-width="1"
          stroke-linejoin="round"
        />
      </svg>
    </button>
```

（齿轮 path 若渲染怪异，执行者可替换为任意简洁齿轮 SVG，保持 14×14 stroke 风格。）

- [ ] **Step 4: +page.svelte 视图切换**

`import` 区加 `import Settings from "$lib/views/Settings.svelte";`；状态加 `let showSettings = $state(false);`；`<TitleBar />` 改 `<TitleBar onsettings={() => (showSettings = true)} />`；主体分支改为：

```svelte
  {#if showSettings}
    <Settings
      {config}
      onback={() => {
        showSettings = false;
        refresh();
      }}
      onchanged={refresh}
    />
  {:else if selected}
    ...（原 CharacterDetail 分支不变）
  {:else}
    ...（原主页分支不变）
  {/if}
```

- [ ] **Step 5: CharacterDetail.svelte 缩略图**

mod 行（`{#each mods as mod (mod.id)}` 内的 glass 卡片）改为：

```svelte
      <div class="glass radius-card px-5 py-3.5 flex items-center justify-between gap-3">
        <div class="flex items-center gap-3 min-w-0">
          {#if mod.thumb}
            <img
              src={mod.thumb}
              alt=""
              class="w-11 h-11 rounded-xl object-cover shrink-0"
              style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
              draggable="false"
            />
          {/if}
          <span class="font-medium truncate">{mod.name}</span>
        </div>
        <Toggle
          checked={mod.enabled}
          ariaLabel={`启用 ${mod.name}`}
          onchange={(next) => toggle(mod, next)}
        />
      </div>
```

Run: `cd app; npm test; npm run check` — Expected: 全绿（既有 CharacterDetail 测试若断言旧结构需同步微调）

- [ ] **Step 6: 提交**

```bash
git add app/src
git commit -m "feat(ui): 设置页（目录/密码本）+ TitleBar 齿轮 + Mod 列表缩略图"
```

---

### Task 7: 打磨（lang / CSP / 残留清理）

**Files:**
- Modify: `app/src/app.html`、`app/src-tauri/tauri.conf.json`

- [ ] **Step 1: app.html `lang="en"` 改 `lang="zh-CN"`**

- [ ] **Step 2: CSP 加固**

`tauri.conf.json` 的 `"csp": null` 改为：

```json
      "csp": "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self' ipc: http://ipc.localhost; font-src 'self' data:"
```

- [ ] **Step 3: 验证构建不炸**

Run: `cd app; npm run build`；然后 `cargo build --release --features tauri/custom-protocol --manifest-path app\src-tauri\Cargo.toml`
Expected: 双双成功

- [ ] **Step 4: 提交**

```bash
git add app/src/app.html app/src-tauri/tauri.conf.json
git commit -m "chore(app): lang=zh-CN + CSP 收紧"
```

---

### Task 8: E2E 验证 + 终审

- [ ] **Step 1: 全量测试**

Run: `cargo test --workspace; cd app; npm test; npm run check`
Expected: 全绿

- [ ] **Step 2: 重建并启动 exe**（按 AGENTS.md 构建段；杀旧进程、双 exe 构建、设 WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS 启动）

- [ ] **Step 3: CDP 自动化验证**

a) 给真实库中某个已有 Mod（如 `Library\mods\Asta\<现存目录>`）放一张 `preview.png`（PowerShell 生成纯色 PNG 或复制任意图），打开该角色页 → 截图确认缩略图出现。
b) 主页点「预设」→ 输入名保存 → 截图确认条目出现；点该预设应用 → toast「已应用预设…」。
c) 点齿轮 → 设置页截图（目录两行 + 密码本）；添加/移除密码各一次。
d) 确认无 CSP 控制台报错（CDP Runtime console 或界面正常渲染即过）。
截图脚本沿用 `%LOCALAPPDATA%\Temp\opencode\cdpshot.mjs`（`http://localhost:9223/json`）。

- [ ] **Step 4: 终审子代理**

对 `dd8252f..HEAD` 全量 diff 做跨任务集成审查（契约一致性、锁序、生命周期、安全面、回归），修复闭环。

- [ ] **Step 5: 提交计划文档**

```bash
git add docs/superpowers/plans/2026-08-18-liquimod-presets-settings.md
git commit -m "docs: 里程碑 6 计划"
```

---

## Self-Review 结论

- **Spec 覆盖**：预设（设计 §4.1 末 + §5 presets 表）→ Task 1/2/4/5；设置页（§5 settings 中的路径项 → Config 已有 library_root/mods_dir；密码本 §5 passwords）→ Task 4/6；缩略图（§4.2「后台生成缩略图」+ §5 缩略图路径 → 采用无 DB 缓存的等效方案）→ Task 3/4/6；打磨 → Task 7（CSP、lang 均来自既有积压清单）。
- **类型一致性**：`save_preset_named/preset_dtos/apply_preset_by_id/thumb_data_url` 在 Task 4 定义并在 Task 4 测试与命令中使用；前端 `api.applyPreset(id, name)` 与后端 `apply_preset(id, name)` 参数名一致；PresetDto/ApplyResultDto 前后端字段一致。
- **已修正的自审发现**：apply_preset 初稿曾 emit `library-changed` 会与前端 watcher toast 重复，已删；thumb_data_url 测试签名两参/三参不一致已统一为三参。
