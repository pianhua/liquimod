use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use xxhash_rust::xxh3::xxh3_64;

pub const MANIFEST_RAW_URL: &str =
    "https://raw.githubusercontent.com/Moonholder/JASM-GameAssets/main/manifest.json";
pub const JSDELIVR_BASE_URL: &str = "https://cdn.jsdelivr.net/gh/Moonholder/JASM-GameAssets@main/";
pub const RAW_BASE_URL: &str = "https://raw.githubusercontent.com/Moonholder/JASM-GameAssets/main/";

const MAX_PARALLEL_DOWNLOADS: usize = 6;
const MAX_RETRIES: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetFileEntry {
    #[serde(alias = "Path", alias = "path")]
    pub path: String,
    #[serde(alias = "Hash", alias = "hash")]
    pub hash: String,
    #[serde(alias = "Size", alias = "size", default)]
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AssetManifest {
    #[serde(alias = "Version", alias = "version")]
    pub version: String,
    #[serde(alias = "Files", alias = "files")]
    pub files: Vec<AssetFileEntry>,
}

#[derive(Debug, Clone)]
pub struct ManifestDiff {
    pub files_to_download: Vec<AssetFileEntry>,
    pub files_to_delete: Vec<String>,
}

impl ManifestDiff {
    pub fn has_changes(&self) -> bool {
        !self.files_to_download.is_empty() || !self.files_to_delete.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSyncProgress {
    pub stage: String, // "checking" | "downloading" | "cleaning" | "completed" | "failed"
    pub percent: u32,
    pub current_file: Option<String>,
    pub downloaded_count: usize,
    pub total_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetSyncResult {
    pub success: bool,
    pub message: String,
    pub version: String,
    pub downloaded_count: usize,
    pub deleted_count: usize,
}

#[derive(Debug, Clone)]
pub struct MirrorInfo {
    pub address: &'static str,
    pub node_name: &'static str,
}

pub static DEFAULT_MIRRORS: &[MirrorInfo] = &[
    MirrorInfo {
        address: "",
        node_name: "GitHub Direct",
    },
    MirrorInfo {
        address: "https://gh-proxy.com/",
        node_name: "高速镜像 1",
    },
    MirrorInfo {
        address: "https://ghproxy.net/",
        node_name: "高速镜像 2",
    },
    MirrorInfo {
        address: "https://wget.la/",
        node_name: "高速镜像 3",
    },
    MirrorInfo {
        address: "https://gh.jix.de5.net/",
        node_name: "高速镜像 4",
    },
    MirrorInfo {
        address: "https://dl.jix.de5.net/",
        node_name: "高速镜像 5",
    },
];

pub struct AssetSyncService {
    client: reqwest::Client,
    asset_root: PathBuf,
}

impl Default for AssetSyncService {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetSyncService {
    pub fn new() -> Self {
        let asset_root = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
            .join("GameAssets");
        Self::with_root(asset_root)
    }

    pub fn with_root(asset_root: PathBuf) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("LiquiMod-GameAssetSync/1.0")
            .build()
            .unwrap_or_default();
        Self { client, asset_root }
    }

    pub fn asset_root(&self) -> &Path {
        &self.asset_root
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.asset_root.join("manifest.json")
    }

    /// 获取本地当前安装的资产版本号
    pub async fn get_local_version(&self) -> Option<String> {
        self.read_local_manifest().await.map(|m| m.version)
    }

    /// 读取本地清单
    pub async fn read_local_manifest(&self) -> Option<AssetManifest> {
        let path = self.manifest_path();
        let content = tokio::fs::read_to_string(&path).await.ok()?;
        serde_json::from_str(&content).ok()
    }

    /// 写入本地清单
    pub async fn write_local_manifest(
        &self,
        manifest: &AssetManifest,
    ) -> Result<(), std::io::Error> {
        tokio::fs::create_dir_all(&self.asset_root).await?;
        let json = serde_json::to_string_pretty(manifest)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        tokio::fs::write(self.manifest_path(), json).await
    }

    /// 测速并按延迟返回可用镜像列表
    pub async fn get_available_mirrors(&self) -> Vec<MirrorInfo> {
        let mut tasks = Vec::new();
        for mirror in DEFAULT_MIRRORS {
            let mirror = mirror.clone();
            let client = self.client.clone();
            let test_url = if mirror.address.is_empty() {
                MANIFEST_RAW_URL.to_string()
            } else {
                format!("{}{}", mirror.address, MANIFEST_RAW_URL)
            };
            tasks.push(tokio::spawn(async move {
                let start = Instant::now();
                let res = tokio::time::timeout(
                    Duration::from_millis(4000),
                    client.get(&test_url).header("Range", "bytes=0-100").send(),
                )
                .await;
                match res {
                    Ok(Ok(resp)) if resp.status().is_success() || resp.status().as_u16() == 206 => {
                        Some((mirror, start.elapsed()))
                    }
                    _ => None,
                }
            }));
        }

        let mut available = Vec::new();
        for task in tasks {
            if let Ok(Some((m, lat))) = task.await {
                available.push((m, lat));
            }
        }

        // 按延迟从小到大排序
        available.sort_by_key(|(_, lat)| *lat);
        let mut result: Vec<MirrorInfo> = available.into_iter().map(|(m, _)| m).collect();
        if !result.iter().any(|m| m.address.is_empty()) {
            result.push(MirrorInfo {
                address: "",
                node_name: "GitHub Direct",
            });
        }
        result
    }

    /// 下载远端最新清单（通过快速镜像重试）
    pub async fn download_remote_manifest(&self, mirrors: &[MirrorInfo]) -> Option<AssetManifest> {
        for mirror in mirrors {
            let url = if mirror.address.is_empty() {
                MANIFEST_RAW_URL.to_string()
            } else {
                format!("{}{}", mirror.address, MANIFEST_RAW_URL)
            };
            if let Ok(resp) = self.client.get(&url).send().await {
                if resp.status().is_success() {
                    if let Ok(text) = resp.text().await {
                        if let Ok(manifest) = serde_json::from_str::<AssetManifest>(&text) {
                            if !manifest.files.is_empty() {
                                return Some(manifest);
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// 检查是否有更新
    pub async fn check_for_updates(
        &self,
        game_filter: Option<&str>,
    ) -> Result<Option<String>, String> {
        let mirrors = self.get_available_mirrors().await;
        let remote = self
            .download_remote_manifest(&mirrors)
            .await
            .ok_or_else(|| "无法连接到数据更新服务器或镜像节点".to_string())?;

        let local = self.read_local_manifest().await;
        if let Some(loc) = local {
            if loc.version == remote.version {
                let diff = self.compute_diff(Some(&loc), &remote, game_filter).await;
                if !diff.has_changes() {
                    return Ok(None);
                }
            }
        }
        Ok(Some(remote.version))
    }

    /// 计算本地与远端的增量差异
    pub async fn compute_diff(
        &self,
        local: Option<&AssetManifest>,
        remote: &AssetManifest,
        game_filter: Option<&str>,
    ) -> ManifestDiff {
        let filter_prefix = game_filter.map(|g| format!("{}/", g.to_lowercase()));

        let mut local_map = HashMap::new();
        if let Some(loc) = local {
            for f in &loc.files {
                let key = f.path.to_lowercase();
                if let Some(prefix) = &filter_prefix {
                    if !key.starts_with(prefix) {
                        continue;
                    }
                }
                local_map.insert(key, f);
            }
        }

        let mut files_to_download = Vec::new();
        let mut remote_paths = HashSet::new();

        for r_file in &remote.files {
            let key = r_file.path.to_lowercase();
            if let Some(prefix) = &filter_prefix {
                if !key.starts_with(prefix) {
                    continue;
                }
            }

            // 安全防御 (LM-P1-001): 严禁路径穿越组件逃逸出资源根目录
            let Ok(target_path) =
                crate::safe_path::ensure_contained(&self.asset_root, Path::new(&r_file.path))
            else {
                continue;
            };

            remote_paths.insert(key.clone());

            if !target_path.exists() {
                files_to_download.push(r_file.clone());
            } else if let Some(l_file) = local_map.get(&key) {
                if !l_file.hash.eq_ignore_ascii_case(&r_file.hash) {
                    files_to_download.push(r_file.clone());
                } else {
                    // 哈希在清单中一致，但校验磁盘实际文件
                    if let Ok(disk_hash) = Self::compute_file_hash(&target_path).await {
                        if !disk_hash.eq_ignore_ascii_case(&r_file.hash) {
                            files_to_download.push(r_file.clone());
                        }
                    } else {
                        files_to_download.push(r_file.clone());
                    }
                }
            } else {
                files_to_download.push(r_file.clone());
            }
        }

        let mut files_to_delete = Vec::new();
        if let Some(loc) = local {
            for f in &loc.files {
                let key = f.path.to_lowercase();
                if let Some(prefix) = &filter_prefix {
                    if !key.starts_with(prefix) {
                        continue;
                    }
                }
                // 安全防御：确保待删除相对路径也是合法的相对路径
                if crate::safe_path::sanitize_relative_path(Path::new(&f.path)).is_ok()
                    && !remote_paths.contains(&key)
                {
                    files_to_delete.push(f.path.clone());
                }
            }
        }

        ManifestDiff {
            files_to_download,
            files_to_delete,
        }
    }

    /// 计算文件的 xxh3 64-bit 哈希（小写 16 进制字符串）
    pub async fn compute_file_hash(path: &Path) -> Result<String, std::io::Error> {
        let bytes = tokio::fs::read(path).await?;
        let h = xxh3_64(&bytes);
        Ok(format!("{:016x}", h))
    }

    /// 执行完整增量同步
    pub async fn sync(
        &self,
        game_filter: Option<&str>,
        progress_tx: Option<mpsc::Sender<AssetSyncProgress>>,
    ) -> Result<AssetSyncResult, String> {
        let send_progress = |p: AssetSyncProgress| {
            if let Some(tx) = &progress_tx {
                let _ = tx.try_send(p);
            }
        };

        send_progress(AssetSyncProgress {
            stage: "checking".to_string(),
            percent: 5,
            current_file: None,
            downloaded_count: 0,
            total_count: 0,
            message: "正在连接云端并测速最优镜像节点...".to_string(),
        });

        let mirrors = self.get_available_mirrors().await;
        let remote = self
            .download_remote_manifest(&mirrors)
            .await
            .ok_or_else(|| "无法获取远端数据清单，请检查网络连接".to_string())?;

        let local = self.read_local_manifest().await;
        let diff = self
            .compute_diff(local.as_ref(), &remote, game_filter)
            .await;

        if !diff.has_changes() {
            let _ = self.write_local_manifest(&remote).await;
            send_progress(AssetSyncProgress {
                stage: "completed".to_string(),
                percent: 100,
                current_file: None,
                downloaded_count: 0,
                total_count: 0,
                message: "当前游戏数据已是最新版本".to_string(),
            });
            return Ok(AssetSyncResult {
                success: true,
                message: "已是最新版本".to_string(),
                version: remote.version,
                downloaded_count: 0,
                deleted_count: 0,
            });
        }

        let total_downloads = diff.files_to_download.len();
        let downloaded_counter = Arc::new(AtomicUsize::new(0));

        send_progress(AssetSyncProgress {
            stage: "downloading".to_string(),
            percent: 10,
            current_file: None,
            downloaded_count: 0,
            total_count: total_downloads,
            message: format!("检测到 {} 个文件需更新，开始下载...", total_downloads),
        });

        // 并发下载差异文件
        let client = self.client.clone();
        let asset_root = self.asset_root.clone();
        let mirrors = Arc::new(mirrors);

        let stream = futures_util::stream::iter(diff.files_to_download.clone())
            .map(|entry| {
                let client = client.clone();
                let asset_root = asset_root.clone();
                let mirrors = mirrors.clone();
                let downloaded_counter = downloaded_counter.clone();
                let progress_tx = progress_tx.clone();

                async move {
                    let res =
                        Self::download_single_file(&client, &asset_root, &entry, &mirrors).await;
                    let count = downloaded_counter.fetch_add(1, Ordering::SeqCst) + 1;
                    let percent = 10 + (count * 80 / total_downloads.max(1)) as u32;

                    if let Some(tx) = &progress_tx {
                        let _ = tx.try_send(AssetSyncProgress {
                            stage: "downloading".to_string(),
                            percent,
                            current_file: Some(entry.path.clone()),
                            downloaded_count: count,
                            total_count: total_downloads,
                            message: format!(
                                "正在下载 ({}/{}): {}",
                                count, total_downloads, entry.path
                            ),
                        });
                    }
                    res
                }
            })
            .buffer_unordered(MAX_PARALLEL_DOWNLOADS);

        let results: Vec<Result<(), String>> = stream.collect().await;
        let failed_errors: Vec<String> = results.into_iter().filter_map(|r| r.err()).collect();

        if !failed_errors.is_empty() {
            return Err(format!(
                "资产同步未全部成功完成 ({} 个文件失败)，已中止写入清单以防数据损坏: {}",
                failed_errors.len(),
                failed_errors[0]
            ));
        }

        let successful_downloads = total_downloads;

        // 清理孤儿文件 (仅在下载全胜后执行)
        let mut deleted_count = 0;
        if !diff.files_to_delete.is_empty() {
            send_progress(AssetSyncProgress {
                stage: "cleaning".to_string(),
                percent: 95,
                current_file: None,
                downloaded_count: successful_downloads,
                total_count: total_downloads,
                message: "正在清理废弃文件...".to_string(),
            });
            for del_rel in &diff.files_to_delete {
                if let Ok(del_path) =
                    crate::safe_path::ensure_contained(&self.asset_root, Path::new(del_rel))
                {
                    if del_path.exists() && tokio::fs::remove_file(&del_path).await.is_ok() {
                        deleted_count += 1;
                    }
                }
            }
        }

        // 全部无误，原子写入最新清单
        self.write_local_manifest(&remote)
            .await
            .map_err(|e| format!("写入本地资产清单失败: {}", e))?;

        send_progress(AssetSyncProgress {
            stage: "completed".to_string(),
            percent: 100,
            current_file: None,
            downloaded_count: successful_downloads,
            total_count: total_downloads,
            message: format!("同步完成：已更新 {} 个文件", successful_downloads),
        });

        Ok(AssetSyncResult {
            success: true,
            message: format!("同步完成，已更新 {} 个文件", successful_downloads),
            version: remote.version,
            downloaded_count: successful_downloads,
            deleted_count,
        })
    }

    /// 下载单个文件并进行原子校验与写入
    async fn download_single_file(
        client: &reqwest::Client,
        asset_root: &Path,
        entry: &AssetFileEntry,
        mirrors: &[MirrorInfo],
    ) -> Result<(), String> {
        let target_path = crate::safe_path::ensure_contained(asset_root, Path::new(&entry.path))
            .map_err(|e| format!("目标路径不安全: {}", e))?;
        let tmp_path = asset_root.join(format!("{}.tmp", entry.path));

        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }

        // 策略 1: jsDelivr CDN
        let cdn_url = format!("{}{}", JSDELIVR_BASE_URL, entry.path);
        if Self::fetch_and_verify(client, &cdn_url, &tmp_path, &target_path, &entry.hash)
            .await
            .is_ok()
        {
            return Ok(());
        }

        // 策略 2: 镜像重试
        for mirror in mirrors {
            let url = if mirror.address.is_empty() {
                format!("{}{}", RAW_BASE_URL, entry.path)
            } else {
                format!("{}{}{}", mirror.address, RAW_BASE_URL, entry.path)
            };
            if Self::fetch_and_verify(client, &url, &tmp_path, &target_path, &entry.hash)
                .await
                .is_ok()
            {
                return Ok(());
            }
        }

        let _ = tokio::fs::remove_file(&tmp_path).await;
        Err(format!("文件下载或哈希校验失败: {}", entry.path))
    }

    async fn fetch_and_verify(
        client: &reqwest::Client,
        url: &str,
        tmp_path: &Path,
        target_path: &Path,
        expected_hash: &str,
    ) -> Result<(), String> {
        for _ in 0..MAX_RETRIES {
            if let Ok(resp) = client.get(url).send().await {
                if resp.status().is_success() {
                    if let Ok(bytes) = resp.bytes().await {
                        let h = xxh3_64(&bytes);
                        let hex = format!("{:016x}", h);
                        if hex.eq_ignore_ascii_case(expected_hash)
                            && tokio::fs::write(tmp_path, &bytes).await.is_ok()
                            && tokio::fs::rename(tmp_path, target_path).await.is_ok()
                        {
                            return Ok(());
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        Err("Fetch or hash verification failed".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn diff_detects_new_and_modified_files() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AssetSyncService::with_root(temp_dir.path().to_path_buf());

        let remote = AssetManifest {
            version: "2026.08.19".to_string(),
            files: vec![
                AssetFileEntry {
                    path: "Honkai/characters.json".to_string(),
                    hash: "abcdef0123456789".to_string(),
                    size: 100,
                },
                AssetFileEntry {
                    path: "Honkai/images/new_char.png".to_string(),
                    hash: "1122334455667788".to_string(),
                    size: 200,
                },
            ],
        };

        let diff = service.compute_diff(None, &remote, None).await;
        assert_eq!(diff.files_to_download.len(), 2);
        assert_eq!(diff.files_to_delete.len(), 0);
        assert!(diff.has_changes());

        // 测试游戏过滤
        let diff_hsr = service.compute_diff(None, &remote, Some("Honkai")).await;
        assert_eq!(diff_hsr.files_to_download.len(), 2);

        let diff_genshin = service.compute_diff(None, &remote, Some("Genshin")).await;
        assert_eq!(diff_genshin.files_to_download.len(), 0);
    }

    #[tokio::test]
    async fn hash_computation_works() {
        let temp_file = tempfile::NamedTempFile::new().unwrap();
        tokio::fs::write(temp_file.path(), b"hello liquimod")
            .await
            .unwrap();

        let hash = AssetSyncService::compute_file_hash(temp_file.path())
            .await
            .unwrap();
        assert_eq!(hash.len(), 16);
    }

    #[tokio::test]
    async fn write_and_read_local_manifest() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AssetSyncService::with_root(temp_dir.path().to_path_buf());

        let manifest = AssetManifest {
            version: "v2026.08.19".to_string(),
            files: vec![AssetFileEntry {
                path: "Honkai/characters.json".to_string(),
                hash: "abcdef0123456789".to_string(),
                size: 50,
            }],
        };

        service.write_local_manifest(&manifest).await.unwrap();
        let read = service.read_local_manifest().await.unwrap();
        assert_eq!(read.version, "v2026.08.19");
        assert_eq!(read.files.len(), 1);
    }

    #[tokio::test]
    async fn diff_detects_orphans_to_delete() {
        let temp_dir = tempfile::tempdir().unwrap();
        let service = AssetSyncService::with_root(temp_dir.path().to_path_buf());

        let local = AssetManifest {
            version: "v1".to_string(),
            files: vec![
                AssetFileEntry {
                    path: "Honkai/characters.json".to_string(),
                    hash: "aaa".to_string(),
                    size: 10,
                },
                AssetFileEntry {
                    path: "Honkai/images/old_orphan.png".to_string(),
                    hash: "bbb".to_string(),
                    size: 20,
                },
            ],
        };

        let remote = AssetManifest {
            version: "v2".to_string(),
            files: vec![AssetFileEntry {
                path: "Honkai/characters.json".to_string(),
                hash: "aaa".to_string(),
                size: 10,
            }],
        };

        let diff = service
            .compute_diff(Some(&local), &remote, Some("Honkai"))
            .await;
        assert_eq!(diff.files_to_delete.len(), 1);
        assert_eq!(diff.files_to_delete[0], "Honkai/images/old_orphan.png");
    }

    #[test]
    fn parses_real_sample_manifest_json() {
        let sample = r#"{"version":"2026.08.08","files":[{"path":"Honkai/characters.json","hash":"bda694639b76b8b6","size":31938}]}"#;
        let manifest: AssetManifest = serde_json::from_str(sample).unwrap();
        assert_eq!(manifest.version, "2026.08.08");
        assert_eq!(manifest.files.len(), 1);
        assert_eq!(manifest.files[0].path, "Honkai/characters.json");
        assert_eq!(manifest.files[0].hash, "bda694639b76b8b6");
        assert_eq!(manifest.files[0].size, 31938);
    }
}
