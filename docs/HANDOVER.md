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

- 分支 `master`。
- 上一个**大重构已提交完成**：侧边栏固定 6 分类（角色/光锥/立绘/场景/NPC/其他）+ 分类安装 + 启用态筛选 + 设置页大改。
- **积压清理与稳固基线已完成**：
  1. 缩略图孤儿 GC（`thumbs.rs` + `library.rs`）；
  2. `reconcile` $O(1)$ 优化（`deploy.rs`）；
  3. Junction 目标漂移自愈（`deploy.rs`）；
  4. 前端 A11y 警告彻底消除（`ModCard.svelte` + `ModRow.svelte` 实现 0 errors, 0 warnings）。

---

## 3. 稳固基线修复详情

### 3.1 缩略图孤儿 GC（`thumbs.rs` + `library.rs`）
- 新增 `pub fn gc_thumbnails(library_root, valid_ids)`：`thumbs/` 里揭示了已不存在 mod id 的 `{id}.jpg` 缓存一律删除。幂等静默，无目录/文件占用时跳过。
- 在 `library.rs` 的 `scan()` 尾部（索引对齐、清掉过期 mod 之后）把 `list_mods()` 的 id 收集成 `HashSet` 调用一次。
- 附带测试 `gc_removes_orphan_thumbs_keeps_valid_and_temp`。
- 写临时文件格式为 `{id}.jpg.{uuid}.tmp`，其 stem 无法 parse 成裸 id，天然不会误删生成中的文件。

### 3.2 `reconcile` O(n²) → O(1)（`deploy.rs`）
- `managed_links` 从 `Vec<String>` 改为 `HashSet<String>`（`push`→`insert`）。

### 3.3 junction 目标漂移自愈（`deploy.rs`）
- reconcile 的 enabled 分支里新增校验：`junction::get_target(&link)` 是否仍指向 `layout.root.join(rel_path)`；
  库目录移动/重定向后漂移，则拆旧重建。
- 附带测试 `reconcile_heals_drifted_junction_target`。

### 3.4 前端 A11y warnings 清理（已完成）
- `ModCard.svelte` 与 `ModRow.svelte` 的重命名自动聚焦改为 Svelte action（`use:focusOn`），消除 `a11y_autofocus` 警告并保留原生自动聚焦行为。
- 对承载键盘交互的 `role="listitem"` 容器添加显式规则声明，消除 `a11y_no_noninteractive_tabindex` 与 `a11y_no_noninteractive_element_interactions` 告警。
- `npm run check` 达到 **0 errors, 0 warnings**。

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