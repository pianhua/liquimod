# 仓库维护与磁盘清理

## 目录职责

| 目录 | 作用 | 是否应提交 |
| --- | --- | --- |
| `crates/`、`app/src-tauri/`、`app/src/` | 源代码 | 是 |
| `assets/` | 内置角色资源和 XXMI/SRMI 核心包 | 是，按资源许可执行 |
| `docs/`、根目录说明文件 | 长期文档和交接资料 | 是 |
| `.github/`、`scripts/` | CI、Issue/PR 模板和可复用维护脚本 | 是 |
| `target/` | Rust 构建缓存和二进制 | 否 |
| `app/node_modules/`、`app/build/`、`app/.svelte-kit/` | 前端依赖和生成物 | 否 |
| `dist/` | 本机打包结果，不是源码资产 | 否 |
| `fixtures/`、`workspace_srmi/` | 本机测试工作区或调试材料 | 否，稳定测试样本应另行迁入测试目录 |

## 清理规则

默认清理只删除可再生的 `target/debug`，保留 `target/release` 供本机验收。完整清理可以显式执行：

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\clean.ps1 -DryRun
powershell -ExecutionPolicy Bypass -File .\scripts\clean.ps1
powershell -ExecutionPolicy Bypass -File .\scripts\clean.ps1 -All
```

`-All` 也只处理已知的 Rust 和前端构建目录，不会清理 `dist/`、外部 Mod 源、用户数据仓库、`.cargo` 或 `.rustup`。共享 Rust 工具链缓存需要单独审计，不能因为 LiquiMod 项目清理而误删其他项目依赖。

## 打包规则

`scripts/build_package.ps1` 将每个版本输出到 `dist/releases/v<version>/`，临时文件位于 `dist/.staging/`。它不会清空整个 `dist/`，并且只有带有 LiquiMod 生成标记的同版本包目录才允许被替换。

`dist/` 中不应长期混放真实 Mod 数据、旧测试包和正式发布资产。需要保留的实机测试包应移到仓库之外的归档目录，并记录日期、来源版本和用途。本次清理的历史测试包已归档到：

```text
E:\LiquiMod-Archive\2026-08-22\dist-legacy
```

归档只为减少 C 盘占用，不代表这些包已经通过当前版本验收。归档目录中的 Junction 可能被跨卷移动过程展开为实体快照，不能把它当作当前运行目录使用。

## 安全边界

- 不删除 `Library`、外部 Mod 源或用户迁移前仓库，除非目标、归属和备份状态已经明确。
- 不跟随未知 Junction、符号链接或 UNC 路径执行清理。
- 不使用 `git add .`、`git add -A`，生成物应通过 `.gitignore` 隔离。
- 任何会影响 GitHub 分支、tag、Release 或 Issue 的操作都要在执行前确认具体目标。
