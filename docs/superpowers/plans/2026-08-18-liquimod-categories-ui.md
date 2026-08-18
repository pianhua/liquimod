# LiquiMod 里程碑 8 实施计划：自定义分类 + 全新布局 + 亮色主题

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 自定义分类体系（左侧边栏导航）+ 全新双栏布局 + 亮色/暗色主题切换 + 角色卡信号灯 + 预设遮挡修复 + 全面中文化。

**Architecture:** 分类纯 DB（`categories` 表 + `mods.category_id`），磁盘目录不动；「角色」是 category_id IS NULL 的虚拟视图。前端 `view` 联合类型驱动内容区，Sidebar/Toolbar/ModCardGrid 新组件，CSS 变量 `data-theme` 双主题。

**Tech Stack:** Rust (rusqlite) / Tauri 2 / Svelte 5 (runes) / Tailwind 4。

**设计文档:** `docs/superpowers/specs/2026-08-18-liquimod-categories-ui-design.md`

**既有事实（执行前必读，勿重复踩坑）：**
- 构建/测试/CDP 命令见 `AGENTS.md`；改前端文件一律用 Edit 工具（PowerShell Set-Content 会毁 UTF-8 中文）。
- DB 迁移惯例：`ALTER TABLE ... ADD COLUMN` + 吞 "duplicate column" 错误（db.rs:62-70）。
- Tauri 命令参数 Rust snake_case → JS invoke 自动 camelCase（`category_id` → `categoryId`）。
- Svelte 行内编辑模式参考 ModRow.svelte（cancelled 标志防 Esc/blur 竞态，必须照搬）。
- `--glass-tint` 在 ModRow.svelte:125 被引用但 app.css 从未定义（既有 bug，Task 3 顺手修）。
- 预设遮挡根因：PresetMenu 所在 header 无定位/z-index，CharacterCard hover 的 transform 建立层叠上下文后压在面板上面。修复 = 工具条容器 `relative z-30`。
- CharacterGrid 网格行高依赖 ResizeObserver 显式计算（WebView2 引擎差异，见 AGENTS.md），ModCardGrid 用 `aspect-video` 卡片不受此限，但网格容器类名需含 `overflow-y-auto` 供滚动记忆查询。
- core 测试 111 + app 33 + 前端 48 为基线，每个 Task 完成后全绿才可提交。

---

### Task 1: core 分类数据模型

**Files:**
- Modify: `crates/liquimod-core/src/models.rs`
- Modify: `crates/liquimod-core/src/db.rs`
- Test: 同文件内 `#[cfg(test)]`

- [ ] **Step 1: models.rs 加 Category 结构 + ModEntry.category_id**

`models.rs` 改为：

```rust
#[derive(Debug, Clone, PartialEq)]
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
    /// 所属自定义分类；None = 角色视图（默认）
    pub category_id: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Preset {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub ord: i64,
    pub mod_count: i64,
}
```

- [ ] **Step 2: db.rs——categories 表 + category_id 迁移 + CRUD**

`init` 的 `execute_batch` 字符串内追加（presets 表之后）：

```sql
CREATE TABLE IF NOT EXISTS categories (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,
  ord INTEGER NOT NULL
);
```

迁移循环（db.rs:63 的 `for col in ["size_bytes", "file_count"]`）替换为：

```rust
        // 旧库迁移：补统计列与分类列（已存在则忽略 duplicate column 错误）
        for sql in [
            "ALTER TABLE mods ADD COLUMN size_bytes INTEGER NOT NULL DEFAULT -1",
            "ALTER TABLE mods ADD COLUMN file_count INTEGER NOT NULL DEFAULT -1",
            "ALTER TABLE mods ADD COLUMN category_id INTEGER REFERENCES categories(id)",
        ] {
            match conn.execute_batch(sql) {
                Ok(()) => {}
                Err(e) if e.to_string().contains("duplicate column") => {}
                Err(e) => return Err(e.into()),
            }
        }
```

`row_to_entry` 加 `category_id: r.get(8)?`，`list_mods` 与 `get_mod` 的 SELECT 列清单尾部追加 `, category_id`。`use crate::models::{Category, ModEntry, Preset};`。

`impl Database` 追加方法：

```rust
    fn validate_category_name(name: &str) -> Result<&str> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LiquiModError::InvalidName("分类名不能为空".into()));
        }
        Ok(name)
    }

    pub fn create_category(&self, name: &str) -> Result<i64> {
        let name = Self::validate_category_name(name)?;
        let ord: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(ord), 0) + 1 FROM categories",
            [],
            |r| r.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO categories (name, ord) VALUES (?1, ?2)",
            rusqlite::params![name, ord],
        ).map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                LiquiModError::InvalidName(format!("分类已存在：{name}"))
            }
            other => LiquiModError::Db(other),
        })?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn rename_category(&self, id: i64, name: &str) -> Result<()> {
        let name = Self::validate_category_name(name)?;
        let n = self.conn.execute(
            "UPDATE categories SET name = ?2 WHERE id = ?1",
            rusqlite::params![id, name],
        ).map_err(|e| match e {
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                LiquiModError::InvalidName(format!("分类已存在：{name}"))
            }
            other => LiquiModError::Db(other),
        })?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        }
        Ok(())
    }

    /// 删除分类：其中 Mod 全部移回角色视图（category_id = NULL）。
    pub fn delete_category(&self, id: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE mods SET category_id = NULL WHERE category_id = ?1",
            rusqlite::params![id],
        )?;
        let n = tx.execute("DELETE FROM categories WHERE id = ?1", rusqlite::params![id])?;
        tx.commit()?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        }
        Ok(())
    }

    /// 与相邻分类交换 ord（delta = ±1）；越界则不动。
    pub fn move_category(&self, id: i64, delta: i64) -> Result<()> {
        let mut ordered = self.list_categories()?;
        let Some(i) = ordered.iter().position(|c| c.id == id) else {
            return Err(LiquiModError::ModNotFound(format!("分类 {id}")));
        };
        let j = i as i64 + delta;
        if j < 0 || j as usize >= ordered.len() {
            return Ok(());
        }
        let (a, b) = (ordered[i].clone(), ordered[j as usize].clone());
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE categories SET ord = ?2 WHERE id = ?1", rusqlite::params![a.id, b.ord])?;
        tx.execute("UPDATE categories SET ord = ?2 WHERE id = ?1", rusqlite::params![b.id, a.ord])?;
        tx.commit()?;
        ordered.clear();
        Ok(())
    }

    pub fn list_categories(&self) -> Result<Vec<Category>> {
        let mut stmt = self.conn.prepare(
            "SELECT c.id, c.name, c.ord,
                    (SELECT COUNT(*) FROM mods m WHERE m.category_id = c.id)
             FROM categories c ORDER BY c.ord",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(Category {
                id: r.get(0)?,
                name: r.get(1)?,
                ord: r.get(2)?,
                mod_count: r.get(3)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<Vec<_>, _>>()?)
    }

    pub fn set_mod_category(&self, mod_id: i64, category_id: Option<i64>) -> Result<()> {
        if let Some(cid) = category_id {
            let exists: i64 = self.conn.query_row(
                "SELECT COUNT(*) FROM categories WHERE id = ?1",
                rusqlite::params![cid],
                |r| r.get(0),
            )?;
            if exists == 0 {
                return Err(LiquiModError::ModNotFound(format!("分类 {cid}")));
            }
        }
        let n = self.conn.execute(
            "UPDATE mods SET category_id = ?2 WHERE id = ?1",
            rusqlite::params![mod_id, category_id],
        )?;
        if n == 0 {
            return Err(LiquiModError::ModNotFound(mod_id.to_string()));
        }
        Ok(())
    }
```

- [ ] **Step 3: 测试（追加到 db.rs tests）**

```rust
    #[test]
    fn category_crud_and_mod_count() {
        let db = Database::open_in_memory().unwrap();
        let a = db.create_category("武器").unwrap();
        let b = db.create_category("光影").unwrap();
        let m = db.upsert_mod("Firefly", "Sword", "mods/Firefly/Sword").unwrap();
        db.set_mod_category(m, Some(a)).unwrap();
        let cats = db.list_categories().unwrap();
        assert_eq!(cats.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["武器", "光影"]);
        assert_eq!(cats[0].mod_count, 1);
        assert_eq!(cats[1].mod_count, 0);
        assert_eq!(db.get_mod(m).unwrap().category_id, Some(a));
        db.rename_category(b, "UI").unwrap();
        assert_eq!(db.list_categories().unwrap()[1].name, "UI");
        let _ = (a, b);
    }

    #[test]
    fn category_rejects_empty_and_duplicate() {
        let db = Database::open_in_memory().unwrap();
        db.create_category("武器").unwrap();
        assert!(matches!(
            db.create_category("武器"),
            Err(LiquiModError::InvalidName(_))
        ));
        assert!(matches!(
            db.create_category("  "),
            Err(LiquiModError::InvalidName(_))
        ));
    }

    #[test]
    fn delete_category_moves_mods_back_to_null() {
        let db = Database::open_in_memory().unwrap();
        let c = db.create_category("武器").unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        db.set_mod_category(m, Some(c)).unwrap();
        db.delete_category(c).unwrap();
        assert_eq!(db.get_mod(m).unwrap().category_id, None);
        assert!(db.list_categories().unwrap().is_empty());
        assert!(db.delete_category(c).is_err());
    }

    #[test]
    fn move_category_swaps_with_neighbor() {
        let db = Database::open_in_memory().unwrap();
        let a = db.create_category("A").unwrap();
        let b = db.create_category("B").unwrap();
        let c = db.create_category("C").unwrap();
        db.move_category(b, -1).unwrap();
        let names: Vec<String> = db.list_categories().unwrap().into_iter().map(|x| x.name).collect();
        assert_eq!(names, vec!["B", "A", "C"]);
        db.move_category(a, 1).unwrap(); // 到边界外，不动
        db.move_category(c, 1).unwrap();
        let names: Vec<String> = db.list_categories().unwrap().into_iter().map(|x| x.name).collect();
        assert_eq!(names, vec!["B", "A", "C"]);
    }

    #[test]
    fn set_mod_category_validates() {
        let db = Database::open_in_memory().unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        assert!(db.set_mod_category(m, Some(999)).is_err());
        db.set_mod_category(m, None).unwrap();
        assert!(db.set_mod_category(999, None).is_err());
    }

    #[test]
    fn migration_adds_category_column_to_old_db() {
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
        assert_eq!(db.get_mod(1).unwrap().category_id, None);
    }

    #[test]
    fn upsert_preserves_category_id() {
        // scan 的 upsert 只更新 rel_path，不得冲掉已归类
        let db = Database::open_in_memory().unwrap();
        let c = db.create_category("武器").unwrap();
        let m = db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        db.set_mod_category(m, Some(c)).unwrap();
        db.upsert_mod("A", "m1", "mods/A/m1").unwrap();
        assert_eq!(db.get_mod(m).unwrap().category_id, Some(c));
    }
```

- [ ] **Step 4: 验证 + 提交**

Run: `cargo test -p liquimod-core`（全绿）、`cargo clippy --workspace --all-targets`、`cargo fmt --all`
Commit: `feat(core): 分类数据模型——categories 表、mods.category_id、CRUD 与调序`

---

### Task 2: app 壳——config 主题字段 + 分类命令 + 视图过滤

**Files:**
- Modify: `app/src-tauri/src/config.rs`
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`（注册命令）
- Test: commands.rs / config.rs 内 tests

- [ ] **Step 1: config.rs 加 theme 与 character_category_name**

```rust
fn default_theme() -> String {
    "auto".into()
}
fn default_character_category_name() -> String {
    "角色".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    pub mods_dir: Option<PathBuf>,
    #[serde(default)]
    pub auto_enable: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_character_category_name")]
    pub character_category_name: String,
}
```

`load_from` 的 fallback 分支补 `theme: default_theme(), character_category_name: default_character_category_name()`。测试补：

```rust
    #[test]
    fn theme_and_category_name_default_and_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.json");
        std::fs::write(&path, r#"{"library_root":"C:/L","mods_dir":null}"#).unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.theme, "auto");
        assert_eq!(c.character_category_name, "角色");
        let mut c = c;
        c.theme = "light".into();
        c.character_category_name = "机体".into();
        c.save_to(&path).unwrap();
        let c2 = Config::load_from(&path);
        assert_eq!(c2.theme, "light");
        assert_eq!(c2.character_category_name, "机体");
    }
```

注意 config.rs 既有测试构造 `Config { library_root, mods_dir, auto_enable }` 字面量的两处（save_load_roundtrip、auto_enable_defaults_false_and_roundtrips 不涉及字面量；save_load_roundtrip 涉及）需补新字段——`..Default::default()` 不可用于非 Default 结构，直接补 `theme: "auto".into(), character_category_name: "角色".into()`。commands.rs 测试中两处 `Config { ... auto_enable: ... }` 字面量同样补齐。

- [ ] **Step 2: commands.rs——DTO 与新命令**

`ConfigDto` 加字段：

```rust
pub struct ConfigDto {
    pub library_root: String,
    pub mods_dir: Option<String>,
    pub auto_enable: bool,
    pub theme: String,
    pub character_category_name: String,
}
```

`config_dto` 补 `theme: c.theme.clone(), character_category_name: c.character_category_name.clone()`。

`ModDto` 加 `pub category_id: Option<i64>`；`ModRow` 结构加 `category_id: Option<i64>`。新增：

```rust
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CategoryDto {
    pub id: i64,
    pub name: String,
    pub ord: i64,
    pub mod_count: i64,
}
```

`character_summaries`：两处 filter 改为只统计未归类——`filter(|m| m.character == c.internal_name && m.category_id.is_none())` 与 `filter(|m| m.category_id.is_none() && !known.contains(&m.character.as_str()))`。

`collect_mod_rows` 的 filter 改为 `filter(|m| m.character == character && m.category_id.is_none())`，map 里补 `category_id: m.category_id`。新增按谓词收集的通用函数与 DTO 组装：

```rust
fn collect_rows_where(
    lib: &Library,
    pred: impl Fn(&liquimod_core::models::ModEntry) -> bool,
) -> Result<Vec<ModRow>, String> {
    let mut rows: Vec<ModRow> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| pred(m))
        .map(|m| {
            let dir = lib.layout.mod_dir(&m.character, &m.name);
            ModRow {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                size_bytes: m.size_bytes,
                file_count: m.file_count,
                category_id: m.category_id,
                dir,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

fn rows_to_dtos(root: &Path, rows: Vec<ModRow>) -> Vec<ModDto> {
    rows.into_iter()
        .map(|m| {
            let thumb = thumb_data_url(root, &m.dir, m.id);
            ModDto {
                id: m.id,
                name: m.name,
                enabled: m.enabled,
                installed_at: m.installed_at,
                thumb,
                size_bytes: m.size_bytes,
                file_count: m.file_count,
                path: m.dir.display().to_string(),
                category_id: m.category_id,
            }
        })
        .collect()
}
```

`collect_mod_rows` 改为调用 `collect_rows_where(lib, |m| m.character == character && m.category_id.is_none())`；`mod_list` 与 `list_mods` 命令内的 DTO 组装改用 `rows_to_dtos`。

新命令（追加到 commands.rs 末尾、tests 之前）：

```rust
#[tauri::command]
pub async fn list_categories(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<CategoryDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db
            .list_categories()
            .map_err(|e| e.to_string())
            .map(|cs| {
                cs.into_iter()
                    .map(|c| CategoryDto { id: c.id, name: c.name, ord: c.ord, mod_count: c.mod_count })
                    .collect()
            })
    })
    .await
    .map_err(|e| format!("读取分类失败：{e}"))?
}

#[tauri::command]
pub async fn create_category(
    state: tauri::State<'_, AppState>,
    name: String,
) -> Result<i64, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.create_category(&name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("新建分类失败：{e}"))?
}

#[tauri::command]
pub async fn rename_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.rename_category(id, &name).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("重命名分类失败：{e}"))?
}

#[tauri::command]
pub async fn delete_category(
    state: tauri::State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.delete_category(id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("删除分类失败：{e}"))?
}

#[tauri::command]
pub async fn move_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    delta: i64,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.move_category(id, delta).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移动分类失败：{e}"))?
}

#[tauri::command]
pub async fn set_mod_category(
    state: tauri::State<'_, AppState>,
    id: i64,
    category_id: Option<i64>,
) -> Result<(), String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let lib = library.lock().unwrap();
        lib.db.set_mod_category(id, category_id).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("移动 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn list_category_mods(
    state: tauri::State<'_, AppState>,
    category_id: i64,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = library.lock().unwrap();
            let root = lib.layout.root.clone();
            let rows = collect_rows_where(&lib, move |m| m.category_id == Some(category_id))?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取分类 Mod 失败：{e}"))?
}

#[tauri::command]
pub async fn list_all_mods(state: tauri::State<'_, AppState>) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = library.lock().unwrap();
            let root = lib.layout.root.clone();
            let rows = collect_rows_where(&lib, |_| true)?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取全部 Mod 失败：{e}"))?
}

/// 未分类 = 未归类（category_id NULL）且不属于任何已知游戏角色。
#[tauri::command]
pub async fn list_uncategorized_mods(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<ModDto>, String> {
    let library = std::sync::Arc::clone(&state.library);
    tauri::async_runtime::spawn_blocking(move || {
        let (root, rows) = {
            let lib = library.lock().unwrap();
            let root = lib.layout.root.clone();
            let known: Vec<&str> = Hsr::shared()
                .characters()
                .iter()
                .map(|c| c.internal_name.as_str())
                .collect();
            let rows = collect_rows_where(&lib, |m| {
                m.category_id.is_none() && !known.contains(&m.character.as_str())
            })?;
            (root, rows)
        };
        Ok(rows_to_dtos(&root, rows))
    })
    .await
    .map_err(|e| format!("读取未分类 Mod 失败：{e}"))?
}

#[tauri::command]
pub fn set_theme(state: tauri::State<AppState>, theme: String) -> Result<ConfigDto, String> {
    if !["auto", "light", "dark"].contains(&theme.as_str()) {
        return Err("主题只能是 auto / light / dark".to_string());
    }
    let mut config = state.config.lock().unwrap();
    config.theme = theme.clone();
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    tracing::info!("theme = {theme}");
    Ok(config_dto(&config))
}

#[tauri::command]
pub fn set_character_category_name(
    state: tauri::State<AppState>,
    name: String,
) -> Result<ConfigDto, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("名称不能为空".to_string());
    }
    let mut config = state.config.lock().unwrap();
    config.character_category_name = name;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(config_dto(&config))
}
```

`lib.rs` 的 `generate_handler!` 追加：`commands::list_categories, commands::create_category, commands::rename_category, commands::delete_category, commands::move_category, commands::set_mod_category, commands::list_category_mods, commands::list_all_mods, commands::list_uncategorized_mods, commands::set_theme, commands::set_character_category_name,`

- [ ] **Step 3: 测试（追加到 commands.rs tests）**

```rust
    #[test]
    fn summaries_exclude_categorized_mods() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        let m = lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        let c = lib.db.create_category("武器").unwrap();
        lib.db.set_mod_category(m.id, Some(c)).unwrap();
        let out = character_summaries(&lib, Hsr::shared()).unwrap();
        let acheron = out.iter().find(|x| x.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.total, 0);
        assert!(mod_list(&lib, "Acheron").unwrap().is_empty());
    }

    #[test]
    fn collect_rows_where_all_and_uncategorized() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        lib.add_folder(src.path(), "Stranger", "M2").unwrap();
        let all = collect_rows_where(&lib, |_| true).unwrap();
        assert_eq!(all.len(), 2);
        let known: Vec<&str> = Hsr::shared()
            .characters()
            .iter()
            .map(|c| c.internal_name.as_str())
            .collect();
        let uncat = collect_rows_where(&lib, |m| {
            m.category_id.is_none() && !known.contains(&m.character.as_str())
        })
        .unwrap();
        assert_eq!(uncat.len(), 1);
        assert_eq!(uncat[0].name, "M2");
    }
```

- [ ] **Step 4: 验证 + 提交**

Run: `cargo test --workspace`、`cargo clippy --workspace --all-targets`、`cargo fmt --all`
Commit: `feat(app): 分类命令组 + list_all/uncategorized + config 主题与角色分类名`

---

### Task 3: 主题系统——CSS 变量 data-theme 化 + 设置页外观区

**Files:**
- Modify: `app/src/app.css`
- Create: `app/src/lib/theme.ts`
- Modify: `app/src/lib/views/Settings.svelte`
- Modify: `app/src/lib/api.ts`（ConfigDto 字段 + setTheme/setCharacterCategoryName + mock get_config）
- Test: `app/src/lib/theme.test.ts`

- [ ] **Step 1: app.css——dark 变量挂 data-theme，补 --glass-tint**

把 `@media (prefers-color-scheme: dark) { :root { ... } }` 整块替换为：

```css
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --glass-bg: rgba(28, 30, 42, 0.38);
    --glass-stroke: rgba(255, 255, 255, 0.14);
    --glass-highlight: rgba(255, 255, 255, 0.16);
    --glass-tint: rgba(255, 255, 255, 0.06);
    --surface: #15161f;
    --text: #f2f2f7;
    --text-secondary: #98989f;
    --accent: #409cff;
    --accent-fill: rgba(64, 156, 255, 0.16);
    --danger: #ff453a;
    --blob-a: rgba(64, 156, 255, 0.13);
    --blob-b: rgba(191, 90, 242, 0.1);
    --shadow-soft:
      inset 0 0.5px 0 var(--glass-highlight),
      0 8px 24px rgba(0, 0, 0, 0.35),
      0 2px 6px rgba(0, 0, 0, 0.28);
    --shadow-lift:
      inset 0 0.5px 0 var(--glass-highlight),
      0 16px 44px rgba(0, 0, 0, 0.5),
      0 4px 10px rgba(0, 0, 0, 0.32);
  }
}

:root[data-theme="dark"] {
  --glass-bg: rgba(28, 30, 42, 0.38);
  --glass-stroke: rgba(255, 255, 255, 0.14);
  --glass-highlight: rgba(255, 255, 255, 0.16);
  --glass-tint: rgba(255, 255, 255, 0.06);
  --surface: #15161f;
  --text: #f2f2f7;
  --text-secondary: #98989f;
  --accent: #409cff;
  --accent-fill: rgba(64, 156, 255, 0.16);
  --danger: #ff453a;
  --blob-a: rgba(64, 156, 255, 0.13);
  --blob-b: rgba(191, 90, 242, 0.1);
  --shadow-soft:
    inset 0 0.5px 0 var(--glass-highlight),
    0 8px 24px rgba(0, 0, 0, 0.35),
    0 2px 6px rgba(0, 0, 0, 0.28);
  --shadow-lift:
    inset 0 0.5px 0 var(--glass-highlight),
    0 16px 44px rgba(0, 0, 0, 0.5),
    0 4px 10px rgba(0, 0, 0, 0.32);
}
```

`:root`（亮色）块内 `--glass-highlight` 后补一行：`--glass-tint: rgba(255, 255, 255, 0.5);`

- [ ] **Step 2: theme.ts**

```ts
export type ThemeChoice = "auto" | "light" | "dark";

export function resolveTheme(choice: string, systemDark: boolean): "light" | "dark" {
  if (choice === "dark") return "dark";
  if (choice === "light") return "light";
  return systemDark ? "dark" : "light";
}

let mediaHooked = false;

/// 按配置应用主题；auto 时跟随系统并监听系统切换（监听只挂一次）。
export function applyTheme(choice: string) {
  const mq = window.matchMedia("(prefers-color-scheme: dark)");
  document.documentElement.dataset.theme = resolveTheme(choice, mq.matches);
  if (!mediaHooked) {
    mediaHooked = true;
    mq.addEventListener("change", () => {
      // 仅 auto 模式跟随；锁定亮/暗时忽略系统变化——读当前已解析值无法区分，
      // 因此在 +page 里保存当前 choice。此处重新读 dataset 上的 choice 标记。
      const c = document.documentElement.dataset.themeChoice ?? "auto";
      if (c === "auto") {
        document.documentElement.dataset.theme = mq.matches ? "dark" : "light";
      }
    });
  }
  document.documentElement.dataset.themeChoice =
    choice === "light" || choice === "dark" ? choice : "auto";
}
```

- [ ] **Step 3: theme.test.ts**

```ts
import { describe, expect, it } from "vitest";
import { resolveTheme } from "./theme";

describe("resolveTheme", () => {
  it("锁定亮暗优先于系统", () => {
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });
  it("auto 跟随系统", () => {
    expect(resolveTheme("auto", true)).toBe("dark");
    expect(resolveTheme("auto", false)).toBe("light");
    expect(resolveTheme("未知值", true)).toBe("dark");
  });
});
```

- [ ] **Step 4: api.ts——ConfigDto 加字段 + 两个命令 + mock**

`ConfigDto` 加 `theme: string; character_category_name: string;`。mock `get_config` 返回补 `theme: "auto", character_category_name: "角色"`。`call` 的 switch 加：

```ts
      case "set_theme":
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: String(args?.theme ?? "auto"), character_category_name: "角色" } as T;
      case "set_character_category_name": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "名称不能为空";
        return { library_root: "C:/mock/Library", mods_dir: null, auto_enable: false, theme: "auto", character_category_name: n } as T;
      }
```

`api` 对象加：

```ts
  setTheme: (theme: string) => call<ConfigDto>("set_theme", { theme }),
  setCharacterCategoryName: (name: string) => call<ConfigDto>("set_character_category_name", { name }),
```

- [ ] **Step 5: Settings.svelte 加「外观」区（放在「目录」区之后）**

script 加：

```ts
  let catNameDraft = $state("");

  async function pickTheme(t: string) {
    try {
      const c = await api.setTheme(t);
      applyTheme(c.theme);
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }

  async function saveCatName() {
    const v = catNameDraft.trim();
    if (!v || v === config?.character_category_name) return;
    try {
      await api.setCharacterCategoryName(v);
      toast("已更新分类名称");
      onchanged();
    } catch (e) {
      toast(String(e));
    }
  }
```

import 加 `import { applyTheme } from "$lib/theme";`。onMount 里补 `catNameDraft = config?.character_category_name ?? "角色";`（注意 config 由父组件传入，onMount 时可能已就绪；用 `$effect(() => { if (config && !catNameDraft) catNameDraft = config.character_category_name; })` 更稳——用后者）。

模板在「目录」`</section>` 后插入：

```svelte
    <section class="glass radius-panel p-5 flex flex-col gap-3">
      <h3 class="text-sm font-semibold text-secondary">外观</h3>
      <div class="flex items-center justify-between gap-3">
        <p class="text-sm font-medium">主题</p>
        <div class="flex gap-1 p-0.5 radius-pill" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">
          {#each [["auto", "跟随系统"], ["light", "亮色"], ["dark", "暗色"]] as [value, label] (value)}
            <button
              class="radius-pill h-7 px-3 text-xs cursor-pointer transition-colors"
              class:accent-fill={config?.theme === value}
              class:accent-text={config?.theme === value}
              onclick={() => pickTheme(value)}
            >
              {label}
            </button>
          {/each}
        </div>
      </div>
      <div class="flex items-center justify-between gap-3">
        <div class="min-w-0">
          <p class="text-sm font-medium">角色分类名称</p>
          <p class="text-xs text-secondary">不同游戏叫法不同（如「机体」「干员」）</p>
        </div>
        <div class="flex gap-1.5 shrink-0">
          <input
            bind:value={catNameDraft}
            aria-label="角色分类名称"
            class="h-8 w-28 px-3 text-sm bg-transparent outline-none rounded-full"
            style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)"
            onkeydown={(e) => e.key === "Enter" && saveCatName()}
          />
          <button
            class="accent-fill accent-text radius-pill h-8 px-3.5 text-sm font-medium cursor-pointer disabled:opacity-50"
            disabled={!catNameDraft.trim() || catNameDraft.trim() === config?.character_category_name}
            onclick={saveCatName}
          >
            保存
          </button>
        </div>
      </div>
    </section>
```

- [ ] **Step 6: 验证 + 提交**

Run: `cd app; npx vitest run; npm run check; npm run build`
Commit: `feat(ui): 主题系统——data-theme 双主题变量、设置页外观区、角色分类名可改`

---

### Task 4: 布局骨架——Sidebar / Toolbar / view 状态机 + api 分类接口

**Files:**
- Create: `app/src/lib/view.ts`
- Create: `app/src/lib/components/Sidebar.svelte`
- Create: `app/src/lib/components/Toolbar.svelte`
- Modify: `app/src/lib/api.ts`（CategoryDto + 9 个命令 + mock）
- Rewrite: `app/src/routes/+page.svelte`
- Test: `app/src/lib/view.test.ts`、`app/src/lib/components/Sidebar.test.ts`

- [ ] **Step 1: api.ts——CategoryDto 与分类命令**

加类型：

```ts
export interface CategoryDto {
  id: number;
  name: string;
  ord: number;
  mod_count: number;
}
```

`ModDto` 加 `category_id: number | null;`。mockMods 三条各补 `category_id: null`（第三条 `category_id: 1`）。mock 分类数据与分支：

```ts
const mockCategories: CategoryDto[] = [
  { id: 1, name: "武器", ord: 1, mod_count: 1 },
  { id: 2, name: "光影", ord: 2, mod_count: 0 },
];
```

`call` switch 加：

```ts
      case "list_categories":
        return structuredClone(mockCategories) as T;
      case "create_category": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "分类名不能为空";
        if (mockCategories.some((c) => c.name === n)) throw `分类已存在：${n}`;
        const c = { id: Math.max(0, ...mockCategories.map((x) => x.id)) + 1, name: n, ord: mockCategories.length + 1, mod_count: 0 };
        mockCategories.push(c);
        return c.id as T;
      }
      case "rename_category": {
        const n = String(args?.name ?? "").trim();
        if (!n) throw "分类名不能为空";
        if (mockCategories.some((c) => c.name === n && c.id !== Number(args?.id))) throw `分类已存在：${n}`;
        const c = mockCategories.find((x) => x.id === Number(args?.id));
        if (!c) throw "分类不存在";
        c.name = n;
        return undefined as T;
      }
      case "delete_category": {
        const id = Number(args?.id);
        const i = mockCategories.findIndex((x) => x.id === id);
        if (i < 0) throw "分类不存在";
        mockCategories.splice(i, 1);
        for (const m of mockMods) if (m.category_id === id) m.category_id = null;
        return undefined as T;
      }
      case "move_category": {
        const id = Number(args?.id);
        const delta = Number(args?.delta);
        const sorted = [...mockCategories].sort((a, b) => a.ord - b.ord);
        const i = sorted.findIndex((x) => x.id === id);
        const j = i + delta;
        if (i >= 0 && j >= 0 && j < sorted.length) {
          const t = sorted[i].ord;
          sorted[i].ord = sorted[j].ord;
          sorted[j].ord = t;
        }
        return undefined as T;
      }
      case "set_mod_category": {
        const m = mockMods.find((x) => x.id === Number(args?.id));
        if (!m) throw "Mod 不存在";
        const cid = args?.categoryId == null ? null : Number(args.categoryId);
        if (cid !== null && !mockCategories.some((c) => c.id === cid)) throw "分类不存在";
        m.category_id = cid;
        for (const c of mockCategories)
          c.mod_count = mockMods.filter((x) => x.category_id === c.id).length;
        return undefined as T;
      }
      case "list_category_mods":
        return structuredClone(mockMods.filter((m) => m.category_id === Number(args?.categoryId))) as T;
      case "list_all_mods":
        return structuredClone(mockMods) as T;
      case "list_uncategorized_mods":
        return [] as T;
```

`api` 对象加：

```ts
  listCategories: () => call<CategoryDto[]>("list_categories"),
  createCategory: (name: string) => call<number>("create_category", { name }),
  renameCategory: (id: number, name: string) => call<void>("rename_category", { id, name }),
  deleteCategory: (id: number) => call<void>("delete_category", { id }),
  moveCategory: (id: number, delta: number) => call<void>("move_category", { id, delta }),
  setModCategory: (id: number, categoryId: number | null) =>
    call<void>("set_mod_category", { id, categoryId }),
  listCategoryMods: (categoryId: number) => call<ModDto[]>("list_category_mods", { categoryId }),
  listAllMods: () => call<ModDto[]>("list_all_mods"),
  listUncategorizedMods: () => call<ModDto[]>("list_uncategorized_mods"),
```

- [ ] **Step 2: view.ts（视图状态 + 纯函数）**

```ts
import type { ModDto } from "$lib/api";

export type View =
  | { kind: "home" }
  | { kind: "all" }
  | { kind: "uncat" }
  | { kind: "category"; id: number; name: string }
  | { kind: "character"; name: string; display: string };

export type ModSort = "recent" | "name" | "enabled";

export function viewKey(v: View): string {
  switch (v.kind) {
    case "home":
      return "home";
    case "all":
      return "all";
    case "uncat":
      return "uncat";
    case "category":
      return `cat:${v.id}`;
    case "character":
      return `char:${v.name}`;
  }
}

export function filterMods(mods: ModDto[], query: string): ModDto[] {
  const q = query.trim().toLowerCase();
  if (!q) return mods;
  return mods.filter((m) => m.name.toLowerCase().includes(q));
}

export function sortMods(mods: ModDto[], sort: ModSort): ModDto[] {
  const arr = [...mods];
  switch (sort) {
    case "recent":
      return arr.sort((a, b) => b.installed_at - a.installed_at);
    case "name":
      return arr.sort((a, b) => a.name.localeCompare(b.name, "zh-Hans-CN"));
    case "enabled":
      return arr.sort(
        (a, b) => Number(b.enabled) - Number(a.enabled) || a.name.localeCompare(b.name, "zh-Hans-CN"),
      );
  }
}
```

- [ ] **Step 3: view.test.ts**

```ts
import { describe, expect, it } from "vitest";
import { filterMods, sortMods, viewKey, type View } from "./view";
import type { ModDto } from "./api";

function mod(id: number, name: string, enabled: boolean, installed_at: number): ModDto {
  return { id, name, enabled, installed_at, thumb: null, size_bytes: 0, file_count: 0, path: "", category_id: null };
}

describe("viewKey", () => {
  it("每种视图唯一", () => {
    const keys = [
      viewKey({ kind: "home" }),
      viewKey({ kind: "all" }),
      viewKey({ kind: "uncat" }),
      viewKey({ kind: "category", id: 1, name: "A" }),
      viewKey({ kind: "category", id: 2, name: "A" }),
      viewKey({ kind: "character", name: "Firefly", display: "流萤" }),
    ];
    expect(new Set(keys).size).toBe(keys.length);
  });
});

describe("filterMods", () => {
  it("按名称不区分大小写过滤", () => {
    const mods = [mod(1, "Summer Skin", false, 0), mod(2, "战斗特效", false, 0)];
    expect(filterMods(mods, "summer").map((m) => m.id)).toEqual([1]);
    expect(filterMods(mods, "战斗").map((m) => m.id)).toEqual([2]);
    expect(filterMods(mods, "")).toHaveLength(2);
  });
});

describe("sortMods", () => {
  const mods = [
    mod(1, "B", false, 100),
    mod(2, "A", true, 50),
    mod(3, "C", true, 200),
  ];
  it("recent 按安装时间倒序", () => {
    expect(sortMods(mods, "recent").map((m) => m.id)).toEqual([3, 1, 2]);
  });
  it("name 按名称", () => {
    expect(sortMods(mods, "name").map((m) => m.id)).toEqual([2, 1, 3]);
  });
  it("enabled 启用优先再按名称", () => {
    expect(sortMods(mods, "enabled").map((m) => m.id)).toEqual([2, 3, 1]);
  });
  it("不改变原数组", () => {
    sortMods(mods, "recent");
    expect(mods.map((m) => m.id)).toEqual([1, 2, 3]);
  });
});
```

- [ ] **Step 4: Sidebar.svelte**

```svelte
<script lang="ts">
  import { api, type CategoryDto } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import type { View } from "$lib/view";
  import SearchBar from "./SearchBar.svelte";

  let {
    view,
    categories,
    charCatName,
    allCount,
    charCount,
    uncatCount,
    query = $bindable(),
    onnavigate,
    onchanged,
  }: {
    view: View;
    categories: CategoryDto[];
    charCatName: string;
    allCount: number;
    charCount: number;
    uncatCount: number;
    query: string;
    onnavigate: (v: View) => void;
    onchanged: () => void;
  } = $props();

  let creating = $state(false);
  let newName = $state("");
  let renamingId = $state<number | null>(null);
  let renameDraft = $state("");
  let renameCancelled = $state(false);
  let menuFor = $state<number | null>(null);
  let confirmingDelete = $state<number | null>(null);
  let busy = $state(false);

  function isActive(key: string): boolean {
    if (key === "all") return view.kind === "all";
    if (key === "home") return view.kind === "home" || view.kind === "character";
    if (key === "uncat") return view.kind === "uncat";
    return view.kind === "category" && String(view.id) === key;
  }

  async function createCategory() {
    const v = newName.trim();
    if (!v || busy) {
      creating = false;
      return;
    }
    busy = true;
    try {
      await api.createCategory(v);
      newName = "";
      creating = false;
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  function startRename(c: CategoryDto) {
    menuFor = null;
    renamingId = c.id;
    renameDraft = c.name;
  }

  async function commitRename(id: number) {
    if (renameCancelled) {
      renameCancelled = false;
      return;
    }
    const v = renameDraft.trim();
    renamingId = null;
    if (!v || busy) return;
    busy = true;
    try {
      await api.renameCategory(id, v);
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function move(id: number, delta: number) {
    if (busy) return;
    busy = true;
    menuFor = null;
    try {
      await api.moveCategory(id, delta);
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }

  async function remove(c: CategoryDto) {
    if (confirmingDelete !== c.id) {
      confirmingDelete = c.id;
      return;
    }
    if (busy) return;
    busy = true;
    menuFor = null;
    confirmingDelete = null;
    try {
      await api.deleteCategory(c.id);
      if (view.kind === "category" && view.id === c.id) onnavigate({ kind: "home" });
      onchanged();
    } catch (e) {
      toast(String(e));
    } finally {
      busy = false;
    }
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      menuFor = null;
      confirmingDelete = null;
    }
  }}
/>

<aside class="w-52 shrink-0 flex flex-col min-h-0 px-3 pb-3 pt-1">
  <div class="pb-2.5 shrink-0">
    <SearchBar bind:value={query} />
  </div>
  <nav class="flex-1 min-h-0 overflow-y-auto flex flex-col gap-0.5" aria-label="分类导航">
    {#each [
      { key: "all", label: "全部 Mod", count: allCount },
      { key: "home", label: charCatName, count: charCount },
      { key: "uncat", label: "未分类", count: uncatCount },
    ] as item (item.key)}
      <button
        class="flex items-center justify-between h-9 px-3 radius-card text-sm cursor-pointer transition-colors"
        style={isActive(item.key)
          ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
          : ""}
        aria-current={isActive(item.key) ? "page" : undefined}
        onclick={() =>
          onnavigate(item.key === "all" ? { kind: "all" } : item.key === "home" ? { kind: "home" } : { kind: "uncat" })}
      >
        <span class="truncate">{item.label}</span>
        <span class="text-xs text-secondary shrink-0">{item.count}</span>
      </button>
    {/each}

    {#if categories.length > 0}
      <div class="mx-3 my-2 shrink-0" style="border-top: 0.5px solid var(--glass-stroke)"></div>
    {/if}

    {#each categories as c (c.id)}
      <div class="relative">
        {#if renamingId === c.id}
          <input
            bind:value={renameDraft}
            aria-label={`重命名分类 ${c.name}`}
            class="w-full h-9 px-3 text-sm bg-transparent outline-none radius-card"
            style="box-shadow: inset 0 0 0 1.5px var(--accent)"
            onkeydown={(e) => {
              if (e.key === "Enter") commitRename(c.id);
              else if (e.key === "Escape") {
                renameCancelled = true;
                renamingId = null;
              }
            }}
            onblur={() => commitRename(c.id)}
            autofocus
          />
        {:else}
          <button
            class="w-full flex items-center justify-between h-9 pl-3 pr-1.5 radius-card text-sm cursor-pointer transition-colors"
            style={isActive(String(c.id))
              ? "background: var(--accent-fill); color: var(--accent); font-weight: 600"
              : ""}
            aria-current={isActive(String(c.id)) ? "page" : undefined}
            onclick={() => onnavigate({ kind: "category", id: c.id, name: c.name })}
          >
            <span class="truncate">{c.name}</span>
            <span class="flex items-center gap-0.5 shrink-0">
              <span class="text-xs text-secondary">{c.mod_count}</span>
              <span
                role="button"
                tabindex="0"
                aria-label={`分类操作 ${c.name}`}
                class="w-6 h-6 grid place-items-center rounded-full text-secondary transition-colors hover:bg-[var(--glass-stroke)]"
                onclick={(e) => {
                  e.stopPropagation();
                  confirmingDelete = null;
                  menuFor = menuFor === c.id ? null : c.id;
                }}
                onkeydown={(e) => {
                  if (e.key === "Enter") {
                    e.stopPropagation();
                    menuFor = menuFor === c.id ? null : c.id;
                  }
                }}
              >
                <svg width="12" height="12" viewBox="0 0 12 12" fill="currentColor">
                  <circle cx="6" cy="2.5" r="1.2" /><circle cx="6" cy="6" r="1.2" /><circle cx="6" cy="9.5" r="1.2" />
                </svg>
              </span>
            </span>
          </button>
        {/if}
        {#if menuFor === c.id}
          <button
            class="fixed inset-0 z-40 cursor-default bg-transparent"
            aria-label="关闭分类菜单"
            tabindex="-1"
            onclick={() => {
              menuFor = null;
              confirmingDelete = null;
            }}
          ></button>
          <div class="glass radius-panel absolute right-0 top-10 z-50 w-44 p-1.5 flex flex-col gap-0.5">
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)]" onclick={() => startRename(c)}>重命名</button>
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] disabled:opacity-40" disabled={busy} onclick={() => move(c.id, -1)}>上移</button>
            <button class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] disabled:opacity-40" disabled={busy} onclick={() => move(c.id, 1)}>下移</button>
            <button
              class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors"
              style={confirmingDelete === c.id ? "background: var(--danger); color: white" : "color: var(--danger)"}
              onclick={() => remove(c)}
            >
              {confirmingDelete === c.id
                ? c.mod_count > 0
                  ? `确认删除（${c.mod_count} 个 Mod 移回）`
                  : "确认删除"
                : "删除"}
            </button>
          </div>
        {/if}
      </div>
    {/each}
  </nav>

  <div class="shrink-0 pt-2">
    {#if creating}
      <input
        bind:value={newName}
        aria-label="新分类名称"
        placeholder="分类名称…"
        class="w-full h-9 px-3 text-sm bg-transparent outline-none radius-card"
        style="box-shadow: inset 0 0 0 1.5px var(--accent)"
        onkeydown={(e) => {
          if (e.key === "Enter") createCategory();
          else if (e.key === "Escape") {
            newName = "";
            creating = false;
          }
        }}
        onblur={createCategory}
        autofocus
      />
    {:else}
      <button
        class="w-full h-9 px-3 radius-card text-sm text-secondary cursor-pointer text-left transition-colors hover:bg-[var(--glass-stroke)]"
        onclick={() => (creating = true)}
      >
        ＋ 新建分类
      </button>
    {/if}
  </div>
</aside>
```

- [ ] **Step 5: Toolbar.svelte（含预设遮挡修复：relative z-30）**

```svelte
<script lang="ts">
  import PresetMenu from "./PresetMenu.svelte";
  import type { ModSort } from "$lib/view";

  let {
    crumbs,
    sort = $bindable(),
    showSort,
    onapplied,
  }: {
    crumbs: string[];
    sort: ModSort;
    showSort: boolean;
    onapplied: () => void;
  } = $props();
</script>

<div class="relative z-30 flex items-center justify-between h-12 px-6 shrink-0">
  <nav class="text-sm text-secondary truncate" aria-label="面包屑">
    {#each crumbs as crumb, i (i)}
      {#if i > 0}<span class="mx-1.5 opacity-50">›</span>{/if}
      <span class={i === crumbs.length - 1 ? "font-semibold" : ""} style={i === crumbs.length - 1 ? "color: var(--text)" : ""}>{crumb}</span>
    {/each}
  </nav>
  <div class="flex items-center gap-2.5 shrink-0">
    {#if showSort}
      <div class="glass radius-pill h-9 px-3 flex items-center">
        <select
          bind:value={sort}
          aria-label="排序方式"
          class="bg-transparent outline-none text-sm cursor-pointer"
        >
          <option value="recent">最近安装</option>
          <option value="name">名称</option>
          <option value="enabled">启用优先</option>
        </select>
      </div>
    {/if}
    <PresetMenu {onapplied} />
  </div>
</div>
```

注：select 的 option 在暗色下背景由系统渲染，可接受；`<select>` 内文字色继承 `--text`。

- [ ] **Step 6: +page.svelte 重写**

```svelte
<script lang="ts">
  import { onMount, tick } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import {
    api,
    isTauri,
    type CategoryDto,
    type CharacterSummary,
    type ConfigDto,
    type ModDto,
  } from "$lib/api";
  import { toast } from "$lib/toast.svelte";
  import { applyTheme } from "$lib/theme";
  import { viewKey, type ModSort, type View } from "$lib/view";
  import { enqueueInstalls, installJobs } from "$lib/install.svelte";
  import InstallOverlay from "$lib/components/InstallOverlay.svelte";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import Toolbar from "$lib/components/Toolbar.svelte";
  import CharacterGrid from "$lib/views/CharacterGrid.svelte";
  import CharacterDetail from "$lib/views/CharacterDetail.svelte";
  import Settings from "$lib/views/Settings.svelte";

  let config = $state<ConfigDto | null>(null);
  let characters = $state<CharacterSummary[]>([]);
  let categories = $state<CategoryDto[]>([]);
  let view = $state<View>({ kind: "home" });
  let viewMods = $state<ModDto[]>([]);
  let query = $state("");
  let sort = $state<ModSort>("recent");
  let showSettings = $state(false);
  let error = $state("");
  let dragHover = $state(false);

  let charCatName = $derived(config?.character_category_name ?? "角色");
  let charModTotal = $derived(characters.reduce((n, c) => n + c.total, 0));
  let allCount = $derived(charModTotal + categories.reduce((n, c) => n + c.mod_count, 0));
  let uncatCount = $derived(
    characters.find((c) => c.internal_name === "Others")?.total ?? 0,
  );
  let crumbs = $derived.by((): string[] => {
    switch (view.kind) {
      case "home":
        return [charCatName];
      case "all":
        return ["全部 Mod"];
      case "uncat":
        return ["未分类"];
      case "category":
        return [view.name];
      case "character":
        return [charCatName, view.display];
    }
  });
  let showSort = $derived(view.kind === "all" || view.kind === "uncat" || view.kind === "category");
  let selectedCharacter = $derived.by((): CharacterSummary | null => {
    if (view.kind !== "character") return null;
    return (
      characters.find((c) => c.internal_name === view.name) ?? {
        internal_name: view.name,
        display_name: view.display,
        image: null,
        total: 0,
        enabled: 0,
      }
    );
  });

  // 滚动记忆：display:none 会被浏览器重置 scrollTop，按视图显式保存/恢复
  let contentEl = $state<HTMLDivElement | null>(null);
  const scrollMem = new Map<string, number>();

  function saveScroll() {
    const sc = contentEl?.querySelector(".overflow-y-auto");
    if (sc) scrollMem.set(viewKey(view), sc.scrollTop);
  }

  async function restoreScroll() {
    await tick();
    const sc = contentEl?.querySelector(".overflow-y-auto");
    if (sc) sc.scrollTop = scrollMem.get(viewKey(view)) ?? 0;
  }

  async function loadViewMods() {
    if (view.kind === "category") viewMods = await api.listCategoryMods(view.id);
    else if (view.kind === "all") viewMods = await api.listAllMods();
    else if (view.kind === "uncat") viewMods = await api.listUncategorizedMods();
  }

  async function refresh() {
    error = "";
    try {
      config = await api.getConfig();
      applyTheme(config.theme);
      characters = await api.getCharacters();
      categories = await api.listCategories();
      await loadViewMods();
    } catch (e) {
      error = String(e);
    }
  }

  async function navigate(v: View) {
    if (!showSettings) saveScroll();
    showSettings = false;
    view = v;
    query = "";
    await refresh();
    await restoreScroll();
  }

  function openSettings() {
    // 仅在非设置页时采样滚动（设置打开时内容区可能已隐藏，scrollTop 已被浏览器归零）
    if (!showSettings) saveScroll();
    showSettings = true;
  }

  async function closeSettings() {
    showSettings = false;
    await refresh();
    await restoreScroll();
  }

  onMount(() => {
    void refresh();
    if (!isTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/webviewWindow").then(({ getCurrentWebviewWindow }) => {
      getCurrentWebviewWindow()
        .onDragDropEvent((event) => {
          const t = event.payload.type;
          if (t === "enter" || t === "over") dragHover = true;
          else if (t === "leave") dragHover = false;
          else if (t === "drop") {
            dragHover = false;
            if (event.payload.paths.length > 0)
              enqueueInstalls(event.payload.paths, refresh);
          }
        })
        .then((u) => {
          if (cancelled) u();
          else unlisten = u;
        })
        .catch(() => {});
    });
    let unlistenChanged: (() => void) | undefined;
    let unlistenToast: (() => void) | undefined;
    listen<{ added: number; removed: number }>("library-changed", (e) => {
      if (cancelled) return;
      const { added, removed } = e.payload;
      if (added > 0 || removed > 0) toast(`检测到仓库变动：+${added} / -${removed}`);
      refresh();
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenChanged = u;
      })
      .catch(() => {});
    listen<string>("liquimod-toast", (e) => {
      if (cancelled) return;
      toast(e.payload);
    })
      .then((u) => {
        if (cancelled) u();
        else unlistenToast = u;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
      unlisten?.();
      unlistenChanged?.();
      unlistenToast?.();
    };
  });
</script>

<div class="flex flex-col h-screen">
  <TitleBar onsettings={openSettings} />
  {#if error}
    <div class="glass radius-panel mx-6 mt-1 px-4 py-2.5 text-sm shrink-0" style="color: var(--danger)">
      {error}
    </div>
  {/if}
  <div class="flex flex-1 min-h-0">
    <Sidebar
      {view}
      {categories}
      {charCatName}
      {allCount}
      charCount={charModTotal}
      {uncatCount}
      bind:query
      onnavigate={navigate}
      onchanged={refresh}
    />
    <div bind:this={contentEl} class="flex flex-col flex-1 min-w-0 min-h-0">
      {#if showSettings}
        <Settings {config} onback={closeSettings} onchanged={refresh} />
      {:else}
        <Toolbar {crumbs} bind:sort {showSort} onapplied={refresh} />
        {#if view.kind === "home"}
          <header class="px-6 pt-1 pb-3 shrink-0">
            <h1 class="text-2xl font-bold tracking-tight">{charCatName}</h1>
            <p class="text-xs text-secondary mt-0.5">{characters.length} 位 · {charModTotal} 个 Mod</p>
          </header>
          <CharacterGrid
            {characters}
            {query}
            onselect={(c) => navigate({ kind: "character", name: c.internal_name, display: c.display_name })}
          />
        {:else if view.kind === "character" && selectedCharacter}
          <CharacterDetail
            character={selectedCharacter}
            {categories}
            modsDirConfigured={config?.mods_dir != null}
            onback={() => navigate({ kind: "home" })}
            onconfigured={refresh}
          />
        {:else}
          <!-- ModCardGrid 由 Task 5 提供；本任务先占位保证骨架可编译 -->
          <div class="flex-1 min-h-0 overflow-y-auto px-6 pb-8">
            <p class="text-secondary text-center mt-24">该视图 {viewMods.length} 个 Mod（卡片网格见下一任务）</p>
          </div>
        {/if}
      {/if}
    </div>
  </div>
  {#if dragHover}
    <div class="fixed inset-3 z-40 pointer-events-none radius-panel"
      style="border: 2px dashed var(--accent, #409CFF); background: rgba(64,156,255,0.06)"></div>
  {/if}
  <InstallOverlay jobs={installJobs} {characters} onInstalled={refresh} />
</div>
```

CharacterDetail 暂收 `categories` prop——Task 5 才用到；本任务在 CharacterDetail props 里先加 `categories: CategoryDto[]`（import type CategoryDto），模板不用，避免 Task 5 改签名时连锁改动。

- [ ] **Step 7: Sidebar.test.ts**

```ts
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import Sidebar from "./Sidebar.svelte";
import type { CategoryDto } from "$lib/api";

const cats: CategoryDto[] = [
  { id: 1, name: "武器", ord: 1, mod_count: 2 },
  { id: 2, name: "光影", ord: 2, mod_count: 0 },
];

function props(over: Partial<Parameters<typeof Sidebar>[0]> = {}) {
  return {
    view: { kind: "home" } as const,
    categories: cats,
    charCatName: "角色",
    allCount: 5,
    charCount: 3,
    uncatCount: 1,
    query: "",
    onnavigate: vi.fn(),
    onchanged: vi.fn(),
    ...over,
  };
}

describe("Sidebar", () => {
  it("渲染内置条目与自定义分类及计数", () => {
    render(Sidebar, { props: props() });
    expect(screen.getByText("全部 Mod")).toBeTruthy();
    expect(screen.getByText("角色")).toBeTruthy();
    expect(screen.getByText("未分类")).toBeTruthy();
    expect(screen.getByText("武器")).toBeTruthy();
    expect(screen.getByText("2")).toBeTruthy();
  });

  it("点击条目导航", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("全部 Mod"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "all" });
    await fireEvent.click(screen.getByText("武器"));
    expect(p.onnavigate).toHaveBeenCalledWith({ kind: "category", id: 1, name: "武器" });
  });

  it("当前视图高亮", () => {
    render(Sidebar, { props: props({ view: { kind: "category", id: 1, name: "武器" } }) });
    const btn = screen.getByText("武器").closest("button")!;
    expect(btn.getAttribute("aria-current")).toBe("page");
  });

  it("新建分类行内输入并提交", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByText("＋ 新建分类"));
    const input = screen.getByLabelText("新分类名称");
    await fireEvent.input(input, { target: { value: "UI" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    expect(p.onchanged).toHaveBeenCalled();
  });

  it("分类菜单删除需二次确认", async () => {
    const p = props();
    render(Sidebar, { props: p });
    await fireEvent.click(screen.getByLabelText("分类操作 武器"));
    await fireEvent.click(screen.getByText("删除"));
    expect(screen.getByText("确认删除（2 个 Mod 移回）")).toBeTruthy();
    await fireEvent.click(screen.getByText("确认删除（2 个 Mod 移回）"));
    expect(p.onchanged).toHaveBeenCalled();
  });
});
```

- [ ] **Step 8: 验证 + 提交**

Run: `cd app; npx vitest run; npm run check; npm run build`
Commit: `feat(ui): 左侧边栏 + 工具条布局骨架，view 状态机与按视图滚动记忆，预设面板层级修复`

---

### Task 5: ModCard 网格 + CategoryMenu + 信号灯 + ModRow 归类

**Files:**
- Create: `app/src/lib/components/CategoryMenu.svelte`
- Create: `app/src/lib/components/ModCard.svelte`
- Create: `app/src/lib/components/ModCardGrid.svelte`
- Modify: `app/src/lib/components/CharacterCard.svelte`（信号灯）
- Modify: `app/src/lib/components/ModRow.svelte`（移到分类按钮）
- Modify: `app/src/lib/views/CharacterDetail.svelte`（接线 categories）
- Modify: `app/src/routes/+page.svelte`（占位换成 ModCardGrid）
- Test: `app/src/lib/components/ModCardGrid.test.ts`、`app/src/lib/components/CharacterCard.test.ts`

- [ ] **Step 1: CategoryMenu.svelte（ModRow/ModCard 共用）**

```svelte
<script lang="ts">
  import type { CategoryDto } from "$lib/api";

  let {
    categories,
    current,
    label,
    onpick,
  }: {
    categories: CategoryDto[];
    current: number | null;
    label: string;
    onpick: (id: number | null) => void;
  } = $props();

  let open = $state(false);

  function pick(id: number | null) {
    open = false;
    if (id !== current) onpick(id);
  }
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape" && open) open = false;
  }}
/>

<div class="relative">
  <button
    class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer"
    aria-label={label}
    title="移到分类"
    aria-expanded={open}
    onclick={() => (open = !open)}
  >
    <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
      <path d="M1.5 3.5a1 1 0 0 1 1-1h2.6l1 1.2h5.4a1 1 0 0 1 1 1v5.8a1 1 0 0 1-1 1H2.5a1 1 0 0 1-1-1v-6Z" stroke="currentColor" stroke-width="1.1" stroke-linejoin="round" />
      <path d="M5 8h3.5M7.3 6.8 8.7 8l-1.4 1.2" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
    </svg>
  </button>
  {#if open}
    <button
      class="fixed inset-0 z-40 cursor-default bg-transparent"
      aria-label="关闭分类菜单"
      tabindex="-1"
      onclick={() => (open = false)}
    ></button>
    <div class="glass radius-panel absolute right-0 top-9 z-50 w-44 p-1.5 flex flex-col gap-0.5">
      <button
        class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
        onclick={() => pick(null)}
      >
        <span>角色（默认）</span>
        {#if current === null}<span class="accent-text text-xs">✓</span>{/if}
      </button>
      {#each categories as c (c.id)}
        <button
          class="text-left text-sm px-2.5 py-1.5 rounded-lg cursor-pointer transition-colors hover:bg-[var(--glass-stroke)] flex items-center justify-between"
          onclick={() => pick(c.id)}
        >
          <span class="truncate">{c.name}</span>
          {#if current === c.id}<span class="accent-text text-xs">✓</span>{/if}
        </button>
      {/each}
      {#if categories.length === 0}
        <p class="text-xs text-secondary px-2.5 py-1.5">还没有自定义分类，在左侧边栏底部新建</p>
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 2: ModCard.svelte**

```svelte
<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import Toggle from "./Toggle.svelte";
  import CategoryMenu from "./CategoryMenu.svelte";

  let {
    mod,
    categories,
    catLabel,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
  }: {
    mod: ModDto;
    categories: CategoryDto[];
    catLabel: string;
    ontoggle: (next: boolean) => void;
    onrename: (name: string) => Promise<boolean>;
    onuninstall: () => Promise<void>;
    onopen: () => void;
    onmove: (categoryId: number | null) => void;
  } = $props();

  let renaming = $state(false);
  let draft = $state("");
  let confirming = $state(false);
  let busy = $state(false);
  let cancelled = $state(false);

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
    if (cancelled) {
      cancelled = false;
      return;
    }
    const v = draft.trim();
    if (!v || v === mod.name || busy) {
      renaming = false;
      return;
    }
    busy = true;
    try {
      const ok = await onrename(v);
      if (ok) renaming = false;
    } finally {
      busy = false;
    }
  }

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

  function onCardKeydown(e: KeyboardEvent) {
    if (renaming || confirming) return;
    if (e.key !== " " && e.key !== "Enter") return;
    if ((e.target as HTMLElement).closest("button, input, select")) return;
    e.preventDefault();
    ontoggle(!mod.enabled);
  }
</script>

<div
  role="listitem"
  tabindex="0"
  aria-label={mod.name}
  class="group glass radius-card overflow-hidden outline-none transition-all duration-200 hover:-translate-y-0.5 hover:shadow-[var(--shadow-lift)] focus-visible:shadow-[inset_0_0_0_2px_var(--accent)]"
  onkeydown={onCardKeydown}
>
  {#if confirming}
    <div class="aspect-video grid place-items-center px-4">
      <p class="text-sm text-center">确认卸载 <span class="font-medium">{mod.name}</span>？<br />文件将被删除</p>
    </div>
    <div class="px-4 pb-4 flex items-center justify-center gap-2">
      <button
        class="radius-pill h-8 px-3.5 text-sm font-medium text-white cursor-pointer disabled:opacity-50"
        style="background: var(--danger)"
        disabled={busy}
        onclick={confirmUninstall}
      >
        确认卸载
      </button>
      <button class="glass radius-pill h-8 px-3.5 text-sm cursor-pointer" onclick={() => (confirming = false)}>
        取消
      </button>
    </div>
  {:else}
    <div class="relative aspect-video overflow-hidden">
      {#if mod.thumb}
        <img
          src={mod.thumb}
          alt=""
          class="w-full h-full object-cover transition-transform duration-300 group-hover:scale-105"
          draggable="false"
          onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = "none")}
        />
      {:else}
        <div class="w-full h-full grid place-items-center text-3xl font-semibold text-secondary"
          style="background: var(--glass-tint)">
          {mod.name.slice(0, 1)}
        </div>
      {/if}
      {#if !renaming}
        <div class="absolute top-2 right-2 flex gap-1.5 opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
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
          <CategoryMenu
            {categories}
            current={mod.category_id}
            label={`移到分类 ${mod.name}`}
            onpick={onmove}
          />
          <button
            class="glass radius-pill w-8 h-8 grid place-items-center cursor-pointer transition-colors hover:text-white"
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
    </div>
    <div class="px-4 py-3">
      {#if renaming}
        <input
          bind:value={draft}
          aria-label={`新名字 ${mod.name}`}
          class="w-full h-8 px-3 text-sm font-medium bg-transparent outline-none rounded-full"
          style="box-shadow: inset 0 0 0 1.5px var(--accent)"
          onkeydown={(e) => {
            if (e.key === "Enter") commitRename();
            else if (e.key === "Escape") {
              cancelled = true;
              renaming = false;
            }
          }}
          onblur={commitRename}
          autofocus
        />
      {:else}
        <p class="font-medium text-sm truncate">{mod.name}</p>
        <p class="text-xs text-secondary mt-0.5">
          {fmtSize(mod.size_bytes)} · {mod.file_count < 0 ? "—" : mod.file_count} 文件 · {fmtDate(mod.installed_at)}
        </p>
      {/if}
      {#if !renaming}
        <div class="flex items-center justify-between mt-2">
          <span class="text-[11px] text-secondary radius-pill px-2 py-0.5" style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke)">{catLabel}</span>
          <Toggle
            checked={mod.enabled}
            ariaLabel={`启用 ${mod.name}`}
            onchange={(next) => ontoggle(next)}
          />
        </div>
      {/if}
    </div>
  {/if}
</div>
```

- [ ] **Step 3: ModCardGrid.svelte**

```svelte
<script lang="ts">
  import type { CategoryDto, ModDto } from "$lib/api";
  import { filterMods, sortMods, type ModSort } from "$lib/view";
  import ModCard from "./ModCard.svelte";

  let {
    mods,
    categories,
    sort,
    query,
    catLabelOf,
    ontoggle,
    onrename,
    onuninstall,
    onopen,
    onmove,
  }: {
    mods: ModDto[];
    categories: CategoryDto[];
    sort: ModSort;
    query: string;
    catLabelOf: (m: ModDto) => string;
    ontoggle: (m: ModDto, next: boolean) => void;
    onrename: (m: ModDto, name: string) => Promise<boolean>;
    onuninstall: (m: ModDto) => Promise<void>;
    onopen: (m: ModDto) => void;
    onmove: (m: ModDto, categoryId: number | null) => void;
  } = $props();

  let shown = $derived(sortMods(filterMods(mods, query), sort));
</script>

<div class="grid grid-cols-[repeat(auto-fill,minmax(230px,1fr))] gap-5 px-6 pt-2 pb-8 overflow-y-auto flex-1 min-h-0 content-start">
  {#each shown as m (m.id)}
    <ModCard
      mod={m}
      {categories}
      catLabel={catLabelOf(m)}
      ontoggle={(next) => ontoggle(m, next)}
      onrename={(name) => onrename(m, name)}
      onuninstall={() => onuninstall(m)}
      onopen={() => onopen(m)}
      onmove={(cid) => onmove(m, cid)}
    />
  {/each}
  {#if shown.length === 0}
    <p class="text-secondary col-span-full text-center mt-24">
      {mods.length === 0 ? "这里还没有 Mod" : "没有匹配的 Mod"}
    </p>
  {/if}
</div>
```

- [ ] **Step 4: CharacterCard 信号灯 + 底部玻璃条**

CharacterCard.svelte 的底部信息区（37-47 行）替换为独立玻璃条，并在卡片右上角加信号灯。整个组件改为：

```svelte
<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();

  // 信号灯：恰好 1 个启用 = 绿；2 个及以上 = 黄；0 = 灰
  let dot = $derived(
    character.enabled === 1
      ? { color: "#34c759", glow: "0 0 6px rgba(52,199,89,0.9)" }
      : character.enabled >= 2
        ? { color: "#ffd60a", glow: "0 0 6px rgba(255,214,10,0.9)" }
        : { color: "rgba(142,142,147,0.65)", glow: "none" },
  );
</script>

<div
  role="button"
  tabindex="0"
  class="radius-card relative overflow-hidden cursor-pointer transition-all duration-200 hover:scale-[1.03] hover:-translate-y-0.5 active:scale-[0.98] outline-none"
  style="box-shadow: inset 0 0 0 0.5px var(--glass-stroke), var(--shadow-soft)"
  {onclick}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onclick();
    }
  }}
>
  <div class="w-full" style="padding-top: 100%"></div>
  {#if character.image}
    <img
      src={portraitUrl(character.image)}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top"
      loading="lazy"
      draggable="false"
    />
  {:else}
    <div class="glass absolute inset-0 grid place-items-center text-4xl font-bold text-secondary">
      {character.display_name.slice(0, 1)}
    </div>
  {/if}
  <span
    class="absolute top-2.5 right-2.5 w-2.5 h-2.5 rounded-full z-10"
    title={character.enabled > 0 ? `${character.enabled} 个 Mod 启用中` : "没有启用的 Mod"}
    style:background={dot.color}
    style:box-shadow={dot.glow}
  ></span>
  <div class="absolute inset-x-2 bottom-2 glass radius-pill pl-3 pr-2 py-1.5 flex items-center justify-between gap-1.5 pointer-events-none z-10">
    <span class="text-[13px] font-medium truncate">{character.display_name}</span>
    {#if character.total > 0}
      <span class="text-[11px] text-secondary shrink-0">{character.enabled}/{character.total}</span>
    {/if}
  </div>
</div>
```

注意：删除了原来的黑色渐变遮罩与白色文字药丸——信息玻璃条替代，亮色下也可读。

- [ ] **Step 5: ModRow 加「移到分类」**

props 加 `categories: CategoryDto[]` 与 `onmove: (categoryId: number | null) => void`；import CategoryMenu。在操作按钮组里「重命名」与「卸载」之间插入：

```svelte
        <CategoryMenu
          {categories}
          current={mod.category_id}
          label={`移到分类 ${mod.name}`}
          onpick={onmove}
        />
```

- [ ] **Step 6: CharacterDetail 接线**

props 已有 `categories: CategoryDto[]`（Task 4 加的）；script 加：

```ts
  async function moveCategory(mod: ModDto, categoryId: number | null) {
    error = "";
    try {
      await api.setModCategory(mod.id, categoryId);
      if (categoryId !== null) {
        // 移出角色视图后从列表消失
        mods = mods.filter((m) => m.id !== mod.id);
      }
      onconfigured(); // 刷新侧边栏计数
    } catch (e) {
      error = String(e);
    }
  }
```

ModRow 调用加 `onmove={(cid) => moveCategory(mod, cid)}`。

- [ ] **Step 7: +page.svelte 占位换成 ModCardGrid**

import 加 `import ModCardGrid from "$lib/components/ModCardGrid.svelte";`，script 加：

```ts
  function catLabelOf(m: ModDto): string {
    if (m.category_id == null) return charCatName;
    return categories.find((c) => c.id === m.category_id)?.name ?? "未分类";
  }

  async function toggleViewMod(m: ModDto, next: boolean) {
    try {
      await api.setModEnabled(m.id, next);
      m.enabled = next;
    } catch (e) {
      toast(String(e));
    }
  }

  async function renameViewMod(m: ModDto, name: string): Promise<boolean> {
    try {
      await api.renameMod(m.id, name);
      m.name = name;
      return true;
    } catch (e) {
      toast(String(e));
      return false;
    }
  }

  async function uninstallViewMod(m: ModDto) {
    try {
      await api.uninstallMod(m.id);
      await refresh();
    } catch (e) {
      toast(String(e));
    }
  }

  async function openViewMod(m: ModDto) {
    if (!isTauri()) return;
    try {
      const { openPath } = await import("@tauri-apps/plugin-opener");
      await openPath(m.path);
    } catch (e) {
      toast(String(e));
    }
  }

  async function moveViewMod(m: ModDto, categoryId: number | null) {
    try {
      await api.setModCategory(m.id, categoryId);
      await refresh();
    } catch (e) {
      toast(String(e));
    }
  }
```

else 分支占位替换为：

```svelte
        {:else}
          <ModCardGrid
            mods={viewMods}
            {categories}
            {sort}
            {query}
            {catLabelOf}
            ontoggle={toggleViewMod}
            onrename={renameViewMod}
            onuninstall={uninstallViewMod}
            onopen={openViewMod}
            onmove={moveViewMod}
          />
        {/if}
```

- [ ] **Step 8: 测试**

`CharacterCard.test.ts`：

```ts
import { render } from "@testing-library/svelte";
import { describe, expect, it } from "vitest";
import CharacterCard from "./CharacterCard.svelte";
import type { CharacterSummary } from "$lib/api";

function c(enabled: number, total = 3): CharacterSummary {
  return { internal_name: "Firefly", display_name: "流萤", image: null, total, enabled };
}

describe("CharacterCard 信号灯", () => {
  it("恰好 1 个启用 = 绿灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(1), onclick: () => {} } });
    const dot = container.querySelector("span[title]")!;
    expect(dot.getAttribute("style")).toContain("#34c759");
    expect(dot.getAttribute("title")).toBe("1 个 Mod 启用中");
  });
  it("2 个及以上 = 黄灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(2), onclick: () => {} } });
    expect(container.querySelector("span[title]")!.getAttribute("style")).toContain("#ffd60a");
  });
  it("0 个 = 灰灯", () => {
    const { container } = render(CharacterCard, { props: { character: c(0, 0), onclick: () => {} } });
    const dot = container.querySelector("span[title]")!;
    expect(dot.getAttribute("style")).toContain("142,142,147");
    expect(dot.getAttribute("title")).toBe("没有启用的 Mod");
  });
});
```

`ModCardGrid.test.ts`：

```ts
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import ModCardGrid from "./ModCardGrid.svelte";
import type { ModDto } from "$lib/api";

function mod(id: number, name: string, enabled: boolean, installed_at: number, category_id: number | null = null): ModDto {
  return { id, name, enabled, installed_at, thumb: null, size_bytes: 2048, file_count: 3, path: "", category_id };
}

function props(mods: ModDto[]) {
  return {
    mods,
    categories: [{ id: 1, name: "武器", ord: 1, mod_count: 0 }],
    sort: "recent" as const,
    query: "",
    catLabelOf: (m: ModDto) => (m.category_id ? "武器" : "角色"),
    ontoggle: vi.fn(),
    onrename: vi.fn(async () => true),
    onuninstall: vi.fn(async () => {}),
    onopen: vi.fn(),
    onmove: vi.fn(),
  };
}

describe("ModCardGrid", () => {
  it("渲染卡片与副行信息", () => {
    render(ModCardGrid, { props: props([mod(1, "大剑", false, 100)]) });
    expect(screen.getByText("大剑")).toBeTruthy();
    expect(screen.getByText(/2 KB · 3 文件/)).toBeTruthy();
    expect(screen.getByText("角色")).toBeTruthy();
  });

  it("搜索过滤", () => {
    const p = props([mod(1, "大剑", false, 100), mod(2, "特效", false, 50)]);
    render(ModCardGrid, { props: { ...p, query: "特效" } });
    expect(screen.queryByText("大剑")).toBeNull();
    expect(screen.getByText("特效")).toBeTruthy();
  });

  it("空格启停", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    const card = screen.getByLabelText("大剑");
    await fireEvent.keyDown(card, { key: " " });
    expect(p.ontoggle).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }), true);
  });

  it("卸载需二次确认", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    await fireEvent.click(screen.getByLabelText("卸载 大剑"));
    expect(screen.getByText(/确认卸载/)).toBeTruthy();
    await fireEvent.click(screen.getByRole("button", { name: "确认卸载" }));
    expect(p.onuninstall).toHaveBeenCalled();
  });

  it("移到分类回调", async () => {
    const p = props([mod(1, "大剑", false, 100)]);
    render(ModCardGrid, { props: p });
    await fireEvent.click(screen.getByLabelText("移到分类 大剑"));
    await fireEvent.click(screen.getByText("武器"));
    expect(p.onmove).toHaveBeenCalledWith(expect.objectContaining({ id: 1 }), 1);
  });

  it("空态", () => {
    render(ModCardGrid, { props: props([]) });
    expect(screen.getByText("这里还没有 Mod")).toBeTruthy();
  });
});
```

- [ ] **Step 9: 验证 + 提交**

Run: `cd app; npx vitest run; npm run check; npm run build`
Commit: `feat(ui): Mod 大卡片网格 + 移到分类菜单 + 角色卡信号灯与玻璃信息条`

---

### Task 6: 中文化收尾 + E2E 实测 + 终审

**Files:**
- Modify: 任何残留英文的 `.svelte`/`.ts`（先 grep 确认）
- Modify: `app/src-tauri/tauri.conf.json`（窗口标题如需）
- Modify: `AGENTS.md`（末尾追加里程碑 8 小节）

- [ ] **Step 1: 中文化排查**

Run: `cd app; rg -n "[A-Za-z]{4,}" src --glob "*.svelte" -g "*.ts" | rg -v "import|from|class=|style=|aria-label|http|svg|path d=|viewBox|const |let |function|return|var\(--"` 人工过一遍命中，把面向用户的英文文案（按钮、占位符、空态、title）全部改中文。代码标识符不动。

- [ ] **Step 2: 构建真实 exe 并 CDP 实测**

按 `AGENTS.md`：杀进程 → `npm run build` → `cargo build --release --features tauri/custom-protocol --manifest-path app\src-tauri\Cargo.toml`。用 `$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"` 启动，`%LOCALAPPDATA%\Temp\opencode\cdpeval.mjs` + `cdpshot.mjs` 实测：

1. 侧边栏渲染：内置三条 + 计数正确（全部 Mod = 角色 Mod + 各分类之和）。
2. 新建分类「武器」→ 出现在侧边栏 → 进入分类视图（空态）。
3. 角色详情里把演示 Mod 移到「武器」→ 详情列表消失 → 「武器」视图出现该卡片 → 侧边栏计数 +1 → 移回「角色（默认）」复原。
4. 角色卡信号灯：启用 1 个 Mod 的角色显示绿点（elementFromPoint / getComputedStyle 验证颜色）；演示 Mod 停用后回灰。
5. 预设菜单：打开后 `document.elementFromPoint(面板中心坐标)` 命中面板元素而非角色卡（遮挡修复验证）。
6. 主题：设置页切亮色 → `documentElement.dataset.theme === "light"` → 截图目测玻璃质感 → 切回 auto。
7. 滚动记忆：「全部 Mod」滚动 → 切「角色」→ 切回「全部 Mod」滚动位置保持。
8. 搜索：输入角色名片段，角色网格过滤；分类视图输入 Mod 名过滤。
9. 亮色 + 暗色各截一张全图存档 `%LOCALAPPDATA%\Temp\opencode\m8-light.png` / `m8-dark.png`。

- [ ] **Step 3: AGENTS.md 追加里程碑 8 小节**

在文件末尾追加：

```markdown

## 里程碑 8（分类与新布局）

- 分类纯 DB：`categories` 表 + `mods.category_id`（NULL = 角色视图），磁盘目录不变。
- 「角色」是虚拟分类，显示名 config.character_category_name 可改；「未分类」= 未归类且不属于已知角色。
- 主题：config.theme = auto|light|dark，`document.documentElement.dataset.theme` 驱动 CSS 变量；auto 监听 prefers-color-scheme。
- 布局：Sidebar（分类导航）+ Toolbar（面包屑/排序/预设）+ view 状态机；滚动记忆按 viewKey 存 Map。
- 预设等浮层面板的祖先必须有 `relative z-30`（transform 的卡片会建层叠上下文盖住无定位面板）。
```

- [ ] **Step 4: 终审子代理 + 修复**

派终审子代理（规格对照 + 质量双阶段），修复 Critical/Important 后收尾提交。

- [ ] **Step 5: 最终验证 + 提交**

Run: `cargo test --workspace; cargo clippy --workspace --all-targets; cargo fmt --all; cd app; npx vitest run; npm run check`
Commit: `chore(ui): 中文化收尾与里程碑 8 终审修复`

---

## Self-Review 记录

- **规格覆盖**：§2 布局（Task 4）、§3 数据模型（Task 1/2）、§4 视觉（Task 3/5，Mod 大卡片、角色卡玻璃条、信号灯、亮色参数、预设修复）、§5 错误处理（分类重名/空/不存在、删除确认——Task 1/4）、§6 测试（各 Task 内嵌）、中文化（Task 6）。全部有对应任务。
- **命名一致性**：`CategoryDto`/`Category` 字段 id/name/ord/mod_count 两端一致；`set_mod_category(id, category_id)` Rust → JS `{id, categoryId}`；`viewKey/filterMods/sortMods` 定义于 Task 4 Step 2，Task 5 引用一致；`charCatName` prop 名在 +page/Sidebar 一致；`onmove`/`catLabelOf` 在 ModCardGrid/ModCard/+page 一致。
- **类型一致性**：ModDto.category_id 在 api.ts（Task 4 Step 1）、Rust ModDto（Task 2）、view.test.ts 工厂函数三处一致。CharacterDetail 的 `categories` prop 在 Task 4 加入、Task 5 使用。
- **已知留白（有意）**：ModCardGrid 在 Task 4 是占位（编译可过），Task 5 替换；拖拽归类不做（YAGNI）；分类不做图标/颜色。
