use crate::error::Result;
use crate::filesystem::{choose_strategy, DeployStrategy};
use crate::library::Library;
use crate::models::{ModEntry, ModStorageKind};
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
        self.deploy_path(&entry, &source, &link)?;
        if let Err(e) = self.library.db.set_enabled(id, true) {
            let _ = self.remove_deployed_path(&entry, &link, self.strategy());
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
        self.deploy_path(&entry, &source, &link)?;
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
                self.remove_deployed_path(&entry, &link, DeployStrategy::CopyFallback)?;
            }
            return Ok(());
        }
        let op = self.library.db.op_begin("disable", &id.to_string())?;
        self.remove_deployed_path(&entry, &link, self.strategy())?;
        self.library.db.set_enabled(id, false)?;
        if cleanup_runtime {
            self.cleanup_runtime(id)?;
        }
        self.library.db.op_finish(op)
    }

    fn deploy_path(&self, entry: &ModEntry, source: &Path, link: &Path) -> Result<()> {
        match self.strategy() {
            DeployStrategy::Junction => {
                self.prepare_link(entry, link)?;
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

    fn remove_deployed_path(
        &self,
        entry: &ModEntry,
        link: &Path,
        strategy: DeployStrategy,
    ) -> Result<()> {
        match strategy {
            DeployStrategy::Junction => {
                if Self::deployment_link_target(link)?.is_some() {
                    self.ensure_expected_junction_target(entry, link)?;
                    Self::delete_junction(link)?;
                } else if link.exists() {
                    if legacy_copy_path(link) {
                        std::fs::remove_dir_all(link)?;
                    } else {
                        return Err(unexpected_deployment_path_error(link));
                    }
                }
            }
            DeployStrategy::CopyFallback if legacy_copy_path(link) => {
                std::fs::remove_dir_all(link)?;
            }
            DeployStrategy::CopyFallback if link.exists() => {
                return Err(unexpected_deployment_path_error(link));
            }
            _ => {}
        }
        Ok(())
    }

    /// 创建 Junction 前只移除确认属于当前 Mod 的部署链接，绝不覆盖普通目录或未知链接。
    fn prepare_link(&self, entry: &ModEntry, link: &Path) -> Result<()> {
        if Self::deployment_link_target(link)?.is_some() {
            self.ensure_expected_junction_target(entry, link)?;
            Self::delete_junction(link)?;
        } else if link.exists() {
            return Err(unexpected_deployment_path_error(link));
        }
        Ok(())
    }

    fn ensure_expected_junction_target(&self, entry: &ModEntry, link: &Path) -> Result<()> {
        let target = Self::deployment_link_target(link)?.ok_or_else(|| {
            crate::error::LiquiModError::Junction(format!(
                "unable to inspect deployment link at {}",
                link.display()
            ))
        })?;
        for expected in self.deployment_targets(entry)? {
            if paths_equivalent(&target, &expected)? {
                return Ok(());
            }
        }
        Err(crate::error::LiquiModError::Junction(format!(
            "refusing to remove unexpected Junction at {}: target {} is not managed for Mod #{} ({}/{})",
            link.display(),
            target.display(),
            entry.id,
            entry.character,
            entry.name
        )))
    }

    fn deployment_targets(&self, entry: &ModEntry) -> Result<Vec<PathBuf>> {
        let source = match entry.storage_kind {
            ModStorageKind::Managed => {
                let source = self.library.layout.root.join(&entry.rel_path);
                if !path_is_within(&source, &self.library.layout.mods_root())? {
                    return Err(crate::error::LiquiModError::Junction(format!(
                        "managed Mod path escapes the LiquiMod library: {}",
                        source.display()
                    )));
                }
                source
            }
            ModStorageKind::External => {
                let source = entry
                    .source_path
                    .as_deref()
                    .map(PathBuf::from)
                    .ok_or_else(|| {
                        crate::error::LiquiModError::Junction(format!(
                            "external Mod #{} has no saved source path",
                            entry.id
                        ))
                    })?;
                if !source.is_absolute() {
                    return Err(crate::error::LiquiModError::Junction(format!(
                        "external Mod #{} has a non-absolute source path: {}",
                        entry.id,
                        source.display()
                    )));
                }
                source
            }
        };
        Ok(vec![source, self.library.layout.runtime_mod_dir(entry.id)])
    }

    fn junction_exists(link: &Path) -> Result<bool> {
        match junction::exists(link) {
            Ok(exists) => Ok(exists),
            // junction::exists asks Windows for reparse metadata. Normal files and directories
            // report ERROR_NOT_A_REPARSE_POINT rather than `Ok(false)`.
            Err(error) if error.raw_os_error() == Some(4390) => Ok(false),
            Err(error) => Err(crate::error::LiquiModError::Junction(format!(
                "unable to inspect Junction {}: {error}",
                link.display()
            ))),
        }
    }

    /// Return a Junction target even when the target itself has disappeared.
    ///
    /// `junction::exists` treats a dangling Junction as absent, while Windows still exposes its
    /// reparse metadata through `symlink_metadata` and `read_link`. We only delete it after the
    /// returned target is proven to belong to the current Mod (or a managed orphan root).
    fn deployment_link_target(link: &Path) -> Result<Option<PathBuf>> {
        if Self::junction_exists(link)? {
            return junction::get_target(link)
                .map(Some)
                .map_err(|error| crate::error::LiquiModError::Junction(error.to_string()));
        }

        let metadata = match std::fs::symlink_metadata(link) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !metadata.file_type().is_symlink() {
            return Ok(None);
        }

        let target = std::fs::read_link(link)?;
        Ok(Some(if target.is_absolute() {
            target
        } else {
            link.parent().unwrap_or_else(|| Path::new(".")).join(target)
        }))
    }

    fn delete_junction(link: &Path) -> Result<()> {
        junction::delete(link).map_err(|e| crate::error::LiquiModError::Junction(e.to_string()))?;
        if link.exists() {
            std::fs::remove_dir(link)?;
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
                self.refresh(e.id)?;
            } else {
                self.remove_deployed_path(e, &link, strategy)?;
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
                if let Some(target) = Self::deployment_link_target(&path)? {
                    if path_is_within(&target, &self.library.layout.mods_root())?
                        || path_is_within(&target, &self.library.layout.runtime_root())?
                    {
                        Self::delete_junction(&path)?;
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

fn legacy_copy_path(path: &Path) -> bool {
    path.is_dir() && path.join(COPY_MARKER).is_file()
}

fn unexpected_deployment_path_error(path: &Path) -> crate::error::LiquiModError {
    crate::error::LiquiModError::Junction(format!(
        "refusing to remove unexpected deployment path: {}",
        path.display()
    ))
}

fn normalized_path(path: &Path) -> Result<PathBuf> {
    match path.canonicalize() {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(std::path::absolute(path)?)
        }
        Err(error) => Err(error.into()),
    }
}

fn paths_equivalent(left: &Path, right: &Path) -> Result<bool> {
    let left = normalized_path(left)?;
    let right = normalized_path(right)?;
    #[cfg(windows)]
    {
        Ok(left
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy()))
    }
    #[cfg(not(windows))]
    {
        Ok(left == right)
    }
}

fn path_is_within(path: &Path, root: &Path) -> Result<bool> {
    let path = normalized_path(path)?;
    let root = normalized_path(root)?;
    let mut path_components = path.components();
    for root_component in root.components() {
        let Some(path_component) = path_components.next() else {
            return Ok(false);
        };
        if !path_components_equivalent(path_component.as_os_str(), root_component.as_os_str()) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(windows)]
fn path_components_equivalent(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    left.to_string_lossy()
        .eq_ignore_ascii_case(&right.to_string_lossy())
}

#[cfg(not(windows))]
fn path_components_equivalent(left: &std::ffi::OsStr, right: &std::ffi::OsStr) -> bool {
    left == right
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
    fn path_is_within_rejects_a_nonexistent_path_that_lexically_escapes_the_root() {
        let temp = tempfile::tempdir().unwrap();
        let managed_root = temp.path().join("Library").join("mods");
        fs::create_dir_all(&managed_root).unwrap();
        let escaped_path = managed_root.join("..").join("outside");

        assert!(!path_is_within(&escaped_path, &managed_root).unwrap());
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

    #[test]
    fn enable_refuses_to_replace_an_empty_unmanaged_directory() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Occupied")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let link = mods_dir.join(Deployer::link_name(&entry));
        fs::create_dir_all(&link).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .enable(entry.id)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("unexpected deployment path"),
            "unexpected error: {err}"
        );
        assert!(link.is_dir());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(lib.db.pending_ops().unwrap().len(), 1);
    }

    #[test]
    fn disable_rejects_an_unexpected_junction_without_deleting_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Unexpected")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let unexpected_root = tempfile::tempdir().unwrap();
        let unexpected_target = unexpected_root.path().join("not-liquimod");
        fs::create_dir_all(&unexpected_target).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::create(&unexpected_target, &link).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .disable(entry.id)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected Junction"));
        assert!(junction::exists(&link).unwrap());
        assert_eq!(junction::get_target(&link).unwrap(), unexpected_target);
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(lib.db.pending_ops().unwrap().len(), 1);
    }

    #[test]
    fn refresh_rejects_an_unexpected_junction_without_replacing_it() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "UnexpectedRefresh")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let unexpected_root = tempfile::tempdir().unwrap();
        let unexpected_target = unexpected_root.path().join("not-liquimod");
        fs::create_dir_all(&unexpected_target).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::create(&unexpected_target, &link).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .refresh(entry.id)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected Junction"));
        assert!(junction::exists(&link).unwrap());
        assert_eq!(junction::get_target(&link).unwrap(), unexpected_target);
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(lib.db.pending_ops().unwrap().len(), 1);
    }

    #[test]
    fn recover_rebuilds_a_missing_enabled_deployment_and_finishes_pending_operation() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "Recovery")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        lib.db.set_enabled(entry.id, true).unwrap();
        lib.db.op_begin("refresh", &entry.id.to_string()).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));

        Deployer::new(&lib, &mods_dir).recover().unwrap();

        assert!(junction::exists(&link).unwrap());
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert!(lib.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn recover_leaves_pending_operation_when_an_unexpected_junction_needs_manual_resolution() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "RecoveryBlocked")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let unexpected_root = tempfile::tempdir().unwrap();
        let unexpected_target = unexpected_root.path().join("not-liquimod");
        fs::create_dir_all(&unexpected_target).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::create(&unexpected_target, &link).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();
        lib.db.op_begin("refresh", &entry.id.to_string()).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .recover()
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected Junction"));
        assert!(junction::exists(&link).unwrap());
        assert_eq!(junction::get_target(&link).unwrap(), unexpected_target);
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(lib.db.pending_ops().unwrap().len(), 2);
    }

    #[test]
    fn recover_removes_the_expected_disabled_deployment_and_finishes_pending_operation() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "RecoveryDisable")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let deployer = Deployer::new(&lib, &mods_dir);
        deployer.enable(entry.id).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        lib.db.set_enabled(entry.id, false).unwrap();
        lib.db.op_begin("disable", &entry.id.to_string()).unwrap();

        deployer.recover().unwrap();

        assert!(!link.exists());
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
        assert!(lib.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn recover_preserves_enabled_external_mod_when_its_source_is_unavailable() {
        let (_t, lib, mods_dir) = setup();
        let source_parent = tempfile::tempdir().unwrap();
        let source = source_parent.path().join("OfflineRecovery");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("mod.ini"), "external").unwrap();
        let entry = lib
            .add_external_folder(&source, "Firefly", "OfflineRecovery")
            .unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();
        let op = lib.db.op_begin("refresh", &entry.id.to_string()).unwrap();
        fs::remove_dir_all(&source).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .recover()
            .unwrap_err()
            .to_string();

        assert!(err.contains("external source unavailable"));
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(
            lib.db.pending_ops().unwrap(),
            vec![(op, "refresh".to_string(), entry.id.to_string())]
        );
        assert!(!mods_dir.join(Deployer::link_name(&entry)).exists());
    }

    #[test]
    fn manual_resolution_allows_recovery_to_finish_a_blocked_refresh() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "RecoveryResolved")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let unexpected_root = tempfile::tempdir().unwrap();
        let unexpected_target = unexpected_root.path().join("not-liquimod");
        fs::create_dir_all(&unexpected_target).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::create(&unexpected_target, &link).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();
        lib.db.op_begin("refresh", &entry.id.to_string()).unwrap();
        let deployer = Deployer::new(&lib, &mods_dir);

        assert!(deployer.recover().is_err());
        junction::delete(&link).unwrap();
        fs::remove_dir(&link).unwrap();

        deployer.recover().unwrap();

        assert!(junction::exists(&link).unwrap());
        assert_eq!(
            junction::get_target(&link).unwrap(),
            lib.layout.mod_dir("Firefly", "RecoveryResolved")
        );
        assert!(lib.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn external_mod_enable_and_disable_never_modify_its_source_directory() {
        let (_t, lib, mods_dir) = setup();
        let source_parent = tempfile::tempdir().unwrap();
        let source = source_parent.path().join("ExternalSource");
        fs::create_dir_all(&source).unwrap();
        let sentinel = source.join("sentinel.ini");
        fs::write(&sentinel, "keep this source unchanged").unwrap();
        let entry = lib
            .add_external_folder(&source, "Firefly", "ExternalSource")
            .unwrap();
        let deployer = Deployer::new(&lib, &mods_dir);
        let link = mods_dir.join(Deployer::link_name(&entry));

        deployer.enable(entry.id).unwrap();
        assert_eq!(
            fs::read_to_string(&sentinel).unwrap(),
            "keep this source unchanged"
        );
        assert!(paths_equivalent(&junction::get_target(&link).unwrap(), &source).unwrap());
        deployer.disable(entry.id).unwrap();

        assert_eq!(
            fs::read_to_string(&sentinel).unwrap(),
            "keep this source unchanged"
        );
        assert!(source.is_dir());
        assert!(!link.exists());
        assert!(!lib.layout.mod_dir("Firefly", "ExternalSource").exists());
    }

    #[test]
    fn reconcile_only_removes_orphaned_junctions_to_managed_deployment_roots() {
        let (_t, lib, mods_dir) = setup();
        let retained_target = lib.layout.root.join("tmp").join("unrelated");
        let removable_target = lib.layout.mods_root().join("Removed");
        fs::create_dir_all(&retained_target).unwrap();
        fs::create_dir_all(&removable_target).unwrap();
        let retained_link = mods_dir.join("unrelated-library-link");
        let removable_link = mods_dir.join("old-liquimod-link");
        junction::create(&retained_target, &retained_link).unwrap();
        junction::create(&removable_target, &removable_link).unwrap();

        Deployer::new(&lib, &mods_dir).reconcile().unwrap();

        assert!(junction::exists(&retained_link).unwrap());
        assert_eq!(
            junction::get_target(&retained_link).unwrap(),
            retained_target
        );
        assert!(!removable_link.exists());
    }

    #[test]
    fn disable_removes_a_dangling_junction_only_when_its_missing_target_is_expected() {
        let (_t, lib, mods_dir) = setup();
        let source_parent = tempfile::tempdir().unwrap();
        let source = source_parent.path().join("DanglingExternal");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("mod.ini"), "external").unwrap();
        let entry = lib
            .add_external_folder(&source, "Firefly", "DanglingExternal")
            .unwrap();
        let deployer = Deployer::new(&lib, &mods_dir);
        let link = mods_dir.join(Deployer::link_name(&entry));
        deployer.enable(entry.id).unwrap();
        fs::remove_dir_all(&source).unwrap();

        assert!(!link.exists());
        assert!(fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(!junction::exists(&link).unwrap());

        deployer.disable(entry.id).unwrap();

        assert!(matches!(
            fs::symlink_metadata(&link),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        assert!(!lib.db.get_mod(entry.id).unwrap().enabled);
        assert!(lib.db.pending_ops().unwrap().is_empty());
    }

    #[test]
    fn disable_preserves_a_dangling_junction_when_its_target_is_not_the_current_mod() {
        let (_t, lib, mods_dir) = setup();
        fs::create_dir_all(lib.layout.mod_dir("Firefly", "UnexpectedDangling")).unwrap();
        let entry = lib.scan().unwrap()[0].clone();
        let unexpected_root = tempfile::tempdir().unwrap();
        let unexpected_target = unexpected_root.path().join("not-liquimod");
        fs::create_dir_all(&unexpected_target).unwrap();
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::create(&unexpected_target, &link).unwrap();
        fs::remove_dir_all(&unexpected_target).unwrap();
        lib.db.set_enabled(entry.id, true).unwrap();

        let err = Deployer::new(&lib, &mods_dir)
            .disable(entry.id)
            .unwrap_err()
            .to_string();

        assert!(err.contains("unexpected Junction"));
        assert!(matches!(
            fs::symlink_metadata(&link),
            Ok(metadata) if metadata.file_type().is_symlink()
        ));
        assert!(lib.db.get_mod(entry.id).unwrap().enabled);
        assert_eq!(lib.db.pending_ops().unwrap().len(), 1);
    }
}
