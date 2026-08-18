//! Mod 预览图缩略：确定性缓存路径 thumbs/{id}.jpg，源新则重生。

use std::path::{Path, PathBuf};

const IMAGE_EXTS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];
const THUMB_LONG_EDGE: u32 = 384;

fn collect_images(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 1 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if ft.is_dir() && !ft.is_symlink() {
            collect_images(&path, depth + 1, out);
        } else if ft.is_file()
            && path
                .extension()
                .map(|e| {
                    let e = e.to_string_lossy().to_lowercase();
                    IMAGE_EXTS.contains(&e.as_str())
                })
                .unwrap_or(false)
        {
            out.push(path);
        }
    }
}

/// 在 mod 目录内（最深 2 层）找预览图：preview stem 优先，否则字典序第一张。
pub fn find_preview_image(mod_dir: &Path) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    collect_images(mod_dir, 0, &mut candidates);
    candidates.sort();
    candidates
        .iter()
        .find(|p| {
            p.file_stem()
                .map(|s| s.to_string_lossy().eq_ignore_ascii_case("preview"))
                .unwrap_or(false)
        })
        .cloned()
        .or_else(|| candidates.into_iter().next())
}

/// 生成/复用缩略图。无预览图或解码失败返回 Ok(None)。mtime 比较失败时保守重生。
pub fn ensure_thumbnail(
    library_root: &Path,
    mod_dir: &Path,
    mod_id: i64,
) -> crate::error::Result<Option<PathBuf>> {
    let Some(src) = find_preview_image(mod_dir) else {
        return Ok(None);
    };
    let thumb_dir = library_root.join("thumbs");
    let dest = thumb_dir.join(format!("{mod_id}.jpg"));
    let fresh = match (
        std::fs::metadata(&src).and_then(|m| m.modified()),
        std::fs::metadata(&dest).and_then(|m| m.modified()),
    ) {
        (Ok(s), Ok(d)) => d >= s,
        _ => false,
    };
    if fresh {
        return Ok(Some(dest));
    }
    std::fs::create_dir_all(&thumb_dir)?;
    let img = match image::open(&src) {
        Ok(i) => i,
        Err(_) => return Ok(None), // 损坏图片不阻断列表
    };
    let thumb = img.thumbnail(THUMB_LONG_EDGE, THUMB_LONG_EDGE);
    thumb
        .save_with_format(&dest, image::ImageFormat::Jpeg)
        .map_err(std::io::Error::other)?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_preview_stem_first() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.save(dir.path().join("aaa.png")).unwrap();
        img.save(dir.path().join("Preview.PNG")).unwrap();
        let found = find_preview_image(dir.path()).unwrap();
        assert_eq!(found.file_stem().unwrap().to_string_lossy(), "Preview");
    }

    #[test]
    fn falls_back_to_first_image_and_searches_subdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("textures")).unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 255, 0, 255]));
        img.save(dir.path().join("textures/b.png")).unwrap();
        let found = find_preview_image(dir.path()).unwrap();
        assert!(found.ends_with("b.png"));
    }

    #[test]
    fn no_image_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mod.ini"), "x").unwrap();
        assert!(find_preview_image(dir.path()).is_none());
    }

    #[test]
    fn ensure_thumbnail_caches_and_regenerates_when_source_newer() {
        let lib_dir = tempfile::tempdir().unwrap();
        let mod_dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(900, 300, image::Rgba([1, 2, 3, 255]));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t1 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7)
            .unwrap()
            .unwrap();
        assert!(t1.ends_with("7.jpg"));
        let thumb = image::open(&t1).unwrap();
        assert_eq!(thumb.width(), 384);
        let mtime1 = std::fs::metadata(&t1).unwrap().modified().unwrap();
        // 缓存新鲜：直接命中
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let t2 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7)
            .unwrap()
            .unwrap();
        let mtime2 = std::fs::metadata(&t2).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
        // 源更新：重生
        std::thread::sleep(std::time::Duration::from_millis(1100));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t3 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7)
            .unwrap()
            .unwrap();
        let mtime3 = std::fs::metadata(&t3).unwrap().modified().unwrap();
        assert!(mtime3 > mtime2);
    }
}
