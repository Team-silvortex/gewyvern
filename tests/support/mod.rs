use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact,
    SessionId, TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::{handshake_debug_template, udp_debug_template};
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

pub fn tcp_state_fact(id: u64, cookie: u64, old: u8, new: u8) -> FactEnvelope {
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
            sport: 12345,
            dport: 443,
            family: 2,
            old,
            new,
        }),
    }
}

pub fn packet_fact(id: u64, cookie: u64, tcp_flags: u16) -> FactEnvelope {
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
            dir: PacketDir::Egress,
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
