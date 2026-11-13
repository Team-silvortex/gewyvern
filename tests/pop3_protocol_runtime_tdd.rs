mod support;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{packet_fact_with_dir_and_payload_bytes, tcp_state_fact_with_ports};

fn check(entry: &str, phase: &str, payload: &[u8], matched: bool) {
    for path in [
        format!("protocols/pop3/{entry}/main.gewy"),
        format!("dsl/pop3_{}_path.gewy", entry.replace('-', "_")),
    ] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(&path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        session.ingest(tcp_state_fact_with_ports(1, 90, 1, 2, 45000, 110));
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
            Some(110),
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
fn pop3_auth_candidates_use_explicit_complete_status_text() {
    for entry in ["auth", "list"] {
        check(entry, "receive_auth_ok", b"+OK signed in\r\n", true);
        for body in [
            b"+OK\r\n".as_slice(),
            b"+OK signed in later\r\n",
            b"+OK signed i",
            b"+OK 2 768\r\n",
            b"+OK Mailpit POP3 server\r\n",
            b"+OK not a status\r\n",
        ] {
            check(entry, "receive_auth_ok", body, false);
        }
    }
    check(
        "auth-denied",
        "receive_auth_denied",
        b"-ERR invalid password\r\n",
        true,
    );
    for body in [
        b"-ERR user not authenticated\r\n".as_slice(),
        b"-ERR no such message\r\n",
        b"-ERR authentication text only\r\n",
        b"-ERR invalid password format\r\n",
        b"-ERR invalid passwor",
    ] {
        check("auth-denied", "receive_auth_denied", body, false);
    }
}

#[test]
fn pop3_banner_and_user_ack_are_not_generic_ok() {
    for entry in ["auth", "auth-denied", "list"] {
        check(
            entry,
            "receive_banner",
            b"+OK Mailpit POP3 server\r\n",
            true,
        );
        check(entry, "receive_user_ok", b"+OK User accepted\r\n", true);
        for body in [
            b"+OK\r\n".as_slice(),
            b"+OK signed in\r\n",
            b"+OK message marked for deletion\r\n",
            b"+OK goodbye\r\n",
        ] {
            check(entry, "receive_banner", body, false);
            check(entry, "receive_user_ok", body, false);
        }
    }
}

#[test]
fn pop3_list_supports_decimal_counts_not_only_the_fixture_mailbox_size() {
    for count in [0, 1, 2, 9, 10, 42, 99, 100, 999] {
        check(
            "list",
            "receive_list_ready",
            format!("+OK {count} messages (768 octets)\r\n1 355\r\n2 413\r\n.\r\n").as_bytes(),
            true,
        );
    }
}

#[test]
fn pop3_list_rejects_stat_uidl_retr_delete_and_malformed_counts() {
    for body in [
        b"+OK 2 768\r\n".as_slice(),
        b"+OK unique-id listing follows\r\n",
        b"+OK 355 octets\r\n",
        b"+OK message marked for deletion\r\n",
        b"+OK 2 messages",
        b"+OK X messages (768 octets)\r\n",
        b"+OK : messages (768 octets)\r\n",
        b"+OK 1: messages (768 octets)\r\n",
        b"+OK /00 messages (768 octets)\r\n",
        b"+OK 1000 messages (768 octets)\r\n",
        b"+OK 2 messagesx(768 octets)\r\n",
    ] {
        check("list", "receive_list_ready", body, false);
    }
}

#[test]
fn pop3_list_provenance_never_combines_conditions_from_different_packets() {
    for path in ["protocols/pop3/list/main.gewy", "dsl/pop3_list_path.gewy"] {
        let binding = compile_file(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join(path)
                .to_string_lossy(),
        )
        .unwrap();
        let mut session =
            RuntimeSession::start(SessionConfig::for_binding(binding).unwrap()).unwrap();
        session.ingest(tcp_state_fact_with_ports(1, 90, 1, 2, 45000, 110));
        for (id, payload) in [
            (2, b"+OK 2 768\r\n".as_slice()),
            (3, b"+OK X messages (768 octets)\r\n"),
            (4, b"+OK 2 messages (768 octets)\r\n"),
            (5, b"+OK 355 octets\r\n"),
        ] {
            let bytes: Vec<_> = payload
                .iter()
                .enumerate()
                .map(|(i, b)| (i as u16, *b))
                .collect();
            session.ingest(packet_fact_with_dir_and_payload_bytes(
                id,
                90,
                0x18,
                PacketDir::Ingress,
                Some(45000),
                Some(110),
                &bytes,
            ));
        }
        session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(120));
        let export = session.export_bundle();
        let stages: Vec<_> = export
            .program_flows
            .iter()
            .flat_map(|f| &f.stages)
            .filter(|s| s.phase.as_deref() == Some("receive_list_ready"))
            .collect();
        assert_eq!(stages.len(), 1, "{path}");
        assert_eq!(stages[0].at, gewyvern::ledger::FactId(4), "{path}");
        let replay = ExportBundle::from_json(&export.to_json()).unwrap();
        assert_eq!(replay.program_flows, export.program_flows);
        assert_eq!(replay.reasons, export.reasons);
    }
}
