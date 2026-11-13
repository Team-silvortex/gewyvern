mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, tcp_state_fact_with_ports};

fn check(entry: &str, phase: &str, payload: &[u8], matched: bool) {
    check_scoped(entry, phase, payload, matched, (PacketDir::Ingress, 554));
}

fn check_scoped(entry: &str, phase: &str, payload: &[u8], matched: bool, scope: (PacketDir, u16)) {
    for path in [
        format!("protocols/rtsp/{entry}/main.gewy"),
        format!("dsl/rtsp_{}_path.gewy", entry.replace('-', "_")),
    ] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(&path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        session.ingest(tcp_state_fact_with_ports(1, 90, 1, 2, 45000, scope.1));
        let bytes: Vec<_> = payload
            .iter()
            .enumerate()
            .map(|(at, b)| (at as u16, *b))
            .collect();
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            2,
            90,
            0x18,
            scope.0,
            Some(45000),
            Some(scope.1),
            &bytes,
        ));
        session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));
        let export = session.export_bundle();
        assert_eq!(
            export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .any(|s| s.phase.as_deref() == Some(phase)),
            matched,
            "{path} {phase} {payload:?}"
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
fn rtsp_predicate_replay_rejects_invalid_ports_kinds_and_extra_fields() {
    let binding = compile_file(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("protocols/rtsp/options/main.gewy")
            .to_string_lossy(),
    )
    .unwrap();
    let session = RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
    let export = session.export_bundle();
    let value: serde_json::Value = serde_json::from_str(&export.to_json()).unwrap();
    let rules = value["reason_profile"]["rules"].as_array().unwrap();
    let index = rules
        .iter()
        .position(|rule| rule["predicate"]["kind"] == "rtsp_response_observed")
        .unwrap();
    for (field, invalid) in [
        ("remote_port", serde_json::json!(-1)),
        ("remote_port", serde_json::json!(65536)),
        ("remote_port", serde_json::json!(null)),
        ("response", serde_json::json!("unknown")),
        ("response", serde_json::json!(false)),
        ("dir", serde_json::json!("egress")),
    ] {
        let mut bad = value.clone();
        bad["reason_profile"]["rules"][index]["predicate"][field] = invalid;
        assert!(
            ExportBundle::from_json(&bad.to_string()).is_err(),
            "{field}"
        );
    }
}

#[test]
fn rtsp_real_order_and_cseq_width_do_not_control_outcome_matching() {
    for cseq in [1, 12, 12345] {
        for (entry, phase, headers) in [
            (
                "options",
                "receive_options_ok",
                "Public: DESCRIBE, SETUP, PLAY\r\n",
            ),
            (
                "describe",
                "receive_describe_ok",
                "Content-Type: application/sdp\r\nContent-Length: 148\r\n",
            ),
            (
                "setup",
                "receive_setup_ok",
                "Session: a7f981;timeout=55\r\nTransport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n",
            ),
            (
                "play",
                "receive_play_ok",
                "Session: a7f981;timeout=55\r\nRTP-Info: url=rtsp://test/track\r\n",
            ),
        ] {
            check(
                entry,
                phase,
                format!("RTSP/1.0 200 OK\r\nCSeq: {cseq}\r\nServer: gortsplib\r\n{headers}\r\n")
                    .as_bytes(),
                true,
            );
            check(
                entry,
                phase,
                format!("RTSP/1.0 200 OK\r\n{headers}cSeQ: {cseq}\r\n\r\n").as_bytes(),
                true,
            );
        }
    }
}
#[test]
fn rtsp_keepalive_pause_denial_and_body_headers_are_not_success_candidates() {
    for (entry, phase) in [
        ("options", "receive_options_ok"),
        ("describe", "receive_describe_ok"),
        ("setup", "receive_setup_ok"),
        ("play", "receive_play_ok"),
    ] {
        for payload in [
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nSession: a7f981\r\n\r\n".as_slice(),
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Type: text/parameters\r\n\r\n",
            b"RTSP/1.0 461 Unsupported Transport\r\nCSeq: 1\r\nSession: a7f981\r\nTransport: RTP/AVP/TCP\r\n\r\n",
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 999\r\n\r\nPublic: OPTIONS\r\nContent-Type: application/sdp\r\nTransport: RTP/AVP/TCP\r\nSession: a7f981\r\nRTP-Info: url=rtsp://test/track\r\n",
        ] {check(entry,phase,payload,false);}
    }
}
#[test]
fn rtsp_missing_duplicate_or_truncated_headers_fail_closed() {
    for payload in [
        b"RTSP/1.0 200 OK\r\nPublic: OPTIONS\r\n\r\n".as_slice(),
        b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nCSEQ: 2\r\nPublic: OPTIONS\r\n\r\n",
        b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\n",
        b"RTSP/1.0 2000 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\n\r\n",
    ] {
        check("options", "receive_options_ok", payload, false);
    }
    check(
        "options",
        "receive_options_ok",
        format!(
            "RTSP/1.0 200 OK\r\nCSeq: 1\r\nX-Pad: {}\r\nPublic: OPTIONS\r\n\r\n",
            "x".repeat(512)
        )
        .as_bytes(),
        false,
    );
}

#[test]
fn rtsp_scope_and_header_boundary_are_explicit() {
    let prefix = "RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\nX-Pad: ";
    let exact = format!("{prefix}{}\r\n\r\n", "x".repeat(512 - prefix.len() - 4));
    check("options", "receive_options_ok", exact.as_bytes(), true);
    for scope in [(PacketDir::Egress, 554), (PacketDir::Ingress, 8554)] {
        check_scoped(
            "options",
            "receive_options_ok",
            exact.as_bytes(),
            false,
            scope,
        );
    }
    use gewyvern::ir::{FlowPredicate, RtspResponseKind};
    let predicate = FlowPredicate::RtspResponseObserved {
        remote_port: 554,
        kind: RtspResponseKind::Options,
    };
    assert_eq!(
        predicate.required_payload_offsets(),
        (0..512).collect::<Vec<_>>()
    );
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("protocols/rtsp/options/main.gewy"),
    )
    .unwrap();
    for invalid in [
        "rtsp_response_observed:remote:70000:options_ok",
        "rtsp_response_observed:remote:rtsp:unknown",
        "rtsp_response_observed:remote:rtsp:options_ok:extra",
    ] {
        assert!(
            gewyvern::dsl::compile_str(
                &source.replace("rtsp_response_observed:remote:rtsp:options_ok", invalid)
            )
            .is_err()
        );
    }
}
