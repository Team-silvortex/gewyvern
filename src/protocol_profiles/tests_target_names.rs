use super::{
    protocol_target_name_for_template_id, protocol_target_name_for_template_id_from_dir,
    resolve_protocol_profile,
};
use std::fs;

#[test]
fn template_id_resolves_to_builtin_protocol_target_name() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_target_name_for_template_id("http_request_path"),
        Some("scan:http:request".to_string())
    );
    assert_eq!(
        protocol_target_name_for_template_id("tls_client_path"),
        Some("scan:tls:client".to_string())
    );
    assert_eq!(
        protocol_target_name_for_template_id("redis_zadd_path"),
        Some("scan:redis:zadd".to_string())
    );
}

#[test]
fn canonical_template_head_extracts_without_the_atom_prefix() {
    assert_eq!(
        super::extract_template_id("template :http_request_path\n"),
        Some("http_request_path".to_string())
    );
}

#[test]
fn oversized_template_source_is_ignored() {
    let path = std::env::temp_dir().join(format!(
        "gewyvern-oversized-template-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut source = "template(:oversized_template)\n".to_string();
    source.push_str(&"x".repeat(super::MAX_GEWYLANG_SOURCE_BYTES));
    fs::write(&path, source).unwrap();

    assert_eq!(super::read_template_id_from_dsl(&path), None);

    fs::remove_file(path).unwrap();
}

#[test]
fn template_id_resolves_to_packaged_protocol_target_name() {
    let _lock = super::tests_env::lock();
    assert_eq!(
        protocol_target_name_for_template_id("redis_auth_required_path"),
        Some("scan:redis:auth-required".to_string())
    );
}

#[test]
fn empty_or_unknown_template_id_has_no_protocol_target_name() {
    let _lock = super::tests_env::lock();
    assert_eq!(protocol_target_name_for_template_id(""), None);
    assert_eq!(protocol_target_name_for_template_id("socket_session"), None);
    assert_eq!(
        protocol_target_name_for_template_id("totally_unknown_template"),
        None
    );
}

#[test]
fn packaged_registry_root_target_name_uses_main_gewy_template() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-protocol-target-name-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("custom").join("observe");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=custom_observe\nversion=0.16.0\nentry=main.gewy\nregister.protocol=custom\nregister.entry=observe\nregister.default=true\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        "template(:custom_observe_path)\n",
    )
    .unwrap();

    let resolved = protocol_target_name_for_template_id_from_dir(&root, "custom_observe_path");

    fs::remove_dir_all(&root).unwrap();

    assert_eq!(resolved, Some("scan:custom:observe".to_string()));
}

#[test]
fn resolved_profile_path_still_matches_template_id() {
    let _lock = super::tests_env::lock();
    let profile = resolve_protocol_profile("http", Some("request")).expect("http request");
    assert!(profile.dsl_path.contains("/protocols/http/request"));
    assert_eq!(
        protocol_target_name_for_template_id("http_request_path"),
        Some("scan:http:request".to_string())
    );
}
