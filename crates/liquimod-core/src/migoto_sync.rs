//! 3Dmigoto (SRMI) 核心套件内置初始化、云端下载安装与版本同步模块

use crate::error::{LiquiModError, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 默认内置的标准 d3dx.ini 模板文本
pub const EMBEDDED_D3DX_INI_TEMPLATE: &str = r#"; 3Dmigoto for Honkai: Star Rail (SRMI) Configuration File
; Managed by LiquiMod

[Loader]
target = StarRail.exe
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

[Constants]
; Global constants for mod variables
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigotoDownloadProgress {
    pub stage: String, // "downloading" | "extracting" | "completed" | "failed"
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

/// 获取 SRMI 最新的 Release 版本信息（支持 GitHub 直连、代理及国内镜像加速）
pub async fn check_latest_srmi_release(
    github_token: Option<&str>,
    mirror_url: Option<&str>,
) -> Result<MigotoReleaseInfo> {
    let base_api = if let Some(mirror) = mirror_url {
        let clean = mirror.trim_end_matches('/');
        format!(
            "{}/https://api.github.com/repos/SpectrumQT/SRMI-Package/releases/latest",
            clean
        )
    } else {
        "https://api.github.com/repos/SpectrumQT/SRMI-Package/releases/latest".to_string()
    };

    let client = reqwest::Client::builder()
        .user_agent("LiquiMod-Client")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| LiquiModError::Io(std::io::Error::other(e.to_string())))?;

    let mut req = client.get(&base_api);
    if let Some(token) = github_token {
        if !token.trim().is_empty() {
            req = req.header("Authorization", format!("token {}", token.trim()));
        }
    }

    let res = req.send().await.map_err(|e| {
        LiquiModError::Io(std::io::Error::other(format!(
            "检查 3DMigoto 更新失败: {}",
            e
        )))
    })?;

    if !res.status().is_success() {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "GitHub API 响应错误状态码: {}",
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
        asset_name,
        size_bytes,
    })
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

        if rel_name.is_empty() {
            continue;
        }

        // 避免路径穿越
        let clean_path = PathBuf::from(rel_name);
        if clean_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            continue;
        }

        let out_path = target_dir.join(&clean_path);

        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // 如果是 d3dx.ini 且本地已存在，优先保留本地已有 d3dx.ini 或做合并，避免破坏用户已配置的路径
            if out_path.file_name().and_then(|n| n.to_str()) == Some("d3dx.ini")
                && out_path.is_file()
            {
                // 仅解压为 d3dx.ini.upstream，保留用户现存 d3dx.ini
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
