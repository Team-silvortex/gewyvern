use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(root().join(path)).unwrap()
}

#[test]
fn macos_release_preflight_is_machine_readable_and_fail_closed() {
    let release = read("src/bin/gewyvern_leserpent_release.rs");
    let developer_workflow = read("crates/gewyvern-dev/src/main.rs");
    let fixture = read("docs/fixtures/leserpent_macos_release_preflight.json");
    let report: Value = serde_json::from_str(&fixture).unwrap();

    assert!(release.contains("Some(\"preflight\") => Action::Preflight"));
    assert!(release.contains("Some(\"account-proof\") => Action::AccountProof"));
    assert!(release.contains("ACCOUNT_PROOF_SCHEMA_VERSION: u32 = 2"));
    assert!(release.contains("verify_account_proof(&options.app, evidence)?"));
    assert!(release.contains("#[serde(deny_unknown_fields)]"));
    assert!(release.contains("read_bounded_regular_file("));
    assert!(release.contains("MAX_ACCOUNT_CONFIG_BYTES"));
    assert!(release.contains("sha256_hex(&plist_bytes)"));
    assert!(release.contains("configuration_sha256 != plist_sha256"));
    assert!(release.contains("binary_sha256 != file_sha256(&executable)?"));
    assert!(release.contains("developer_id_identity_count"));
    assert!(release.contains("notary_profile_is_valid"));
    assert!(release.contains("apple_release_tool_missing"));
    assert!(release.contains("developer_id_application_identity_missing"));
    assert!(release.contains("notary_keychain_profile_not_requested"));
    assert!(release.contains("notary_keychain_profile_unavailable"));
    assert!(release.contains("DAEMON_EXECUTABLE: &str = \"leserpentd\""));
    assert!(release.contains("for payload in nested_native_payloads(&options.app)?"));
    assert!(release.contains("native signing snapshot is missing the local orchestra daemon"));
    assert!(release.contains("nested signature Team ID does not match the app bundle"));
    assert!(release.contains("--require-ready"));
    assert!(release.contains("release preflight is blocked"));
    assert!(release.contains("const CODESIGN_PATH: &str = \"/usr/bin/codesign\""));
    assert!(release.contains("const SPCTL_PATH: &str = \"/usr/sbin/spctl\""));
    assert!(release.contains("command.env(\"PATH\", SYSTEM_PATH)"));
    assert!(release.contains("command.env_remove(\"DEVELOPER_DIR\").env_remove(\"TOOLCHAINS\")"));
    for unpinned in [
        "Command::new(\"codesign\")",
        "Command::new(\"ditto\")",
        "Command::new(\"plutil\")",
        "Command::new(\"security\")",
        "Command::new(\"spctl\")",
        "Command::new(\"xcrun\")",
    ] {
        assert!(
            !release.contains(unpinned),
            "unpinned Apple tool: {unpinned}"
        );
    }
    assert!(
        developer_workflow.contains("--identity and --notary-profile must be supplied together")
    );
    assert!(developer_workflow.contains("desktop-apple-release-preflight"));
    assert!(developer_workflow.contains("desktop-developer-id-sign"));
    assert!(developer_workflow.contains("desktop-apple-notarize"));
    assert!(developer_workflow.contains("desktop-apple-release-verify"));
    assert_eq!(report["schema_version"], 2);
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        report["daemon_executable_sha256"].as_str().unwrap().len(),
        64
    );
    assert_eq!(report["release_ready"], false);
    assert_eq!(report["result"], "blocked");
    assert_eq!(report["developer_id_application_identities"], 0);
    assert_eq!(report["notary_profile_requested"], false);
    assert_eq!(report["notary_profile_valid"], false);
    assert_eq!(report["apple_tools"].as_object().unwrap().len(), 8);
    assert!(
        report["apple_tools"]
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.as_bool() == Some(true))
    );
    assert_eq!(
        report["blockers"],
        serde_json::json!([
            "developer_id_application_identity_missing",
            "notary_keychain_profile_not_requested"
        ])
    );
    assert!(report.get("password").is_none());
    assert!(report.get("token").is_none());
}
