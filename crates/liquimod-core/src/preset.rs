//! 预设 = 启用清单快照；应用 = 全量对账重建 Junction。

use crate::deploy::Deployer;
use crate::error::{LiquiModError, Result};
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
///
/// 逐项容错：单个 mod 失败（如库目录已消失）不阻塞其余 mod，失败被收集为聚合错误返回；
/// 此时预设可能已被部分应用，重跑 `apply_preset` 或 `Deployer::recover` 会收敛到目标状态。
pub fn apply_preset(lib: &Library, mods_dir: &Path, preset_id: i64) -> Result<(usize, usize)> {
    // 关键校验 (LM-P2-012): 必须先验证 preset 真实存在，防止无效 ID 将全部 Mod 清空禁用
    let presets = lib.db.list_presets()?;
    if !presets.iter().any(|p| p.id == preset_id) {
        return Err(LiquiModError::Io(std::io::Error::other(format!(
            "预设 (ID: {}) 不存在，已阻止错误应用",
            preset_id
        ))));
    }

    let want: std::collections::HashSet<i64> =
        lib.db.preset_mod_ids(preset_id)?.into_iter().collect();
    let dep = Deployer::new(lib, mods_dir);
    let (mut enabled, mut disabled) = (0usize, 0usize);
    let mut failures: Vec<String> = Vec::new();
    for m in lib.list()? {
        let outcome = if want.contains(&m.id) && !m.enabled {
            dep.enable(m.id).map(|()| enabled += 1)
        } else if !want.contains(&m.id) && m.enabled {
            dep.disable(m.id).map(|()| disabled += 1)
        } else {
            Ok(())
        };
        if let Err(e) = outcome {
            failures.push(format!("{} ({e})", m.name));
        }
    }
    if failures.is_empty() {
        Ok((enabled, disabled))
    } else {
        Err(LiquiModError::Io(std::io::Error::other(format!(
            "部分 Mod 应用失败：{}",
            failures.join("；")
        ))))
    }
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
        assert!(!junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(a).unwrap()))
        )
        .unwrap());
        assert!(junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(b).unwrap()))
        )
        .unwrap());
        assert!(!junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(c).unwrap()))
        )
        .unwrap());
        // 幂等：再应用一次零变更
        assert_eq!(apply_preset(&lib, mdir.path(), pid).unwrap(), (0, 0));
    }

    #[test]
    fn apply_mixed_enable_and_disable_in_one_pass() {
        let (_l, mdir, lib, a, b, c) = fixture();
        let dep = Deployer::new(&lib, mdir.path());
        dep.enable(b).unwrap();
        dep.enable(c).unwrap();
        let pid = lib.db.save_preset("p", &[a, b]).unwrap();
        let (en, dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!((en, dis), (1, 1)); // a 启用、c 停用，b 已就位
        assert!(lib.db.get_mod(a).unwrap().enabled);
        assert!(lib.db.get_mod(b).unwrap().enabled);
        assert!(!lib.db.get_mod(c).unwrap().enabled);
        assert!(junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(a).unwrap()))
        )
        .unwrap());
        assert!(junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(b).unwrap()))
        )
        .unwrap());
        assert!(!junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(c).unwrap()))
        )
        .unwrap());
    }

    #[test]
    fn apply_ignores_stale_mod_ids() {
        let (_l, mdir, lib, a, _b, _c) = fixture();
        let pid = lib.db.save_preset("p", &[a, 99999]).unwrap();
        let (en, _dis) = apply_preset(&lib, mdir.path(), pid).unwrap();
        assert_eq!(en, 1);
        assert!(junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(a).unwrap()))
        )
        .unwrap());
    }

    #[test]
    fn apply_dead_folder_reports_and_applies_rest() {
        let (_l, mdir, lib, a, b, _c) = fixture();
        let pid = lib.db.save_preset("p", &[a, b]).unwrap();
        std::fs::remove_dir_all(lib.layout.mods_root().join("Asta/m1")).unwrap();
        let msg = apply_preset(&lib, mdir.path(), pid)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("m1"), "got: {msg}");
        // a 失败不阻塞 b：b 的 junction 已建、DB 已置位
        assert!(lib.db.get_mod(b).unwrap().enabled);
        assert!(junction::exists(
            mdir.path()
                .join(Deployer::link_name(&lib.db.get_mod(b).unwrap()))
        )
        .unwrap());
        assert!(!lib.db.get_mod(a).unwrap().enabled);
    }
}
