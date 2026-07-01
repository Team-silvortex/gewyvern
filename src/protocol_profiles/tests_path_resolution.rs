use super::profiles::PROTOCOL_PROFILES;
use super::resolve_built_in_dsl_path;
use super::tests_env::EnvGuard;
use std::fs;
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
