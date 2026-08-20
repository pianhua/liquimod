# LiquiMod

面向《崩坏：星穹铁道》（3Dmigoto）的现代化、极速、无感 Mod 管理器。基于 Rust Core 2021 + Tauri 2.x + Svelte 5 (Runes) + Tailwind CSS v4 构建。

[![Version](https://img.shields.io/badge/version-v0.2.9-blue.svg)](https://github.com/pianhua/liquimod/releases)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5.0-FF3E00.svg)](https://svelte.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 核心特性

- **高效部署**：基于 NTFS 目录联接（Junction）实现 Mod 秒级启用与禁用，零额外磁盘空间占用；
- **工业级数据安全**：全量 SafePath 路径沙箱防护（CWE-22），预写事务日志（`op_log`）支持崩溃自愈与状态恢复；
- **内存安全模型**：核心数据基于 `Arc<[CharacterInfo]>` 线程安全只读快照，杜绝 Use-After-Free；
- **游戏集成与无感热重载**：智能嗅探游戏主程序与启动器路径；通过独立的 `liquimod-refresh-helper` 实现游戏内无感 F10 热重载；
- **原生物理重排拖拽**：基于原生 Pointer Events 的高精度卡片物理重排引擎，带流畅跟随与平滑位移插槽；
- **极光流体氛围美学**：Apple Liquid Glass 风格，支持立绘双层极光环境光漫反射（Ambient Frosted Glow）与暗房 Lightbox；
- **全格式解压与智能密码本**：支持 `.zip` / `.7z` / `.rar` / `.tar` 解压与解压炸弹防护，内置历史密码记忆。

---

## 项目维护与交接文档

- **[HANDOVER.md](HANDOVER.md)**：项目交接与工程维护全景手册（系统机制、数据库 Schema、安全修复记录与 Roadmap）
- **[AGENTS.md](AGENTS.md)**：架构与开发守则（代码铁律、状态机规范、质量门禁）
- **[STYLE.md](STYLE.md)**：Apple Liquid Glass 界面设计规范与组件约定

---

## 系统要求

- Windows 10 / 11 (x64)
- Microsoft Edge WebView2 运行时（Windows 11 已内置）

---

## 本地编译与构建

### 1. 环境准备

- [Rust 1.80+](https://rustup.rs/)
- [Node.js 18+](https://nodejs.org/) & `npm`

### 2. 编译步骤

```bash
# 1. 构建前端静态资源
cd app && npm install && npm run build && cd ..

# 2. 编译 F10 刷新助手
cargo build --release -p liquimod-refresh-helper

# 3. 编译主程序 (内嵌前端)
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
```

产物位于 `target/release/liquimod-app.exe` 与 `target/release/liquimod-refresh-helper.exe`。

### 3. 一键打包发布 (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
```

执行后将在 `dist/` 目录生成便携版 ZIP 压缩包与 SHA256 校验和。

---

## 测试与质量门禁

```bash
# Rust 代码检查与全量单测 (157 个用例)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# 前端类型检查与 Vitest 单元测试 (22 套件 100 用例)
cd app && npm run check && npm test
```

---

## 开源许可证

本项目采用 [MIT 许可证](LICENSE)。

