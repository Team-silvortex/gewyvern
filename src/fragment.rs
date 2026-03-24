use crate::ledger::FactKindTag;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentDescriptor {
    pub id: &'static str,
    pub version: u32,
    pub hookpoints: Vec<HookPoint>,
    pub emits: Vec<FactKindTag>,
    pub requires: Vec<FactKindTag>,
    pub maps: Vec<MapSpec>,
    pub capabilities: Vec<CapabilityFlag>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HookPoint {
    TracePoint(&'static str),
    KProbe(&'static str),
    TCIngress,
    TCEgress,
}

impl HookPoint {
    pub fn label(&self) -> String {
        match self {
            Self::TracePoint(name) => format!("tracepoint:{name}"),
            Self::KProbe(name) => format!("kprobe:{name}"),
            Self::TCIngress => "tc:ingress".into(),
            Self::TCEgress => "tc:egress".into(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MapSpec {
    pub name: &'static str,
    pub kind: MapKind,
    pub max_entries: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapKind {
    RingBuf,
    Hash,
    LruHash,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum CapabilityFlag {
    TcpState,
    PacketMeta,
    RouteMeta,
}

#[derive(Clone, Debug, Default)]
pub struct FragmentRegistry {
    descriptors: BTreeMap<&'static str, FragmentDescriptor>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachPlan {
    pub fragments: Vec<FragmentDescriptor>,
    pub hook_graph: Vec<HookBinding>,
    pub fact_graph: Vec<FactBinding>,
    pub dependency_graph: Vec<DependencyEdge>,
    pub coverage: CoverageReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HookBinding {
    pub fragment_id: &'static str,
    pub hookpoint: HookPoint,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FactBinding {
    pub fragment_id: &'static str,
    pub emits: Vec<FactKindTag>,
    pub requires: Vec<FactKindTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyEdge {
    pub fragment_id: &'static str,
    pub depends_on: &'static str,
    pub fact_kind: FactKindTag,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CoverageReport {
    pub required: Vec<FactKindTag>,
    pub covered: Vec<FactKindTag>,
    pub missing: Vec<FactKindTag>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachReport {
    pub fragments_loaded: Vec<String>,
    pub hookpoints_attached: Vec<String>,
    pub hookpoints_failed: Vec<String>,
    pub required_fact_kinds_coverage: CoverageReport,
    pub ringbuf_stats: RingBufStats,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RingBufStats {
    pub maps: usize,
    pub total_max_entries: u32,
}

#[derive(Debug, Eq, PartialEq)]
pub enum RegistryError {
    DuplicateFragmentId(String),
    MissingFragment(String),
    HookConflict(String),
    FactConflict(String),
    MissingCoverage(Vec<FactKindTag>),
}

impl FragmentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, descriptor: FragmentDescriptor) -> Result<(), RegistryError> {
        if self.descriptors.contains_key(descriptor.id) {
            return Err(RegistryError::DuplicateFragmentId(descriptor.id.into()));
        }
        self.descriptors.insert(descriptor.id, descriptor);
        Ok(())
    }

    pub fn descriptor(&self, id: &str) -> Option<&FragmentDescriptor> {
        self.descriptors.get(id)
    }

    pub fn plan<'a, I>(&self, fragment_ids: I) -> Result<AttachPlan, RegistryError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut fragments = Vec::new();
        for id in fragment_ids {
            let descriptor = self
                .descriptor(id)
                .cloned()
                .ok_or_else(|| RegistryError::MissingFragment(id.into()))?;
            fragments.push(descriptor);
        }

        let mut hook_owners: BTreeMap<HookPoint, &'static str> = BTreeMap::new();
        let mut fact_owners: BTreeMap<FactKindTag, &'static str> = BTreeMap::new();
        let mut hook_graph = Vec::new();
        let mut fact_graph = Vec::new();

        for fragment in &fragments {
            for hookpoint in &fragment.hookpoints {
                if let Some(existing) = hook_owners.insert(hookpoint.clone(), fragment.id) {
                    return Err(RegistryError::HookConflict(format!(
                        "{} already owned by {}",
                        hookpoint.label(),
                        existing
                    )));
                }
                hook_graph.push(HookBinding {
                    fragment_id: fragment.id,
                    hookpoint: hookpoint.clone(),
                });
            }

            for emitted in &fragment.emits {
                if let Some(existing) = fact_owners.insert(*emitted, fragment.id) {
                    return Err(RegistryError::FactConflict(format!(
                        "{} emitted by both {} and {}",
                        emitted, existing, fragment.id
                    )));
                }
            }

            fact_graph.push(FactBinding {
                fragment_id: fragment.id,
                emits: fragment.emits.clone(),
                requires: fragment.requires.clone(),
            });
        }

        let required: BTreeSet<_> = fragments
            .iter()
            .flat_map(|fragment| fragment.requires.iter().copied())
            .collect();
        let covered: BTreeSet<_> = fragments
            .iter()
            .flat_map(|fragment| fragment.emits.iter().copied())
            .collect();
        let missing: Vec<_> = required.difference(&covered).copied().collect();
        if !missing.is_empty() {
            return Err(RegistryError::MissingCoverage(missing));
        }

        let mut dependency_graph = Vec::new();
        for fragment in &fragments {
            for requirement in &fragment.requires {
                if let Some((producer, _)) = fragments
                    .iter()
                    .find_map(|candidate| candidate.emits.contains(requirement).then_some((candidate.id, requirement)))
                {
                    dependency_graph.push(DependencyEdge {
                        fragment_id: fragment.id,
                        depends_on: producer,
                        fact_kind: *requirement,
                    });
                }
            }
        }

        Ok(AttachPlan {
            fragments,
            hook_graph,
            fact_graph,
            dependency_graph,
            coverage: CoverageReport {
                required: required.iter().copied().collect(),
                covered: covered.iter().copied().collect(),
                missing,
            },
        })
    }

    pub fn attach_report(&self, plan: &AttachPlan) -> AttachReport {
        let maps = plan
            .fragments
            .iter()
            .flat_map(|fragment| fragment.maps.iter())
            .filter(|spec| spec.kind == MapKind::RingBuf)
            .collect::<Vec<_>>();

        AttachReport {
            fragments_loaded: plan.fragments.iter().map(|fragment| fragment.id.into()).collect(),
            hookpoints_attached: plan
                .hook_graph
                .iter()
                .map(|binding| format!("{}@{}", binding.fragment_id, binding.hookpoint.label()))
                .collect(),
            hookpoints_failed: Vec::new(),
            required_fact_kinds_coverage: plan.coverage.clone(),
            ringbuf_stats: RingBufStats {
                maps: maps.len(),
                total_max_entries: maps.iter().map(|map| map.max_entries).sum(),
            },
        }
    }
}

pub fn builtin_registry() -> FragmentRegistry {
    let mut registry = FragmentRegistry::new();
    let fragments = [
        FragmentDescriptor {
            id: "tcp_state_fragment",
            version: 1,
            hookpoints: vec![HookPoint::TracePoint("sock/inet_sock_set_state")],
            emits: vec![FactKindTag::TcpState],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::TcpState],
        },
        FragmentDescriptor {
            id: "tcp_packet_meta_fragment",
            version: 1,
            hookpoints: vec![HookPoint::TCIngress],
            emits: vec![FactKindTag::PacketMeta],
            requires: vec![FactKindTag::TcpState],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::PacketMeta],
        },
        FragmentDescriptor {
            id: "route_meta_fragment",
            version: 1,
            hookpoints: vec![HookPoint::KProbe("ip_route_output_flow")],
            emits: vec![FactKindTag::RouteDecision],
            requires: vec![FactKindTag::TcpState],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::RouteMeta],
        },
    ];

    for fragment in fragments {
        registry
            .register(fragment)
            .expect("builtin registry must stay valid");
    }
    registry
}

#[cfg(test)]
mod tests {
    use super::{
        builtin_registry, CapabilityFlag, FragmentDescriptor, FragmentRegistry, HookPoint, MapKind,
        MapSpec, RegistryError,
    };
    use crate::ledger::FactKindTag;

    #[test]
    fn builtin_handshake_plan_has_full_coverage() {
        let registry = builtin_registry();
        let plan = registry
            .plan([
                "tcp_state_fragment",
                "tcp_packet_meta_fragment",
                "route_meta_fragment",
            ])
            .unwrap();

        assert!(plan.coverage.missing.is_empty());
        assert_eq!(plan.fragments.len(), 3);
        assert_eq!(plan.hook_graph.len(), 3);
    }

    #[test]
    fn registry_rejects_hookpoint_conflicts() {
        let mut registry = FragmentRegistry::new();
        let first = test_fragment("a", HookPoint::TCIngress, FactKindTag::TcpState, vec![]);
        let second = test_fragment("b", HookPoint::TCIngress, FactKindTag::PacketMeta, vec![]);
        registry.register(first).unwrap();
        registry.register(second).unwrap();

        let err = registry.plan(["a", "b"]).unwrap_err();
        assert!(matches!(err, RegistryError::HookConflict(_)));
    }

    #[test]
    fn registry_rejects_missing_required_fact_coverage() {
        let mut registry = FragmentRegistry::new();
        let fragment = test_fragment(
            "needs_route",
            HookPoint::TCEgress,
            FactKindTag::PacketMeta,
            vec![FactKindTag::RouteDecision],
        );
        registry.register(fragment).unwrap();

        let err = registry.plan(["needs_route"]).unwrap_err();
        assert_eq!(
            err,
            RegistryError::MissingCoverage(vec![FactKindTag::RouteDecision])
        );
    }

    fn test_fragment(
        id: &'static str,
        hookpoint: HookPoint,
        emits: FactKindTag,
        requires: Vec<FactKindTag>,
    ) -> FragmentDescriptor {
        FragmentDescriptor {
            id,
            version: 1,
            hookpoints: vec![hookpoint],
            emits: vec![emits],
            requires,
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 1024,
            }],
            capabilities: vec![CapabilityFlag::TcpState],
        }
    }
}
