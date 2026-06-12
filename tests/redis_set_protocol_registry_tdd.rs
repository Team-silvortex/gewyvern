use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{protocol_default_entry, protocol_dsl_path, protocol_entries};

#[test]
fn redis_sadd_registry_entry_resolves_to_packaged_sadd_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("sadd")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/sadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("set-add")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/sadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("member-add")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/sadd".to_string())
    );
}

#[test]
fn redis_smembers_registry_entry_resolves_to_packaged_smembers_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("smembers")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/smembers".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("set-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/smembers".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("members-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/smembers".to_string())
    );
}

#[test]
fn redis_zadd_registry_entry_resolves_to_packaged_zadd_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zadd")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-add")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zadd".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-add")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zadd".to_string())
    );
}

#[test]
fn redis_zrange_registry_entry_resolves_to_packaged_zrange_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrange")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrange".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrange".to_string())
    );
}

#[test]
fn redis_zrem_registry_entry_resolves_to_packaged_zrem_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrem")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrem".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-remove")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrem".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-remove")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrem".to_string())
    );
}

#[test]
fn redis_zcard_registry_entry_resolves_to_packaged_zcard_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zcard")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcard".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-count")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcard".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-count")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcard".to_string())
    );
}

#[test]
fn redis_zscore_registry_entry_resolves_to_packaged_zscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zscore")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-score")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-read-member")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zscore".to_string())
    );
}

#[test]
fn redis_zrank_registry_entry_resolves_to_packaged_zrank_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrank")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrank".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-rank")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrank".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-rank-member")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrank".to_string())
    );
}

#[test]
fn redis_zrevrank_registry_entry_resolves_to_packaged_zrevrank_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrevrank")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrank".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-member-revrank")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrank".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-revrank-member")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrank".to_string())
    );
}

#[test]
fn redis_zrangebyscore_registry_entry_resolves_to_packaged_zrangebyscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrangebyscore")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrangebyscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-range-score")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrangebyscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-read")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrangebyscore".to_string())
    );
}

#[test]
fn redis_zrevrangebyscore_registry_entry_resolves_to_packaged_zrevrangebyscore_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zrevrangebyscore")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrangebyscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-revrange-score")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrangebyscore".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-read-reverse")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zrevrangebyscore".to_string())
    );
}

#[test]
fn redis_zincrby_registry_entry_resolves_to_packaged_zincrby_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zincrby")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zincrby".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-score-increment")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zincrby".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-bump")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zincrby".to_string())
    );
}

#[test]
fn redis_zcount_registry_entry_resolves_to_packaged_zcount_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zcount")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcount".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-range-count")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcount".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-window-count")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zcount".to_string())
    );
}

#[test]
fn redis_zpopmin_registry_entry_resolves_to_packaged_zpopmin_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zpopmin")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmin".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-pop-min")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmin".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-pop-lowest")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmin".to_string())
    );
}

#[test]
fn redis_zpopmax_registry_entry_resolves_to_packaged_zpopmax_path() {
    assert_eq!(
        protocol_dsl_path("redis", Some("zpopmax")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmax".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("sorted-pop-max")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmax".to_string())
    );
    assert_eq!(
        protocol_dsl_path("redis", Some("score-pop-highest")),
        Some("/Users/Shared/chroot/dev/gewyvern/protocols/redis/zpopmax".to_string())
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
}

#[test]
fn redis_sadd_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_sadd_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_sadd_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_smembers_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_smembers_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_smembers_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zadd_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zadd_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zadd_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrange_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrange_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zrange_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrem_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrem_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zrem_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zcard_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zcard_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zcard_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zscore_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zscore_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrank_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrank_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zrank_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrevrank_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrevrank_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zrevrank_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrangebyscore_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrangebyscore_path.gewy")
            .unwrap();
    assert_eq!(binding.template.id, "redis_zrangebyscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zrevrangebyscore_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zrevrangebyscore_path.gewy")
            .unwrap();
    assert_eq!(binding.template.id, "redis_zrevrangebyscore_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zincrby_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zincrby_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zincrby_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zcount_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zcount_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zcount_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zpopmin_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zpopmin_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zpopmin_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}

#[test]
fn redis_zpopmax_dsl_compiles_into_template_binding() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/redis_zpopmax_path.gewy").unwrap();
    assert_eq!(binding.template.id, "redis_zpopmax_path");
    assert_eq!(binding.template.fragment_set.len(), 4);
}
