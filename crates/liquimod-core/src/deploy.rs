use crate::error::Result;
use crate::library::Library;
use crate::models::ModEntry;
use std::path::{Path, PathBuf};

pub struct Deployer<'a> {
    pub library: &'a Library,
    pub mods_dir: PathBuf,
}

impl<'a> Deployer<'a> {
    pub fn new(library: &'a Library, mods_dir: &Path) -> Self {
        Self {
            library,
            mods_dir: mods_dir.to_path_buf(),
        }
    }

    /// 3Dmigoto Mods 目录中的链接名：角色--Mod名（确定性，避免跨角色重名冲突）
    pub fn link_name(entry: &ModEntry) -> String {
        format!("{}--{}", entry.character, entry.name)
    }

    pub fn enable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if entry.enabled {
            return Ok(());
        }
        let target = self.library.layout.root.join(&entry.rel_path);
        if !target.is_dir() {
            return Err(crate::error::LiquiModError::ModNotFound(format!(
                "library folder missing: {}",
                target.display()
            )));
        }
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        Self::prepare_link(&link)?;
        junction::create(&target, &link)
            .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        self.library.db.set_enabled(id, true)?;
        self.library.db.op_finish(op)
    }

    /// 删除 junction 并清理 junction crate 残留的空目录。
    fn remove_junction(link: &Path) -> Result<()> {
        junction::delete(link).map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        if link.exists() {
            std::fs::remove_dir(link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        Ok(())
    }

    /// 创建 junction 前清理链接路径：已有 junction 拆旧重建（自愈悬空/错指），
    /// 非 junction 的空目录移除，非空则报错（绝不碰用户数据）。
    fn prepare_link(link: &Path) -> Result<()> {
        // 用 junction::exists 而非 Path::exists：后者会穿透 junction，悬空链接会被误判为不存在
        if junction::exists(link).unwrap_or(false) {
            Self::remove_junction(link)?;
        } else if link.symlink_metadata().is_ok() {
            std::fs::remove_dir(link).map_err(|e| {
                crate::error::LiquiModError::Junction(format!(
                    "path occupied by non-junction entry: {} ({e})",
                    link.display()
                ))
            })?;
        }
        Ok(())
    }

    pub fn disable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if !entry.enabled {
            return Ok(());
        }
        let op = self.library.db.op_begin("disable", &id.to_string())?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        if junction::exists(&link).unwrap_or(false) {
            Self::remove_junction(&link)?;
        } else if link.exists() {
            std::fs::remove_dir(&link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        }
        self.library.db.set_enabled(id, false)?;
        self.library.db.op_finish(op)
    }

    /// 让 Mods 目录与数据库启用状态一致。
    /// 安全规则：只碰指向本仓库的 junction；用户自己放的目录/文件一律不动。
    pub fn reconcile(&self) -> Result<()> {
        let entries = self.library.db.list_mods()?;
        let mut managed_links: Vec<String> = Vec::new();
        for e in &entries {
            let link = self.mods_dir.join(Self::link_name(e));
            managed_links.push(Self::link_name(e));
            let exists = junction::exists(&link).unwrap_or(false);
            if e.enabled && !exists {
                let target = self.library.layout.root.join(&e.rel_path);
                if !target.is_dir() {
                    continue; // 库目录已消失：留给 scan 对账，跳过不报错
                }
                Self::prepare_link(&link)?;
                junction::create(&target, &link)
                    .map_err(|err| crate::error::LiquiModError::Junction(err.to_string()))?;
            } else if !e.enabled && exists {
                Self::remove_junction(&link)?;
            }
        }
        // 清理指向本仓库、但数据库已无记录的孤儿 junction
        if self.mods_dir.is_dir() {
            for item in std::fs::read_dir(&self.mods_dir)? {
                let item = item?;
                let name = item.file_name().to_string_lossy().into_owned();
                if managed_links.contains(&name) {
                    continue;
                }
                let path = item.path();
                if junction::exists(&path).unwrap_or(false) {
                    if let Ok(target) = junction::get_target(&path) {
                        if target.starts_with(&self.library.layout.root) {
                            Self::remove_junction(&path)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// 返回每个受管 mod 与其部署状态是否一致。
    pub fn status(&self) -> Result<Vec<(ModEntry, bool)>> {
        let mut out = Vec::new();
        for e in self.library.db.list_mods()? {
            let link = self.mods_dir.join(Self::link_name(&e));
            let actual = junction::exists(&link).unwrap_or(false);
            out.push((e.clone(), actual == e.enabled));
        }
        Ok(out)
    }

    /// 启动时调用：存在未完成的操作日志 → 全量对账（操作均幂等），然后结清日志。
    pub fn recover(&self) -> Result<()> {
        let pending = self.library.db.pending_ops()?;
        if pending.is_empty() {
            return Ok(());
        }
        self.reconcile()?;
        for (op_id, _, _) in pending {
            self.library.db.op_finish(op_id)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Library, std::path::PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::init(&tmp.path().join("lib")).unwrap();
        let mods_dir = tmp.path().join("GameMods");
        fs::create_dir_all(&mods_dir).unwrap();
        (tmp, lib, mods_dir)
    }

    #[test]
    fn enable_creates_junction_disable_removes_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();

        let link = mods_dir.join(Deployer::link_name(&entry));
        assert!(junction::exists(&link).unwrap());
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);

        d.disable(entry.id).unwrap();
        assert!(!link.exists());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);

        d.disable(entry.id).unwrap();
    }

    #[test]
    fn enable_heals_stale_empty_dir_from_crashed_disable() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let d = Deployer::new(&lib, &mods_dir);

        // 模拟 disable 崩溃残留：junction 已删但空目录还在，且 DB 仍是 enabled=false
        fs::create_dir_all(mods_dir.join(Deployer::link_name(&entry))).unwrap();

        d.enable(entry.id).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        assert!(junction::exists(&link).unwrap());
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
    }

    #[test]
    fn enable_rejects_missing_library_folder() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        fs::remove_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        assert!(d.enable(entry.id).is_err());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
    }

    #[test]
    fn reconcile_fixes_drift_and_ignores_foreign_content() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        fs::create_dir_all(lib.layout.mod_dir("Acheron", "Black")).unwrap();
        let mods = lib.scan().unwrap();
        let firefly = mods
            .iter()
            .find(|m| m.character == "Firefly")
            .unwrap()
            .clone();
        let acheron = mods
            .iter()
            .find(|m| m.character == "Acheron")
            .unwrap()
            .clone();

        let d = Deployer::new(&lib, &mods_dir);
        d.enable(firefly.id).unwrap();
        d.enable(acheron.id).unwrap();

        junction::delete(mods_dir.join(Deployer::link_name(&acheron))).unwrap();
        fs::remove_dir(mods_dir.join(Deployer::link_name(&acheron))).unwrap();
        fs::create_dir_all(mods_dir.join("MyOwnMod")).unwrap();
        fs::write(mods_dir.join("readme.txt"), b"hi").unwrap();

        d.reconcile().unwrap();

        assert!(junction::exists(mods_dir.join(Deployer::link_name(&acheron))).unwrap());
        assert!(mods_dir.join("MyOwnMod").is_dir());
        assert!(mods_dir.join("readme.txt").is_file());

        let st = d.status().unwrap();
        assert!(st.iter().all(|(_, ok)| *ok));
    }

    #[test]
    fn status_detects_missing_junction() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();
        junction::delete(mods_dir.join(Deployer::link_name(&entry))).unwrap();
        fs::remove_dir(mods_dir.join(Deployer::link_name(&entry))).unwrap();
        let st = d.status().unwrap();
        assert_eq!(st.len(), 1);
        assert!(!st[0].1);
    }

    #[test]
    fn recover_completes_pending_ops() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        lib.db.op_begin("enable", &entry.id.to_string()).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        d.recover().unwrap();

        assert!(lib.db.pending_ops().unwrap().is_empty());
        assert!(junction::exists(mods_dir.join(Deployer::link_name(&entry))).unwrap());
    }

    #[test]
    fn recover_heals_crashed_disable_leftover() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        // 模拟 disable 在 junction::delete 之后、remove_dir 之前崩溃：
        // DB 仍是 enabled=true、op 未结清、Mods 里留空目录
        lib.db.set_enabled(entry.id, true).unwrap();
        lib.db.op_begin("disable", &entry.id.to_string()).unwrap();
        fs::create_dir_all(mods_dir.join(Deployer::link_name(&entry))).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        d.recover().unwrap();

        assert!(junction::exists(mods_dir.join(Deployer::link_name(&entry))).unwrap());
        assert!(lib.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn reconcile_removes_only_orphans_pointing_into_library() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();

        // 孤儿 junction：指向本仓库，但数据库无记录
        let orphan = mods_dir.join("Ghost--Mod");
        junction::create(lib.layout.mod_dir("Firefly", "Summer"), &orphan).unwrap();

        // 用户自己的 junction：指向库外
        let outside = _t.path().join("elsewhere");
        fs::create_dir_all(&outside).unwrap();
        let foreign = mods_dir.join("UserLink");
        junction::create(&outside, &foreign).unwrap();

        let d = Deployer::new(&lib, &mods_dir);
        d.reconcile().unwrap();

        assert!(!orphan.exists());
        assert!(junction::exists(&foreign).unwrap());
    }
}
