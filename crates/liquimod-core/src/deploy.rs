use crate::d3d;
use crate::error::Result;
use crate::filesystem::{choose_strategy, DeployStrategy};
use crate::library::Library;
use crate::models::ModEntry;
use crate::variants;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const COPY_MARKER: &str = ".liquimod-managed";

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

    pub fn strategy(&self) -> DeployStrategy {
        choose_strategy(&self.library.layout.root, &self.mods_dir)
    }

    pub fn strategy_label(&self) -> &'static str {
        self.strategy().label()
    }

    /// 3Dmigoto Mods 目录中的链接名：角色__Mod名__ID。
    pub fn link_name(entry: &ModEntry) -> String {
        format!("{}__{}__{}", entry.character, entry.name, entry.id)
    }

    fn source_dir(&self, entry: &ModEntry) -> Result<PathBuf> {
        let root = self.library.layout.root.join(&entry.rel_path);
        if !root.is_dir() {
            return Err(crate::error::LiquiModError::ModNotFound(format!(
                "library folder missing: {}",
                root.display()
            )));
        }
        let needs_runtime =
            !variants::detect_variants(&root).is_empty() || d3d::has_ini_variables(&root);
        if !needs_runtime {
            return Ok(root);
        }
        let runtime = self.library.layout.runtime_mod_dir(entry.id);
        variants::materialize(&root, entry.active_variant.as_deref(), &runtime)?;
        d3d::isolate_ini_variables(&runtime, entry.id)?;
        Ok(runtime)
    }

    pub fn enable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if entry.enabled {
            return Ok(());
        }
        let source = self.source_dir(&entry)?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        self.deploy_path(&source, &link)?;
        if let Err(e) = self.library.db.set_enabled(id, true) {
            let _ = Self::remove_deployed_path(&link, self.strategy());
            let _ = self.cleanup_runtime(id);
            return Err(e);
        }
        self.library.db.op_finish(op)
    }

    /// 已启用 Mod 的物理部署刷新，用于切换变体、修复复制漂移或重新生成运行副本。
    pub fn refresh(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if !entry.enabled {
            return Ok(());
        }
        let source = self.source_dir(&entry)?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        let op = self.library.db.op_begin("refresh", &id.to_string())?;
        self.deploy_path(&source, &link)?;
        self.library.db.op_finish(op)
    }

    pub fn disable(&self, id: i64) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        if !entry.enabled {
            if matches!(self.strategy(), DeployStrategy::CopyFallback)
                && link.join(COPY_MARKER).is_file()
            {
                Self::remove_deployed_path(&link, DeployStrategy::CopyFallback)?;
            }
            return Ok(());
        }
        let op = self.library.db.op_begin("disable", &id.to_string())?;
        Self::remove_deployed_path(&link, self.strategy())?;
        self.library.db.set_enabled(id, false)?;
        self.cleanup_runtime(id)?;
        self.library.db.op_finish(op)
    }

    fn deploy_path(&self, source: &Path, link: &Path) -> Result<()> {
        match self.strategy() {
            DeployStrategy::Junction => {
                Self::prepare_link(link)?;
                junction::create(source, link)
                    .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
            }
            DeployStrategy::CopyFallback => {
                Self::prepare_copy_destination(link)?;
                copy_dir_recursive(source, link)?;
                std::fs::write(link.join(COPY_MARKER), b"LiquiMod managed deployment\n")?;
            }
        }
        Ok(())
    }

    fn cleanup_runtime(&self, id: i64) -> Result<()> {
        let runtime = self.library.layout.runtime_mod_dir(id);
        if runtime.exists() {
            std::fs::remove_dir_all(runtime)?;
        }
        Ok(())
    }

    fn remove_deployed_path(link: &Path, strategy: DeployStrategy) -> Result<()> {
        match strategy {
            DeployStrategy::Junction if junction::exists(link).unwrap_or(false) => {
                junction::delete(link)
                    .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
                if link.exists() {
                    std::fs::remove_dir(link)?;
                }
            }
            DeployStrategy::Junction if link.is_dir() && link.join(COPY_MARKER).is_file() => {
                std::fs::remove_dir_all(link)?;
            }
            DeployStrategy::CopyFallback if link.join(COPY_MARKER).is_file() => {
                std::fs::remove_dir_all(link)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// 创建 Junction 前清理链接路径；非空用户目录永不覆盖。
    fn prepare_link(link: &Path) -> Result<()> {
        if junction::exists(link).unwrap_or(false) {
            junction::delete(link)
                .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
            if link.exists() {
                std::fs::remove_dir(link)?;
            }
        } else if link.is_dir() {
            if std::fs::read_dir(link)?.next().is_none() {
                std::fs::remove_dir(link)?;
            } else {
                return Err(crate::error::LiquiModError::Junction(format!(
                    "path occupied by non-empty directory: {}",
                    link.display()
                )));
            }
        }
        Ok(())
    }

    fn prepare_copy_destination(link: &Path) -> Result<()> {
        if junction::exists(link).unwrap_or(false) {
            Self::remove_deployed_path(link, DeployStrategy::Junction)?;
        } else if link.exists() {
            if link.is_dir() && link.join(COPY_MARKER).is_file() {
                std::fs::remove_dir_all(link)?;
            } else {
                return Err(crate::error::LiquiModError::Junction(format!(
                    "path occupied by unmanaged content: {}",
                    link.display()
                )));
            }
        }
        std::fs::create_dir_all(link)?;
        Ok(())
    }

    /// 重新对齐磁盘状态与 DB `enabled`，同时修复变体运行副本和 Copy 漂移。
    pub fn reconcile(&self) -> Result<()> {
        let entries = self.library.db.list_mods()?;
        let strategy = self.strategy();
        let mut enabled_links = HashSet::new();
        for e in &entries {
            let link = self.mods_dir.join(Self::link_name(e));
            if e.enabled {
                enabled_links.insert(Self::link_name(e));
                if self.source_dir(e).is_err() {
                    self.library.db.set_enabled(e.id, false)?;
                    Self::remove_deployed_path(&link, strategy)?;
                    self.cleanup_runtime(e.id)?;
                    continue;
                }
                self.refresh(e.id)?;
            } else {
                Self::remove_deployed_path(&link, strategy)?;
                self.cleanup_runtime(e.id)?;
            }
        }
        if matches!(strategy, DeployStrategy::Junction) {
            self.clean_orphaned_junctions(&enabled_links)?;
        }
        Ok(())
    }

    fn clean_orphaned_junctions(&self, managed_links: &HashSet<String>) -> Result<()> {
        if let Ok(entries) = std::fs::read_dir(&self.mods_dir) {
            for item in entries.flatten() {
                let name = item.file_name().to_string_lossy().into_owned();
                if managed_links.contains(&name) {
                    continue;
                }
                let path = item.path();
                if junction::exists(&path).unwrap_or(false) {
                    if let Ok(target) = junction::get_target(&path) {
                        if target.starts_with(&self.library.layout.root) {
                            Self::remove_deployed_path(&path, DeployStrategy::Junction)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn status(&self) -> Result<Vec<(ModEntry, bool)>> {
        let strategy = self.strategy();
        let mut out = Vec::new();
        for e in self.library.db.list_mods()? {
            let link = self.mods_dir.join(Self::link_name(&e));
            let ok = if e.enabled {
                match strategy {
                    DeployStrategy::Junction => {
                        let source = self.source_dir(&e)?;
                        junction::exists(&link).unwrap_or(false)
                            && junction::get_target(&link)
                                .map(|t| t == source)
                                .unwrap_or(false)
                    }
                    DeployStrategy::CopyFallback => {
                        link.is_dir() && link.join(COPY_MARKER).is_file()
                    }
                }
            } else {
                !junction::exists(&link).unwrap_or(false) && !link.join(COPY_MARKER).is_file()
            };
            out.push((e, ok));
        }
        Ok(out)
    }

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

fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        if ft.is_dir() {
            std::fs::create_dir_all(&to)?;
            copy_dir_recursive(&from, &to)?;
        } else if ft.is_file() {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
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
    fn enable_creates_deployment_and_disable_removes_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Summer")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();
        assert!(
            junction::exists(mods_dir.join(Deployer::link_name(&entry))).unwrap()
                || mods_dir.join(Deployer::link_name(&entry)).is_dir()
        );
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        d.disable(entry.id).unwrap();
        assert!(!mods_dir.join(Deployer::link_name(&entry)).exists());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
    }

    #[test]
    fn variant_enable_materializes_only_selected_content() {
        let (_t, lib, mods_dir) = setup();
        let root = lib.layout.mod_dir("Firefly", "Summer");
        fs::create_dir_all(root.join("Option A")).unwrap();
        fs::create_dir_all(root.join("Option B")).unwrap();
        fs::write(root.join("base.ini"), "base").unwrap();
        fs::write(root.join("Option A").join("choice.ini"), "a").unwrap();
        fs::write(root.join("Option B").join("choice.ini"), "b").unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        assert_eq!(entry.active_variant.as_deref(), Some("Option A"));
        let d = Deployer::new(&lib, &mods_dir);
        d.enable(entry.id).unwrap();
        let runtime = lib.layout.runtime_mod_dir(entry.id);
        assert_eq!(fs::read_to_string(runtime.join("choice.ini")).unwrap(), "a");
        lib.db
            .set_active_variant(entry.id, Some("Option B"))
            .unwrap();
        d.refresh(entry.id).unwrap();
        assert_eq!(fs::read_to_string(runtime.join("choice.ini")).unwrap(), "b");
    }
}
