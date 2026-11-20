// Fact fixtures keep protocol fields explicit at call sites for auditability.
#![allow(clippy::too_many_arguments)]

use gewyvern::export::ExportBundle;
use gewyvern::flow::{ProcessView, ProgramFinding, ProgramFindingCause};
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId, SockLineageFact,
    TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::TemplateBinding;
use std::time::{Duration, SystemTime};

use super::{AnalysisAugmenter, AnalysisSnapshot, push_analysis_augmentation};

pub(super) fn payload_bytes_for_tests(bytes: &[u8]) -> Vec<(u16, u8)> {
    bytes
        .iter()
        .copied()
        .enumerate()
        .map(|(i, b)| (i as u16, b))
        .collect()
}

pub(super) fn snmp_payload_bytes_for_tests(pdu: u8) -> Vec<(u16, u8)> {
    fn tlv(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = vec![tag, body.len() as u8];
        bytes.extend_from_slice(body);
        bytes
    }
    let binding = tlv(0x30, &[6, 8, 43, 6, 1, 2, 1, 1, 6, 0, 5, 0]);
    let mut body = vec![2, 1, 1, 2, 1, 0, 2, 1, u8::from(pdu == 0xa5)];
    body.extend(tlv(0x30, &binding));
    let mut message = vec![2, 1, 1];
    message.extend(tlv(4, b"public"));
    message.extend(tlv(pdu, &body));
    payload_bytes_for_tests(&tlv(0x30, &message))
}

pub(super) fn stun_payload_bytes_for_tests(kind: u16) -> Vec<(u16, u8)> {
    let mut bytes = kind.to_be_bytes().to_vec();
    bytes.extend([0, 0, 0x21, 0x12, 0xa4, 0x42]);
    bytes.extend([0x5a; 12]);
    match kind {
        0x0101 => bytes.extend([0, 0x20, 0, 8, 0, 1, 0x32, 0x9a, 0x5e, 0x12, 0xa4, 0x43]),
        0x0111 => bytes.extend([0, 9, 0, 4, 0, 0, 4, 20]),
        _ => {}
    }
    let length = (bytes.len() - 20) as u16;
    bytes[2..4].copy_from_slice(&length.to_be_bytes());
    bytes
        .into_iter()
        .enumerate()
        .map(|(i, b)| (i as u16, b))
        .collect()
}

pub(super) fn ntp_payload_bytes_for_tests(flags: u8) -> Vec<(u16, u8)> {
    let mut bytes = [0; 48];
    bytes[0] = flags;
    bytes[1] = if flags >> 6 == 3 { 0 } else { 10 };
    bytes[31] = 1;
    bytes[39] = 2;
    bytes[47] = 3;
    bytes
        .into_iter()
        .enumerate()
        .map(|(i, b)| (i as u16, b))
        .collect()
}
pub(super) struct MlHookAugmenter;

impl AnalysisAugmenter for MlHookAugmenter {
    fn augment(&self, _export: &ExportBundle, snapshot: &mut AnalysisSnapshot) {
        snapshot.primary_failure_confidence = "ml-candidate".into();
        snapshot
            .competing_hypotheses
            .push("augmenter:ml_rerank_hook".into());
        push_analysis_augmentation(
            snapshot,
            "ml-hook",
            "ml_rerank_hook",
            "placeholder augmentation slot for future rerank/enrich passes",
            "advisory",
            Some("candidate".into()),
            Some("MlHookAugmenter".into()),
            Some("{\"source\":\"test\"}".into()),
        );
    }
}

pub(super) fn push_synthetic_missing_stage_finding(
    export: &mut gewyvern::export::ExportBundle,
    flow: &gewyvern::flow::ProgramFlow,
    module_label: &str,
    network_module_kind: &str,
    phase: &str,
    phase_kind: &str,
    phase_transition: &str,
    phase_transition_kind: &str,
    suspect_area: &str,
    summary: &str,
    supporting_fragment: &str,
    evidence_trace: &str,
) {
    export.program_findings.push(ProgramFinding {
        program_flow: flow.id,
        process: flow.process.clone(),
        operation: flow.operation.clone(),
        module_label: module_label.into(),
        network_module_kind: network_module_kind.into(),
        phase: Some(phase.into()),
        phase_kind: Some(phase_kind.into()),
        phase_transition: Some(phase_transition.into()),
        phase_transition_kind: Some(phase_transition_kind.into()),
        suspect_area: suspect_area.into(),
        cause: ProgramFindingCause::MissingCoreStage,
        summary: summary.into(),
        supporting_fragments: vec![supporting_fragment.into()],
        evidence_trace: vec![evidence_trace.into()],
    });
}

pub(super) fn synthetic_process_view(pid: u32, comm: &str) -> ProcessView {
    ProcessView {
        pid,
        tid: pid,
        cgroup_id: 4242,
        comm: comm.into(),
    }
}

pub(super) fn coerce_export_process(
    mut export: gewyvern::export::ExportBundle,
    process: &ProcessView,
) -> gewyvern::export::ExportBundle {
    for flow in &mut export.program_flows {
        flow.process = Some(process.clone());
    }
    for finding in &mut export.program_findings {
        finding.process = Some(process.clone());
    }
    for finding in &mut export.module_findings {
        finding.process = Some(process.clone());
    }
    export
}

pub(super) fn merge_exports_for_tests(
    exports: Vec<gewyvern::export::ExportBundle>,
) -> gewyvern::export::ExportBundle {
    let mut iter = exports.into_iter();
    let mut merged = iter.next().expect("expected at least one export");
    for export in iter {
        merged.facts.extend(export.facts);
        merged.rejected_facts.extend(export.rejected_facts);
        merged
            .rejected_fact_summary
            .extend(export.rejected_fact_summary);
        merged.flows.extend(export.flows);
        merged.program_flows.extend(export.program_flows);
        merged.program_findings.extend(export.program_findings);
        merged.module_findings.extend(export.module_findings);
        merged.reasons.extend(export.reasons);
    }
    merged.debug_summary.accepted_facts = merged.facts.len() as u64;
    merged.debug_summary.rejected_facts = merged.rejected_facts.len() as u64;
    merged.debug_summary.flows = merged.flows.len() as u64;
    merged.debug_summary.program_flows = merged.program_flows.len() as u64;
    merged.debug_summary.program_findings = merged.program_findings.len() as u64;
    merged.debug_summary.module_findings = merged.module_findings.len() as u64;
    merged.debug_summary.reasons = merged.reasons.len() as u64;
    merged
}

pub(super) fn sock_lineage_fact_for_tests(
    id: u64,
    cookie: u64,
    pid: u32,
    comm: &str,
) -> FactEnvelope {
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

pub(super) fn tcp_state_fact_with_ports_for_tests(
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

pub(super) fn packet_fact_with_dir_and_payload_for_tests(
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
    FactEnvelope {
        id: FactId(id),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(id * 10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            udp_offload: None,
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

pub(super) fn packet_fact_with_dir_and_payload_bytes_for_tests(
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
            udp_offload: None,
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

pub(super) fn udp_packet_fact_with_dir_and_ports_and_payload_for_tests(
    id: u64,
    cookie: u64,
    tot_len: u32,
    dir: PacketDir,
    local_port: Option<u16>,
    remote_port: Option<u16>,
    payload_byte0: Option<u8>,
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
            udp_offload: None,
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port,
            remote_port,
            payload_byte0,
            payload_byte1: None,
            payload_prefix2: None,
            payload_prefix4,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: std::collections::BTreeMap::new(),
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

pub(super) fn udp_packet_fact_with_payload_bytes_for_tests(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    local_port: u16,
    remote_port: u16,
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
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            udp_offload: None,
            netns: 1,
            sk_cookie: Some(cookie),
            dir,
            local_port: Some(local_port),
            remote_port: Some(remote_port),
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
            l4_proto: 17,
            tot_len: 128,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

pub(super) fn packet_fact_with_l4_proto_and_payload_bytes_for_tests(
    id: u64,
    cookie: Option<u64>,
    dir: PacketDir,
    l4_proto: u8,
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
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            udp_offload: None,
            netns: 1,
            sk_cookie: cookie,
            dir,
            local_port: None,
            remote_port: None,
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
            l4_proto,
            tot_len: 128,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

pub(super) fn export_from_test_facts(
    binding: TemplateBinding,
    facts: Vec<FactEnvelope>,
) -> ExportBundle {
    let config = SessionConfig::for_binding(binding).expect("binding should validate");
    let mut session = RuntimeSession::start(config).expect("session should start");
    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(SystemTime::UNIX_EPOCH + Duration::from_millis(100));
    session.export_bundle()
}
