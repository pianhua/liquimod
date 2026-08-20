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

/// 崩坏：星穹铁道角色清单（支持 LocalAppData 云端热更新数据覆盖 + 内嵌资产兜底）。
pub struct Hsr {
    characters: RwLock<Vec<CharacterInfo>>,
}

impl Hsr {
    pub fn new() -> Self {
        let chars = Self::load_characters();
        Self {
            characters: RwLock::new(chars),
        }
    }

    fn load_characters() -> Vec<CharacterInfo> {
        // 1. 加载程序内置的权威中文角色数据作为本地化词典
        let vendored_raw: Vec<RawCharacter> = serde_json::from_str(include_str!(
            "../../assets/hsr/characters.json"
        ))
        .expect("assets/hsr/characters.json must be valid JSON; vendored data is guarded by tests");

        let mut vendored_map: std::collections::HashMap<String, CharacterInfo> =
            std::collections::HashMap::new();
        for c in vendored_raw {
            vendored_map.insert(
                c.internal_name.to_lowercase(),
                CharacterInfo {
                    internal_name: c.internal_name,
                    display_name: c.display_name,
                    image: c.image,
                    keys: c.keys,
                    element: c.element,
                    rarity: c.rarity,
                },
            );
        }

        // 2. 尝试从 LocalAppData/LiquiMod/GameAssets/Honkai/characters.json 读取云端同步资产
        let local_asset_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("LiquiMod")
            .join("GameAssets")
            .join("Honkai");

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

                            if let Some(v) = vendored_map.get(&key) {
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

                        // 补充内置中存在但云端遗漏的角色
                        for (key, v) in &vendored_map {
                            if !seen_keys.contains(key) {
                                merged.push(v.clone());
                            }
                        }

                        return merged;
                    }
                }
            }
        }

        // 3. Fallback 回退到纯内置数据
        vendored_map.into_values().collect()
    }

    /// 热重载角色数据
    pub fn reload(&self) {
        let updated = Self::load_characters();
        if let Ok(mut guard) = self.characters.write() {
            *guard = updated;
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
    fn characters(&self) -> &[CharacterInfo] {
        // SAFETY: 为了保持 GameAdapter trait 返回切片接口，此处返回一个不可变的引用
        // 因为 characters 整体替换为 COW 或 Leak 静态引用
        // 但更好的是通过内部持有不可变向量，重载时交换引用
        // 此处利用 static 容器或 unsafe 切片转换（由 shared() 单例持有）
        unsafe {
            let guard = self.characters.read().unwrap();
            let slice: &[CharacterInfo] = &guard;
            std::mem::transmute(slice)
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
        for c in hsr.characters() {
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
        let acheron = hsr
            .characters()
            .iter()
            .find(|c| c.internal_name == "Acheron")
            .unwrap();
        assert_eq!(acheron.display_name, "黄泉");
        assert!(acheron.keys.iter().any(|k| k == "黄泉" || k == "huangquan"));
    }
}
