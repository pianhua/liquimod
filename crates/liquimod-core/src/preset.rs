//! 预设 = 启用清单快照；应用 = 全量对账重建 Junction。

use crate::deploy::Deployer;
use crate::error::Result;
use crate::library::Library;
use std::path::Path;

/// 当前启用的 mod id 列表（按 id 升序，便于测试断言）。
pub fn snapshot_enabled(lib: &Library) -> Result<Vec<i64>> {
    let mut ids: Vec<i64> = lib
        .list()?
        .into_iter()
        .filter(|m| m.enabled)
        .map(|m| m.id)
        .collect();
    ids.sort_unstable();
    Ok(ids)
}

/// 应用预设：启用清单内、停用清单外。返回 (启用数, 停用数)。清单内不存在的 id 静默忽略。
pub fn apply_preset(lib: &Library, mods_dir: &Path, preset_id: i64) -> Result<(usize, usize)> {
    let want: std::collections::HashSet<i64> =
        lib.db.preset_mod_ids(preset_id)?.into_iter().collect();
    let dep = Deployer::new(lib, mods_dir);
    let (mut enabled, mut disabled) = (0usize, 0usize);
    for m in lib.list()? {
        if want.contains(&m.id) && !m.enabled {
            dep.enable(m.id)?;
            enabled += 1;
        } else if !want.contains(&m.id) && m.enabled {
            dep.disable(m.id)?;
            disabled += 1;
        }
    }
    Ok((enabled, disabled))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;

    fn fixture() -> (tempfile::TempDir, tempfile::TempDir, Library, i64, i64, i64) {
        let lib_dir = tempfile::tempdir().unwrap();
        let mods_dir = tempfile::tempdir().unwrap();
        let lib = Library::init(lib_dir.path()).unwrap();
        let root = lib.layout.mods_root();
        std::fs::create_dir_all(root.join("Asta/m1")).unwrap();
        std::fs::create_dir_all(root.join("Asta/m2")).unwrap();
        std::fs::create_dir_all(root.join("Asta/m3")).unwrap();
        let a = lib.db.upsert_mod("Asta", "m1", "mods/Asta/m1").unwrap();
        let b = lib.db.upsert_mod("Asta", "m2", "mods/Asta/m2").unwrap();
        let c = lib.db.upsert_mod("Asta", "m3", "mods/Asta/m3").unwrap();
        (lib_dir, mods_dir, lib, a, b, c)
    }

    #[test]
    fn snapshot_returns_enabled_ids() {
        let (_l, mdir, lib, a, b, _c) = fixture();
        let dep = Deployer::new(&lib, mdir.path());
        dep.enable(a).unwrap();
        dep.enable(b).unwrap();
        let ids = snapshot_enabled(&lib).unwrap();
        assert_eq!(ids, vec![a, b]);
    }

    #[test]
    fn apply_enables_listed_and_disables_rest() {
        let (_l, mdir, lib, a, b, c) = fixture();
        let dep = Deployer::new(&lib, mdir.path());
        dep.enable(a).unwrap();
        dep.enable(b).unwrap();
        dep.enable(c).unwrap();
        let pid = lib.db.save_preset("p", &[b]).unwrap();
        let (en, dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!((en, dis), (0, 2)); // b 已启用，a/c 被停用
        assert!(!lib.db.get_mod(a).unwrap().enabled);
        assert!(lib.db.get_mod(b).unwrap().enabled);
        assert!(!lib.db.get_mod(c).unwrap().enabled);
        // 幂等：再应用一次零变更
        assert_eq!(apply_preset(&lib, mdir.path(), pid).unwrap(), (0, 0));
    }

    #[test]
    fn apply_ignores_stale_mod_ids() {
        let (_l, mdir, lib, a, _b, _c) = fixture();
        let pid = lib.db.save_preset("p", &[a, 99999]).unwrap();
        let (en, _dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!(en, 1);
    }
}
