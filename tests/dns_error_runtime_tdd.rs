use std::time::SystemTime;

use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{ProgramFindingCause, ProgramOperation};
use gewyvern::fragment::AttachFailure;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};

const ERROR_CASES: [(u8, &str); 4] = [
    (1, "receive_formerr"),
    (2, "receive_servfail"),
    (3, "receive_nxdomain"),
    (5, "receive_refused"),
];

fn packet_only_config() -> SessionConfig {
    let package = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("protocols/dns/error");
    let binding = compile_file(package.to_str().unwrap()).unwrap();
    let mut config = SessionConfig::for_binding(binding).unwrap();
    for id in ["route_meta_fragment", "sock_lineage_fragment"] {
        config.attach_failures.extend(
            config
                .registry
                .descriptor(id)
                .unwrap()
                .hookpoints
                .iter()
                .map(|hookpoint| AttachFailure {
                    fragment_id: id.into(),
                    hookpoint: hookpoint.clone(),
                    error: "not observed by packet-only DNS test".into(),
                }),
        );
    }
    config
}

fn response_header(rcode: u8) -> [u8; 12] {
    [0x12, 0x34, 0x80, rcode, 0, 0, 0, 0, 0, 0, 0, 0]
}

fn packet_fact(payload: &[u8]) -> FactEnvelope {
    let at = |offset| payload.get(offset).copied();
    FactEnvelope {
        id: FactId(1),
        ts: SystemTime::UNIX_EPOCH,
        cpu: CpuId(0),
        ifindex: Some(1),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(53),
            dir: PacketDir::Ingress,
            local_port: Some(53000),
            remote_port: Some(53),
            payload_byte0: at(0),
            payload_byte1: at(1),
            payload_prefix2: payload.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])),
            payload_prefix4: payload
                .get(..4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            payload_byte4: at(4),
            payload_byte5: at(5),
            payload_byte9: at(9),
            payload_byte10: at(10),
            payload_byte13: at(13),
            payload_bytes: payload
                .iter()
                .enumerate()
                .map(|(offset, byte)| (offset as u16, *byte))
                .collect(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len: 28 + payload.len() as u32,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

fn run(config: SessionConfig, fact: FactEnvelope) -> ExportBundle {
    let mut session = RuntimeSession::start(config).unwrap();
    session.ingest(fact.clone());
    let export = session.into_export_bundle();
    assert_eq!(export.facts, vec![fact]);
    assert!(export.rejected_facts.is_empty());
    assert_eq!(
        export.attach_report.fragments_loaded,
        ["udp_packet_meta_fragment"]
    );
    assert_eq!(export.attach_report.hookpoints_failed.len(), 2);
    let flow = &export.flows[0];
    assert!(flow.process.is_none());
    assert!(flow.path.segments.is_empty());
    assert!(flow.evidence.tcp_state_facts.is_empty());
    assert!(flow.evidence.route_facts.is_empty());
    assert!(flow.evidence.lineage_facts.is_empty());
    let decoded = ExportBundle::from_json(&export.to_json()).unwrap();
    assert_eq!(decoded, export);
    let replay = decoded.replay().unwrap();
    assert_eq!(replay.facts, export.facts);
    assert_eq!(replay.flows, export.flows);
    assert_eq!(replay.program_flows, export.program_flows);
    assert_eq!(replay.protocol_ir, export.protocol_ir);
    assert_eq!(replay.program_findings, export.program_findings);
    export
}

fn assert_no_dns_error(export: &ExportBundle) {
    assert!(export.program_flows[0].stages.is_empty());
    assert!(export.protocol_ir.is_empty());
    assert!(export.program_findings.is_empty());
}

#[test]
fn dns_error_header_offsets_are_declared_and_supported() {
    let export = run(packet_only_config(), packet_fact(&response_header(3)));
    let rules = &export
        .binding_diagnostics
        .program_model
        .as_ref()
        .unwrap()
        .rules;
    assert_eq!(rules.len(), 6);
    for rule in &rules[2..] {
        assert!(rule.supported);
        assert!(rule.unsupported_payload_offsets.is_empty());
    }
}

#[test]
fn dns_error_each_rcode_materializes_ir_without_missing_alternatives() {
    for (rcode, phase) in ERROR_CASES {
        let export = run(packet_only_config(), packet_fact(&response_header(rcode)));
        let stages = &export.program_flows[0].stages;
        assert_eq!(stages.len(), 1, "rcode {rcode}");
        assert_eq!(stages[0].phase.as_deref(), Some(phase));
        assert_eq!(stages[0].at, FactId(1));
        assert!(export.program_findings.is_empty(), "rcode {rcode}");
        assert_eq!(export.protocol_ir.len(), 1, "rcode {rcode}");
        assert_eq!(export.protocol_ir[0].operation, "dns_error");
        assert_eq!(export.protocol_ir[0].protocol, "dns");
        assert_eq!(export.protocol_ir[0].entry, "error");
    }
}

#[test]
fn dns_error_short_headers_remain_packet_evidence_without_classification() {
    for (rcode, _) in ERROR_CASES {
        for length in 0..12 {
            let fact = packet_fact(&response_header(rcode)[..length]);
            assert_no_dns_error(&run(packet_only_config(), fact));
        }
    }
}

#[test]
fn dns_error_requires_observed_header_end_not_total_packet_length() {
    let mut fact = packet_fact(&response_header(3));
    let FactKind::PacketMeta(packet) = &mut fact.kind else {
        unreachable!()
    };
    packet.payload_bytes.remove(&11);
    assert_no_dns_error(&run(packet_only_config(), fact));
}

#[test]
fn dns_error_noerror_queries_and_wrong_scope_do_not_classify() {
    assert_no_dns_error(&run(packet_only_config(), packet_fact(&response_header(0))));
    let mut query_header = response_header(3);
    query_header[2] = 0;
    assert_no_dns_error(&run(packet_only_config(), packet_fact(&query_header)));
    for mutate in [
        (|packet: &mut PacketMetaFact| packet.remote_port = Some(54)) as fn(&mut PacketMetaFact),
        |packet| packet.dir = PacketDir::Egress,
        |packet| packet.l4_proto = 6,
    ] {
        let mut fact = packet_fact(&response_header(3));
        let FactKind::PacketMeta(packet) = &mut fact.kind else {
            unreachable!()
        };
        mutate(packet);
        assert_no_dns_error(&run(packet_only_config(), fact));
    }
}

#[test]
fn dns_error_alternative_handling_does_not_change_other_operations() {
    let mut config = packet_only_config();
    config.template.program_model.as_mut().unwrap().operation =
        ProgramOperation::Custom("unrelated_error_sequence".into());
    let export = run(config, packet_fact(&response_header(1)));
    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(
        export.program_findings[0].phase.as_deref(),
        Some("receive_servfail")
    );
    assert_eq!(
        export.program_findings[0].cause,
        ProgramFindingCause::MissingCoreStage
    );
}

#[test]
fn dns_error_alternatives_do_not_suppress_other_modules() {
    let mut config = packet_only_config();
    let rules = &mut config.template.program_model.as_mut().unwrap().rules;
    rules
        .iter_mut()
        .find(|rule| rule.phase.as_deref() == Some("receive_refused"))
        .unwrap()
        .module = Some("independent_error_module".into());
    let export = run(config, packet_fact(&response_header(3)));
    assert_eq!(export.program_findings.len(), 1);
    assert_eq!(
        export.program_findings[0].module_label,
        "independent_error_module"
    );
    assert_eq!(
        export.program_findings[0].phase.as_deref(),
        Some("receive_refused")
    );
}
