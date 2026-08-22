# LiquiMod v0.6.0

## 重点

- 对齐标准 XXMI/SRMI 核心布局，内置可更新的 `Packages/SRMI` 与 `Packages/XXMI`。
- 使用 `XXMI Launcher.exe` 作为正式主程序名称，保留管理员权限以确保 `StarRail.exe` 注入成功。
- 模组启动改为 XXMI 原生 Hook 流程，不依赖 `3Dmigoto Loader.exe` 命令行窗口。
- 数据、数据库、日志、托管 3DMigoto 和 Mod 仓库默认位于程序目录旁，摆脱 C 盘用户配置目录。
- 支持外部 Mod 源目录、Junction 部署和便携式数据迁移。
- 主界面新增 Mod 压缩包导入入口，支持 `.zip`、`.7z`、`.rar` 多选安装。
- 记录注入、权限、3DMigoto `Mods` Junction 与迁移清理的踩坑结论。

## 已知限制

Windows 普通权限资源管理器无法直接把文件拖入管理员窗口；请使用主界面顶部“导入”按钮。真正恢复跨权限拖放需要后续拆分普通 UI 与管理员 Hook Helper。

## 验证

- Svelte 检查：0 errors / 0 warnings
- 前端测试：109 passed
- Rust 工作区测试：应用 45 passed、核心 162 passed、刷新助手 5 passed
- NSIS 正式打包：通过
