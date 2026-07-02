use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

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
fn redis_publish_registry_entry_resolves_to_packaged_publish_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("publish")),
        Some(protocol_fixture_path("redis/publish").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("pubsub-send")),
        Some(protocol_fixture_path("redis/publish").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("channel-write")),
        Some(protocol_fixture_path("redis/publish").to_string())
    );
}

#[test]
fn redis_subscribe_registry_entry_resolves_to_packaged_subscribe_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("subscribe")),
        Some(protocol_fixture_path("redis/subscribe").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("pubsub-listen")),
        Some(protocol_fixture_path("redis/subscribe").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("channel-read")),
        Some(protocol_fixture_path("redis/subscribe").to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_publish_subscribe_surface_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"publish".to_string()));
    assert!(entries.contains(&"subscribe".to_string()));

    let publish = protocol_surface("redis", "publish").expect("redis publish surface should exist");
    assert_eq!(
        publish.shelf.expect("redis publish shelf should exist").key,
        "publish"
    );

    let subscribe =
        protocol_surface("redis", "subscribe").expect("redis subscribe surface should exist");
    assert_eq!(
        subscribe
            .shelf
            .expect("redis subscribe shelf should exist")
            .key,
        "subscribe"
    );
}

#[test]
fn redis_publish_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_publish_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_publish_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_subscribe_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_subscribe_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_subscribe_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
