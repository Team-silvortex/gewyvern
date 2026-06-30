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
fn postgres_session_registry_entry_resolves_to_packaged_session_path() {
    assert_eq!(
        protocol_dsl_path("postgres", Some("session")),
        Some(protocol_fixture_path("postgres/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("postgres", Some("query-session")),
        Some(protocol_fixture_path("postgres/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("postgres", Some("auth-query")),
        Some(protocol_fixture_path("postgres/session").to_string())
    );
}

#[test]
fn postgres_default_entry_stays_query_after_session_addition() {
    assert_eq!(
        protocol_default_entry("postgres"),
        Some("query".to_string())
    );

    let entries = protocol_entries("postgres").expect("postgres entries should resolve");
    assert!(entries.contains(&"session".to_string()));
    assert!(entries.contains(&"query".to_string()));
}

#[test]
fn postgres_query_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("postgres_query_session.gewy")).unwrap();
    assert_eq!(binding.template.id, "postgres_query_session");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn memcached_miss_registry_entry_resolves_to_packaged_miss_path() {
    assert_eq!(
        protocol_dsl_path("memcached", Some("miss")),
        Some(protocol_fixture_path("memcached/miss").to_string())
    );
    assert_eq!(
        protocol_dsl_path("memcached", Some("cache-miss")),
        Some(protocol_fixture_path("memcached/miss").to_string())
    );
    assert_eq!(
        protocol_dsl_path("memcached-miss", None),
        Some(protocol_fixture_path("memcached/miss").to_string())
    );
}

#[test]
fn memcached_miss_surface_stays_in_get_shelf() {
    let surface =
        protocol_surface("memcached", "miss").expect("memcached miss surface should exist");
    let shelf = surface.shelf.expect("memcached miss shelf should exist");
    assert_eq!(shelf.key, "get");
    assert!(shelf.entries.contains(&"get".to_string()));
    assert!(shelf.entries.contains(&"miss".to_string()));
    assert_eq!(
        surface
            .entry_semantics
            .expect("memcached miss semantics should exist")
            .category,
        "cache-miss-path"
    );
}

#[test]
fn memcached_miss_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("memcached_miss_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "memcached_miss_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn memcached_not_stored_registry_entry_resolves_to_packaged_path() {
    assert_eq!(
        protocol_dsl_path("memcached", Some("not-stored")),
        Some(protocol_fixture_path("memcached/not-stored").to_string())
    );
    assert_eq!(
        protocol_dsl_path("memcached", Some("store-miss")),
        Some(protocol_fixture_path("memcached/not-stored").to_string())
    );
    assert_eq!(
        protocol_dsl_path("memcached-not-stored", None),
        Some(protocol_fixture_path("memcached/not-stored").to_string())
    );
}

#[test]
fn memcached_not_stored_surface_stays_in_set_shelf() {
    let surface = protocol_surface("memcached", "not-stored")
        .expect("memcached not-stored surface should exist");
    let shelf = surface
        .shelf
        .expect("memcached not-stored shelf should exist");
    assert_eq!(shelf.key, "set");
    assert!(shelf.entries.contains(&"set".to_string()));
    assert!(shelf.entries.contains(&"not-stored".to_string()));
    assert_eq!(
        surface
            .entry_semantics
            .expect("memcached not-stored semantics should exist")
            .category,
        "cache-not-stored-path"
    );
}

#[test]
fn memcached_not_stored_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("memcached_not_stored_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "memcached_not_stored_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_publish_registry_entry_resolves_to_packaged_publish_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("publish")),
        Some(protocol_fixture_path("mqtt/publish").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("send")),
        Some(protocol_fixture_path("mqtt/publish").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("message")),
        Some(protocol_fixture_path("mqtt/publish").to_string())
    );
}

#[test]
fn mqtt_subscribe_registry_entry_resolves_to_packaged_subscribe_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("subscribe")),
        Some(protocol_fixture_path("mqtt/subscribe").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("read")),
        Some(protocol_fixture_path("mqtt/subscribe").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("listen")),
        Some(protocol_fixture_path("mqtt/subscribe").to_string())
    );
}

#[test]
fn mqtt_connack_registry_entry_resolves_to_packaged_connack_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("connack")),
        Some(protocol_fixture_path("mqtt/connack").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("connect-ack")),
        Some(protocol_fixture_path("mqtt/connack").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("broker-ack")),
        Some(protocol_fixture_path("mqtt/connack").to_string())
    );
}

#[test]
fn mqtt_disconnect_registry_entry_resolves_to_packaged_disconnect_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("disconnect")),
        Some(protocol_fixture_path("mqtt/disconnect").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("close")),
        Some(protocol_fixture_path("mqtt/disconnect").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("teardown")),
        Some(protocol_fixture_path("mqtt/disconnect").to_string())
    );
}

#[test]
fn mqtt_pubrel_registry_entry_resolves_to_packaged_pubrel_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("pubrel")),
        Some(protocol_fixture_path("mqtt/pubrel").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("qos2-release")),
        Some(protocol_fixture_path("mqtt/pubrel").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("resume")),
        Some(protocol_fixture_path("mqtt/pubrel").to_string())
    );
}

#[test]
fn mqtt_pubrec_registry_entry_resolves_to_packaged_pubrec_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("pubrec")),
        Some(protocol_fixture_path("mqtt/pubrec").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("qos2-receipt")),
        Some(protocol_fixture_path("mqtt/pubrec").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("stage-2")),
        Some(protocol_fixture_path("mqtt/pubrec").to_string())
    );
}

#[test]
fn mqtt_pubcomp_registry_entry_resolves_to_packaged_pubcomp_path() {
    assert_eq!(
        protocol_dsl_path("mqtt", Some("pubcomp")),
        Some(protocol_fixture_path("mqtt/pubcomp").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("qos2-complete")),
        Some(protocol_fixture_path("mqtt/pubcomp").to_string())
    );
    assert_eq!(
        protocol_dsl_path("mqtt", Some("complete")),
        Some(protocol_fixture_path("mqtt/pubcomp").to_string())
    );
}

#[test]
fn mqtt_default_entry_stays_connect_after_surface_additions() {
    assert_eq!(protocol_default_entry("mqtt"), Some("connect".to_string()));

    let entries = protocol_entries("mqtt").expect("mqtt entries should resolve");
    assert!(entries.contains(&"connect".to_string()));
    assert!(entries.contains(&"connack".to_string()));
    assert!(entries.contains(&"publish".to_string()));
    assert!(entries.contains(&"subscribe".to_string()));
    assert!(entries.contains(&"disconnect".to_string()));
    assert!(entries.contains(&"pubrel".to_string()));
    assert!(entries.contains(&"pubrec".to_string()));
    assert!(entries.contains(&"pubcomp".to_string()));
}

#[test]
fn mqtt_connack_surface_stays_in_session_shelf() {
    let surface = protocol_surface("mqtt", "connack").expect("mqtt connack surface should exist");
    assert_eq!(
        surface.shelf.expect("mqtt connack shelf should exist").key,
        "session"
    );
    assert_eq!(
        surface
            .entry_semantics
            .expect("mqtt connack semantics should exist")
            .category,
        "broker-acknowledgement-path"
    );
}

#[test]
fn mqtt_publish_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_publish_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_publish_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_connack_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_connack_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_connack_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_subscribe_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_subscribe_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_subscribe_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_disconnect_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_disconnect_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_disconnect_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_pubrel_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_pubrel_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_pubrel_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_pubrec_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_pubrec_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_pubrec_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn mqtt_pubcomp_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("mqtt_pubcomp_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "mqtt_pubcomp_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn sip_invite_registry_entry_resolves_to_packaged_invite_path() {
    assert_eq!(
        protocol_dsl_path("sip", Some("invite")),
        Some(protocol_fixture_path("sip/invite").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("call")),
        Some(protocol_fixture_path("sip/invite").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("session")),
        Some(protocol_fixture_path("sip/invite").to_string())
    );
}

#[test]
fn sip_bye_registry_entry_resolves_to_packaged_bye_path() {
    assert_eq!(
        protocol_dsl_path("sip", Some("bye")),
        Some(protocol_fixture_path("sip/bye").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("hangup")),
        Some(protocol_fixture_path("sip/bye").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("terminate")),
        Some(protocol_fixture_path("sip/bye").to_string())
    );
}

#[test]
fn sip_default_entry_stays_register_after_invite_addition() {
    assert_eq!(protocol_default_entry("sip"), Some("register".to_string()));

    let entries = protocol_entries("sip").expect("sip entries should resolve");
    assert!(entries.contains(&"register".to_string()));
    assert!(entries.contains(&"invite".to_string()));
    assert!(entries.contains(&"bye".to_string()));
}

#[test]
fn sip_invite_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("sip_invite_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "sip_invite_path");
    assert_eq!(binding.template.fragment_set.len(), 3);
}

#[test]
fn sip_bye_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("sip_bye_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "sip_bye_path");
    assert_eq!(binding.template.fragment_set.len(), 3);
}

#[test]
fn rtsp_play_registry_entry_resolves_to_packaged_play_path() {
    assert_eq!(
        protocol_dsl_path("rtsp", Some("play")),
        Some(protocol_fixture_path("rtsp/play").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("start")),
        Some(protocol_fixture_path("rtsp/play").to_string())
    );
    assert_eq!(
        protocol_dsl_path("rtsp", Some("stream")),
        Some(protocol_fixture_path("rtsp/setup").to_string())
    );
}

#[test]
fn rtsp_default_entry_stays_options_after_play_addition() {
    assert_eq!(protocol_default_entry("rtsp"), Some("options".to_string()));

    let entries = protocol_entries("rtsp").expect("rtsp entries should resolve");
    assert!(entries.contains(&"options".to_string()));
    assert!(entries.contains(&"describe".to_string()));
    assert!(entries.contains(&"setup".to_string()));
    assert!(entries.contains(&"play".to_string()));
}

#[test]
fn rtsp_play_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("rtsp_play_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "rtsp_play_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn amqp_consume_registry_entry_resolves_to_packaged_consume_path() {
    assert_eq!(
        protocol_dsl_path("amqp", Some("consume")),
        Some(protocol_fixture_path("amqp/consume").to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("receive")),
        Some(protocol_fixture_path("amqp/consume").to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", Some("deliver")),
        Some(protocol_fixture_path("amqp/consume").to_string())
    );
}

#[test]
fn amqp_default_entry_stays_session_after_consume_addition() {
    assert_eq!(protocol_default_entry("amqp"), Some("session".to_string()));

    let entries = protocol_entries("amqp").expect("amqp entries should resolve");
    assert!(entries.contains(&"start".to_string()));
    assert!(entries.contains(&"session".to_string()));
    assert!(entries.contains(&"publish".to_string()));
    assert!(entries.contains(&"consume".to_string()));
}

#[test]
fn amqp_consume_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("amqp_basic_consume_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "amqp_basic_consume_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
