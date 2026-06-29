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
