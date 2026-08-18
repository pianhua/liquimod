# LiquiMod 里程碑 7「可用性攻坚」实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把已有后端能力全部透到 UI（删除/重命名/打开目录/自动启用/日志），修掉滚动丢失与空格启停缺失，Mod 行信息密度升级。

**Architecture:** core 加 DB 迁移（size_bytes/file_count）+ `Library::rename_mod`；app 壳加 rename_mod/read_log/set_auto_enable 命令 + 安装后自动启用 + tracing 滚动日志 + 启动 Deployer::recover；前端新组件 ModRow（72px 缩略图/副行信息/hover 操作/行内确认卸载/行内重命名/空格启停），主页改 keep-alive 保滚动。

**Tech Stack:** Rust (rusqlite/junction/tracing/tracing-appender) + Tauri 2 + Svelte 5 + Vitest。

**既有事实（勿重复探索）：**
- `db.rs`：`Database{conn}`，`init(conn)` 在 `execute_batch` 建表后 `PRAGMA foreign_keys=ON`；`row_to_entry` 按列序号读；`upsert_mod` 返回 id；`get_mod/list_mods/remove_mod/op_begin/op_finish` 已有。SELECT 列表两处（list_mods 行 102、get_mod 行 111）目前都是 `id, character, name, rel_path, enabled, installed_at`。
- `models.rs`：`ModEntry { id, character, name, rel_path, enabled, installed_at }`。
- `paths.rs`：`is_valid_segment(s)` 校验非空/无分隔符/不以空格或点结尾；`layout.mod_dir(character, name)`。
- `error.rs`：`LiquiModError::{Io, Db, Junction(String), ModNotFound(String), InvalidName(String), DestinationExists{character,name}, ...}`。
- `library.rs`：`Library { layout, db }`（字段均 pub）；`scan()` 内 upsert 后删孤儿（行 38-75）；`add_folder` 是校验+复制范式；测试在文件底部 `#[cfg(test)] mod tests`，用 `tempfile` + `Library::init(tmp.path())`。
- `deploy.rs`：`Deployer::new(&lib, mods_dir)`、`enable(id)/disable(id)/reconcile()/recover()`（均 pub）；`link_name(entry) = {character}--{name}`。enable/disable 内部已写 op_log。
- `commands.rs`：`ConfigDto{library_root, mods_dir}`（行 54）；`ModDto{id,name,enabled,installed_at,thumb}`（行 82）；`collect_mod_rows`（行 159，锁内收集 + `ModRow{id,name,enabled,installed_at,dir}`，sort_by name）；`thumb_data_url(root, dir, id)`（行 239）；`humanize_install_error`（行 312，已映射 DestinationExists/InvalidName——rename 直接复用）；`remove_entry(lib, mods_dir, id)`（行 329）；`install_mod` Tauri 命令（行 442，spawn_blocking 内 install_entry 后 maybe_refresh_game）；测试在文件底部。
- `config.rs`（app）：`Config{library_root, mods_dir}` derive Serialize/Deserialize；`config_path()` = %APPDATA%/LiquiMod/config.json；`save_to(&path)`。
- `state.rs`：`AppState { config: Arc<Mutex<Config>>, library: Arc<Mutex<Library>>, watcher, refresh, config_path }`（字段 pub，命令里直接 `state.config.lock().unwrap()`）。
- `lib.rs`（app）：`run()` 注册插件（dialog/opener）+ `manage(AppState::bootstrap())` + setup 里 `start_watcher`；`invoke_handler` 已有 13 命令含 `uninstall_mod`；`reconcile_and_diff` 纯函数可测。
- app `Cargo.toml` 已有：tauri、serde、serde_json、tokio、base64、tauri-plugin-dialog、tauri-plugin-opener、liquimod-core（path 依赖）、tempfile（dev）。
- 前端：`api.ts` mock 层（`call()` 内 isTauri 分支 + 真实 invoke）；`Toggle.svelte`（props `checked/onchange/ariaLabel`）；`toast.svelte.ts`（`toast(msg)`）；`CharacterDetail.svelte` 持有 `mods` state + `error` string；`+page.svelte` 条件渲染 Settings/CharacterDetail/主页（**切换即销毁，这是 B1 根因**）；Vitest 测试范式见 `PresetMenu.test.ts`（@testing-library/svelte + mock api）。
- **UI 代码主模型已定稿在本计划内，子代理逐字转录，禁止自由发挥样式。**
- **PowerShell 严禁改写前端源文件（编码陷阱）——用 Edit 工具。**

---

### Task 1: core — DB 迁移 + Mod 统计 + rename_mod

**Files:**
- Modify: `crates/liquimod-core/src/models.rs`（ModEntry 加两字段）
- Modify: `crates/liquimod-core/src/db.rs`（迁移 + SQL + rename_mod/update_stats/name_taken）
- Modify: `crates/liquimod-core/src/library.rs`（dir_stats + scan 统计 + Library::rename_mod）
- Test: 同文件内 `#[cfg(test)] mod tests`

- [ ] **Step 1: 失败测试（db 迁移 + 统计 + rename）**

`db.rs` tests 追加：

```rust
#[test]
fn migration_adds_stats_columns_to_old_db() {
    // 旧库没有 size_bytes/file_count：用裸连接建旧 schema，再 Database::open 迁移
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("old.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE mods (
               id INTEGER PRIMARY KEY,
               character TEXT NOT NULL,
               name TEXT NOT NULL,
               rel_path TEXT NOT NULL,
               enabled INTEGER NOT NULL DEFAULT 0,
               installed_at INTEGER NOT NULL,
               UNIQUE(character, name)
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO mods (character, name, rel_path, installed_at) VALUES ('A','m1','mods/A/m1',1)",
            [],
        )
        .unwrap();
    }
    let db = Database::open(&path).unwrap();
    let m = db.get_mod(1).unwrap();
    assert_eq!(m.size_bytes, -1); // 旧行默认 -1（未统计）
    assert_eq!(m.file_count, -1);
}

#[test]
fn rename_mod_updates_name_and_rel_path() {
    let db = Database::open_in_memory().unwrap();
    let id = db.upsert_mod("A", "old", "mods/A/old").unwrap();
    db.rename_mod(id, "new", "mods/A/new").unwrap();
    let m = db.get_mod(id).unwrap();
    assert_eq!(m.name, "new");
    assert_eq!(m.rel_path, "mods/A/new");
    assert!(m.enabled == false && m.installed_at > 0);
}

#[test]
fn update_stats_roundtrip() {
    let db = Database::open_in_memory().unwrap();
    let id = db.upsert_mod("A", "m", "mods/A/m").unwrap();
    db.update_stats(id, 12345, 7).unwrap();
    let m = db.get_mod(id).unwrap();
    assert_eq!((m.size_bytes, m.file_count), (12345, 7));
}

#[test]
fn name_taken_excludes_self() {
    let db = Database::open_in_memory().unwrap();
    let id = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
    db.upsert_mod("A", "m2", "mods/A/m2").unwrap();
    assert!(db.name_taken("A", "m2", id).unwrap());
    assert!(!db.name_taken("A", "m1", id).unwrap()); // 自己不算占用
    assert!(!db.name_taken("B", "m2", id).unwrap()); // 跨角色不冲突
}
```

`library.rs` tests 追加：

```rust
#[test]
fn dir_stats_counts_files_and_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("a.bin"), vec![0u8; 100]).unwrap();
    std::fs::create_dir(tmp.path().join("sub")).unwrap();
    std::fs::write(tmp.path().join("sub/b.bin"), vec![0u8; 50]).unwrap();
    assert_eq!(dir_stats(tmp.path()), (150, 2));
}

#[test]
fn dir_stats_missing_dir_returns_minus_one() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(dir_stats(&tmp.path().join("nope")), (-1, -1));
}

#[test]
fn rename_mod_moves_dir_and_updates_db() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m = lib.add_folder(src.path(), "A", "old").unwrap();
    let renamed = lib.rename_mod(m.id, "new").unwrap();
    assert_eq!(renamed.name, "new");
    assert!(lib.layout.mod_dir("A", "new").is_dir());
    assert!(!lib.layout.mod_dir("A", "old").exists());
}

#[test]
fn rename_mod_rejects_conflict_and_invalid() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m1 = lib.add_folder(src.path(), "A", "m1").unwrap();
    lib.add_folder(src.path(), "A", "m2").unwrap();
    assert!(matches!(
        lib.rename_mod(m1.id, "m2"),
        Err(crate::error::LiquiModError::DestinationExists { .. })
    ));
    assert!(matches!(
        lib.rename_mod(m1.id, "a/b"),
        Err(crate::error::LiquiModError::InvalidName(_))
    ));
    // 冲突失败后目录原样
    assert!(lib.layout.mod_dir("A", "m1").is_dir());
}

#[test]
fn scan_updates_stats() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let dir = lib.layout.mod_dir("A", "m1");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.bin"), vec![0u8; 42]).unwrap();
    lib.scan().unwrap();
    let m = &lib.list().unwrap()[0];
    assert_eq!((m.size_bytes, m.file_count), (42, 1));
}
```

- [ ] **Step 2: 跑测试确认全红**

Run: `cargo test -p liquimod-core`
Expected: FAIL（编译错误：字段/方法不存在）

- [ ] **Step 3: models.rs 加字段**

```rust
pub struct ModEntry {
    pub id: i64,
    pub character: String,
    pub name: String,
    pub rel_path: String,
    pub enabled: bool,
    pub installed_at: i64,
    /// 目录总字节数；-1 = 未统计
    pub size_bytes: i64,
    /// 文件数；-1 = 未统计
    pub file_count: i64,
}
```

- [ ] **Step 4: db.rs 迁移 + 新方法**

`init()` 在建表 `execute_batch` 之后、`PRAGMA foreign_keys` 之前插入：

```rust
// 旧库迁移：补统计列（已存在则忽略 duplicate column 错误）
for col in ["size_bytes", "file_count"] {
    let sql = format!("ALTER TABLE mods ADD COLUMN {col} INTEGER NOT NULL DEFAULT -1");
    match conn.execute_batch(&sql) {
        Ok(()) => {}
        Err(e) if e.to_string().contains("duplicate column") => {}
        Err(e) => return Err(e.into()),
    }
}
```

`row_to_entry` 加两行：

```rust
size_bytes: r.get(6)?,
file_count: r.get(7)?,
```

两处 SELECT 列表都改为：

```rust
"SELECT id, character, name, rel_path, enabled, installed_at, size_bytes, file_count FROM mods ..."
```

新方法：

```rust
pub fn rename_mod(&self, id: i64, new_name: &str, new_rel: &str) -> Result<()> {
    self.conn.execute(
        "UPDATE mods SET name = ?2, rel_path = ?3 WHERE id = ?1",
        rusqlite::params![id, new_name, new_rel],
    )?;
    Ok(())
}

pub fn update_stats(&self, id: i64, size_bytes: i64, file_count: i64) -> Result<()> {
    self.conn.execute(
        "UPDATE mods SET size_bytes = ?2, file_count = ?3 WHERE id = ?1",
        rusqlite::params![id, size_bytes, file_count],
    )?;
    Ok(())
}

pub fn name_taken(&self, character: &str, name: &str, exclude_id: i64) -> Result<bool> {
    let n: i64 = self.conn.query_row(
        "SELECT COUNT(*) FROM mods WHERE character = ?1 AND name = ?2 AND id != ?3",
        rusqlite::params![character, name, exclude_id],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}
```

- [ ] **Step 5: library.rs dir_stats + scan 统计 + rename_mod**

文件底部（`copy_dir_recursive` 旁）加：

```rust
/// 递归统计目录（总字节, 文件数）；任何一级读不了就返回 (-1, -1)（前端显示 "—"）。
fn dir_stats(dir: &std::path::Path) -> (i64, i64) {
    let mut stack = vec![dir.to_path_buf()];
    let (mut size, mut count) = (0i64, 0i64);
    while let Some(d) = stack.pop() {
        let rd = match std::fs::read_dir(&d) {
            Ok(r) => r,
            Err(_) => return (-1, -1),
        };
        for e in rd.flatten() {
            let ft = match e.file_type() {
                Ok(t) => t,
                Err(_) => continue,
            };
            if ft.is_dir() && !ft.is_symlink() {
                stack.push(e.path());
            } else if ft.is_file() {
                count += 1;
                size += e.metadata().map(|m| m.len()).unwrap_or(0) as i64;
            }
        }
    }
    (size, count)
}
```

`scan()` 内 `self.db.upsert_mod(&character, &name, &rel)?;` 改为：

```rust
let id = self.db.upsert_mod(&character, &name, &rel)?;
let (size, count) = dir_stats(&mod_entry.path());
self.db.update_stats(id, size, count)?;
```

`Library` impl 加：

```rust
/// 重命名仓库内 Mod（只动文件系统与 DB；Junction 重建由调用方负责）。
/// 校验失败/冲突时目录保持原样。
pub fn rename_mod(&self, id: i64, new_name: &str) -> Result<ModEntry> {
    if !is_valid_segment(new_name) {
        return Err(crate::error::LiquiModError::InvalidName(new_name.into()));
    }
    let entry = self.db.get_mod(id)?;
    if entry.name == new_name {
        return Ok(entry);
    }
    if self.db.name_taken(&entry.character, new_name, id)? {
        return Err(crate::error::LiquiModError::DestinationExists {
            character: entry.character.clone(),
            name: new_name.into(),
        });
    }
    let old_dir = self.layout.root.join(&entry.rel_path);
    let new_rel = format!("mods/{}/{}", entry.character, new_name);
    let new_dir = self.layout.root.join(&new_rel);
    std::fs::rename(&old_dir, &new_dir)?;
    if let Err(e) = self.db.rename_mod(id, new_name, &new_rel) {
        let _ = std::fs::rename(&new_dir, &old_dir); // DB 失败回滚目录
        return Err(e);
    }
    self.db.get_mod(id)
}
```

- [ ] **Step 6: 跑测试确认全绿**

Run: `cargo test -p liquimod-core && cargo clippy -p liquimod-core --all-targets`
Expected: 全过（含既有 97+ 个），clippy 净

- [ ] **Step 7: Commit**

```bash
git add crates/liquimod-core
git commit -m "feat(core): mods 表统计列迁移 + dir_stats + Library::rename_mod"
```

---

### Task 2: app — rename/read_log/set_auto_enable 命令 + 安装自动启用 + tracing 日志 + 启动 recover

**Files:**
- Modify: `app/src-tauri/Cargo.toml`（tracing 三件套）
- Modify: `app/src-tauri/src/config.rs`（auto_enable + log_dir）
- Modify: `app/src-tauri/src/commands.rs`（DTO 字段 + rename_entry/maybe_auto_enable/read_log_tail/set_auto_enable/install_mod 接线）
- Modify: `app/src-tauri/src/lib.rs`（tracing 初始化 + 启动 recover + 注册命令）
- Test: `commands.rs` / `config.rs` 底部 tests

- [ ] **Step 1: 失败测试**

`config.rs` tests 追加：

```rust
#[test]
fn auto_enable_defaults_false_and_roundtrips() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("config.json");
    // 旧配置（无 auto_enable 字段）反序列化默认 false
    std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
    let c: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(!c.auto_enable);
    let mut c = c;
    c.auto_enable = true;
    c.save_to(&path).unwrap();
    let c2: Config = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(c2.auto_enable);
}
```

`commands.rs` tests 追加：

```rust
#[test]
fn rename_entry_disabled_mod() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m = lib.add_folder(src.path(), "A", "old").unwrap();
    rename_entry(&lib, None, m.id, "new").unwrap();
    assert_eq!(lib.db.get_mod(m.id).unwrap().name, "new");
    assert!(lib.layout.mod_dir("A", "new").is_dir());
}

#[test]
fn rename_entry_enabled_rebuilds_junction() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m = lib.add_folder(src.path(), "A", "old").unwrap();
    let mods = tempfile::tempdir().unwrap();
    crate::commands::set_enabled(&lib, Some(mods.path()), m.id, true).unwrap();
    rename_entry(&lib, Some(mods.path()), m.id, "new").unwrap();
    assert!(junction::exists(&mods.path().join("A--new")).unwrap());
    assert!(!mods.path().join("A--old").exists());
    assert!(lib.db.get_mod(m.id).unwrap().enabled);
}

#[test]
fn rename_entry_conflict_keeps_everything() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m1 = lib.add_folder(src.path(), "A", "m1").unwrap();
    lib.add_folder(src.path(), "A", "m2").unwrap();
    let mods = tempfile::tempdir().unwrap();
    crate::commands::set_enabled(&lib, Some(mods.path()), m1.id, true).unwrap();
    let err = rename_entry(&lib, Some(mods.path()), m1.id, "m2").unwrap_err();
    assert!(err.contains("已存在同名 Mod"));
    // 冲突后：名字未变、junction 仍是旧的、仍启用
    assert_eq!(lib.db.get_mod(m1.id).unwrap().name, "m1");
    assert!(junction::exists(&mods.path().join("A--m1")).unwrap());
    assert!(lib.db.get_mod(m1.id).unwrap().enabled);
}

#[test]
fn maybe_auto_enable_deploys_when_on() {
    let tmp = tempfile::tempdir().unwrap();
    let lib = Library::init(tmp.path()).unwrap();
    let src = tempfile::tempdir().unwrap();
    std::fs::write(src.path().join("mod.ini"), b"x").unwrap();
    let m = lib.add_folder(src.path(), "A", "m1").unwrap();
    let mods = tempfile::tempdir().unwrap();
    let mut c = Config {
        library_root: tmp.path().to_path_buf(),
        mods_dir: Some(mods.path().to_path_buf()),
        auto_enable: false,
    };
    maybe_auto_enable(&lib, &c, m.id, None);
    assert!(!lib.db.get_mod(m.id).unwrap().enabled); // 关：不动
    c.auto_enable = true;
    maybe_auto_enable(&lib, &c, m.id, None);
    assert!(lib.db.get_mod(m.id).unwrap().enabled); // 开：部署
}

#[test]
fn read_log_tail_truncates() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("logs");
    std::fs::create_dir_all(&dir).unwrap();
    let body: String = (0..300).map(|i| format!("line {i}\n")).collect();
    std::fs::write(dir.join("liquimod.log.2026-08-18"), body).unwrap();
    let s = read_log_tail(&dir, 64 * 1024).unwrap();
    assert_eq!(s.lines().count(), 200);
    assert!(s.contains("line 299"));
    assert!(!s.contains("line 0\n"));
    assert_eq!(read_log_tail(&tmp.path().join("nope"), 1024).unwrap(), "（暂无日志）");
}
```

- [ ] **Step 2: 跑测试确认红**

Run: `cargo test -p liquimod-app`
Expected: FAIL（编译错误：auto_enable / rename_entry / maybe_auto_enable / read_log_tail 不存在）

- [ ] **Step 3: Cargo.toml 加依赖**

`app/src-tauri/Cargo.toml` `[dependencies]` 加：

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["fmt"] }
tracing-appender = "0.2"
```

- [ ] **Step 4: config.rs 加字段**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    pub mods_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_enable: bool,
}
```

impl 加：

```rust
/// 日志目录：%APPDATA%/LiquiMod/logs
pub fn log_dir() -> PathBuf {
    Self::config_path()
        .parent()
        .expect("配置路径应有父目录")
        .join("logs")
}
```

（`Config` 其它构造点如有字面量初始化需补 `auto_enable: false`——编译器会指出。）

- [ ] **Step 5: commands.rs DTO + 新函数**

`ConfigDto` 加 `pub auto_enable: bool`；`config_dto` 加 `auto_enable: c.auto_enable`。

`ModDto` 加 `pub size_bytes: i64, pub file_count: i64, pub path: String`；`ModRow` 加 `size_bytes: i64, file_count: i64`；`collect_mod_rows` map 里补 `size_bytes: m.size_bytes, file_count: m.file_count`；两处构造 `ModDto` 处（`mod_list` 与 `list_mods` 命令内）补：

```rust
size_bytes: m.size_bytes,
file_count: m.file_count,
path: m.dir.display().to_string(),
thumb,
```

新函数（放在 `remove_entry` 旁）：

```rust
/// 重命名：启用中则 拆 Junction → 改名 → 按新名重建。冲突时恢复原启用状态。
pub fn rename_entry(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
    new_name: &str,
) -> Result<(), String> {
    let entry = lib.db.get_mod(id).map_err(|e| e.to_string())?;
    if !entry.enabled {
        return lib
            .rename_mod(id, new_name)
            .map(|_| ())
            .map_err(|e| humanize_install_error(&e));
    }
    let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录")?;
    let dep = Deployer::new(lib, mods_dir);
    dep.disable(id).map_err(|e| e.to_string())?;
    if let Err(e) = lib.rename_mod(id, new_name) {
        let _ = dep.enable(id); // 改名失败，恢复旧 junction
        return Err(humanize_install_error(&e));
    }
    dep.enable(id).map_err(|e| e.to_string())?;
    tracing::info!("renamed mod {id} to {new_name}");
    Ok(())
}

/// 安装后自动启用（设置开启时）；失败仅告警，不否决安装。
pub fn maybe_auto_enable(
    lib: &Library,
    config: &Config,
    mod_id: i64,
    app: Option<&tauri::AppHandle>,
) {
    if !config.auto_enable {
        return;
    }
    let Some(dir) = &config.mods_dir else {
        return;
    };
    if let Err(e) = Deployer::new(lib, dir).enable(mod_id) {
        tracing::warn!("auto-enable failed for mod {mod_id}: {e}");
        if let Some(app) = app {
            let _ = app.emit("liquimod-toast", format!("自动启用失败：{e}"));
        }
    } else {
        tracing::info!("auto-enabled mod {mod_id}");
    }
}

/// 读最新滚动日志尾部（最多 max_bytes、最后 200 行）。
pub fn read_log_tail(log_dir: &Path, max_bytes: u64) -> Result<String, String> {
    let rd = match std::fs::read_dir(log_dir) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("（暂无日志）".into()),
        Err(e) => return Err(format!("读取日志目录失败：{e}")),
    };
    let mut files: Vec<_> = rd
        .flatten()
        .filter(|f| f.file_name().to_string_lossy().starts_with("liquimod.log"))
        .collect();
    files.sort_by_key(|f| f.metadata().and_then(|m| m.modified()).ok());
    let Some(latest) = files.last() else {
        return Ok("（暂无日志）".into());
    };
    let bytes = std::fs::read(latest.path()).map_err(|e| format!("读取日志失败：{e}"))?;
    let start = bytes.len().saturating_sub(max_bytes as usize);
    let text = String::from_utf8_lossy(&bytes[start..]);
    let lines: Vec<&str> = text.lines().collect();
    let keep = if lines.len() > 200 {
        &lines[lines.len() - 200..]
    } else {
        &lines[..]
    };
    Ok(keep.join("\n"))
}
```

commands.rs 顶部 use 需有 `crate::config::Config`、`tauri::Emitter`（emit）。没有则补。

- [ ] **Step 6: install_mod 接线自动启用**

`install_mod` 命令 spawn_blocking 前加：

```rust
let config_arc = std::sync::Arc::clone(&state.config);
```

闭包内 `if matches!(result, ...)` 块改为：

```rust
if let Ok(InstallResultDto::Installed { mod_id, .. }) = &result {
    let mod_id = *mod_id;
    let cfg = config_arc.lock().unwrap().clone();
    maybe_auto_enable(&lib, &cfg, mod_id, Some(&app2));
    tracing::info!("installed mod {mod_id}");
    drop(lib); // 先释放库锁，maybe_refresh_game 可能阻塞数分钟（UAC）
    maybe_refresh_game(&app2, &refresh);
}
```

（原块只在 Installed 时 drop(lib)+refresh；现在 drop 移进此分支——Installed 之外的分支 lib 随闭包结束自动释放，等价。）

- [ ] **Step 7: Tauri 薄命令 + 注册 + 日志初始化 + 启动 recover**

commands.rs 加：

```rust
#[tauri::command]
pub async fn rename_mod(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    let refresh = std::sync::Arc::clone(&state.refresh);
    tauri::async_runtime::spawn_blocking(move || {
        let mods_dir = state.config.lock().unwrap().mods_dir.clone();
        let lib = library.lock().unwrap();
        rename_entry(&lib, mods_dir.as_deref(), id, &name)?;
        drop(lib);
        maybe_refresh_game(&app, &refresh);
        Ok(())
    })
    .await
    .map_err(|e| format!("重命名任务失败：{e}"))?
}

#[tauri::command]
pub fn set_auto_enable(state: tauri::State<AppState>, enabled: bool) -> Result<ConfigDto, String> {
    let mut config = state.config.lock().unwrap();
    config.auto_enable = enabled;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("auto_enable = {enabled}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn read_log() -> Result<String, String> {
    read_log_tail(&crate::config::Config::log_dir(), 64 * 1024)
}
```

注意 `rename_mod` 里 `state` 是 `State<'_, AppState>`，spawn_blocking 要 'static——参照既有 `choose_mods_dir` 是同步命令不走 spawn_blocking。rename_mod 改为同步命令（文件操作小）：

```rust
#[tauri::command]
pub fn rename_mod(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    {
        let lib = state.library.lock().unwrap();
        rename_entry(&lib, mods_dir.as_deref(), id, &name)?;
    }
    maybe_refresh_game(&app, &state.refresh);
    Ok(())
}
```

`maybe_refresh_game` 既有签名若是 `(&AppHandle, &Arc<Mutex<Option<RefreshClient>>>)` 直接传 `&state.refresh`。

lib.rs `run()` 开头（Builder 之前）：

```rust
let log_dir = config::Config::log_dir();
std::fs::create_dir_all(&log_dir).ok();
let appender = tracing_appender::rolling::daily(&log_dir, "liquimod.log");
let (nb, guard) = tracing_appender::non_blocking(appender);
std::mem::forget(guard); // 驻留整个进程生命周期
tracing_subscriber::fmt()
    .with_writer(nb)
    .with_ansi(false)
    .with_max_level(tracing::Level::INFO)
    .init();
tracing::info!("LiquiMod starting");
```

lib.rs setup 内 `start_watcher(...)` 之前加启动恢复：

```rust
// 启动恢复：完成上次崩溃遗留的启停事务（op_log）
let state = app.state::<AppState>();
let mods_dir = state.config.lock().unwrap().mods_dir.clone();
if let Some(dir) = mods_dir {
    let lib = state.library.lock().unwrap();
    if let Err(e) = liquimod_core::deploy::Deployer::new(&lib, &dir).recover() {
        tracing::warn!("startup recover failed: {e}");
    }
}
```

`invoke_handler` 列表加：`commands::rename_mod, commands::set_auto_enable, commands::read_log,`

set_enabled/uninstall/apply_preset 的 Tauri 命令里各补一行 `tracing::info!`（如 `info!("set mod {id} enabled={enabled}")`）。

- [ ] **Step 8: 跑测试确认绿**

Run: `cargo test -p liquimod-app && cargo clippy -p liquimod-app --all-targets && cargo fmt --all`
Expected: 全过

- [ ] **Step 9: Commit**

```bash
git add app/src-tauri
git commit -m "feat(app): rename/read_log/set_auto_enable 命令 + 安装自动启用 + tracing 滚动日志 + 启动恢复"
```

---

### Task 3: 前端 — api.ts + ModRow 组件 + CharacterDetail 接线

**Files:**
- Modify: `app/src/lib/api.ts`
- Create: `app/src/lib/components/ModRow.svelte`
- Create: `app/src/lib/components/ModRow.test.ts`
- Modify: `app/src/lib/views/CharacterDetail.svelte`

- [ ] **Step 1: api.ts 更新（先改，测试依赖它）**

`ConfigDto` 加 `auto_enable: boolean`；`ModDto` 加 `size_bytes: number; file_count: number; path: string`。

mockMods 三条各补 `size_bytes: 12345678, file_count: 42, path: "C:/mock/Library/mods/Firefly/Summer Skin"`（path 各自名字）。`get_config` mock 补 `auto_enable: false`。

`call()` mock 分支加：

```ts
case "rename_mod": {
  const m = mockMods.find((x) => x.id === Number(args?.id));
  const n = String(args?.name ?? "").trim();
  if (!n) throw "名字不合法（不能为空，不能含 / \\，不能以空格或点结尾）";
  if (mockMods.some((x) => x.id !== m?.id && x.name === n)) throw `已存在同名 Mod：${n}`;
  if (m) m.name = n;
  return undefined as T;
}
case "set_auto_enable":
  return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: Boolean(args?.enabled) } as T;
case "read_log":
  return "2026-08-18T10:00:00 INFO LiquiMod starting\n2026-08-18T10:01:00 INFO installed mod 99" as T;
```

`api` 对象加：

```ts
renameMod: (id: number, name: string) => call<void>("rename_mod", { id, name }),
setAutoEnable: (enabled: boolean) => call<ConfigDto>("set_auto_enable", { enabled }),
readLog: () => call<string>("read_log"),
```

- [ ] **Step 2: 失败测试 ModRow.test.ts**

范式参照 `PresetMenu.test.ts`（@testing-library/svelte，render + fireEvent）：

```ts
import { render, fireEvent, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModRow from "./ModRow.svelte";
import type { ModDto } from "$lib/api";

const mod: ModDto = {
  id: 1,
  name: "Summer Skin",
  enabled: false,
  installed_at: new Date(2026, 7, 12).getTime() / 1000,
  thumb: null,
  size_bytes: 12.5 * 1024 * 1024,
  file_count: 42,
  path: "C:/mock/m",
};

function setup(overrides = {}) {
  const props = {
    mod: { ...mod },
    ontoggle: vi.fn(),
    onrename: vi.fn(async () => true),
    onuninstall: vi.fn(async () => {}),
    onopen: vi.fn(),
    ...overrides,
  };
  render(ModRow, { props });
  return props;
}

describe("ModRow", () => {
  it("显示名字与副行信息", () => {
    setup();
    screen.getByText("Summer Skin");
    screen.getByText(/12\.5 MB · 42 文件 · 8月12日/);
  });

  it("统计缺失时显示 —", () => {
    setup({ mod: { ...mod, size_bytes: -1, file_count: -1 } });
    screen.getByText(/— · — 文件/);
  });

  it("空格键切换启停", async () => {
    const p = setup();
    await fireEvent.keyDown(screen.getByRole("listitem"), { key: " " });
    expect(p.ontoggle).toHaveBeenCalledWith(true);
  });

  it("打开按钮回调", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("打开目录 Summer Skin"));
    expect(p.onopen).toHaveBeenCalled();
  });

  it("重命名：编辑→Enter 提交成功关闭", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "New Name" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(p.onrename).toHaveBeenCalledWith("New Name"));
    await waitFor(() => expect(screen.queryByDisplayValue("New Name")).toBeNull());
  });

  it("重命名失败保持编辑态", async () => {
    const p = setup({ onrename: vi.fn(async () => false) });
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.input(input, { target: { value: "X" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => expect(p.onrename).toHaveBeenCalled());
    screen.getByDisplayValue("X"); // 仍在编辑
  });

  it("Esc 取消重命名", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("重命名 Summer Skin"));
    const input = screen.getByDisplayValue("Summer Skin");
    await fireEvent.keyDown(input, { key: "Escape" });
    expect(screen.queryByDisplayValue("Summer Skin")).toBeNull();
    expect(p.onrename).not.toHaveBeenCalled();
  });

  it("卸载需二次确认", async () => {
    const p = setup();
    await fireEvent.click(screen.getByLabelText("卸载 Summer Skin"));
    screen.getByText(/确认卸载/);
    await fireEvent.click(screen.getByText("取消"));
    expect(p.onuninstall).not.toHaveBeenCalled();
    await fireEvent.click(screen.getByLabelText("卸载 Summer Skin"));
    await fireEvent.click(screen.getByText("确认卸载"));
    await waitFor(() => expect(p.onuninstall).toHaveBeenCalled());
  });
});
```

- [ ] **Step 3: 跑测试确认红**

Run: `cd app; npx vitest run src/lib/components/ModRow.test.ts`
Expected: FAIL（ModRow 不存在）

- [ ] **Step 4: ModRow.svelte（UI 定稿，逐字转录）**

```svelte
<script lang="ts">
  import type { ModDto } from "$lib/api";
  import Toggle from "./Toggle.svelte";

  let {
    mod,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
  }: {
    mod: ModDto;
    ontoggle: (next: boolean) => void;
    onrename: (name: string) => Promise<boolean>;
    onuninstall: () => Promise<void>;
    onopen: () => void;
  } = $props();

  let renaming = $state(false);
  let draft = $state("");
  let confirming = $state(false);
  let busy = $state(false);

  function fmtSize(b: number): string {
    if (b < 0) return "—";
    if (b < 1024) return `${b} B`;
    if (b < 1024 * 1024) return `${(b / 1024).toFixed(0)} KB`;
    if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
    return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  function fmtDate(ts: number): string {
    const d = new Date(ts * 1000);
    return `${d.getMonth() + 1}月${d.getDate()}日`;
  }

  function startRename() {
    draft = mod.name;
    renaming = true;
  }

  async function commitRename() {
    const v = draft.trim();
    if (!v || v === mod.name || busy) {
      renaming = false;
      return;
    }
    busy = true;
    const ok = await onrename(v);
    busy = false;
    if (ok) renaming = false;
  }

  async function confirmUninstall() {
    if (busy) return;
    busy = true;
    await onuninstall();
    busy = false;
    confirming = false;
  }

  function onRowKeydown(e: KeyboardEvent) {
    if (renaming || confirming) return;
    if (e.key !== " " && e.key !== "Enter") return;
    if ((e.target as HTMLElement).closest("button, input")) return;
    e.preventDefault();
    ontoggle(!mod.enabled);
  }
</script>

<div
  role="listitem"
  tabindex="0"
  aria-label={mod.name}
  class="group glass radius-card px-5 py-4 flex items-center gap-4 outline-none transition-shadow focus-visible:shadow-[inset_0_0_0_2px_var(--accent)]"
  onkeydown={onRowKeydown}
>
  {#if confirming}
    <div class="flex-1 flex items-center justify-between gap-3 min-w-0">
      <p class="text-sm truncate">
        确认卸载 <span class="font-medium">{mod.name}</span>？文件将被删除
      </p>
      <div class="flex items-center gap-2 shrink-0">
        <button
          class="radius-pill h-8 px-3.5 text-sm font-medium text-white cursor-pointer disabled:opacity-50"
          style="background: var(--danger)"
          disabled={busy}
          onclick={confirmUninstall}
        >
          确认卸载
        </button>
        <button
          class="glass radius-pill h-8 px-3.5 text-sm cursor-pointer"
          onclick={() => (confirming = false)}
        >
          取消
        </button>
      </div>
    </div>
  {:else}
    {#if mod.thumb}
      <img
        src={mod.thumb}
        alt=""
        class="w-[72px] h-[72px] rounded-[14px] object-cover shrink-0"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
        draggable="false"
        onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")}
      />
    {:else}
      <div
        class="w-[72px] h-[72px] rounded-[14px] shrink-0 grid place-items-center text-xl font-semibold text-secondary"
        style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke); background: var(--glass-tint)"
      >
        {mod.name.slice(0, 1)}
      </div>
    {/if}

    <div class="flex-1 min-w-0">
      {#if renaming}
        <input
          bind:value={draft}
          aria-label={`新名字 ${mod.name}`}
          class="w-full h-8 px-3 text-sm font-medium bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 1.5px var(--accent)"
          onkeydown={(e) => {
            if (e.key === "Enter") commitRename();
            else if (e.key === "Escape") renaming = false;
          }}
          onblur={commitRename}
          autofocus
        />
      {:else}
        <p class="font-medium truncate">{mod.name}</p>
        <p class="text-xs text-secondary mt-0.5">
          {fmtSize(mod.size_bytes)} · {mod.file_count < 0 ? "—" : mod.file_count} 文件 · {fmtDate(mod.installed_at)}
        </p>
      {/if}
    </div>

    {#if !renaming}
      <div
        class="flex items-center gap-1.5 shrink-0 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
      >
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
          aria-label={`打开目录 ${mod.name}`}
          title="打开目录"
          onclick={onopen}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M1.5 3.5a1 1 0 0 1 1-1h2.6l1 1.2h5.4a1 1 0 0 1 1 1v5.8a1 1 0 0 1-1 1H2.5a1 1 0 0 1-1-1v-6Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
          </svg>
        </button>
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
          aria-label={`重命名 ${mod.name}`}
          title="重命名"
          onclick={startRename}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M8.6 2.2 10.8 4.4 4.7 10.5l-2.9.7.7-2.9 6.1-6.1Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
          </svg>
        </button>
        <button
          class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer transition-colors hover:text-white"
          style="--tw-text-opacity: 1"
          onmouseenter={(e) => ((e.currentTarget as HTMLElement).style.background = "var(--danger)")}
          onmouseleave={(e) => ((e.currentTarget as HTMLElement).style.background = "")}
          aria-label={`卸载 ${mod.name}`}
          title="卸载"
          onclick={() => (confirming = true)}
        >
          <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
            <path d="M2 3.5h9M5 3.5V2.3a.8.8 0 0 1 .8-.8h1.4a.8.8 0 0 1 .8.8v1.2M3.2 3.5l.5 7a1 1 0 0 0 1 .9h3.6a1 1 0 0 0 1-.9l.5-7" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
          </svg>
        </button>
      </div>
    {/if}

    <Toggle
      checked={mod.enabled}
      ariaLabel={`启用 ${mod.name}`}
      onchange={(next) => ontoggle(next)}
    />
  {/if}
</div>
```

- [ ] **Step 5: CharacterDetail.svelte 接线**

imports 加 `import ModRow from "$lib/components/ModRow.svelte";`、`isTauri` 已从 api import（检查现有 import 行补上）。

script 加：

```ts
async function renameMod(mod: ModDto, name: string): Promise<boolean> {
  error = "";
  try {
    await api.renameMod(mod.id, name);
    mod.name = name;
    return true;
  } catch (e) {
    error = String(e);
    return false;
  }
}

async function uninstallMod(mod: ModDto) {
  error = "";
  try {
    await api.uninstallMod(mod.id);
    mods = mods.filter((m) => m.id !== mod.id);
  } catch (e) {
    error = String(e);
    throw e; // 让 ModRow 的 confirming 态复位
  }
}

async function openModDir(mod: ModDto) {
  if (!isTauri()) return;
  try {
    const { openPath } = await import("@tauri-apps/plugin-opener");
    await openPath(mod.path);
  } catch (e) {
    error = String(e);
  }
}
```

模板 `{#each mods as mod (mod.id)}` 块内整段替换为：

```svelte
<ModRow
  {mod}
  ontoggle={(next) => toggle(mod, next)}
  onrename={(name) => renameMod(mod, name)}
  onuninstall={() => uninstallMod(mod)}
  onopen={() => openModDir(mod)}
/>
```

（删去旧的 glass div + Toggle 直写代码。）

注意：`mods.filter` 重赋值后 `mods.length` 副标题自动更新（$state）。uninstallMod 里 catch 后 throw 会让 ModRow 的 busy 复位但 confirming 保持——改为不 throw，ModRow confirmUninstall 里 finally 复位：

ModRow.confirmUninstall 用 try/finally：

```ts
async function confirmUninstall() {
  if (busy) return;
  busy = true;
  try {
    await onuninstall();
    confirming = false;
  } finally {
    busy = false;
  }
}
```

且 CharacterDetail.uninstallMod 不 throw（删去 `throw e` 行；error 已显示在顶部）。

- [ ] **Step 6: 跑测试确认绿**

Run: `cd app; npx vitest run; npm run check`
Expected: 全过（36+8 个）

- [ ] **Step 7: Commit**

```bash
git add app/src
git commit -m "feat(ui): ModRow 组件（72px 缩略图/副行信息/hover 操作/行内卸载确认与重命名/空格启停）"
```

---

### Task 4: 前端 — 主页 keep-alive 滚动保持 + 设置页扩充

**Files:**
- Modify: `app/src/routes/+page.svelte`
- Modify: `app/src/lib/views/Settings.svelte`
- Modify: `app/src/lib/views/Settings.test.ts`（若存在则加用例；不存在则新建）

- [ ] **Step 1: +page.svelte keep-alive（B1 修复）**

根因：主页被 `{:else}` 条件销毁，滚动位置丢失。改为常驻 + `hidden`：

`{:else}` 整段（header + CharacterGrid）改为：

```svelte
<div class:hidden={showSettings || selected !== null} class="flex flex-col flex-1 min-h-0">
  <header class="flex items-end justify-between px-8 pt-3 pb-5 shrink-0">
    <div>
      <h1 class="text-[34px] leading-tight font-bold tracking-tight">角色</h1>
      <p class="text-sm text-secondary mt-0.5">{characters.length} 位角色 · {modTotal} 个 Mod</p>
    </div>
    <div class="flex items-center gap-2.5">
      <PresetMenu onapplied={refresh} />
      <SearchBar bind:value={query} />
    </div>
  </header>
  <CharacterGrid {characters} {query} onselect={(c) => (selected = c)} />
</div>
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
  <CharacterDetail
    character={selected}
    modsDirConfigured={config?.mods_dir != null}
    onback={() => {
      selected = null;
      refresh();
    }}
    onconfigured={refresh}
  />
{/if}
```

（`{#if showSettings}` 提前到主页 div 之后；主页组件常驻 → 滚动容器 DOM 不销毁 → scrollTop 自然保留，搜索词也保留。Svelte 5 支持 `class:` 与 `class` 共存。）

- [ ] **Step 2: Settings 失败测试**

`Settings.test.ts` 已有则追加（参照其现有 mock 方式），否则新建：

```ts
import { render, fireEvent, screen, waitFor } from "@testing-library/svelte";
import { describe, expect, it, vi, beforeEach } from "vitest";
import Settings from "./Settings.svelte";

const mocks = vi.hoisted(() => ({
  listPasswords: vi.fn(async () => [] as string[]),
  setAutoEnable: vi.fn(async () => ({})),
  readLog: vi.fn(async () => "INFO hello log"),
}));
vi.mock("$lib/api", async (importOriginal) => {
  const orig = await importOriginal<typeof import("$lib/api")>();
  return {
    ...orig,
    api: {
      ...orig.api,
      listPasswords: mocks.listPasswords,
      setAutoEnable: mocks.setAutoEnable,
      readLog: mocks.readLog,
    },
  };
});

const config = { library_root: "C:/L", mods_dir: null, auto_enable: false };

describe("Settings 行为与日志", () => {
  beforeEach(() => vi.clearAllMocks());

  it("自动启用开关调用 setAutoEnable", async () => {
    render(Settings, { props: { config, onback: vi.fn(), onchanged: vi.fn() } });
    await fireEvent.click(screen.getByRole("switch", { name: "自动启用" }));
    expect(mocks.setAutoEnable).toHaveBeenCalledWith(true);
  });

  it("日志区加载并刷新", async () => {
    render(Settings, { props: { config, onback: vi.fn(), onchanged: vi.fn() } });
    await waitFor(() => screen.getByText(/hello log/));
    await fireEvent.click(screen.getByText("刷新"));
    expect(mocks.readLog).toHaveBeenCalledTimes(2);
  });
});
```

- [ ] **Step 3: 跑测试确认红**

Run: `cd app; npx vitest run src/lib/views/Settings.test.ts`
Expected: FAIL

- [ ] **Step 4: Settings.svelte 加两区**

imports 加 `import Toggle from "$lib/components/Toggle.svelte";`。

script 加：

```ts
let logText = $state("");

onMount(async () => {
  try {
    logText = await api.readLog();
  } catch {
    logText = "";
  }
});

async function toggleAutoEnable(next: boolean) {
  try {
    await api.setAutoEnable(next);
    onchanged();
  } catch (e) {
    toast(String(e));
  }
}

async function refreshLog() {
  try {
    logText = await api.readLog();
  } catch (e) {
    toast(String(e));
  }
}

async function copyLog() {
  try {
    await navigator.clipboard.writeText(logText);
    toast("日志已复制");
  } catch {
    toast("复制失败");
  }
}
```

（既有 onMount 的 listPasswords 保留；两个 onMount 可合并为一个。）

模板：「解压密码本」section 之后加：

```svelte
<section class="glass radius-panel p-5 flex items-center justify-between">
  <div>
    <h3 class="text-sm font-semibold text-secondary">行为</h3>
    <p class="text-sm font-medium mt-1">自动启用</p>
    <p class="text-xs text-secondary">安装成功后立即部署到 Mods 目录</p>
  </div>
  <Toggle checked={config?.auto_enable ?? false} ariaLabel="自动启用" onchange={toggleAutoEnable} />
</section>

<section class="glass radius-panel p-5 flex flex-col gap-3">
  <div class="flex items-center justify-between">
    <h3 class="text-sm font-semibold text-secondary">日志</h3>
    <div class="flex gap-2">
      <button class="glass radius-pill h-7 px-3 text-xs cursor-pointer" onclick={refreshLog}>刷新</button>
      <button class="glass radius-pill h-7 px-3 text-xs cursor-pointer" onclick={copyLog}>复制</button>
    </div>
  </div>
  <pre
    class="text-xs font-mono rounded-xl p-3 max-h-48 overflow-auto whitespace-pre-wrap break-all select-text"
    style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
  >{logText || "（暂无日志）"}</pre>
</section>
```

- [ ] **Step 5: 跑测试确认绿**

Run: `cd app; npx vitest run; npm run check`
Expected: 全过

- [ ] **Step 6: Commit**

```bash
git add app/src
git commit -m "feat(ui): 主页 keep-alive 保滚动 + 设置页自动启用开关与日志查看"
```

---

### Task 5: E2E 验证 + 终审

**Files:** 无新代码（除非修复）

- [ ] **Step 1: 全量测试 + 构建 exe**

```bash
cargo test --workspace
cd app; npm test; npm run check; cd ..
Stop-Process -Name liquimod-app -Force -ErrorAction SilentlyContinue
cd app; npm run build; cd ..
cargo build --release --features tauri/custom-protocol --manifest-path app\src-tauri\Cargo.toml
```

Expected: 全绿；exe 更新

- [ ] **Step 2: CDP E2E（主模型亲自执行）**

启动：`$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"; Start-Process target\release\liquimod-app.exe`，用 `%LOCALAPPDATA%\Temp\opencode\cdpeval.mjs` / `cdpshot.mjs`：

1. **B1 回归**：主页滚到 800 → 进 Firefly → 返回 → scrollTop ≈ 800（不是 0）
2. **Mod 行操作**：Firefly 详情行 hover 出现三按钮（截图）；打开目录按钮存在
3. **重命名**：行内改名 liquimod-test-firefly → "流萤测试" → 刷新后名字保持（DB+目录都改）
4. **空格启停**：聚焦行按 Space → 开关切换（mods_dir 未配置则应有错误提示文案——属正确路径）
5. **卸载确认**：点卸载出行内确认 → 取消 → 行恢复（不真删演示 Mod）
6. **设置页**：自动启用开关切换 → 读 %APPDATA%\LiquiMod\config.json 确认 auto_enable 持久化；日志区有内容（启动日志）
7. 副行信息（大小/文件数/日期）显示

- [ ] **Step 3: 双阶段终审（spec 审查 + 质量审查子代理）**

对照设计文档 `docs/superpowers/specs/2026-08-18-liquimod-usability-design.md` 逐条核；修 Important 及以上。

- [ ] **Step 4: Commit 收尾 + 向主人交付**

---

## Self-Review 记录

- Spec §2.1 B1/B3 ✓（Task 4/3）；§2.2 三操作 ✓（Task 3；打开目录用已授权的 openPath 而非 revealItemInDir，免改 capability）；§2.3 信息密度 ✓（Task 1/2/3）；§2.4 自动启用 ✓（Task 2/4）；§2.5 日志 ✓（Task 2/4）；§2.6 启动 recover ✓（Task 2）。
- 类型一致性：ModDto 新字段 `size_bytes/file_count/path` 在 Task 2（Rust）与 Task 3（TS）同名；`rename_mod` 命令名两端一致；`onrename -> Promise<boolean>` 契约在 ModRow 与 CharacterDetail 一致；`confirmUninstall` try/finally 版以 Task 3 Step 5 修正版为准（实现者两处都看到，Step 5 明确说"改为不 throw"）。
- `Config` 字面量构造点（state.rs bootstrap 等）加字段后编译器会逼出所有点——已在 Step 4 注明。
- rename_mod 用同步命令（避免 State 生命周期问题）已在 Step 7 修正，测试直接调 `rename_entry` 纯函数不受影响。
