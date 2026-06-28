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
fn redis_session_registry_entry_resolves_to_packaged_session_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("session")),
        Some(protocol_fixture_path("redis/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("connect")),
        Some(protocol_fixture_path("redis/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("health")),
        Some(protocol_fixture_path("redis/ping").to_string())
    );
}

#[test]
fn redis_set_registry_entry_resolves_to_packaged_set_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("set")),
        Some(protocol_fixture_path("redis/set").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("write")),
        Some(protocol_fixture_path("redis/set").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("kv-write")),
        Some(protocol_fixture_path("redis/set").to_string())
    );
}

#[test]
fn redis_get_registry_entry_resolves_to_packaged_get_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("get")),
        Some(protocol_fixture_path("redis/get").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("read")),
        Some(protocol_fixture_path("redis/get").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("kv-read")),
        Some(protocol_fixture_path("redis/get").to_string())
    );
}

#[test]
fn redis_del_registry_entry_resolves_to_packaged_del_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("del")),
        Some(protocol_fixture_path("redis/del").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("delete")),
        Some(protocol_fixture_path("redis/del").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("remove")),
        Some(protocol_fixture_path("redis/del").to_string())
    );
}

#[test]
fn redis_incr_registry_entry_resolves_to_packaged_incr_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("incr")),
        Some(protocol_fixture_path("redis/incr").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("increment")),
        Some(protocol_fixture_path("redis/incr").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("count-up")),
        Some(protocol_fixture_path("redis/incr").to_string())
    );
}

#[test]
fn redis_decr_registry_entry_resolves_to_packaged_decr_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("decr")),
        Some(protocol_fixture_path("redis/decr").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("decrement")),
        Some(protocol_fixture_path("redis/decr").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("count-down")),
        Some(protocol_fixture_path("redis/decr").to_string())
    );
}

#[test]
fn redis_mget_registry_entry_resolves_to_packaged_mget_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("mget")),
        Some(protocol_fixture_path("redis/mget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("multi-read")),
        Some(protocol_fixture_path("redis/mget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("bulk-read")),
        Some(protocol_fixture_path("redis/mget").to_string())
    );
}

#[test]
fn redis_mset_registry_entry_resolves_to_packaged_mset_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("mset")),
        Some(protocol_fixture_path("redis/mset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("multi-write")),
        Some(protocol_fixture_path("redis/mset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("bulk-write")),
        Some(protocol_fixture_path("redis/mset").to_string())
    );
}

#[test]
fn redis_exists_registry_entry_resolves_to_packaged_exists_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("exists")),
        Some(protocol_fixture_path("redis/exists").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("present")),
        Some(protocol_fixture_path("redis/exists").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("key-check")),
        Some(protocol_fixture_path("redis/exists").to_string())
    );
}

#[test]
fn redis_expire_registry_entry_resolves_to_packaged_expire_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("expire")),
        Some(protocol_fixture_path("redis/expire").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("set-ttl")),
        Some(protocol_fixture_path("redis/expire").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("expiry")),
        Some(protocol_fixture_path("redis/expire").to_string())
    );
}

#[test]
fn redis_ttl_registry_entry_resolves_to_packaged_ttl_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("ttl")),
        Some(protocol_fixture_path("redis/ttl").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("time-to-live")),
        Some(protocol_fixture_path("redis/ttl").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("key-ttl")),
        Some(protocol_fixture_path("redis/ttl").to_string())
    );
}

#[test]
fn redis_pttl_registry_entry_resolves_to_packaged_pttl_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("pttl")),
        Some(protocol_fixture_path("redis/pttl").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("precise-ttl")),
        Some(protocol_fixture_path("redis/pttl").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("ms-ttl")),
        Some(protocol_fixture_path("redis/pttl").to_string())
    );
}

#[test]
fn redis_hget_registry_entry_resolves_to_packaged_hget_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("hget")),
        Some(protocol_fixture_path("redis/hget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("hash-read")),
        Some(protocol_fixture_path("redis/hget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("field-read")),
        Some(protocol_fixture_path("redis/hget").to_string())
    );
}

#[test]
fn redis_hset_registry_entry_resolves_to_packaged_hset_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("hset")),
        Some(protocol_fixture_path("redis/hset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("hash-write")),
        Some(protocol_fixture_path("redis/hset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("field-write")),
        Some(protocol_fixture_path("redis/hset").to_string())
    );
}

#[test]
fn redis_hmget_registry_entry_resolves_to_packaged_hmget_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("hmget")),
        Some(protocol_fixture_path("redis/hmget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("hash-multi-read")),
        Some(protocol_fixture_path("redis/hmget").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("fields-read")),
        Some(protocol_fixture_path("redis/hmget").to_string())
    );
}

#[test]
fn redis_hmset_registry_entry_resolves_to_packaged_hmset_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("hmset")),
        Some(protocol_fixture_path("redis/hmset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("hash-multi-write")),
        Some(protocol_fixture_path("redis/hmset").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("fields-write")),
        Some(protocol_fixture_path("redis/hmset").to_string())
    );
}

#[test]
fn redis_lpush_registry_entry_resolves_to_packaged_lpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lpush")),
        Some(protocol_fixture_path("redis/lpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-prepend")),
        Some(protocol_fixture_path("redis/lpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-push")),
        Some(protocol_fixture_path("redis/lpush").to_string())
    );
}

#[test]
fn redis_rpush_registry_entry_resolves_to_packaged_rpush_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("rpush")),
        Some(protocol_fixture_path("redis/rpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-append")),
        Some(protocol_fixture_path("redis/rpush").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-push")),
        Some(protocol_fixture_path("redis/rpush").to_string())
    );
}

#[test]
fn redis_lpop_registry_entry_resolves_to_packaged_lpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("lpop")),
        Some(protocol_fixture_path("redis/lpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-pop-left")),
        Some(protocol_fixture_path("redis/lpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("left-pop")),
        Some(protocol_fixture_path("redis/lpop").to_string())
    );
}

#[test]
fn redis_rpop_registry_entry_resolves_to_packaged_rpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("rpop")),
        Some(protocol_fixture_path("redis/rpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("list-pop-right")),
        Some(protocol_fixture_path("redis/rpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("right-pop")),
        Some(protocol_fixture_path("redis/rpop").to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_surface_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"ping".to_string()));
    assert!(entries.contains(&"session".to_string()));
    assert!(entries.contains(&"set".to_string()));
    assert!(entries.contains(&"get".to_string()));
    assert!(entries.contains(&"del".to_string()));
    assert!(entries.contains(&"incr".to_string()));
    assert!(entries.contains(&"decr".to_string()));
    assert!(entries.contains(&"mget".to_string()));
    assert!(entries.contains(&"mset".to_string()));
    assert!(entries.contains(&"exists".to_string()));
    assert!(entries.contains(&"expire".to_string()));
    assert!(entries.contains(&"ttl".to_string()));
    assert!(entries.contains(&"pttl".to_string()));
    assert!(entries.contains(&"hget".to_string()));
    assert!(entries.contains(&"hset".to_string()));
    assert!(entries.contains(&"hmget".to_string()));
    assert!(entries.contains(&"hmset".to_string()));
    assert!(entries.contains(&"lpush".to_string()));
    assert!(entries.contains(&"rpush".to_string()));
    assert!(entries.contains(&"lpop".to_string()));
    assert!(entries.contains(&"rpop".to_string()));
}

#[test]
fn redis_session_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_session_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_session_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_set_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_set_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_set_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_get_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_get_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_get_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_del_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_del_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_del_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_incr_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_incr_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_incr_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_decr_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_decr_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_decr_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_mget_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_mget_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_mget_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_mset_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_mset_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_mset_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_exists_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_exists_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_exists_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_expire_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_expire_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_expire_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_ttl_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_ttl_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_ttl_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_pttl_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_pttl_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_pttl_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_hget_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_hget_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_hget_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_hset_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_hset_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_hset_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_hmget_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_hmget_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_hmget_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_hmset_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_hmset_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_hmset_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lpush_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_lpush_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_lpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_rpush_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_rpush_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_rpush_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_lpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_lpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_lpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_rpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_rpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_rpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
