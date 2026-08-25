const WINDOWS_APP_MANIFEST: &str = include_str!("windows-app-manifest.xml");

fn main() {
    println!("cargo:rerun-if-changed=windows-app-manifest.xml");
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        // rc.exe/Tauri resource generation has historically re-encoded non-ASCII
        // XML comments as CP936, producing an invalid activation-context manifest.
        // Keep the embedded release manifest ASCII-only so Windows can parse it.
        assert!(
            WINDOWS_APP_MANIFEST.is_ascii(),
            "windows-app-manifest.xml must contain ASCII only"
        );
        let windows = tauri_build::WindowsAttributes::new().app_manifest(WINDOWS_APP_MANIFEST);
        let attributes = tauri_build::Attributes::new().windows_attributes(windows);
        tauri_build::try_build(attributes).expect("failed to run Tauri release build script");
    } else {
        tauri_build::build();
    }
}
