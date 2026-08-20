# LiquiMod · 星轨流光

<div align="center">

**崩坏：星穹铁道（3Dmigoto）现代化高性能 Mod 管理器**

[![Version](https://img.shields.io/badge/version-v0.2.0-blue.svg)](https://github.com/pianhua/liquimod/releases)
[![Rust](https://img.shields.io/badge/Rust-1.80%2B-orange.svg)](https://www.rust-lang.org/)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-24C8D8.svg)](https://v2.tauri.app/)
[![Svelte 5](https://img.shields.io/badge/Svelte-5.0-FF3E00.svg)](https://svelte.dev/)
[![CI Quality Gate](https://github.com/pianhua/liquimod/actions/workflows/ci.yml/badge.svg)](https://github.com/pianhua/liquimod/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

*极致流体美学 · 工业级数据韧性 · F10 内存无缝热重载 · 0 配置秒启动*

</div>

---

## ✨ 核心特性

- 🚀 **极速原生架构**：底层采用 Rust 2024 高性能引擎，结合 SQLite 持久化元数据与原生 NTFS 目录结区（Junction）秒级部署，启用/禁用零等待、零卡顿。
- 🎨 **现代化液态毛玻璃界面**：基于 Tauri 2 + Svelte 5 构建，支持明亮（Light）与暗黑（Dark）双套主题无缝自适应切换。
- 🖼️ **影院级全景图集与大图 Lightbox**：
  - 自动递归扫描 6 层子目录图片，支持 `.png`, `.jpg`, `.jpeg`, `.webp`, `.bmp`, `.gif`, `.avif` 等所有主流格式；
  - 纯暗房影院查看器：支持鼠标滚轮 `50% ~ 500%` 无级缩放、双击自适应、平移拖拽、键盘翻页与底部微缩图导航；
  - **无损封面记忆机制**：绝不覆盖或破坏 Mod 磁盘中的原始文件，支持一键设为封面与一键恢复默认。
- 🎮 **INI 动态热键智能解析**：精准提取 Mod 配置文件中的按键绑定（如 `$swaphead`、分档变量、说明注释与反向切换快捷键）。
- ⚡ **游戏内无感刷新 Helper**：搭载独立的 `liquimod-refresh-helper`，通过虚拟 F10 按键模拟触发 3Dmigoto 热重载，告别繁琐切屏。
- 📂 **智能分类与预设管理**：支持角色库、光锥、立绘、场景、NPC 与自定义分类管理，支持一键创建/应用多套外观方案预设。
- 🛡️ **冲突检测与自愈引擎**：实时监测 Shader 命名冲突与文件散落，自动清理孤儿 Junction 与缩略图缓存。

---

## 🛠️ 技术栈

| 模块 | 技术选型 | 说明 |
| :--- | :--- | :--- |
| **Core 核心引擎** | Rust 2024, SQLite (rusqlite), `image`, `notify` | 数据库管理、目录监听、热键解析、压缩包解压 |
| **Desktop 桌面端** | Tauri 2.0, Rust | 原生系统级交互、拖拽解包、窗口管理 |
| **UI 视图层** | Svelte 5 (Runes 响应式), Tailwind CSS, Vite | 现代化流光毛玻璃组件、全局层级 Esc 状态机 |
| **刷新助手** | Windows API, `liquimod-refresh-helper` | 游戏窗口定位与虚拟按键分发 |

---

## 🚀 快速开始

### 环境依赖

- [Rust 1.80+](https://rustup.rs/)
- [Node.js 18+](https://nodejs.org/) & `npm`
- Windows 10/11 (WebView2 运行时)

### 编译与构建

```bash
# 1. 安装前端依赖并构建前端静态资源
cd app
npm install
npm run build
cd ..

# 2. 编译 Tauri 主程序（内嵌前端资源）
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml

# 3. 编译游戏内热重载 Helper
cargo build --release -p liquimod-refresh-helper
```

> **产物位置**：`target/release/liquimod-app.exe` 和 `target/release/liquimod-refresh-helper.exe`。
> 请保持两个可执行文件放置在同一目录下运行。

### 自动化测试

```bash
# 运行 Rust 全工作区测试
cargo test --workspace

# 运行前端单元测试与类型检查
cd app
npm test
npm run check
```

---

## 📄 开源许可证

本项目基于 [MIT 许可证](LICENSE) 开源。
