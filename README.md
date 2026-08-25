# LiquiMod

> 面向 Windows 的 3Dmigoto Mod 管理器，让 Mod 的导入、整理、部署和切换更简单。

[![CI Quality Gate](https://github.com/pianhua/liquimod/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/pianhua/liquimod/actions/workflows/ci.yml)
[![Latest beta](https://img.shields.io/github/v/release/pianhua/liquimod?include_prereleases&label=latest%20beta)](https://github.com/pianhua/liquimod/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

LiquiMod 是一个使用 Rust、Tauri 2 和 Svelte 5 构建的 Windows 桌面应用，专注于管理《崩坏：星穹铁道》的 3Dmigoto/XXMI Mod。

当前版本：**`v0.1.0-beta.1`**。这是重新整理后的第一个公开 Beta，适合在做好数据备份后进行体验和反馈。

## 下载

从 [GitHub Releases](https://github.com/pianhua/liquimod/releases) 下载最新版本：

| 文件 | 说明 |
| --- | --- |
| `LiquiMod-Windows-x64-setup.exe` | Windows 安装包，适合常规安装 |
| `LiquiMod-Windows-x64.zip` | 便携版，解压后即可运行 |

每个构建包都附带对应的 SHA-256 校验文件。首次使用建议先备份自己的 Mod 和配置数据。

## 功能

- 导入 `.zip`、`.7z`、`.rar` 压缩包或文件夹形式的 Mod
- 连接外部 Mod 目录，不复制、移动、接管或删除源文件
- 按角色分类，支持搜索、排序、收藏和自定义顺序
- 选择 Mod 变体，并提示多个 Mod 同时启用的风险
- 使用预设快速切换多角色 Mod 组合
- 管理压缩包密码
- 一键启动游戏并注入 3Dmigoto/XXMI
- 通过 F10 手动热重载 Mod

## 当前部署限制

`v0.1.0-beta.1` 目前只支持以下部署条件：

- LiquiMod 数据根目录和 3Dmigoto 的 `Mods` 目录位于同一卷
- 该卷使用 NTFS 或 ReFS 文件系统
- 部署方式为 Junction

跨卷、非 NTFS/ReFS 文件系统下的 `CopyFallback` 复制部署目前**暂不支持**。如果看到相关提示，请将 LiquiMod 数据根目录迁移到与 3Dmigoto `Mods` 目录相同的 NTFS/ReFS 卷后重试。

此外，崩溃恢复和部署修复路径已经包含在程序中，但本 Beta 尚未完成真实故障注入验证。遇到异常时，请保留日志和现场目录，不要直接删除运行期文件。

## 快速开始

1. 安装或解压 LiquiMod。
2. 在设置中选择游戏、3Dmigoto/XXMI 和数据根目录。
3. 导入 Mod，或连接已有的外部 Mod 目录。
4. 按角色整理 Mod，选择需要的变体并启用。
5. 启动游戏；需要刷新时使用顶部的 F10 热重载按钮。

## 系统要求

- Windows 10/11 x64
- Microsoft Edge WebView2 Evergreen Runtime
- 已正确配置的游戏与 3Dmigoto/XXMI 环境
- LiquiMod 数据根目录与 3Dmigoto `Mods` 目录位于同一 NTFS/ReFS 卷

## 从源码构建

需要 Rust stable、Node.js 22 和 npm：

```powershell
git clone https://github.com/pianhua/liquimod.git
cd liquimod

Push-Location app
npm ci
npm run check
npm test
npm run build
Pop-Location

cargo build --release -p liquimod-refresh-helper
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
```

也可以使用打包脚本生成安装包和便携包：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
```

## 项目结构

```text
liquimod/
├── crates/liquimod-core/           Rust 核心库：数据库、扫描、部署和诊断
├── crates/liquimod-cli/            CLI 调试工具
├── crates/liquimod-refresh-helper/ Windows 权限分离助手
├── app/src-tauri/                  Tauri 后端、生命周期和 IPC
├── app/src/                        Svelte 前端、页面和组件
├── assets/                         内置角色数据与默认资源
├── scripts/                        本地构建与打包脚本
└── .github/workflows/              CI 与 Release 流水线
```

## 反馈与贡献

请通过 [Issues](https://github.com/pianhua/liquimod/issues) 报告可复现的问题或提出功能建议。提交日志、截图或配置前，请先删除个人路径、账号信息和不应公开的 Mod 内容。

## 免责声明

LiquiMod 是社区独立工具，与 HoYoverse、miHoYo、3DMigoto 或 XXMI 项目没有隶属关系。请遵守游戏服务条款、Mod 作者许可和当地法律法规，并自行备份重要数据。

## 许可证

[MIT License](LICENSE)
