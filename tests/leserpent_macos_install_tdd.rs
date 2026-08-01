use std::fs;
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(root().join(path)).unwrap()
}

#[test]
fn macos_installer_is_native_bounded_and_rollback_capable() {
    let installer = read("src/leserpent_macos_install.rs");
    let entrypoint = read("src/bin/gewyvern_leserpent_install.rs");
    let fixture = read("docs/fixtures/leserpent_macos_install_rollback.json");

    assert!(entrypoint.contains("leserpent_macos_install::{InstallOptions, execute}"));
    assert!(!installer.contains("std::process::Command"));
    assert!(installer.contains("MAX_BUNDLE_FILES"));
    assert!(installer.contains("MAX_BUNDLE_BYTES"));
    assert!(installer.contains("application bundle contains a symbolic link"));
    assert!(installer.contains("DAEMON_EXECUTABLE: &str = \"leserpentd\""));
    assert!(installer.contains("validate_native_payloads(app)?"));
    assert!(installer.contains("local orchestra daemon is unavailable"));
    assert!(installer.contains("current_identity_matches = daemon_available"));
    assert!(installer.contains("legacy_release_version(&identity.version)"));
    assert!(installer.contains("accepts_a_daemonless_legacy_release_but_rejects_future_versions"));
    assert!(installer.contains("native_payload_hash"));
    assert!(installer.contains("replace_link(&options.root, \"current\""));
    assert!(installer.contains("replace_link(&options.root, \"previous\""));
    assert!(installer.contains("restore_link"));
    assert!(installer.contains("source application cannot be inside the install root"));
    assert!(fixture.contains("\"apple_release_evidence\": false"));
    assert!(fixture.contains("\"rolled-back-live-control-fixture\""));
}
