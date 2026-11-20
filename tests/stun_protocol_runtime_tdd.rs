mod support;
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
        format!("protocols/stun/{entry}/main.gewy"),
        format!("dsl/stun_{}_path.gewy", entry.replace('-', "_")),
    ] {
        let binding = gewyvern::dsl::compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        let mut fact = support::stun_packet(1, 71, scope.0, (40000, scope.1), bytes);
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
            "{entry} {phase}: {bytes:02x?}"
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
fn complete_binding_messages_require_direction_type_and_mandatory_fields() {
    for (entry, phase, kind, dir) in [
        ("binding", "send_request", 0x0001, PacketDir::Egress),
        ("binding", "receive_response", 0x0101, PacketDir::Ingress),
        (
            "binding-error",
            "receive_error_response",
            0x0111,
            PacketDir::Ingress,
        ),
    ] {
        let good = support::stun_message(kind);
        check(entry, phase, &good, true, (dir, 3478, 17), None);
        for end in 0..good.len() {
            check(entry, phase, &good[..end], false, (dir, 3478, 17), None);
        }
        for at in [0, 1, 4, 7, 19] {
            check(entry, phase, &good, false, (dir, 3478, 17), Some(at));
        }
        for (port, proto) in [(123, 17), (3478, 6)] {
            check(entry, phase, &good, false, (dir, port, proto), None);
        }
        let opposite = if dir == PacketDir::Egress {
            PacketDir::Ingress
        } else {
            PacketDir::Egress
        };
        check(entry, phase, &good, false, (opposite, 3478, 17), None);
        for at in [0, 1, 2, 3, 4, 7] {
            let mut bad = good.clone();
            bad[at] ^= 0x80;
            check(entry, phase, &bad, false, (dir, 3478, 17), None);
        }
        if kind != 1 {
            let empty = support::stun_message(kind & 0xff);
            check(entry, phase, &empty, false, (dir, 3478, 17), None);
        }
    }
    let mut no_address = support::stun_message(0x0101);
    no_address[21] = 0x21;
    check(
        "binding",
        "receive_response",
        &no_address,
        false,
        (PacketDir::Ingress, 3478, 17),
        None,
    );
    for (at, value) in [(25, 3), (23, 7)] {
        let mut bad = support::stun_message(0x0101);
        bad[at] = value;
        check(
            "binding",
            "receive_response",
            &bad,
            false,
            (PacketDir::Ingress, 3478, 17),
            None,
        );
    }
    for (at, value) in [(21, 10), (26, 7), (27, 100)] {
        let mut bad = support::stun_message(0x0111);
        bad[at] = value;
        check(
            "binding-error",
            "receive_error_response",
            &bad,
            false,
            (PacketDir::Ingress, 3478, 17),
            None,
        );
    }
}

#[test]
fn attribute_padding_bounds_and_first_occurrence_are_explicit() {
    let mut ipv6 = support::stun_message(1);
    ipv6[0] = 1;
    ipv6[3] = 24;
    ipv6.extend([0, 0x20, 0, 20, 0, 2, 0x32, 0x9a]);
    ipv6.extend([0x42; 16]);
    check(
        "binding",
        "receive_response",
        &ipv6,
        true,
        (PacketDir::Ingress, 3478, 17),
        None,
    );
    ipv6[25] = 1;
    check(
        "binding",
        "receive_response",
        &ipv6,
        false,
        (PacketDir::Ingress, 3478, 17),
        None,
    );
    let mut request = support::stun_message(1);
    request.extend([0xff, 0xff, 0, 1, 0x41, 0xaa, 0xbb, 0xcc]);
    request[3] = 8;
    check(
        "binding",
        "send_request",
        &request,
        true,
        (PacketDir::Egress, 3478, 17),
        None,
    );
    for (at, value) in [(3, 7), (23, 5)] {
        let mut bad = request.clone();
        bad[at] = value;
        check(
            "binding",
            "send_request",
            &bad,
            false,
            (PacketDir::Egress, 3478, 17),
            None,
        );
    }
    let mut bound = support::stun_message(1);
    bound.extend([0xff, 0xff, 0, 104]);
    bound.extend([0; 104]);
    bound[3] = 108;
    check(
        "binding",
        "send_request",
        &bound,
        true,
        (PacketDir::Egress, 3478, 17),
        None,
    );
    bound.extend([0xff, 0xff, 0, 0]);
    bound[3] = 112;
    check(
        "binding",
        "send_request",
        &bound,
        false,
        (PacketDir::Egress, 3478, 17),
        None,
    );
    let mut duplicate = support::stun_message(0x0101);
    duplicate.extend([0, 0x20, 0, 0]);
    duplicate[3] = 16;
    check(
        "binding",
        "receive_response",
        &duplicate,
        true,
        (PacketDir::Ingress, 3478, 17),
        None,
    );
    let mut first_bad = support::stun_message(1);
    first_bad[0] = 1;
    first_bad.extend([0, 0x20, 0, 0]);
    first_bad.extend(&support::stun_message(0x0101)[20..]);
    first_bad[3] = 16;
    check(
        "binding",
        "receive_response",
        &first_bad,
        false,
        (PacketDir::Ingress, 3478, 17),
        None,
    );
}

#[test]
fn binding_error_is_not_an_access_denial_and_codec_is_strict() {
    let semantics = gewyvern::protocol_profiles::protocol_surface("stun", "binding-error")
        .unwrap()
        .entry_semantics
        .unwrap();
    assert_eq!(
        semantics.primary_failure_mode.as_deref(),
        Some("semantic_error")
    );
    assert_eq!(
        semantics.primary_failure_detail.as_deref(),
        Some("protocol_error")
    );
    let source = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/protocols/stun/binding/main.gewy"
    ))
    .unwrap();
    let binding = gewyvern::dsl::compile_str(&source).unwrap();
    let export = RuntimeSession::start(SessionConfig::for_binding(binding).unwrap())
        .unwrap()
        .export_bundle();
    let value: serde_json::Value = serde_json::from_str(&export.to_json()).unwrap();
    let index = value["reason_profile"]["rules"]
        .as_array()
        .unwrap()
        .iter()
        .position(|r| r["predicate"]["kind"] == "stun_binding_observed")
        .unwrap();
    for (key, bad) in [
        ("message", serde_json::json!("allocate")),
        ("remote_port", serde_json::json!(65536)),
        ("remote_port", serde_json::json!(-1)),
        ("remote_port", serde_json::json!(null)),
        ("dir", serde_json::json!("ingress")),
    ] {
        let mut changed = value.clone();
        changed["reason_profile"]["rules"][index]["predicate"][key] = bad;
        assert!(ExportBundle::from_json(&changed.to_string()).is_err());
    }
    for bad in ["request:extra", "allocate", ""] {
        assert!(
            gewyvern::dsl::compile_str(&source.replace(
                "stun_binding_observed:remote:stun:request",
                &format!("stun_binding_observed:remote:stun:{bad}")
            ))
            .is_err()
        );
    }
    for bad in [
        None,
        Some(serde_json::json!(-1)),
        Some(serde_json::json!(65536)),
    ] {
        let mut changed = value.clone();
        let predicate = changed["reason_profile"]["rules"][index]["predicate"]
            .as_object_mut()
            .unwrap();
        let expected = if let Some(bad) = bad {
            predicate.insert("remote_port".into(), bad);
            gewyvern::export::ExportError::InvalidValue("STUN port out of range".into())
        } else {
            predicate.remove("remote_port");
            gewyvern::export::ExportError::InvalidShape("missing STUN port".into())
        };
        assert_eq!(
            ExportBundle::from_json(&changed.to_string()).unwrap_err(),
            expected
        );
    }
    let predicate = gewyvern::ir::FlowPredicate::StunBindingObserved {
        remote_port: 3478,
        kind: gewyvern::ir::StunBindingKind::Response,
    };
    assert_eq!(
        predicate.required_payload_offsets(),
        (0..128).collect::<Vec<_>>()
    );
}
