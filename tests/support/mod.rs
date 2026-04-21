use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact,
    SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::{handshake_debug_template, udp_debug_template, udp_process_debug_template};
use std::time::{Duration, SystemTime};

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

pub fn packet_fact_with_dir(
    id: u64,
    cookie: u64,
    tcp_flags: u16,
    dir: PacketDir,
) -> FactEnvelope {
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
            payload_prefix2,
            payload_prefix4,
            payload_byte4,
            payload_byte13: None,
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
    udp_packet_fact_with_dir_and_ports_and_payload(
        id, cookie, tot_len, dir, None, None, None, None,
    )
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

pub fn udp_packet_fact_with_dir_and_ports_and_byte(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
) -> FactEnvelope {
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        id,
        cookie,
        tot_len,
        dir,
        local_port,
        remote_port,
        payload_byte0,
        None,
        None,
    )
}

pub fn udp_packet_fact_with_dir_and_ports_and_payload(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
) -> FactEnvelope {
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
        id,
        cookie,
        tot_len,
        dir,
        local_port,
        remote_port,
        payload_byte0,
        payload_prefix2,
        None,
    )
}

pub fn udp_packet_fact_with_dir_and_ports_and_payload_prefix4(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
) -> FactEnvelope {
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
            dir,
            local_port,
            remote_port,
            payload_byte0,
            payload_prefix2,
            payload_prefix4,
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

pub fn udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
    payload_prefix2: Option<u16>,
    payload_prefix4: Option<u32>,
    payload_byte13: Option<u8>,
) -> FactEnvelope {
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
            dir,
            local_port,
            remote_port,
            payload_byte0,
            payload_prefix2,
            payload_prefix4,
            payload_byte4: None,
            payload_byte13,
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

pub fn sock_lineage_fact(id: u64, cookie: u64, pid: u32, comm: &str) -> FactEnvelope {
    let mut comm_bytes = [0u8; 16];
    let bytes = comm.as_bytes();
    let len = bytes.len().min(comm_bytes.len());
    comm_bytes[..len].copy_from_slice(&bytes[..len]);

    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "sock_lineage_fragment".into(),
        kind: FactKind::SockLineage(SockLineageFact {
            netns: 1,
            sk_cookie: cookie,
            pid,
            tid: pid,
            cgroup_id: 4242,
            comm: comm_bytes,
        }),
    }
}
