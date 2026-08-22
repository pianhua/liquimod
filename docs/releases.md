# LiquiMod 版本历史

本表是 Git tag、GitHub Release 和仓库变更说明的导航，不替代每个版本的完整源码。历史版本保留用于回滚和问题对比；未经核实的旧版本细节不在这里补写。

| 版本 | 状态 | 说明 | 记录 |
| --- | --- | --- | --- |
| `v0.1.0-beta` | 历史 tag | 早期测试基线，未建立正式 GitHub Release | [tag](https://github.com/pianhua/liquimod/releases/tag/v0.1.0-beta) |
| `v0.3.0` | 历史 Release | 早期公开发行版本 | [Release](https://github.com/pianhua/liquimod/releases/tag/v0.3.0) |
| `v0.3.1` | 历史 Release | 早期维护版本 | [Release](https://github.com/pianhua/liquimod/releases/tag/v0.3.1) |
| `v0.4.0` | 历史基线 | 从此版本重新对齐 XXMI/SRMI 生态的开发起点 | [Release](https://github.com/pianhua/liquimod/releases/tag/v0.4.0) |
| `v0.5.0` | 历史 Release | 重新开发阶段的中间版本 | [Release](https://github.com/pianhua/liquimod/releases/tag/v0.5.0) |
| `v0.5.1` | 历史 Release | 注入、3DMigoto 核心和便携化问题的关键修复版本 | [Release](https://github.com/pianhua/liquimod/releases/tag/v0.5.1) |
| `v0.6.0` | 历史 Release | v0.6 系列稳定化前的版本 | [变更说明](../RELEASE_NOTES_v0.6.0.md) / [Release](https://github.com/pianhua/liquimod/releases/tag/v0.6.0) |
| `v0.6.1` | 当前稳定基线 | F10 热重载修复，用户已完成实机验收；外部 Mod 批量扫描和 Hash/变量诊断仍按决策暂缓 | [变更说明](../RELEASE_NOTES_v0.6.1.md) / [Release](https://github.com/pianhua/liquimod/releases/tag/v0.6.1) |

## 版本管理约定

- 历史 tag 和正式 Release 默认不删除，以保留回滚、二分和实机问题对照能力。
- 新版本必须在合并到 `main` 后创建 tag，并等待 Release workflow 完成后再宣布发布完成。
- 版本说明至少包含：用户可见变化、风险或已知边界、自动化验证、未覆盖的 Windows 实机验证和回滚参考。
- 暂缓功能必须同时出现在 `DECISIONS.md`、`HANDOVER.md` 和对应 GitHub Issue 中。
