mod support;
use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::{FactKind, PacketDir};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};

fn check(
    entry: &str,
    label: &str,
    bytes: &[u8],
    expected: bool,
    scope: (PacketDir, u16),
    hole: bool,
) {
    for path in [
        format!("protocols/snmp/{entry}/main.gewy"),
        format!("dsl/snmp_{}_path.gewy", entry.replace('-', "_")),
    ] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        let mut fact = support::snmp_packet(1, 90, scope.0, (45000, scope.1), bytes);
        if hole && let FactKind::PacketMeta(p) = &mut fact.kind {
            p.payload_bytes.remove(&5);
        }
        session.ingest(fact);
        session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));
        let export = session.export_bundle();
        assert_eq!(
            export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .any(|s| s.phase.as_deref() == Some(label)),
            expected,
            "{entry} {label} {bytes:02x?}"
        );
        let replay = ExportBundle::from_json(&export.to_json())
            .unwrap()
            .replay()
            .unwrap();
        assert_eq!(replay.program_flows, export.program_flows);
        assert_eq!(replay.reasons, export.reasons);
        assert_eq!(replay.protocol_ir, export.protocol_ir);
    }
}

#[test]
fn variable_community_and_ber_lengths_roundtrip_for_all_read_packages() {
    for (entry, pdu, phase, response) in [
        ("get", 0xa0, "send_get_request", "receive_get_response"),
        (
            "get-next",
            0xa1,
            "send_get_next_request",
            "receive_get_next_response",
        ),
        ("bulk", 0xa5, "send_bulk_request", "receive_bulk_response"),
    ] {
        for community in [b"x".as_slice(), b"public", b"gewyvern-lab"] {
            for long in [false, true] {
                check(
                    entry,
                    phase,
                    &support::snmp_message(pdu, community, long),
                    true,
                    (PacketDir::Egress, 161),
                    false,
                );
                check(
                    entry,
                    response,
                    &support::snmp_message(0xa2, community, long),
                    true,
                    (PacketDir::Ingress, 161),
                    false,
                );
            }
        }
    }
}

#[test]
fn truncated_sparse_wrong_scope_and_legacy_byte13_lookalikes_fail_closed() {
    let good = support::snmp_message(0xa0, b"public", false);
    for end in 0..good.len() {
        check(
            "get",
            "send_get_request",
            &good[..end],
            false,
            (PacketDir::Egress, 161),
            false,
        );
    }
    for scope in [(PacketDir::Ingress, 161), (PacketDir::Egress, 162)] {
        check("get", "send_get_request", &good, false, scope, false);
    }
    check(
        "get",
        "send_get_request",
        &good,
        false,
        (PacketDir::Egress, 161),
        true,
    );
    let mut fake = vec![0; 40];
    fake[0] = 0x30;
    fake[13] = 0xa0;
    check(
        "get",
        "send_get_request",
        &fake,
        false,
        (PacketDir::Egress, 161),
        false,
    );
    for (offset, value) in [(1, 0x80), (1, 0xff), (4, 3), (5, 5), (13, 0xa1)] {
        let mut bad = good.clone();
        bad[offset] = value;
        check(
            "get",
            "send_get_request",
            &bad,
            false,
            (PacketDir::Egress, 161),
            false,
        );
    }
    let mut tail = good.clone();
    tail.push(0);
    check(
        "get",
        "send_get_request",
        &tail,
        false,
        (PacketDir::Egress, 161),
        false,
    );
    check(
        "get",
        "send_get_request",
        &support::snmp_message(0xa0, &[b'x'; 120], false),
        false,
        (PacketDir::Egress, 161),
        false,
    );
}

#[test]
fn snmp_predicate_export_rejects_invalid_fields_and_dsl_vocabulary() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/protocols/snmp/get/main.gewy"
    ))
    .unwrap();
    let config = SessionConfig::for_binding(gewyvern::dsl::compile_str(&source).unwrap()).unwrap();
    let export = RuntimeSession::start(config).unwrap().export_bundle();
    let value: serde_json::Value = serde_json::from_str(&export.to_json()).unwrap();
    let index = value["reason_profile"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| r["predicate"]["kind"] == "snmp_pdu_observed")
        .unwrap();
    for (key, bad) in [
        ("remote_port", serde_json::json!(-1)),
        ("remote_port", serde_json::json!(65536)),
        ("pdu", serde_json::json!("set")),
        ("dir", serde_json::json!("egress")),
    ] {
        let mut altered = value.clone();
        altered["reason_profile"]["rules"][index]["predicate"][key] = bad;
        assert!(ExportBundle::from_json(&altered.to_string()).is_err());
    }
    for invalid in ["get:extra", "set", "v3", "", "response:remote:161"] {
        assert!(
            gewyvern::dsl::compile_str(&source.replace(
                "snmp_pdu_observed:remote:snmp:get",
                &format!("snmp_pdu_observed:remote:snmp:{invalid}")
            ))
            .is_err()
        );
    }
    let pred = gewyvern::ir::FlowPredicate::SnmpPduObserved {
        remote_port: 161,
        kind: gewyvern::ir::SnmpPduKind::Get,
    };
    assert_eq!(
        pred.required_payload_offsets(),
        (0..128).collect::<Vec<_>>()
    );
}

#[test]
fn v1_read_is_distinct_from_v2_bulk_and_response_is_not_success() {
    let mut get = support::snmp_message(0xa0, b"public", false);
    get[4] = 0;
    check(
        "get",
        "send_get_request",
        &get,
        true,
        (PacketDir::Egress, 161),
        false,
    );
    let mut bulk = support::snmp_message(0xa5, b"public", false);
    bulk[4] = 0;
    check(
        "bulk",
        "send_bulk_request",
        &bulk,
        false,
        (PacketDir::Egress, 161),
        false,
    );
    let mut error = support::snmp_message(0xa2, b"public", false);
    // public community, one-byte request-id: error-status/index values at 20/23.
    error[20] = 16;
    error[23] = 1;
    check(
        "get",
        "receive_get_response",
        &error,
        true,
        (PacketDir::Ingress, 161),
        false,
    );
    error[20] = 19;
    check(
        "get",
        "receive_get_response",
        &error,
        false,
        (PacketDir::Ingress, 161),
        false,
    );
}
