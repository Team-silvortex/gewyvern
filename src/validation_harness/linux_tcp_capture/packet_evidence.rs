use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::{CaptureEvent, ExportBundle, ValidationError};
use crate::fragment::{AttachFailure, FragmentRegistry, HookPoint, MapKind, MapSpec};
use crate::ledger::{CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId};
use crate::runtime::{RuntimeSession, SessionConfig};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PacketEvent {
    pub sequence: u64,
    pub monotonic_ns: u64,
    pub cookie: u64,
    pub cpu: u16,
    pub ifindex: u32,
    pub source: u32,
    pub destination: u32,
    pub seq: u32,
    pub ack: u32,
    pub sport: u16,
    pub dport: u16,
    pub total_len: u16,
    pub window: u16,
    pub payload_len: u16,
    pub prefix_len: u16,
    pub flags: u8,
    pub ip_header_len: u8,
    pub tcp_header_len: u8,
    pub prefix: Vec<u8>,
}

const RULES: &str = include_str!("../../../ebpf/validation/http_prefix.gewy");

pub(super) fn packet_status_candidates(
    export: &ExportBundle,
) -> Result<Vec<(u64, u16)>, ValidationError> {
    let mut result = Vec::new();
    for flow in &export.program_flows {
        for stage in &flow.stages {
            let status = match stage.phase.as_deref() {
                Some("http_403_candidate") => 403,
                Some("http_500_candidate") => 500,
                _ => continue,
            };
            let cookie = export
                .facts
                .iter()
                .find_map(|fact| match &fact.kind {
                    FactKind::PacketMeta(packet) if fact.id == stage.at => packet.sk_cookie,
                    _ => None,
                })
                .ok_or_else(|| ValidationError::new("HTTP candidate has no packet evidence"))?;
            result.push((cookie, status));
        }
    }
    result.sort_unstable();
    result.dedup();
    Ok(result)
}

pub(super) fn reconstruct_packets(
    tcp: &ExportBundle,
    states: &[CaptureEvent],
    packets: &[PacketEvent],
    clock: (u64, SystemTime),
) -> Result<ExportBundle, ValidationError> {
    let port = states.first().map_or(0, |state| state.dport);
    let rules = RULES.replace("remote:3000:", &format!("remote:{port}:"));
    reconstruct_packets_with_rules(tcp, states, packets, clock, &rules)
}

pub(super) fn reconstruct_packets_with_rules(
    tcp: &ExportBundle,
    states: &[CaptureEvent],
    packets: &[PacketEvent],
    clock: (u64, SystemTime),
    rules: &str,
) -> Result<ExportBundle, ValidationError> {
    if packets.is_empty() || packets.len() > 128 || states.len() != tcp.facts.len() {
        return Err(ValidationError::new("invalid packet evidence count"));
    }
    let remote_port = states
        .first()
        .map(|s| s.dport)
        .ok_or_else(|| ValidationError::new("packet capture has no TCP states"))?;
    if states.iter().any(|s| s.dport != remote_port) {
        return Err(ValidationError::new(
            "packet fixture requires one owned server port",
        ));
    }
    let binding =
        crate::dsl::compile_str(rules).map_err(|e| ValidationError::new(format!("{e:?}")))?;
    let mut config =
        SessionConfig::for_binding(binding).map_err(|e| ValidationError::new(e.to_string()))?;
    let mut registry = FragmentRegistry::new();
    for id in &config.template.fragment_set {
        let mut descriptor = if id == "tcp_state_fragment" {
            tcp.attach_plan.fragments.iter().find(|f| f.id == *id)
        } else {
            config.registry.descriptor(id)
        }
        .ok_or_else(|| ValidationError::new("missing capture descriptor"))?
        .clone();
        if id == "tcp_packet_meta_fragment" {
            descriptor.hookpoints = vec![HookPoint::CgroupSkbIngress];
            descriptor.sampled_payload_offsets = (0..64).collect();
            descriptor.maps = vec![
                MapSpec {
                    name: "events".into(),
                    kind: MapKind::Hash,
                    max_entries: 128,
                },
                MapSpec {
                    name: "counts".into(),
                    kind: MapKind::Hash,
                    max_entries: 1,
                },
            ];
            descriptor
                .emits
                .retain(|kind| *kind == crate::ledger::FactKindTag::PacketMeta);
            descriptor
                .evidence_classes
                .retain(|spec| spec.fact_kind == crate::ledger::FactKindTag::PacketMeta);
        } else if id != "tcp_state_fragment" {
            // A package can declare more probes than this fixture actually observes.
            // Preserve those declarations as unavailable, never successful attachments.
            config
                .attach_failures
                .extend(descriptor.hookpoints.iter().map(|hookpoint| AttachFailure {
                    fragment_id: id.clone(),
                    hookpoint: hookpoint.clone(),
                    error: "not observed by bounded ingress/state validation capture".into(),
                }));
        }
        registry
            .register(descriptor)
            .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    }
    config.registry = registry;
    let mut facts: Vec<_> = states
        .iter()
        .zip(&tcp.facts)
        .map(|(state, fact)| (state.monotonic_ns, fact.clone()))
        .collect();
    for (index, packet) in packets.iter().enumerate() {
        let state = states
            .iter()
            .find(|s| s.cookie == packet.cookie)
            .ok_or_else(|| ValidationError::new("packet has no captured TCP socket identity"))?;
        let header_len = u16::from(packet.ip_header_len) + u16::from(packet.tcp_header_len);
        let syn_ack = packet.flags & 0x17 == 0x12;
        let in_lifetime = if syn_ack {
            packet.payload_len == 0
                && states.iter().any(|s| {
                    s.cookie == packet.cookie
                        && s.new_state == 2
                        && s.monotonic_ns <= packet.monotonic_ns
                })
                && states.iter().any(|s| {
                    s.cookie == packet.cookie
                        && s.new_state == 1
                        && s.monotonic_ns >= packet.monotonic_ns
                })
        } else {
            packet.payload_len > 0
                && states.iter().any(|s| {
                    s.cookie == packet.cookie
                        && s.new_state == 1
                        && s.monotonic_ns <= packet.monotonic_ns
                })
        };
        if packet.sequence != index as u64 + 1
            || packet.source != u32::from_be_bytes(state.daddr)
            || packet.destination != u32::from_be_bytes(state.saddr)
            || packet.sport != state.dport
            || packet.dport != state.sport
            || !(20..=60).contains(&packet.ip_header_len)
            || packet.ip_header_len % 4 != 0
            || !(20..=60).contains(&packet.tcp_header_len)
            || packet.tcp_header_len % 4 != 0
            || packet.total_len.checked_sub(header_len) != Some(packet.payload_len)
            || !in_lifetime
            || packet.prefix_len != packet.payload_len.min(64)
            || packet.prefix.len() != 64
            || packet.prefix[usize::from(packet.prefix_len)..]
                .iter()
                .any(|b| *b != 0)
            || packet.monotonic_ns < clock.0
            || packet.monotonic_ns - clock.0 > 30_000_000_000
            || !states.iter().any(|s| {
                s.cookie == packet.cookie
                    && s.new_state == 7
                    && s.monotonic_ns >= packet.monotonic_ns
            })
        {
            return Err(ValidationError::new(
                "packet identity, framing or lifetime mismatch",
            ));
        }
        let bytes = &packet.prefix[..usize::from(packet.prefix_len)];
        let at = |n| bytes.get(n).copied();
        let ts = clock
            .1
            .checked_add(Duration::from_nanos(packet.monotonic_ns - clock.0))
            .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
            .and_then(|d| u64::try_from(d.as_millis()).ok())
            .and_then(|ms| SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(ms)))
            .ok_or_else(|| ValidationError::new("packet clock overflow"))?;
        facts.push((
            packet.monotonic_ns,
            FactEnvelope {
                id: FactId(0),
                ts,
                cpu: CpuId(packet.cpu),
                ifindex: Some(packet.ifindex),
                session: SessionId(1),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: state.netns,
                    sk_cookie: Some(packet.cookie),
                    dir: PacketDir::Ingress,
                    local_port: Some(packet.dport),
                    remote_port: Some(packet.sport),
                    payload_byte0: at(0),
                    payload_byte1: at(1),
                    payload_prefix2: bytes.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])),
                    payload_prefix4: bytes
                        .get(..4)
                        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
                    payload_byte4: at(4),
                    payload_byte5: at(5),
                    payload_byte9: at(9),
                    payload_byte10: at(10),
                    payload_byte13: at(13),
                    payload_bytes: bytes
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (i as u16, *b))
                        .collect(),
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: u32::from(packet.total_len),
                    tcp_flags: u16::from(packet.flags),
                    seq: Some(packet.seq),
                    ack: Some(packet.ack),
                    window: Some(packet.window),
                }),
            },
        ));
    }
    facts.sort_by_key(|(ns, _)| *ns);
    let mut session =
        RuntimeSession::start(config).map_err(|e| ValidationError::new(e.to_string()))?;
    let expected_count = facts.len();
    for (index, (_, mut fact)) in facts.into_iter().enumerate() {
        fact.id = FactId(index as u64 + 1);
        session.ingest(fact);
    }
    let mut export = session.into_export_bundle();
    export.ingest_trust_mode = "unverified-local".into();
    if !export.rejected_facts.is_empty()
        || export.facts.len() != expected_count
        || export.flows.len() != tcp.flows.len()
    {
        return Err(ValidationError::new(
            "runtime rejected or misjoined packet evidence",
        ));
    }
    let decoded = ExportBundle::from_json(&export.to_json())
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    let replay = decoded
        .replay()
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    if replay.facts != export.facts
        || replay.flows != export.flows
        || replay.reasons != export.reasons
        || replay.program_flows != export.program_flows
        || replay.protocol_ir != export.protocol_ir
    {
        return Err(ValidationError::new("packet evidence replay diverged"));
    }
    Ok(export)
}

#[cfg(test)]
mod tests {
    use super::super::reconstruct_at;
    use super::*;

    fn sample() -> (ExportBundle, Vec<CaptureEvent>, Vec<PacketEvent>) {
        let mut states = Vec::new();
        let mut packets = Vec::new();
        for (index, prefix) in [
            b"HTTP/1.1 403 Forbidden".as_slice(),
            b"HTTP/1.1 500 Internal Server Error",
        ]
        .into_iter()
        .enumerate()
        {
            let cookie = index as u64 + 1;
            for (offset, old_state, new_state) in [(0, 2, 1), (10, 1, 7)] {
                states.push(CaptureEvent {
                    sequence: states.len() as u64 + 1,
                    monotonic_ns: 10 + index as u64 * 20 + offset,
                    cookie,
                    netns: 42,
                    cpu: 0,
                    sport: 4000 + index as u16,
                    dport: 3000,
                    family: 2,
                    old_state,
                    new_state,
                    saddr: [172, 20, 0, 1],
                    daddr: [172, 20, 0, 2],
                });
            }
            let mut bytes = vec![0; 64];
            bytes[..prefix.len()].copy_from_slice(prefix);
            packets.push(PacketEvent {
                sequence: cookie,
                monotonic_ns: 15 + index as u64 * 20,
                cookie,
                cpu: 0,
                ifindex: 7,
                source: u32::from_be_bytes([172, 20, 0, 2]),
                destination: u32::from_be_bytes([172, 20, 0, 1]),
                seq: 123,
                ack: 456,
                sport: 3000,
                dport: 4000 + index as u16,
                total_len: 40 + prefix.len() as u16,
                window: 2048,
                payload_len: prefix.len() as u16,
                prefix_len: prefix.len() as u16,
                flags: 0x18,
                ip_header_len: 20,
                tcp_header_len: 20,
                prefix: bytes,
            });
        }
        let tcp = reconstruct_at(
            &states,
            &[(1, 4000, 3000, 42), (2, 4001, 3000, 42)],
            4,
            0,
            (0, SystemTime::UNIX_EPOCH),
            ([172, 20, 0, 1], [172, 20, 0, 2]),
        )
        .unwrap();
        (tcp, states, packets)
    }

    #[test]
    fn packet_bytes_drive_dsl_status_and_replay() {
        let (tcp, states, packets) = sample();
        let export =
            reconstruct_packets(&tcp, &states, &packets, (0, SystemTime::UNIX_EPOCH)).unwrap();
        assert_eq!(
            packet_status_candidates(&export).unwrap(),
            vec![(1, 403), (2, 500)]
        );
        assert_eq!(export.facts.len(), 6);
        assert_eq!(
            export
                .attach_plan
                .fragments
                .iter()
                .find(|f| f.id == "tcp_packet_meta_fragment")
                .unwrap()
                .hookpoints,
            vec![HookPoint::CgroupSkbIngress]
        );
    }

    #[test]
    fn existing_ftp_rules_preserve_missing_process_and_route_capture() {
        let (_, mut states, mut packets) = sample();
        for state in &mut states {
            state.dport = 21;
        }
        for (packet, bytes) in packets
            .iter_mut()
            .zip([b"530 Denied\r\n".as_slice(), b"230 Welcome\r\n"])
        {
            packet.sport = 21;
            packet.prefix.fill(0);
            packet.prefix[..bytes.len()].copy_from_slice(bytes);
            packet.payload_len = bytes.len() as u16;
            packet.prefix_len = bytes.len() as u16;
            packet.total_len = 40 + bytes.len() as u16;
        }
        let tcp = reconstruct_at(
            &states,
            &[(1, 4000, 21, 42), (2, 4001, 21, 42)],
            4,
            0,
            (0, SystemTime::UNIX_EPOCH),
            ([172, 20, 0, 1], [172, 20, 0, 2]),
        )
        .unwrap();
        let export = reconstruct_packets_with_rules(
            &tcp,
            &states,
            &packets,
            (0, SystemTime::UNIX_EPOCH),
            include_str!("../../../protocols/ftp/denied/main.gewy"),
        )
        .unwrap();
        assert_eq!(export.facts.len(), 6);
        for id in ["route_meta_fragment", "sock_lineage_fragment"] {
            assert!(
                !export
                    .attach_report
                    .fragments_loaded
                    .iter()
                    .any(|name| name == id)
            );
            assert!(
                export
                    .attach_report
                    .hookpoints_failed
                    .iter()
                    .any(|name| name.starts_with(id))
            );
        }
        let denied: Vec<_> = export
            .program_flows
            .iter()
            .flat_map(|flow| &flow.stages)
            .filter(|stage| stage.phase.as_deref() == Some("receive_auth_denied"))
            .collect();
        assert_eq!(denied.len(), 1);
        assert!(export.facts.iter().any(|fact| fact.id == denied[0].at
            && matches!(&fact.kind, FactKind::PacketMeta(packet) if packet.sk_cookie == Some(1))));
        assert!(export.flows.iter().all(|flow| flow.process.is_none()));
    }

    #[test]
    fn rejects_foreign_identity_malformed_prefix_and_outside_lifetime() {
        let (tcp, states, packets) = sample();
        let mutations: [fn(&mut PacketEvent); 9] = [
            |p| p.cookie = 99,
            |p| p.dport += 1,
            |p| p.source += 1,
            |p| p.total_len += 1,
            |p| p.prefix_len = 65,
            |p| {
                p.prefix.pop();
            },
            |p| p.prefix[63] = 1,
            |p| p.monotonic_ns = 21,
            |p| p.sequence = 7,
        ];
        for mutate in mutations {
            let mut bad = packets.clone();
            mutate(&mut bad[0]);
            assert!(reconstruct_packets(&tcp, &states, &bad, (0, SystemTime::UNIX_EPOCH)).is_err());
        }
    }

    #[test]
    fn status_digits_without_http_start_line_do_not_classify() {
        let (tcp, states, mut packets) = sample();
        packets[0].prefix[0] = b'X';
        let export =
            reconstruct_packets(&tcp, &states, &packets, (0, SystemTime::UNIX_EPOCH)).unwrap();
        assert_eq!(packet_status_candidates(&export).unwrap(), vec![(2, 500)]);
        packets[1].prefix[9..12].copy_from_slice(b"200");
        let export =
            reconstruct_packets(&tcp, &states, &packets, (0, SystemTime::UNIX_EPOCH)).unwrap();
        assert!(packet_status_candidates(&export).unwrap().is_empty());
    }

    #[test]
    fn recorded_kernel_prefixes_replay_into_http_statuses() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/fixtures/gewyvern_packet_capture_linux_x86_64_20261104.json"
        ))
        .unwrap();
        let sample = &fixture["capture_sample"];
        let summary = &sample["summary"];
        let states: Vec<CaptureEvent> = serde_json::from_value(sample["events"].clone()).unwrap();
        let packets: Vec<PacketEvent> = serde_json::from_value(sample["packets"].clone()).unwrap();
        let expected: Vec<(u64, u16, u16, u32)> =
            serde_json::from_value(summary["selected_sockets"].clone()).unwrap();
        let addresses = serde_json::from_value(summary["addresses"].clone()).unwrap();
        let anchor = summary["clock_anchor_unix_ns"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap();
        let clock = (
            summary["clock_anchor_monotonic_ns"].as_u64().unwrap(),
            SystemTime::UNIX_EPOCH
                + Duration::new(
                    (anchor / 1_000_000_000) as u64,
                    (anchor % 1_000_000_000) as u32,
                ),
        );
        let tcp =
            reconstruct_at(&states, &expected, states.len() as u64, 0, clock, addresses).unwrap();
        let export = reconstruct_packets(&tcp, &states, &packets, clock).unwrap();
        let statuses: Vec<(u64, u16)> =
            serde_json::from_value(summary["kernel_dsl_http_statuses"].clone()).unwrap();
        assert_eq!(packet_status_candidates(&export).unwrap(), statuses);
        assert_eq!(export.facts.len(), 12);
        assert_eq!(export.flows.len(), 2);
        let prefixes = super::super::response_prefix::verified_replay(&export).unwrap();
        assert!(
            prefixes
                .iter()
                .all(|p| p.outcome == super::super::response_prefix::Outcome::MissingOrigin)
        );
    }

    #[test]
    fn recorded_split_and_body_decoy_replay_without_promoting_packet_candidates() {
        use super::super::response_prefix::{ResponsePrefix, verified_replay, verified_statuses};
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/fixtures/gewyvern_http_boundaries_linux_x86_64_20261104.json"
        ))
        .unwrap();
        let sample = &fixture["capture_sample"];
        let summary = &sample["summary"];
        let states: Vec<CaptureEvent> = serde_json::from_value(sample["events"].clone()).unwrap();
        let packets: Vec<PacketEvent> = serde_json::from_value(sample["packets"].clone()).unwrap();
        let expected = serde_json::from_value::<Vec<(u64, u16, u16, u32)>>(
            summary["selected_sockets"].clone(),
        )
        .unwrap();
        let anchor = summary["clock_anchor_unix_ns"]
            .as_str()
            .unwrap()
            .parse::<u128>()
            .unwrap();
        let clock = (
            summary["clock_anchor_monotonic_ns"].as_u64().unwrap(),
            SystemTime::UNIX_EPOCH
                + Duration::new(
                    (anchor / 1_000_000_000) as u64,
                    (anchor % 1_000_000_000) as u32,
                ),
        );
        let tcp = reconstruct_at(
            &states,
            &expected,
            states.len() as u64,
            0,
            clock,
            ([127, 0, 0, 1], [127, 0, 0, 1]),
        )
        .unwrap();
        let export = reconstruct_packets(&tcp, &states, &packets, clock).unwrap();
        let prefixes = verified_replay(&export).unwrap();
        assert_eq!(
            prefixes,
            serde_json::from_value::<Vec<ResponsePrefix>>(sample["prefixes"].clone()).unwrap()
        );
        assert_eq!(
            verified_statuses(&prefixes),
            serde_json::from_value::<Vec<(u64, u16)>>(summary["verified_http_statuses"].clone())
                .unwrap()
        );
        assert_eq!(
            packet_status_candidates(&export).unwrap(),
            vec![(expected[1].0, 500)]
        );
        assert_eq!(export.facts.len(), 16);
        let mut bad = packets.clone();
        bad[0].monotonic_ns = bad[1].monotonic_ns;
        assert!(
            reconstruct_packets(&tcp, &states, &bad, clock).is_err(),
            "late SYN-ACK cannot supply a fabricated origin"
        );
    }
}
