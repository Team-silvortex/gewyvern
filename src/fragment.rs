use crate::ledger::FactKindTag;
use crate::template::{FragmentParamValue, TemplateBinding};
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
    pub params: Vec<FragmentParamSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentParamSpec {
    pub key: &'static str,
    pub value_type: FragmentParamType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FragmentParamType {
    Bool,
    U64,
    String,
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
    SockLineage,
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
pub struct AttachFailure {
    pub fragment_id: &'static str,
    pub hookpoint: HookPoint,
    pub error: String,
}

impl AttachFailure {
    pub fn label(&self) -> String {
        format!("{}@{}", self.fragment_id, self.hookpoint.label())
    }
}

pub fn summarize_attach_failures(
    report: &AttachReport,
) -> Vec<crate::export::AttachFailureSummaryItem> {
    let mut counts = BTreeMap::<&'static str, u64>::new();

    for label in &report.hookpoints_failed {
        let hookpoint_kind = match label
            .split_once('@')
            .and_then(|(_, hookpoint)| hookpoint.split_once(':'))
        {
            Some(("tc", "ingress")) => "tc_ingress",
            Some(("tc", "egress")) => "tc_egress",
            Some(("tracepoint", _)) => "tracepoint",
            Some(("kprobe", _)) => "kprobe",
            _ => "unknown",
        };
        *counts.entry(hookpoint_kind).or_default() += 1;
    }

    counts
        .into_iter()
        .map(|(hookpoint_kind, count)| crate::export::AttachFailureSummaryItem {
            hookpoint_kind: hookpoint_kind.into(),
            count,
        })
        .collect()
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
    UnknownFragmentParam { fragment_id: String, key: String },
    InvalidFragmentParamType {
        fragment_id: String,
        key: String,
        expected: &'static str,
    },
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

    pub fn validate_binding_params(&self, binding: &TemplateBinding) -> Result<(), RegistryError> {
        for (fragment_id, params) in &binding.fragment_params {
            let descriptor = self
                .descriptor(fragment_id)
                .ok_or_else(|| RegistryError::MissingFragment(fragment_id.clone()))?;
            for (key, value) in params {
                let spec = descriptor
                    .params
                    .iter()
                    .find(|spec| spec.key == key)
                    .ok_or_else(|| RegistryError::UnknownFragmentParam {
                        fragment_id: fragment_id.clone(),
                        key: key.clone(),
                    })?;
                let type_matches = matches!(
                    (&spec.value_type, value),
                    (FragmentParamType::Bool, FragmentParamValue::Bool(_))
                        | (FragmentParamType::U64, FragmentParamValue::U64(_))
                        | (FragmentParamType::String, FragmentParamValue::String(_))
                );
                if !type_matches {
                    return Err(RegistryError::InvalidFragmentParamType {
                        fragment_id: fragment_id.clone(),
                        key: key.clone(),
                        expected: spec.value_type.label(),
                    });
                }
            }
        }
        Ok(())
    }

    pub fn attach_report(&self, plan: &AttachPlan) -> AttachReport {
        self.attach_report_with_failures(plan, std::iter::empty::<String>())
    }

    pub fn attach_report_with_failure_records<I>(
        &self,
        plan: &AttachPlan,
        failures: I,
    ) -> AttachReport
    where
        I: IntoIterator<Item = AttachFailure>,
    {
        self.attach_report_with_failures(plan, failures.into_iter().map(|failure| failure.label()))
    }

    pub fn attach_report_with_failures<I>(
        &self,
        plan: &AttachPlan,
        failed_hookpoints: I,
    ) -> AttachReport
    where
        I: IntoIterator<Item = String>,
    {
        let failed: BTreeSet<_> = failed_hookpoints.into_iter().collect();
        let mut attached = Vec::new();
        let mut failed_report = Vec::new();
        let mut loaded_fragments = BTreeSet::new();
        let mut matched_failures = BTreeSet::new();

        for binding in &plan.hook_graph {
            let label = format!("{}@{}", binding.fragment_id, binding.hookpoint.label());
            if failed.contains(&label) {
                matched_failures.insert(label.clone());
                failed_report.push(label);
            } else {
                loaded_fragments.insert(binding.fragment_id);
                attached.push(label);
            }
        }

        for failure in failed.difference(&matched_failures) {
            failed_report.push(failure.clone());
        }

        let maps = plan
            .fragments
            .iter()
            .filter(|fragment| loaded_fragments.contains(fragment.id))
            .flat_map(|fragment| fragment.maps.iter())
            .filter(|spec| spec.kind == MapKind::RingBuf)
            .collect::<Vec<_>>();

        AttachReport {
            fragments_loaded: plan
                .fragments
                .iter()
                .filter(|fragment| loaded_fragments.contains(fragment.id))
                .map(|fragment| fragment.id.into())
                .collect(),
            hookpoints_attached: attached,
            hookpoints_failed: failed_report,
            required_fact_kinds_coverage: plan.coverage.clone(),
            ringbuf_stats: RingBufStats {
                maps: maps.len(),
                total_max_entries: maps.iter().map(|map| map.max_entries).sum(),
            },
        }
    }
}

impl FragmentParamType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U64 => "u64",
            Self::String => "string",
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
            params: vec![],
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
            params: vec![],
        },
        FragmentDescriptor {
            id: "udp_packet_meta_fragment",
            version: 1,
            hookpoints: vec![HookPoint::TCIngress],
            emits: vec![FactKindTag::PacketMeta],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::PacketMeta],
            params: vec![FragmentParamSpec {
                key: "min_len",
                value_type: FragmentParamType::U64,
            }],
        },
        FragmentDescriptor {
            id: "route_meta_fragment",
            version: 1,
            hookpoints: vec![HookPoint::KProbe("ip_route_output_flow")],
            emits: vec![FactKindTag::RouteDecision],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::RouteMeta],
            params: vec![],
        },
        FragmentDescriptor {
            id: "sock_lineage_fragment",
            version: 1,
            hookpoints: vec![HookPoint::TracePoint("syscalls/sys_enter_connect")],
            emits: vec![FactKindTag::SockLineage],
            requires: vec![],
            maps: vec![MapSpec {
                name: "events",
                kind: MapKind::RingBuf,
                max_entries: 4096,
            }],
            capabilities: vec![CapabilityFlag::SockLineage],
            params: vec![FragmentParamSpec {
                key: "capture_comm",
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
