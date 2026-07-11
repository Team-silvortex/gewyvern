use super::*;

const REDIS_AUTH_REQUIRED_TARGET_NAME: &str = "scan:redis:auth-required";
const REDIS_AUTH_DENIED_TARGET_NAME: &str = "scan:redis:auth-denied";
const REDIS_ERROR_TARGET_NAME: &str = "scan:redis:error";
const REDIS_WRONGTYPE_TARGET_NAME: &str = "scan:redis:wrongtype";
const REDIS_BUSYGROUP_TARGET_NAME: &str = "scan:redis:busygroup";
const REDIS_READONLY_TARGET_NAME: &str = "scan:redis:readonly";
const REDIS_NOSCRIPT_TARGET_NAME: &str = "scan:redis:noscript";
const REDIS_MOVED_TARGET_NAME: &str = "scan:redis:moved";
const REDIS_ASK_TARGET_NAME: &str = "scan:redis:ask";
const REDIS_TRYAGAIN_TARGET_NAME: &str = "scan:redis:tryagain";
const REDIS_LOADING_TARGET_NAME: &str = "scan:redis:loading";
const REDIS_CROSSSLOT_TARGET_NAME: &str = "scan:redis:crossslot";
const REDIS_CLUSTERDOWN_TARGET_NAME: &str = "scan:redis:clusterdown";

#[test]
fn summary_json_carries_redis_auth_required_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/auth-required/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94001, 46001, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94001,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94001, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94001, 2, 3, 43079, 6379),
            redis_auth_request_fact(5, 94001),
            redis_response_fact(6, 94001, &[(0, 0x2d), (1, 0x4e), (2, 0x4f), (3, 0x41)]),
        ],
    );
    let json = summary_json(REDIS_AUTH_REQUIRED_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_auth_required\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"auth_required\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_auth_denied_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/auth-denied/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94002, 46002, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94002,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94002, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94002, 2, 3, 43079, 6379),
            redis_auth_request_fact(5, 94002),
            redis_response_fact(
                6,
                94002,
                &[(0, 0x2d), (1, 0x57), (2, 0x52), (3, 0x4f), (6, 0x50)],
            ),
        ],
    );
    let json = summary_json(REDIS_AUTH_DENIED_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_auth_denied\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"access_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_error_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/error/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94003, 46003, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94003,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94003, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94003, 2, 3, 43079, 6379),
            redis_get_request_fact(5, 94003),
            redis_response_fact(6, 94003, &[(0, 0x2d), (1, 0x45), (2, 0x52), (3, 0x52)]),
        ],
    );
    let json = summary_json(REDIS_ERROR_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_error\"]"),
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
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_wrongtype_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/wrongtype/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94004, 46004, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94004,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94004, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94004, 2, 3, 43079, 6379),
            redis_get_request_fact(5, 94004),
            redis_response_fact(
                6,
                94004,
                &[(0, 0x2d), (1, 0x57), (2, 0x52), (3, 0x4f), (6, 0x54)],
            ),
        ],
    );
    let json = summary_json(REDIS_WRONGTYPE_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_wrongtype\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"semantic_error\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"protocol_constraint_violation\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_busygroup_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/busygroup/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94005, 46005, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94005,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94005, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94005, 2, 3, 43079, 6379),
            redis_xgroup_request_fact(5, 94005),
            redis_response_fact(6, 94005, &[(0, 0x2d), (1, 0x42), (2, 0x55), (3, 0x53)]),
        ],
    );
    let json = summary_json(REDIS_BUSYGROUP_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_busygroup\"]"),
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
fn summary_json_carries_redis_readonly_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/readonly/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94006, 46006, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94006,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94006, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94006, 2, 3, 43079, 6379),
            redis_set_request_fact(5, 94006),
            redis_response_fact(6, 94006, &[(0, 0x2d), (1, 0x52), (2, 0x45), (3, 0x41)]),
        ],
    );
    let json = summary_json(REDIS_READONLY_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_readonly\"]"),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_mode\":\"server_denied\""),
        "json={}",
        json
    );
    assert!(
        json.contains("\"primary_failure_detail\":\"access_denied\""),
        "json={}",
        json
    );
}

#[test]
fn summary_json_carries_redis_noscript_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/noscript/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94007, 46007, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94007,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94007, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94007, 2, 3, 43079, 6379),
            redis_evalsha_request_fact(5, 94007),
            redis_response_fact(6, 94007, &[(0, 0x2d), (1, 0x4e), (2, 0x4f), (3, 0x53)]),
        ],
    );
    let json = summary_json(REDIS_NOSCRIPT_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_noscript\"]"),
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
fn summary_json_carries_redis_moved_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/moved/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94008, 46008, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94008,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94008, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94008, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94008),
            redis_response_fact(6, 94008, &[(0, 0x2d), (1, 0x4d), (2, 0x4f), (3, 0x56)]),
        ],
    );
    let json = summary_json(REDIS_MOVED_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_moved\"]"),
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
fn summary_json_carries_redis_ask_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/ask/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94009, 46009, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94009,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94009, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94009, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94009),
            redis_response_fact(6, 94009, &[(0, 0x2d), (1, 0x41), (2, 0x53), (3, 0x4b)]),
        ],
    );
    let json = summary_json(REDIS_ASK_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_ask\"]"),
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
fn summary_json_carries_redis_tryagain_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/tryagain/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94010, 46010, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94010,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94010, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94010, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94010),
            redis_response_fact(6, 94010, &[(0, 0x2d), (1, 0x54), (2, 0x52), (3, 0x59)]),
        ],
    );
    let json = summary_json(REDIS_TRYAGAIN_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_tryagain\"]"),
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
fn summary_json_carries_redis_loading_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/loading/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94011, 46011, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94011,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94011, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94011, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94011),
            redis_response_fact(6, 94011, &[(0, 0x2d), (1, 0x4c), (2, 0x4f), (3, 0x41)]),
        ],
    );
    let json = summary_json(REDIS_LOADING_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_loading\"]"),
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
fn summary_json_carries_redis_crossslot_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/crossslot/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94012, 46012, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94012,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94012, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94012, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94012),
            redis_response_fact(6, 94012, &[(0, 0x2d), (1, 0x43), (2, 0x52), (3, 0x4f)]),
        ],
    );
    let json = summary_json(REDIS_CROSSSLOT_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_crossslot\"]"),
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
fn summary_json_carries_redis_clusterdown_detail() {
    let export = redis_error_export(
        &protocol_fixture_path("redis/clusterdown/main.gewy"),
        vec![
            sock_lineage_fact_for_tests(1, 94013, 46013, "redis-cli"),
            route_fact(
                2,
                SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                94013,
                7,
                SessionId(1),
            ),
            tcp_state_fact_with_ports_for_tests(3, 94013, 1, 2, 43079, 6379),
            tcp_state_fact_with_ports_for_tests(4, 94013, 2, 3, 43079, 6379),
            redis_cluster_error_request_fact(5, 94013),
            redis_response_fact(6, 94013, &[(0, 0x2d), (1, 0x43), (2, 0x4c), (3, 0x55)]),
        ],
    );
    let json = summary_json(REDIS_CLUSTERDOWN_TARGET_NAME, &export);
    assert!(
        json.contains("\"operations\":[\"redis_clusterdown\"]"),
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
