use super::*;

pub fn run_handshake_session(facts: Vec<FactEnvelope>) -> gewyvern::export::ExportBundle {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);
    session.export_bundle()
}

pub fn run_udp_session(facts: Vec<FactEnvelope>) -> gewyvern::export::ExportBundle {
    let config = SessionConfig::for_template(udp_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);
    session.export_bundle()
}

pub fn run_udp_process_session(facts: Vec<FactEnvelope>) -> gewyvern::export::ExportBundle {
    let config = SessionConfig::for_template(udp_process_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);
    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);
    session.export_bundle()
}

pub fn tcp_state_fact(id: u64, cookie: u64, old: u8, new: u8) -> FactEnvelope {
    tcp_state_fact_with_ports(id, cookie, old, new, 12345, 443)
}

pub fn tcp_state_fact_with_ports(
    id: u64,
    cookie: u64,
    old: u8,
    new: u8,
    sport: u16,
    dport: u16,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_state_fragment".into(),
        kind: FactKind::TcpState(TcpStateFact {
            netns: 1,
            sk_cookie: cookie,
            saddr: [0; 16],
            daddr: [0; 16],
            sport,
            dport,
            family: 2,
            old,
            new,
        }),
    }
}

pub fn packet_fact(id: u64, cookie: u64, tcp_flags: u16) -> FactEnvelope {
    packet_fact_with_dir(id, cookie, tcp_flags, PacketDir::Egress)
}

pub fn packet_fact_with_dir(id: u64, cookie: u64, tcp_flags: u16, dir: PacketDir) -> FactEnvelope {
    packet_fact_with_dir_and_payload(id, cookie, tcp_flags, dir, None, None, None, None, None)
}

pub fn packet_fact_with_dir_and_payload(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
) -> FactEnvelope {
    packet_fact_with_dir_and_payload_and_byte4(
        id,
        cookie,
        tcp_flags,
        dir,
        local_port,
        remote_port,
        payload_byte0,
        payload_prefix2,
        payload_prefix4,
        None,
    )
}

pub fn packet_fact_with_dir_and_payload_and_byte1(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_byte1: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1,
            payload_prefix2,
            payload_prefix4,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn packet_fact_with_dir_and_payload_and_byte10(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_byte10: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1: None,
            payload_prefix2,
            payload_prefix4,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn packet_fact_with_dir_and_payload_and_byte4(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
    payload_byte4: Option<u8>,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1: None,
            payload_prefix2,
            payload_prefix4,
            payload_byte4,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn packet_fact_with_dir_and_payload_and_bytes4_and5(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
    payload_byte4: Option<u8>,
    payload_byte5: Option<u8>,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1: None,
            payload_prefix2,
            payload_prefix4,
            payload_byte4,
            payload_byte5,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn packet_fact_with_dir_and_payload_and_bytes4_5_and9(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
    payload_byte4: Option<u8>,
    payload_byte5: Option<u8>,
    payload_byte9: Option<u8>,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1: None,
            payload_prefix2,
            payload_prefix4,
            payload_byte4,
            payload_byte5,
            payload_byte9,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn packet_fact_with_dir_and_payload_bytes(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_bytes: &[(u16, u8)],
) -> FactEnvelope {
    let byte_at = |target: u16| {
        payload_bytes
            .iter()
            .find_map(|(offset, value)| (*offset == target).then_some(*value))
    };
    let payload_byte0 = byte_at(0);
    let payload_byte1 = byte_at(1);
    let payload_byte2 = byte_at(2);
    let payload_byte3 = byte_at(3);
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: local_port.or(Some(42310)),
            remote_port: remote_port.or(Some(443)),
            payload_byte0,
            payload_byte1,
            payload_prefix2: payload_byte0
                .zip(payload_byte1)
                .map(|(b0, b1)| u16::from_be_bytes([b0, b1])),
            payload_prefix4: payload_byte0
                .zip(payload_byte1)
                .zip(payload_byte2)
                .zip(payload_byte3)
                .map(|(((b0, b1), b2), b3)| u32::from_be_bytes([b0, b1, b2, b3])),
            payload_byte4: byte_at(4),
            payload_byte5: byte_at(5),
            payload_byte9: byte_at(9),
            payload_byte10: byte_at(10),
            payload_byte13: byte_at(13),
            payload_bytes: payload_bytes.iter().copied().collect(),
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags,
            seq: Some(id as u32),
            ack: None,
            window: Some(65535),
        }),
    }
}

pub fn udp_packet_fact(id: u64, cookie: u64, tot_len: u32) -> FactEnvelope {
    udp_packet_fact_with_dir(id, cookie, tot_len, PacketDir::Egress)
}

pub fn udp_packet_fact_with_dir(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
) -> FactEnvelope {
    udp_packet_fact_with_dir_and_ports_and_payload(id, cookie, tot_len, dir, None, None, None, None)
}

pub fn udp_packet_fact_with_dir_and_ports(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
) -> FactEnvelope {
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        id,
        cookie,
        tot_len,
        dir,
        local_port,
        remote_port,
        None,
        None,
        None,
    )
}
