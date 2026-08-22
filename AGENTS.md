# LiquiMod 开发与 Agent 协作守则

> 当前基线：v0.6.1（Windows 10/11 x64）
>
> 技术栈：Rust workspace + Tauri 2 + Svelte 5 + Tailwind CSS v4 + SQLite + WebView2
>
> STYLE.md 负责 UI 规范，HANDOVER.md 负责当前交接状态，DECISIONS.md 负责已确认的产品取舍。本文件约束开发者、自动化 Agent 和后续接手者的工作方式。

## 1. 总原则：先对齐，再方案，再实现

涉及行为、数据、安全、游戏运行态、发布或用户体验的任务必须按以下顺序进行：

1. 复述用户目标、目标动作和不希望引入的副作用。
2. 调查代码、测试、配置、Issue/PR/CI 和实际运行行为。
3. 说明可行方案、取舍、风险、影响范围和回滚方式。
4. 获得用户对关键行为和边界的明确确认。
5. 形成与确认内容一致的方案并实现。
6. 运行验证并报告事实、未验证项和下一步。

存在会改变产品行为的歧义时，必须停在调查/对齐阶段，不得擅自决定。局部、低风险修复也要在开始时复述范围。

## 2. 当前产品行为基线

### 2.1 Mod 启停与游戏运行态

- 同一角色允许同时启用多个 Mod，不做数量互斥。
- 启用 1 个显示绿灯；启用 2 个及以上显示黄灯风险提示；提示可在设置中关闭。
- 游戏运行期间不自动发送 F10。用户通过顶部常驻热重载按钮手动发送 F10。
- Junction 策略下，游戏运行期间允许启用/禁用 Mod；禁用保留运行期目录，退出后清理。
- CopyFallback 策略下，游戏运行期间禁止会改变实体复制部署的启停操作。
- 游戏运行期间禁止安装、连接外部 Mod、卸载、重命名、移动、切换变体、应用预设、修复部署、迁移仓库和清理旧仓库。
- 扫描可更新索引，但游戏运行期间不能主动重建物理部署。

### 2.2 Hash、变量和按键

- crates/liquimod-core/src/d3d.rs 具备 INI 解析、Mod 按键读取、Hash 冲突检测、变量冲突检测和运行副本变量隔离能力。
- 当前产品策略不以 Hash 检查阻止启停，也不自动执行冲突拦截；这些接口保留给诊断、后续 UI 和进一步产品决策。
- 必须区分“核心可检测”和“当前界面自动调用/限制”。
- 真实 3Dmigoto Mod 的 d3dx.ini、[Key]、[Constants]、hash 和变体结构可能不同，不能假设统一格式。

### 2.3 存储与外部 Mod

- 便携版配置位于程序目录下的 `config/config.json`；旧版 `%APPDATA%/LiquiMod` 配置只用于迁移兼容，它不是大文件 Mod 仓库。
- 默认数据根目录优先为程序所在卷的 LiquiModData；其下为 Library 和托管 3DMigoto 工作区。
- 设置页可迁移到其他盘符；迁移复制、校验、切换配置并重新对账，旧仓库在用户明确清理前保留。
- 外部 Mod 只保存规范化来源路径和索引，不复制、移动、接管或删除源文件。
- 外部源离线显示警告，并禁止依赖源文件的启用、打开和写入操作。
- 所有路径操作都要经过安全边界检查；不得跟随未知链接、删除非 LiquiMod 目录或接受越界路径。

## 3. 仓库结构与职责

~~~text
liquimod/
├── crates/liquimod-core/            Rust 核心：数据库、扫描、归档、部署、3Dmigoto、诊断
├── crates/liquimod-cli/             CLI 调试工具
├── crates/liquimod-refresh-helper/  Windows F10 刷新助手
├── app/src-tauri/                   Tauri 生命周期、状态、IPC 和配置
├── app/src/                         Svelte 页面、组件、主题和前端状态
├── assets/                          内置角色数据和默认资源
├── scripts/                         本地清理、图标和打包脚本
├── .github/workflows/               CI 与 tag 发布流水线
├── STYLE.md                         UI 视觉与交互约束
├── AGENTS.md                        开发与 Agent 协作守则
├── DECISIONS.md                     已确认产品决策与暂缓项
└── HANDOVER.md                      当前版本交接与验收说明
~~~

核心模块：

- config.rs：配置路径、默认数据根、迁移状态和 3DMigoto 托管路径。
- state.rs：配置、Library、Watcher、刷新助手、游戏看门狗和延迟清理。
- commands.rs：Tauri IPC、参数校验、运行态防护和异步阻塞任务调度。
- library.rs / db.rs：索引、SQLite 迁移、托管/外部来源、分类、排序和预设。
- storage.rs：仓库统计、跨盘迁移、安全复制、完整性校验和迁移报告。
- deploy.rs / filesystem.rs：Junction、CopyFallback、部署、恢复和延迟清理。
- d3d.rs：3Dmigoto INI、按键、Hash/变量检查和变量隔离。
- refresh.rs 与 liquimod-refresh-helper：进程检测、命名管道和手动 F10。
- watch.rs：库和外部来源监控。
- view.ts、页面和组件：搜索/滚动记忆、UI 状态和交互。

## 4. 数据安全与实现铁律

1. Mod 启停或物理部署变更必须遵循 SQLite op_log 写前记录和恢复路径。
2. Junction 只能删除已确认指向 LiquiMod 目标的链接；实体目录、未知文件或非预期目标必须报错停止。
3. CopyFallback 只能清理由 LiquiMod 创建且带有可验证标记的运行目录。
4. 运行副本清理必须考虑游戏文件句柄，不能强制删除用户或游戏正在使用的目录。
5. 压缩包、数据库、文件扫描和系统 API 等阻塞操作不得卡住 Tauri/UI 主线程。
6. 相对路径先经 safe_path 净化，再做 containment 校验；压缩包必须阻止绝对路径、盘符、UNC、..、链接逃逸和资源配额超限。
7. 不使用 std::mem::transmute 延长生命周期；Windows unsafe 只能封装在最小 OS 边界，并校验输入、句柄和返回值。
8. 删除前确认绝对目标、归属和内容；不确定时宁可失败。
9. 后台分类同步只处理 category_id IS NULL 的新条目，不能覆盖用户手动分类。
10. 异步刷新、扫描和安装必须防止过期响应覆盖新状态；切换仓库后重新绑定 Library、Watcher 和部署状态。
11. 外部 Mod 的索引与物理源分离：索引失败不能删除源文件，源离线不能伪装成托管 Mod。

## 5. 本地质量门禁

提交前必须执行：

~~~powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

Push-Location app
npm ci
npm run check
npm test
npm run build
Pop-Location
~~~

发布包或 Tauri 配置变化时还要：

~~~powershell
cargo build --release -p liquimod-refresh-helper
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
~~~

必须检查 dist/ 中的 ZIP、NSIS 安装器、SHA256 文件和便携包内容。

## 6. 标准 GitHub 工作规范

所有进入 main 的功能、修复和文档变更遵循同一流程。

### 6.1 同步并开分支

~~~powershell
git status --short --branch
git fetch origin
git switch main
git pull --ff-only origin main
git switch -c codex/<topic>
~~~

若工作区有不属于当前任务的改动，先确认归属；不能覆盖、重置或顺手提交。禁止直接在 main 上开发和提交，禁止 force push。

### 6.2 开发、检查和提交

1. 修改前查看相关代码、测试和现有差异。
2. 每个提交保持单一意图。
3. 检查并只暂存确认属于任务的路径：

~~~powershell
git diff
git diff --check
git add -- path/to/file1 path/to/file2
git diff --cached --check
git commit -m "<type>: <summary>"
~~~

4. 禁止 git add .、git add -A 和 git add --all。
5. PR 前查看 git status、git diff main...HEAD 和测试结果。

### 6.3 推送和 PR

~~~powershell
git push -u origin codex/<topic>
gh pr create --base main --head <owner>:codex/<topic>
gh pr checks <number> --repo pianhua/liquimod
~~~

PR 的 base 固定为 main，head 必须明确仓库所有者和分支。未完成时使用 Draft，准备合并时转为 Ready for review。PR 描述必须包含问题、方案、风险、测试、未覆盖实机验证和回滚方式。CI 未全部通过不得合并。

### 6.4 合并 main 后再次检查

只有完成必要 Review、用户授权和必需 CI 后才能合并。合并完成后必须：

~~~powershell
gh pr merge <number> --squash --delete-branch=false
git fetch origin
git switch main
git pull --ff-only origin main
git log -1 --oneline --decorate
git status --short --branch
~~~

涉及 Rust、Tauri、发布脚本、版本号或资源变更时，必须在合并后的 main 再跑完整门禁和生产构建，不能只接受 PR 分支旧构建结果。

### 6.5 正式发布

1. 在发布分支完成版本号、README、变更说明、打包脚本和 workflow。
2. PR 合并到 main，确认 main 的合并提交就是要发布的提交。
3. 创建并推送 tag：

~~~powershell
git tag -a v<version> -m "LiquiMod v<version>"
git push origin v<version>
~~~

4. 等待 .github/workflows/release.yml，确认前端、Rust、测试、刷新助手、主程序、NSIS 和资产上传全部成功。
5. 确认 Release 非 Draft、非 Pre-release，且 ZIP、安装器和两个 SHA256 文件均存在。
6. 向用户报告准确 commit、PR、workflow、tag、Release URL 和验收风险；不能把“标签已推送”说成“发行已完成”。

## 7. UI 变更专项

UI 改动必须同时阅读 STYLE.md，至少检查 Light/Dark、亮暗透明立绘、所有按钮/Toggle/菜单/弹窗/Toast、拖拽首中末项、滚动列表、WebView2 层叠和焦点。启动本地开发服务器后，应使用浏览器验证可见页面，不得只凭源码推测 UI 正确。

## 8. Agent 交互格式

每个重要阶段都要提供可核对状态：

1. 开始：当前理解、调查范围、需要确认的取舍。
2. 实施中：已完成阶段、阻塞和下一步。
3. 验证：具体命令和结果。
4. 交付：改动、提交/PR/CI/发布状态、剩余风险和验收步骤。

禁止隐瞒失败 CI、未经对齐扩展范围、把推测写成事实、把旧文档当现状、删除用户数据或在工作区存在不明改动时强行提交。
