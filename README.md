# LiquiMod

面向《崩坏：星穹铁道》3Dmigoto Mod 的 Windows 现代化桌面管理器。LiquiMod 使用 Rust Core、Tauri 2、Svelte 5 和 Tailwind CSS 构建，提供 Mod 导入、分类、启停、预设、变体选择、SRMI 骨骼蒙皮套件、原生 Hook 无感注入和部署自愈。

[![Release](https://img.shields.io/badge/release-v0.5.0-blue.svg)](https://github.com/pianhua/liquimod/releases/tag/v0.5.0)
[![CI](https://github.com/pianhua/liquimod/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pianhua/liquimod/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00.svg)](https://svelte.dev/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## 当前版本

`v0.5.0` 引入了对标 XXMI 的工业级标准渲染与注入架构，内置完整的 SRMI Compute Shader 骨骼蒙皮套件与标准 `d3dx.ini` 模板，支持原生 Win32 Hook 无感游戏拉起与注入（无需第三方 Loader.exe 弹窗），并提供 3DMigoto 双包流式下载与智能配置合并。

- [下载 v0.5.0](https://github.com/pianhua/liquimod/releases/tag/v0.5.0)
- [查看 CI](https://github.com/pianhua/liquimod/actions)
- [查看 v0.5.0 完整变更](https://github.com/pianhua/liquimod/compare/v0.4.0...v0.5.0)

## 功能

### Mod 管理与 SRMI 渲染支持

- 内置开箱即用的 **SRMI 核心蒙皮套件** 与标准 `d3dx.ini` 模板，完美支持高精角色 Mod
- 导入文件夹以及 `.zip`、`.7z`、`.rar` Mod 压缩包
- 直接连接外部 Mod 文件夹，不复制、不接管、不删除源文件；源目录离线时显示告警并禁止启用
- 自动识别角色，支持手动归属和未分类 Mod 管理
- 启用、禁用、重命名、移动、断开外部连接或卸载托管 Mod
- 自定义封面、立绘预览、搜索、排序、收藏和拖拽排序
- 密码本支持加密压缩包的历史密码尝试

### 部署与兼容性

- NTFS 卷优先使用 Junction，启停时不复制 Mod 内容
- exFAT、移动盘或跨卷场景自动降级为 CopyFallback
- CopyFallback 使用 LiquiMod 标记保护运行目录，只清理由自身创建的部署
- 目录监控自动发现库内变化，并在游戏退出后完成物理部署对账
- SQLite `op_log` 写前事务日志支持崩溃恢复和启动自愈

### 数据存储

- 默认数据根目录位于软件所在盘的 `LiquiModData`，避免 Mod 库随着系统盘配置目录无限膨胀
- 设置页支持将核心仓库迁移到任意可用盘符；迁移采用复制、校验、原子切换，旧仓库在用户确认前保留
- 迁移范围包括 Mod 库、SQLite 数据库、封面缓存、日志以及由 LiquiMod 托管的 3DMigoto 工作区
- 外部 Mod 只记录来源路径；断开连接只移除 LiquiMod 索引，不会删除外部文件夹

### 变体与多 Mod 提示

- 识别 `Option`、编号和 `[Variant]` 等明确命名的变体目录
- 将基础资源与选中变体合并到运行副本，变体文件覆盖同路径基础文件
- 同一角色启用一个 Mod 时显示绿灯，启用多个时显示黄灯和详情警告；提示可在设置中关闭
- 提供 Mod Hash 与变量冲突诊断面板，帮助排查多 Mod 冲突

### 游戏集成与原生无感挂钩

- 内置原生 Win32 Hook 注入与游戏拉起引擎，一键启动并挂载 3DMigoto，告别外部 Loader.exe 弹窗与黑框
- 进程看门狗实时显示游戏运行状态
- 游戏运行期间允许同卷 NTFS/ReFS Junction Mod 启停；不限制同角色启用数量，启用多个 Mod 时以黄灯提示潜在风险
- 游戏运行期间继续阻止安装、卸载、重命名、移动、应用预设、文件变体和部署修复
- 支持常驻“热重载”按钮手动发送 F10 热刷新
- 独立的 `liquimod-refresh-helper.exe` 统一承接提权、F10 触发与 Hook 注入
- 设置页全面检查 WebView2、VC++、目录权限、SRMI 蒙皮套件就绪度与部署模式

## 安装与首次使用

1. 从 [v0.5.0 Release](https://github.com/pianhua/liquimod/releases/tag/v0.5.0) 下载 `LiquiMod-*.exe` 安装包或 `LiquiMod-Windows-x64.zip` 便携包。
2. 安装包支持选择安装位置；便携包解压后保持 `liquimod-app.exe` 与 `liquimod-refresh-helper.exe` 位于同一目录。
3. 启动 LiquiMod，在设置中确认数据存储根目录。默认位于软件所在盘的 `LiquiModData`，也可以迁移到其他盘符。
4. 在设置中配置游戏主程序路径（支持自动探测），点击“下载/更新 3DMigoto”即可一键完成环境初始化。
5. 导入 Mod；若不希望复制文件，可在角色详情中使用“连接外部”直接挂载已有文件夹。

3Dmigoto 和游戏文件需要用户自行准备。首次使用前建议先在设置页完成环境诊断；涉及 Junction、CopyFallback 或 F10 刷新的操作可能触发管理员权限提示。

## 安全边界

LiquiMod 的部署操作以保护用户文件为优先：

- ZIP、远端资产和图片路径经过相对路径净化与 containment 校验，拒绝绝对路径、盘符、UNC 和 `..` 越界路径。
- 遇到非空实体目录、未知文件或非 LiquiMod 部署目标时，部署器会报错并停止，不强制删除。
- 游戏运行期间，目录监控和全库扫描只更新索引，不主动重建 Junction 或复制部署目录；安装等文件变更会被明确阻止。
- 运行期禁用 Junction 时只拆除入口，可能仍被游戏引用的运行副本延迟到游戏退出后清理。
- 游戏退出后会自动进行一次部署对账；也可以在设置页手动执行部署修复。

## 系统要求

- Windows 10/11 x64
- Rust stable（仅源码构建需要）
- Node.js 22 和 npm（仅源码构建需要）
- Microsoft Edge WebView2 Evergreen Runtime
- 已安装并正确配置的 3Dmigoto 环境

Windows 11 通常已包含 WebView2；Windows 10 或精简系统可能需要单独安装。

## 从源码构建

### 开发模式

```powershell
cd app
npm ci
npm run tauri dev
```

### 构建前端与 Windows 程序

在仓库根目录执行：

```powershell
cd app
npm ci
npm run build
cd ..

cargo build --release -p liquimod-refresh-helper
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
```

构建产物：

- `target/release/liquimod-app.exe`
- `target/release/liquimod-refresh-helper.exe`

### 生成便携 ZIP

推荐使用仓库脚本，它会构建前端、编译两个 Windows 程序，并生成 ZIP 与 SHA256 校验文件：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
```

产物位于 `dist/`：

- `dist/LiquiMod-Windows-x64.zip`
- `dist/SHA256SUMS.txt`

## 质量门禁

提交前执行完整检查：

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

cd app
npm run check
npm test
npm run build
```

发布前必须以 CI 的实际结果为准。RAR 真实压缩包和 Windows 符号链接权限测试可能需要额外本机 fixture 或系统权限。

## 项目结构

```text
liquimod/
├── crates/liquimod-core/           Rust 核心库：数据库、扫描、归档、部署、诊断
├── crates/liquimod-cli/            CLI 调试与脚本工具
├── crates/liquimod-refresh-helper/ Win32 F10 刷新助手
├── app/src-tauri/                  Tauri 命令、状态和桌面生命周期
├── app/src/                        Svelte 前端界面与状态管理
├── assets/                         内置角色数据与默认资源
├── scripts/                        本地构建、打包和维护脚本
└── .github/workflows/              CI 与 Windows Release workflow
```

## 贡献

欢迎提交 Issue 和 Pull Request。提交前请确保：

1. 改动范围与问题描述一致，并补充必要测试。
2. 通过上方 Rust 与前端质量门禁。
3. 涉及部署、路径、数据库或游戏运行态时，说明 Windows 实机验证环境和结果。

## 免责声明

LiquiMod 是社区开发的独立工具，与 HoYoverse、miHoYo 或 3DMigoto 项目没有隶属关系。请遵守游戏服务条款、Mod 作者许可和当地法律法规，并自行备份重要数据。

## 许可证

本项目采用 [MIT License](LICENSE)。
