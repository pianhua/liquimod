# LiquiMod 正式交接说明

> 交接版本：v0.5.0
>
> 交接日期：2026-08-22
>
> 当前主线：main
>
> 当前发布提交：430ad2b（Release v0.5.0）

本文件是本阶段工作的正式交接记录。交接后，后续开发者或 Agent 应以仓库代码、测试、CI 和本文件为现状依据；更早版本的交接文档仅作历史参考。

## 1. 交接状态

- v0.5.0 已合并到 main。
- PR：[pianhua/liquimod#11](https://github.com/pianhua/liquimod/pull/11)
- GitHub Release：[v0.5.0](https://github.com/pianhua/liquimod/releases/tag/v0.5.0)
- 发布工作流已完成前端检查、Rust 格式、刷新助手、Clippy、Rust 测试、主程序构建、NSIS、便携包、校验文件和 Release 上传。
- 交接时应保持 main 与 origin/main 对齐，工作区干净，版本标签为 v0.5.0。

发行资产：

- [LiquiMod-Windows-x64-setup.exe](https://github.com/pianhua/liquimod/releases/download/v0.5.0/LiquiMod-Windows-x64-setup.exe)
- [LiquiMod-Windows-x64.zip](https://github.com/pianhua/liquimod/releases/download/v0.5.0/LiquiMod-Windows-x64.zip)
- 安装器和 ZIP 各自带有 .sha256 校验文件。

## 2. v0.5.0 已完成内容

### 2.1 SRMI 骨骼蒙皮套件与 3DMigoto 渲染
- 内置开箱即用的 1071 行标准 `d3dx.ini` 模板，启用 `[Include]` 递归加载、`[Rendering]` (ini_params = 120, allow_buffer_resize = 1) 与 `global $costume_mods = 1`。
- 内置 `Core/SRMI/` 全套蒙皮着色器（`BatchedPose.ini`、`SingleSkinning.hlsl`、`MultiSkinning.hlsl`），彻底解决高精 Mod（如飞霄）身体隐形问题。
- 重构 3DMigoto 双包流式下载与智能配置合并（并行拉取 DLLs 与 Core 套件，保留用户自定义参数）。

### 2.2 原生无感启动与 Win32 Hook 注入
- 升级 `liquimod-refresh-helper` 为游戏原生伴侣，封装 `3dmloader.dll` / Win32 Hook API。
- 用户在 LiquiMod 点击启动游戏一气呵成完成拉起与注入，无需第三方 Loader.exe 弹窗与黑框。

### 2.3 存储与变量安全性
- 移除破坏性的 `isolate_ini_variables`，依托 3Dmigoto 原生路径命名空间与目录隔离，保障 Mod 差分按键与 SRMI 握手通信。
- 提供 Hash 碰撞与变量冲突只读诊断面板。

### 2.1 存储架构

- 默认大文件数据根目录优先放在软件所在盘的 LiquiModData。
- 核心仓库默认为 <数据根>\Library；托管 3DMigoto 工作区默认为 <数据根>\3DMigoto。
- %APPDATA%\LiquiMod\config.json 只保存配置和仓库位置等元数据。
- 设置页支持跨盘迁移：复制、统计、空间检查、SQLite 完整性检查、切换配置和重新对账。
- 迁移完成后旧仓库保留，用户明确执行清理旧仓库且游戏未运行时才删除。
- 迁移失败保留原仓库，半成品不能被当作完成状态。

### 2.2 托管 Mod 与外部 Mod

- 托管 Mod 由 LiquiMod 管理库内目录和部署索引。
- 外部 Mod 只记录源目录，不复制、移动、接管或删除源文件。
- 外部源离线显示警告，并禁止依赖源文件的启用、打开和写入。
- 断开外部连接只移除 LiquiMod 索引关系，外部文件夹保持不变。
- 文件监控覆盖托管库和已连接的外部来源。

### 2.3 游戏运行期间的 Mod 操作

| 操作 | 游戏运行中 | 说明 |
| --- | --- | --- |
| Junction 模式启用/禁用 | 允许 | 快速切换；禁用保留运行期目录，退出后清理 |
| CopyFallback 模式启用/禁用 | 禁止 | 避免运行时复制/删除实体文件 |
| 连接外部、安装、卸载 | 禁止 | 属于源文件或索引结构变更 |
| 重命名、移动、切换变体 | 禁止 | 需要改变库内容或运行副本 |
| 应用预设、修复部署、迁移仓库 | 禁止 | 需要批量或物理部署变更 |
| 手动 F10 热刷新 | 允许 | 仅由用户点击顶部热重载按钮触发 |

所有 Mod 开关都不自动发送 F10。用户改变 Mod 后自行判断何时点击 F10，以降低自动热重载的时序和兼容风险。

### 2.4 多 Mod、Hash 和 3Dmigoto

- 同一角色允许同时启用多个 Mod。
- 启用 1 个显示绿灯，2 个及以上显示黄灯；黄色提示可在设置中关闭。
- 这只是风险提示，不是互斥限制，也不是 Hash 冲突判定。
- 核心层保留 scan_mod_hashes、detect_conflicts、detect_variable_conflicts 和按键读取能力；当前版本不自动以 Hash 冲突阻止启停。
- 运行副本执行变量隔离，但不能声称所有 3Dmigoto 冲突都已解决。
- 真实 Mod 仍需结合实际 d3dx.ini 和游戏验收；变体由游戏内按键或 Mod UI 操作，LiquiMod 不自动发送 F10。

## 3. 当前架构速览

~~~text
liquimod/
├── crates/liquimod-core/
│   ├── archive/       压缩包识别、解压和安装
│   ├── assets_sync/   内置角色数据同步与文件校验
│   ├── d3d.rs         3Dmigoto INI、按键、Hash/变量检查
│   ├── db.rs          SQLite schema、迁移、Mod/预设/分类
│   ├── deploy.rs      Junction/CopyFallback 部署与恢复
│   ├── filesystem.rs  卷类型和部署策略选择
│   ├── library.rs     扫描、索引、托管/外部来源
│   ├── refresh.rs     游戏检测、helper 管道、手动 F10
│   ├── storage.rs     仓库统计、迁移和安全复制
│   ├── variants.rs    变体目录识别和运行副本
│   └── watch.rs       文件系统监控
├── crates/liquimod-refresh-helper/
│   └── src/main.rs    Windows F10 刷新助手
├── app/src-tauri/
│   ├── src/config.rs  配置、默认数据根和迁移状态
│   ├── src/state.rs   AppState、看门狗、Watcher、helper
│   ├── src/commands.rs Tauri IPC 和运行态防护
│   └── tauri.conf.json Tauri 资源与 NSIS 配置
├── app/src/
│   ├── lib/components/ UI 原子组件、Toggle、菜单、Toast、Tooltip
│   ├── lib/views/      角色列表、角色详情、设置
│   ├── lib/api.ts      IPC DTO 和前端 API
│   ├── lib/view.ts     搜索/滚动记忆与导航状态
│   └── app.css         主题 Token、玻璃容器和全局交互
└── .github/workflows/
    ├── ci.yml          PR/main 质量门禁
    └── release.yml     v* tag 的 Windows 发布流水线
~~~

## 4. 接手后的第一步

~~~powershell
git status --short --branch
git fetch origin
git switch main
git pull --ff-only origin main
Get-Content AGENTS.md
Get-Content STYLE.md
~~~

若工作区不是干净的，先确认改动归属，不能重置或删除。之后阅读相关模块、测试和最近 PR，再决定是否创建任务分支。

## 5. 标准开发、检查、合并和发布流程

完整规范在 AGENTS.md，关键要求如下。

### 5.1 必须先对齐

对行为、安全、数据和发布有影响的任务必须先复述用户目标、调查现状、说明方案利弊和风险、获得确认，再正式实现。不能在关键取舍尚未确认时把推测写进代码或发布版本。

### 5.2 分支和 PR

~~~powershell
git fetch origin
git switch main
git pull --ff-only origin main
git switch -c codex/<topic>

git diff --check
git add -- <明确文件路径>
git diff --cached --check
git commit -m "<type>: <summary>"
git push -u origin codex/<topic>
gh pr create --base main --head <owner>:codex/<topic>
~~~

禁止直接提交 main、禁止 git add . 或 git add -A、禁止 force push、禁止跳过 CI 合并。

### 5.3 合并 main 后再次检查

PR 前端和 Rust 检查全部通过、完成必要 Review 后才能合并。合并后必须同步 main、确认合并提交和工作区状态；涉及 Rust、Tauri、发布脚本、版本号或资源时，必须在合并后的 main 再跑本地门禁和生产构建。

### 5.4 发布

~~~powershell
git tag -a v<version> -m "LiquiMod v<version>"
git push origin v<version>
gh run list --repo pianhua/liquimod --workflow release.yml --limit 5
gh release view v<version> --repo pianhua/liquimod
~~~

只有 workflow 成功且 Release 资产完整时，才可以报告正式发布完成。

## 6. 当前质量门禁

~~~powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

Push-Location app
npm run check
npm test
npm run build
Pop-Location
~~~

发布或打包改动还要编译刷新助手、主程序并执行 scripts/build_package.ps1，检查 ZIP、安装器和 SHA256 文件。

## 7. 接受验收的重点

- 安装包和便携包在不同盘符运行，默认 LiquiModData 位于软件所在盘。
- 跨盘迁移、迁移中断、旧仓库保留和清理。
- 托管 Mod、外部 Mod、外部源离线/恢复，断开连接不删源文件。
- NTFS/ReFS Junction 与移动盘/非兼容卷 CopyFallback。
- 游戏运行期间 Junction 启停、CopyFallback 防护和退出后清理。
- 手动 F10，确认没有自动 F10。
- 同角色多个 Mod 的黄灯提示和设置开关。
- 真实 3Dmigoto Mod 的按键、变体、Hash/变量情况。
- 自定义排序首项、中间项、末项、滚动后拖拽没有错位或上弹抖动。
- Light/Dark 主题和亮/暗/透明立绘下所有按钮、菜单、弹窗、Toast 可读。

## 8. 已知边界与后续方向

以下不是 v0.4.0 发布阻塞项，而是后续可独立规划的工作：

1. 基于真实 3Dmigoto Mod 样本完善 Hash/变量冲突诊断 UI；没有用户确认前，不升级为自动互斥或自动停用。
2. 用更多真实游戏进程名、加载器和 Windows 权限环境进行实机验证。
3. 收集日常使用中的拖拽、主题对比度、外部目录变更和迁移边界问题。
4. 若未来引入批量操作、预设自动应用或自动 F10，必须单独完成需求对齐、风险评估和开关设计。

## 9. 正式交接结语

本阶段任务到此结束。v0.4.0 已完成代码、测试、PR、合并、构建和 GitHub Release 流程。后续由接手者依据 AGENTS.md 的对齐、分支、检查、合并和发布规则继续进行。

交接后的任何新方案都必须先与用户完全对齐，再进入实现；任何“看起来合理”的产品行为都不能替代用户确认。
