# LiquiMod 里程碑 2：解压管线 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 支持 zip/7z/rar 解压（含密码），自动尝试密码本，递归嵌套压缩包，内容根解析，产出 `install_archive` 编排函数与 CLI `install` 命令。

**Architecture:** 在 liquimod-core 新增 `archive` 模块（detect/extract/password），复用里程碑 1 的 db（加 passwords 表）、library（`Library::add_folder`）、error（新增归档错误变体）。临时解压到 `{app_data}/tmp/<uuid>`，成功后经 `add_folder` 入库。

**Tech Stack:** Rust 2021, sevenz-rust2 (features: aes256, util), zip (features: aes-crypto, deflate), unrar, uuid, tempfile(仅测试), rusqlite bundled。

**验收命令（每任务必须绿）：**
```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

**已知约束（勿再核实）：**
- 机器无 WinRAR/7z.exe —— rar 解压测试无法生成真实夹具：rar 测试用合成 magic bytes 做格式检测测试；解压路径测试标 `#[ignore = "needs real rar fixture"]`。
- 无 symlink 特权 —— 沿用里程碑 1 模式，相关测试标 ignore。
- crate 版本：用 `cargo add` 取最新稳定版；下方 API 来自 context7 官方文档（2026-08），实现时若签名不符，对照 `~/.cargo/registry/src` 下源码核验并在汇报中注明偏差。

---

## 归档格式魔法字节（已确认）

| 格式 | magic bytes |
|---|---|
| zip | `50 4B 03 04` / `50 4B 05 06`(空) / `50 4B 07 08`(spanned) |
| 7z  | `37 7A BC AF 27 1C` |
| rar | `52 61 72 21 1A 07 00` (rar4) / `52 61 72 21 1A 07 01 00` (rar5) |

---

## Task 1: passwords 表 + PasswordBook

**Files:**
- Modify: `crates/liquimod-core/src/db.rs`
- Create: `crates/liquimod-core/src/archive/mod.rs`
- Test: `crates/liquimod-core/src/db.rs`（同文件 #[cfg(test)] 模块，沿用现有模式）

- [ ] **Step 1: 写失败测试（db.rs 测试模块内）**

```rust
#[test]
fn password_book_add_list_remove() {
    let db = Database::open_in_memory().unwrap();
    db.add_password("pw-a").unwrap();
    db.add_password("pw-b").unwrap();
    db.add_password("pw-a").unwrap(); // 去重，不报错
    assert_eq!(db.list_passwords().unwrap(), vec!["pw-a", "pw-b"]);
    db.remove_password("pw-a").unwrap();
    assert_eq!(db.list_passwords().unwrap(), vec!["pw-b"]);
}
```

- [ ] **Step 2: 运行测试确认 FAIL**：`cargo test -p liquimod-core password_book`（报不存在的方法）

- [ ] **Step 3: 实现**

`db.rs` 的 `SCHEMA`（现有建表 SQL 常量）追加：

```sql
CREATE TABLE IF NOT EXISTS passwords (
    value TEXT PRIMARY KEY,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

新增方法（放在现有 impl 块内，沿用现有参数风格 `&self` / `&mut self`，照抄相邻方法的 rusqlite 调用模式）：

```rust
pub fn add_password(&self, value: &str) -> Result<()>
pub fn remove_password(&self, value: &str) -> Result<()>
pub fn list_passwords(&self) -> Result<Vec<String>>  // ORDER BY created_at, rowid
```

`add_password` 用 `INSERT OR IGNORE`。

- [ ] **Step 4: 新建 `archive/mod.rs`**（本任务只放 PasswordBook 外壳，后续任务填充）：

```rust
pub mod detect;      // Task 2 创建，本任务先不建；mod.rs 暂只含本任务内容
```

本任务的 `archive/mod.rs` 内容：

```rust
use crate::db::Database;
use crate::error::Result;

pub struct PasswordBook<'a> {
    db: &'a Database,
}

impl<'a> PasswordBook<'a> {
    pub fn new(db: &'a Database) -> Self { Self { db } }
    pub fn candidates(&self) -> Result<Vec<String>> { self.db.list_passwords() }
    pub fn learn(&self, password: &str) -> Result<()> { self.db.add_password(password) }
}
```

在 `lib.rs` 加 `pub mod archive;`（若 lib.rs 用 `mod`+`pub use` 风格则照抄现有行式）。

- [ ] **Step 5: 测试绿 + clippy 干净 + commit**

```
feat(core): passwords 表与 PasswordBook
```

---

## Task 2: 格式检测 + 归档错误变体

**Files:**
- Modify: `crates/liquimod-core/src/error.rs`
- Create: `crates/liquimod-core/src/archive/detect.rs`
- Modify: `crates/liquimod-core/src/archive/mod.rs`（加 `pub mod detect;`）

- [ ] **Step 1: 失败测试**（detect.rs 内 #[cfg(test)]，用 tempfile + 手写 magic bytes 生成假文件）：

```rust
#[test]
fn detects_zip_by_magic()          // 写 PK\x03\x04 头 → ArchiveFormat::Zip
#[test]
fn detects_7z_by_magic()           // 7z magic → SevenZ
#[test]
fn detects_rar4_and_rar5()         // 两组 rar magic → Rar
#[test]
fn unknown_bytes_are_unsupported() // 随机字节 → Error::UnsupportedArchive
#[test]
fn empty_file_is_unsupported()
```

- [ ] **Step 2: 确认 FAIL** `cargo test -p liquimod-core detect`

- [ ] **Step 3: error.rs 新增变体**（照抄现有 thiserror 风格）：

```rust
#[error("unsupported archive format: {0}")]
UnsupportedArchive(std::path::PathBuf),
#[error("archive requires a password: {0}")]
PasswordRequired(std::path::PathBuf),
#[error("wrong password for archive: {0}")]
WrongPassword(std::path::PathBuf),
#[error("archive error in {path}: {source}")]
Archive {
    path: std::path::PathBuf,
    #[source] source: Box<dyn std::error::Error + Send + Sync>,
},
```

- [ ] **Step 4: detect.rs 实现**

```rust
pub enum ArchiveFormat { Zip, SevenZ, Rar }

pub fn detect_format(path: &Path) -> Result<ArchiveFormat> {
    // 读前 8 字节（不足按实际长度比对），按上表 magic 匹配
    // 不匹配 → Err(Error::UnsupportedArchive(path.to_path_buf()))
}
```

注意：`detect_format` 只认 magic bytes，不信任扩展名（测试里给一个 `.zip` 扩展名但 7z 内容的文件 → SevenZ）。

- [ ] **Step 5: 测试绿 + clippy + commit** `feat(core): 归档格式检测与错误变体`

---

## Task 3: zip 解压（含密码）

**Files:**
- Create: `crates/liquimod-core/src/archive/zip.rs`
- Modify: `crates/liquimod-core/Cargo.toml`, `archive/mod.rs`（`pub mod zip_extract;`——zip 与 crate zip 重名，模块命名 `zip_extract`）

**依赖（cargo add）：**
```
cargo add zip --features aes-crypto,deflate -p liquimod-core
```
（若 aes-crypto feature 名在当前版本不存在，查 `~/.cargo/registry/src/*/zip-*/Cargo.toml` 的 [features] 并选用 AES 相关 feature，汇报注明。）

**API 事实（来自 context7 官方文档）：**
- `ZipArchive::new(file)?`
- `archive.by_index_decrypt(i, password_bytes)?` → 错密码报 `zip::result::ZipError::InvalidPassword`
- 条目加密检测：`archive.by_index(i)?.encrypted()`
- 未加密条目正常 `archive.by_index(i)?`
- 路径安全：必须用 `entry.enclosed_name()`，忽略 None（zip-slip 防护）

**测试夹具**：用 zip crate 自身 `ZipWriter` 在测试里现造（`zip::write::SimpleFileOptions`；加密夹具用 `with_aes_encryption(AesMode::Aes256, "pw")` —— 若该 API 不存在则只测 ZipCrypto `with_password` 或跳过加密写入，改测：未加密夹具 + 错误密码场景用"加密检测分支"单测覆盖，并在汇报注明）。

- [ ] **Step 1: 失败测试**

```rust
#[test] fn extracts_plain_zip()                    // 造 2 文件 zip → 解压到临时目录 → 断言文件内容与目录结构
#[test] fn reports_password_required_for_encrypted() // 加密夹具，password=None → Err(PasswordRequired)
#[test] fn wrong_password_maps_to_wrong_password()   // 加密夹具 + 错误密码 → Err(WrongPassword)
#[test] fn correct_password_extracts()               // 若夹具可造；否则 #[ignore]
#[test] fn rejects_zip_slip_entry()                  // 手工构造含 ../ 条目（ZipWriter 写名为 ../evil.txt）→ 该条目被跳过
```

- [ ] **Step 2: 确认 FAIL**

- [ ] **Step 3: 实现** `archive/zip_extract.rs`：

```rust
pub fn extract_zip(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()>
```

逻辑：打开 → 遍历 0..len → 若任一 entry encrypted 且 password=None → 立即 `Err(PasswordRequired)`；encrypted 走 `by_index_decrypt`，`InvalidPassword` → `Err(WrongPassword)`；目录条目建目录；文件条目 `enclosed_name()`（None 则跳过）→ 创建父目录 → `io::copy`。其他 ZipError 包成 `Error::Archive { path, source }`。

- [ ] **Step 4: 测试绿 + clippy + commit** `feat(core): zip 解压（AES/ZipCrypto 密码支持）`

---

## Task 4: 7z 解压（含密码）

**Files:**
- Create: `crates/liquimod-core/src/archive/sevenz.rs`
- Modify: Cargo.toml, archive/mod.rs

**依赖：**
```
cargo add sevenz-rust2 --features aes256,util -p liquimod-core
```
（util 提供 decompress_file 便捷函数；若 feature 名不符查源码 Cargo.toml。）

**API 事实（context7 官方文档，已确认）：**
```rust
use sevenz_rust2::{decompress_file_with_password, decompress_file, Archive, Password};
let pwd = Password::from("secret");
decompress_file_with_password("encrypted.7z", "./out/", pwd)?;
// 错误: sevenz_rust2::Error::PasswordRequired / Error::MaybeBadPassword / Error::BadSignature
```
`decompress_file(src, dest)` 用于无密码。加密探测：`Archive::open_with_password` 用 `Password::empty()` 试开，PasswordRequired 即加密（或先 `Archive::open` 失败判定——以实现时源码为准，优先用库暴露的判定方式）。

- [ ] **Step 1: 失败测试**（夹具：用 sevenz-rust2 自身的压缩函数现造 .7z，若写侧 API 支持密码则造加密夹具；否则加密路径测试 #[ignore] 并注明）

```rust
#[test] fn extracts_plain_7z()
#[test] fn reports_password_required()      // 加密夹具 + None → PasswordRequired
#[test] fn wrong_password_maps()            // MaybeBadPassword → WrongPassword
```

- [ ] **Step 2: 确认 FAIL**

- [ ] **Step 3: 实现** `archive/sevenz.rs`：

```rust
pub fn extract_7z(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()>
```

PasswordRequired → 我们的 `Error::PasswordRequired`；MaybeBadPassword → `Error::WrongPassword`；其他 → `Error::Archive`。

- [ ] **Step 4: 测试绿 + clippy + commit** `feat(core): 7z 解压（密码支持）`

---

## Task 5: rar 解压（含密码）

**Files:**
- Create: `crates/liquimod-core/src/archive/rar.rs`
- Modify: Cargo.toml, archive/mod.rs

**依赖：**
```
cargo add unrar -p liquimod-core
```
（MSVC 链接器已验证可用——rusqlite bundled 编译成功。）

**API 事实（context7 官方文档）：**
```rust
use unrar::{Archive, error::{Code, When}};
Archive::new(path).open_for_processing()?            // 无密码
Archive::with_password(path, pw).open_for_processing()?
// 加密无密码: Code::MissingPassword (When::Open)
// 遍历: while let Some(header) = archive.read_header()? { archive = header.extract_with_base(dest)? /* 或 header.skip()? */ }
```
⚠️ 实现时必须从 `~/.cargo/registry/src/*/unrar-*/src` 核验：`extract_with_base` 的确切方法名/签名、错密码时的 Code（BadData vs 其他），偏差写入汇报。

- [ ] **Step 1: 失败测试**（无 WinRAR，无法造真实 rar：检测已由 Task 2 覆盖；这里测"非 rar 内容喂给 rar 提取器 → Error::Archive 而非 panic"；真实解压测试 #[ignore]）

```rust
#[test] fn garbage_rar_returns_archive_error()
#[test] #[ignore = "needs real rar fixture"] fn extracts_real_rar()
#[test] #[ignore = "needs real rar fixture"] fn wrong_password_maps()
```

- [ ] **Step 2: 确认 FAIL**（garbage 测试）

- [ ] **Step 3: 实现** `archive/rar.rs`：

```rust
pub fn extract_rar(archive_path: &Path, dest: &Path, password: Option<&str>) -> Result<()>
```

MissingPassword → `Error::PasswordRequired`；CRC/BadData 且提供了密码 → `Error::WrongPassword`（无法区分时按源码实际语义选择，汇报注明）；目录条目 skip，文件 extract 到 dest。

- [ ] **Step 4: 测试绿 + clippy + commit** `feat(core): rar 解压（密码支持，真实夹具测试 ignore）`

---

## Task 6: 嵌套递归 + 内容根解析

**Files:**
- Create: `crates/liquimod-core/src/archive/mod.rs` 中新增 `resolve_content_root`、`extract_recursive`
- Modify: archive/mod.rs

**规则：**
- `resolve_content_root(dir)`：若 dir 下只有一个条目且是目录 → 递归下钻（最多 10 层）；否则返回 dir。这是 mod 包常见结构（`FooMod-v1/FooMod/...`）。
- `extract_recursive(archive, dest, password, depth)`：解压 → 扫描产物中的嵌套压缩包（用 detect_format，忽略 UnsupportedArchive）→ 递归解压到 `dest/__nested_<n>`（深度上限 5，超限跳过并收集警告）。

- [ ] **Step 1: 失败测试**

```rust
#[test] fn content_root_unwraps_single_wrapper_dir()   // dest/A/B/file → B 为根
#[test] fn content_root_stops_at_multiple_entries()
#[test] fn nested_zip_inside_zip_is_extracted()        // 外层 zip 含 inner.zip → 产物含 inner 解压结果
#[test] fn depth_limit_stops_recursion()               // 造 6 层嵌套 → 第 6 层不解压，返回警告计数
```

- [ ] **Step 2: 确认 FAIL**

- [ ] **Step 3: 实现**（函数签名）

```rust
pub fn resolve_content_root(dir: &Path) -> Result<PathBuf>
pub struct ExtractReport { pub nested_warnings: Vec<String> }
pub fn extract_recursive(archive_path: &Path, dest: &Path, password: Option<&str>, depth: u32, report: &mut ExtractReport) -> Result<()>
```

分发逻辑：`detect_format` → 调 zip_extract/sevenz/rar 对应函数。

- [ ] **Step 4: 测试绿 + clippy + commit** `feat(core): 嵌套解压与内容根解析`

---

## Task 7: install_archive 编排

**Files:**
- Create: `crates/liquimod-core/src/archive/install.rs`
- Modify: archive/mod.rs, lib.rs（pub use）

**行为（对应设计文档沉浸式安装流）：**

```rust
pub enum InstallOutcome {
    Installed { mod_id: i64, name: String, warnings: Vec<String> },
    NeedsPassword,                    // 密码本全部失败或未提供，由调用方（UI/CLI）收集密码后带 password 重试
}

pub fn install_archive(
    db: &Database,
    library: &Library,
    archive_path: &Path,
    explicit_password: Option<&str>,
) -> Result<InstallOutcome>
```

流程：
1. 临时目录 `{app_data}/tmp/liquimod-<uuid>`（用 paths.rs 现有 app_data 定位；无则参照其模式）。
2. 候选密码序列：`explicit_password` 优先，然后密码本 `PasswordBook::candidates()`；第一轮先试 `None`（无密码）。
3. 对每个候选调 `extract_recursive`：成功 → break；`WrongPassword` → 试下一个；`PasswordRequired` 且无更多候选 → 清理临时目录，返回 `NeedsPassword`；其他错误 → 清理后向上传播。
4. 成功且用了密码 → `PasswordBook::learn(pw)`。
5. `resolve_content_root(临时目录)` → `Library::add_folder(db, root_dir, name=archive 文件茎)`（add_folder 签名以 library.rs 现有为准）→ 把内容移入库目录。
6. 清理临时目录（含失败路径——用 guard 结构或显式清理，测试覆盖失败不落垃圾）。
7. 返回 `Installed { mod_id, name, warnings }`。

- [ ] **Step 1: 失败测试**

```rust
#[test] fn installs_plain_zip_into_library()        // zip 入库 → 库目录存在、db 有记录、outcome Installed
#[test] fn wrong_book_then_explicit_password_works() // 密码本含错密码 + explicit 正确 → 成功且 learn
#[test] fn all_passwords_fail_returns_needs_password() // 加密包 + 全错 → NeedsPassword，临时目录已清理
#[test] fn unencrypted_archive_needs_no_password()
#[test] fn temp_dir_cleaned_on_failure()             // 失败路径 tmp 下无残留 liquimod-* 目录
```

测试用临时 app_data 根（paths.rs 若支持注入根目录则注入；不支持则给 paths 加 `#[cfg(test)]` 可注入的 override——先查现有 paths.rs 再定）。

- [ ] **Step 2: 确认 FAIL**

- [ ] **Step 3: 实现**（上述流程）

- [ ] **Step 4: 测试绿 + clippy + commit** `feat(core): install_archive 编排（密码本自动尝试）`

---

## Task 8: CLI install 命令 + 端到端验收

**Files:**
- Modify: `crates/liquimod-cli/src/main.rs`

- [ ] **Step 1: 实现**（照抄现有子命令的 clap 模式）：

```
liquimod install <archive路径> [--password <pw>] [--password-book] [--name <名称>]
```

- 正常 → 打印 `Installed: <name> (id <id>)` 与警告列表
- `NeedsPassword` → 交互式提示输入密码（若 stdin 非 TTY 则报错退出码 2 提示传 --password），输入后带密码重试；重试成功自动 learn 进密码本
- `--password-book` 行为即默认（自动尝试密码本），flag 可省

- [ ] **Step 2: e2e 验收脚本**（PowerShell，手工跑一次并粘贴输出进汇报）：

```powershell
# 造一个普通 zip、一个 AES 加密 zip（用 cargo test 夹具或 python），走 CLI install → list → enable → disable → 断言
```

- [ ] **Step 3: 全量验证**

```powershell
cargo build --workspace; cargo test --workspace; cargo clippy --workspace -- -D warnings
```

- [ ] **Step 4: commit** `feat(cli): install 命令与端到端验收`

---

## 收尾（里程碑 2 完成后）

- [ ] 终审：对照设计文档 `docs/superpowers/specs/2026-08-17-liquimod-design.md` 解压章节逐条核对
- [ ] 里程碑 1 遗留 Minor 顺带评估是否在本里程碑顺手修（见会话纪要清单），不顺手则记录留待里程碑 3
