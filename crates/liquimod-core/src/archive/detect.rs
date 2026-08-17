use std::io::Read;
use std::path::Path;

use crate::error::{LiquiModError, Result};

pub enum ArchiveFormat {
    Zip,
    SevenZ,
    Rar,
}

pub fn detect_format(path: &Path) -> Result<ArchiveFormat> {
    let mut buf = [0u8; 8];
    let mut file = std::fs::File::open(path)?;
    let n = file.read(&mut buf)?;
    let head = &buf[..n];

    if head.starts_with(&[0x50, 0x4B, 0x03, 0x04])
        || head.starts_with(&[0x50, 0x4B, 0x05, 0x06])
        || head.starts_with(&[0x50, 0x4B, 0x07, 0x08])
    {
        return Ok(ArchiveFormat::Zip);
    }
    if head.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Ok(ArchiveFormat::SevenZ);
    }
    if head.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00])
        || head.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00])
    {
        return Ok(ArchiveFormat::Rar);
    }
    Err(LiquiModError::UnsupportedArchive(path.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(name: &str, bytes: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(bytes).unwrap();
        (dir, path)
    }

    #[test]
    fn detects_zip_by_magic() {
        for magic in [&[0x50, 0x4B, 0x03, 0x04][..], &[0x50, 0x4B, 0x05, 0x06][..], &[0x50, 0x4B, 0x07, 0x08][..]] {
            let (_dir, path) = write_file("a.bin", magic);
            assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::Zip));
        }
    }

    #[test]
    fn detects_7z_by_magic() {
        let (_dir, path) = write_file("a.bin", &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]);
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::SevenZ));
    }

    #[test]
    fn detects_rar4_and_rar5() {
        let (_dir, path) = write_file("a.bin", &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00]);
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::Rar));
        let (_dir, path) = write_file("a.bin", &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00]);
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::Rar));
    }

    #[test]
    fn unknown_bytes_are_unsupported() {
        let (_dir, path) = write_file("a.bin", &[0xDE, 0xAD, 0xBE, 0xEF]);
        assert!(matches!(
            detect_format(&path),
            Err(crate::error::LiquiModError::UnsupportedArchive(_))
        ));
    }

    #[test]
    fn empty_file_is_unsupported() {
        let (_dir, path) = write_file("a.bin", &[]);
        assert!(matches!(
            detect_format(&path),
            Err(crate::error::LiquiModError::UnsupportedArchive(_))
        ));
    }

    #[test]
    fn extension_is_not_trusted() {
        let (_dir, path) = write_file("x.zip", &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]);
        assert!(matches!(detect_format(&path).unwrap(), ArchiveFormat::SevenZ));
    }
}
