use super::profiles::PROTOCOL_PROFILES;
use super::tests_env::EnvGuard;
use super::{
    ProtocolCatalogSnapshot, resolve_built_in_dsl_path, resolve_protocol_profile_from_dir,
};
use std::fs;
#[cfg(target_family = "unix")]
use std::os::unix::fs as unix_fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn built_in_profile_dsl_paths_stay_packaging_relative() {
    let forbidden_prefixes = [
        "/Users/",
        "/home/",
        "/private/",
        "/var/folders/",
        env!("CARGO_MANIFEST_DIR"),
    ];

    for profile in PROTOCOL_PROFILES {
        for entry in profile.entries {
            assert!(
                !Path::new(entry.dsl_path).is_absolute(),
                "{}:{} should not embed an absolute dsl_path: {}",
                profile.name,
                entry.mode,
                entry.dsl_path
            );
            for prefix in forbidden_prefixes {
                assert!(
                    !entry.dsl_path.starts_with(prefix),
                    "{}:{} should not embed local checkout path prefix {} in {}",
                    profile.name,
                    entry.mode,
                    prefix,
                    entry.dsl_path
                );
            }
            assert!(
                entry.dsl_path.starts_with("dsl/") || entry.dsl_path.starts_with("protocols/"),
                "{}:{} should use packaged dsl/ or protocols/ relative path, got {}",
                profile.name,
                entry.mode,
                entry.dsl_path
            );
        }
    }
}

#[test]
fn built_in_dsl_path_resolves_relative_packaged_paths() {
    let _lock = super::tests_env::lock();
    let root = std::env::temp_dir().join(format!(
        "gewyvern-packaged-relative-dsl-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after epoch")
            .as_nanos()
    ));
    let dsl_dir = root.join("dsl");
    let protocol_dir = root.join("protocols/__packaged-relative-only/main");
    fs::create_dir_all(&dsl_dir).expect("test dsl directory should be created");
    fs::create_dir_all(&protocol_dir).expect("test protocol directory should be created");

    let dsl_file = dsl_dir.join("__packaged_relative_only.gewy");
    let protocol_file = protocol_dir.join("main.gewy");
    fs::write(&dsl_file, "template(:http_request_path)\n")
        .expect("test dsl file should be written");
    fs::write(&protocol_file, "template(:redis_auth_required)\n")
        .expect("test protocol file should be written");
    let _guard = EnvGuard::set("GEWY_SHARE_ROOT", root.to_string_lossy().into_owned());

    assert_eq!(
        PathBuf::from(resolve_built_in_dsl_path(
            "dsl/__packaged_relative_only.gewy"
        )),
        dsl_file
    );
    assert_eq!(
        PathBuf::from(resolve_built_in_dsl_path(
            "protocols/__packaged-relative-only/main/main.gewy"
        )),
        protocol_file
    );

    fs::remove_dir_all(&root).expect("test root should be removable");
}

#[test]
fn built_in_dsl_path_falls_back_to_packaged_share_root() {
    let _lock = super::tests_env::lock();
    let root = std::env::temp_dir().join(format!(
        "gewyvern-packaged-dsl-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dsl_dir = root.join("dsl");
    fs::create_dir_all(&dsl_dir).unwrap();
    let file = dsl_dir.join("http_request_path.gewy");
    fs::write(&file, "template(:http_request_path)\n").unwrap();
    let _guard = EnvGuard::set("GEWY_SHARE_ROOT", root.to_string_lossy().into_owned());
    let resolved = resolve_built_in_dsl_path("/definitely/missing/dsl/http_request_path.gewy");
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(PathBuf::from(resolved), file);
}

#[test]
fn profile_resolves_from_an_explicit_registry_directory() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-packaged-protocol-registry-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("http").join("request");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=http_request\nversion=0.18.2\nentry=main.gewy\nregister.protocol=http\nregister.entry=request\nregister.default=true\n",
    )
    .unwrap();
    fs::write(package_dir.join("main.gewy"), "template(:http_request)\n").unwrap();
    let resolved = resolve_protocol_profile_from_dir(&root, "http", Some("request"))
        .map(|profile| profile.dsl_path);
    let expected = fs::canonicalize(&package_dir)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(resolved, Some(expected));
}

#[test]
fn catalog_snapshot_is_request_local_and_refreshes_on_rediscovery() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-protocol-snapshot-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("custom").join("observe");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(package_dir.join("main.gewy"), "template(:custom_observe)\n").unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "entry=main.gewy\nregister.protocol=custom\nregister.entry=observe\nregister.default=true\n",
    )
    .unwrap();

    let first = ProtocolCatalogSnapshot::discover_in(&root).expect("first catalog snapshot");
    fs::write(
        package_dir.join("gewy.pkg"),
        "entry=main.gewy\nregister.protocol=custom\nregister.entry=inspect\nregister.default=true\n",
    )
    .unwrap();
    let second = ProtocolCatalogSnapshot::discover_in(&root).expect("second catalog snapshot");

    assert!(
        first
            .resolve_protocol_profile("custom", Some("observe"))
            .is_some()
    );
    assert!(
        first
            .resolve_protocol_profile("custom", Some("inspect"))
            .is_none()
    );
    assert!(
        second
            .resolve_protocol_profile("custom", Some("inspect"))
            .is_some()
    );
    fs::remove_dir_all(&root).unwrap();
}

#[cfg(target_family = "unix")]
#[test]
fn registry_scan_ignores_symlinked_directories() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-protocol-registry-symlink-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("mysql").join("session");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=mysql_session\nversion=0.18.2\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
    )
    .unwrap();
    fs::write(package_dir.join("main.gewy"), "template(:mysql_session)\n").unwrap();
    unix_fs::symlink(root.join("mysql"), root.join("mysql-link")).unwrap();

    let targets = super::default_protocol_scan_set_from_dir(root.to_str().unwrap()).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].protocol, "mysql");
    assert_eq!(targets[0].entry, "session");
}
