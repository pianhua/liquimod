#![cfg(windows)]

use liquimod_core::deploy::{Deployer, DeploymentStatusKind};
use liquimod_core::library::Library;
use liquimod_core::models::ModEntry;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_FLAG_BACKUP_SEMANTICS,
    FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_SHARE_MODE, OPEN_EXISTING,
};

struct Fixture {
    _temp: TempDir,
    mods_dir: PathBuf,
    library: Library,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().expect("create test temp directory");
        let library_root = temp.path().join("Library");
        let mods_dir = temp.path().join("GameMods");
        fs::create_dir_all(&mods_dir).expect("create test Mods directory");
        let library = Library::init(&library_root).expect("initialize test Library");
        let deployer = Deployer::new(&library, &mods_dir);
        assert_eq!(
            deployer.strategy(),
            liquimod_core::filesystem::DeployStrategy::Junction,
            "Windows fault-injection tests require an NTFS/ReFS same-volume fixture"
        );
        Self {
            _temp: temp,
            mods_dir,
            library,
        }
    }

    fn managed_mod(&self, name: &str) -> ModEntry {
        let root = self.library.layout.mod_dir("Firefly", name);
        fs::create_dir_all(&root).expect("create managed Mod directory");
        fs::write(root.join("mod.ini"), b"[Constants]\n").expect("write managed Mod file");
        self.library
            .scan()
            .expect("scan managed Mod")
            .into_iter()
            .find(|entry| entry.name == name)
            .expect("managed Mod should be indexed")
    }

    fn deployer(&self) -> Deployer<'_> {
        Deployer::new(&self.library, &self.mods_dir)
    }

    fn link(&self, entry: &ModEntry) -> PathBuf {
        self.mods_dir.join(Deployer::link_name(entry))
    }
}

struct ExclusiveHandle(HANDLE);

impl Drop for ExclusiveHandle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn open_exclusive(path: &Path, directory: bool) -> io::Result<ExclusiveHandle> {
    use std::os::windows::ffi::OsStrExt;

    let wide = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect::<Vec<_>>();
    let flags = if directory {
        FILE_FLAGS_AND_ATTRIBUTES(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
    } else {
        FILE_FLAGS_AND_ATTRIBUTES(0)
    };
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            FILE_GENERIC_READ.0,
            FILE_SHARE_MODE(0),
            None,
            OPEN_EXISTING,
            flags,
            None,
        )
    }
    .map_err(|error| io::Error::other(error.to_string()))?;
    Ok(ExclusiveHandle(handle))
}

fn assert_pending(library: &Library, operation: &str, id: i64) {
    let pending = library.db.pending_ops().expect("read pending operations");
    assert!(
        pending
            .iter()
            .any(|(_, op, payload)| op == operation && payload == &id.to_string()),
        "pending operations did not include {operation}/{id}: {pending:?}"
    );
}

#[test]
fn locked_junction_preserves_state_and_recovers_after_unlock() {
    let fixture = Fixture::new();
    let entry = fixture.managed_mod("LockedRefresh");
    let deployer = fixture.deployer();
    deployer.enable(entry.id).expect("initial deployment");
    let link = fixture.link(&entry);

    // Open the Junction itself without FILE_SHARE_DELETE to model a loader/game handle retaining
    // the deployment entry. Inspection must fail closed rather than deleting an unverified path.
    let _lock = open_exclusive(&link, true).expect("open Junction without delete sharing");
    let error = deployer
        .disable(entry.id)
        .expect_err("disable must fail while the Junction is exclusively open");

    let error_text = error.to_string();
    assert!(
        error_text.contains("being used")
            || error_text.contains("Access is denied")
            || error_text.contains("拒绝访问")
            || error_text.contains("os error 32"),
        "unexpected locked Junction error: {error_text}"
    );
    assert!(fs::symlink_metadata(&link)
        .expect("inspect locked Junction metadata")
        .file_type()
        .is_symlink());
    assert!(fixture.library.db.get_mod(entry.id).unwrap().enabled);
    assert_pending(&fixture.library, "disable", entry.id);

    drop(_lock);
    deployer
        .recover()
        .expect("pending disable should reconcile after releasing the Junction lock");

    assert!(junction::exists(&link).expect("inspect recovered Junction"));
    assert!(fixture.library.db.get_mod(entry.id).unwrap().enabled);
    assert!(fixture.library.db.pending_ops().unwrap().is_empty());
}

#[test]
fn locked_runtime_file_leaves_pending_disable_and_recover_cleans_it_after_unlock() {
    let fixture = Fixture::new();
    let root = fixture.library.layout.mod_dir("Firefly", "LockedRuntime");
    fs::create_dir_all(root.join("Option A")).expect("create variant directory");
    fs::write(root.join("Option A").join("choice.ini"), b"variant").expect("write variant file");
    let entry = fixture
        .library
        .scan()
        .expect("scan variant Mod")
        .into_iter()
        .find(|entry| entry.name == "LockedRuntime")
        .expect("variant Mod should be indexed");
    let deployer = fixture.deployer();
    deployer
        .enable(entry.id)
        .expect("initial variant deployment");
    let runtime = fixture.library.layout.runtime_mod_dir(entry.id);
    let runtime_file = runtime.join("choice.ini");
    assert!(runtime_file.is_file());
    let _lock =
        open_exclusive(&runtime_file, false).expect("open runtime file without delete sharing");

    let error = deployer
        .disable(entry.id)
        .expect_err("disable must report a locked runtime cleanup failure");

    let error_text = error.to_string();
    assert!(
        error_text.contains("being used")
            || error_text.contains("Access is denied")
            || error_text.contains("拒绝访问")
            || error_text.contains("os error 5")
            || error_text.contains("os error 32"),
        "unexpected runtime cleanup error: {error_text}"
    );
    assert!(!fixture.link(&entry).exists());
    assert!(!fixture.library.db.get_mod(entry.id).unwrap().enabled);
    assert_pending(&fixture.library, "disable", entry.id);
    assert!(runtime_file.exists());

    drop(_lock);
    deployer
        .recover()
        .expect("pending disable should recover after releasing the runtime file lock");

    assert!(!runtime.exists());
    assert!(fixture.library.db.pending_ops().unwrap().is_empty());
}

#[test]
fn reopened_library_recovers_a_missing_enabled_junction_from_pending_operation() {
    let temp = tempfile::tempdir().expect("create test temp directory");
    let library_root = temp.path().join("Library");
    let mods_dir = temp.path().join("GameMods");
    fs::create_dir_all(&mods_dir).expect("create test Mods directory");
    let entry_id;
    let entry_name;
    {
        let library = Library::init(&library_root).expect("initialize test Library");
        let root = library.layout.mod_dir("Firefly", "RestartRecovery");
        fs::create_dir_all(&root).expect("create Mod directory");
        fs::write(root.join("mod.ini"), b"restart").expect("write Mod file");
        let entry = library
            .scan()
            .expect("scan Mod")
            .into_iter()
            .next()
            .expect("Mod should be indexed");
        entry_id = entry.id;
        entry_name = entry.name.clone();
        let deployer = Deployer::new(&library, &mods_dir);
        deployer.enable(entry.id).expect("initial deployment");
        let link = mods_dir.join(Deployer::link_name(&entry));
        junction::delete(&link).expect("remove deployment to simulate interruption");
        if link.exists() {
            fs::remove_dir(&link).expect("remove residual Junction directory entry");
        }
        library
            .db
            .op_begin("refresh", &entry.id.to_string())
            .expect("record interrupted refresh");
    }

    let library = Library::open(&library_root).expect("reopen Library after simulated restart");
    let deployer = Deployer::new(&library, &mods_dir);
    deployer.recover().expect("recover interrupted operation");
    let entry = library.db.get_mod(entry_id).expect("read recovered Mod");
    let link = mods_dir.join(Deployer::link_name(&entry));

    assert_eq!(entry.name, entry_name);
    assert!(entry.enabled);
    assert!(junction::exists(&link).expect("inspect recovered deployment"));
    assert_eq!(
        Deployer::new(&library, &mods_dir)
            .inspect_status()
            .expect("inspect recovered status")[0]
            .kind,
        DeploymentStatusKind::Deployed
    );
    assert!(library.db.pending_ops().unwrap().is_empty());
}

#[test]
fn offline_external_source_keeps_enabled_intent_and_pending_recovery_operation() {
    let fixture = Fixture::new();
    let source_parent = tempfile::tempdir().expect("create external source parent");
    let source = source_parent.path().join("OfflineSource");
    fs::create_dir_all(&source).expect("create external source");
    fs::write(source.join("mod.ini"), b"external").expect("write external Mod file");
    let entry = fixture
        .library
        .add_external_folder(&source, "Firefly", "OfflineSource")
        .expect("index external Mod");
    fixture.library.db.set_enabled(entry.id, true).unwrap();
    fixture
        .library
        .db
        .op_begin("refresh", &entry.id.to_string())
        .expect("record interrupted external refresh");
    fs::remove_dir_all(&source).expect("take external source offline");

    let error = fixture
        .deployer()
        .recover()
        .expect_err("offline external source must block recovery");

    assert!(error.to_string().contains("external source unavailable"));
    assert!(fixture.library.db.get_mod(entry.id).unwrap().enabled);
    assert_pending(&fixture.library, "refresh", entry.id);
    assert!(!fixture
        .library
        .layout
        .mod_dir("Firefly", "OfflineSource")
        .exists());
}

#[test]
fn unknown_junction_during_restart_is_preserved_for_manual_resolution() {
    let fixture = Fixture::new();
    let entry = fixture.managed_mod("ManualResolution");
    fixture.library.db.set_enabled(entry.id, true).unwrap();
    let link = fixture.link(&entry);
    let foreign_parent = tempfile::tempdir().expect("create foreign target parent");
    let foreign_target = foreign_parent.path().join("foreign");
    fs::create_dir_all(&foreign_target).expect("create foreign target");
    junction::create(&foreign_target, &link).expect("create foreign Junction");
    fixture
        .library
        .db
        .op_begin("refresh", &entry.id.to_string())
        .expect("record interrupted refresh");

    let error = fixture
        .deployer()
        .recover()
        .expect_err("recovery must stop on an unknown Junction");

    assert!(error.to_string().contains("unexpected Junction"));
    assert!(junction::exists(&link).expect("foreign Junction must remain"));
    assert_eq!(junction::get_target(&link).unwrap(), foreign_target);
    assert!(fixture.library.db.get_mod(entry.id).unwrap().enabled);
    assert_pending(&fixture.library, "refresh", entry.id);
}

fn alternate_volume_root(known_path: &Path) -> Option<PathBuf> {
    let known_volume = liquimod_core::filesystem::volume_root(known_path)?;
    for letter in b'A'..=b'Z' {
        let root = PathBuf::from(format!("{}:\\", char::from(letter)));
        if !root.is_dir() {
            continue;
        }
        let Some(volume) = liquimod_core::filesystem::volume_root(&root) else {
            continue;
        };
        if volume != known_volume {
            return Some(root);
        }
    }
    None
}

#[test]
fn cross_volume_strategy_rejects_deployment_and_only_cleans_marked_legacy_copy() {
    let library_temp = tempfile::tempdir().expect("create default-volume test directory");
    let Some(other_volume) = alternate_volume_root(library_temp.path()) else {
        eprintln!("skipping cross-volume test: no second volume is available");
        return;
    };
    let mods_temp = match tempfile::Builder::new()
        .prefix("liquimod-cross-volume-")
        .tempdir_in(&other_volume)
    {
        Ok(temp) => temp,
        Err(error) => {
            eprintln!(
                "skipping cross-volume test: cannot create a directory on {}: {error}",
                other_volume.display()
            );
            return;
        }
    };
    let library_root = library_temp.path().join("Library");
    let mods_dir = mods_temp.path().join("GameMods");
    fs::create_dir_all(&mods_dir).expect("create cross-volume Mods directory");
    let library = Library::init(&library_root).expect("initialize cross-volume Library");
    let source = library.layout.mod_dir("Firefly", "CrossVolume");
    fs::create_dir_all(&source).expect("create cross-volume test Mod");
    fs::write(source.join("mod.ini"), b"cross-volume").expect("write test Mod file");
    let entry = library
        .scan()
        .expect("scan cross-volume Mod")
        .into_iter()
        .next()
        .expect("cross-volume Mod should be indexed");
    let deployer = Deployer::new(&library, &mods_dir);

    assert_eq!(
        deployer.strategy(),
        liquimod_core::filesystem::DeployStrategy::CopyFallback
    );
    let volume_description =
        liquimod_core::filesystem::same_volume_filesystem(&library_root, &mods_dir)
            .expect("describe cross-volume filesystem state");
    assert!(
        volume_description.contains("不同卷") || !volume_description.eq_ignore_ascii_case("NTFS"),
        "cross-volume diagnostics must not report a plain same-volume filesystem: {volume_description}"
    );
    let link = mods_dir.join(Deployer::link_name(&entry));
    let error = deployer
        .enable(entry.id)
        .expect_err("cross-volume deployment must remain unsupported");
    assert!(error.to_string().contains("Junction"));
    assert!(!link.exists());
    assert!(!library.db.get_mod(entry.id).unwrap().enabled);
    assert_pending(&library, "enable", entry.id);

    // A legacy copy directory is removable only when its trusted marker is present. This does
    // not enable CopyFallback; it verifies the narrowly scoped cleanup compatibility branch.
    fs::create_dir_all(&link).expect("create marked legacy copy directory");
    fs::write(link.join(".liquimod-managed"), b"legacy").expect("write legacy marker");
    fs::write(link.join("sentinel.ini"), b"legacy copy").expect("write legacy copy file");
    deployer
        .disable(entry.id)
        .expect("disabled Mod should clean a marked legacy copy");
    assert!(!link.exists());
}
