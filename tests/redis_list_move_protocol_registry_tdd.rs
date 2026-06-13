use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

#[test]
fn redis_rpoplpush_registry_entry_resolves_to_packaged_rpoplpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("rpoplpush")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/rpoplpush".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-move-right-to-left")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/rpoplpush".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-pop-left-push")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/rpoplpush".to_string())
    );
}

#[test]
fn redis_brpoplpush_registry_entry_resolves_to_packaged_brpoplpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("brpoplpush")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpoplpush".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-move-right-to-left")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpoplpush".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-blocking-pop-left-push")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/brpoplpush".to_string())
    );
}

#[test]
fn redis_lmove_registry_entry_resolves_to_packaged_lmove_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lmove")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-left-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-directional-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-right-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmove".to_string())
    );
}

#[test]
fn redis_blmove_registry_entry_resolves_to_packaged_blmove_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blmove")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-right-left-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-directional-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmove".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-left-right-move")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmove".to_string())
    );
}

#[test]
fn redis_lmpop_registry_entry_resolves_to_packaged_lmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lmpop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-multi-pop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-pop-many")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/lmpop".to_string())
    );
}

#[test]
fn redis_blmpop_registry_entry_resolves_to_packaged_blmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("blmpop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-blocking-multi-pop")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmpop".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("blocking-list-pop-many")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/blmpop".to_string())
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
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_rpoplpush_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_rpoplpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_brpoplpush_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_brpoplpush_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_brpoplpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lmove_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_lmove_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_lmove_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_blmove_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_blmove_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_blmove_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lmpop_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_lmpop_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_lmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_blmpop_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_blmpop_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_blmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
