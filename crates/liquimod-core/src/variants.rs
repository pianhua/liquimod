use crate::error::{LiquiModError, Result};
use crate::paths::is_valid_segment;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModVariant {
    pub name: String,
}

/// 识别 Mod 根目录下的互斥变体目录。
/// 只识别明确的命名约定，避免把 textures、ShaderFixes 等普通资源目录误判为变体。
pub fn detect_variants(root: &Path) -> Vec<ModVariant> {
    let mut variants = std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let ft = entry.file_type().ok()?;
            if !ft.is_dir() || ft.is_symlink() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_variant_name(&name) {
                Some(ModVariant { name })
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    variants.sort_by(|a, b| natural_name_cmp(&a.name, &b.name));
    variants
}

pub fn active_variant_name(root: &Path, requested: Option<&str>) -> Option<String> {
    let variants = detect_variants(root);
    if variants.is_empty() {
        return None;
    }
    requested
        .filter(|name| variants.iter().any(|v| v.name == *name))
        .map(str::to_owned)
        .or_else(|| variants.first().map(|v| v.name.clone()))
}

pub fn is_variant_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() || !is_valid_segment(trimmed) || trimmed.starts_with('.') {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("option ")
        || lower.starts_with("[variant]")
        || lower.starts_with("variant ")
        || (trimmed.len() > 3
            && trimmed.as_bytes()[0].is_ascii_digit()
            && trimmed.as_bytes()[1].is_ascii_digit()
            && matches!(trimmed.as_bytes()[2], b'_' | b'-' | b' '))
}

/// 把基础资源与一个变体合并到运行目录。变体资源覆盖同路径基础资源。
pub fn materialize(root: &Path, variant: Option<&str>, destination: &Path) -> Result<()> {
    let chosen = active_variant_name(root, variant);
    if variant.is_some() && chosen.is_none() {
        return Err(LiquiModError::InvalidName(
            variant.unwrap_or_default().to_string(),
        ));
    }
    if destination.exists() {
        std::fs::remove_dir_all(destination)?;
    }
    std::fs::create_dir_all(destination)?;

    let mut skip_dirs: HashSet<String> =
        detect_variants(root).into_iter().map(|v| v.name).collect();
    if let Ok(relative) = destination.strip_prefix(root) {
        if let Some(std::path::Component::Normal(name)) = relative.components().next() {
            skip_dirs.insert(name.to_string_lossy().into_owned());
        }
    }
    copy_children(root, destination, &skip_dirs)?;
    if let Some(name) = chosen {
        copy_children(&root.join(&name), destination, &HashSet::new())?;
    }
    Ok(())
}

fn copy_children(source: &Path, destination: &Path, skip_dirs: &HashSet<String>) -> Result<()> {
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let from = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let ft = entry.file_type()?;
        if ft.is_symlink() || skip_dirs.contains(&name) || name == ".liquimod-installing" {
            continue;
        }
        let to = destination.join(&name);
        if ft.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_children(&from, &to, &HashSet::new())?;
        } else if ft.is_file() {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn natural_name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let al = a.to_ascii_lowercase();
    let bl = b.to_ascii_lowercase();
    variant_rank(&al)
        .cmp(&variant_rank(&bl))
        .then_with(|| leading_number(&al).cmp(&leading_number(&bl)))
        .then_with(|| al.cmp(&bl))
}

fn variant_rank(value: &str) -> u8 {
    if leading_number(value) != u32::MAX {
        0
    } else if value.starts_with("option ") {
        1
    } else if value.starts_with("[variant]") {
        2
    } else if value.starts_with("variant ") {
        3
    } else {
        4
    }
}

fn leading_number(value: &str) -> u32 {
    let digits = value
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect::<String>();
    digits.parse().unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detects_only_known_variant_names() {
        let root = tempfile::tempdir().unwrap();
        for name in [
            "Option B",
            "Option A",
            "01_RedDress",
            "[Variant] ShortHair",
            "textures",
        ] {
            fs::create_dir(root.path().join(name)).unwrap();
        }
        let names = detect_variants(root.path())
            .into_iter()
            .map(|v| v.name)
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            vec!["01_RedDress", "Option A", "Option B", "[Variant] ShortHair"]
        );
    }

    #[test]
    fn materialize_overlays_variant_over_base() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("mod.ini"), "base").unwrap();
        fs::write(root.path().join("common.bin"), "base-common").unwrap();
        fs::create_dir(root.path().join("Option A")).unwrap();
        fs::write(root.path().join("Option A").join("mod.ini"), "variant").unwrap();
        fs::write(
            root.path().join("Option A").join("variant.bin"),
            "only-variant",
        )
        .unwrap();
        let dest = root.path().join("runtime");
        materialize(root.path(), Some("Option A"), &dest).unwrap();
        assert_eq!(fs::read_to_string(dest.join("mod.ini")).unwrap(), "variant");
        assert_eq!(
            fs::read_to_string(dest.join("common.bin")).unwrap(),
            "base-common"
        );
        assert_eq!(
            fs::read_to_string(dest.join("variant.bin")).unwrap(),
            "only-variant"
        );
        assert!(!dest.join("Option A").exists());
    }
}
