//! Mod 预览图缩略：确定性缓存路径 thumbs/{id}.jpg，源新则重生。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const IMAGE_EXTS: [&str; 7] = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "avif"];
const THUMB_LONG_EDGE: u32 = 384;

fn collect_images(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    if depth > 4 {
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

/// 在 mod 目录内找预览图：若指定了 custom_cover 且文件存在直接使用，
/// 否则 preview stem 优先，再次字典序第一张。
pub fn find_preview_image(mod_dir: &Path, custom_cover: Option<&str>) -> Option<PathBuf> {
    if let Some(rel) = custom_cover {
        let custom_path = mod_dir.join(rel);
        if custom_path.is_file() {
            return Some(custom_path);
        }
    }

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
    custom_cover: Option<&str>,
) -> crate::error::Result<Option<PathBuf>> {
    let Some(src) = find_preview_image(mod_dir, custom_cover) else {
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
    let tmp = thumb_dir.join(format!("{mod_id}.jpg.{}.tmp", uuid::Uuid::new_v4()));
    {
        let file = std::fs::File::create(&tmp)?;
        let writer = std::io::BufWriter::new(file);
        let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, 80);
        thumb
            .to_rgb8()
            .write_with_encoder(encoder)
            .map_err(std::io::Error::other)?;
    }
    std::fs::rename(&tmp, &dest)?;
    Ok(Some(dest))
}

/// 删除缩略图缓存（mod 被移除时调用，防止 rowid 复用后串图）。不存在则静默。
pub fn remove_thumbnail(library_root: &Path, mod_id: i64) {
    let dest = library_root.join("thumbs").join(format!("{mod_id}.jpg"));
    match std::fs::remove_file(&dest) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => {}
    }
}

/// 回收孤儿缩略图：`thumbs/` 里揭示了已不存在的 mod id 的缓存一律删除。
/// 幂等、静默——无缓存目录或文件被占用时跳过，不影响主流程。
pub fn gc_thumbnails(library_root: &Path, valid_ids: &HashSet<i64>) {
    let thumb_dir = library_root.join("thumbs");
    let Ok(entries) = std::fs::read_dir(&thumb_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        // 只清理 "{id}.jpg"；生成中的 "{id}.jpg.{uuid}.tmp" 无法解析为裸 id，天然跳过
        let Some(id) = stem.parse::<i64>().ok() else {
            continue;
        };
        if !valid_ids.contains(&id) {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_thumbnail_deletes_cache_and_tolerates_missing() {
        let lib_dir = tempfile::tempdir().unwrap();
        let mod_dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([9, 9, 9, 255]));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 3, None)
            .unwrap()
            .unwrap();
        assert!(t.exists());
        remove_thumbnail(lib_dir.path(), 3);
        assert!(!t.exists());
        remove_thumbnail(lib_dir.path(), 3); // 幂等
    }

    #[test]
    fn gc_removes_orphan_thumbs_keeps_valid_and_temp() {
        let lib_dir = tempfile::tempdir().unwrap();
        let thumb_dir = lib_dir.path().join("thumbs");
        std::fs::create_dir_all(&thumb_dir).unwrap();
        // 已有缩略图：1.jpg（有效）、2.jpg（孤儿）、非缩略图文件不该动
        std::fs::write(thumb_dir.join("1.jpg"), b"a").unwrap();
        std::fs::write(thumb_dir.join("2.jpg"), b"b").unwrap();
        std::fs::write(thumb_dir.join("3.jpg.abc.tmp"), b"c").unwrap();
        std::fs::write(thumb_dir.join("note.txt"), b"d").unwrap();

        let valid = [1i64].into_iter().collect();
        gc_thumbnails(lib_dir.path(), &valid);

        assert!(thumb_dir.join("1.jpg").exists()); // 有效保留
        assert!(!thumb_dir.join("2.jpg").exists()); // 孤儿删除
        assert!(thumb_dir.join("3.jpg.abc.tmp").exists()); // tmp 跳过
        assert!(thumb_dir.join("note.txt").exists()); // 非缩略图不动
                                                      // 无 thumbs 目录时静默
        gc_thumbnails(&lib_dir.path().join("nope"), &valid);
    }

    #[test]
    fn finds_preview_stem_first() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.save(dir.path().join("aaa.png")).unwrap();
        img.save(dir.path().join("Preview.PNG")).unwrap();
        let found = find_preview_image(dir.path(), None).unwrap();
        assert_eq!(found.file_stem().unwrap().to_string_lossy(), "Preview");
    }

    #[test]
    fn custom_cover_takes_precedence_over_preview() {
        let dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([255, 0, 0, 255]));
        img.save(dir.path().join("preview.png")).unwrap();
        img.save(dir.path().join("my_custom.png")).unwrap();
        let found = find_preview_image(dir.path(), Some("my_custom.png")).unwrap();
        assert_eq!(found.file_stem().unwrap().to_string_lossy(), "my_custom");
    }

    #[test]
    fn falls_back_to_first_image_and_searches_subdir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("textures")).unwrap();
        let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([0, 255, 0, 255]));
        img.save(dir.path().join("textures/b.png")).unwrap();
        let found = find_preview_image(dir.path(), None).unwrap();
        assert!(found.ends_with("b.png"));
    }

    #[test]
    fn no_image_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("mod.ini"), "x").unwrap();
        assert!(find_preview_image(dir.path(), None).is_none());
    }

    #[test]
    fn ensure_thumbnail_caches_and_regenerates_when_source_newer() {
        let lib_dir = tempfile::tempdir().unwrap();
        let mod_dir = tempfile::tempdir().unwrap();
        let img = image::RgbaImage::from_pixel(900, 300, image::Rgba([1, 2, 3, 255]));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t1 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7, None)
            .unwrap()
            .unwrap();
        assert!(t1.ends_with("7.jpg"));
        let thumb = image::open(&t1).unwrap();
        assert_eq!(thumb.width(), 384);
        let mtime1 = std::fs::metadata(&t1).unwrap().modified().unwrap();
        // 缓存新鲜：直接命中
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let t2 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7, None)
            .unwrap()
            .unwrap();
        let mtime2 = std::fs::metadata(&t2).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
        // 源更新：重生
        std::thread::sleep(std::time::Duration::from_millis(1100));
        img.save(mod_dir.path().join("preview.png")).unwrap();
        let t3 = ensure_thumbnail(lib_dir.path(), mod_dir.path(), 7, None)
            .unwrap()
            .unwrap();
        let mtime3 = std::fs::metadata(&t3).unwrap().modified().unwrap();
        assert!(mtime3 > mtime2);
    }
}
