use super::{AttachFailureSummaryItem, DebugSummary, ExportError, JsonValue};
use crate::fragment::{
    AttachPlan, AttachReport, BindingDiagnostics, CapabilityFlag, CoverageReport, DependencyEdge,
    EvidenceClassSpec, EvidenceTier, FactBinding, FragmentDescriptor, FragmentParamSpec,
    FragmentParamType, HookBinding, HookPoint, MapKind, MapSpec, ModelDiagnostics, RingBufStats,
    RuleDiagnostics, RuleTier,
};
use crate::ledger::FactKindTag;

mod decode;
mod encode;

pub(super) use self::decode::{
    parse_attach_plan, parse_attach_report, parse_binding_diagnostics, parse_debug_summary,
};
pub(super) use self::encode::{
    attach_failure_summary_json, attach_plan_json, attach_report_json, binding_diagnostics_json,
    debug_summary_json,
};

fn parse_fact_kind_list(value: &JsonValue) -> Result<Vec<FactKindTag>, ExportError> {
    value
        .as_array()?
        .iter()
        .map(|item| {
            FactKindTag::from_str(item.as_str()?)
                .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))
        })
        .collect()
}

fn parse_hookpoint(input: &str) -> Result<HookPoint, ExportError> {
    if let Some(value) = input.strip_prefix("tracepoint:") {
        return Ok(HookPoint::TracePoint(value.to_string()));
    }
    if let Some(value) = input.strip_prefix("kprobe:") {
        return Ok(HookPoint::KProbe(value.to_string()));
    }
    match input {
        "tc:ingress" => Ok(HookPoint::TCIngress),
        "tc:egress" => Ok(HookPoint::TCEgress),
        "cgroup_skb:ingress" => Ok(HookPoint::CgroupSkbIngress),
        "cgroup_skb:egress" => Ok(HookPoint::CgroupSkbEgress),
        _ => Err(ExportError::InvalidValue("unknown hookpoint".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::export::json::JsonParser;
    use crate::fragment::{FragmentRegistry, builtin_registry};

    #[test]
    fn hookpoint_labels_roundtrip_including_cgroup_egress() {
        for (hookpoint, label) in [
            (
                HookPoint::TracePoint("sock/inet_sock_set_state".into()),
                "tracepoint:sock/inet_sock_set_state",
            ),
            (
                HookPoint::KProbe("ip_route_output_flow".into()),
                "kprobe:ip_route_output_flow",
            ),
            (HookPoint::TCIngress, "tc:ingress"),
            (HookPoint::TCEgress, "tc:egress"),
            (HookPoint::CgroupSkbIngress, "cgroup_skb:ingress"),
            (HookPoint::CgroupSkbEgress, "cgroup_skb:egress"),
        ] {
            assert_eq!(hookpoint.label(), label);
            assert_eq!(parse_hookpoint(label).unwrap(), hookpoint);
        }
    }

    #[test]
    fn cgroup_egress_attach_plan_roundtrips_descriptors_and_hook_graph() {
        let mut descriptor = builtin_registry()
            .descriptor("udp_packet_meta_fragment")
            .unwrap()
            .clone();
        descriptor.hookpoints = vec![HookPoint::CgroupSkbIngress, HookPoint::CgroupSkbEgress];
        let mut registry = FragmentRegistry::new();
        registry.register(descriptor).unwrap();
        let plan = registry.plan(["udp_packet_meta_fragment"]).unwrap();

        let json = attach_plan_json(&plan).render();
        let decoded = parse_attach_plan(&JsonParser::new(&json).parse().unwrap()).unwrap();

        assert_eq!(plan.hook_graph.len(), 2);
        assert_eq!(decoded, plan);
    }

    #[test]
    fn cgroup_hookpoint_decode_rejects_unknown_direction() {
        assert_eq!(
            parse_hookpoint("cgroup_skb:unknown"),
            Err(ExportError::InvalidValue("unknown hookpoint".into()))
        );
    }
}
