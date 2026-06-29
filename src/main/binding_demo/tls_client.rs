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
}
