# LiquiMod

《崩坏：星穹铁道》3Dmigoto Mod 的 Windows 桌面管理器。基于 Rust、Tauri 2 和 Svelte 5 构建，支持 Mod 导入、分类、启停、变体选择、预设和一键启动注入。

当前仓库基线为 `v0.1.0-beta.1`。这是一个重新整理后的预发布起点，不延续旧仓库的提交、Issue、PR 或发布记录。

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## 下载

正式构建发布后，可从 GitHub Releases 获取：

- `LiquiMod-Windows-x64-setup.exe`：NSIS 安装包
- `LiquiMod-Windows-x64.zip`：便携版，解压即用

## 功能

- 导入 `.zip`、`.7z`、`.rar` 压缩包或文件夹形式的 Mod
- 连接外部 Mod 目录，不复制、不接管源文件
- 按角色分类、搜索、排序、收藏和自定义排序
- Mod 变体选择、多 Mod 风险提示
- 预设管理：一键切换多角色 Mod 组合
- 压缩包密码本
- 使用同一 NTFS/ReFS 卷上的 Junction 部署 Mod
- 一键启动游戏并注入 3Dmigoto/XXMI
- F10 热重载

跨卷或非 NTFS/ReFS 文件系统的 CopyFallback 复制部署目前暂不支持。检测到此类路径时，请将 LiquiMod 数据根与 3Dmigoto `Mods` 目录放在同一 NTFS/ReFS 卷后重试。

## 系统要求

- Windows 10/11 x64
- Microsoft Edge WebView2 Evergreen Runtime
- 已正确配置的游戏与 3Dmigoto/XXMI 环境

## 从源码构建

需要：

- Rust stable
- Node.js 22 + npm

```powershell
cd app
npm ci
npm run build
cd ..

cargo build --release -p liquimod-refresh-helper
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
```

构建产物：

- `target/release/XXMI Launcher.exe`
- `target/release/liquimod-refresh-helper.exe`

或者使用打包脚本：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
```

## 项目结构

```text
liquimod/
├── crates/liquimod-core/           Rust 核心库
├── crates/liquimod-cli/            CLI 调试工具
├── crates/liquimod-refresh-helper/ 提升权限的注入/F10 助手
├── app/src-tauri/                  Tauri 后端与 IPC
├── app/src/                        Svelte 前端
├── assets/                         内置角色数据与默认资源
├── scripts/                        本地构建与打包脚本
└── .github/workflows/              CI 与 Release 流水线
```

## 免责声明

LiquiMod 是社区独立工具，与 HoYoverse、miHoYo、3DMigoto 或 XXMI 项目没有隶属关系。请遵守游戏服务条款、Mod 作者许可和当地法律法规，并自行备份重要数据。

## 归档安全

RAR 支持继续使用 `unrar`，并对压缩包执行路径净化、越界检查和资源限制。修改归档引擎时，必须同时验证加密归档、密码错误、绝对路径、盘符、UNC、`..` 遍历、链接逃逸和安装事务回滚。

## 许可证

[MIT License](LICENSE)
