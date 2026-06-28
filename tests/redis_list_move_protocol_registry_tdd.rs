use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}
#[test]
fn redis_rpoplpush_registry_entry_resolves_to_packaged_rpoplpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("rpoplpush")),
        Some(protocol_fixture_path("redis/rpoplpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-move-right-to-left")),
        Some(protocol_fixture_path("redis/rpoplpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-pop-left-push")),
        Some(protocol_fixture_path("redis/rpoplpush").to_string())
    );
}

#[test]
fn redis_brpoplpush_registry_entry_resolves_to_packaged_brpoplpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("brpoplpush")),
        Some(protocol_fixture_path("redis/brpoplpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-move-right-to-left")),
        Some(protocol_fixture_path("redis/brpoplpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-blocking-pop-left-push")),
        Some(protocol_fixture_path("redis/brpoplpush").to_string())
    );
}

#[test]
fn redis_lmove_registry_entry_resolves_to_packaged_lmove_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lmove")),
        Some(protocol_fixture_path("redis/lmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-move")),
        Some(protocol_fixture_path("redis/lmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-left-move")),
        Some(protocol_fixture_path("redis/lmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-directional-move")),
        Some(protocol_fixture_path("redis/lmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-right-move")),
        Some(protocol_fixture_path("redis/lmove").to_string())
    );
}

#[test]
fn redis_blmove_registry_entry_resolves_to_packaged_blmove_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blmove")),
        Some(protocol_fixture_path("redis/blmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-move")),
        Some(protocol_fixture_path("redis/blmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-right-left-move")),
        Some(protocol_fixture_path("redis/blmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-directional-move")),
        Some(protocol_fixture_path("redis/blmove").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-left-right-move")),
        Some(protocol_fixture_path("redis/blmove").to_string())
    );
}

#[test]
fn redis_lmpop_registry_entry_resolves_to_packaged_lmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lmpop")),
        Some(protocol_fixture_path("redis/lmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-multi-pop")),
        Some(protocol_fixture_path("redis/lmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-pop-many")),
        Some(protocol_fixture_path("redis/lmpop").to_string())
    );
}

#[test]
fn redis_blmpop_registry_entry_resolves_to_packaged_blmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blmpop")),
        Some(protocol_fixture_path("redis/blmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-multi-pop")),
        Some(protocol_fixture_path("redis/blmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-list-pop-many")),
        Some(protocol_fixture_path("redis/blmpop").to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_list_move_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"rpoplpush".to_string()));
    assert!(entries.contains(&"brpoplpush".to_string()));
    assert!(entries.contains(&"lmove".to_string()));
    assert!(entries.contains(&"blmove".to_string()));
    assert!(entries.contains(&"lmpop".to_string()));
    assert!(entries.contains(&"blmpop".to_string()));
    assert!(entries.contains(&"ping".to_string()));
}

#[test]
fn redis_rpoplpush_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_rpoplpush_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_rpoplpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_brpoplpush_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_brpoplpush_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_brpoplpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lmove_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_lmove_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_lmove_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_blmove_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_blmove_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_blmove_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lmpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_lmpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_lmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_blmpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_blmpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_blmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
