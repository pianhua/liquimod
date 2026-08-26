# LiquiMod

面向 Windows 的高性能 3Dmigoto / XXMI Mod 管理器。基于 Rust、Tauri 2 与 Svelte 5 构建，专注于解决传统 3Dmigoto 工作流中的内存膨胀、目录污染、繁琐配置与视觉陈旧问题。

[![Latest Release](https://img.shields.io/github/v/release/pianhua/liquimod?include_prereleases&label=Release&color=2563eb)](https://github.com/pianhua/liquimod/releases)
[![CI Status](https://img.shields.io/github/actions/workflow/status/pianhua/liquimod/ci.yml?branch=main&label=CI)](https://github.com/pianhua/liquimod/actions/workflows/ci.yml)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20x64-0284c7)](https://github.com/pianhua/liquimod/releases)
[![Rust](https://img.shields.io/badge/Rust-2021%20Edition-dea584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![Tauri](https://img.shields.io/badge/Tauri-v2-24c8db?logo=tauri&logoColor=white)](https://tauri.app/)
[![Svelte](https://img.shields.io/badge/Svelte-v5-ff3e00?logo=svelte&logoColor=white)](https://svelte.dev/)
[![Tailwind CSS](https://img.shields.io/badge/Tailwind-v4-38bdf8?logo=tailwindcss&logoColor=white)](https://tailwindcss.com/)
[![License](https://img.shields.io/badge/License-MIT-emerald.svg)](LICENSE)

---

## 核心优势

- **Rust 原生性能与超低内存驻留**
  全栈底层由 Rust + SQLite 构建，严格管理内存生命周期与异步任务，从根源杜绝长时间后台运行的内存泄漏与卡顿，毫秒级完成海量 Mod 库的索引与对账。
- **Junction 目录解耦与零拷贝部署**
  基于 NTFS / ReFS 目录联接（Junction）实现实体 Mod 的极速即时挂载。Mod 资产不必全部堆积在 3Dmigoto 的 `Mods` 文件夹内；支持将资产存放在独立工作区或直接关联外部已有目录，零磁盘冗余占用，绝不修改、移动或污染原始文件。
- **内嵌 XXMI 运行态与一键启动更新**
  内置 XXMI 注入生命周期管理与权限分离助手，集成 3Dmigoto 核心运行态一键同步与在线更新机制。无需手动下载、解压覆盖或反复配置 `d3dx.ini`，开箱即用并支持顶栏无感 F10 热重载。
- **现代化 visionOS 液态玻璃界面**
  深度融合次表面散射、微晶折射与自研 Duotone Liquid Lens 矢量图标体系，配备 3D 悬浮立绘与流体动效，提供更具质感的桌面视觉与交互体验。

---

## 环境要求

- **操作系统**：Windows 10 / 11 x64
- **系统组件**：Microsoft Edge WebView2 Evergreen Runtime
- **文件系统**：LiquiMod 数据目录需与 3Dmigoto `Mods` 目录位于同一 NTFS 或 ReFS 驱动器卷（用于支持 Junction 目录联接）

---

## 致谢与参考 (Acknowledgements)

LiquiMod 在核心架构设计与工作流实现中，借鉴了以下社区优秀开源项目的思路与实践：

- [**XXMI-Launcher**](https://github.com/SpectrumQT/XXMI-Launcher) *(GPL-3.0)* — 在 XXMI 注入生命周期、进程协同与启动器调度架构上提供了重要参考。
- [**JASM**](https://github.com/Jorixon/JASM) *(GPL-3.0)* — 在 3Dmigoto Mod 目录管理模式与索引对账逻辑上提供了参考思路。
- [**SSMT4-Alpha**](https://github.com/StarBobis/SSMT4-Alpha) *(GPL-3.0)* — 在 Mod 变体结构解析与游戏适配器设计上提供了参考。

---

## 免责声明 (Disclaimer)

1. **工具定位**：LiquiMod 仅为面向本地文件系统的**纯本地 Mod 资产管理器与目录调度工具**。
2. **非协助制作**：本项目本身**不包含、不分发、不生成、亦不协助制作**任何违反游戏开发商 / 运营商用户协议、服务条款或版权保护机制的游戏资产与 Mod 内容。
3. **无官方隶属**：本项目与 HoYoverse、miHoYo、3Dmigoto 官方或 XXMI 团队无任何商业合作或官方隶属关系。使用者在使用第三方 Mod 或相关工具时，应自行承担相应风险并遵守相关法律法规及游戏许可协议。

---

## 开源许可证

本项目基于 [MIT License](LICENSE) 协议开源。
