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
    let binding = compile_file(&path).expect("redis failure DSL should compile");
    annotate_export_trust(
        export_from_test_facts(binding, facts),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    )
}

#[path = "redis_failure_semantics/part_001.rs"]
mod part_001;
#[path = "redis_failure_semantics/part_002.rs"]
mod part_002;
