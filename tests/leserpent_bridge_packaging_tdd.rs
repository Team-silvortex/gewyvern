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
        "cargo build --locked --release -p leserpent-protocol -p leserpentd --bin leserpent-compat-bridge --bin leserpentd --features leserpentd/native-ssh"
    ));
    assert_eq!(
        project
            .matches("<Exec Command=\"cargo build --locked --release")
            .count(),
        1
    );
    assert!(project.contains("SkipRustCompatibilityBridge"));
    assert!(project.contains("DestinationFiles=\"$(PublishDir)leserpentd\""));
    assert!(project.contains("RejectCrossPlatformRustCompatibilityBridge"));
    assert!(project.contains("packages.development.lock.json"));
    assert!(project.contains("'$(PublishAot)' == 'true'"));
    assert!(project.contains("NativeAotToolchainVersion"));
    assert!(project.contains("KnownILCompilerPack"));
    let publish_profile = std::fs::read_to_string(
        root.join("apps/leserpent/src/Leserpent/Properties/PublishProfiles/native-aot.pubxml"),
    )
    .unwrap();
    assert!(publish_profile.contains("<StripSymbols>true</StripSymbols>"));
    assert!(publish_profile.contains("<Link>deploy/install.sh</Link>"));
    assert!(publish_profile.contains("<Link>deploy/leserpent.service</Link>"));
    assert!(
        root.join("apps/leserpent/src/Leserpent/packages.development.lock.json")
            .is_file()
    );

    let installer =
        std::fs::read_to_string(root.join("apps/leserpent/deploy/linux/install.sh")).unwrap();
    assert!(
        installer.contains(
            "for required in Leserpent leserpent-compat-bridge leserpentd libe_sqlite3.so"
        )
    );
    assert!(installer.contains("chmod 0755 \"${release_dir}/leserpent-compat-bridge\""));
    assert!(installer.contains("chmod 0755 \"${release_dir}/leserpentd\""));
    assert!(installer.contains("--rollback"));
    assert!(installer.contains("release_link_target"));
    assert!(installer.contains("replace_release_link previous"));
    assert!(installer.contains("local pending=\"${prefix}/.${name}.new\""));
    assert!(!installer.contains("target=\"$2\" pending="));
    assert!(installer.contains("cannot rollback: previous release link is missing or unsafe"));
    assert!(installer.contains("cannot upgrade: current release link is missing or unsafe"));
    assert!(installer.contains("rollback health check failed; restored original release"));
    assert!(installer.contains("verify_bundle \"${SOURCE_DIR}\""));
    assert!(installer.contains("sha256sum --strict --check SHA256SUMS"));
    assert!(installer.contains("release_hash=\"$(sha256sum \"${SOURCE_DIR}/SHA256SUMS\""));
    assert!(installer.contains("flock -w 120 9"));
    assert!(installer.contains("exec 9<\"${DESTDIR}\""));
    assert!(installer.contains("refusing to use / as a staged Leserpent installation root"));
    assert!(installer.contains("cleanup_uncommitted_release"));
    assert!(installer.contains("--keep-releases must be an integer from 2 through 64"));
    assert!(installer.contains("cp -a --no-preserve=ownership"));
    assert!(!installer.contains("-name '*.dbg' -delete"));
    assert_eq!(installer.matches("health_check() {").count(), 1);
    assert!(
        installer.find("flock -w 120 9").unwrap()
            < installer.find("verify_bundle \"${SOURCE_DIR}\"").unwrap()
    );
    assert!(
        installer.find("verify_bundle \"${SOURCE_DIR}\"").unwrap()
            < installer
                .find("if [[ \"${ACTION}\" == rollback ]]")
                .unwrap()
    );

    let smoke =
        std::fs::read_to_string(root.join("scripts/validation/leserpent_linux_bundle_smoke.sh"))
            .unwrap();
    assert!(smoke.contains("staged-upgrade-current-previous-link"));
    assert!(smoke.contains("unsafe-existing-current-link-rejection"));
    assert!(smoke.contains("configuration-preserved-across-upgrade-rollback"));
    assert!(smoke.contains("state-preserved-across-upgrade-rollback"));
    assert!(smoke.contains("explicit-atomic-rollback"));
    assert!(smoke.contains("rolled-back-live-native-aot-health"));
    assert!(smoke.contains("tampered-bundle-rejection-before-mutation"));
    assert!(smoke.contains("exact-bundle-inventory"));
    assert!(smoke.contains("content-addressed-release-identity"));
    assert!(smoke.contains("staged-install-lock-before-bundle-verification"));
    assert!(smoke.contains("unsafe-staging-root-rejection"));
    assert!(smoke.contains("flock -n 8"));

    let retained =
        std::fs::read_to_string(root.join("docs/fixtures/leserpent_linux_bundle_smoke.json"))
            .unwrap();
    assert!(retained.contains("\"staged-install-lock-before-bundle-verification\""));
    assert!(retained.contains("\"unsafe-staging-root-rejection\""));
    assert!(retained.contains("\"failed-upgrade-release-and-unit-restoration\""));
    assert!(retained.contains("\"daemon_sha256\""));

    let environment =
        std::fs::read_to_string(root.join("apps/leserpent/deploy/linux/leserpent.env.example"))
            .unwrap();
    assert!(
        environment
            .contains("LESERPENT_RUST_BRIDGE_BIN=/opt/leserpent/current/leserpent-compat-bridge")
    );
    assert!(environment.contains("LESERPENT_RUST_BRIDGE_TIMEOUT_MS=2000"));
    assert!(environment.contains("# LESERPENT_DAEMON_SOCKET=/run/leserpent/leserpentd.sock"));
    assert!(environment.contains("# LESERPENT_DAEMON_TOKEN="));
}

#[test]
fn deployment_bridge_validation_is_strict_and_pre_effect() {
    let root = repository_root();
    let endpoints = std::fs::read_to_string(
        root.join("apps/leserpent/src/Leserpent/ProgramRuntimeEndpoints.cs"),
    )
    .unwrap();
    let compatibility =
        std::fs::read_to_string(root.join("crates/leserpent-protocol/src/compatibility_v1.rs"))
            .unwrap();
    let bridge = std::fs::read_to_string(
        root.join("crates/leserpent-protocol/src/bin/leserpent-compat-bridge.rs"),
    )
    .unwrap();
    let daemon_authority = std::fs::read_to_string(
        root.join("apps/leserpent/src/Leserpent/ControlPlane/DaemonDeploymentAuthority.cs"),
    )
    .unwrap();

    let local_capability_check = endpoints
        .find("runtime_deployment_not_supported")
        .expect("deployment capability check");
    let compatibility_check = endpoints
        .find("NormalizeRuntimeDeploymentRequestAsync")
        .expect("deployment compatibility check");
    let remote_effect = endpoints
        .find("discovery.DeployAsync")
        .expect("remote deployment effect");
    assert!(local_capability_check < compatibility_check);
    assert!(compatibility_check < remote_effect);
    assert!(endpoints.contains("deploymentAuthority.DeployAsync"));
    assert!(daemon_authority.contains("UnixDomainSocketEndPoint"));
    assert!(daemon_authority.contains("daemon_socket_unsafe"));
    assert!(daemon_authority.contains("deployment_receipt"));
    assert!(compatibility.contains("deny_unknown_fields"));
    assert!(compatibility.contains("decode_runtime_deployment_request"));
    assert!(bridge.contains("ValidateRuntimeDeploymentRequest"));
    assert!(
        root.join(
            "crates/leserpent-protocol/tests/fixtures/legacy-runtime-deployment-request-v1.json"
        )
        .is_file()
    );
}
