use super::*;

pub(crate) fn run_binding_demo(binding: TemplateBinding) -> ExportBundle {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let fragments = &binding.template.fragment_set;
    let tcp_demo_dport = binding
        .template
        .program_model
        .as_ref()
        .map(|model| match &model.operation {
            gewyvern::flow::ProgramOperation::Custom(value) if value == "postgres_connect" => 5432,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "redis_connect" => 6379,
            gewyvern::flow::ProgramOperation::Custom(value) if value == "mysql_connect" => 3306,
            _ => 443,
        })
        .unwrap_or(443);
    let is_dns_lookup = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "dns_lookup"
            )
        });
    let is_http_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_request"
            )
        });
    let is_tls_client = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "tls_client"
            )
        });
    let is_http_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_server_response"
            )
        });
    let is_http3_request = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_request"
            )
        });
    let is_http3_server_response = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http3_server_response"
            )
        });
    let is_hy2_auth = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_auth"
            )
        });
    let is_hy2_udp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_udp_relay"
            )
        });
    let is_hy2_tcp_relay = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "hy2_tcp_relay"
            )
        });
    let is_socks5_session = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "socks5_session"
            )
        });
    let is_http_connect_tunnel = binding
        .template
        .program_model
        .as_ref()
        .is_some_and(|model| {
            matches!(
                &model.operation,
                gewyvern::flow::ProgramOperation::Custom(value) if value == "http_connect_tunnel"
            )
        });
    let facts = if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_http_server_response
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 77,
                    pid: 8080,
                    tid: 8080,
                    cgroup_id: 8080,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..5].copy_from_slice(b"nginx");
                        comm
                    },
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 77,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 80,
                    dport: 53000,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(3),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 77,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 80,
                    dport: 53000,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(4),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(77),
                    dir: PacketDir::Ingress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 140,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
            FactEnvelope {
                id: FactId(5),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(77),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 220,
                    tcp_flags: 0x18,
                    seq: Some(2),
                    ack: Some(2),
                    window: Some(65535),
                }),
            },
        ]
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_http_request
    {
        let mut facts = vec![FactEnvelope {
            id: FactId(1),
            ts: base,
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(2),
            fragment_id: "sock_lineage_fragment".into(),
            kind: FactKind::SockLineage(SockLineageFact {
                netns: 1,
                sk_cookie: 99,
                pid: 4242,
                tid: 4242,
                cgroup_id: 4242,
                comm: {
                    let mut comm = [0u8; 16];
                    comm[..4].copy_from_slice(b"curl");
                    comm
                },
            }),
        }];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(
                2,
                base + Duration::from_millis(10),
                99,
                2,
                SessionId(2),
            ));
        }
        let offset = facts.len() as u64 + 1;
        facts.extend([
            FactEnvelope {
                id: FactId(offset),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 99,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: 443,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 1),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 99,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: 443,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 2),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 120,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
            FactEnvelope {
                id: FactId(offset + 3),
                ts: base + Duration::from_millis(50),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Ingress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 180,
                    tcp_flags: 0x18,
                    seq: Some(2),
                    ack: Some(2),
                    window: Some(65535),
                }),
            },
        ]);
        facts
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
        && is_tls_client
    {
        let mut facts = vec![FactEnvelope {
            id: FactId(1),
            ts: base,
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(2),
            fragment_id: "sock_lineage_fragment".into(),
            kind: FactKind::SockLineage(SockLineageFact {
                netns: 1,
                sk_cookie: 88,
                pid: 4242,
                tid: 4242,
                cgroup_id: 4242,
                comm: {
                    let mut comm = [0u8; 16];
                    comm[..4].copy_from_slice(b"curl");
                    comm
                },
            }),
        }];
        if fragments.contains(&"route_meta_fragment") {
            facts.push(route_fact(
                2,
                base + Duration::from_millis(10),
                88,
                2,
                SessionId(2),
            ));
        }
        let offset = facts.len() as u64 + 1;
        facts.extend([
            FactEnvelope {
                id: FactId(offset),
                ts: base + Duration::from_millis(20),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 88,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 1),
                ts: base + Duration::from_millis(30),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 88,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 2,
                    new: 3,
                }),
            },
            FactEnvelope {
                id: FactId(offset + 2),
                ts: base + Duration::from_millis(40),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(88),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 96,
                    tcp_flags: 0x18,
                    seq: Some(1),
                    ack: Some(1),
                    window: Some(65535),
                }),
            },
        ]);
        facts
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"tcp_packet_meta_fragment")
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 42,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(42),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 60,
                    tcp_flags: 0x02,
                    seq: Some(1),
                    ack: None,
                    window: Some(65535),
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
        ]
    } else if fragments.contains(&"tcp_state_fragment")
        && fragments.contains(&"sock_lineage_fragment")
    {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 42,
                    pid: 4242,
                    tid: 4242,
                    cgroup_id: 4242,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..4].copy_from_slice(b"curl");
                        comm
                    },
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 42,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: tcp_demo_dport,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
        ]
    } else if fragments.contains(&"udp_packet_meta_fragment")
        && fragments.contains(&"sock_lineage_fragment")
    {
        if is_http3_server_response {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 177,
                        pid: 8080,
                        tid: 8080,
                        cgroup_id: 8080,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..5].copy_from_slice(b"nginx");
                            comm
                        },
                    }),
                },
                FactEnvelope {
                    id: FactId(2),
                    ts: base + Duration::from_millis(10),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Ingress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(177),
                        dir: PacketDir::Egress,
                        local_port: Some(443),
                        remote_port: Some(53000),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::ConnectionClose],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_http3_request {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 99,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: Some(53000),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::ConnectionClose],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_hy2_tcp_relay {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 211,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 211, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x44),
                            (1u16, 0x01),
                        ]),
                    }),
                },
                FactEnvelope {
                    id: FactId(10),
                    ts: base + Duration::from_millis(90),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(211),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::from([(0u16, 0x00)]),
                    }),
                },
            ]
        } else if is_hy2_udp_relay {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 199,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 199, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(9),
                    ts: base + Duration::from_millis(80),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Datagram],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(10),
                    ts: base + Duration::from_millis(90),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(199),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Datagram],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_hy2_auth {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 188,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..8].copy_from_slice(b"hysteria");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 188, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xc0),
                        payload_byte1: None,
                        payload_prefix2: Some(0xc300),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 1300,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Initial),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        payload_byte0: Some(0xe0),
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 220,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: true,
                        packet_type: Some(QuicPacketType::Handshake),
                        frame_types: vec![QuicFrameType::Crypto],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Egress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::QuicMeta(gewyvern::ledger::QuicMetaFact {
                        netns: 1,
                        sk_cookie: Some(188),
                        dir: PacketDir::Ingress,
                        local_port: Some(42310),
                        remote_port: Some(443),
                        long_header: false,
                        packet_type: None,
                        frame_types: vec![QuicFrameType::Stream],
                        payload_bytes: std::collections::BTreeMap::new(),
                    }),
                },
            ]
        } else if is_socks5_session {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 155,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 155, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 155,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54000,
                        dport: 1080,
                        family: 2,
                        old: 1,
                        new: 2,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 155,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54000,
                        dport: 1080,
                        family: 2,
                        old: 2,
                        new: 3,
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Egress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x01),
                        payload_prefix2: Some(0x0501),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x01),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 80,
                        tcp_flags: 0x18,
                        seq: Some(1),
                        ack: Some(1),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Ingress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x00),
                        payload_prefix2: Some(0x0500),
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x00),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 80,
                        tcp_flags: 0x18,
                        seq: Some(2),
                        ack: Some(2),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(7),
                    ts: base + Duration::from_millis(60),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Egress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x01),
                        payload_prefix2: Some(0x0501),
                        payload_prefix4: Some(0x05010003),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x01),
                            (2u16, 0x00),
                            (3u16, 0x03),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 92,
                        tcp_flags: 0x18,
                        seq: Some(3),
                        ack: Some(3),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(8),
                    ts: base + Duration::from_millis(70),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(155),
                        dir: PacketDir::Ingress,
                        local_port: Some(54000),
                        remote_port: Some(1080),
                        payload_byte0: Some(0x05),
                        payload_byte1: Some(0x00),
                        payload_prefix2: Some(0x0500),
                        payload_prefix4: Some(0x05000001),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x05),
                            (1u16, 0x00),
                            (2u16, 0x00),
                            (3u16, 0x01),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 92,
                        tcp_flags: 0x18,
                        seq: Some(4),
                        ack: Some(4),
                        window: Some(65535),
                    }),
                },
            ]
        } else if is_http_connect_tunnel {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 166,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 166, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 166,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54100,
                        dport: 8080,
                        family: 2,
                        old: 1,
                        new: 2,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_state_fragment".into(),
                    kind: FactKind::TcpState(TcpStateFact {
                        netns: 1,
                        sk_cookie: 166,
                        saddr: [0; 16],
                        daddr: [0; 16],
                        sport: 54100,
                        dport: 8080,
                        family: 2,
                        old: 2,
                        new: 3,
                    }),
                },
                FactEnvelope {
                    id: FactId(5),
                    ts: base + Duration::from_millis(40),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(166),
                        dir: PacketDir::Egress,
                        local_port: Some(54100),
                        remote_port: Some(8080),
                        payload_byte0: Some(0x43),
                        payload_byte1: Some(0x4f),
                        payload_prefix2: Some(0x434f),
                        payload_prefix4: Some(0x434f4e4e),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x43),
                            (1u16, 0x4f),
                            (2u16, 0x4e),
                            (3u16, 0x4e),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 110,
                        tcp_flags: 0x18,
                        seq: Some(1),
                        ack: Some(1),
                        window: Some(65535),
                    }),
                },
                FactEnvelope {
                    id: FactId(6),
                    ts: base + Duration::from_millis(50),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "tcp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(166),
                        dir: PacketDir::Ingress,
                        local_port: Some(54100),
                        remote_port: Some(8080),
                        payload_byte0: Some(0x32),
                        payload_byte1: Some(0x30),
                        payload_prefix2: Some(0x3230),
                        payload_prefix4: Some(0x32303020),
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::from([
                            (0u16, 0x32),
                            (1u16, 0x30),
                            (2u16, 0x30),
                            (3u16, 0x20),
                        ]),
                        l3_proto: 0x0800,
                        l4_proto: 6,
                        tot_len: 96,
                        tcp_flags: 0x18,
                        seq: Some(2),
                        ack: Some(2),
                        window: Some(65535),
                    }),
                },
            ]
        } else if is_dns_lookup {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 99,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                FactEnvelope {
                    id: FactId(3),
                    ts: base + Duration::from_millis(20),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 72,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                FactEnvelope {
                    id: FactId(4),
                    ts: base + Duration::from_millis(30),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Ingress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 96,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
            ]
        } else {
            vec![
                FactEnvelope {
                    id: FactId(1),
                    ts: base,
                    cpu: CpuId(0),
                    ifindex: Some(2),
                    session: SessionId(2),
                    fragment_id: "sock_lineage_fragment".into(),
                    kind: FactKind::SockLineage(SockLineageFact {
                        netns: 1,
                        sk_cookie: 99,
                        pid: 4242,
                        tid: 4242,
                        cgroup_id: 4242,
                        comm: {
                            let mut comm = [0u8; 16];
                            comm[..4].copy_from_slice(b"curl");
                            comm
                        },
                    }),
                },
                FactEnvelope {
                    id: FactId(2),
                    ts: base + Duration::from_millis(10),
                    cpu: CpuId(0),
                    ifindex: Some(3),
                    session: SessionId(2),
                    fragment_id: "udp_packet_meta_fragment".into(),
                    kind: FactKind::PacketMeta(PacketMetaFact {
                        netns: 1,
                        sk_cookie: Some(99),
                        dir: PacketDir::Egress,
                        local_port: None,
                        remote_port: None,
                        payload_byte0: None,
                        payload_byte1: None,
                        payload_prefix2: None,
                        payload_prefix4: None,
                        payload_byte4: None,
                        payload_byte5: None,
                        payload_byte9: None,
                        payload_byte10: None,
                        payload_byte13: None,
                        payload_bytes: std::collections::BTreeMap::new(),
                        l3_proto: 0x0800,
                        l4_proto: 17,
                        tot_len: 72,
                        tcp_flags: 0,
                        seq: None,
                        ack: None,
                        window: None,
                    }),
                },
                route_fact(3, base + Duration::from_millis(20), 99, 3, SessionId(2)),
            ]
        }
    } else if fragments.contains(&"udp_packet_meta_fragment") {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(3),
                session: SessionId(2),
                fragment_id: "udp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
                    l3_proto: 0x0800,
                    l4_proto: 17,
                    tot_len: 72,
                    tcp_flags: 0,
                    seq: None,
                    ack: None,
                    window: None,
                }),
            },
            route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
        ]
    } else {
        eprintln!("{}", UiLocale::detect().msg("unsupported_fragment_combo"));
        std::process::exit(2);
    };

    let config = SessionConfig::for_binding(binding).expect("dsl binding should validate");
    let mut session = RuntimeSession::start(config).expect("dsl session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);

    let export = session.export_bundle();
    let replay = ExportBundle::from_json(&export.to_json())
        .expect("runtime should export replayable json")
        .replay()
        .expect("export should replay");

    assert_eq!(
        export.reasons, replay.reasons,
        "replay should stay deterministic"
    );
    export
}
