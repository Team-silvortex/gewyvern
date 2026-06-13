use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

#[test]
fn redis_blpop_registry_entry_resolves_to_packaged_blpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blpop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-pop-left")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-blocking-pop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blpop".to_string())
    );
}

#[test]
fn redis_brpop_registry_entry_resolves_to_packaged_brpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("brpop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-pop-right")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-blocking-pop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpop".to_string())
    );
}

#[test]
fn redis_bzpopmin_registry_entry_resolves_to_packaged_bzpopmin_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzpopmin")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmin".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-pop-min")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmin".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-lowest")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmin".to_string())
    );
}

#[test]
fn redis_bzpopmax_registry_entry_resolves_to_packaged_bzpopmax_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzpopmax")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmax".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-pop-max")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmax".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-highest")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzpopmax".to_string())
    );
}

#[test]
fn redis_bzmpop_registry_entry_resolves_to_packaged_bzmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("bzmpop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-blocking-multi-pop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-blocking-pop-many")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/bzmpop".to_string())
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
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_blpop_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_blpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_brpop_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_brpop_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_brpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzpopmin_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_bzpopmin_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_bzpopmin_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzpopmax_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_bzpopmax_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_bzpopmax_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_bzmpop_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_bzmpop_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_bzmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
