# LiquiMod 开发与 Agent 协作守则

> 当前基线：v0.1.0-beta.1（Windows 10/11 x64）
>
> 技术栈：Rust workspace + Tauri 2 + Svelte 5 + Tailwind CSS v4 + SQLite + WebView2

本文件只描述仓库的开发约束、产品基线和交付流程，不记录个人工作日志、交接过程或历史仓库清理过程。

## 1. 工作原则

涉及行为、数据、安全、游戏运行态、发布或用户体验的任务必须按以下顺序进行：

1. 复述目标、范围和不希望引入的副作用。
2. 调查相关代码、测试、配置和实际运行行为。
3. 说明方案、取舍、风险、影响范围和回滚方式。
4. 对会改变产品行为的关键边界取得明确确认。
5. 按确认后的方案实现，并进行与风险相称的验证。
6. 报告具体改动、验证结果、未验证项和剩余风险。

不得把推测写成已验证事实，也不得借修复问题之名顺手扩大范围。

## 2. 产品行为基线

### 2.1 Mod 启停与游戏运行态

- 同一角色允许同时启用多个 Mod；启用多个时显示风险提示，提示可在设置中关闭。
- 游戏运行期间不自动发送 F10。用户通过顶部热重载按钮手动发送 F10。
- 同卷 NTFS/ReFS Junction 模式下，游戏运行期间允许启用或禁用 Mod；禁用时保留运行期目录，退出后清理。
- 游戏运行期间禁止安装、连接外部 Mod、卸载、重命名、移动、切换变体、应用预设、修复部署、迁移仓库和清理旧仓库。
- 扫描可以更新索引，但游戏运行期间不能主动重建物理部署。

### 2.2 部署策略

- 当前只支持：应用数据根和 3Dmigoto \`Mods\` 目录位于同一 NTFS/ReFS 卷上的 Junction 部署。
- \`CopyFallback\` 只表示检测到无法安全使用 Junction 的路径条件；复制部署目前不支持，不得创建 Mod 的复制副本。
- 检测到 \`CopyFallback\` 时，启用、安装或修复等需要实体部署的操作必须失败并给出迁移到同卷 NTFS/ReFS 的明确提示。
- 清理逻辑可以识别并删除带有可信 LiquiMod 标记的历史复制目录，但不能把清理分支误认为 CopyFallback 已实现。
- Junction 只能删除已确认指向 LiquiMod 目标的链接；实体目录、未知文件或非预期目标必须报错停止。

### 2.3 Hash、变量和按键

- \`crates/liquimod-core/src/d3d.rs\` 提供 INI 解析、Mod 按键读取、Hash 冲突检测、变量冲突检测和运行副本变量隔离。
- 当前产品策略不以 Hash 检查阻止启停，也不自动执行冲突拦截；这些接口用于诊断和后续界面决策。
- 必须区分“核心可检测”和“当前界面自动调用或限制”。真实 3Dmigoto Mod 的 \`d3dx.ini\`、\`[Key]\`、\`[Constants]\`、Hash 和变体结构可能不同，不能假设统一格式。

### 2.4 存储与外部 Mod

- 便携版配置位于程序目录下的 \`config/config.json\`；旧版 \`%APPDATA%/LiquiMod\` 只用于迁移兼容。
- 默认数据根目录优先为程序所在卷的 \`LiquiModData\`，其下包含 Library 和托管 3DMigoto 工作区。
- 设置页迁移必须复制、校验、切换配置并重新对账；旧仓库在用户明确清理前保留。
- 外部 Mod 只保存规范化来源路径和索引，不复制、移动、接管或删除源文件。
- 外部源离线时显示警告，并禁止依赖源文件的启用、打开和写入操作。

## 3. 仓库结构与职责

\`\`\`text
liquimod/
├── crates/liquimod-core/            Rust 核心：数据库、扫描、归档、部署、3Dmigoto、诊断
├── crates/liquimod-cli/             CLI 调试工具
├── crates/liquimod-refresh-helper/  Windows 权限分离助手（注入 / F10）
├── app/src-tauri/                   Tauri 生命周期、状态、IPC 和配置
├── app/src/                         Svelte 页面、组件、主题和前端状态
├── assets/                          内置角色数据和默认资源
├── scripts/                         本地清理、图标和打包脚本
├── .github/workflows/               CI 与 tag 发布流水线
└── AGENTS.md                        开发与 Agent 协作守则
\`\`\`

核心模块职责：

- \`config.rs\`：配置路径、默认数据根、迁移状态和 3DMigoto 托管路径。
- \`state.rs\`：配置、Library、Watcher、刷新助手、游戏看门狗和延迟清理。
- \`commands/\`：Tauri IPC、参数校验、运行态防护和异步阻塞任务调度。
- \`library.rs\` / \`db.rs\`：索引、SQLite 迁移、托管/外部来源、分类、排序和预设。
- \`storage.rs\`：仓库统计、跨盘迁移、安全复制、完整性校验和迁移报告。
- \`deploy.rs\` / \`filesystem.rs\`：Junction 部署、策略检测、恢复和对账。
- \`d3d.rs\`：3Dmigoto INI、按键、Hash/变量检查和变量隔离。
- \`refresh.rs\` 与 \`liquimod-refresh-helper\`：进程检测、命名管道和手动 F10。
- \`watch.rs\`：Library 和外部来源监控。
- \`app/src/lib\`：页面、组件、视图状态和交互。

## 4. 数据安全与实现铁律

1. Mod 启停或物理部署变更必须遵循 SQLite \`op_log\` 写前记录和恢复路径。
2. 恢复和部署修复必须以数据库 \`enabled\` 状态为准，对磁盘部署进行安全对账。
3. 旧复制目录只能在存在可验证 LiquiMod 标记且通过归属检查时清理。
4. 运行副本清理必须考虑游戏文件句柄，不能强制删除用户或游戏正在使用的目录。
5. 压缩包、数据库、文件扫描和系统 API 等阻塞操作不得卡住 Tauri/UI 主线程。
6. 相对路径先经 \`safe_path\` 净化，再做 containment 校验；压缩包必须阻止绝对路径、盘符、UNC、\`..\`、链接逃逸和资源配额超限。
7. 不使用 \`std::mem::transmute\` 延长生命周期；Windows \`unsafe\` 只能封装在最小 OS 边界，并校验输入、句柄和返回值。
8. 删除前确认绝对目标、归属和内容；不确定时宁可失败。
9. 后台分类同步只处理 \`category_id IS NULL\` 的新条目，不能覆盖用户手动分类。
10. 异步刷新、扫描和安装必须防止过期响应覆盖新状态；切换仓库后重新绑定 Library、Watcher 和部署状态。
11. 外部 Mod 的索引与物理源分离：索引失败不能删除源文件，源离线不能伪装成托管 Mod。

## 5. 本地质量门禁

提交前必须执行：

\`\`\`powershell
cargo fmt --all -- --check
cargo build --release -p liquimod-refresh-helper
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

Push-Location app
npm ci
npm run check
npm test
npm run build
Pop-Location
\`\`\`

发布包或 Tauri 配置变化时还要执行：

\`\`\`powershell
cargo build --release -p liquimod-refresh-helper
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
\`\`\`

必须检查生成的 ZIP、NSIS 安装器、SHA256 文件和便携包内容。未做过的实机、故障注入或特殊文件系统验证必须明确标注为未验证。

## 6. Git 与 GitHub 工作规范

### 6.1 开始任务

\`\`\`powershell
git status --short --branch
git fetch origin
git switch main
git pull --ff-only origin main
git switch -c codex/<topic>
\`\`\`

若工作区有不属于当前任务的改动，先确认归属；不能覆盖、重置或顺手提交。禁止直接在 \`main\` 上开发和提交，禁止 force push。

### 6.2 提交

1. 修改前查看相关代码、测试和现有差异。
2. 每个提交保持单一意图。
3. 使用 \`git diff --check\` 和暂存区检查。
4. 只暂存确认属于任务的路径；禁止使用 \`git add .\`、\`git add -A\` 或 \`git add --all\`。
5. 提交信息使用清晰的单一意图描述，例如 \`fix: reject unsupported copy deployment\`。

### 6.3 PR 与合并

- PR 的 base 固定为 \`main\`，head 必须明确仓库所有者和分支。
- PR 描述必须包含问题、方案、风险、测试、未覆盖实机验证和回滚方式。
- CI 未全部通过不得合并；涉及 UI 时必须检查主题、拖拽、滚动、弹窗、Toast 和焦点。
- 合并后重新同步 \`main\`，并在合并后的提交上确认必要的构建和测试结果。

### 6.4 发布

- 版本号、tag 和发布说明必须在发布前统一，不能用 tag 代替验证。
- 只有在 \`main\` 上完成必要 CI、生产构建和用户验收后，才创建并推送 \`v<version>\` tag。
- 发布报告必须区分代码已合并、workflow 已通过、tag 已推送和 Release 已完成。

## 7. 交付报告

每个重要阶段都要提供可核对状态：

1. 开始：目标、调查范围、需要确认的取舍。
2. 实施中：已完成阶段、阻塞和下一步。
3. 验证：具体命令和结果。
4. 交付：改动、提交/PR/CI/发布状态、剩余风险和验收步骤。

不得隐瞒失败 CI、未经确认扩展范围、删除用户数据或在工作区存在不明改动时强行提交。
