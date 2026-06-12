use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

#[test]
fn redis_xadd_registry_entry_resolves_to_packaged_xadd_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xadd")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-append")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-write")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xadd".to_string())
    );
}

#[test]
fn redis_xread_registry_entry_resolves_to_packaged_xread_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xread")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xread".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xread".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-consume")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xread".to_string())
    );
}

#[test]
fn redis_xrange_registry_entry_resolves_to_packaged_xrange_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xrange")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-range")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-history")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrange".to_string())
    );
}

#[test]
fn redis_xrevrange_registry_entry_resolves_to_packaged_xrevrange_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xrevrange")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrevrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-range-reverse")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrevrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-history-reverse")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xrevrange".to_string())
    );
}

#[test]
fn redis_xdel_registry_entry_resolves_to_packaged_xdel_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xdel")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xdel".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-delete")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xdel".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-prune-entry")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xdel".to_string())
    );
}

#[test]
fn redis_xtrim_registry_entry_resolves_to_packaged_xtrim_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xtrim")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xtrim".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-trim")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xtrim".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-prune")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xtrim".to_string())
    );
}

#[test]
fn redis_xlen_registry_entry_resolves_to_packaged_xlen_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("xlen")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xlen".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-length")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xlen".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("stream-count")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/xlen".to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_stream_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"xadd".to_string()));
    assert!(entries.contains(&"xread".to_string()));
    assert!(entries.contains(&"xrange".to_string()));
    assert!(entries.contains(&"xrevrange".to_string()));
    assert!(entries.contains(&"xdel".to_string()));
    assert!(entries.contains(&"xtrim".to_string()));
    assert!(entries.contains(&"xlen".to_string()));
    assert!(entries.contains(&"ping".to_string()));
}

#[test]
fn redis_xadd_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xadd_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xadd_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xread_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xread_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xread_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xrange_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xrange_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xrange_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xrevrange_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xrevrange_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xrevrange_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xdel_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xdel_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xdel_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xtrim_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xtrim_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xtrim_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_xlen_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_xlen_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_xlen_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
