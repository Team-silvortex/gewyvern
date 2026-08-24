use super::{
    CapabilityFlag, EvidenceClassSpec, EvidenceTier, FactKindTag, FragmentDescriptor,
    FragmentParamSpec, FragmentParamType, FragmentRegistry, HookPoint, MapKind, MapSpec,
};
use std::sync::OnceLock;

pub fn builtin_registry() -> FragmentRegistry {
    builtin_registry_ref().clone()
}

pub(crate) fn builtin_registry_ref() -> &'static FragmentRegistry {
    static REGISTRY: OnceLock<FragmentRegistry> = OnceLock::new();
    REGISTRY.get_or_init(build_builtin_registry)
}

fn build_builtin_registry() -> FragmentRegistry {
    let mut registry = FragmentRegistry::new();
    let fragments = [
        FragmentDescriptor {
            id: "tcp_state_fragment".into(),
            version: 1,
            hookpoints: vec![HookPoint::TracePoint("sock/inet_sock_set_state".into())],
            emits: vec![FactKindTag::TcpState],
            evidence_classes: vec![EvidenceClassSpec {
                fact_kind: FactKindTag::TcpState,
                tier: EvidenceTier::CoreRequirement,
            }],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::TcpState],
            sampled_payload_offsets: vec![],
            params: vec![],
        },
        FragmentDescriptor {
            id: "tcp_packet_meta_fragment".into(),
            version: 1,
            hookpoints: vec![HookPoint::TCIngress],
            emits: vec![FactKindTag::PacketMeta, FactKindTag::QuicMeta],
            evidence_classes: vec![
                EvidenceClassSpec {
                    fact_kind: FactKindTag::PacketMeta,
                    tier: EvidenceTier::CoreRequirement,
                },
                EvidenceClassSpec {
                    fact_kind: FactKindTag::QuicMeta,
                    tier: EvidenceTier::OptionalEnhancement,
                },
            ],
            requires: vec![FactKindTag::TcpState],
            maps: vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::PacketMeta],
            sampled_payload_offsets: vec![0, 1, 4, 5, 9, 10, 13],
            params: vec![FragmentParamSpec {
                key: "sample_payload_offsets".into(),
                value_type: FragmentParamType::String,
            }],
        },
        FragmentDescriptor {
            id: "udp_packet_meta_fragment".into(),
            version: 1,
            hookpoints: vec![HookPoint::TCIngress],
            emits: vec![FactKindTag::PacketMeta, FactKindTag::QuicMeta],
            evidence_classes: vec![
                EvidenceClassSpec {
                    fact_kind: FactKindTag::PacketMeta,
                    tier: EvidenceTier::CoreRequirement,
                },
                EvidenceClassSpec {
                    fact_kind: FactKindTag::QuicMeta,
                    tier: EvidenceTier::OptionalEnhancement,
                },
            ],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::PacketMeta],
            sampled_payload_offsets: vec![0, 1, 4, 5, 9, 10, 13],
            params: vec![
                FragmentParamSpec {
                    key: "min_len".into(),
                    value_type: FragmentParamType::U64,
                },
                FragmentParamSpec {
                    key: "sample_payload_offsets".into(),
                    value_type: FragmentParamType::String,
                },
            ],
        },
        FragmentDescriptor {
            id: "route_meta_fragment".into(),
            version: 1,
            hookpoints: vec![HookPoint::KProbe("ip_route_output_flow".into())],
            emits: vec![FactKindTag::RouteDecision],
            evidence_classes: vec![EvidenceClassSpec {
                fact_kind: FactKindTag::RouteDecision,
                tier: EvidenceTier::CoreRequirement,
            }],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::RouteMeta],
            sampled_payload_offsets: vec![],
            params: vec![],
        },
        FragmentDescriptor {
            id: "sock_lineage_fragment".into(),
            version: 1,
            hookpoints: vec![HookPoint::TracePoint("syscalls/sys_enter_connect".into())],
            emits: vec![FactKindTag::SockLineage],
            evidence_classes: vec![EvidenceClassSpec {
                fact_kind: FactKindTag::SockLineage,
                tier: EvidenceTier::OptionalEnhancement,
            }],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::SockLineage],
            sampled_payload_offsets: vec![],
            params: vec![FragmentParamSpec {
                key: "capture_comm".into(),
                value_type: FragmentParamType::Bool,
            }],
        },
    ];

    for fragment in fragments {
        registry
            .register(fragment)
            .expect("builtin registry must stay valid");
    }
    registry
}
