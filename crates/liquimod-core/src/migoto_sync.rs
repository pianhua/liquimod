//! 3Dmigoto (SRMI) 核心套件内置初始化、云端下载安装与版本同步模块

use crate::error::{LiquiModError, Result};
use base64::Engine;
use futures_util::StreamExt;
use p384::ecdsa::{signature::hazmat::PrehashVerifier, Signature, VerifyingKey};
use p384::pkcs8::DecodePublicKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 默认内置的标准 d3dx.ini 模板文本
pub const EMBEDDED_D3DX_INI_TEMPLATE: &str = r#"; 3Dmigoto for Honkai: Star Rail (SRMI) Configuration File
; Managed by LiquiMod

[Loader]
target = StarRail.exe
loader = XXMI Launcher.exe
module = d3d11.dll
require_admin = false
delay = 0

[Hunting]
hunting = 0
marking_actions = clipboard

[Logging]
calls = 0
debug = 0
show_warnings = 0

[Rendering]
track_texture_updates = 0
track_region_hashes = 0
track_implicit_index_buffers = 1
allow_buffer_resize = 1

[System]
dll_initialization_delay = 0

[Constants]
; Global constants for mod variables

[Include]
include = Core\SRMI\main.ini
include_recursive = Mods
exclude_recursive = DISABLED*
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigotoReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub package: String,
    #[serde(default)]
    pub signature: Option<String>,
    #[serde(default)]
    pub manifest_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigotoDownloadProgress {
    pub stage: String, // "downloading" | "extracting" | "completed" | "failed"
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

/// 获取 LiquiMod 默认的内置/托管 3DMigoto 目录路径。
/// 当前版本始终落在可执行文件旁边，避免回到 `%APPDATA%`/C 盘。
pub fn default_managed_migoto_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("3DMigoto")
}

/// 在指定的目标目录下初始化 3Dmigoto 工作区
pub fn init_migoto_workspace(target_dir: &Path) -> Result<PathBuf> {
    if !target_dir.exists() {
        std::fs::create_dir_all(target_dir)?;
    }

    let mods_dir = target_dir.join("Mods");
    if !mods_dir.exists() {
        std::fs::create_dir_all(&mods_dir)?;
    }

    let shader_fixes_dir = target_dir.join("ShaderFixes");
    if !shader_fixes_dir.exists() {
        std::fs::create_dir_all(&shader_fixes_dir)?;
    }

    let ini_path = target_dir.join("d3dx.ini");
    if !ini_path.exists() {
        std::fs::write(&ini_path, EMBEDDED_D3DX_INI_TEMPLATE)?;
    }

    Ok(ini_path)
}

/// 检查目标目录是否已经是合法的 3Dmigoto 工作区
pub fn is_migoto_workspace(target_dir: &Path) -> bool {
    if !target_dir.is_dir() {
        return false;
    }
    target_dir.join("d3dx.ini").is_file()
}

pub const XXMI_PUBLIC_KEY: &str =
    "MHYwEAYHKoZIzj0CAQYFK4EEACIDYgAEYac352uRGKZh6LOwK0fVDW/TpyECEfnRtUp+bP2PJPP63SWOkJ3a/d9pAnPfYezRVJ1hWjZtpRTT8HEAN/b4mWpJvqO43SAEV/1Q6vz9Rk/VvRV3jZ6B/tmqVnIeHKEb";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PackageKind {
    Srmi,
    Xxmi,
}

impl PackageKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Srmi => "SRMI",
            Self::Xxmi => "XXMI",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PackageStatus {
    pub package: PackageKind,
    pub installed_version: Option<String>,
    pub package_dir: String,
    pub ready: bool,
    pub missing_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePaths {
    pub data_root: PathBuf,
    pub runtime_root: PathBuf,
    pub injector_dll: PathBuf,
    pub d3d11_dll: PathBuf,
    pub d3dcompiler_dll: PathBuf,
    pub nvapi_dll: PathBuf,
    pub d3dx_ini: PathBuf,
    pub mods_dir: PathBuf,
}

fn package_dir(data_root: &Path, package: PackageKind) -> PathBuf {
    data_root.join("Packages").join(package.name())
}

pub fn package_file(data_root: &Path, package: PackageKind, name: &str) -> PathBuf {
    package_dir(data_root, package).join(name)
}

pub fn runtime_paths(data_root: &Path) -> RuntimePaths {
    let runtime_root = data_root.join("3DMigoto");
    RuntimePaths {
        data_root: data_root.to_path_buf(),
        runtime_root: runtime_root.clone(),
        injector_dll: package_file(data_root, PackageKind::Xxmi, "3dmloader.dll"),
        d3d11_dll: runtime_root.join("d3d11.dll"),
        d3dcompiler_dll: runtime_root.join("d3dcompiler_47.dll"),
        nvapi_dll: runtime_root.join("nvapi64.dll"),
        d3dx_ini: runtime_root.join("d3dx.ini"),
        mods_dir: runtime_root.join("Mods"),
    }
}

pub fn package_statuses(data_root: &Path) -> Vec<PackageStatus> {
    [PackageKind::Srmi, PackageKind::Xxmi]
        .into_iter()
        .map(|package| package_status(data_root, package))
        .collect()
}

pub fn package_status(data_root: &Path, package: PackageKind) -> PackageStatus {
    let dir = package_dir(data_root, package);
    let mut missing_files = Vec::new();
    let required = match package {
        PackageKind::Srmi => vec!["Manifest.json", "d3dx.ini", "Core\\SRMI\\main.ini"],
        PackageKind::Xxmi => vec![
            "Manifest.json",
            "3dmloader.dll",
            "d3d11.dll",
            "d3dcompiler_47.dll",
        ],
    };
    for file in required {
        if !dir.join(file).is_file() {
            missing_files.push(file.to_string());
        }
    }
    let installed_version = read_manifest_version(&dir.join("Manifest.json"));
    PackageStatus {
        package,
        installed_version,
        package_dir: dir.display().to_string(),
        ready: missing_files.is_empty(),
        missing_files,
    }
}

/// 将随程序分发的标准 SRMI/XXMI 核心包补入当前便携数据根。
///
/// 发行包可能把核心放在 exe 旁的 `Packages`，也可能由 Tauri 放在
/// `resources/Packages`。只在目标包不完整时复制，且通过与官方安装相同的
/// 结构/签名校验后再原子替换，绝不覆盖已经完整的用户更新版本。
pub fn seed_bundled_packages(data_root: &Path) -> Result<usize> {
    let mut source_roots = vec![data_root.join("Packages")];
    if let Some(exe_parent) = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
    {
        source_roots.push(exe_parent.join("Packages"));
        source_roots.push(exe_parent.join("resources").join("Packages"));
    }

    let mut seeded = 0;
    for package in [PackageKind::Srmi, PackageKind::Xxmi] {
        if package_status(data_root, package).ready {
            continue;
        }

        let target = package_dir(data_root, package);
        let source = source_roots
            .iter()
            .map(|root| root.join(package.name()))
            .find(|candidate| {
                if !candidate.is_dir() || same_path(candidate, &target) {
                    return false;
                }
                verify_package_layout(candidate, package).is_ok()
            });
        let Some(source) = source else {
            continue;
        };

        let package_parent = data_root.join("Packages");
        std::fs::create_dir_all(&package_parent)?;
        let temp = package_parent.join(format!(
            ".{}-bundled-{}",
            package.name(),
            uuid::Uuid::new_v4()
        ));
        if temp.exists() {
            std::fs::remove_dir_all(&temp)?;
        }
        let result = (|| -> Result<()> {
            copy_directory_contents(&source, &temp, &[])?;
            verify_package_layout(&temp, package)?;
            replace_managed_directory(&temp, &target)
        })();
        if result.is_err() && temp.exists() {
            let _ = std::fs::remove_dir_all(&temp);
        }
        result?;
        seeded += 1;
    }
    Ok(seeded)
}

fn same_path(left: &Path, right: &Path) -> bool {
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn read_manifest_version(path: &Path) -> Option<String> {
    let data = std::fs::read_to_string(path).ok()?;
    serde_json::from_str::<serde_json::Value>(&data).ok()?["version"]
        .as_str()
        .map(str::to_string)
}

fn github_api_url(owner: &str, repo: &str, mirror_url: Option<&str>) -> String {
    let original = format!("https://api.github.com/repos/{owner}/{repo}/releases/latest");
    mirror_url
        .filter(|value| !value.trim().is_empty())
        .map(|mirror| format!("{}/{}", mirror.trim_end_matches('/'), original))
        .unwrap_or(original)
}

fn mirrored_download_url(url: &str, mirror_url: Option<&str>) -> String {
    mirror_url
        .filter(|value| !value.trim().is_empty())
        .map(|mirror| format!("{}/{}", mirror.trim_end_matches('/'), url))
        .unwrap_or_else(|| url.to_string())
}

fn release_signature(body: &str) -> Option<String> {
    let marker = body.find("## Signature")?;
    body[marker..]
        .lines()
        .skip(1)
        .map(str::trim)
        .find_map(|line| {
            let value = line.strip_prefix('-')?.trim();
            if value.is_empty() {
                None
            } else if base64::engine::general_purpose::STANDARD
                .decode(value)
                .is_ok()
            {
                Some(value.to_string())
            } else {
                None
            }
        })
}

async fn fetch_latest_package_release(
    package: &str,
    owner: &str,
    repo: &str,
    asset_prefix: &str,
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-XXMI-Compatible")
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let mut request = client.get(github_api_url(owner, repo, mirror_url));
    if let Some(token) = github_token.filter(|value| !value.trim().is_empty()) {
        request = request.header("Authorization", format!("token {}", token.trim()));
    }
    let response = request.send().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "检查 {package} 官方套件更新失败：{e}"
        )))
    })?;
    if !response.status().is_success() {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "{package} GitHub API 返回状态码：{}",
            response.status()
        ))));
    }
    let json = response.json::<serde_json::Value>().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "解析 {package} Release 信息失败：{e}"
        )))
    })?;
    let tag_name = json["tag_name"].as_str().unwrap_or_default().to_string();
    let version = tag_name.trim_start_matches('v');
    let asset_name = format!("{asset_prefix}{version}.zip");
    let Some(assets) = json["assets"].as_array() else {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "{package} Release 没有安装包资产"
        ))));
    };
    let Some(asset) = assets
        .iter()
        .find(|asset| asset["name"].as_str() == Some(&asset_name))
    else {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "{package} Release 缺少官方资产 {asset_name}"
        ))));
    };
    let download_url = asset["browser_download_url"].as_str().map(str::to_string);
    let manifest_url = assets
        .iter()
        .find(|asset| asset["name"].as_str() == Some("Manifest.json"))
        .and_then(|asset| asset["browser_download_url"].as_str())
        .map(str::to_string);
    Ok(MigotoReleaseInfo {
        tag_name,
        name: json["name"].as_str().unwrap_or_default().to_string(),
        body: json["body"].as_str().unwrap_or_default().to_string(),
        published_at: json["published_at"].as_str().map(str::to_string),
        download_url,
        asset_name: Some(asset_name),
        size_bytes: asset["size"].as_u64(),
        package: package.to_string(),
        signature: release_signature(json["body"].as_str().unwrap_or_default()),
        manifest_url,
    })
}

fn verify_official_signature(signature: Option<&str>, data: &[u8]) -> Result<()> {
    let signature = signature.ok_or_else(|| {
        LiquiModError::Io(std::io::Error::other(
            "官方 Release 没有签名，已拒绝安装未验证的核心套件",
        ))
    })?;
    let public_key = base64::engine::general_purpose::STANDARD
        .decode(XXMI_PUBLIC_KEY)
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature)
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let key = VerifyingKey::from_public_key_der(&public_key)
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let signature = Signature::from_der(&signature)
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    key.verify_prehash(&Sha256::digest(data), &signature)
        .map_err(|_| {
            LiquiModError::Io(std::io::Error::other(
                "官方核心套件签名校验失败，已拒绝安装",
            ))
        })
}

/// 获取 SRMI 最新的 Release 版本信息（支持 GitHub 直连、代理及国内镜像加速）
pub async fn check_latest_srmi_release(
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    fetch_latest_package_release(
        "SRMI",
        "SpectrumQT",
        "SRMI-Package",
        "SRMI-TEST-PACKAGE-v",
        github_token,
        mirror_url,
    )
    .await
}

/// 获取 XXMI 注入器套件的最新官方 Release。
pub async fn check_latest_xxmi_release(
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    fetch_latest_package_release(
        "XXMI",
        "SpectrumQT",
        "XXMI-Libs-Package",
        "XXMI-PACKAGE-v",
        github_token,
        mirror_url,
    )
    .await
}

/// 流式下载并一键解压安装 3DMigoto Release 套件到目标目录
pub async fn download_and_install_migoto(
    download_url: &str,
    target_dir: &Path,
    mirror_url: Option<&str>,
    github_token: Option<&str>,
    progress_tx: Option<tokio::sync::mpsc::Sender<MigotoDownloadProgress>>,
) -> Result<()> {
    std::fs::create_dir_all(target_dir)?;

    let final_url = if let Some(mirror) = mirror_url {
        let clean = mirror.trim_end_matches('/');
        if download_url.starts_with("http") {
            format!("{}/{}", clean, download_url)
        } else {
            download_url.to_string()
        }
    } else {
        download_url.to_string()
    };

    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-Client")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;

    let mut req = client.get(&final_url);
    if let Some(token) = github_token {
        if !token.trim().is_empty() {
            req = req.header("Authorization", format!("token {}", token.trim()));
        }
    }

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "downloading".to_string(),
                percent: 0.0,
                downloaded_bytes: 0,
                total_bytes: None,
                message: "正在连接下载 3DMigoto 核心安装包...".to_string(),
            })
            .await;
    }

    let res = req.send().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "连接 3DMigoto 安装包下载失败: {}",
            e
        )))
    })?;

    if !res.status().is_success() {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "下载失败，HTTP 状态码: {}",
            res.status()
        ))));
    }

    let total_bytes = res.content_length();
    let mut downloaded_bytes: u64 = 0;
    let mut stream = res.bytes_stream();
    let mut zip_bytes = Vec::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk
            .map_err(|e| LiquiModError::Io(std::io::Error::other(format!("下载流中断: {}", e))))?;
        downloaded_bytes += chunk.len() as u64;
        zip_bytes.extend_from_slice(&chunk);

        if let Some(tx) = &progress_tx {
            let percent = if let Some(tot) = total_bytes {
                if tot > 0 {
                    (downloaded_bytes as f32 / tot as f32) * 100.0
                } else {
                    50.0
                }
            } else {
                50.0
            };
            let _ = tx
                .send(MigotoDownloadProgress {
                    stage: "downloading".to_string(),
                    percent: percent.min(99.0),
                    downloaded_bytes,
                    total_bytes,
                    message: format!(
                        "正在下载: {} / {}",
                        format_bytes(downloaded_bytes),
                        total_bytes
                            .map(format_bytes)
                            .unwrap_or_else(|| "未知".into())
                    ),
                })
                .await;
        }
    }

    // 下载完成，开始解压与部署
    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "extracting".to_string(),
                percent: 99.0,
                downloaded_bytes,
                total_bytes,
                message: "正在解压并覆写 3DMigoto 核心套件...".to_string(),
            })
            .await;
    }

    let target_buf = target_dir.to_path_buf();
    tokio::task::spawn_blocking(move || extract_migoto_zip_to_dir(&zip_bytes, &target_buf))
        .await
        .map_err(|e| LiquiModError::Io(std::io::Error::other(format!("解压任务失败: {}", e))))??;

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "completed".to_string(),
                percent: 100.0,
                downloaded_bytes,
                total_bytes,
                message: "3DMigoto 核心套件已成功安装/更新！".to_string(),
            })
            .await;
    }

    Ok(())
}

/// 将 3DMigoto 的 zip 字节解压到指定目标目录，智能识别并脱掉顶层根目录包装
fn extract_migoto_zip_to_dir(zip_bytes: &[u8], target_dir: &Path) -> Result<()> {
    let reader = std::io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!("无法解析 ZIP 压缩包: {}", e)))
    })?;

    // 1. 探测是否存在公共顶级包装目录（如 "SRMI-PACKAGE-v2.4.2/"）
    let mut common_prefix = None;
    let file_count = archive.len();
    if file_count > 0 {
        if let Ok(first_entry) = archive.by_index(0) {
            let name = first_entry.name().replace('\\', "/");
            if let Some((top, _)) = name.split_once('/') {
                common_prefix = Some(format!("{}/", top));
            }
        }
        // 校验是否所有文件都以该前缀开头
        if let Some(prefix) = &common_prefix {
            let all_match = (0..file_count).all(|i| {
                if let Ok(e) = archive.by_index(i) {
                    let n = e.name().replace('\\', "/");
                    n.starts_with(prefix)
                } else {
                    false
                }
            });
            if !all_match {
                common_prefix = None;
            }
        }
    }

    const MAX_FILE_COUNT: usize = 10_000;
    const MAX_TOTAL_DECOMPRESSED_BYTES: u64 = 512 * 1024 * 1024; // 512 MB
    const MAX_SINGLE_FILE_BYTES: u64 = 200 * 1024 * 1024; // 200 MB

    if file_count > MAX_FILE_COUNT {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "压缩包文件数量 ({}) 超出安全上限 ({})",
            file_count, MAX_FILE_COUNT
        ))));
    }

    let mut total_decompressed: u64 = 0;

    // 2. 解压每个文件
    for i in 0..file_count {
        let mut entry = archive.by_index(i).map_err(|e| {
            LiquiModError::Io(std::io::Error::other(format!("读取压缩包条目失败: {}", e)))
        })?;

        let raw_name = entry.name().replace('\\', "/");
        let rel_name = if let Some(prefix) = &common_prefix {
            raw_name.strip_prefix(prefix).unwrap_or(&raw_name)
        } else {
            &raw_name
        };

        if rel_name.trim().is_empty() {
            continue;
        }

        // 安全防御 (LM-P1-002): 严禁绝对路径、盘符、UNC 与 ParentDir 逃逸出目标目录
        let out_path = crate::safe_path::ensure_contained(target_dir, Path::new(rel_name))
            .map_err(|_| {
                LiquiModError::Io(std::io::Error::other(format!(
                    "压缩包包含越界路径：{}",
                    entry.name()
                )))
            })?;

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            let entry_size = entry.size();
            if entry_size > MAX_SINGLE_FILE_BYTES {
                return Err(LiquiModError::Io(std::io::Error::other(format!(
                    "单个文件解压大小 ({} MB) 超出安全阈值",
                    entry_size / 1024 / 1024
                ))));
            }
            total_decompressed += entry_size;
            if total_decompressed > MAX_TOTAL_DECOMPRESSED_BYTES {
                return Err(LiquiModError::Io(std::io::Error::other(
                    "解压总数据量超出安全配额，已终止解压以防资源耗尽".to_string(),
                )));
            }

            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // 如果是 d3dx.ini 且本地已存在，优先保留本地已有 d3dx.ini
            if out_path.file_name().and_then(|n| n.to_str()) == Some("d3dx.ini")
                && out_path.is_file()
            {
                let upstream_path = target_dir.join("d3dx.ini.upstream");
                let mut outfile = std::fs::File::create(upstream_path)?;
                std::io::copy(&mut entry, &mut outfile)?;
                continue;
            }

            let mut outfile = std::fs::File::create(&out_path)?;
            std::io::copy(&mut entry, &mut outfile)?;
        }
    }

    // 确保 Mods 目录存在
    let mods_dir = target_dir.join("Mods");
    if !mods_dir.exists() {
        std::fs::create_dir_all(&mods_dir)?;
    }

    // 确保 d3dx.ini 存在
    let ini_path = target_dir.join("d3dx.ini");
    if !ini_path.exists() {
        std::fs::write(&ini_path, EMBEDDED_D3DX_INI_TEMPLATE)?;
    }

    Ok(())
}

async fn download_package_bytes(
    url: &str,
    mirror_url: Option<&str>,
    github_token: Option<&str>,
    progress_tx: Option<&tokio::sync::mpsc::Sender<MigotoDownloadProgress>>,
    package_name: &str,
) -> Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-XXMI-Compatible")
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let mut request = client.get(mirrored_download_url(url, mirror_url));
    if let Some(token) = github_token.filter(|value| !value.trim().is_empty()) {
        request = request.header("Authorization", format!("token {}", token.trim()));
    }
    let response = request.send().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "下载 {package_name} 官方套件失败：{e}"
        )))
    })?;
    if !response.status().is_success() {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "下载 {package_name} 失败，HTTP 状态码：{}",
            response.status()
        ))));
    }
    let total_bytes = response.content_length();
    let mut downloaded_bytes = 0u64;
    let mut data = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
        downloaded_bytes += chunk.len() as u64;
        data.extend_from_slice(&chunk);
        if let Some(tx) = progress_tx {
            let percent = total_bytes
                .filter(|total| *total > 0)
                .map(|total| (downloaded_bytes as f32 / total as f32 * 100.0).min(99.0))
                .unwrap_or(50.0);
            let _ = tx
                .send(MigotoDownloadProgress {
                    stage: "downloading".to_string(),
                    percent,
                    downloaded_bytes,
                    total_bytes,
                    message: format!(
                        "正在下载 {package_name}：{} / {}",
                        format_bytes(downloaded_bytes),
                        total_bytes
                            .map(format_bytes)
                            .unwrap_or_else(|| "未知".to_string())
                    ),
                })
                .await;
        }
    }
    Ok(data)
}

fn verify_manifest_files(root: &Path, required: &[&str]) -> Result<()> {
    let manifest_path = root.join("Manifest.json");
    let manifest = serde_json::from_str::<serde_json::Value>(
        &std::fs::read_to_string(&manifest_path).map_err(|_| {
            LiquiModError::Io(std::io::Error::other(format!(
                "XXMI 套件缺少 Manifest.json：{}",
                manifest_path.display()
            )))
        })?,
    )
    .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;
    let signatures = manifest["signatures"].as_object().ok_or_else(|| {
        LiquiModError::Io(std::io::Error::other("XXMI Manifest.json 缺少 signatures"))
    })?;
    for name in required {
        let path = root.join(name);
        if !path.is_file() {
            return Err(LiquiModError::Io(std::io::Error::other(format!(
                "XXMI 套件缺少关键文件：{name}"
            ))));
        }
        let signature = signatures
            .get(*name)
            .and_then(|value| value.as_str())
            .ok_or_else(|| {
                LiquiModError::Io(std::io::Error::other(format!(
                    "XXMI Manifest.json 缺少 {name} 的签名"
                )))
            })?;
        let bytes = std::fs::read(&path)?;
        verify_official_signature(Some(signature), &bytes)?;
    }
    Ok(())
}

fn verify_package_layout(root: &Path, package: PackageKind) -> Result<()> {
    let required = match package {
        PackageKind::Srmi => ["Manifest.json", "d3dx.ini", "Core\\SRMI\\main.ini"].as_slice(),
        PackageKind::Xxmi => [
            "Manifest.json",
            "3dmloader.dll",
            "d3d11.dll",
            "d3dcompiler_47.dll",
        ]
        .as_slice(),
    };
    for name in required {
        if !root.join(name).is_file() {
            return Err(LiquiModError::Io(std::io::Error::other(format!(
                "{} 官方套件缺少关键文件：{}",
                package.name(),
                name
            ))));
        }
    }
    if package == PackageKind::Xxmi {
        verify_manifest_files(root, &["3dmloader.dll", "d3d11.dll", "d3dcompiler_47.dll"])?;
    }
    Ok(())
}

fn replace_managed_directory(source: &Path, target: &Path) -> Result<()> {
    let backup = target.with_file_name(format!(
        ".{}.old-{}",
        target
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package"),
        uuid::Uuid::new_v4()
    ));
    if target.exists() {
        std::fs::rename(target, &backup)?;
    }
    if let Err(error) = std::fs::rename(source, target) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, target);
        }
        return Err(error.into());
    }
    if backup.exists() {
        std::fs::remove_dir_all(backup)?;
    }
    Ok(())
}

fn ensure_standard_d3dx_includes(path: &Path) -> Result<()> {
    let mut content = std::fs::read_to_string(path)?;
    let needs_core = !content.lines().any(|line| {
        line.trim()
            .eq_ignore_ascii_case("include = Core\\SRMI\\main.ini")
    });
    let needs_mods = !content
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("include_recursive = Mods"));
    if !needs_core && !needs_mods {
        return Ok(());
    }
    if !content.ends_with(['\n', '\r']) {
        content.push_str("\r\n");
    }
    content.push_str("\r\n[Include]\r\n");
    if needs_core {
        content.push_str("include = Core\\SRMI\\main.ini\r\n");
    }
    if needs_mods {
        content.push_str("include_recursive = Mods\r\n");
    }
    content.push_str("exclude_recursive = DISABLED*\r\n");
    std::fs::write(path, content)?;
    Ok(())
}

/// 判断运行目录里的 d3dx.ini 是否仍是 LiquiMod 为“仅创建工作区”生成的占位配置。
///
/// 首次启动时需要先创建 Mods 目录，旧实现同时写入了一个精简模板；如果随后不把
/// 官方 SRMI d3dx.ini 替换进去，XXMI 注入链会缺少官方 Loader/Include 配置。
/// 只识别带有 LiquiMod 标记的文件，避免覆盖用户自行维护的标准配置。
fn is_liquimod_placeholder_d3dx_ini(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|content| content.contains("; Managed by LiquiMod"))
        .unwrap_or(false)
}

/// 安装已通过官方 ECDSA 校验的 SRMI/XXMI 包到便携式 `Packages` 缓存。
/// 不触碰 Library、Mods 和外部 Mod 源。
pub async fn install_official_package(
    release: &MigotoReleaseInfo,
    data_root: &Path,
    mirror_url: Option<&str>,
    github_token: Option<&str>,
    progress_tx: Option<tokio::sync::mpsc::Sender<MigotoDownloadProgress>>,
) -> Result<()> {
    let package = match release.package.as_str() {
        "XXMI" => PackageKind::Xxmi,
        _ => PackageKind::Srmi,
    };
    let url = release.download_url.as_deref().ok_or_else(|| {
        LiquiModError::Io(std::io::Error::other("官方 Release 缺少安装包下载地址"))
    })?;
    let data = download_package_bytes(
        url,
        mirror_url,
        github_token,
        progress_tx.as_ref(),
        package.name(),
    )
    .await?;
    verify_official_signature(release.signature.as_deref(), &data)?;

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "extracting".to_string(),
                percent: 99.0,
                downloaded_bytes: data.len() as u64,
                total_bytes: Some(data.len() as u64),
                message: format!("正在校验并安装 {} 官方套件…", package.name()),
            })
            .await;
    }

    let package_parent = data_root.join("Packages");
    std::fs::create_dir_all(&package_parent)?;
    let temp = package_parent.join(format!(".{}-tmp-{}", package.name(), uuid::Uuid::new_v4()));
    if temp.exists() {
        std::fs::remove_dir_all(&temp)?;
    }
    std::fs::create_dir_all(&temp)?;
    let manifest_data = if let Some(manifest_url) = release.manifest_url.as_deref() {
        Some(
            download_package_bytes(manifest_url, mirror_url, github_token, None, package.name())
                .await?,
        )
    } else {
        None
    };
    let result = (|| -> Result<()> {
        extract_migoto_zip_to_dir(&data, &temp)?;
        if let Some(manifest) = &manifest_data {
            let _: serde_json::Value = serde_json::from_slice(manifest).map_err(|e| {
                LiquiModError::Io(std::io::Error::other(format!(
                    "{} Manifest.json 无效：{}",
                    package.name(),
                    e
                )))
            })?;
            std::fs::write(temp.join("Manifest.json"), manifest)?;
        } else if package == PackageKind::Srmi {
            let asset_name = release.asset_name.as_deref().unwrap_or("SRMI-PACKAGE.zip");
            let manifest = serde_json::json!({
                "version": release.tag_name.trim_start_matches('v'),
                "signatures": { asset_name: release.signature.clone().unwrap_or_default() }
            });
            std::fs::write(
                temp.join("Manifest.json"),
                serde_json::to_vec_pretty(&manifest)
                    .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?,
            )?;
        }
        verify_package_layout(&temp, package)?;
        replace_managed_directory(&temp, &package_dir(data_root, package))?;
        Ok(())
    })();
    if result.is_err() && temp.exists() {
        let _ = std::fs::remove_dir_all(&temp);
    }
    result?;

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "completed".to_string(),
                percent: 100.0,
                downloaded_bytes: data.len() as u64,
                total_bytes: Some(data.len() as u64),
                message: format!("{} 官方套件已安装并完成签名校验", package.name()),
            })
            .await;
    }
    Ok(())
}

fn copy_directory_contents(source: &Path, target: &Path, skip: &[&str]) -> Result<()> {
    if !source.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        if skip
            .iter()
            .any(|value| name_text.eq_ignore_ascii_case(value))
        {
            continue;
        }
        let source_path = entry.path();
        let target_path = target.join(&name);
        if entry.file_type()?.is_dir() {
            copy_directory_contents(&source_path, &target_path, skip)?;
        } else if entry.file_type()?.is_file() {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(source_path, target_path)?;
        }
    }
    Ok(())
}

/// 将 SRMI 包和 XXMI 二进制部署成标准的便携式 3DMigoto 工作区。
/// `Mods` 目录只由上层 Deployer 建立 Junction，不会复制 Mod 实体文件。
pub fn prepare_managed_runtime(
    data_root: &Path,
    game_exe: &Path,
    mode: crate::d3d::MigotoWorkMode,
    delay_ms: u64,
) -> Result<RuntimePaths> {
    let srmi = package_status(data_root, PackageKind::Srmi);
    let xxmi = package_status(data_root, PackageKind::Xxmi);
    if !srmi.ready || !xxmi.ready {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "XXMI/SRMI 核心尚未安装完整（SRMI 缺少 {:?}；XXMI 缺少 {:?}）",
            srmi.missing_files, xxmi.missing_files
        ))));
    }
    let paths = runtime_paths(data_root);
    std::fs::create_dir_all(&paths.runtime_root)?;
    let package_ini = package_file(data_root, PackageKind::Srmi, "d3dx.ini");
    if package_ini.is_file()
        && (!paths.d3dx_ini.is_file() || is_liquimod_placeholder_d3dx_ini(&paths.d3dx_ini))
    {
        std::fs::copy(package_ini, &paths.d3dx_ini)?;
    }
    if !paths.d3dx_ini.is_file() {
        init_migoto_workspace(&paths.runtime_root)?;
    }
    copy_directory_contents(
        &package_dir(data_root, PackageKind::Srmi),
        &paths.runtime_root,
        &[
            "Manifest.json",
            "Mods",
            "ShaderFixes",
            "ShaderCache",
            "d3dx.ini",
        ],
    )?;
    let package_user_ini = package_file(data_root, PackageKind::Srmi, "d3dx_user.ini");
    let runtime_user_ini = paths.runtime_root.join("d3dx_user.ini");
    if package_user_ini.is_file() && !runtime_user_ini.is_file() {
        std::fs::copy(package_user_ini, runtime_user_ini)?;
    }
    std::fs::create_dir_all(&paths.mods_dir)?;
    std::fs::create_dir_all(paths.runtime_root.join("ShaderFixes"))?;
    ensure_standard_d3dx_includes(&paths.d3dx_ini)?;
    std::fs::copy(
        package_file(data_root, PackageKind::Xxmi, "d3d11.dll"),
        &paths.d3d11_dll,
    )?;
    std::fs::copy(
        package_file(data_root, PackageKind::Xxmi, "d3dcompiler_47.dll"),
        &paths.d3dcompiler_dll,
    )?;
    let package_nvapi = package_file(data_root, PackageKind::Xxmi, "nvapi64.dll");
    if package_nvapi.is_file() {
        std::fs::copy(package_nvapi, &paths.nvapi_dll)?;
    }
    let target_name = game_exe
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| game_exe.to_path_buf());
    crate::d3d::update_d3dx_ini_target(&paths.d3dx_ini, &target_name)?;
    crate::d3d::ensure_xxmi_loader_name(&paths.d3dx_ini)?;
    crate::d3d::update_d3dx_ini_initialization_delay(&paths.d3dx_ini, delay_ms)?;
    crate::d3d::update_d3dx_ini_mode(&paths.d3dx_ini, mode)?;
    Ok(paths)
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_migoto_workspace() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("SRMI");

        assert!(!is_migoto_workspace(&target));
        let ini = init_migoto_workspace(&target).unwrap();
        assert!(ini.is_file());
        assert!(is_migoto_workspace(&target));
        assert!(target.join("Mods").is_dir());
        assert!(target.join("ShaderFixes").is_dir());

        let content = std::fs::read_to_string(&ini).unwrap();
        assert!(content.contains("[Loader]"));
        assert!(content.contains("target = StarRail.exe"));
    }

    #[test]
    fn prepare_managed_runtime_replaces_liquimod_placeholder_with_official_ini() {
        let temp = tempfile::tempdir().unwrap();
        let packages =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/builtin-core/Packages");
        copy_directory_contents(&packages, &temp.path().join("Packages"), &[]).unwrap();

        let runtime_root = temp.path().join("3DMigoto");
        init_migoto_workspace(&runtime_root).unwrap();
        assert!(is_liquimod_placeholder_d3dx_ini(
            &runtime_root.join("d3dx.ini")
        ));

        let paths = prepare_managed_runtime(
            temp.path(),
            Path::new("StarRail.exe"),
            crate::d3d::MigotoWorkMode::Play,
            0,
        )
        .unwrap();
        let content = std::fs::read_to_string(&paths.d3dx_ini).unwrap();

        assert!(!content.contains("; Managed by LiquiMod"));
        assert!(content.contains("loader = XXMI Launcher.exe"));
        assert!(content.contains("include = Core\\SRMI\\main.ini"));
        assert!(content.contains("include_recursive = Mods"));

        std::fs::write(
            &paths.d3dx_ini,
            content.replace("loader = XXMI Launcher.exe", "loader = liquimod-app.exe"),
        )
        .unwrap();
        let paths = prepare_managed_runtime(
            temp.path(),
            Path::new("StarRail.exe"),
            crate::d3d::MigotoWorkMode::Play,
            0,
        )
        .unwrap();
        let migrated_content = std::fs::read_to_string(&paths.d3dx_ini).unwrap();
        assert!(migrated_content.contains("loader = XXMI Launcher.exe"));
        assert!(!migrated_content.contains("loader = liquimod-app.exe"));
    }

    #[test]
    fn test_extract_migoto_zip_strips_common_prefix() {
        use std::io::Write;
        let mut zip_data = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_data));
            let options = zip::write::SimpleFileOptions::default();
            writer
                .start_file("SRMI-PACKAGE-v2.4.2/d3d11.dll", options)
                .unwrap();
            writer.write_all(b"fake d3d11").unwrap();
            writer
                .start_file("SRMI-PACKAGE-v2.4.2/ShaderFixes/fix.hlsl", options)
                .unwrap();
            writer.write_all(b"fake shader").unwrap();
            writer.finish().unwrap();
        }

        let temp = tempfile::tempdir().unwrap();
        extract_migoto_zip_to_dir(&zip_data, temp.path()).unwrap();

        assert!(temp.path().join("d3d11.dll").is_file());
        assert!(temp.path().join("ShaderFixes/fix.hlsl").is_file());
        assert!(temp.path().join("Mods").is_dir());
        assert!(temp.path().join("d3dx.ini").is_file());
    }

    #[test]
    fn bundled_core_packages_have_official_layout() {
        let packages =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/builtin-core/Packages");
        verify_package_layout(&packages.join("SRMI"), PackageKind::Srmi).unwrap();
        verify_package_layout(&packages.join("XXMI"), PackageKind::Xxmi).unwrap();
    }
}
