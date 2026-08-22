# 贡献与开发规范

LiquiMod 是 Windows 10/11 x64 上的 Rust workspace + Tauri 2 + Svelte 5 项目。涉及 Mod 文件、3Dmigoto、游戏进程、权限、数据迁移、发布或用户体验的改动，必须先调查现状、说明风险并完成对齐。

## 开发流程

```powershell
git fetch origin
git switch main
git pull --ff-only origin main
git switch -c codex/<topic>
```

修改前检查现有差异；提交时只暂存确认属于当前任务的路径，不使用 `git add .`、`git add -A` 或 force push。

## 提交前检查

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

Push-Location app
npm ci
npm run check
npm test
npm run build
Pop-Location
```

涉及 Tauri、发布脚本、资源或版本号时，还要执行生产构建并检查 `dist/releases/v<version>/` 中的便携包、安装器和 SHA256 文件。

## Pull Request 要求

PR 描述必须说明：问题、方案、影响范围、风险、测试命令和结果、未覆盖的 Windows 实机验证、回滚方式。涉及 UI 时需要检查 Light/Dark、拖拽、滚动、Toast、弹窗和 WebView2 层叠；涉及游戏运行态时不能把“游戏启动”当作“注入成功”。

只有 CI 通过并完成必要 Review 后才能合并到 `main`。正式发布必须以合并后的 `main` 创建 tag，并以 Release workflow 和实际资产为准。
