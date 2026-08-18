# LiquiMod

崩铁（3Dmigoto）Mod 管理器。Rust core + Tauri 2 + Svelte 5。

## 构建

```bash
# 前端
cd app && npm install && npm run build
# 主程序（必须带 tauri/custom-protocol，否则 exe 不内嵌前端、导航到 devUrl）
cargo build --release --features tauri/custom-protocol --manifest-path app/src-tauri/Cargo.toml
# F10 刷新 helper（必须与 liquimod-app.exe 同目录，运行时 current_exe().parent() 定位）
cargo build --release -p liquimod-refresh-helper
```

产物：`target/release/liquimod-app.exe` + `target/release/liquimod-refresh-helper.exe`。
dev 模式（`cargo run`）下 helper 不存在于 target/debug，自动刷新会 toast 提示并跳过——正常。

## 测试 / 检查

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all -- --check
cd app && npm test && npm run check
```

## 调试 WebView2

设 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223` 启动 exe，然后 CDP 连 `http://localhost:9223/json`。WebView2 缓存位于 `%LOCALAPPDATA%\com.liquimod.app\EBWebView`，前端异常时先删。

已知引擎差异：WebView2 的 grid auto 行高计算无视 grid item 的 aspect-ratio/padding-top 撑高——UI 勿依赖内容撑高定 grid 行高（CharacterGrid 用 ResizeObserver 显式写 grid-auto-rows）。
