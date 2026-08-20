# LiquiMod

面向《崩坏：星穹铁道》（3Dmigoto）的轻量级、高性能 Mod 管理器。基于 Rust 与 Tauri 2 / Svelte 5 构建。

[![Version](https://img.shields.io/badge/version-v0.2.0-blue.svg)](https://github.com/pianhua/liquimod/releases)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5.0-FF3E00.svg)](https://svelte.dev/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

---

## 功能特性

- **高效部署**：基于 NTFS 目录联接（Junction）实现 Mod 秒级启用与禁用，不占用额外磁盘空间。
- **数据安全与自愈**：写入操作前记录事务日志（`op_log`），支持崩溃自愈与状态一致性恢复；Junction 仅操作符号链接，不删除实体文件。
- **游戏集成与热重载**：智能嗅探游戏主程序与启动器路径；通过独立的 `liquimod-refresh-helper` 实现游戏内无感热重载（F10）。
- **资产预览与按键解析**：自动递归扫描 Mod 多层目录图片并支持大图查看；智能解析 INI 配置文件中的按键绑定与注释。
- **分类与预设管理**：内置角色、光锥、立绘、场景及自定义分类；支持外观方案预设的一键保存与应用。
- **双主题适配**：适配明亮（Light）与暗黑（Dark）主题。

---

## 系统要求

- Windows 10 / 11 (x64)
- Microsoft Edge WebView2 运行时（Windows 11 已内置）

---

## 本地编译

### 1. 环境准备

- [Rust 1.80+](https://rustup.rs/)
- [Node.js 18+](https://nodejs.org/) & `npm`

### 2. 编译步骤

```bash
# 1. 构建前端静态资源
cd app
npm install
npm run build
cd ..

# 2. 编译 F10 刷新助手
cargo build --release -p liquimod-refresh-helper

# 3. 编译主程序
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
```

编译产物位于 `target/release/liquimod-app.exe` 与 `target/release/liquimod-refresh-helper.exe`，运行时需保持两者处于同一目录。

### 3. 一键打包脚本 (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build_package.ps1
```

执行后将在 `dist/` 目录生成便携版 ZIP 压缩包与 SHA256 校验和。

---

## 测试与质量检查

```bash
# Rust 测试
cargo test --workspace

# 前端类型检查与单元测试
cd app
npm run check
npm test
```

---

## 开源许可证

本项目采用 [MIT 许可证](LICENSE)。
