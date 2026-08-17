# LiquiMod 里程碑 3：Tauri 壳 + 液态玻璃主界面 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Tauri 2 桌面壳 + Svelte 5 液态玻璃主界面：角色网格浏览、搜索、Mod 列表与 Junction 启停，明暗主题跟随系统。

**Architecture:** 新增 `games` 模块（core，HSR 角色数据 vendored 自 JASM 资产）提供角色清单；新增 `app/`（create-tauri-app Svelte-TS 模板）含 `app/src-tauri`（workspace 成员，IPC 命令层包 liquimod-core）；前端 Tailwind v4 + 玻璃设计 tokens（设计文档 §7）。配置持久化在 `%APPDATA%/LiquiMod/config.json`（library_root + mods_dir）。

**Tech Stack:** Tauri 2、Svelte 5（runes）+ SvelteKit（create-tauri-app 当前 svelte-ts 模板即 SvelteKit + adapter-static，静态导出到 `build/`）、TypeScript、Tailwind CSS v4（@tailwindcss/vite）、Vitest + @testing-library/svelte、serde/serde_json、dirs。

**模板结构备注（Task 2 实测）：** 脚手架是 SvelteKit 变体——入口是 `src/routes/+page.svelte`（非 App.svelte/main.ts），HTML 壳是 `src/app.html`，`frontendDist` 为 `../build`，静态目录默认 `static/`（Task 5 改指 `../assets/hsr`）。全局 CSS 放 `src/app.css` 并在 `src/routes/+layout.svelte` 中 import。

**环境前置：** Windows + Node 24 + Rust 1.96（已确认）；WebView2 Runtime（Win11 自带）。角色立绘源：`C:\Users\10697\Desktop\JASM\src\GIMI-ModManager.WinUI\Assets\Games\Honkai\`（characters.json + Images/Characters/，84 个文件 7.8MB）。

**范围外（后续里程碑）：** 拖放安装流（M4）、watcher 对账与 F10 helper（M5）、预设/设置页/中文角色名（M6）。

---

### Task 1: games 模块 — HSR 角色数据入库（core）

**Files:**
- Create: `assets/hsr/characters.json`（复制自 JASM）
- Create: `assets/hsr/images/<若干图片>`（复制自 JASM Images/Characters/）
- Create: `crates/liquimod-core/src/games/mod.rs`
- Create: `crates/liquimod-core/src/games/hsr.rs`
- Modify: `crates/liquimod-core/Cargo.toml`
- Modify: `crates/liquimod-core/src/lib.rs`

- [ ] **Step 1: 复制 JASM 资产到仓库**

```powershell
New-Item -ItemType Directory -Force assets\hsr\images
Copy-Item "C:\Users\10697\Desktop\JASM\src\GIMI-ModManager.WinUI\Assets\Games\Honkai\characters.json" assets\hsr\
Copy-Item "C:\Users\10697\Desktop\JASM\src\GIMI-ModManager.WinUI\Assets\Games\Honkai\Images\Characters\*" assets\hsr\images\
```
预期：assets\hsr\characters.json 存在；images 下 84 个文件。

- [ ] **Step 2: core 加 serde 依赖**

`crates/liquimod-core/Cargo.toml` `[dependencies]` 追加：

```toml
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 3: 写失败测试（games/hsr.rs 测试模块先行）**

创建 `crates/liquimod-core/src/games/mod.rs`：

```rust
pub mod hsr;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CharacterInfo {
    pub internal_name: String,
    pub display_name: String,
    pub image: String,
}

pub trait Game {
    fn id(&self) -> &'static str;
    fn characters(&self) -> &[CharacterInfo];
}
```

创建 `crates/liquimod-core/src/games/hsr.rs`（先只有测试，实现为占位编译不过的下一步补——按 TDD 先写测试再实现）：

```rust
use super::{CharacterInfo, Game};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct RawCharacter {
    #[serde(rename = "InternalName")]
    internal_name: String,
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Image")]
    image: String,
}

/// 崩坏：星穹铁道角色清单（数据 vendored 自 JASM 资产，见 assets/hsr/）。
pub struct Hsr {
    characters: Vec<CharacterInfo>,
}

impl Hsr {
    pub fn new() -> Self {
        let raw: Vec<RawCharacter> =
            serde_json::from_str(include_str!("../../../../assets/hsr/characters.json"))
                .expect("assets/hsr/characters.json must be valid JSON");
        Self {
            characters: raw
                .into_iter()
                .map(|c| CharacterInfo {
                    internal_name: c.internal_name,
                    display_name: c.display_name,
                    image: c.image,
                })
                .collect(),
        }
    }

    pub fn shared() -> &'static Hsr {
        static INSTANCE: OnceLock<Hsr> = OnceLock::new();
        INSTANCE.get_or_init(Hsr::new)
    }
}

impl Default for Hsr {
    fn default() -> Self {
        Self::new()
    }
}

impl Game for Hsr {
    fn id(&self) -> &'static str {
        "hsr"
    }
    fn characters(&self) -> &[CharacterInfo] {
        &self.characters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_characters_from_vendored_json() {
        let hsr = Hsr::new();
        assert_eq!(hsr.id(), "hsr");
        assert!(hsr.characters().len() > 50, "expected full HSR roster");
        for c in hsr.characters() {
            assert!(!c.internal_name.is_empty());
            assert!(!c.display_name.is_empty());
            assert!(!c.image.is_empty());
            assert!(!c.internal_name.contains(['/', '\\']));
        }
    }

    #[test]
    fn every_character_image_exists_on_disk() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/hsr/images");
        for c in Hsr::new().characters() {
            assert!(dir.join(&c.image).is_file(), "missing image {}", c.image);
        }
    }

    #[test]
    fn shared_returns_same_instance() {
        assert!(std::ptr::eq(Hsr::shared(), Hsr::shared()));
    }
}
```

- [ ] **Step 4: 注册模块并跑测试**

`crates/liquimod-core/src/lib.rs` 顶部追加 `pub mod games;`（按字母序放在 error 后、library 前，保持现有排序风格）。

Run: `cargo test -p liquimod-core games`
Expected: 3 个测试全 PASS。

- [ ] **Step 5: 质量门 + Commit**

```powershell
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
git add assets crates/liquimod-core
git commit -m "feat(core): games 模块与 HSR 角色数据（vendored 自 JASM）"
```

---

### Task 2: Tauri 脚手架 + workspace 集成 + dialog 插件

**Files:**
- Create: `app/`（create-tauri-app 输出，Svelte-TS 模板）
- Modify: `Cargo.toml`（workspace members）
- Modify: `app/tauri.conf.json` 实际路径 `app/src-tauri/tauri.conf.json`
- Modify: `app/package.json`

- [ ] **Step 1: 生成脚手架**

```powershell
npm create tauri-app@latest app -- --template svelte-ts --manager npm --identifier com.liquimod.app -y
```
若交互提示不消失则改用 `npx create-tauri-app@latest app --template svelte-ts --manager npm --identifier com.liquimod.app -y`。
预期：app/ 含 package.json、src/、src-tauri/、vite.config.ts。

- [ ] **Step 2: 品牌化与窗口配置**

`app/package.json`：`"name"` 改为 `"liquimod-app"`。

`app/src-tauri/tauri.conf.json`：
- `productName`: `"LiquiMod"`
- `app.windows[0]` 设为：

```json
{
  "title": "LiquiMod",
  "width": 1200,
  "height": 800,
  "minWidth": 900,
  "minHeight": 600,
  "decorations": false
}
```

`app/index.html` `<title>` 改为 `LiquiMod`。

- [ ] **Step 3: 加入 cargo workspace 并接 core**

根 `Cargo.toml`：

```toml
members = ["crates/liquimod-core", "crates/liquimod-cli", "app/src-tauri"]
```

`app/src-tauri/Cargo.toml` `[dependencies]` 追加：

```toml
liquimod-core = { path = "../../crates/liquimod-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dirs = "6"
```

Run: `cargo check -p liquimod-app`（crate 名以脚手架生成的为准，常见为 `app` 或 `liquimod-app`；用 `cargo metadata` 确认后把 package name 改为 `liquimod-app`）
Expected: 编译通过。

- [ ] **Step 4: 安装依赖 + dialog 插件**

```powershell
cd app
npm install
npm run tauri add dialog
```
预期：tauri-plugin-dialog 加入 Cargo.toml 与 package.json，并自动在 lib.rs 注册 `.plugin(tauri_plugin_dialog::init())`。

- [ ] **Step 5: 验证前端构建**

Run: `cd app; npm run build`
Expected: vite 构建成功生成 dist/。

- [ ] **Step 6: Commit**

```powershell
git add app Cargo.toml Cargo.lock
git commit -m "feat(app): Tauri 2 + Svelte 5 脚手架接入 workspace"
```

---

### Task 3: 配置持久化 config.rs（src-tauri，TDD）

**Files:**
- Create: `app/src-tauri/src/config.rs`
- Modify: `app/src-tauri/src/lib.rs`

**设计：** `Config { library_root, mods_dir }` 存 `%APPDATA%/LiquiMod/config.json`。首次运行默认 library_root = config 同目录下 `Library/`。路径可注入以便测试。

- [ ] **Step 1: 写 config.rs（含测试）**

```rust
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub library_root: PathBuf,
    pub mods_dir: Option<PathBuf>,
}

impl Config {
    /// 平台配置路径：%APPDATA%/LiquiMod/config.json
    pub fn config_path() -> PathBuf {
        dirs::config_dir()
            .expect("无法定位用户配置目录")
            .join("LiquiMod")
            .join("config.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::config_path())
    }

    /// 文件缺失或损坏时回退默认（library_root = config 文件同目录 Library/）。
    pub fn load_from(path: &Path) -> Self {
        match fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        {
            Some(c) => c,
            None => Self {
                library_root: path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join("Library"),
                mods_dir: None,
            },
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        self.save_to(&Self::config_path())
    }

    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self).expect("Config 序列化"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_next_to_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("LiquiMod").join("config.json");
        let c = Config::load_from(&path);
        assert_eq!(c.library_root, dir.path().join("LiquiMod").join("Library"));
        assert_eq!(c.mods_dir, None);
    }

    #[test]
    fn save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        let c = Config {
            library_root: PathBuf::from("C:/lib"),
            mods_dir: Some(PathBuf::from("C:/game/Mods")),
        };
        c.save_to(&path).unwrap();
        assert_eq!(Config::load_from(&path), c);
    }

    #[test]
    fn corrupt_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("c.json");
        fs::write(&path, "{ not json").unwrap();
        let c = Config::load_from(&path);
        assert_eq!(c.library_root, dir.path().join("Library"));
    }
}
```

- [ ] **Step 2: tempfile dev-dependency + 模块注册**

`app/src-tauri/Cargo.toml` 追加：

```toml
[dev-dependencies]
tempfile = "3"
```

`app/src-tauri/src/lib.rs` 顶部加 `mod config;`（暂 pub 未用，后续 task 用）。

- [ ] **Step 3: 跑测试 + Commit**

Run: `cargo test -p liquimod-app`（crate 名以实际为准）
Expected: 3 PASS。
```powershell
git add app/src-tauri
git commit -m "feat(app): 应用配置持久化（config.json）"
```

---

### Task 4: IPC 命令层 + 汇总逻辑（src-tauri，TDD 在纯函数上）

**Files:**
- Create: `app/src-tauri/src/commands.rs`
- Create: `app/src-tauri/src/state.rs`
- Modify: `app/src-tauri/src/lib.rs`

**设计：** 业务逻辑写成不依赖 tauri 宏的纯函数（可单测），`#[tauri::command]` 只做薄包装。Library 为同步 rusqlite，用 `Mutex` 共享。错误统一 `Result<T, String>`（人话消息）。

- [ ] **Step 1: state.rs**

```rust
use crate::config::Config;
use liquimod_core::library::Library;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    pub config: Mutex<Config>,
    pub config_path: PathBuf,
    pub library: Mutex<Library>,
}

impl AppState {
    /// 启动：读配置 → 打开（或初始化）库。
    pub fn bootstrap() -> Self {
        let config = Config::load();
        let library = Library::open(&config.library_root)
            .or_else(|_| Library::init(&config.library_root))
            .expect("无法打开 Mod 库");
        Self {
            config_path: Config::config_path(),
            config: Mutex::new(config),
            library: Mutex::new(library),
        }
    }
}
```

- [ ] **Step 2: commands.rs（纯函数 + 薄命令）**

```rust
use crate::config::Config;
use crate::state::AppState;
use liquimod_core::deploy::Deployer;
use liquimod_core::games::{Game, CharacterInfo};
use liquimod_core::library::Library;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConfigDto {
    pub library_root: String,
    pub mods_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CharacterSummary {
    pub internal_name: String,
    pub display_name: String,
    pub image: Option<String>,
    pub total: usize,
    pub enabled: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ModDto {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub installed_at: i64,
}

pub fn config_dto(c: &Config) -> ConfigDto {
    ConfigDto {
        library_root: c.library_root.display().to_string(),
        mods_dir: c.mods_dir.as_ref().map(|p| p.display().to_string()),
    }
}

/// 角色网格汇总：游戏角色按数据顺序在前，未匹配的 Mod 归入最后的 "Others"。
pub fn character_summaries(
    lib: &Library,
    game: &dyn Game,
) -> Result<Vec<CharacterSummary>, String> {
    let mods = lib.list().map_err(|e| e.to_string())?;
    let mut out: Vec<CharacterSummary> = Vec::new();
    for c in game.characters() {
        let group: Vec<_> = mods.iter().filter(|m| m.character == c.internal_name).collect();
        out.push(summary(c, group.len(), group.iter().filter(|m| m.enabled).count()));
    }
    let known: Vec<&str> = game.characters().iter().map(|c| c.internal_name.as_str()).collect();
    let others: Vec<_> = mods
        .iter()
        .filter(|m| !known.contains(&m.character.as_str()))
        .collect();
    if !others.is_empty() {
        out.push(CharacterSummary {
            internal_name: "Others".into(),
            display_name: "其他".into(),
            image: None,
            total: others.len(),
            enabled: others.iter().filter(|m| m.enabled).count(),
        });
    }
    Ok(out)
}

fn summary(c: &CharacterInfo, total: usize, enabled: usize) -> CharacterSummary {
    CharacterSummary {
        internal_name: c.internal_name.clone(),
        display_name: c.display_name.clone(),
        image: Some(c.image.clone()),
        total,
        enabled,
    }
}

pub fn mod_list(lib: &Library, character: &str) -> Result<Vec<ModDto>, String> {
    let mut mods: Vec<ModDto> = lib
        .list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|m| m.character == character)
        .map(|m| ModDto {
            id: m.id,
            name: m.name,
            enabled: m.enabled,
            installed_at: m.installed_at,
        })
        .collect();
    mods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(mods)
}

pub fn set_enabled(
    lib: &Library,
    mods_dir: Option<&Path>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let mods_dir = mods_dir.ok_or("未配置 3Dmigoto Mods 目录，请先选择目录")?;
    if !mods_dir.is_dir() {
        return Err(format!("Mods 目录不存在：{}", mods_dir.display()));
    }
    let deployer = Deployer::new(lib, mods_dir);
    let r = if enabled {
        deployer.enable(id)
    } else {
        deployer.disable(id)
    };
    r.map_err(|e| e.to_string())
}

pub fn set_mods_dir(c: &mut Config, path: PathBuf) -> Result<ConfigDto, String> {
    if !path.is_dir() {
        return Err(format!("目录不存在：{}", path.display()));
    }
    c.mods_dir = Some(path);
    Ok(config_dto(c))
}

// ---- Tauri 薄命令 ----

#[tauri::command]
pub fn get_config(state: tauri::State<AppState>) -> ConfigDto {
    config_dto(&state.config.lock().unwrap())
}

#[tauri::command]
pub fn choose_mods_dir(state: tauri::State<AppState>, path: String) -> Result<ConfigDto, String> {
    let mut config = state.config.lock().unwrap();
    let dto = set_mods_dir(&mut config, PathBuf::from(path))?;
    config
        .save_to(&state.config_path)
        .map_err(|e| format!("配置保存失败：{e}"))?;
    Ok(dto)
}

#[tauri::command]
pub fn get_characters(state: tauri::State<AppState>) -> Result<Vec<CharacterSummary>, String> {
    let lib = state.library.lock().unwrap();
    character_summaries(&lib, liquimod_core::games::hsr::Hsr::shared())
}

#[tauri::command]
pub fn list_mods(state: tauri::State<AppState>, character: String) -> Result<Vec<ModDto>, String> {
    let lib = state.library.lock().unwrap();
    mod_list(&lib, &character)
}

#[tauri::command]
pub fn set_mod_enabled(
    state: tauri::State<AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    let mods_dir = state.config.lock().unwrap().mods_dir.clone();
    let lib = state.library.lock().unwrap();
    set_enabled(&lib, mods_dir.as_deref(), id, enabled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liquimod_core::games::hsr::Hsr;

    fn temp_lib() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::init(dir.path()).unwrap();
        (dir, lib)
    }

    #[test]
    fn summaries_group_mods_and_bucket_others() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "M1").unwrap();
        lib.add_folder(src.path(), "Stranger", "M2").unwrap();
        let out = character_summaries(&lib, Hsr::shared()).unwrap();
        let acheron = out.iter().find(|c| c.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.total, 1);
        let others = out.iter().find(|c| c.internal_name == "Others").unwrap();
        assert_eq!(others.total, 1);
        assert_eq!(others.image, None);
    }

    #[test]
    fn mod_list_filters_and_sorts() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        lib.add_folder(src.path(), "Acheron", "B").unwrap();
        lib.add_folder(src.path(), "Acheron", "A").unwrap();
        lib.add_folder(src.path(), "Bailu", "C").unwrap();
        let mods = mod_list(&lib, "Acheron").unwrap();
        assert_eq!(mods.iter().map(|m| m.name.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
    }

    #[test]
    fn set_enabled_requires_mods_dir() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        let entry = lib.add_folder(src.path(), "Acheron", "M").unwrap();
        let err = set_enabled(&lib, None, entry.id, true).unwrap_err();
        assert!(err.contains("Mods 目录"));
    }

    #[test]
    fn set_enabled_creates_and_removes_junction() {
        let (_d, lib) = temp_lib();
        let src = tempfile::tempdir().unwrap();
        fs::write(src.path().join("f.txt"), "x").unwrap();
        let entry = lib.add_folder(src.path(), "Acheron", "M").unwrap();
        let mods = tempfile::tempdir().unwrap();
        set_enabled(&lib, Some(mods.path()), entry.id, true).unwrap();
        assert!(mods.path().join("Acheron - M").exists());
        set_enabled(&lib, Some(mods.path()), entry.id, false).unwrap();
        assert!(!mods.path().join("Acheron - M").exists());
    }

    #[test]
    fn set_mods_dir_rejects_missing() {
        let mut c = Config { library_root: PathBuf::from("x"), mods_dir: None };
        assert!(set_mods_dir(&mut c, PathBuf::from("C:/no/such/dir")).is_err());
        assert!(c.mods_dir.is_none());
    }
}
```

注：junction 链接名规则以 `Deployer::link_name` 实际实现为准（里程碑 1 已测）；若实际不是 `"<char> - <name>"`，测试断言改用 `Deployer::link_name(&entry)` 计算期望值。`use std::fs;` 别漏。

- [ ] **Step 3: lib.rs 接线**

`app/src-tauri/src/lib.rs`：

```rust
mod commands;
mod config;
mod state;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state::AppState::bootstrap())
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::choose_mods_dir,
            commands::get_characters,
            commands::list_mods,
            commands::set_mod_enabled,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

（脚手架若已有 run() 骨架/ greet 命令，替换之；保留 main.rs 不动。）

- [ ] **Step 4: 测试 + 质量门 + Commit**

```powershell
cargo test -p liquimod-app
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt
```
Expected: 5 PASS。
```powershell
git add app/src-tauri
git commit -m "feat(app): IPC 命令层（角色汇总/Mod 列表/启停/配置）"
```

---

### Task 5: 前端设计系统 + API 封装

**Files:**
- Modify: `app/vite.config.ts`
- Modify: `app/src/app.css`（模板自带则改写）
- Create: `app/src/lib/api.ts`
- Modify: `app/package.json`（devDeps）

**设计 tokens 直接取自设计文档 §7**（浅色 rgba(255,255,255,.28) / 深色 rgba(28,30,42,.38)、blur(28px) saturate(1.75)、发丝高光、去彩色化阴影、圆角 26/20/18/胶囊、强调色 #0A84FF / #409CFF）。

- [ ] **Step 1: 装 Tailwind v4 + 接 vite/sveltekit**

```powershell
cd app
npm i -D tailwindcss @tailwindcss/vite
```

`app/vite.config.ts` 全文：

```ts
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [sveltekit(), tailwindcss()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
});
```

`app/svelte.config.js` 把静态目录指向单一资产来源（立绘直接在 `/images/<file>` 可访问，dev 与打包一致）：

```js
import adapter from "@sveltejs/adapter-static";

export default {
  kit: {
    adapter: adapter({ fallback: "index.html" }),
    files: { assets: "../assets/hsr" },
  },
};
```

（保留模板原有的 adapter 配置项，仅追加 files.assets；adapter-static 需 `fallback: "index.html"` 供 Tauri SPA 模式。若模板已配置等价项则合并。）

- [ ] **Step 2: app.css（玻璃配方）**

`app/src/app.css` 全文：

```css
@import "tailwindcss";

:root {
  --glass-bg: rgba(255, 255, 255, 0.28);
  --glass-stroke: rgba(255, 255, 255, 0.45);
  --glass-highlight: rgba(255, 255, 255, 0.7);
  --surface: #f2f2f7;
  --text: #1c1c1e;
  --text-secondary: #6e6e73;
  --accent: #0a84ff;
  --accent-fill: rgba(10, 132, 255, 0.14);
  --shadow-soft:
    inset 0 0.5px 0 var(--glass-highlight),
    0 8px 24px rgba(0, 0, 0, 0.08),
    0 2px 6px rgba(0, 0, 0, 0.06);
}

@media (prefers-color-scheme: dark) {
  :root {
    --glass-bg: rgba(28, 30, 42, 0.38);
    --glass-stroke: rgba(255, 255, 255, 0.14);
    --glass-highlight: rgba(255, 255, 255, 0.16);
    --surface: #1a1b26;
    --text: #f2f2f7;
    --text-secondary: #98989f;
    --accent: #409cff;
    --accent-fill: rgba(64, 156, 255, 0.16);
    --shadow-soft:
      inset 0 0.5px 0 var(--glass-highlight),
      0 8px 24px rgba(0, 0, 0, 0.35),
      0 2px 6px rgba(0, 0, 0, 0.28);
  }
}

html,
body {
  height: 100%;
  margin: 0;
  background: var(--surface);
  color: var(--text);
  font-family:
    "Segoe UI Variable",
    "Segoe UI",
    "PingFang SC",
    "Microsoft YaHei",
    system-ui,
    sans-serif;
  user-select: none;
  overflow: hidden;
}

.glass {
  background: var(--glass-bg);
  backdrop-filter: blur(28px) saturate(1.75);
  -webkit-backdrop-filter: blur(28px) saturate(1.75);
  border: 0.5px solid var(--glass-stroke);
  box-shadow: var(--shadow-soft);
}

.radius-window { border-radius: 26px; }
.radius-panel { border-radius: 20px; }
.radius-card { border-radius: 18px; }
.radius-pill { border-radius: 9999px; }

.accent-text { color: var(--accent); }
.accent-fill { background: var(--accent-fill); }
.text-secondary { color: var(--text-secondary); }
```

- [ ] **Step 3: api.ts（invoke 封装 + 类型）**

`app/src/lib/api.ts`：

```ts
import { invoke } from "@tauri-apps/api/core";

export interface ConfigDto {
  library_root: string;
  mods_dir: string | null;
}

export interface CharacterSummary {
  internal_name: string;
  display_name: string;
  image: string | null;
  total: number;
  enabled: number;
}

export interface ModDto {
  id: number;
  name: string;
  enabled: boolean;
  installed_at: number;
}

export const api = {
  getConfig: () => invoke<ConfigDto>("get_config"),
  chooseModsDir: (path: string) => invoke<ConfigDto>("choose_mods_dir", { path }),
  getCharacters: () => invoke<CharacterSummary[]>("get_characters"),
  listMods: (character: string) => invoke<ModDto[]>("list_mods", { character }),
  setModEnabled: (id: number, enabled: boolean) =>
    invoke<void>("set_mod_enabled", { id, enabled }),
};

/// 立绘 URL（vite publicDir 指向 assets/hsr）。
export function portraitUrl(image: string): string {
  return `/images/${image}`;
}

/// 网格搜索过滤（不区分大小写，匹配显示名与内部名）。
export function filterCharacters(
  list: CharacterSummary[],
  query: string,
): CharacterSummary[] {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (c) =>
      c.display_name.toLowerCase().includes(q) ||
      c.internal_name.toLowerCase().includes(q),
  );
}
```

- [ ] **Step 4: 全局样式挂载 + 验证构建 + Commit**

创建 `app/src/routes/+layout.svelte`（SvelteKit 全局布局，挂全局 CSS）：

```svelte
<script lang="ts">
  import "../app.css";
  let { children } = $props();
</script>

{@render children()}
```

Run: `cd app; npm run build`
Expected: 构建通过生成 build/（若 svelte-check 脚本存在也跑 `npm run check`）。
```powershell
git add app
git commit -m "feat(app): 液态玻璃设计 tokens 与前端 API 封装"
```

---

### Task 6: 主界面组件（Svelte 5 runes）

**Files:**
- Create: `app/src/lib/components/TitleBar.svelte`
- Create: `app/src/lib/components/SearchBar.svelte`
- Create: `app/src/lib/components/Toggle.svelte`
- Create: `app/src/lib/components/CharacterCard.svelte`
- Create: `app/src/lib/views/CharacterGrid.svelte`
- Create: `app/src/lib/views/CharacterDetail.svelte`
- Modify: `app/src/routes/+page.svelte`（全量替换模板内容，充当原计划的 App.svelte）

**交互：** 大标题导航 + 搜索胶囊；角色卡立绘铺满 + 底部渐隐 + 名字玻璃胶囊；点卡进入角色详情（Mod 列表，iOS 开关启停）；未配置 mods_dir 时启停报错提示并提供"选择目录"按钮（tauri-plugin-dialog JS API）。窗口无边框 → 自绘标题栏（拖拽区 + 最小化/最大化/关闭）。

- [ ] **Step 1: TitleBar.svelte**

```svelte
<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  const win = getCurrentWindow();
</script>

<div
  data-tauri-drag-region
  class="flex items-center justify-between h-10 px-3 shrink-0 select-none"
>
  <span class="text-sm font-semibold" data-tauri-drag-region>LiquiMod</span>
  <div class="flex gap-2">
    <button aria-label="最小化" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.minimize()}>–</button>
    <button aria-label="最大化" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.toggleMaximize()}>□</button>
    <button aria-label="关闭" class="glass radius-pill w-8 h-8 grid place-items-center" onclick={() => win.close()}>×</button>
  </div>
</div>
```

- [ ] **Step 2: SearchBar.svelte**

```svelte
<script lang="ts">
  let { value = $bindable("") }: { value: string } = $props();
</script>

<input
  bind:value
  type="search"
  placeholder="搜索角色…"
  class="glass radius-pill px-4 h-9 w-64 outline-none text-sm bg-transparent"
/>
```

- [ ] **Step 3: Toggle.svelte（iOS 开关）**

```svelte
<script lang="ts">
  let {
    checked,
    onchange,
  }: { checked: boolean; onchange: (next: boolean) => void } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  class="radius-pill relative w-11 h-7 transition-colors duration-200 shrink-0"
  style:background={checked ? "var(--accent)" : "var(--glass-stroke)"}
  onclick={() => onchange(!checked)}
>
  <span
    class="absolute top-0.5 left-0.5 w-6 h-6 rounded-full bg-white shadow transition-transform duration-200"
    style:transform={checked ? "translateX(16px)" : "translateX(0)"}
  ></span>
</button>
```

- [ ] **Step 4: CharacterCard.svelte**

```svelte
<script lang="ts">
  import { portraitUrl, type CharacterSummary } from "$lib/api";

  let {
    character,
    onclick,
  }: { character: CharacterSummary; onclick: () => void } = $props();
</script>

<button
  class="radius-card relative overflow-hidden aspect-[3/4] group cursor-pointer"
  {onclick}
>
  {#if character.image}
    <img
      src={portraitUrl(character.image)}
      alt={character.display_name}
      class="absolute inset-0 w-full h-full object-cover object-top"
      loading="lazy"
    />
  {:else}
    <div class="glass absolute inset-0 grid place-items-center text-4xl font-bold text-secondary">
      {character.display_name.slice(0, 1)}
    </div>
  {/if}
  <div class="absolute inset-x-0 bottom-0 h-1/3 bg-gradient-to-t from-black/45 to-transparent"></div>
  <div class="absolute bottom-2 inset-x-2 flex items-center justify-between">
    <span class="glass radius-pill px-3 py-1 text-sm font-medium text-white">
      {character.display_name}
    </span>
    {#if character.total > 0}
      <span class="glass radius-pill px-2 py-1 text-xs text-white">
        {character.enabled}/{character.total}
      </span>
    {/if}
  </div>
</button>
```

- [ ] **Step 5: CharacterGrid.svelte**

```svelte
<script lang="ts">
  import { filterCharacters, type CharacterSummary } from "$lib/api";
  import CharacterCard from "$lib/components/CharacterCard.svelte";

  let {
    characters,
    query,
    onselect,
  }: {
    characters: CharacterSummary[];
    query: string;
    onselect: (c: CharacterSummary) => void;
  } = $props();

  let filtered = $derived(filterCharacters(characters, query));
</script>

<div class="grid grid-cols-[repeat(auto-fill,minmax(160px,1fr))] gap-4 p-5 overflow-y-auto h-full">
  {#each filtered as c (c.internal_name)}
    <CharacterCard character={c} onclick={() => onselect(c)} />
  {/each}
  {#if filtered.length === 0}
    <p class="text-secondary col-span-full text-center mt-16">没有匹配的角色</p>
  {/if}
</div>
```

- [ ] **Step 6: CharacterDetail.svelte**

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CharacterSummary, type ModDto } from "$lib/api";
  import Toggle from "$lib/components/Toggle.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  let {
    character,
    modsDirConfigured,
    onback,
    onconfigured,
  }: {
    character: CharacterSummary;
    modsDirConfigured: boolean;
    onback: () => void;
    onconfigured: () => void;
  } = $props();

  let mods = $state<ModDto[]>([]);
  let error = $state("");

  onMount(async () => {
    mods = await api.listMods(character.internal_name);
  });

  async function toggle(mod: ModDto, next: boolean) {
    error = "";
    try {
      await api.setModEnabled(mod.id, next);
      mod.enabled = next;
    } catch (e) {
      error = String(e);
    }
  }

  async function pickModsDir() {
    const path = await open({ directory: true, title: "选择 3Dmigoto Mods 目录" });
    if (typeof path === "string") {
      await api.chooseModsDir(path);
      onconfigured();
    }
  }
</script>

<div class="flex flex-col h-full">
  <div class="flex items-center gap-3 px-5 pt-2">
    <button class="glass radius-pill px-3 h-8 text-sm" onclick={onback}>‹ 返回</button>
    <h2 class="text-xl font-bold">{character.display_name}</h2>
  </div>

  {#if !modsDirConfigured}
    <div class="glass radius-panel mx-5 mt-3 p-3 flex items-center justify-between">
      <span class="text-sm">未配置 3Dmigoto Mods 目录，无法启用 Mod</span>
      <button class="accent-fill accent-text radius-pill px-3 h-8 text-sm font-medium" onclick={pickModsDir}>
        选择目录
      </button>
    </div>
  {/if}
  {#if error}
    <p class="mx-5 mt-2 text-sm" style="color: var(--accent)">{error}</p>
  {/if}

  <div class="flex flex-col gap-2 p-5 overflow-y-auto">
    {#each mods as mod (mod.id)}
      <div class="glass radius-card px-4 py-3 flex items-center justify-between">
        <span class="font-medium">{mod.name}</span>
        <Toggle checked={mod.enabled} onchange={(next) => toggle(mod, next)} />
      </div>
    {/each}
    {#if mods.length === 0}
      <p class="text-secondary text-center mt-16">该角色还没有 Mod，拖入压缩包即可安装</p>
    {/if}
  </div>
</div>
```

- [ ] **Step 7: +page.svelte（替换模板，主界面装配）**

`app/src/routes/+page.svelte` 全文：

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CharacterSummary, type ConfigDto } from "$lib/api";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import SearchBar from "$lib/components/SearchBar.svelte";
  import CharacterGrid from "$lib/views/CharacterGrid.svelte";
  import CharacterDetail from "$lib/views/CharacterDetail.svelte";

  let config = $state<ConfigDto | null>(null);
  let characters = $state<CharacterSummary[]>([]);
  let query = $state("");
  let selected = $state<CharacterSummary | null>(null);

  async function refresh() {
    config = await api.getConfig();
    characters = await api.getCharacters();
  }

  onMount(refresh);
</script>

<div class="flex flex-col h-screen">
  <TitleBar />
  {#if selected}
    <CharacterDetail
      character={selected}
      modsDirConfigured={config?.mods_dir != null}
      onback={() => (selected = null)}
      onconfigured={refresh}
    />
  {:else}
    <header class="flex items-end justify-between px-5 pb-2">
      <h1 class="text-3xl font-bold">角色</h1>
      <SearchBar bind:value={query} />
    </header>
    <CharacterGrid {characters} {query} onselect={(c) => (selected = c)} />
  {/if}
</div>
```

（Task 5 的 +layout.svelte 已挂全局 CSS，此处无需再 import。）

- [ ] **Step 8: 构建验证 + Commit**

```powershell
cd app
npm run build
```
Expected: vite 构建成功（无类型错误；若有 svelte-check 脚本也跑 `npm run check`，没有则跳过）。
```powershell
git add app
git commit -m "feat(app): 液态玻璃主界面（角色网格 + Mod 启停）"
```

---

### Task 7: Vitest 组件测试 + 全量验收

**Files:**
- Modify: `app/package.json`（test 脚本 + devDeps）
- Modify: `app/vite.config.ts`（vitest 配置）
- Create: `app/src/lib/api.test.ts`
- Create: `app/src/lib/views/CharacterGrid.test.ts`
- Create: `app/src/lib/views/CharacterDetail.test.ts`

- [ ] **Step 1: 装测试依赖 + 脚本**

```powershell
cd app
npm i -D vitest jsdom @testing-library/svelte @testing-library/jest-dom
```

`app/package.json` scripts 加 `"test": "vitest run"`。

`app/vite.config.ts` 追加 test 字段（改用 vitest/config 的 defineConfig 承载类型）：

```ts
import { defineConfig } from "vitest/config";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [sveltekit(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  test: {
    environment: "jsdom",
  },
});
```

- [ ] **Step 2: api.test.ts（纯函数）**

```ts
import { describe, expect, it } from "vitest";
import { filterCharacters, portraitUrl, type CharacterSummary } from "./api";

const list: CharacterSummary[] = [
  { internal_name: "Acheron", display_name: "Acheron", image: "acheron.png", total: 2, enabled: 1 },
  { internal_name: "Firefly", display_name: "Firefly", image: "firefly.png", total: 0, enabled: 0 },
];

describe("filterCharacters", () => {
  it("空查询返回全部", () => {
    expect(filterCharacters(list, "  ")).toHaveLength(2);
  });
  it("按显示名或内部名过滤（不区分大小写）", () => {
    expect(filterCharacters(list, "fire")).map((c) => c.internal_name).toEqual(["Firefly"]);
    expect(filterCharacters(list, "ACHERON")).toHaveLength(1);
  });
  it("无匹配返回空", () => {
    expect(filterCharacters(list, "zzz")).toEqual([]);
  });
});

describe("portraitUrl", () => {
  it("拼接 images 路径", () => {
    expect(portraitUrl("acheron.png")).toBe("/images/acheron.png");
  });
});
```

注：`filterCharacters(list, "fire").map(...)` 写法若 TS 抱怨则拆两行。

- [ ] **Step 3: CharacterGrid.test.ts**

```ts
import { render, screen, fireEvent } from "@testing-library/svelte";
import { describe, expect, it, vi } from "vitest";
import CharacterGrid from "./CharacterGrid.svelte";
import type { CharacterSummary } from "$lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const characters: CharacterSummary[] = [
  { internal_name: "Acheron", display_name: "Acheron", image: "acheron.png", total: 3, enabled: 2 },
  { internal_name: "Others", display_name: "其他", image: null, total: 1, enabled: 0 },
];

describe("CharacterGrid", () => {
  it("渲染角色卡与启用计数", () => {
    render(CharacterGrid, { characters, query: "", onselect: () => {} });
    expect(screen.getByText("Acheron")).toBeTruthy();
    expect(screen.getByText("2/3")).toBeTruthy();
    expect(screen.getByText("其他")).toBeTruthy();
  });

  it("搜索过滤后显示空态", () => {
    render(CharacterGrid, { characters, query: "zzz", onselect: () => {} });
    expect(screen.getByText("没有匹配的角色")).toBeTruthy();
  });

  it("点击卡片触发 onselect", async () => {
    const onselect = vi.fn();
    render(CharacterGrid, { characters, query: "", onselect });
    await fireEvent.click(screen.getByText("Acheron"));
    expect(onselect).toHaveBeenCalledWith(characters[0]);
  });
});
```

- [ ] **Step 4: CharacterDetail.test.ts**

```ts
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import CharacterDetail from "./CharacterDetail.svelte";
import { invoke } from "@tauri-apps/api/core";
import type { CharacterSummary } from "$lib/api";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));

const character: CharacterSummary = {
  internal_name: "Acheron",
  display_name: "Acheron",
  image: "acheron.png",
  total: 1,
  enabled: 0,
};

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => mockedInvoke.mockReset());

describe("CharacterDetail", () => {
  it("加载并渲染 Mod 列表", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 7, name: "Summer Skin", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => expect(screen.getByText("Summer Skin")).toBeTruthy());
    expect(mockedInvoke).toHaveBeenCalledWith("list_mods", { character: "Acheron" });
  });

  it("点击开关调用 set_mod_enabled 并更新状态", async () => {
    mockedInvoke.mockResolvedValue([
      { id: 7, name: "Summer Skin", enabled: false, installed_at: 1 },
    ]);
    render(CharacterDetail, {
      character,
      modsDirConfigured: true,
      onback: () => {},
      onconfigured: () => {},
    });
    await waitFor(() => screen.getByRole("switch"));
    mockedInvoke.mockResolvedValue(undefined);
    await fireEvent.click(screen.getByRole("switch"));
    expect(mockedInvoke).toHaveBeenCalledWith("set_mod_enabled", { id: 7, enabled: true });
    await waitFor(() =>
      expect(screen.getByRole("switch").getAttribute("aria-checked")).toBe("true"),
    );
  });

  it("未配置 mods_dir 时显示配置提示", async () => {
    mockedInvoke.mockResolvedValue([]);
    render(CharacterDetail, {
      character,
      modsDirConfigured: false,
      onback: () => {},
      onconfigured: () => {},
    });
    expect(screen.getByText("选择目录")).toBeTruthy();
  });
});
```

- [ ] **Step 5: 全量验收（全部必须绿）**

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cd app
npm test
npm run build
npm run tauri build -- --no-bundle
```
Expected：cargo 全绿；vitest 全 PASS；vite 构建成功；tauri --no-bundle 编译出 exe（证明 Tauri 壳完整可构建）。

- [ ] **Step 6: 人工 smoke（记录给主人）**

```powershell
cd app
npm run tauri dev
```
检查项：无边框窗口可拖拽/最小化/关闭；角色网格显示立绘；搜索过滤；点角色进详情；配置 Mods 目录后开关可启停（文件系统出现/消失 Junction）；系统明暗切换时界面跟随。

- [ ] **Step 7: Commit**

```powershell
git add app
git commit -m "test(app): Vitest 组件测试与 API 单测"
```

---

## 终审对照（自检记录）

- 设计 §7 全部 tokens → Task 5 Step 2 ✅
- 角色网格/搜索（MVP 范围）→ Task 6 ✅
- 启停（Junction）→ Task 4 set_enabled + Task 6 Toggle ✅
- 前端 Vitest + mock IPC（§8）→ Task 7 ✅
- 拖放安装、进度浮层、watcher、预设、设置页 → 明确留给 M4–M6 ✅
