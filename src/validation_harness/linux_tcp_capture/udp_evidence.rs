use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::ValidationError;
use crate::export::ExportBundle;
use crate::fragment::{AttachFailure, FragmentRegistry, HookPoint, MapKind, MapSpec};
#[cfg(test)]
use crate::ledger::FactKind;
use crate::ledger::{FactId, FactKindTag, PacketDir, SessionId};
use crate::packet_capture::{CapturedUdpDatagram, PacketObservation};
use crate::runtime::{RuntimeSession, SessionConfig};

#[derive(Clone, Copy, Debug)]
pub(super) enum Addresses {
    V4([u8; 4], [u8; 4]),
    V6([u8; 16], [u8; 16]),
}

impl From<([u8; 4], [u8; 4])> for Addresses {
    fn from((source, destination): ([u8; 4], [u8; 4])) -> Self {
        Self::V4(source, destination)
    }
}

impl Addresses {
    pub fn version(self) -> u8 {
        match self {
            Self::V4(..) => 4,
            Self::V6(..) => 6,
        }
    }

    pub fn octets(self) -> ([u8; 16], [u8; 16]) {
        match self {
            Self::V4(source, destination) => {
                let (mut local, mut remote) = ([0; 16], [0; 16]);
                local[..4].copy_from_slice(&source);
                remote[..4].copy_from_slice(&destination);
                (local, remote)
            }
            Self::V6(source, destination) => (source, destination),
        }
    }

    pub fn l3_proto(self) -> u16 {
        match self {
            Self::V4(..) => 0x0800,
            Self::V6(..) => 0x86dd,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Datagram {
    pub sequence: u64,
    pub monotonic_ns: u64,
    pub cookie: u64,
    pub cpu: u32,
    pub ifindex: u32,
    pub source: [u8; 16],
    pub destination: [u8; 16],
    pub sport: u16,
    pub dport: u16,
    pub total_len: u32,
    pub payload_len: u16,
    pub prefix_len: u16,
    pub ip_header_len: u8,
    pub ip_version: u8,
    pub prefix: Vec<u8>,
}

pub(super) struct Context {
    pub sockets: [(u64, u16); 2],
    pub remote_port: u16,
    pub addresses: Addresses,
    pub netns: u32,
    pub clock: (u64, SystemTime),
    pub end_ns: u64,
}

pub(super) fn validate<'a>(
    event: &'a Datagram,
    egress: bool,
    context: &Context,
) -> Result<&'a [u8], ValidationError> {
    let port = context
        .sockets
        .iter()
        .find(|(cookie, _)| *cookie == event.cookie)
        .map(|(_, port)| *port);
    let (local, remote) = context.addresses.octets();
    let tuple = if egress {
        (event.source, event.destination, event.sport, event.dport)
    } else {
        (event.destination, event.source, event.dport, event.sport)
    };
    if context.remote_port == 0
        || context
            .sockets
            .iter()
            .any(|(cookie, port)| *cookie == 0 || *port == 0)
        || context.sockets[0].0 == context.sockets[1].0
        || context.sockets[0].1 == context.sockets[1].1
        || context.end_ns < context.clock.0
        || port.is_none()
        || tuple != (local, remote, port.unwrap_or_default(), context.remote_port)
        || event.ip_version != context.addresses.version()
        || match context.addresses {
            Addresses::V4(..) => {
                !(20..=60).contains(&event.ip_header_len) || !event.ip_header_len.is_multiple_of(4)
            }
            Addresses::V6(..) => event.ip_header_len != 40,
        }
        || event.total_len != u32::from(event.ip_header_len) + 8 + u32::from(event.payload_len)
        || event.payload_len > 128
        || event.prefix_len != event.payload_len
        || event.prefix.len() != 128
        || event.sequence == 0
        || event.sequence > 128
        || event.cpu > u32::from(u16::MAX)
        || event.monotonic_ns < context.clock.0
        || event.monotonic_ns > context.end_ns
        || context.end_ns.saturating_sub(context.clock.0) > 30_000_000_000
    {
        return Err(ValidationError::new(
            "UDP datagram identity, framing, prefix or capture lifetime mismatch",
        ));
    }
    Ok(&event.prefix[..usize::from(event.prefix_len)])
}

pub(super) fn reconstruct(
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
    rules: &str,
) -> Result<ExportBundle, ValidationError> {
    for events in [ingress, egress] {
        if events.is_empty()
            || events.len() > 128
            || events
                .iter()
                .enumerate()
                .any(|(i, e)| e.sequence != i as u64 + 1)
        {
            return Err(ValidationError::new("UDP capture sequence is incomplete"));
        }
    }
    let binding =
        crate::dsl::compile_str(rules).map_err(|e| ValidationError::new(format!("{e:?}")))?;
    let mut config =
        SessionConfig::for_binding(binding).map_err(|e| ValidationError::new(e.to_string()))?;
    let mut registry = FragmentRegistry::new();
    for id in &config.template.fragment_set {
        let mut descriptor = config
            .registry
            .descriptor(id)
            .ok_or_else(|| ValidationError::new("missing UDP package fragment"))?
            .clone();
        if id == "udp_packet_meta_fragment" {
            descriptor.hookpoints = vec![HookPoint::CgroupSkbIngress, HookPoint::CgroupSkbEgress];
            descriptor.sampled_payload_offsets = (0..128).collect();
            descriptor
                .emits
                .retain(|kind| *kind == FactKindTag::PacketMeta);
            descriptor
                .evidence_classes
                .retain(|spec| spec.fact_kind == FactKindTag::PacketMeta);
            descriptor.maps = [
                "ingress_events",
                "egress_events",
                "ingress_counts",
                "egress_counts",
            ]
            .into_iter()
            .map(|name| MapSpec {
                name: name.into(),
                kind: MapKind::Hash,
                max_entries: if name.ends_with("events") { 128 } else { 1 },
            })
            .collect();
        } else {
            config
                .attach_failures
                .extend(descriptor.hookpoints.iter().map(|hookpoint| AttachFailure {
                    fragment_id: id.clone(),
                    hookpoint: hookpoint.clone(),
                    error: "not observed by bounded UDP validation capture".into(),
                }));
        }
        registry
            .register(descriptor)
            .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    }
    config.registry = registry;
    let mut facts = Vec::new();
    for (events, outbound) in [(ingress, false), (egress, true)] {
        for event in events {
            let bytes = validate(event, outbound, context)?;
            let ts = context
                .clock
                .1
                .checked_add(Duration::from_nanos(event.monotonic_ns - context.clock.0))
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .and_then(|d| u64::try_from(d.as_millis()).ok())
                .and_then(|ms| SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(ms)))
                .ok_or_else(|| ValidationError::new("UDP capture clock overflow"))?;
            facts.push((
                event.monotonic_ns,
                outbound,
                event.sequence,
                CapturedUdpDatagram {
                    source: std::net::SocketAddr::new(
                        if event.ip_version == 6 {
                            std::net::IpAddr::V6(event.source.into())
                        } else {
                            std::net::IpAddr::V4(
                                [
                                    event.source[0],
                                    event.source[1],
                                    event.source[2],
                                    event.source[3],
                                ]
                                .into(),
                            )
                        },
                        event.sport,
                    ),
                    destination: std::net::SocketAddr::new(
                        if event.ip_version == 6 {
                            std::net::IpAddr::V6(event.destination.into())
                        } else {
                            std::net::IpAddr::V4(
                                [
                                    event.destination[0],
                                    event.destination[1],
                                    event.destination[2],
                                    event.destination[3],
                                ]
                                .into(),
                            )
                        },
                        event.dport,
                    ),
                    ip_header_len: event.ip_header_len,
                    total_len: event.total_len,
                    payload_len: event.payload_len,
                    payload_prefix: bytes,
                }
                .into_fact(PacketObservation {
                    id: FactId(0),
                    ts,
                    cpu: event.cpu,
                    ifindex: event.ifindex,
                    session: SessionId(1),
                    netns: context.netns,
                    socket_cookie: event.cookie,
                    direction: if outbound {
                        PacketDir::Egress
                    } else {
                        PacketDir::Ingress
                    },
                })
                .map_err(|error| ValidationError::new(error.to_string()))?,
            ));
        }
    }
    facts.sort_by_key(|(ns, outbound, sequence, _)| (*ns, !outbound, *sequence));
    let mut session =
        RuntimeSession::start(config).map_err(|e| ValidationError::new(e.to_string()))?;
    for (i, (_, _, _, mut fact)) in facts.into_iter().enumerate() {
        fact.id = FactId(i as u64 + 1);
        session.ingest(fact);
    }
    let mut export = session.into_export_bundle();
    export.ingest_trust_mode = "unverified-local".into();
    if !export.rejected_facts.is_empty()
        || export.facts.len() != ingress.len() + egress.len()
        || export.flows.len() != 2
        || export.flows.iter().any(|f| f.process.is_some())
    {
        return Err(ValidationError::new(
            "UDP facts rejected, misjoined or attributed without evidence",
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
        || replay.program_findings != export.program_findings
        || decoded.attach_plan != export.attach_plan
    {
        return Err(ValidationError::new("UDP export/replay diverged"));
    }
    Ok(export)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Datagram, Context) {
        (
            Datagram {
                sequence: 1,
                monotonic_ns: 2,
                cookie: 1,
                cpu: 0,
                ifindex: 1,
                source: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                destination: [127, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                sport: 4000,
                dport: 53,
                total_len: 40,
                payload_len: 12,
                prefix_len: 12,
                ip_header_len: 20,
                ip_version: 4,
                prefix: vec![0; 128],
            },
            Context {
                remote_port: 53,
                sockets: [(1, 4000), (2, 4001)],
                addresses: ([127, 0, 0, 1], [127, 0, 0, 2]).into(),
                netns: 1,
                clock: (1, SystemTime::UNIX_EPOCH),
                end_ns: 3,
            },
        )
    }
    #[test]
    fn ipv6_datagrams_validate_all_address_bytes_and_family_framing() {
        let (mut event, mut context) = fixture();
        let source = "fd01:2345:6789::1"
            .parse::<std::net::Ipv6Addr>()
            .unwrap()
            .octets();
        let destination = "fd01:2345:6789::2"
            .parse::<std::net::Ipv6Addr>()
            .unwrap()
            .octets();
        context.addresses = Addresses::V6(source, destination);
        event.source = source;
        event.destination = destination;
        event.ip_version = 6;
        event.ip_header_len = 40;
        event.total_len = 60;
        assert_eq!(validate(&event, true, &context).unwrap().len(), 12);
        assert!(validate(&event, false, &context).is_err());
        for offset in 0..16 {
            for source in [true, false] {
                let mut bad = event.clone();
                if source {
                    bad.source[offset] ^= 1;
                } else {
                    bad.destination[offset] ^= 1;
                }
                assert!(validate(&bad, true, &context).is_err());
            }
        }
        for kind in 0..8 {
            let mut bad = event.clone();
            match kind {
                0 => bad.ip_version = 4,
                1 => bad.ip_version = 0,
                2 => bad.ip_header_len = 20,
                3 => bad.ip_header_len = 48,
                4 => bad.total_len = 20,
                5 => bad.payload_len = 129,
                6 => bad.prefix_len -= 1,
                _ => bad.cookie = 3,
            }
            assert!(validate(&bad, true, &context).is_err());
        }
        let encoded = serde_json::to_value(&event).unwrap();
        for field in ["ip_version", "source"] {
            let mut missing = encoded.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<Datagram>(missing).is_err());
        }
        let mut legacy = encoded;
        legacy["source"] = serde_json::json!(0x7f000001);
        assert!(serde_json::from_value::<Datagram>(legacy).is_err());
    }

    #[test]
    fn ipv6_replay_preserves_ethertype_and_complete_ip_packet_length() {
        let (mut first, mut context) = fixture();
        context.addresses = Addresses::V6(
            "fd01::1".parse::<std::net::Ipv6Addr>().unwrap().octets(),
            "fd01::2".parse::<std::net::Ipv6Addr>().unwrap().octets(),
        );
        (first.source, first.destination) = context.addresses.octets();
        first.ip_version = 6;
        first.ip_header_len = 40;
        first.total_len = 60;
        let mut second = first.clone();
        second.cookie = 2;
        second.sport = 4001;
        second.sequence = 2;
        let egress = [first, second];
        let ingress: Vec<_> = egress
            .iter()
            .map(|e| {
                let mut reply = e.clone();
                std::mem::swap(&mut reply.source, &mut reply.destination);
                std::mem::swap(&mut reply.sport, &mut reply.dport);
                reply
            })
            .collect();
        let export = reconstruct(
            &ingress,
            &egress,
            &context,
            include_str!("../../../protocols/dns/udp/main.gewy"),
        )
        .unwrap();
        assert_eq!(export.facts.len(), 4);
        assert_eq!(export.flows.len(), 2);
        assert!(
            export
                .facts
                .iter()
                .all(|f| matches!(&f.kind, FactKind::PacketMeta(p)
            if p.l3_proto == 0x86dd && p.tot_len == 60 && p.l4_proto == 17))
        );
        let mut wrong_context = context;
        wrong_context.addresses = ([127, 0, 0, 1], [127, 0, 0, 2]).into();
        assert!(
            reconstruct(
                &ingress,
                &egress,
                &wrong_context,
                include_str!("../../../protocols/dns/udp/main.gewy")
            )
            .is_err()
        );
    }

    #[test]
    fn datagrams_reject_foreign_identity_wrong_direction_truncation_and_bad_lifetime() {
        let (event, context) = fixture();
        assert_eq!(validate(&event, true, &context).unwrap().len(), 12);
        assert!(validate(&event, false, &context).is_err());
        for change in 0..7 {
            let mut bad = event.clone();
            match change {
                0 => bad.cookie = 3,
                1 => bad.sport = 4001,
                2 => bad.monotonic_ns = 4,
                3 => bad.total_len = 39,
                4 => bad.prefix_len = 11,
                5 => {
                    bad.prefix.pop();
                }
                _ => bad.ip_header_len = 21,
            }
            assert!(validate(&bad, true, &context).is_err());
        }
    }
    #[test]
    fn udp_replay_uses_datagrams_without_inventing_tcp_process_or_route_facts() {
        let (first, context) = fixture();
        let mut second = first.clone();
        second.cookie = 2;
        second.sport = 4001;
        second.sequence = 2;
        let egress = [first, second];
        let ingress: Vec<_> = egress
            .iter()
            .map(|e| {
                let mut r = e.clone();
                std::mem::swap(&mut r.source, &mut r.destination);
                std::mem::swap(&mut r.sport, &mut r.dport);
                r
            })
            .collect();
        let export = reconstruct(
            &ingress,
            &egress,
            &context,
            include_str!("../../../protocols/dns/udp/main.gewy"),
        )
        .unwrap();
        assert_eq!(export.facts.len(), 4);
        assert!(export.facts.iter().all(
            |f| matches!(&f.kind, FactKind::PacketMeta(p) if p.l4_proto==17 && p.seq.is_none())
        ));
        assert_eq!(
            export.attach_report.fragments_loaded,
            vec!["udp_packet_meta_fragment"]
        );
        assert!(
            export
                .attach_report
                .hookpoints_attached
                .iter()
                .any(|h| h.ends_with("cgroup_skb:egress"))
        );
    }
}
