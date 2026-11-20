mod support;
use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::{FactKind, PacketDir};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};

fn check(
    entry: &str,
    phase: &str,
    bytes: &[u8],
    expected: bool,
    scope: (PacketDir, u16, u8),
    hole: Option<u16>,
) {
    for path in [
        format!("protocols/ntp/{entry}/main.gewy"),
        format!("dsl/ntp_{entry}_path.gewy"),
    ] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        let mut fact = support::ntp_packet(1, 91, scope.0, (45000, scope.1), bytes);
        if let FactKind::PacketMeta(p) = &mut fact.kind {
            p.l4_proto = scope.2;
            if let Some(at) = hole {
                p.payload_bytes.remove(&at);
            }
        }
        session.ingest(fact);
        session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
        let export = session.export_bundle();
        assert_eq!(
            export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .any(|s| s.phase.as_deref() == Some(phase)),
            expected,
            "{entry} {phase} {bytes:02x?}"
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
fn versions_and_leap_bits_do_not_define_query_versus_sync_intent() {
    for (entry, phase, response) in [
        ("client", "send_request", "receive_response"),
        ("query", "send_query", "receive_response"),
        ("sync", "send_sync_request", "receive_sync_response"),
    ] {
        for version in [3, 4] {
            for leap in 0..=2 {
                check(
                    entry,
                    phase,
                    &support::ntp_header(leap << 6 | version << 3 | 3),
                    true,
                    (PacketDir::Egress, 123, 17),
                    None,
                );
                check(
                    entry,
                    response,
                    &support::ntp_header(leap << 6 | version << 3 | 4),
                    true,
                    (PacketDir::Ingress, 123, 17),
                    None,
                );
            }
        }
    }
}

#[test]
fn unsynchronised_kod_and_missing_timestamps_are_not_usable_time_replies() {
    for flags in [0x1c, 0x24, 0xdc, 0xe4] {
        for stratum in [0, 10, 16, 255] {
            let mut bytes = support::ntp_header(flags);
            bytes[1] = stratum;
            check(
                "query",
                "receive_response",
                &bytes,
                true,
                (PacketDir::Ingress, 123, 17),
                None,
            );
            check(
                "sync",
                "receive_sync_response",
                &bytes,
                flags >> 6 != 3 && stratum == 10,
                (PacketDir::Ingress, 123, 17),
                None,
            );
        }
    }
    for start in [24, 32, 40] {
        let mut bytes = support::ntp_header(0x24);
        bytes[start..start + 8].fill(0);
        check(
            "sync",
            "receive_sync_response",
            &bytes,
            false,
            (PacketDir::Ingress, 123, 17),
            None,
        );
    }
}

#[test]
fn complete_base_header_and_exact_transport_scope_are_required() {
    let good = support::ntp_header(0x24);
    for end in 0..48 {
        check(
            "query",
            "receive_response",
            &good[..end],
            false,
            (PacketDir::Ingress, 123, 17),
            None,
        );
    }
    for scope in [
        (PacketDir::Egress, 123, 17),
        (PacketDir::Ingress, 161, 17),
        (PacketDir::Ingress, 123, 6),
    ] {
        check("query", "receive_response", &good, false, scope, None);
    }
    for offset in [0, 1, 10, 47] {
        check(
            "query",
            "receive_response",
            &good,
            false,
            (PacketDir::Ingress, 123, 17),
            Some(offset),
        );
    }
    for flags in [0x04, 0x14, 0x2c, 0x23, 0x25, 0x26, 0x27] {
        check(
            "query",
            "receive_response",
            &support::ntp_header(flags),
            false,
            (PacketDir::Ingress, 123, 17),
            None,
        );
    }
    // Extension/MAC bytes are outside this base-header predicate, not authenticated.
    let mut extended = good;
    extended.extend([0xff; 80]);
    check(
        "query",
        "receive_response",
        &extended,
        true,
        (PacketDir::Ingress, 123, 17),
        None,
    );
}

#[test]
fn ntp_dsl_export_and_required_samples_are_strict() {
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/protocols/ntp/query/main.gewy"
    ))
    .unwrap();
    let config = SessionConfig::for_binding(gewyvern::dsl::compile_str(&source).unwrap()).unwrap();
    let export = RuntimeSession::start(config).unwrap().export_bundle();
    let value: serde_json::Value = serde_json::from_str(&export.to_json()).unwrap();
    let index = value["reason_profile"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| r["predicate"]["kind"] == "ntp_message_observed")
        .unwrap();
    for (key, bad) in [
        ("remote_port", serde_json::json!(-1)),
        ("remote_port", serde_json::json!(65536)),
        ("remote_port", serde_json::json!(null)),
        ("message", serde_json::json!("sync_success")),
        ("dir", serde_json::json!("egress")),
    ] {
        let mut changed = value.clone();
        changed["reason_profile"]["rules"][index]["predicate"][key] = bad;
        assert!(ExportBundle::from_json(&changed.to_string()).is_err());
    }
    for invalid in ["request:extra", "sync_success", "", "v4"] {
        assert!(
            gewyvern::dsl::compile_str(&source.replace(
                "ntp_message_observed:remote:ntp:request",
                &format!("ntp_message_observed:remote:ntp:{invalid}")
            ))
            .is_err()
        );
    }
    let pred = gewyvern::ir::FlowPredicate::NtpMessageObserved {
        remote_port: 123,
        kind: gewyvern::ir::NtpMessageKind::TimeReply,
    };
    assert_eq!(pred.required_payload_offsets(), (0..48).collect::<Vec<_>>());
}
