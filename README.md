# LiquiMod

面向《崩坏：星穹铁道》3Dmigoto Mod 的 Windows 桌面管理器。LiquiMod 使用 Rust Core、Tauri 2、Svelte 5 和 Tailwind CSS 构建，提供 Mod 导入、分类、启停、预设、变体选择、冲突诊断和部署自愈。

[![Release](https://img.shields.io/badge/release-v0.3.1-blue.svg)](https://github.com/pianhua/liquimod/releases/tag/v0.3.1)
[![CI](https://github.com/pianhua/liquimod/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pianhua/liquimod/actions/workflows/ci.yml)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-5-FF3E00.svg)](https://svelte.dev/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## 当前版本

`v0.3.1` 是面向 Windows 10/11 x64 的稳定修订版本，提供便携式 ZIP 包。发布包包含主程序、F10 刷新助手、README 和许可证，不包含游戏本体或 3Dmigoto。

- [下载 v0.3.1](https://github.com/pianhua/liquimod/releases/tag/v0.3.1)
- [查看 CI](https://github.com/pianhua/liquimod/actions)
- [查看 v0.3.1 完整变更](https://github.com/pianhua/liquimod/compare/v0.3.0...v0.3.1)

## 功能

### Mod 管理

- 导入文件夹以及 `.zip`、`.7z`、`.rar` Mod 压缩包
- 自动识别角色，支持手动归属和未分类 Mod 管理
- 启用、禁用、重命名、移动、卸载和批量应用预设
- 自定义封面、立绘预览、搜索、排序、收藏和拖拽排序
- 密码本支持加密压缩包的历史密码尝试

### 部署与兼容性

- NTFS 卷优先使用 Junction，启停时不复制 Mod 内容
- exFAT、移动盘或跨卷场景自动降级为 CopyFallback
- CopyFallback 使用 LiquiMod 标记保护运行目录，只清理由自身创建的部署
- 目录监控自动发现库内变化，并在游戏退出后完成物理部署对账
- SQLite `op_log` 写前事务日志支持崩溃恢复和启动自愈

### 变体与冲突诊断

- 识别 `Option`、编号和 `[Variant]` 等明确命名的变体目录
- 将基础资源与选中变体合并到运行副本，变体文件覆盖同路径基础文件
- 对 3DMigoto 的 Mod hash、Constants 和变量名冲突提供诊断
- 运行副本中的全局变量按 Mod ID 隔离，减少跨 Mod 命名冲突

### 游戏集成与环境诊断

- 进程看门狗显示游戏运行状态
- 游戏运行期间阻止启停、卸载、重命名、移动、预设、变体和部署修复等危险操作
- 安装时若开启自动启用，游戏运行期间只完成入库，自动启用会被延后
- 独立的 `liquimod-refresh-helper.exe` 负责发送 F10 刷新信号
- 设置页检查 WebView2、VC++、目录权限、游戏/加载器配置和部署模式
- 提供 WebView2 下载入口、Defender 排除命令和部署修复入口

## 安装与首次使用

1. 从 [v0.3.1 Release](https://github.com/pianhua/liquimod/releases/tag/v0.3.1) 下载 `LiquiMod-Windows-x64.zip`。
2. 将 ZIP 解压到可写目录，保持 `liquimod-app.exe` 与 `liquimod-refresh-helper.exe` 位于同一目录。
3. 启动 `liquimod-app.exe`。
4. 在设置中配置 Mod 库目录、3Dmigoto 的 `Mods` 目录、游戏可执行文件和加载器路径。
5. 导入 Mod，确认角色归属后再启用。

3Dmigoto 和游戏文件需要用户自行准备。首次使用前建议先在设置页完成环境诊断；涉及 Junction、CopyFallback 或 F10 刷新的操作可能触发管理员权限提示。

## 安全边界

LiquiMod 的部署操作以保护用户文件为优先：

- ZIP、远端资产和图片路径经过相对路径净化与 containment 校验，拒绝绝对路径、盘符、UNC 和 `..` 越界路径。
- 遇到非空实体目录、未知文件或非 LiquiMod 部署目标时，部署器会报错并停止，不强制删除。
- 游戏运行期间，目录监控和全库扫描只更新索引，不主动重建 Junction 或复制部署目录；库内安装仍可进行，但自动启用会被延后。
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

`v0.3.1` 当前验证结果：Rust 测试 207 个通过，4 个环境相关测试按条件忽略；前端 22 个测试套件、100 个测试通过。RAR 真实压缩包和 Windows 符号链接权限测试需要额外本机 fixture 或系统权限。

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
