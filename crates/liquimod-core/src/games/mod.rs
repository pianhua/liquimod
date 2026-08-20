use std::fs;
use std::path::Path;

pub mod hsr;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct CharacterInfo {
    pub internal_name: String,
    pub display_name: String,
    pub image: String,
    #[serde(default)]
    pub keys: Vec<String>,
    pub element: Option<String>,
    pub rarity: Option<u8>,
}

use std::sync::Arc;

pub trait GameAdapter: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn characters(&self) -> Arc<[CharacterInfo]>;
    /// 游戏主进程可执行文件名（小写，含 .exe）。
    fn process_names(&self) -> &'static [&'static str];
    /// 默认目标程序特征名
    fn default_target_hint(&self) -> &'static str;
}

pub use GameAdapter as Game;

pub struct GameRegistry {
    adapters: Vec<Box<dyn GameAdapter>>,
}

impl GameRegistry {
    pub fn global() -> &'static GameRegistry {
        static REGISTRY: std::sync::OnceLock<GameRegistry> = std::sync::OnceLock::new();
        REGISTRY.get_or_init(|| Self {
            adapters: vec![Box::new(hsr::Hsr::new())],
        })
    }

    pub fn list(&self) -> Vec<(&'static str, &'static str)> {
        self.adapters
            .iter()
            .map(|a| (a.id(), a.display_name()))
            .collect()
    }

    pub fn get(&self, id: &str) -> Option<&(dyn GameAdapter + 'static)> {
        self.adapters
            .iter()
            .find(|a| a.id() == id)
            .map(|b| b.as_ref())
    }

    pub fn default_game(&self) -> &(dyn GameAdapter + 'static) {
        self.adapters[0].as_ref()
    }
}

const MAX_DEPTH: usize = 8;
const MAX_FILE_BYTES: u64 = 256 * 1024;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// 从解压目录内容推断角色：合并 ini/txt/json 文本与全部文件名作为语料，
/// 统计每个角色（内部名 / 显示名 / 关键字 / 立绘文件名stem）的小写命中次数，取最高者。
pub fn infer_character(dir: &Path, game: &dyn Game) -> Option<String> {
    let mut corpus = String::new();
    let mut budget = MAX_TOTAL_BYTES;
    collect_text(dir, 0, &mut budget, &mut corpus);
    let mut best: Option<(usize, &str)> = None;
    let chars = game.characters();
    for c in chars.iter() {
        let mut score = 0usize;
        let stem = Path::new(&c.image)
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        let mut candidates = vec![
            c.internal_name.to_lowercase(),
            c.display_name.to_lowercase(),
            stem,
        ];
        for k in &c.keys {
            candidates.push(k.to_lowercase());
        }

        for needle in candidates {
            if needle.chars().count() < 2 {
                continue;
            }
            score += corpus.matches(&needle).count();
        }
        if score > 0 && best.is_none_or(|(best_score, _)| score > best_score) {
            best = Some((score, c.internal_name.as_str()));
        }
    }
    best.map(|(_, name)| name.to_owned())
}

fn collect_text(dir: &Path, depth: usize, budget: &mut usize, out: &mut String) {
    if depth > MAX_DEPTH || *budget == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if *budget == 0 {
            break;
        }
        out.push_str(&entry.file_name().to_string_lossy().to_lowercase());
        out.push('\n');
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            collect_text(&path, depth + 1, budget, out);
        } else if meta.is_file()
            && meta.len() <= MAX_FILE_BYTES
            && matches!(
                path.extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    .as_deref(),
                Some("ini" | "txt" | "json")
            )
        {
            if let Ok(bytes) = fs::read(&path) {
                *budget = budget.saturating_sub(bytes.len());
                out.push_str(&String::from_utf8_lossy(&bytes).to_lowercase());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_game() -> &'static crate::games::hsr::Hsr {
        crate::games::hsr::Hsr::shared()
    }

    #[test]
    fn infers_character_from_ini_mentions() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("mod.ini"),
            "[Constants]\n; Firefly costume swap\nglobal $firefly = 1\n",
        )
        .unwrap();
        assert_eq!(
            infer_character(tmp.path(), fixture_game()),
            Some("Firefly".to_string())
        );
    }

    #[test]
    fn infers_from_folder_names_when_no_ini_match() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("Acheron_HD_Textures");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("readme.txt"), b"no hints here").unwrap();
        assert_eq!(
            infer_character(tmp.path(), fixture_game()),
            Some("Acheron".to_string())
        );
    }

    #[test]
    fn returns_none_without_hints() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("mod.ini"), "[Constants]\nglobal $x = 1\n").unwrap();
        assert_eq!(infer_character(tmp.path(), fixture_game()), None);
    }

    #[test]
    fn most_mentioned_character_wins() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("mod.ini"),
            "; acheron acheron acheron\n; blade\n",
        )
        .unwrap();
        assert_eq!(
            infer_character(tmp.path(), fixture_game()),
            Some("Acheron".to_string())
        );
    }

    #[test]
    fn tie_prefers_earlier_character_in_game_order() {
        let tmp = tempfile::tempdir().unwrap();
        let chars = fixture_game().characters();
        let first = &chars[0].internal_name;
        let second = &chars[1].internal_name;
        std::fs::write(
            tmp.path().join("mod.ini"),
            format!("; {} {}\n", second.to_lowercase(), first.to_lowercase()),
        )
        .unwrap();
        assert_eq!(
            infer_character(tmp.path(), fixture_game()),
            Some(first.clone())
        );
    }

    #[test]
    fn game_registry_lists_and_retrieves_adapters() {
        let reg = GameRegistry::global();
        let list = reg.list();
        assert!(!list.is_empty());
        assert_eq!(list[0].0, "hsr");

        let hsr = reg.get("hsr").unwrap();
        assert_eq!(hsr.display_name(), "崩坏：星穹铁道");
        assert!(!hsr.characters().is_empty());
    }
}
