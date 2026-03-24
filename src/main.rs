use gewyvern::export::ExportBundle;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact, SessionId,
    TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::handshake_debug_template;
use std::time::{Duration, SystemTime};

fn main() {
    let template = handshake_debug_template();
    let config = SessionConfig::for_template(template)
        .expect("builtin template should be valid");
    let mut session = RuntimeSession::start(config).expect("session startup should succeed");

    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    session.ingest(FactEnvelope {
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
            dport: 443,
            family: 2,
            old: 1,
            new: 2,
        }),
    });
    session.ingest(FactEnvelope {
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
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: 60,
            tcp_flags: 0x02,
            seq: Some(1),
            ack: None,
            window: Some(65535),
        }),
    });
    session.ingest(FactEnvelope {
        id: FactId(3),
        ts: base + Duration::from_millis(20),
        cpu: CpuId(1),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "route_meta_fragment".into(),
        kind: FactKind::RouteDecision(RouteDecisionFact {
            netns: 1,
            sk_cookie: Some(42),
            fib_table: Some(254),
            oif: 2,
            gw: None,
        }),
    });
    session.freeze(base + Duration::from_secs(6));

    let export = session.export_bundle();
    let json = export.to_json();
    let replay = ExportBundle::from_json(&json)
        .expect("runtime should export replayable json")
        .replay()
        .expect("export should replay");

    println!(
        "template={} fragments={} flows={} reasons={} replay_consistent={}",
        replay.template_id,
        replay.attach_report.fragments_loaded.len(),
        replay.flows.len(),
        replay.reasons.len(),
        replay.reasons == export.reasons
    );
}
