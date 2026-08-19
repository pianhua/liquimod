# LiquiMod 项目交接文档

> 交接日期：2026-08-19
> 交接人：pianhua / ZCode 会话
> 接手后请先通读本文件 + `AGENTS.md` + `STYLE.md`，再动代码。

---

## 1. 项目概览

LiquiMod —— 崩坏：星穹铁道（3Dmigoto）Mod 管理器。
技术栈：Rust core + Tauri 2 + Svelte 5（runes）+ Tailwind v4，玻璃拟态 UI。

### 构建命令（顺序固定，不可反）

```bash
# 前端
cd app && npm install && npm run build
# 主程序（必须带 tauri/custom-protocol，否则 exe 不内嵌前端、只会导航到 devUrl）
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
# F10 刷新 helper（须与 liquimod-app.exe 同目录，运行时 current_exe().parent() 定位）
cargo build --release -p liquimod-refresh-helper
```

产物：`target/release/liquimod-app.exe` + `target/release/liquimod-refresh-helper.exe`
**改前端后必须 `npm run build` 再 `cargo build`**（先 build 再 build，顺序固定）。

### 质量检查

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd app && npm test && npm run check
```

注意：`cargo test` 只接受一个测试名/参数的 package filter，别写 `-p liquimod-core deploy:: thumbs::` 这种多 filter，按整包跑（`cargo test -p liquimod-core`）。

---

## 2. 当前代码状态

- 分支 `master`，最近提交 `c0f919a`（重命名当前分类时同步面包屑视图名）。
- 上一个**大重构已提交完成**：侧边栏固定 6 分类（角色/光锥/立绘/场景/NPC/其他）+ 分类安装 + 启用态筛选 + 设置页大改。
- **本次会话的积压 minor 清理改动，尚未提交（在工作区）**，见 §3。

### 即将接触的核心模块（都在 `crates/liquimod-core/src/`）
- `db.rs`：SQLite（rusqlite），`categories` 表 + `mods.category_id`（NULL = 角色视图）。
- `commands.rs`（Tauri app 层）：`character_to_category_id` 推导/安装/扫描归类，分类一处置维。
- `deploy.rs`：junction 创建/删除/`reconcile`。
- `thumbs.rs`：缩略图生成 + 孤儿 GC（本次新增）。
- `library.rs`：`scan()` 索引 + 缩略图 GC 接入（本次新增）。
- `layout.rs`：库目录结构。
- `games/`：`Game` trait（`id/characters/process_names`）、`Hsr::shared()`、`infer_character`。
- 前端在 `app/src/lib/`：`view.ts`（view 状态机 + 滚动记忆）、`Sidebar.svelte`、`+page.svelte`、`InstallOverlay.svelte`、`Settings.svelte`、`ModCard.svelte`、`ModRow.svelte`、`CategoryMenu.svelte`、`Toggle.svelte`。

---

## 3. 本次会话：积压 minor 清理（已完成，未提交）

针对 HANDOVER 已知积压，属于"稳固当前版本、不做扩展"的收尾。**以下改动都在工作区，改完后请统一提交**。

### 3.1 缩略图孤儿 GC（`thumbs.rs` + `library.rs`）
- 新增 `pub fn gc_thumbnails(library_root, valid_ids)`：`thumbs/` 里揭示了已不存在 mod id 的 `{id}.jpg` 缓存一律删除。幂等静默，无目录/文件占用时跳过。
- 只在 `library.rs` 的 `scan()` 尾部（索引对齐、清掉过期 mod 之后）把 `list_mods()` 的 id 收集成 `HashSet` 调用一次。
- 附带测试 `gc_removes_orphan_thumbs_keeps_valid_and_temp`。
- 写临时文件格式为 `{id}.jpg.{uuid}.tmp`，其 stem 无法 parse 成裸 id，天然不会误删生成中的文件。

### 3.2 `reconcile` O(n²) → O(1)（`deploy.rs`）
- `managed_links` 从 `Vec<String>` 改为 `HashSet<String>`（`push`→`insert`）。

### 3.3 junction 目标漂移自愈（`deploy.rs`）
- reconcile 的 enabled 分支里新增校验：`junction::get_target(&link)` 是否仍指向 `layout.root.join(rel_path)`；
  库目录移动/重定向后漂移，则拆旧重建。
- 附带测试 `reconcile_heals_drifted_junction_target`。

### 3.4 日志时间戳（已实现，无需改）
- `Settings.svelte` 的 `formatLog` 已把 tracing 的 UTC RFC3339 转成本地时间（早前提交 `3945a82`）。

### 3.5 ModRow 键盘可达性（已实现，无需改）
- `ModRow.svelte` 已有 `tabindex="0" / role="listitem" / aria-label / onRowKeydown`（空格/Enter 切换启用）。

---

## 4. 唯一剩余待办：前端 A11y warnings 清理（最后积压项）

`svelte-check` 目前 **0 errors、6 warnings**，分布两文件。修完后跑 §1 全套验证并连同 §3 提交。

1. **`ModCard.svelte:185`** 与 **`ModRow.svelte:150`** 的 `autofocus` 警告（`a11y_autofocus`）。
   - 这是重命名输入框**故意要自动聚焦**，不能去掉行为。改用小的 Svelte action（如 `function focusOn(el){ el.focus() }` + `<input use:focusOn …/>` 或现有 helper）替换，通过 lint 且聚焦行为不变。
2. **两文件 img 的 `onerror` 各 2 条警告**（共 4 条）：`onerror={(e)=>((e.currentTarget as HTMLImageElement).style.display="none")}`，用于坏图隐藏。
   - 触发「tabIndex 非负 / 非交互元素监听 mouse/keyboard 事件」类告警。根因在相关 img 的 `alt` 缺失/脚本监听，或兄弟占位 `div` 的事件/tabindex 问题。
   - **接手第一步先通读两文件 `<script>` + 相关 markup（img + 无图占位 div）定位根因，再最小化修**，别为插改动删大段文本（上一会话曾因此破坏测试，靠读坏 context 精准 Edit 才恢复）。

修完立刻跑：
```bash
npm run check   # 期望 0 warnings
npm test
```
再跑 §1 的 cargo test / clippy / fmt，最后全量构建并提交。

### 提交建议
本次会话改动（§3 + §4）归一次 commit；若分开，按「后端 core（GC/reconcile/漂移）」与「前端 a11y」两个子提交。

---

## 5. 接手者反复踩的坑（重要）

1. **工具死循环**：本会话反复出现过填充重复占位文本 + 反复调工具的循环。接手时保持「读文件 → 精准 Edit（小锚点）→ 验证」的节奏，不要为了延宕而重复调工具。
2. **`HANDOVER.md` 根目录被忽略**：根目录 `HANDOVER.md` 在 `.gitignore`（`/HANDOVER.md`）中，它是本地草稿交接，**绝不能提交**。本文件是 `docs/HANDOVER.md`，可提交（本会话新建，交接用）。
3. **图片识别**：一律走 `mcp__mimo-image__analyze_image`（MiMo），**绝不要把图片喂给主模型**（主模型读图会坏）。截图识别用 CDP 截图 → MiMo 复核。
4. **前端中文文件**：必须用 Edit/Write 工具，勿用 PowerShell 写（编码会损坏）。
5. **改 UI 前先读 `STYLE.md`**（铁律）：
   - 图标操作按钮统一 `w-8 h-8 glass radius-pill`（32px 玻璃圆钮），禁用 24px 透明小图标残留。
   - 圆角：按钮/菜单项 8px、卡片 `radius-card`(18)、面板 `radius-panel`(20)、胶囊 `radius-pill`，不硬写 `rounded-xl`。
   - 按钮高度三层：`h-9` 主 / `h-8` 标准 / `h-7` 小，同层严格同规格。
   - 颜色一律用 app.css 的 `--*` 变量，禁止硬编码色值；亮/暗两主题都须验证对比度。
   - 文本层级：页标题 `text-2xl`、主文本/按钮 `text-sm`、次要 `text-xs text-secondary`。
   - 浮层面板（预设/分类菜单）祖先必须自带定位 + z-index（Toolbar 是 `relative z-30`）——transform 的卡片会建层叠上下文盖住无定位面板。
6. **CDP 调试**：跑 exe 必须带 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"`；工具（`cdpeval.mjs` / `cdpshot.mjs`）在 `%LOCALAPPDATA%\Temp\opencode\`，用 node 解析 JSON（本机无 jq）。WebView2 出异常先删 `%LOCALAPPDATA%\com.liquimod.app\EBWebView`。CDP 探针点弹层项要 scope 在面板元素内（按文本找按钮会误命中侧边栏导航）。
7. **WebView2 grid 行高**：grid auto 行高计算无视 item 的 aspect-ratio/padding-top 撑高——UI 勿依赖内容撑高定 grid 行高，`CharacterGrid` 用 ResizeObserver 显式写 `grid-auto-rows`。
8. **CDP ECONNREFUSED**：运行的 exe 没带 debug port 启动就会这样；kill 后带 env var 重启。

---

## 6. 存档说明 / 遗留方向（非本次任务，仅备忘）

- **多游戏扩展未做**：分类体系仍耦在 HSR 角色表上；若要扩展到其他游戏，需把「分类由 `character` 推导 `category_id`」的逻辑下沉去游戏化（下沉到 `Game` trait）。本次有意不做。
- 上一个大重构的完整设计与说明见 `docs/superpowers/plans/`（尤其 `2026-08-18-liquimod-categories-ui.md`）。

---

## 7. 快速验证清单（每次改动后）

- [ ] `cargo test --workspace`
- [ ] `cargo clippy --workspace --all-targets`
- [ ] `cargo fmt --all -- --check`
- [ ] `cd app && npm test`
- [ ] `cd app && npm run check`（0 errors，目标 0 warnings）
- [ ] 全量构建：`cd app && npm run build` → `cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml`
- [ ] CDP 实测关键流程 + 亮/暗截图识图复核