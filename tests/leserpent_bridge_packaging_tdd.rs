use std::path::PathBuf;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn linux_publish_builds_and_installs_the_rust_compatibility_bridge() {
    let root = repository_root();
    assert!(
        root.join("crates/leserpent-protocol/src/bin/leserpent-compat-bridge.rs")
            .is_file()
    );
    let ignore = std::fs::read_to_string(root.join(".gitignore")).unwrap();
    assert!(ignore.contains("!**/src/bin/"));
    assert!(ignore.contains("!**/src/bin/*.rs"));

    let project =
        std::fs::read_to_string(root.join("apps/leserpent/src/Leserpent/Leserpent.csproj"))
            .unwrap();
    assert!(project.contains("BuildRustCompatibilityBridge"));
    assert!(project.contains(
        "cargo build --locked --release -p leserpent-protocol --bin leserpent-compat-bridge"
    ));
    assert!(project.contains("RejectCrossPlatformRustCompatibilityBridge"));

    let installer =
        std::fs::read_to_string(root.join("apps/leserpent/deploy/linux/install.sh")).unwrap();
    assert!(
        installer.contains("for required in Leserpent leserpent-compat-bridge libe_sqlite3.so")
    );
    assert!(installer.contains("chmod 0755 \"${release_dir}/leserpent-compat-bridge\""));

    let environment =
        std::fs::read_to_string(root.join("apps/leserpent/deploy/linux/leserpent.env.example"))
            .unwrap();
    assert!(
        environment
            .contains("LESERPENT_RUST_BRIDGE_BIN=/opt/leserpent/current/leserpent-compat-bridge")
    );
    assert!(environment.contains("LESERPENT_RUST_BRIDGE_TIMEOUT_MS=2000"));
}
