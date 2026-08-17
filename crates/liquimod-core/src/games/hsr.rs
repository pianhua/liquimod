use super::{CharacterInfo, Game};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct RawCharacter {
    #[serde(rename = "InternalName")]
    internal_name: String,
    #[serde(rename = "DisplayName")]
    display_name: String,
    #[serde(rename = "Image")]
    image: String,
}

/// 崩坏：星穹铁道角色清单（数据 vendored 自 JASM 资产，见 assets/hsr/）。
pub struct Hsr {
    characters: Vec<CharacterInfo>,
}

impl Hsr {
    pub fn new() -> Self {
        let raw: Vec<RawCharacter> = serde_json::from_str(include_str!(
            "../../assets/hsr/characters.json"
        ))
        .expect("assets/hsr/characters.json must be valid JSON; vendored data is guarded by tests");
        Self {
            characters: raw
                .into_iter()
                .map(|c| CharacterInfo {
                    internal_name: c.internal_name,
                    display_name: c.display_name,
                    image: c.image,
                })
                .collect(),
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

impl Game for Hsr {
    fn id(&self) -> &'static str {
        "hsr"
    }
    fn characters(&self) -> &[CharacterInfo] {
        &self.characters
    }
}

#[cfg(test)]
mod tests {
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
    fn every_character_image_exists_on_disk() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/hsr/images");
        for c in Hsr::new().characters() {
            assert!(dir.join(&c.image).is_file(), "missing image {}", c.image);
        }
    }

    #[test]
    fn shared_returns_same_instance() {
        assert!(std::ptr::eq(Hsr::shared(), Hsr::shared()));
    }
}
