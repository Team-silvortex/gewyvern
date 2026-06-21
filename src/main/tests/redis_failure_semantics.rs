use super::*;

fn redis_auth_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Egress,
        Some(43079),
        Some(6379),
        &[
            (0, 0x2a),
            (1, 0x32),
            (2, 0x0d),
            (3, 0x0a),
            (8, 0x41),
            (9, 0x55),
            (10, 0x54),
            (11, 0x48),
        ],
    )
}

fn redis_get_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Egress,
        Some(43079),
        Some(6379),
        &[
            (0, 0x2a),
            (1, 0x32),
            (2, 0x0d),
            (3, 0x0a),
            (8, 0x47),
            (9, 0x45),
            (10, 0x54),
        ],
    )
}

fn redis_response_fact(id: u64, cookie: u64, payload: &[(u16, u8)]) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Ingress,
        Some(43079),
        Some(6379),
        payload,
    )
}

fn redis_xgroup_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Egress,
        Some(43079),
        Some(6379),
        &[
            (0, 0x2a),
            (1, 0x35),
            (2, 0x0d),
            (3, 0x0a),
            (5, 0x36),
            (8, 0x58),
            (9, 0x47),
            (10, 0x52),
            (13, 0x50),
        ],
    )
}

fn redis_set_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Egress,
        Some(43079),
        Some(6379),
        &[
            (0, 0x2a),
            (1, 0x33),
            (2, 0x0d),
            (3, 0x0a),
            (8, 0x53),
            (9, 0x45),
            (10, 0x54),
        ],
    )
}

fn redis_evalsha_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    packet_fact_with_dir_and_payload_bytes_for_tests(
        id,
        cookie,
        0,
        PacketDir::Egress,
        Some(43079),
        Some(6379),
        &[
            (0, 0x2a),
            (1, 0x32),
            (2, 0x0d),
            (3, 0x0a),
            (8, 0x45),
            (9, 0x56),
            (10, 0x41),
            (11, 0x4c),
            (12, 0x53),
            (13, 0x48),
            (14, 0x41),
        ],
    )
}

fn redis_cluster_error_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    redis_get_request_fact(id, cookie)
}
fn redis_write_error_request_fact(id: u64, cookie: u64) -> FactEnvelope {
    redis_set_request_fact(id, cookie)
}
fn redis_error_export(path: &str, facts: Vec<FactEnvelope>) -> gewyvern::export::ExportBundle {
    let binding = compile_file(path).expect("redis failure DSL should compile");
    annotate_export_trust(
        export_from_test_facts(binding, facts),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

#[test]
fn summary_json_carries_redis_auth_required_detail() {
    let export = redis_error_export(
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/auth-required/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/auth-denied/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/error/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/wrongtype/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/busygroup/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/readonly/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/noscript/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/moved/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/ask/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/tryagain/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/loading/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/crossslot/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/clusterdown/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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

#[test]
fn summary_json_carries_redis_masterdown_detail() {
    let export = redis_error_export(
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/masterdown/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/oom/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/busy/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/execabort/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
        "/Users/Shared/chroot/dev/gewyvern/protocols/redis/misconf/main.gewy",
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
    let json = summary_json("dsl_demo", &export);
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
