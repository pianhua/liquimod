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

/// Read-only result of comparing the database enabled state with the physical deployment.
///
/// This intentionally does not repair, materialize, or delete anything. It is used by
/// diagnostics surfaces that must be safe to refresh while the user is investigating an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeploymentStatusKind {
    Disabled,
    Deployed,
    Missing,
    Mismatched,
    Unexpected,
    SourceUnavailable,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DeploymentStatus {
    pub entry: ModEntry,
    pub kind: DeploymentStatusKind,
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

    /// Return a read-only, per-Mod deployment inspection.
    ///
    /// Unlike `reconcile`, this method never changes the database or filesystem. In particular,
    /// variant runtime directories are treated as the expected target but are not materialized
    /// during inspection.
    pub fn inspect_status(&self) -> Result<Vec<DeploymentStatus>> {
        let strategy = self.strategy();
        let mut out = Vec::new();
        for entry in self.library.db.list_mods()? {
            let link = self.mods_dir.join(Self::link_name(&entry));
            let kind = match strategy {
                DeployStrategy::CopyFallback if entry.enabled => DeploymentStatusKind::Unsupported,
                DeployStrategy::CopyFallback => {
                    if deployed_path_present(&link) {
                        DeploymentStatusKind::Unexpected
                    } else {
                        DeploymentStatusKind::Disabled
                    }
                }
                DeployStrategy::Junction if !entry.enabled => {
                    if deployed_path_present(&link) {
                        DeploymentStatusKind::Unexpected
                    } else {
                        DeploymentStatusKind::Disabled
                    }
                }
                DeployStrategy::Junction => match self.expected_source_for_status(&entry) {
                    None => DeploymentStatusKind::SourceUnavailable,
                    Some(source) if !source.is_dir() => DeploymentStatusKind::Missing,
                    Some(_source) if !junction::exists(&link).unwrap_or(false) => {
                        if deployed_path_present(&link) {
                            DeploymentStatusKind::Mismatched
                        } else {
                            DeploymentStatusKind::Missing
                        }
                    }
                    Some(source)
                        if junction::get_target(&link)
                            .map(|target| target == source)
                            .unwrap_or(false) =>
                    {
                        DeploymentStatusKind::Deployed
                    }
                    Some(_) => DeploymentStatusKind::Mismatched,
                },
            };
            out.push(DeploymentStatus { entry, kind });
        }
        Ok(out)
    }

    /// Backwards-compatible boolean view of the deployment inspection.
    pub fn status(&self) -> Result<Vec<(ModEntry, bool)>> {
        Ok(self
            .inspect_status()?
            .into_iter()
            .map(|status| {
                let healthy = matches!(
                    status.kind,
                    DeploymentStatusKind::Disabled | DeploymentStatusKind::Deployed
                );
                (status.entry, healthy)
            })
            .collect())
    }

    fn expected_source_for_status(&self, entry: &ModEntry) -> Option<PathBuf> {
        let source = self.library.entry_source_dir(entry).ok()?;
        if variants::detect_variants(&source).is_empty() {
            Some(source)
        } else {
            // Do not materialize here. The runtime directory is the path that `enable` and
            // `refresh` would deploy once the selected variant has been materialized.
            Some(self.library.layout.runtime_mod_dir(entry.id))
        }
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

fn deployed_path_present(path: &Path) -> bool {
    junction::exists(path).unwrap_or(false) || path.exists() || path.join(COPY_MARKER).is_file()
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
    fn inspect_reports_disabled_without_mutating_state() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Disabled")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();

        let statuses = Deployer::new(&lib, &mods_dir).inspect_status().unwrap();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].entry.id, entry.id);
        assert_eq!(statuses[0].kind, DeploymentStatusKind::Disabled);
    }

    #[test]
    fn inspect_reports_missing_enabled_deployment_without_materializing_variant() {
        let (_t, lib, mods_dir) = setup();
        let root = lib.layout.mod_dir("Firefly", "Variant");
        fs::create_dir_all(root.join("Option A")).unwrap();
        fs::write(root.join("mod.ini"), "base").unwrap();
        fs::write(root.join("Option A").join("mod.ini"), "variant").unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        lib.db.set_enabled(entry.id, true).unwrap();

        let runtime = lib.layout.runtime_mod_dir(entry.id);
        assert!(!runtime.exists());
        let statuses = Deployer::new(&lib, &mods_dir).inspect_status().unwrap();
        assert_eq!(statuses[0].kind, DeploymentStatusKind::Missing);
        assert!(!runtime.exists());
    }

    #[test]
    fn inspect_reports_occupied_enabled_path_as_mismatched_without_removing_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Occupied")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        lib.db.set_enabled(entry.id, true).unwrap();

        let link = mods_dir.join(Deployer::link_name(&entry));
        fs::create_dir_all(&link).unwrap();
        let statuses = Deployer::new(&lib, &mods_dir).inspect_status().unwrap();

        assert_eq!(statuses[0].kind, DeploymentStatusKind::Mismatched);
        assert!(link.is_dir());
    }

    #[test]
    fn inspect_reports_offline_external_source_without_touching_source() {
        let (_t, lib, mods_dir) = setup();
        let source_parent = tempfile::tempdir().unwrap();
        let source = source_parent.path().join("OfflineExternal");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("mod.ini"), "external").unwrap();
        let entry = lib
            .add_external_folder(&source, "Firefly", "OfflineExternal")
            .unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();
        fs::remove_dir_all(&source).unwrap();

        let statuses = Deployer::new(&lib, &mods_dir).inspect_status().unwrap();

        assert_eq!(statuses[0].kind, DeploymentStatusKind::SourceUnavailable);
        assert!(!source.exists());
        assert!(!lib.layout.mod_dir("Firefly", "OfflineExternal").exists());
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
