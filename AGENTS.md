# LiquiMod 开发手册 (Developer Guide)

崩铁（3Dmigoto）Mod 管理器。Rust core + Tauri 2 + Svelte 5。

## 1. 本地构建与运行

```bash
# 前端安装与构建
cd app && npm install && npm run build

# 主程序（必须带 tauri/custom-protocol，否则 exe 不内嵌前端）
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml

# F10 刷新 helper（必须与 liquimod-app.exe 同目录，运行时 current_exe().parent() 定位）
cargo build --release -p liquimod-refresh-helper
```

产物输出：`target/release/liquimod-app.exe` + `target/release/liquimod-refresh-helper.exe`。

---

## 2. 本地与云端质量检查

```bash
# 后端全量测试与 Clippy 诊断
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check

# 前端类型检查与 Vitest 测试 (95+ 测试用例)
cd app && npm run check && npm test
```

### ⚡ GitHub Actions CI/CD 流水线
- **CI Quality Gate**：每次 push/PR 触发，前后端完全并行起跑（前端极速 Ubuntu 30s 秒通，后端 Windows 增量缓存验证）；
- **Build & Release**：推送版本 Tag（如 `v*`）或在 GitHub 网页手动点击触发，全自动打包生成 `LiquiMod-Windows-x64.zip` 并发布 GitHub Release。

---

## 3. 架构铁律与设计规范

**完整设计规范与交互基准请查阅 `STYLE.md`**。

### 3.1 核心 UI 铁律
1. **图标操作按钮**：统一为 `w-8 h-8 glass radius-pill`（32px 玻璃圆钮），禁止私造 24px/28px 小按钮；
2. **按钮高度三档**：主要控制 `h-9`、标准操作 `h-8`、紧凑微控 `h-7`，同层严格同规格；
3. **圆角系统**：窗口 `radius-window`(26px)、面板 `radius-panel`(20px)、卡片 `radius-card`(18px)、矩形菜单项 `rounded-lg`(8px)、对象与胶囊 `radius-pill`；
4. **色彩与主题**：100% 采用 `app.css` CSS 变量（`--surface`, `--text`, `--accent`, `--glass-*` 等），严禁硬编码色值，亮/暗双主题必须全量验证对比度；
5. **物理触感**：所有开关统一使用 `<Toggle />`（具备 `:active` 横向果冻拉伸与回弹），所有按钮具备物理按压反馈（`scale(0.96)`）。

### 3.2 视图状态与架构铁律
- **全链路搜索记忆**：页面导航必须基于 `viewSearchMemory` 独立快照各个视图的搜索词，返回上一级时 100% 精准恢复过滤状态；
- **滚动记忆恢复**：脱离视图或打开设置前显式调用 `saveScroll()`，渲染后调用 `restoreScroll()`；
- **层叠上下文**：浮层面板祖先容器必须自带定位与层级声明（如 `relative z-30`）；
- **WebView2 渲染引擎避坑**：网格行高必须显式声明（如 `[grid-auto-rows:200px]`），勿依赖 item 内部撑高。
