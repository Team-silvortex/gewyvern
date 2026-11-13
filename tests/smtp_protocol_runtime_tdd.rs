mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, tcp_state_fact_with_ports};

fn check(entry: &str, phase: &str, payload: &[u8], matched: bool) {
    for path in [
        format!("protocols/smtp/{entry}/main.gewy"),
        format!("dsl/smtp_{}_path.gewy", entry.replace('-', "_")),
    ] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(&path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        session.ingest(tcp_state_fact_with_ports(1, 90, 1, 2, 45000, 25));
        let bytes: Vec<_> = payload
            .iter()
            .enumerate()
            .map(|(at, b)| (at as u16, *b))
            .collect();
        session.ingest(packet_fact_with_dir_and_payload_bytes(
            2,
            90,
            0x18,
            PacketDir::Ingress,
            Some(45000),
            Some(25),
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
        let replay = ExportBundle::from_json(&export.to_json()).unwrap();
        assert_eq!(replay.program_flows, export.program_flows);
        assert_eq!(replay.reasons, export.reasons);
        assert_eq!(replay.protocol_ir, export.protocol_ir);
    }
}

#[test]
fn smtp_queue_candidate_requires_explicit_queue_text_not_noop_or_reset() {
    for body in [
        b"250 2.0.0 Ok\r\n".as_slice(),
        b"250 2.0.0",
        b"250 2.1.5 Ok\r\n",
        b"250 2.0.0 Ok: queued a",
    ] {
        check("data", "receive_message_queued", body, false);
    }
    check(
        "data",
        "receive_message_queued",
        b"250 2.0.0 Ok: queued as ABC\r\n",
        true,
    );
}

#[test]
fn smtp_message_denial_does_not_reclassify_recipient_or_generic_policy_errors() {
    for body in [
        b"550 5.1.0 Requested action not taken: mailbox unavailable\r\n".as_slice(),
        b"550 ",
        b"550 5.7.1 Relay access denied\r\n",
        b"550 5.7.1 Message rejecte",
    ] {
        check("data-denied", "receive_message_denied", body, false);
    }
    check(
        "data-denied",
        "receive_message_denied",
        b"550 5.7.1 Message rejected\r\n",
        true,
    );
}

#[test]
fn smtp_multiline_ehlo_does_not_use_later_noop_or_envelope_success() {
    for entry in [
        "auth",
        "auth-denied",
        "mail",
        "rcpt",
        "rcpt-denied",
        "data",
        "data-denied",
    ] {
        for body in [b"250 2.0.0 Ok\r\n".as_slice(), b"250 2.1.0 Ok\r\n", b"250"] {
            check(entry, "receive_ehlo_ok", body, false);
        }
        check(
            entry,
            "receive_ehlo_ok",
            b"250-host greets client\r\n250 SMTPUTF8\r\n",
            true,
        );
    }
}

#[test]
fn smtp_recipient_denial_does_not_reclassify_message_or_generic_policy_errors() {
    for body in [
        b"550 ".as_slice(),
        b"550 5.7.1 Message rejected\r\n",
        b"550 5.7.1 Relay access denied\r\n",
        b"550 5.1.10 invalid\r\n",
    ] {
        check("rcpt-denied", "receive_rcpt_denied", body, false);
    }
    for body in [
        b"550 5.1.0 Mailbox unavailable\r\n".as_slice(),
        b"550 5.1.1 No such user\r\n",
    ] {
        check("rcpt-denied", "receive_rcpt_denied", body, true);
    }
}

#[test]
fn smtp_sender_acceptance_does_not_use_recipient_success_or_truncated_status() {
    for entry in ["mail", "rcpt", "rcpt-denied", "data", "data-denied"] {
        for body in [
            b"250 2.1.5 Ok\r\n".as_slice(),
            b"250 2.1.",
            b"250 2.1.01 invalid\r\n",
        ] {
            check(entry, "receive_mail_ok", body, false);
        }
        check(entry, "receive_mail_ok", b"250 2.1.0 Ok\r\n", true);
    }
}
