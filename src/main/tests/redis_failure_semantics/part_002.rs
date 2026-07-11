use super::*;

const REDIS_MASTERDOWN_TARGET_NAME: &str = "scan:redis:masterdown";
const REDIS_OOM_TARGET_NAME: &str = "scan:redis:oom";
const REDIS_BUSY_TARGET_NAME: &str = "scan:redis:busy";
const REDIS_EXECABORT_TARGET_NAME: &str = "scan:redis:execabort";
const REDIS_MISCONF_TARGET_NAME: &str = "scan:redis:misconf";

#[test]
fn summary_json_carries_redis_masterdown_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/masterdown/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94014, 46014, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94014,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94014, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94014, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94014),
            redis_response_fact(6, 94014, &[(0, 0x2d), (1, 0x4d), (2, 0x41), (3, 0x53)]),
        ],
    );
    let json = summary_json(REDIS_MASTERDOWN_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_masterdown\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_oom_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/oom/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94015, 46015, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94015,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94015, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94015, 2, 3, 43079, 6379),
            redis_write_error_request_fact(5, 94015),
            redis_response_fact(6, 94015, &[(0, 0x2d), (1, 0x4f), (2, 0x4f), (3, 0x4d)]),
        ],
    );
    let json = summary_json(REDIS_OOM_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_oom\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_busy_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/busy/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94016, 46016, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94016,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94016, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94016, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94016),
            redis_response_fact(6, 94016, &[(0, 0x2d), (1, 0x42), (2, 0x55), (3, 0x53)]),
        ],
    );
    let json = summary_json(REDIS_BUSY_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_busy\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_execabort_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/execabort/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94017, 46017, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94017,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94017, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94017, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94017),
            redis_response_fact(6, 94017, &[(0, 0x2d), (1, 0x45), (2, 0x58), (3, 0x45)]),
        ],
    );
    let json = summary_json(REDIS_EXECABORT_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_execabort\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_misconf_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/misconf/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94018, 46018, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94018,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94018, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94018, 2, 3, 43079, 6379),
            redis_write_error_request_fact(5, 94018),
            redis_response_fact(6, 94018, &[(0, 0x2d), (1, 0x4d), (2, 0x49), (3, 0x53)]),
        ],
    );
    let json = summary_json(REDIS_MISCONF_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_misconf\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_error\""),
        "json={}",
        json
    );
}
