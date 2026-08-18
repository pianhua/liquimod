# LiquiMod

崩铁（3Dmigoto）Mod 管理器。Rust core + Tauri 2 + Svelte 5。

## 构建

```bash
# 前端
cd app && npm install && npm run build
# 主程序（必须带 tauri/custom-protocol，否则 exe 不内嵌前端、导航到 devUrl）
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
# F10 刷新 helper（必须与 liquimod-app.exe 同目录，运行时 current_exe().parent() 定位）
cargo build --release -p liquimod-refresh-helper
```

产物：`target/release/liquimod-app.exe` + `target/release/liquimod-refresh-helper.exe`。
dev 模式（`cargo run`）下 helper 不存在于 target/debug，自动刷新会 toast 提示并跳过——正常。

## 测试 / 检查

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd app && npm test && npm run check
```

## 调试 WebView2

设 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` 启动 exe，然后 CDP 连 `http://localhost:9223/json`。WebView2 缓存位于 `%LOCALAPPDATA%\com.liquimod.app\EBWebView`，前端异常时先删。

已知引擎差异：WebView2 的 grid auto 行高计算无视 grid item 的 aspect-ratio/padding-top 撑高——UI 勿依赖内容撑高定 grid 行高（CharacterGrid 用 ResizeObserver 显式写 grid-auto-rows）。

## 里程碑 8（分类与新布局）

- 分类纯 DB：`categories` 表 + `mods.category_id`（NULL = 角色视图），磁盘目录不变；`category_id` 列迁移走 ALTER + 吞 duplicate column 惯例。
- 「角色」是虚拟分类，显示名 config.character_category_name 可改；「未分类」= 未归类且不属于已知角色（约等于角色网格的 Others 桶平铺）。
- 主题：config.theme = auto|light|dark，`document.documentElement.dataset.theme` 驱动 CSS 变量；auto 监听 prefers-color-scheme（theme.ts 单次挂监听，dataset.themeChoice 记录当前选择）。
- 布局：Sidebar（分类导航）+ Toolbar（面包屑/排序/预设）+ view 状态机（`$lib/view.ts`）；滚动记忆按 viewKey 存 Map，切视图前显式保存、刷新后恢复（Chromium display:none 会清零 scrollTop）。
- 浮层面板（预设/分类菜单）的祖先必须自带定位与 z-index（Toolbar 是 `relative z-30`）——transform 的卡片会建层叠上下文盖住无定位面板。
- CDP 探针注意：`[...document.querySelectorAll("button")]` 按文本找按钮会误命中侧边栏导航（如「武器」），点弹层项要 scope 在面板元素内。
