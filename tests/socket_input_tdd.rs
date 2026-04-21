#[cfg(target_family = "unix")]
mod unix_socket_tests {
    use gewyvern::export::fact_to_json;
    use gewyvern::socket_input::{bind_unix_socket_listener, run_unix_socket_session_on_listener};
    use gewyvern::template::udp_debug_template;
    use std::fs;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, SystemTime};

    #[test]
    #[ignore = "requires local unix socket bind permissions"]
    fn unix_socket_session_ingests_fact_json_lines_and_exports_udp_session() {
        let socket_path = format!(
            "/tmp/gw-{}-{}.sock",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let listener = bind_unix_socket_listener(&socket_path).unwrap();
        assert!(fs::metadata(&socket_path).is_ok());

        let handle = thread::spawn(move || run_unix_socket_session_on_listener(&listener, udp_debug_template()));

        thread::sleep(Duration::from_millis(25));
        let mut client = UnixStream::connect(&socket_path).expect("unix socket listener should accept a client");
        let packet = super::support::udp_packet_fact(1, 123, 88);
        let route = super::support::route_fact(2, 123, 5);

        writeln!(client, "{}", fact_to_json(&packet)).unwrap();
        writeln!(client, "{}", fact_to_json(&route)).unwrap();
        drop(client);

        let export = handle.join().unwrap().unwrap();
        assert_eq!(export.template_id, "udp_debug");
        assert_eq!(export.facts.len(), 2);
        assert_eq!(export.flows.len(), 1);
        assert_eq!(export.reasons.len(), 1);
        assert_eq!(export.debug_summary.accepted_facts, 2);
        assert!(!export.debug_summary.degraded);
    }
}

#[cfg(target_family = "unix")]
mod support {
    use gewyvern::ledger::{
        CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact,
        SessionId,
    };
    use std::time::{Duration, SystemTime};

    pub fn udp_packet_fact(id: u64, cookie: u64, tot_len: u32) -> FactEnvelope {
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "udp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                netns: 1,
                sk_cookie: Some(cookie),
                dir: PacketDir::Egress,
                local_port: None,
                remote_port: None,
                payload_byte0: None,
                payload_prefix2: None,
                payload_prefix4: None,
                payload_byte4: None,
                payload_byte13: None,
                l3_proto: 0x0800,
                l4_proto: 17,
                tot_len,
                tcp_flags: 0,
                seq: None,
                ack: None,
                window: None,
            }),
        }
    }

    pub fn route_fact(id: u64, cookie: u64, oif: u32) -> FactEnvelope {
        FactEnvelope {
            id: FactId(id),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
            cpu: CpuId(0),
            ifindex: Some(oif),
            session: SessionId(1),
            fragment_id: "route_meta_fragment".into(),
            kind: FactKind::RouteDecision(RouteDecisionFact {
                netns: 1,
                sk_cookie: Some(cookie),
                fib_table: Some(254),
                oif,
                gw: None,
            }),
        }
    }
}
