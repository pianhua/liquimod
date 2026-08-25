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

    fn source_dir_with_runtime_reuse(
        &self,
        entry: &ModEntry,
        reuse_existing_runtime: bool,
    ) -> Result<PathBuf> {
        let root = self.library.entry_source_dir(entry)?;
        let has_variants = !variants::detect_variants(&root).is_empty();
        if !has_variants {
            return Ok(root);
        }
        let runtime = self.library.layout.runtime_mod_dir(entry.id);
        if reuse_existing_runtime && runtime.is_dir() {
            return Ok(runtime);
        }
        variants::materialize(&root, entry.active_variant.as_deref(), &runtime)?;
        Ok(runtime)
    }

    fn source_dir(&self, entry: &ModEntry) -> Result<PathBuf> {
        self.source_dir_with_runtime_reuse(entry, false)
    }

    pub fn enable(&self, id: i64) -> Result<()> {
        self.enable_with_runtime_reuse(id, false)
    }

    /// 游戏运行期重新启用：复用先前保留的运行副本，避免删除仍被 3Dmigoto 引用的文件。
    pub fn enable_reusing_runtime(&self, id: i64) -> Result<()> {
        self.enable_with_runtime_reuse(id, true)
    }

    fn enable_with_runtime_reuse(&self, id: i64, reuse_existing_runtime: bool) -> Result<()> {
        let entry = self.library.db.get_mod(id)?;
        if entry.enabled {
            return Ok(());
        }
        let runtime_existed = self.library.layout.runtime_mod_dir(id).is_dir();
        let source = self.source_dir_with_runtime_reuse(&entry, reuse_existing_runtime)?;
        let link = self.mods_dir.join(Self::link_name(&entry));
        let op = self.library.db.op_begin("enable", &id.to_string())?;
        self.deploy_path(&source, &link)?;
        if let Err(e) = self.library.db.set_enabled(id, true) {
            let _ = Self::remove_deployed_path(&link, self.strategy());
            if !(reuse_existing_runtime && runtime_existed) {
                let _ = self.cleanup_runtime(id);
            }
            return Err(e);
        }
        self.library.db.op_finish(op)
    }

    /// 已启用 Mod 的物理部署刷新，用于切换变体、清理旧复制部署残留或重新生成运行副本。
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
        self.disable_with_runtime_cleanup(id, true)
    }

    /// 游戏运行期禁用：只拆部署入口，运行副本延迟到游戏退出后清理。
    pub fn disable_preserving_runtime(&self, id: i64) -> Result<()> {
        self.disable_with_runtime_cleanup(id, false)
    }

    fn disable_with_runtime_cleanup(&self, id: i64, cleanup_runtime: bool) -> Result<()> {
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
        if cleanup_runtime {
            self.cleanup_runtime(id)?;
        }
        self.library.db.op_finish(op)
    }

    fn deploy_path(&self, source: &Path, link: &Path) -> Result<()> {
        match self.strategy() {
            DeployStrategy::Junction => {
                Self::prepare_link(link)?;
                junction::create(source, link)
                    .map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
            }
            DeployStrategy::CopyFallback => return Err(Self::junction_required_error()),
        }
        Ok(())
    }

    fn junction_required_error() -> crate::error::LiquiModError {
        crate::error::LiquiModError::Junction(
            "LiquiMod 只使用 3DMigoto Junction，不再创建 Mod 的复制副本；请将应用数据根与 3DMigoto Mods 放在同一 NTFS/ReFS 卷后重试"
                .to_string(),
        )
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

    /// 重新对齐磁盘状态与 DB `enabled`，同时修复变体运行副本并清理旧复制部署残留。
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

    #[test]
    fn live_disable_preserves_and_reuses_variant_runtime() {
        let (_t, lib, mods_dir) = setup();
        let root = lib.layout.mod_dir("Firefly", "LiveSwap");
        fs::create_dir_all(root.join("Option A")).unwrap();
        fs::write(root.join("Option A").join("choice.ini"), "a").unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let deployer = Deployer::new(&lib, &mods_dir);
        deployer.enable(entry.id).unwrap();
        let runtime = lib.layout.runtime_mod_dir(entry.id);
        fs::write(runtime.join("runtime-sentinel.txt"), "keep").unwrap();

        deployer.disable_preserving_runtime(entry.id).unwrap();
        assert!(runtime.join("runtime-sentinel.txt").is_file());
        deployer.enable_reusing_runtime(entry.id).unwrap();
        assert!(runtime.join("runtime-sentinel.txt").is_file());
    }
}
