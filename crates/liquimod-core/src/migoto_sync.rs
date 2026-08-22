//! 3Dmigoto (SRMI) 核心套件内置初始化、云端下载安装与版本同步模块

use crate::error::{LiquiModError, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 默认内置的标准 d3dx.ini 模板文本 (对齐 XXMI 1071 行完整规范)
pub const EMBEDDED_D3DX_INI_TEMPLATE: &str = include_str!("../../../assets/srmi/d3dx.ini");

/// 预置内置的 SRMI 核心套件静态文件映射 (包含 3dmloader.dll, d3d11.dll, d3dcompiler_47.dll, 骨骼蒙皮 Compute Shader、字体、通知及帮助)
pub const EMBEDDED_SRMI_FILES: &[(&str, &[u8])] = &[
    (
        "3dmloader.dll",
        include_bytes!("../../../assets/srmi/3dmloader.dll"),
    ),
    (
        "d3d11.dll",
        include_bytes!("../../../assets/srmi/d3d11.dll"),
    ),
    (
        "d3dcompiler_47.dll",
        include_bytes!("../../../assets/srmi/d3dcompiler_47.dll"),
    ),
    ("d3dx.ini", include_bytes!("../../../assets/srmi/d3dx.ini")),
    (
        "Core/SRMI/main.ini",
        include_bytes!("../../../assets/srmi/Core/SRMI/main.ini"),
    ),
    (
        "Core/SRMI/BatchedPose.ini",
        include_bytes!("../../../assets/srmi/Core/SRMI/BatchedPose.ini"),
    ),
    (
        "Core/SRMI/d3dx_patch.ini",
        include_bytes!("../../../assets/srmi/Core/SRMI/d3dx_patch.ini"),
    ),
    (
        "Core/SRMI/help.ini",
        include_bytes!("../../../assets/srmi/Core/SRMI/help.ini"),
    ),
    (
        "Core/SRMI/Fonts/LiberationSans-Bold.dds",
        include_bytes!("../../../assets/srmi/Core/SRMI/Fonts/LiberationSans-Bold.dds"),
    ),
    (
        "Core/SRMI/Fonts/LiberationSans-Bold.png",
        include_bytes!("../../../assets/srmi/Core/SRMI/Fonts/LiberationSans-Bold.png"),
    ),
    (
        "Core/SRMI/Notifications/HuntingModeGuide.md",
        include_bytes!("../../../assets/srmi/Core/SRMI/Notifications/HuntingModeGuide.md"),
    ),
    (
        "Core/SRMI/Notifications/UserGuide.md",
        include_bytes!("../../../assets/srmi/Core/SRMI/Notifications/UserGuide.md"),
    ),
    (
        "Core/SRMI/Shaders/MultiSkinning1VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/MultiSkinning1VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/MultiSkinning2VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/MultiSkinning2VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/MultiSkinning3VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/MultiSkinning3VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/MultiSkinning4VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/MultiSkinning4VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/MultiSkinning4VG_56.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/MultiSkinning4VG_56.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/SingleSkinning.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/SingleSkinning.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/SingleSkinning1VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/SingleSkinning1VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/SingleSkinning2VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/SingleSkinning2VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/SingleSkinning3VG.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/SingleSkinning3VG.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/SingleSkinning_56.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/SingleSkinning_56.hlsl"),
    ),
    (
        "Core/SRMI/Shaders/TextPrinter.hlsl",
        include_bytes!("../../../assets/srmi/Core/SRMI/Shaders/TextPrinter.hlsl"),
    ),
    (
        "Core/Debugger/Debugger.ini",
        include_bytes!("../../../assets/srmi/Core/Debugger/Debugger.ini"),
    ),
    (
        "Core/Debugger/debug_cb.ini",
        include_bytes!("../../../assets/srmi/Core/Debugger/debug_cb.ini"),
    ),
    (
        "Core/Debugger/Fonts/LiberationSans-Bold.dds",
        include_bytes!("../../../assets/srmi/Core/Debugger/Fonts/LiberationSans-Bold.dds"),
    ),
    (
        "Core/Debugger/Fonts/LiberationSans-Bold.png",
        include_bytes!("../../../assets/srmi/Core/Debugger/Fonts/LiberationSans-Bold.png"),
    ),
    (
        "Core/Debugger/Notifications/CompatibilityMode.md",
        include_bytes!("../../../assets/srmi/Core/Debugger/Notifications/CompatibilityMode.md"),
    ),
    (
        "Core/Debugger/Notifications/ErrorCompatibilityModeDisabled.md",
        include_bytes!(
            "../../../assets/srmi/Core/Debugger/Notifications/ErrorCompatibilityModeDisabled.md"
        ),
    ),
    (
        "Core/Debugger/Notifications/ErrorOldVersionMod.md",
        include_bytes!("../../../assets/srmi/Core/Debugger/Notifications/ErrorOldVersionMod.md"),
    ),
    (
        "Core/Debugger/Notifications/ErrorOldVersionWWMI.md",
        include_bytes!("../../../assets/srmi/Core/Debugger/Notifications/ErrorOldVersionWWMI.md"),
    ),
    (
        "Core/Debugger/Notifications/HuntingModeGuide.md",
        include_bytes!("../../../assets/srmi/Core/Debugger/Notifications/HuntingModeGuide.md"),
    ),
    (
        "Core/Debugger/Notifications/UserGuide.md",
        include_bytes!("../../../assets/srmi/Core/Debugger/Notifications/UserGuide.md"),
    ),
    (
        "Core/Debugger/Shaders/Debugger.cs_5_0.8000.bin",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/Debugger.cs_5_0.8000.bin"),
    ),
    (
        "Core/Debugger/Shaders/Debugger.hlsl",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/Debugger.hlsl"),
    ),
    (
        "Core/Debugger/Shaders/debug_cb.gs_5_0.4.bin",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/debug_cb.gs_5_0.4.bin"),
    ),
    (
        "Core/Debugger/Shaders/debug_cb.hlsl",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/debug_cb.hlsl"),
    ),
    (
        "Core/Debugger/Shaders/debug_cb.ps_5_0.4.bin",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/debug_cb.ps_5_0.4.bin"),
    ),
    (
        "Core/Debugger/Shaders/debug_cb.vs_5_0.4.bin",
        include_bytes!("../../../assets/srmi/Core/Debugger/Shaders/debug_cb.vs_5_0.4.bin"),
    ),
    (
        "ShaderFixes/Sucrose.png",
        include_bytes!("../../../assets/srmi/ShaderFixes/Sucrose.png"),
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigotoReleaseInfo {
    pub tag_name: String,
    pub name: String,
    pub body: String,
    pub published_at: Option<String>,
    pub download_url: Option<String>,
    pub libs_download_url: Option<String>,
    pub asset_name: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigotoDownloadProgress {
    pub stage: String, // "downloading_libs" | "extracting_libs" | "downloading_srmi" | "extracting_srmi" | "completed" | "failed"
    pub percent: f32,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

/// 获取 LiquiMod 默认的内置/托管 3DMigoto 目录路径 (%APPDATA%/LiquiMod/3DMigoto)
pub fn default_managed_migoto_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("LiquiMod")
        .join("3DMigoto")
}

/// 部署内置的完整 SRMI 核心套件到目标目录 (自动校验与升级核心 DLL、蒙皮着色器与 d3dx.ini)
pub fn deploy_embedded_srmi_suite(target_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(target_dir)?;
    std::fs::create_dir_all(target_dir.join("Mods"))?;
    std::fs::create_dir_all(target_dir.join("ShaderFixes"))?;
    std::fs::create_dir_all(target_dir.join("ShaderCache"))?;

    for (rel_path, bytes) in EMBEDDED_SRMI_FILES {
        let dest = target_dir.join(rel_path);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let should_write = if !dest.exists() {
            true
        } else if *rel_path == "d3dx.ini" {
            // d3dx.ini 采用下方专用的智能合并逻辑，不直接覆盖
            false
        } else {
            // 核心 DLL (3dmloader.dll / d3d11.dll / d3dcompiler_47.dll) 与着色器：对比二进制内容，不匹配则自动无感升级
            match std::fs::read(&dest) {
                Ok(existing) => existing != *bytes,
                Err(_) => true,
            }
        };

        if should_write {
            std::fs::write(&dest, bytes)?;
        }
    }

    // 针对 d3dx.ini：若存在则做智能合并，确保 [Include] 与渲染参数就绪同时保留用户自定义配置
    let ini_path = target_dir.join("d3dx.ini");
    if ini_path.is_file() {
        if let Ok(existing_ini) = std::fs::read_to_string(&ini_path) {
            let merged = merge_d3dx_ini(&existing_ini, EMBEDDED_D3DX_INI_TEMPLATE);
            if merged != existing_ini {
                let _ = std::fs::write(&ini_path, merged);
            }
        }
    } else {
        std::fs::write(&ini_path, EMBEDDED_D3DX_INI_TEMPLATE)?;
    }

    Ok(())
}

/// 在指定的目标目录下初始化 3Dmigoto 工作区
pub fn init_migoto_workspace(target_dir: &Path) -> Result<PathBuf> {
    deploy_embedded_srmi_suite(target_dir)?;
    let ini_path = target_dir.join("d3dx.ini");
    Ok(ini_path)
}

/// 检查目标目录是否已经是合法的 3Dmigoto 工作区
pub fn is_migoto_workspace(target_dir: &Path) -> bool {
    if !target_dir.is_dir() {
        return false;
    }
    target_dir.join("d3dx.ini").is_file()
}

/// 辅助方法：从 GitHub API 拉取指定仓库的 Release 信息
async fn fetch_github_release(
    repo_owner: &str,
    repo_name: &str,
    client: &reqwest::Client,
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    let raw_api = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        repo_owner, repo_name
    );
    let base_api = if let Some(mirror) = mirror_url {
        let clean = mirror.trim_end_matches('/');
        format!("{}/{}", clean, raw_api)
    } else {
        raw_api
    };

    let mut req = client.get(&base_api);
    if let Some(token) = github_token {
        if !token.trim().is_empty() {
            req = req.header("Authorization", format!("token {}", token.trim()));
        }
    }

    let res = req.send().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "检查 {}/{} 更新失败: {}",
            repo_owner, repo_name, e
        )))
    })?;

    if !res.status().is_success() {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "GitHub API 响应错误状态码 ({}): {}",
            repo_name,
            res.status()
        ))));
    }

    let json: serde_json::Value = res.json().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "解析 Release JSON 失败: {}",
            e
        )))
    })?;

    let tag_name = json["tag_name"].as_str().unwrap_or("").to_string();
    let name = json["name"].as_str().unwrap_or(&tag_name).to_string();
    let body = json["body"].as_str().unwrap_or("").to_string();
    let published_at = json["published_at"].as_str().map(|s| s.to_string());

    let mut download_url = None;
    let mut asset_name = None;
    let mut size_bytes = None;

    if let Some(assets) = json["assets"].as_array() {
        for asset in assets {
            if let Some(a_name) = asset["name"].as_str() {
                if a_name.ends_with(".zip") {
                    asset_name = Some(a_name.to_string());
                    download_url = asset["browser_download_url"]
                        .as_str()
                        .map(|s| s.to_string());
                    size_bytes = asset["size"].as_u64();
                    break;
                }
            }
        }
    }

    Ok(MigotoReleaseInfo {
        tag_name,
        name,
        body,
        published_at,
        download_url,
        libs_download_url: None,
        asset_name,
        size_bytes,
    })
}

/// 获取 SRMI 最新的 Release 版本信息及关联的 Libs DLL 套件信息
pub async fn check_latest_srmi_release(
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-Client")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;

    let mut srmi_info = fetch_github_release(
        "SpectrumQT",
        "SRMI-Package",
        &client,
        github_token,
        mirror_url,
    )
    .await?;

    if let Ok(libs_info) = fetch_github_release(
        "SpectrumQT",
        "XXMI-Libs-Package",
        &client,
        github_token,
        mirror_url,
    )
    .await
    {
        srmi_info.libs_download_url = libs_info.download_url;
    }

    Ok(srmi_info)
}

/// 辅助流式下载方法
async fn download_zip_stream(
    client: &reqwest::Client,
    url: &str,
    mirror_url: Option<&str>,
    github_token: Option<&str>,
    stage_name: &str,
    progress_tx: &Option<tokio::sync::mpsc::Sender<MigotoDownloadProgress>>,
) -> Result<Vec<u8>> {
    let final_url = if let Some(mirror) = mirror_url {
        let clean = mirror.trim_end_matches('/');
        if url.starts_with("http") {
            format!("{}/{}", clean, url)
        } else {
            url.to_string()
        }
    } else {
        url.to_string()
    };

    let mut req = client.get(&final_url);
    if let Some(token) = github_token {
        if !token.trim().is_empty() {
            req = req.header("Authorization", format!("token {}", token.trim()));
        }
    }

    let res = req
        .send()
        .await
        .map_err(|e| LiquiModError::Io(std::io::Error::other(format!("连接下载失败: {}", e))))?;

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

        if let Some(tx) = progress_tx {
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
                    stage: stage_name.to_string(),
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

    Ok(zip_bytes)
}

/// 流式下载并一键解压安装 3DMigoto 双包套件 (Libs DLL + SRMI Core) 到目标目录
pub async fn download_and_install_migoto(
    download_url: &str,
    target_dir: &Path,
    mirror_url: Option<&str>,
    github_token: Option<&str>,
    progress_tx: Option<tokio::sync::mpsc::Sender<MigotoDownloadProgress>>,
) -> Result<()> {
    std::fs::create_dir_all(target_dir)?;

    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-Client")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;

    // 1. 获取并下载 XXMI-Libs-Package (核心 DLL 套件: 3dmloader.dll, d3d11.dll, d3dcompiler_47.dll)
    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "downloading_libs".to_string(),
                percent: 0.0,
                downloaded_bytes: 0,
                total_bytes: None,
                message: "正在获取 3DMigoto 核心 DLL 依赖包...".to_string(),
            })
            .await;
    }

    if let Ok(libs_info) = fetch_github_release(
        "SpectrumQT",
        "XXMI-Libs-Package",
        &client,
        github_token,
        mirror_url,
    )
    .await
    {
        if let Some(libs_url) = libs_info.download_url {
            let libs_bytes = download_zip_stream(
                &client,
                &libs_url,
                mirror_url,
                github_token,
                "downloading_libs",
                &progress_tx,
            )
            .await?;

            let target_buf = target_dir.to_path_buf();
            tokio::task::spawn_blocking(move || {
                extract_migoto_zip_to_dir(&libs_bytes, &target_buf)
            })
            .await
            .map_err(|e| {
                LiquiModError::Io(std::io::Error::other(format!("解压 DLL 失败: {}", e)))
            })??;
        }
    }

    // 2. 下载并解压 SRMI 套件 (Core/SRMI, ShaderFixes, d3dx.ini)
    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "downloading_srmi".to_string(),
                percent: 0.0,
                downloaded_bytes: 0,
                total_bytes: None,
                message: "正在下载 SRMI 核心套件与着色器...".to_string(),
            })
            .await;
    }

    let srmi_bytes = download_zip_stream(
        &client,
        download_url,
        mirror_url,
        github_token,
        "downloading_srmi",
        &progress_tx,
    )
    .await?;

    let srmi_len = srmi_bytes.len() as u64;

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "extracting_srmi".to_string(),
                percent: 99.0,
                downloaded_bytes: srmi_len,
                total_bytes: Some(srmi_len),
                message: "正在解压并覆写 SRMI 着色器套件...".to_string(),
            })
            .await;
    }

    let target_buf = target_dir.to_path_buf();
    tokio::task::spawn_blocking(move || extract_migoto_zip_to_dir(&srmi_bytes, &target_buf))
        .await
        .map_err(|e| {
            LiquiModError::Io(std::io::Error::other(format!("解压 SRMI 失败: {}", e)))
        })??;

    // 3. 兜底确保内置的 Core/SRMI 套件存在
    let _ = deploy_embedded_srmi_suite(target_dir);

    if let Some(tx) = &progress_tx {
        let _ = tx
            .send(MigotoDownloadProgress {
                stage: "completed".to_string(),
                percent: 100.0,
                downloaded_bytes: srmi_len,
                total_bytes: Some(srmi_len),
                message: "3DMigoto / SRMI 核心套件已成功安装/更新！".to_string(),
            })
            .await;
    }

    Ok(())
}

/// 合并保留用户自定义的 d3dx.ini 参数
fn merge_d3dx_ini(old_content: &str, new_content: &str) -> String {
    let mut target_val = None;
    let mut loader_val = None;

    for line in old_content.lines() {
        let trimmed = line.trim();
        if let Some((k, v)) = trimmed.split_once('=') {
            let key = k.trim().to_lowercase();
            if key == "target" {
                target_val = Some(v.trim().to_string());
            } else if key == "loader" {
                loader_val = Some(v.trim().to_string());
            }
        }
    }

    let mut out = String::new();
    let mut in_loader_sec = false;

    for line in new_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_loader_sec = trimmed[1..trimmed.len() - 1]
                .trim()
                .eq_ignore_ascii_case("loader");
            out.push_str(line);
            out.push('\n');
            continue;
        }

        if in_loader_sec {
            if let Some((k, _)) = trimmed.split_once('=') {
                let key = k.trim().to_lowercase();
                if key == "target" {
                    if let Some(t) = &target_val {
                        out.push_str(&format!("target = {}\n", t));
                        continue;
                    }
                } else if key == "loader" {
                    if let Some(l) = &loader_val {
                        out.push_str(&format!("loader = {}\n", l));
                        continue;
                    }
                }
            }
        }

        out.push_str(line);
        out.push('\n');
    }

    out
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
        let Ok(out_path) = crate::safe_path::ensure_contained(target_dir, Path::new(rel_name))
        else {
            continue;
        };

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

            // 如果是 d3dx.ini 且本地已存在，智能合并并覆写更新（保留自定义 target/loader）
            if out_path.file_name().and_then(|n| n.to_str()) == Some("d3dx.ini")
                && out_path.is_file()
            {
                let mut new_bytes = Vec::new();
                std::io::copy(&mut entry, &mut new_bytes)?;
                let new_str = String::from_utf8_lossy(&new_bytes);
                let old_str = std::fs::read_to_string(&out_path).unwrap_or_default();
                let merged = merge_d3dx_ini(&old_str, &new_str);
                std::fs::write(&out_path, merged)?;
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
        assert!(target.join("3dmloader.dll").is_file());
        assert!(target.join("d3d11.dll").is_file());
        assert!(target.join("d3dcompiler_47.dll").is_file());
        assert!(target.join("Core/SRMI/BatchedPose.ini").is_file());
        assert!(target.join("Core/SRMI/main.ini").is_file());
        assert!(target
            .join("Core/SRMI/Shaders/SingleSkinning.hlsl")
            .is_file());

        let content = std::fs::read_to_string(&ini).unwrap();
        assert!(content.contains("[Loader]"));
        assert!(content.contains("[Include]"));
        assert!(content.contains("include = Core\\SRMI\\main.ini"));
        assert!(content.contains("include_recursive = Mods"));
        assert!(content.contains("global $costume_mods = 1"));
    }

    #[test]
    fn test_deploy_embedded_srmi_suite_upgrades_outdated_dll() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("SRMI");
        std::fs::create_dir_all(&target).unwrap();

        // 模拟用户目录残留的旧版 2.9MB vanilla d3d11.dll
        let dummy_old_dll = b"old-vanilla-3dmigoto-dll-content";
        std::fs::write(target.join("d3d11.dll"), dummy_old_dll).unwrap();

        deploy_embedded_srmi_suite(&target).unwrap();

        let upgraded_bytes = std::fs::read(target.join("d3d11.dll")).unwrap();
        assert_ne!(upgraded_bytes, dummy_old_dll);
        assert_eq!(
            upgraded_bytes.len(),
            include_bytes!("../../../assets/srmi/d3d11.dll").len()
        );
    }

    #[test]
    fn test_merge_d3dx_ini_preserves_custom_target() {
        let old_ini = "[Loader]\ntarget = D:\\Custom\\StarRail.exe\nloader = MyLoader.exe\n";
        let new_ini = "[Loader]\ntarget = StarRail.exe\nloader = XXMI Launcher.exe\n[Include]\ninclude = Core\\SRMI\\main.ini\n";

        let merged = merge_d3dx_ini(old_ini, new_ini);
        assert!(merged.contains("target = D:\\Custom\\StarRail.exe"));
        assert!(merged.contains("loader = MyLoader.exe"));
        assert!(merged.contains("include = Core\\SRMI\\main.ini"));
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
}
