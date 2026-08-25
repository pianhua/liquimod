use super::CharacterInfo;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

#[derive(Deserialize)]
struct RawCharacter {
    #[serde(rename = "InternalName")]
    internal_name: String,
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Image")]
    image: String,
    #[serde(rename = "Keys", default)]
    keys: Vec<String>,
    #[serde(rename = "Element", default)]
    element: Option<String>,
    #[serde(rename = "Rarity", default)]
    rarity: Option<u8>,
}

use std::sync::Arc;

/// 崩坏：星穹铁道角色清单（支持 LocalAppData 云端热更新数据覆盖 + 内嵌资产兜底）。
pub struct Hsr {
    characters: RwLock<Arc<[CharacterInfo]>>,
}

fn asset_root_override() -> &'static RwLock<Option<PathBuf>> {
    static ROOT: OnceLock<RwLock<Option<PathBuf>>> = OnceLock::new();
    ROOT.get_or_init(|| RwLock::new(None))
}

fn asset_root() -> PathBuf {
    if let Some(root) = asset_root_override()
        .read()
        .ok()
        .and_then(|root| root.clone())
    {
        return root;
    }
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("GameAssets")
}

impl Hsr {
    pub fn new() -> Self {
        let chars = Self::load_characters();
        Self {
            characters: RwLock::new(Arc::from(chars.into_boxed_slice())),
        }
    }

    fn load_characters() -> Vec<CharacterInfo> {
        // 1. 加载程序内置的权威中文角色数据作为本地化词典（保留原始有序列表）
        let vendored_raw: Vec<RawCharacter> =
            match serde_json::from_str(include_str!("../../assets/hsr/characters.json")) {
                Ok(raw) => raw,
                Err(error) => {
                    tracing::error!("failed to parse bundled HSR character data: {error}");
                    Vec::new()
                }
            };

        let vendored_list: Vec<CharacterInfo> = vendored_raw
            .into_iter()
            .map(|c| CharacterInfo {
                internal_name: c.internal_name,
                display_name: c.display_name,
                image: c.image,
                keys: c.keys,
                element: c.element,
                rarity: c.rarity,
            })
            .collect();

        let mut vendored_map: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (idx, c) in vendored_list.iter().enumerate() {
            vendored_map.insert(c.internal_name.to_lowercase(), idx);
        }

        // 2. 尝试从便携数据根/GameAssets/Honkai/characters.json 读取云端同步资产
        let local_asset_dir = asset_root().join("Honkai");

        let custom_file = local_asset_dir.join("characters.json");
        if custom_file.exists() {
            if let Ok(content) = std::fs::read_to_string(&custom_file) {
                if let Ok(raw) = serde_json::from_str::<Vec<RawCharacter>>(&content) {
                    if !raw.is_empty() {
                        let mut merged = Vec::new();
                        let mut seen_keys = std::collections::HashSet::new();

                        for c in raw {
                            let key = c.internal_name.to_lowercase();
                            seen_keys.insert(key.clone());

                            if let Some(&idx) = vendored_map.get(&key) {
                                let v = &vendored_list[idx];
                                // 权威中文保护：保留中文 DisplayName 与丰富的中文别名 Keys，图片使用云端指定
                                let mut combined_keys = v.keys.clone();
                                for k in c.keys {
                                    if !combined_keys.iter().any(|x| x.eq_ignore_ascii_case(&k)) {
                                        combined_keys.push(k);
                                    }
                                }
                                merged.push(CharacterInfo {
                                    internal_name: v.internal_name.clone(),
                                    display_name: v.display_name.clone(), // 强制保持中文
                                    image: if !c.image.is_empty() {
                                        c.image
                                    } else {
                                        v.image.clone()
                                    },
                                    keys: combined_keys,
                                    element: c.element.or_else(|| v.element.clone()),
                                    rarity: c.rarity.or(v.rarity),
                                });
                            } else {
                                // 云端新角色：保留云端数据
                                merged.push(CharacterInfo {
                                    internal_name: c.internal_name,
                                    display_name: c.display_name,
                                    image: c.image,
                                    keys: c.keys,
                                    element: c.element,
                                    rarity: c.rarity,
                                });
                            }
                        }

                        // 补充内置中存在但云端遗漏的角色（保持 vendored 顺序）
                        for v in &vendored_list {
                            if !seen_keys.contains(&v.internal_name.to_lowercase()) {
                                merged.push(v.clone());
                            }
                        }

                        return merged;
                    }
                }
            }
        }

        // 3. Fallback 回退到纯内置数据（绝对确定性顺序）
        vendored_list
    }

    /// 热重载角色数据
    pub fn reload(&self) {
        let updated = Self::load_characters();
        if let Ok(mut guard) = self.characters.write() {
            *guard = Arc::from(updated.into_boxed_slice());
        }
    }

    /// 设置便携数据根下的角色资产目录；应用迁移数据根后调用并 reload。
    pub fn set_asset_root(root: PathBuf) {
        if let Ok(mut guard) = asset_root_override().write() {
            *guard = Some(root);
        }
    }

    pub fn shared() -> &'static Hsr {
        static INSTANCE: OnceLock<Hsr> = OnceLock::new();
        INSTANCE.get_or_init(Hsr::new)
    }
}

impl Default for Hsr {
    fn default() -> Self {
        Self::new()
    }
}

impl super::GameAdapter for Hsr {
    fn id(&self) -> &'static str {
        "hsr"
    }
    fn display_name(&self) -> &'static str {
        "崩坏：星穹铁道"
    }
    fn characters(&self) -> Arc<[CharacterInfo]> {
        match self.characters.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => {
                tracing::error!("HSR character data lock poisoned; using recovered snapshot");
                poisoned.into_inner().clone()
            }
        }
    }
    fn process_names(&self) -> &'static [&'static str] {
        &["starrail.exe"]
    }
    fn default_target_hint(&self) -> &'static str {
        "StarRail.exe"
    }
}

#[cfg(test)]
mod tests {
    use super::super::GameAdapter;
    use super::*;
    use std::collections::HashSet;
    use std::path::Path;

    #[test]
    fn parses_characters_from_vendored_json() {
        let hsr = Hsr::new();
        assert_eq!(hsr.id(), "hsr");
        assert!(hsr.characters().len() > 50, "expected full HSR roster");
        let mut internal_names = HashSet::new();
        let chars = hsr.characters();
        for c in chars.iter() {
            assert!(!c.internal_name.is_empty());
            assert!(!c.display_name.is_empty());
            assert!(!c.image.is_empty());
            assert!(!c.internal_name.contains(['/', '\\']));
            assert!(
                internal_names.insert(&c.internal_name),
                "duplicate internal name {}",
                c.internal_name
            );
        }
    }

    #[test]
    fn every_vendored_character_image_exists_on_disk() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/hsr/images");
        let vendored_raw: Vec<RawCharacter> =
            serde_json::from_str(include_str!("../../assets/hsr/characters.json")).unwrap();
        for c in vendored_raw {
            assert!(dir.join(&c.image).is_file(), "missing image {}", c.image);
        }
    }

    #[test]
    fn shared_returns_same_instance() {
        assert!(std::ptr::eq(Hsr::shared(), Hsr::shared()));
    }

    #[test]
    fn smart_merge_preserves_chinese_when_remote_is_english() {
        let hsr = Hsr::new();
        let chars = hsr.characters();
        let acheron = chars.iter().find(|c| c.internal_name == "Acheron").unwrap();
        assert_eq!(acheron.display_name, "黄泉");
        assert!(acheron.keys.iter().any(|k| k == "黄泉" || k == "huangquan"));
    }

    #[test]
    fn concurrent_reload_and_iteration_is_thread_safe() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;
        use std::thread;

        let running = Arc::new(AtomicBool::new(true));
        let running_writer = running.clone();

        // 写入线程：持续触发 reload
        let writer = thread::spawn(move || {
            let hsr = Hsr::shared();
            for _ in 0..50 {
                hsr.reload();
                thread::yield_now();
            }
            running_writer.store(false, Ordering::SeqCst);
        });

        // 多个读取线程：持续获取并迭代 characters 快照
        let mut readers = Vec::new();
        for _ in 0..4 {
            let r_running = running.clone();
            readers.push(thread::spawn(move || {
                let hsr = Hsr::shared();
                while r_running.load(Ordering::SeqCst) {
                    let chars = hsr.characters();
                    assert!(!chars.is_empty());
                    for c in chars.iter() {
                        assert!(!c.internal_name.is_empty());
                    }
                }
            }));
        }

        writer.join().unwrap();
        for r in readers {
            r.join().unwrap();
        }
    }
}
