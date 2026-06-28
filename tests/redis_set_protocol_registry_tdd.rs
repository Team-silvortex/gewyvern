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
fn redis_sadd_registry_entry_resolves_to_packaged_sadd_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("sadd")),
        Some(protocol_fixture_path("redis/sadd").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("set-add")),
        Some(protocol_fixture_path("redis/sadd").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("member-add")),
        Some(protocol_fixture_path("redis/sadd").to_string())
    );
}

#[test]
fn redis_smembers_registry_entry_resolves_to_packaged_smembers_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("smembers")),
        Some(protocol_fixture_path("redis/smembers").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("set-read")),
        Some(protocol_fixture_path("redis/smembers").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("members-read")),
        Some(protocol_fixture_path("redis/smembers").to_string())
    );
}

#[test]
fn redis_zadd_registry_entry_resolves_to_packaged_zadd_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zadd")),
        Some(protocol_fixture_path("redis/zadd").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-add")),
        Some(protocol_fixture_path("redis/zadd").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-add")),
        Some(protocol_fixture_path("redis/zadd").to_string())
    );
}

#[test]
fn redis_zrange_registry_entry_resolves_to_packaged_zrange_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrange")),
        Some(protocol_fixture_path("redis/zrange").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-read")),
        Some(protocol_fixture_path("redis/zrange").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-read")),
        Some(protocol_fixture_path("redis/zrange").to_string())
    );
}

#[test]
fn redis_zrem_registry_entry_resolves_to_packaged_zrem_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrem")),
        Some(protocol_fixture_path("redis/zrem").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-remove")),
        Some(protocol_fixture_path("redis/zrem").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-remove")),
        Some(protocol_fixture_path("redis/zrem").to_string())
    );
}

#[test]
fn redis_zcard_registry_entry_resolves_to_packaged_zcard_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zcard")),
        Some(protocol_fixture_path("redis/zcard").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-count")),
        Some(protocol_fixture_path("redis/zcard").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-count")),
        Some(protocol_fixture_path("redis/zcard").to_string())
    );
}

#[test]
fn redis_zscore_registry_entry_resolves_to_packaged_zscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zscore")),
        Some(protocol_fixture_path("redis/zscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-score")),
        Some(protocol_fixture_path("redis/zscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-read-member")),
        Some(protocol_fixture_path("redis/zscore").to_string())
    );
}

#[test]
fn redis_zrank_registry_entry_resolves_to_packaged_zrank_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrank")),
        Some(protocol_fixture_path("redis/zrank").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-rank")),
        Some(protocol_fixture_path("redis/zrank").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-rank-member")),
        Some(protocol_fixture_path("redis/zrank").to_string())
    );
}

#[test]
fn redis_zrevrank_registry_entry_resolves_to_packaged_zrevrank_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrevrank")),
        Some(protocol_fixture_path("redis/zrevrank").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-revrank")),
        Some(protocol_fixture_path("redis/zrevrank").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-revrank-member")),
        Some(protocol_fixture_path("redis/zrevrank").to_string())
    );
}

#[test]
fn redis_zrangebyscore_registry_entry_resolves_to_packaged_zrangebyscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrangebyscore")),
        Some(protocol_fixture_path("redis/zrangebyscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-range-score")),
        Some(protocol_fixture_path("redis/zrangebyscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-read")),
        Some(protocol_fixture_path("redis/zrangebyscore").to_string())
    );
}

#[test]
fn redis_zrevrangebyscore_registry_entry_resolves_to_packaged_zrevrangebyscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrevrangebyscore")),
        Some(protocol_fixture_path("redis/zrevrangebyscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-revrange-score")),
        Some(protocol_fixture_path("redis/zrevrangebyscore").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-read-reverse")),
        Some(protocol_fixture_path("redis/zrevrangebyscore").to_string())
    );
}

#[test]
fn redis_zincrby_registry_entry_resolves_to_packaged_zincrby_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zincrby")),
        Some(protocol_fixture_path("redis/zincrby").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-score-increment")),
        Some(protocol_fixture_path("redis/zincrby").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-bump")),
        Some(protocol_fixture_path("redis/zincrby").to_string())
    );
}

#[test]
fn redis_zcount_registry_entry_resolves_to_packaged_zcount_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zcount")),
        Some(protocol_fixture_path("redis/zcount").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-range-count")),
        Some(protocol_fixture_path("redis/zcount").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-count")),
        Some(protocol_fixture_path("redis/zcount").to_string())
    );
}

#[test]
fn redis_zpopmin_registry_entry_resolves_to_packaged_zpopmin_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zpopmin")),
        Some(protocol_fixture_path("redis/zpopmin").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-pop-min")),
        Some(protocol_fixture_path("redis/zpopmin").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-pop-lowest")),
        Some(protocol_fixture_path("redis/zpopmin").to_string())
    );
}

#[test]
fn redis_zpopmax_registry_entry_resolves_to_packaged_zpopmax_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zpopmax")),
        Some(protocol_fixture_path("redis/zpopmax").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-pop-max")),
        Some(protocol_fixture_path("redis/zpopmax").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-pop-highest")),
        Some(protocol_fixture_path("redis/zpopmax").to_string())
    );
}

#[test]
fn redis_zmpop_registry_entry_resolves_to_packaged_zmpop_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zmpop")),
        Some(protocol_fixture_path("redis/zmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-multi-pop")),
        Some(protocol_fixture_path("redis/zmpop").to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-pop-many")),
        Some(protocol_fixture_path("redis/zmpop").to_string())
    );
}

#[test]
fn redis_default_entry_stays_ping_after_set_surface_additions() {
    assert_eq!(protocol_default_entry("redis"), Some("ping".to_string()));

    let entries = protocol_entries("redis").expect("redis entries should resolve");
    assert!(entries.contains(&"sadd".to_string()));
    assert!(entries.contains(&"smembers".to_string()));
    assert!(entries.contains(&"zadd".to_string()));
    assert!(entries.contains(&"zrange".to_string()));
    assert!(entries.contains(&"zrem".to_string()));
    assert!(entries.contains(&"zcard".to_string()));
    assert!(entries.contains(&"zscore".to_string()));
    assert!(entries.contains(&"zrank".to_string()));
    assert!(entries.contains(&"zrevrank".to_string()));
    assert!(entries.contains(&"zrangebyscore".to_string()));
    assert!(entries.contains(&"zrevrangebyscore".to_string()));
    assert!(entries.contains(&"zincrby".to_string()));
    assert!(entries.contains(&"zcount".to_string()));
    assert!(entries.contains(&"zpopmin".to_string()));
    assert!(entries.contains(&"zpopmax".to_string()));
    assert!(entries.contains(&"zmpop".to_string()));
}

#[test]
fn redis_sadd_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_sadd_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_sadd_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_smembers_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_smembers_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_smembers_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zadd_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zadd_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zadd_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrange_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrange_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrange_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrem_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrem_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrem_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zcard_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zcard_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zcard_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zscore_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zscore_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrank_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrank_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrank_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrevrank_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrevrank_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrevrank_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrangebyscore_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrangebyscore_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrangebyscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrevrangebyscore_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zrevrangebyscore_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zrevrangebyscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zincrby_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zincrby_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zincrby_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zcount_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zcount_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zcount_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zpopmin_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zpopmin_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zpopmin_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zpopmax_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zpopmax_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zpopmax_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zmpop_dsl_compiles_into_template_binding() {
    let binding = compile_file(&dsl_fixture_path("redis_zmpop_path.gewy")).unwrap();
    assert_eq!(binding.template.id, "redis_zmpop_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
