use super::*;

#[test]
fn summary_json_carries_ldap_bind_denied_detail() {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_bind_denied_path.gewy")
        .expect("ldap_bind_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82917, 54030, "ldapbind"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82917,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82917, 1, 2, 54030, 389),
                tcp_state_fact_with_ports_for_tests(4, 82917, 2, 3, 54030, 389),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82917,
                    0x18,
                    PacketDir::Egress,
                    Some(54030),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x60)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82917,
                    0x18,
                    PacketDir::Ingress,
                    Some(54030),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x61), (9, 0x31)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"directory_bind\""));
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
        json.contains("\"primary_failure_confidence\":\"high\""),
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
fn summary_json_carries_ldap_modify_denied_detail() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_denied_path.gewy")
            .expect("ldap_modify_denied_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82931, 54031, "ldapmodify"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82931,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82931, 1, 2, 54031, 389),
                tcp_state_fact_with_ports_for_tests(4, 82931, 2, 3, 54031, 389),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82931,
                    0x18,
                    PacketDir::Egress,
                    Some(54031),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x66)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82931,
                    0x18,
                    PacketDir::Ingress,
                    Some(54031),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x67), (9, 0x32)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"directory_write\""));
    assert!(json.contains("\"primary_failure_mode\":\"server_denied\""));
    assert!(json.contains("\"primary_failure_detail\":\"access_denied\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}

#[test]
fn summary_json_carries_ldap_modify_constraint_detail() {
    let binding =
        compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/ldap_modify_constraint_path.gewy")
            .expect("ldap_modify_constraint_path DSL should compile");
    let export = annotate_export_trust(
        export_from_test_facts(
            binding,
            vec![
                sock_lineage_fact_for_tests(1, 82932, 54032, "ldapmodify"),
                route_fact(
                    2,
                    SystemTime::UNIX_EPOCH + Duration::from_millis(20),
                    82932,
                    7,
                    SessionId(1),
                ),
                tcp_state_fact_with_ports_for_tests(3, 82932, 1, 2, 54032, 389),
                tcp_state_fact_with_ports_for_tests(4, 82932, 2, 3, 54032, 389),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    5,
                    82932,
                    0x18,
                    PacketDir::Egress,
                    Some(54032),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x66)],
                ),
                packet_fact_with_dir_and_payload_bytes_for_tests(
                    6,
                    82932,
                    0x18,
                    PacketDir::Ingress,
                    Some(54032),
                    Some(389),
                    &[(0, 0x30), (4, 0x01), (5, 0x67), (9, 0x13)],
                ),
            ],
        ),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let json = summary_json("dsl_demo", &export);
    assert!(json.contains("\"primary_module_kind\":\"directory_write\""));
    assert!(json.contains("\"primary_failure_mode\":\"semantic_error\""));
    assert!(json.contains("\"primary_failure_detail\":\"protocol_constraint_violation\""));
    assert!(json.contains("\"primary_failure_confidence\":\"high\""));
    assert!(json.contains("\"primary_failure_basis\":\"direct_protocol_signal\""));
}
