use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

#[test]
fn redis_publish_registry_entry_resolves_to_packaged_publish_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("publish")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/publish".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("pubsub-send")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/publish".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("channel-write")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/publish".to_string())
    );
}

#[test]
fn redis_subscribe_registry_entry_resolves_to_packaged_subscribe_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("subscribe")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/subscribe".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("pubsub-listen")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/subscribe".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("channel-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/subscribe".to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_pubsub_surface_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"publish".to_string()));
    assert!(entries.contains(&"subscribe".to_string()));
}

#[test]
fn redis_publish_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_publish_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_publish_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_subscribe_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_subscribe_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_subscribe_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
