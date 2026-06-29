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
}
