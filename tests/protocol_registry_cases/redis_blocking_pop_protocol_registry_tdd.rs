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
fn redis_blpop_registry_entry_resolves_to_packaged_blpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blpop")),
        Some(protocol_fixture_path("redis/blpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-pop-left")),
        Some(protocol_fixture_path("redis/blpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-blocking-pop")),
        Some(protocol_fixture_path("redis/blpop").to_string())
    );
}

#[test]
fn redis_brpop_registry_entry_resolves_to_packaged_brpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("brpop")),
        Some(protocol_fixture_path("redis/brpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-pop-right")),
        Some(protocol_fixture_path("redis/brpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-blocking-pop")),
        Some(protocol_fixture_path("redis/brpop").to_string())
    );
}

#[test]
fn redis_bzpopmin_registry_entry_resolves_to_packaged_bzpopmin_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzpopmin")),
        Some(protocol_fixture_path("redis/bzpopmin").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-pop-min")),
        Some(protocol_fixture_path("redis/bzpopmin").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-lowest")),
        Some(protocol_fixture_path("redis/bzpopmin").to_string())
    );
}

#[test]
fn redis_bzpopmax_registry_entry_resolves_to_packaged_bzpopmax_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzpopmax")),
        Some(protocol_fixture_path("redis/bzpopmax").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-pop-max")),
        Some(protocol_fixture_path("redis/bzpopmax").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-highest")),
        Some(protocol_fixture_path("redis/bzpopmax").to_string())
    );
}

#[test]
fn redis_bzmpop_registry_entry_resolves_to_packaged_bzmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzmpop")),
        Some(protocol_fixture_path("redis/bzmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-multi-pop")),
        Some(protocol_fixture_path("redis/bzmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-many")),
        Some(protocol_fixture_path("redis/bzmpop").to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_blocking_pop_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"blpop".to_string()));
    assert!(entries.contains(&"brpop".to_string()));
    assert!(entries.contains(&"bzpopmin".to_string()));
    assert!(entries.contains(&"bzpopmax".to_string()));
    assert!(entries.contains(&"bzmpop".to_string()));
    assert!(entries.contains(&"ping".to_string()));
}

#[test]
fn redis_blpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_blpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_blpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_brpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_brpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_brpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzpopmin_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_bzpopmin_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_bzpopmin_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzpopmax_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_bzpopmax_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_bzpopmax_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzmpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_bzmpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_bzmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
