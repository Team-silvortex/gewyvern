use std::collections::BTreeSet;
use std::path::PathBuf;

use ring::digest::{SHA256, digest};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn sha256_hex(bytes: &[u8]) -> String {
    digest(&SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn frontend_package_manifest_matches_every_published_asset() {
    let root = repository_root();
    let manifest_path = root.join("apps/leserpent/frontend-package-manifest.json");
    let manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("frontend package manifest must exist"),
    )
    .expect("frontend package manifest must be JSON");

    assert_eq!(manifest["schema"], "leserpent.frontend-package/v1");
    for field in ["inputsSha256", "assetsSha256"] {
        let value = manifest[field].as_str().expect("digest must be a string");
        assert_eq!(value.len(), 64);
        assert!(value.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }

    let assets = manifest["assets"]
        .as_array()
        .expect("frontend assets must be an array");
    assert_eq!(
        manifest["assetFileCount"].as_u64(),
        Some(assets.len() as u64)
    );
    assert!(!assets.is_empty() && assets.len() <= 128);
    let mut paths = BTreeSet::new();
    let mut total_bytes = 0_u64;
    for asset in assets {
        let relative = asset["path"].as_str().expect("asset path must be a string");
        assert!(relative.starts_with("src/Leserpent/wwwroot/"));
        assert!(!relative.contains(".."));
        assert!(paths.insert(relative));

        let path = root.join("apps/leserpent").join(relative);
        let metadata = std::fs::symlink_metadata(&path).expect("asset must exist");
        assert!(metadata.is_file());
        assert!(!metadata.file_type().is_symlink());
        let bytes = std::fs::read(&path).expect("asset must be readable");
        total_bytes += bytes.len() as u64;
        assert_eq!(asset["bytes"].as_u64(), Some(bytes.len() as u64));
        assert_eq!(asset["sha256"], sha256_hex(&bytes));
    }
    assert_eq!(manifest["assetBytes"].as_u64(), Some(total_bytes));
    assert!(total_bytes <= 4 * 1024 * 1024);
}

#[test]
fn frontend_packaging_is_incremental_bounded_and_publish_integrated() {
    let root = repository_root();
    let coordinator =
        std::fs::read_to_string(root.join("crates/leserpent-frontend-package/src/main.rs"))
            .expect("native frontend package coordinator must exist");
    let package = std::fs::read_to_string(root.join("apps/leserpent/package.json")).unwrap();
    let project =
        std::fs::read_to_string(root.join("apps/leserpent/src/Leserpent/Leserpent.csproj"))
            .unwrap();
    let program =
        std::fs::read_to_string(root.join("apps/leserpent/src/Leserpent/Program.cs")).unwrap();

    for contract in [
        "Context::new(&SHA256)",
        "MAX_INPUT_FILES: usize = 128",
        "MAX_ASSET_FILES: usize = 128",
        "MAX_SCANNED_ENTRIES: usize = 512",
        "MAX_DIRECTORY_DEPTH: usize = 16",
        "MAX_INPUT_BYTES: u64 = 4 * 1024 * 1024",
        "MAX_ASSET_BYTES: u64 = 4 * 1024 * 1024",
        "metadata.file_type().is_symlink()",
        "frontend package input must be a real file",
        "verify && force",
        "package.read_manifest()?.as_ref() == Some(&state)",
        "deny_unknown_fields",
        "frontend package up to date",
        "timed out waiting for frontend package lock",
        "create_new(true)",
        "replace_file(&temporary, &self.manifest_path)",
        "MOVEFILE_REPLACE_EXISTING",
        "SystemRandom::new()",
        "Command::new(npm_program())",
        "installed_version(&installed_package)",
        "Duration::from_secs(300)",
    ] {
        assert!(
            coordinator.contains(contract),
            "missing package contract {contract}"
        );
    }
    assert!(package.contains("\"package:frontend\""));
    assert!(package.contains("\"verify:frontend-package\""));
    assert!(package.contains("cargo run --quiet --locked -p leserpent-frontend-package"));
    assert!(!package.contains("node scripts/package-frontend"));
    assert!(project.contains("Name=\"BuildLeserpentFrontendPackager\""));
    assert!(project.contains("Inputs=\"@(LeserpentFrontendPackagerInput)\""));
    assert!(project.contains("Outputs=\"$(LeserpentFrontendPackagerExecutable)\""));
    assert!(project.contains(
        "<Touch Files=\"$(LeserpentFrontendPackagerExecutable)\" AlwaysCreate=\"true\" />"
    ));
    assert!(project.contains("Name=\"PackageLeserpentFrontend\""));
    assert!(project.contains("BeforeTargets=\"PrepareForBuild\""));
    assert!(project.contains("DependsOnTargets=\"BuildLeserpentFrontendPackager\""));
    assert!(project.contains("'$(Configuration)' == 'Release'"));
    assert!(project.contains("-p leserpent-frontend-package"));
    assert!(project.contains("&quot;$(LeserpentFrontendPackagerExecutable)&quot;"));
    assert!(project.contains("WorkingDirectory=\"$(RepositoryRoot)\""));
    assert!(!project.contains("cargo run"));

    assert!(program.contains("app.MapStaticAssets();"));
    assert!(program.contains("app.UseDefaultFiles();"));
    assert!(program.contains("app.UseDefaultFiles();\n        app.UseRouting();"));
    assert!(program.contains("app.UseResponseCompression();"));
    assert!(!program.contains("app.UseStaticFiles("));
}

#[test]
fn frontend_package_contract_is_tracked_by_the_status_tensor() {
    let root = repository_root();
    let catalog: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("project/status/catalog.json")).unwrap())
            .unwrap();
    let cell = catalog["cells"]
        .as_array()
        .unwrap()
        .iter()
        .find(|cell| cell["id"] == "leserpent-1x/web-console/browser-operations")
        .expect("web console status cell must exist");

    assert_eq!(cell["contract"]["version"], "1.4.7");
    for surface in [
        "content-addressed-frontend-package",
        "incremental-locked-typescript-build",
        "publish-time-stale-asset-prevention",
        "native-frontend-package-coordinator",
        "incremental-native-packager-build",
        "node-free-unchanged-release-path",
        "precompressed-static-asset-endpoints",
    ] {
        assert!(
            cell["contract"]["surfaces"]
                .as_array()
                .unwrap()
                .iter()
                .any(|candidate| candidate == surface),
            "missing web console package surface {surface}"
        );
    }
    for evidence in [
        "crates/leserpent-frontend-package/src/main.rs",
        "apps/leserpent/src/Leserpent/Leserpent.csproj",
        "apps/leserpent/frontend-package-manifest.json",
        "tests/leserpent_frontend_packaging_tdd.rs",
    ] {
        assert!(
            cell["evidence"]
                .as_array()
                .unwrap()
                .iter()
                .any(|entry| entry["path"] == evidence),
            "missing web console package evidence {evidence}"
        );
    }
}
