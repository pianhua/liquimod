# LiquiMod Rust 核心实现计划（里程碑 1）

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现 LiquiMod 的 Rust 核心：Mod 仓库管理、NTFS Junction 部署引擎、SQLite 索引与操作日志，并用 CLI 端到端验证。

**Architecture:** 磁盘文件是唯一真相，SQLite 仅为索引（可重建）。启用 Mod = 在 3Dmigoto Mods 目录创建指向仓库的 Junction；禁用 = 删除 Junction。所有部署操作先写 op_log，崩溃后启动时对账恢复。

**Tech Stack:** Rust 1.96（edition 2021, cargo workspace）、rusqlite(bundled)、junction、thiserror、clap(derive)、tempfile（测试）。

**项目根目录：** `C:\Users\10697\Desktop\liquimod`（已 git init）

**参考源码（遇到不确定的行为时查阅）：** `C:\Users\10697\Desktop\JASM\src`（JASM 的 C# 实现）

---

## 文件结构

```
liquimod/
  Cargo.toml                      # workspace
  crates/
    liquimod-core/
      Cargo.toml
      src/
        lib.rs                    # 模块导出
        error.rs                  # LiquiModError / Result
        paths.rs                  # LibraryLayout：路径与名称校验
        models.rs                 # ModEntry
        db.rs                     # Database：schema、mods CRUD、op_log
        library.rs                # Library：init/open/scan/add_folder/list
        deploy.rs                 # Deployer：enable/disable/reconcile/status/recover
    liquimod-cli/
      Cargo.toml
      src/main.rs                 # clap CLI：init/scan/add/enable/disable/reconcile/status
```

---

### Task 1: Cargo workspace + 错误类型

**Files:**
- Create: `Cargo.toml`
- Create: `crates/liquimod-core/Cargo.toml`
- Create: `crates/liquimod-core/src/lib.rs`
- Create: `crates/liquimod-core/src/error.rs`

- [ ] **Step 1: 创建 workspace 与 core crate 骨架**

`Cargo.toml`（项目根）:
```toml
[workspace]
resolver = "2"
members = ["crates/liquimod-core", "crates/liquimod-cli"]
```

`crates/liquimod-core/Cargo.toml`:
```toml
[package]
name = "liquimod-core"
version = "0.1.0"
edition = "2021"

[dependencies]
thiserror = "2"
rusqlite = { version = "0.32", features = ["bundled"] }
junction = "1"

[dev-dependencies]
tempfile = "3"
```

`crates/liquimod-core/src/lib.rs`:
```rust
pub mod error;
```

`crates/liquimod-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LiquiModError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("junction error: {0}")]
    Junction(String),
    #[error("mod not found: {0}")]
    ModNotFound(String),
    #[error("invalid name: {0}")]
    InvalidName(String),
}

pub type Result<T> = std::result::Result<T, LiquiModError>;
```

- [ ] **Step 2: 验证编译**

Run: `cargo build -p liquimod-core`
Expected: 编译成功（允许未使用警告）

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml crates/liquimod-core
git commit -m "chore: scaffold workspace with core crate and error types"
```

---

### Task 2: LibraryLayout 路径与名称校验

**Files:**
- Create: `crates/liquimod-core/src/paths.rs`
- Modify: `crates/liquimod-core/src/lib.rs`（加 `pub mod paths;`）

- [ ] **Step 1: 写失败测试**

`crates/liquimod-core/src/paths.rs`（先只放测试）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_paths() {
        let l = LibraryLayout::new("C:/lib");
        assert_eq!(l.mods_root().to_str().unwrap(), "C:/lib/mods");
        assert_eq!(l.db_path().to_str().unwrap(), "C:/lib/liquimod.db");
        assert_eq!(l.character_dir("Firefly").to_str().unwrap(), "C:/lib/mods/Firefly");
        assert_eq!(l.mod_dir("Firefly", "Summer").to_str().unwrap(), "C:/lib/mods/Firefly/Summer");
    }

    #[test]
    fn rejects_bad_segments() {
        assert!(!is_valid_segment(""));
        assert!(!is_valid_segment("a/b"));
        assert!(!is_valid_segment("a\\b"));
        assert!(!is_valid_segment(".."));
        assert!(is_valid_segment("流萤 Firefly"));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core paths`
Expected: FAIL（`LibraryLayout` 未定义，编译错误）

- [ ] **Step 3: 实现**

在 `crates/liquimod-core/src/paths.rs` 测试模块之前加入:
```rust
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct LibraryLayout {
    pub root: PathBuf,
}

impl LibraryLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    pub fn mods_root(&self) -> PathBuf {
        self.root.join("mods")
    }
    pub fn db_path(&self) -> PathBuf {
        self.root.join("liquimod.db")
    }
    pub fn character_dir(&self, character: &str) -> PathBuf {
        self.mods_root().join(character)
    }
    pub fn mod_dir(&self, character: &str, name: &str) -> PathBuf {
        self.character_dir(character).join(name)
    }
}

pub fn is_valid_segment(s: &str) -> bool {
    !s.is_empty() && s != ".." && !s.contains(['/', '\\'])
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core paths`
Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/paths.rs crates/liquimod-core/src/lib.rs
git commit -m "feat(core): library path layout and name validation"
```

---

### Task 3: ModEntry 模型 + Database mods CRUD

**Files:**
- Create: `crates/liquimod-core/src/models.rs`
- Create: `crates/liquimod-core/src/db.rs`
- Modify: `crates/liquimod-core/src/lib.rs`（加 `pub mod models; pub mod db;`）

- [ ] **Step 1: 写失败测试**

`crates/liquimod-core/src/db.rs`（先只放测试）:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upsert_list_and_remove() {
        let db = Database::open_in_memory().unwrap();
        let id = db.upsert_mod("Firefly", "Summer", "mods/Firefly/Summer").unwrap();
        // 重复 upsert 幂等
        let id2 = db.upsert_mod("Firefly", "Summer", "mods/Firefly/Summer").unwrap();
        assert_eq!(id, id2);

        db.set_enabled(id, true).unwrap();
        let mods = db.list_mods().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].character, "Firefly");
        assert_eq!(mods[0].name, "Summer");
        assert!(mods[0].enabled);

        let got = db.get_mod(id).unwrap();
        assert_eq!(got.rel_path, "mods/Firefly/Summer");

        db.remove_mod(id).unwrap();
        assert!(db.list_mods().unwrap().is_empty());
        assert!(matches!(db.get_mod(id), Err(crate::error::LiquiModError::ModNotFound(_))));
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core db`
Expected: FAIL（`Database` 未定义）

- [ ] **Step 3: 实现**

`crates/liquimod-core/src/models.rs`:
```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ModEntry {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub rel_path: String,
    pub enabled: bool,
    pub installed_at: i64,
}
```

`crates/liquimod-core/src/db.rs`（测试模块之前）:
```rust
use crate::error::{LiquiModError, Result};
use crate::models::ModEntry;
use rusqlite::Connection;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Database {
    conn: Connection,
}

pub fn now_unix() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             CREATE TABLE IF NOT EXISTS mods (
               id INTEGER PRIMARY KEY,
               character TEXT NOT NULL,
               name TEXT NOT NULL,
               rel_path TEXT NOT NULL,
               enabled INTEGER NOT NULL DEFAULT 0,
               installed_at INTEGER NOT NULL,
               UNIQUE(character, name)
             );
             CREATE TABLE IF NOT EXISTS op_log (
               id INTEGER PRIMARY KEY,
               op TEXT NOT NULL,
               payload TEXT NOT NULL,
               finished INTEGER NOT NULL DEFAULT 0,
               created_at INTEGER NOT NULL
             );",
        )?;
        Ok(Self { conn })
    }

    pub fn upsert_mod(&self, character: &str, name: &str, rel_path: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO mods (character, name, rel_path, installed_at)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(character, name) DO UPDATE SET rel_path = excluded.rel_path",
            rusqlite::params![character, name, rel_path, now_unix()],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM mods WHERE character = ?1 AND name = ?2",
            rusqlite::params![character, name],
            |r| r.get(0),
        )?;
        Ok(id)
    }

    pub fn set_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE mods SET enabled = ?1 WHERE id = ?2",
            rusqlite::params![enabled as i64, id],
        )?;
        Ok(())
    }

    fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<ModEntry> {
        Ok(ModEntry {
            id: r.get(0)?,
            character: r.get(1)?,
            name: r.get(2)?,
            rel_path: r.get(3)?,
            enabled: r.get::<_, i64>(4)? != 0,
            installed_at: r.get(5)?,
        })
    }

    pub fn list_mods(&self) -> Result<Vec<ModEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, character, name, rel_path, enabled, installed_at FROM mods ORDER BY character, name",
        )?;
        let rows = stmt.query_map([], Self::row_to_entry)?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn get_mod(&self, id: i64) -> Result<ModEntry> {
        self.conn
            .query_row(
                "SELECT id, character, name, rel_path, enabled, installed_at FROM mods WHERE id = ?1",
                rusqlite::params![id],
                Self::row_to_entry,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => LiquiModError::ModNotFound(id.to_string()),
                other => LiquiModError::Db(other),
            })
    }

    pub fn remove_mod(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM mods WHERE id = ?1", rusqlite::params![id])?;
        Ok(())
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core db`
Expected: 1 passed

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/models.rs crates/liquimod-core/src/db.rs crates/liquimod-core/src/lib.rs
git commit -m "feat(core): sqlite schema and mods CRUD"
```

---

### Task 4: op_log（崩溃恢复日志）

**Files:**
- Modify: `crates/liquimod-core/src/db.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/liquimod-core/src/db.rs` 的 `mod tests` 中追加:
```rust
    #[test]
    fn op_log_lifecycle() {
        let db = Database::open_in_memory().unwrap();
        let op = db.op_begin("enable", "42").unwrap();
        let pending = db.pending_ops().unwrap();
        assert_eq!(pending, vec![(op, "enable".to_string(), "42".to_string())]);

        db.op_finish(op).unwrap();
        assert!(db.pending_ops().unwrap().is_empty());
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core db::tests::op_log_lifecycle`
Expected: FAIL（方法未定义）

- [ ] **Step 3: 实现**

在 `impl Database` 中追加:
```rust
    pub fn op_begin(&self, op: &str, payload: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO op_log (op, payload, created_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![op, payload, now_unix()],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn op_finish(&self, op_id: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE op_log SET finished = 1 WHERE id = ?1",
            rusqlite::params![op_id],
        )?;
        Ok(())
    }

    pub fn pending_ops(&self) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, op, payload FROM op_log WHERE finished = 0 ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core db`
Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/db.rs
git commit -m "feat(core): operation log for crash recovery"
```

---

### Task 5: Library init / scan（文件系统 ↔ 索引对账）

**Files:**
- Create: `crates/liquimod-core/src/library.rs`
- Modify: `crates/liquimod-core/src/lib.rs`（加 `pub mod library;`）

- [ ] **Step 1: 写失败测试**

`crates/liquimod-core/src/library.rs`（先只放测试）:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn init_creates_layout_and_scan_reconciles() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(tmp.path()).unwrap();
        assert!(lib.layout.mods_root().is_dir());
        assert!(lib.layout.db_path().is_file());

        // 磁盘上出现两个 mod（手动放的），scan 应收录
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        assert_eq!(mods.len(), 2);

        // 删掉一个，scan 应从索引移除
        fs::remove_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        assert_eq!(mods.len(), 1);
        assert_eq!(mods[0].character, "Firefly");

        // open 已存在的库不丢数据
        let lib2 = Library::open(tmp.path()).unwrap();
        assert_eq!(lib2.list().unwrap().len(), 1);
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core library`
Expected: FAIL（`Library` 未定义）

- [ ] **Step 3: 实现**

在 `crates/liquimod-core/src/library.rs` 测试模块之前加入:
```rust
use crate::db::Database;
use crate::error::Result;
use crate::models::ModEntry;
use crate::paths::{is_valid_segment, LibraryLayout};
use std::path::Path;

pub struct Library {
    pub layout: LibraryLayout,
    pub db: Database,
}

impl Library {
    /// 创建（或打开已存在的）库，确保目录与数据库就绪。
    pub fn init(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        std::fs::create_dir_all(layout.mods_root())?;
        let db = Database::open(&layout.db_path())?;
        Ok(Self { layout, db })
    }

    /// 打开已存在的库（数据库必须已存在）。
    pub fn open(root: &Path) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        let db = Database::open(&layout.db_path())?;
        Ok(Self { layout, db })
    }

    pub fn list(&self) -> Result<Vec<ModEntry>> {
        self.db.list_mods()
    }

    /// 扫描磁盘目录，与 SQLite 索引对账：新目录收录、消失目录移除。
    pub fn scan(&self) -> Result<Vec<ModEntry>> {
        let mut seen: Vec<(String, String)> = Vec::new();
        let mods_root = self.layout.mods_root();
        if mods_root.is_dir() {
            for char_entry in std::fs::read_dir(&mods_root)? {
                let char_entry = char_entry?;
                let character = char_entry.file_name().to_string_lossy().into_owned();
                // 跳过 junction/符号链接与非目录：仓库里只允许真实目录
                if !char_entry.file_type()?.is_dir() || !is_valid_segment(&character) {
                    continue;
                }
                for mod_entry in std::fs::read_dir(char_entry.path())? {
                    let mod_entry = mod_entry?;
                    let name = mod_entry.file_name().to_string_lossy().into_owned();
                    if !mod_entry.file_type()?.is_dir() || !is_valid_segment(&name) {
                        continue;
                    }
                    let rel = format!("mods/{}/{}", character, name);
                    self.db.upsert_mod(&character, &name, &rel)?;
                    seen.push((character.clone(), name));
                }
            }
        }
        for m in self.db.list_mods()? {
            if !seen.contains(&(m.character.clone(), m.name.clone())) {
                self.db.remove_mod(m.id)?;
            }
        }
        self.db.list_mods()
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core library`
Expected: 1 passed

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/library.rs crates/liquimod-core/src/lib.rs
git commit -m "feat(core): library init and filesystem reconcile scan"
```

---

### Task 6: add_folder（复制外部文件夹入仓库）

**Files:**
- Modify: `crates/liquimod-core/src/library.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/liquimod-core/src/library.rs` 的 `mod tests` 中追加:
```rust
    #[test]
    fn add_folder_copies_and_indexes() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();

        let src = tmp.path().join("download/MyMod");
        fs::create_dir_all(src.join("textures")).unwrap();
        fs::write(src.join("mod.ini"), b"[Constants]").unwrap();
        fs::write(src.join("textures/a.dds"), b"dds").unwrap();

        let entry = lib.add_folder(&src, "Firefly", "MyMod").unwrap();
        assert_eq!(entry.character, "Firefly");
        assert!(lib.layout.mod_dir("Firefly", "MyMod").join("mod.ini").is_file());
        assert!(lib.layout.mod_dir("Firefly", "MyMod").join("textures/a.dds").is_file());

        // 非法名称被拒
        assert!(lib.add_folder(&src, "bad/name", "x").is_err());
        assert!(lib.add_folder(&src, "Firefly", "..").is_err());
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core library::tests::add_folder_copies_and_indexes`
Expected: FAIL（方法未定义）

- [ ] **Step 3: 实现**

在 `impl Library` 中追加:
```rust
    /// 把外部文件夹复制进仓库并收录索引。已存在同名 mod 则覆盖式合并。
    pub fn add_folder(&self, src: &Path, character: &str, name: &str) -> Result<ModEntry> {
        if !is_valid_segment(character) {
            return Err(crate::error::LiquiModError::InvalidName(character.into()));
        }
        if !is_valid_segment(name) {
            return Err(crate::error::LiquiModError::InvalidName(name.into()));
        }
        let dest = self.layout.mod_dir(character, name);
        std::fs::create_dir_all(&dest)?;
        copy_dir_recursive(src, &dest)?;
        let rel = format!("mods/{}/{}", character, name);
        let id = self.db.upsert_mod(character, name, &rel)?;
        self.db.get_mod(id)
    }
```

在文件末尾（`mod tests` 之前）追加:
```rust
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core library`
Expected: 2 passed

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/library.rs
git commit -m "feat(core): add_folder copies external folder into library"
```

---

### Task 7: Deployer enable / disable（Junction 启停）

**Files:**
- Create: `crates/liquimod-core/src/deploy.rs`
- Modify: `crates/liquimod-core/src/lib.rs`（加 `pub mod deploy;`）

- [ ] **Step 1: 写失败测试**

`crates/liquimod-core/src/deploy.rs`（先只放测试）:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Library, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();
        let mods_dir = tmp.path().join("GameMods");
        fs::create_dir_all(&mods_dir).unwrap();
        (tmp, lib, mods_dir)
    }

    #[test]
    fn enable_creates_junction_disable_removes_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();

        let link = mods_dir.join(Deployer::link_name(&entry));
        assert!(junction::exists(&link).unwrap());
        // 透过 junction 能看到仓库内容
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);

        d.disable(entry.id).unwrap();
        assert!(!link.exists());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);

        // 重复操作幂等
        d.disable(entry.id).unwrap();
    }
}
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core deploy`
Expected: FAIL（`Deployer` 未定义）

- [ ] **Step 3: 实现**

在 `crates/liquimod-core/src/deploy.rs` 测试模块之前加入:
```rust
use crate::error::Result;
use crate::library::Library;
use crate::models::ModEntry;
use std::path::{Path, PathBuf};

pub struct Deployer<'a> {
    pub library: &'a Library,
    pub mods_dir: PathBuf,
}

impl<'a> Deployer<'a> {
    pub fn new(library: &'a Library, mods_dir: &Path) -> Self {
        Self { library, mods_dir: mods_dir.to_path_buf() }
    }

    /// 3Dmigoto Mods 目录中的链接名：角色--Mod名（确定性，避免跨角色重名冲突）
    pub fn link_name(entry: &ModEntry) -> String {
        format!("{}--{}", entry.character, entry.name)
    }

    pub fn enable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if entry.enabled {
            return Ok(());
        }
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        let target = self.library.layout.root.join(&entry.rel_path);
        let link = self.mods_dir.join(Self::link_name(&entry));
        if !link.exists() {
            junction::create(&target, &link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        self.library.db.set_enabled(id, true)?;
        self.library.db.op_finish(op)
    }

    pub fn disable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if !entry.enabled {
            return Ok(());
        }
        let op = self.library.db.op_begin("disable", &id.to_string())?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        if junction::exists(&link).unwrap_or(false) {
            junction::delete(&link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        self.library.db.set_enabled(id, false)?;
        self.library.db.op_finish(op)
    }
}
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core deploy`
Expected: 1 passed

注意：junction 在 NTFS 上创建无需管理员权限。若测试机器非 Windows/NTFS，此测试会失败——本项目目标平台为 Windows NTFS，属预期。

- [ ] **Step 5: Commit**

```bash
git add crates/liquimod-core/src/deploy.rs crates/liquimod-core/src/lib.rs
git commit -m "feat(core): junction-based enable/disable deployer"
```

---

### Task 8: reconcile / status / 崩溃恢复

**Files:**
- Modify: `crates/liquimod-core/src/deploy.rs`

- [ ] **Step 1: 写失败测试**

在 `crates/liquimod-core/src/deploy.rs` 的 `mod tests` 中追加:
```rust
    #[test]
    fn reconcile_fixes_drift_and_ignores_foreign_content() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        let firefly = mods.iter().find(|m| m.character == "Firefly").unwrap().clone();
        let acheron = mods.iter().find(|m| m.character == "Acheron").unwrap().clone();

        let d = Deployer::new(&lib, &mods_dir);
        d.enable(firefly.id).unwrap();
        d.enable(acheron.id).unwrap();

        // 漂移1：主人手动删了一个 junction
        junction::delete(mods_dir.join(Deployer::link_name(&acheron))).unwrap();
        // 漂移2：主人手动往 Mods 里放了自己的非管理目录与文件
        fs::create_dir_all(mods_dir.join("MyOwnMod")).unwrap();
        fs::write(mods_dir.join("readme.txt"), b"hi").unwrap();

        d.reconcile().unwrap();

        // 数据库说启用但 junction 没了 → 重建
        assert!(junction::exists(&mods_dir.join(Deployer::link_name(&acheron))).unwrap());
        // 外部内容原样保留
        assert!(mods_dir.join("MyOwnMod").is_dir());
        assert!(mods_dir.join("readme.txt").is_file());

        // status 报告一致性
        let st = d.status().unwrap();
        assert!(st.iter().all(|(_, ok)| *ok));
    }

    #[test]
    fn status_detects_missing_junction() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();
        junction::delete(mods_dir.join(Deployer::link_name(&entry))).unwrap();
        let st = d.status().unwrap();
        assert_eq!(st.len(), 1);
        assert!(!st[0].1);
    }

    #[test]
    fn recover_completes_pending_ops() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        // 模拟崩溃：op_log 里留一条未完成的 enable，junction 未创建
        lib.db.op_begin("enable", &entry.id.to_string()).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        d.recover().unwrap();

        assert!(lib.db.pending_ops().unwrap().is_empty());
        assert!(junction::exists(&mods_dir.join(Deployer::link_name(&entry))).unwrap());
    }
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo test -p liquimod-core deploy`
Expected: FAIL（`reconcile` / `status` / `recover` 未定义）

- [ ] **Step 3: 实现**

在 `impl<'a> Deployer<'a>` 中追加:
```rust
    /// 让 Mods 目录与数据库启用状态一致。
    /// 安全规则：只碰指向本仓库的 junction；用户自己放的目录/文件一律不动。
    pub fn reconcile(&self) -> Result<()> {
        let entries = self.library.db.list_mods()?;
        let mut managed_links: Vec<String> = Vec::new();
        for e in &entries {
            let link = self.mods_dir.join(Self::link_name(e));
            managed_links.push(Self::link_name(e));
            let exists = junction::exists(&link).unwrap_or(false);
            if e.enabled && !exists {
                let target = self.library.layout.root.join(&e.rel_path);
                junction::create(&target, &link)
                    .map_err(|err| crate::error::LiquiModError::Junction(err.to_string()))?;
            } else if !e.enabled && exists {
                junction::delete(&link)
                    .map_err(|err| crate::error::LiquiModError::Junction(err.to_string()))?;
            }
        }
        // 清理指向本仓库、但数据库已无记录的孤儿 junction
        if self.mods_dir.is_dir() {
            for item in std::fs::read_dir(&self.mods_dir)? {
                let item = item?;
                let name = item.file_name().to_string_lossy().into_owned();
                if managed_links.contains(&name) {
                    continue;
                }
                let path = item.path();
                if junction::exists(&path).unwrap_or(false) {
                    if let Ok(target) = junction::get_target(&path) {
                        if target.starts_with(&self.library.layout.root) {
                            junction::delete(&path)
                                .map_err(|err| crate::error::LiquiModError::Junction(err.to_string()))?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 返回每个受管 mod 与其部署状态是否一致。
    pub fn status(&self) -> Result<Vec<(ModEntry, bool)>> {
        let mut out = Vec::new();
        for e in self.library.db.list_mods()? {
            let link = self.mods_dir.join(Self::link_name(&e));
            let actual = junction::exists(&link).unwrap_or(false);
            out.push((e.clone(), actual == e.enabled));
        }
        Ok(out)
    }

    /// 启动时调用：存在未完成的操作日志 → 全量对账（操作均幂等），然后结清日志。
    pub fn recover(&self) -> Result<()> {
        let pending = self.library.db.pending_ops()?;
        if pending.is_empty() {
            return Ok(());
        }
        self.reconcile()?;
        for (op_id, _, _) in pending {
            self.library.db.op_finish(op_id)?;
        }
        Ok(())
    }
```

- [ ] **Step 4: 运行确认通过**

Run: `cargo test -p liquimod-core deploy`
Expected: 4 passed

- [ ] **Step 5: 全量回归**

Run: `cargo test -p liquimod-core`
Expected: 全部通过（共 9 个测试）

- [ ] **Step 6: Commit**

```bash
git add crates/liquimod-core/src/deploy.rs
git commit -m "feat(core): reconcile, status and crash recovery"
```

---

### Task 9: CLI（端到端验证入口）

**Files:**
- Create: `crates/liquimod-cli/Cargo.toml`
- Create: `crates/liquimod-cli/src/main.rs`

- [ ] **Step 1: 实现**

`crates/liquimod-cli/Cargo.toml`:
```toml
[package]
name = "liquimod-cli"
version = "0.1.0"
edition = "2021"

[dependencies]
liquimod-core = { path = "../liquimod-core" }
clap = { version = "4", features = ["derive"] }
```

`crates/liquimod-cli/src/main.rs`:
```rust
use clap::{Parser, Subcommand};
use liquimod_core::deploy::Deployer;
use liquimod_core::library::Library;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "liquimod", about = "LiquiMod core verification CLI")]
struct Cli {
    #[command(subcommand)]
    cmd: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 初始化仓库目录
    Init { #[arg(long)] root: PathBuf },
    /// 扫描仓库与索引对账
    Scan { #[arg(long)] root: PathBuf },
    /// 复制外部文件夹入仓库
    Add {
        #[arg(long)] root: PathBuf,
        #[arg(long)] src: PathBuf,
        #[arg(long)] character: String,
        #[arg(long)] name: String,
    },
    /// 启用 mod（创建 junction）
    Enable { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf, #[arg(long)] id: i64 },
    /// 禁用 mod（删除 junction）
    Disable { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf, #[arg(long)] id: i64 },
    /// 崩溃恢复 + 全量对账
    Reconcile { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf },
    /// 查看状态一致性
    Status { #[arg(long)] root: PathBuf, #[arg(long)] mods_dir: PathBuf },
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(e) = result {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> liquimod_core::error::Result<()> {
    match cli.cmd {
        Command::Init { root } => {
            Library::init(&root)?;
            println!("initialized library at {}", root.display());
        }
        Command::Scan { root } => {
            let lib = Library::open(&root)?;
            for m in lib.scan()? {
                println!("#{} [{}] {} enabled={}", m.id, m.character, m.name, m.enabled);
            }
        }
        Command::Add { root, src, character, name } => {
            let lib = Library::open(&root)?;
            let m = lib.add_folder(&src, &character, &name)?;
            println!("added #{} [{}] {}", m.id, m.character, m.name);
        }
        Command::Enable { root, mods_dir, id } => {
            let lib = Library::open(&root)?;
            Deployer::new(&lib, &mods_dir).enable(id)?;
            println!("enabled #{id}");
        }
        Command::Disable { root, mods_dir, id } => {
            let lib = Library::open(&root)?;
            Deployer::new(&lib, &mods_dir).disable(id)?;
            println!("disabled #{id}");
        }
        Command::Reconcile { root, mods_dir } => {
            let lib = Library::open(&root)?;
            let d = Deployer::new(&lib, &mods_dir);
            d.recover()?;
            d.reconcile()?;
            println!("reconciled");
        }
        Command::Status { root, mods_dir } => {
            let lib = Library::open(&root)?;
            for (m, ok) in Deployer::new(&lib, &mods_dir).status()? {
                let mark = if ok { "OK" } else { "DRIFT" };
                println!("#{} [{}] {} enabled={} [{}]", m.id, m.character, m.name, m.enabled, mark);
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: 编译**

Run: `cargo build`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add crates/liquimod-cli Cargo.toml Cargo.lock
git commit -m "feat(cli): verification CLI for core workflows"
```

---

### Task 10: 手动端到端验收

- [ ] **Step 1: 造一个假 Mod 并走完整流程**

```powershell
# 假 mod 与假游戏目录
New-Item -ItemType Directory -Force "$env:TEMP\lm-demo\srcmod"; Set-Content "$env:TEMP\lm-demo\srcmod\mod.ini" "[Constants]"
New-Item -ItemType Directory -Force "$env:TEMP\lm-demo\GameMods"

cargo run -p liquimod-cli -- init --root "$env:TEMP\lm-demo\lib"
cargo run -p liquimod-cli -- add --root "$env:TEMP\lm-demo\lib" --src "$env:TEMP\lm-demo\srcmod" --character Firefly --name Summer
cargo run -p liquimod-cli -- enable --root "$env:TEMP\lm-demo\lib" --mods-dir "$env:TEMP\lm-demo\GameMods" --id 1
```

Expected:
- `$env:TEMP\lm-demo\GameMods\Firefly--Summer` 存在且是 junction（`Get-Item` 的 LinkType 为 Junction）
- 透过它能看到 `mod.ini`

- [ ] **Step 2: 漂移与恢复**

```powershell
# 手动删掉 junction 模拟外部漂移，然后 reconcile
Remove-Item "$env:TEMP\lm-demo\GameMods\Firefly--Summer"
cargo run -p liquimod-cli -- status --root "$env:TEMP\lm-demo\lib" --mods-dir "$env:TEMP\lm-demo\GameMods"
cargo run -p liquimod-cli -- reconcile --root "$env:TEMP\lm-demo\lib" --mods-dir "$env:TEMP\lm-demo\GameMods"
```

Expected: status 显示 `DRIFT`；reconcile 后 junction 重建，status 显示 `OK`

- [ ] **Step 3: 禁用并清理**

```powershell
cargo run -p liquimod-cli -- disable --root "$env:TEMP\lm-demo\lib" --mods-dir "$env:TEMP\lm-demo\GameMods" --id 1
Remove-Item -Recurse -Force "$env:TEMP\lm-demo"
```

Expected: junction 被删除，GameMods 目录干净

- [ ] **Step 4: Commit（如有文档/脚本变更）**

```bash
git add -A
git commit -m "test: manual end-to-end verification passed"
```

---

## Self-Review 记录

- **Spec 覆盖**：本计划只覆盖里程碑 1（library + deploy + SQLite + CLI 验证）。archive/watcher/games/refresh/UI 属于后续里程碑，各自出计划。
- **类型一致性**：`LibraryLayout::new(root)` 接收 `impl Into<PathBuf>`；`Deployer::new(&lib, &mods_dir)`；`link_name` 格式 `{character}--{name}` 在 Task 7/8/10 一致。
- **占位符扫描**：无 TBD/TODO，所有代码步骤含完整代码。
