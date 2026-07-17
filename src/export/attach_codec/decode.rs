use super::{
    AttachPlan, AttachReport, BindingDiagnostics, CapabilityFlag, CoverageReport, DebugSummary,
    DependencyEdge, EvidenceClassSpec, EvidenceTier, ExportError, FactBinding, FactKindTag,
    FragmentDescriptor, FragmentParamSpec, FragmentParamType, HookBinding, HookPoint, JsonValue,
    MapKind, MapSpec, ModelDiagnostics, RingBufStats, RuleDiagnostics, RuleTier,
    parse_fact_kind_list, parse_hookpoint,
};

pub(crate) fn parse_attach_plan(value: &JsonValue) -> Result<AttachPlan, ExportError> {
    let object = value.as_object()?;
    Ok(AttachPlan {
        fragments: object
            .get("fragments")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.fragments".into()))?
            .as_array()?
            .iter()
            .map(parse_fragment_descriptor)
            .collect::<Result<Vec<_>, _>>()?,
        hook_graph: object
            .get("hook_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.hook_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_hook_binding)
            .collect::<Result<Vec<_>, _>>()?,
        fact_graph: object
            .get("fact_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.fact_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_fact_binding)
            .collect::<Result<Vec<_>, _>>()?,
        dependency_graph: object
            .get("dependency_graph")
            .ok_or_else(|| ExportError::InvalidShape("attach_plan.dependency_graph".into()))?
            .as_array()?
            .iter()
            .map(parse_dependency_edge)
            .collect::<Result<Vec<_>, _>>()?,
        coverage: parse_coverage(
            object
                .get("coverage")
                .ok_or_else(|| ExportError::InvalidShape("attach_plan.coverage".into()))?,
        )?,
    })
}

pub(crate) fn parse_attach_report(value: &JsonValue) -> Result<AttachReport, ExportError> {
    let object = value.as_object()?;
    Ok(AttachReport {
        fragments_loaded: object
            .get("fragments_loaded")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.fragments_loaded".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        hookpoints_attached: object
            .get("hookpoints_attached")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.hookpoints_attached".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        hookpoints_failed: object
            .get("hookpoints_failed")
            .ok_or_else(|| ExportError::InvalidShape("attach_report.hookpoints_failed".into()))?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        required_fact_kinds_coverage: parse_coverage(
            object.get("required_fact_kinds_coverage").ok_or_else(|| {
                ExportError::InvalidShape("attach_report.required_fact_kinds_coverage".into())
            })?,
        )?,
        ringbuf_stats: {
            let stats = object
                .get("ringbuf_stats")
                .ok_or_else(|| ExportError::InvalidShape("attach_report.ringbuf_stats".into()))?
                .as_object()?;
            RingBufStats {
                maps: stats
                    .get("maps")
                    .ok_or_else(|| {
                        ExportError::InvalidShape("attach_report.ringbuf_stats.maps".into())
                    })?
                    .as_i64()? as usize,
                total_max_entries: stats
                    .get("total_max_entries")
                    .ok_or_else(|| {
                        ExportError::InvalidShape(
                            "attach_report.ringbuf_stats.total_max_entries".into(),
                        )
                    })?
                    .as_i64()? as u32,
            }
        },
    })
}

pub(crate) fn parse_binding_diagnostics(
    value: &JsonValue,
) -> Result<BindingDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(BindingDiagnostics {
        program_model: match object.get("program_model").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_model_diagnostics(value)?),
        },
        reason_model: match object.get("reason_model").unwrap_or(&JsonValue::Null) {
            JsonValue::Null => None,
            value => Some(parse_model_diagnostics(value)?),
        },
    })
}

pub(crate) fn parse_debug_summary(value: &JsonValue) -> Result<DebugSummary, ExportError> {
    let object = value.as_object()?;
    Ok(DebugSummary {
        fragments_loaded: object
            .get("fragments_loaded")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.fragments_loaded".into()))?
            .as_i64()? as u64,
        hookpoints_failed: object
            .get("hookpoints_failed")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.hookpoints_failed".into()))?
            .as_i64()? as u64,
        accepted_facts: object
            .get("accepted_facts")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.accepted_facts".into()))?
            .as_i64()? as u64,
        rejected_facts: object
            .get("rejected_facts")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.rejected_facts".into()))?
            .as_i64()? as u64,
        flows: object
            .get("flows")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.flows".into()))?
            .as_i64()? as u64,
        program_flows: object
            .get("program_flows")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.program_flows".into()))?
            .as_i64()? as u64,
        program_findings: object
            .get("program_findings")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.program_findings".into()))?
            .as_i64()? as u64,
        module_findings: object
            .get("module_findings")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.module_findings".into()))?
            .as_i64()? as u64,
        reasons: object
            .get("reasons")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.reasons".into()))?
            .as_i64()? as u64,
        degraded: object
            .get("degraded")
            .ok_or_else(|| ExportError::InvalidShape("debug_summary.degraded".into()))?
            .as_bool()?,
    })
}

fn parse_model_diagnostics(value: &JsonValue) -> Result<ModelDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(ModelDiagnostics {
        model: object
            .get("model")
            .ok_or_else(|| ExportError::InvalidShape("model_diagnostics.model".into()))?
            .as_str()?
            .to_string(),
        rules: object
            .get("rules")
            .ok_or_else(|| ExportError::InvalidShape("model_diagnostics.rules".into()))?
            .as_array()?
            .iter()
            .map(parse_rule_diagnostics)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_rule_diagnostics(value: &JsonValue) -> Result<RuleDiagnostics, ExportError> {
    let object = value.as_object()?;
    Ok(RuleDiagnostics {
        rule_index: object
            .get("rule_index")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.rule_index".into()))?
            .as_i64()? as usize,
        tier: match object
            .get("tier")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.tier".into()))?
            .as_str()?
        {
            "core_requirement" => RuleTier::CoreRequirement,
            "optional_enhancement" => RuleTier::OptionalEnhancement,
            "unsupported" => RuleTier::Unsupported,
            _ => {
                return Err(ExportError::InvalidValue(
                    "unknown rule diagnostics tier".into(),
                ));
            }
        },
        required_facts: parse_fact_kind_list(
            object.get("required_facts").ok_or_else(|| {
                ExportError::InvalidShape("rule_diagnostics.required_facts".into())
            })?,
        )?,
        supporting_fragments: object
            .get("supporting_fragments")
            .ok_or_else(|| {
                ExportError::InvalidShape("rule_diagnostics.supporting_fragments".into())
            })?
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_str()?.to_string()))
            .collect::<Result<Vec<_>, _>>()?,
        missing_facts: parse_fact_kind_list(
            object.get("missing_facts").ok_or_else(|| {
                ExportError::InvalidShape("rule_diagnostics.missing_facts".into())
            })?,
        )?,
        unsupported_payload_offsets: object
            .get("unsupported_payload_offsets")
            .unwrap_or(&JsonValue::Array(vec![]))
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_i64()? as u16))
            .collect::<Result<Vec<_>, _>>()?,
        supported: object
            .get("supported")
            .ok_or_else(|| ExportError::InvalidShape("rule_diagnostics.supported".into()))?
            .as_bool()?,
    })
}

fn parse_coverage(value: &JsonValue) -> Result<CoverageReport, ExportError> {
    let object = value.as_object()?;
    Ok(CoverageReport {
        required: parse_fact_kind_list(
            object
                .get("required")
                .ok_or_else(|| ExportError::InvalidShape("coverage.required".into()))?,
        )?,
        covered: parse_fact_kind_list(
            object
                .get("covered")
                .ok_or_else(|| ExportError::InvalidShape("coverage.covered".into()))?,
        )?,
        missing: parse_fact_kind_list(
            object
                .get("missing")
                .ok_or_else(|| ExportError::InvalidShape("coverage.missing".into()))?,
        )?,
    })
}

fn parse_fragment_descriptor(value: &JsonValue) -> Result<FragmentDescriptor, ExportError> {
    let object = value.as_object()?;
    Ok(FragmentDescriptor {
        id: object
            .get("id")
            .ok_or_else(|| ExportError::InvalidShape("fragment.id".into()))?
            .as_str()?
            .to_string(),
        version: object
            .get("version")
            .ok_or_else(|| ExportError::InvalidShape("fragment.version".into()))?
            .as_i64()? as u32,
        hookpoints: object
            .get("hookpoints")
            .ok_or_else(|| ExportError::InvalidShape("fragment.hookpoints".into()))?
            .as_array()?
            .iter()
            .map(parse_hookpoint_value)
            .collect::<Result<Vec<_>, _>>()?,
        emits: parse_fact_kind_list(
            object
                .get("emits")
                .ok_or_else(|| ExportError::InvalidShape("fragment.emits".into()))?,
        )?,
        evidence_classes: object
            .get("evidence_classes")
            .unwrap_or(&JsonValue::Array(Vec::new()))
            .as_array()?
            .iter()
            .map(parse_evidence_class_spec)
            .collect::<Result<Vec<_>, _>>()?,
        requires: parse_fact_kind_list(
            object
                .get("requires")
                .ok_or_else(|| ExportError::InvalidShape("fragment.requires".into()))?,
        )?,
        maps: object
            .get("maps")
            .ok_or_else(|| ExportError::InvalidShape("fragment.maps".into()))?
            .as_array()?
            .iter()
            .map(parse_map_spec)
            .collect::<Result<Vec<_>, _>>()?,
        capabilities: object
            .get("capabilities")
            .ok_or_else(|| ExportError::InvalidShape("fragment.capabilities".into()))?
            .as_array()?
            .iter()
            .map(|item| match item.as_str()? {
                "tcp_state" => Ok(CapabilityFlag::TcpState),
                "packet_meta" => Ok(CapabilityFlag::PacketMeta),
                "route_meta" => Ok(CapabilityFlag::RouteMeta),
                "sock_lineage" => Ok(CapabilityFlag::SockLineage),
                _ => Err(ExportError::InvalidValue("unknown capability".into())),
            })
            .collect::<Result<Vec<_>, _>>()?,
        sampled_payload_offsets: object
            .get("sampled_payload_offsets")
            .unwrap_or(&JsonValue::Array(Vec::new()))
            .as_array()?
            .iter()
            .map(|item| Ok(item.as_i64()? as u16))
            .collect::<Result<Vec<_>, _>>()?,
        params: object
            .get("params")
            .unwrap_or(&JsonValue::Array(Vec::new()))
            .as_array()?
            .iter()
            .map(parse_fragment_param_spec)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

fn parse_evidence_class_spec(value: &JsonValue) -> Result<EvidenceClassSpec, ExportError> {
    let object = value.as_object()?;
    Ok(EvidenceClassSpec {
        fact_kind: FactKindTag::from_str(
            object
                .get("fact_kind")
                .ok_or_else(|| {
                    ExportError::InvalidShape("fragment.evidence_class.fact_kind".into())
                })?
                .as_str()?,
        )
        .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))?,
        tier: match object
            .get("tier")
            .ok_or_else(|| ExportError::InvalidShape("fragment.evidence_class.tier".into()))?
            .as_str()?
        {
            "core_requirement" => EvidenceTier::CoreRequirement,
            "optional_enhancement" => EvidenceTier::OptionalEnhancement,
            _ => return Err(ExportError::InvalidValue("unknown evidence tier".into())),
        },
    })
}

fn parse_fragment_param_spec(value: &JsonValue) -> Result<FragmentParamSpec, ExportError> {
    let object = value.as_object()?;
    Ok(FragmentParamSpec {
        key: object
            .get("key")
            .ok_or_else(|| ExportError::InvalidShape("fragment.param.key".into()))?
            .as_str()?
            .to_string(),
        value_type: match object
            .get("value_type")
            .ok_or_else(|| ExportError::InvalidShape("fragment.param.value_type".into()))?
            .as_str()?
        {
            "bool" => FragmentParamType::Bool,
            "u64" => FragmentParamType::U64,
            "string" => FragmentParamType::String,
            _ => {
                return Err(ExportError::InvalidValue(
                    "unknown fragment param type".into(),
                ));
            }
        },
    })
}

fn parse_hookpoint_value(value: &JsonValue) -> Result<HookPoint, ExportError> {
    parse_hookpoint(value.as_str()?)
}

fn parse_map_spec(value: &JsonValue) -> Result<MapSpec, ExportError> {
    let object = value.as_object()?;
    Ok(MapSpec {
        name: object
            .get("name")
            .ok_or_else(|| ExportError::InvalidShape("map.name".into()))?
            .as_str()?
            .to_string(),
        kind: match object
            .get("kind")
            .ok_or_else(|| ExportError::InvalidShape("map.kind".into()))?
            .as_str()?
        {
            "ringbuf" => MapKind::RingBuf,
            "hash" => MapKind::Hash,
            "lru_hash" => MapKind::LruHash,
            _ => return Err(ExportError::InvalidValue("unknown map kind".into())),
        },
        max_entries: object
            .get("max_entries")
            .ok_or_else(|| ExportError::InvalidShape("map.max_entries".into()))?
            .as_i64()? as u32,
    })
}

fn parse_hook_binding(value: &JsonValue) -> Result<HookBinding, ExportError> {
    let object = value.as_object()?;
    Ok(HookBinding {
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("hook_binding.fragment_id".into()))?
            .as_str()?
            .to_string(),
        hookpoint: parse_hookpoint(
            object
                .get("hookpoint")
                .ok_or_else(|| ExportError::InvalidShape("hook_binding.hookpoint".into()))?
                .as_str()?,
        )?,
    })
}

fn parse_fact_binding(value: &JsonValue) -> Result<FactBinding, ExportError> {
    let object = value.as_object()?;
    Ok(FactBinding {
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("fact_binding.fragment_id".into()))?
            .as_str()?
            .to_string(),
        emits: parse_fact_kind_list(
            object
                .get("emits")
                .ok_or_else(|| ExportError::InvalidShape("fact_binding.emits".into()))?,
        )?,
        requires: parse_fact_kind_list(
            object
                .get("requires")
                .ok_or_else(|| ExportError::InvalidShape("fact_binding.requires".into()))?,
        )?,
    })
}

fn parse_dependency_edge(value: &JsonValue) -> Result<DependencyEdge, ExportError> {
    let object = value.as_object()?;
    Ok(DependencyEdge {
        fragment_id: object
            .get("fragment_id")
            .ok_or_else(|| ExportError::InvalidShape("edge.fragment_id".into()))?
            .as_str()?
            .to_string(),
        depends_on: object
            .get("depends_on")
            .ok_or_else(|| ExportError::InvalidShape("edge.depends_on".into()))?
            .as_str()?
            .to_string(),
        fact_kind: FactKindTag::from_str(
            object
                .get("fact_kind")
                .ok_or_else(|| ExportError::InvalidShape("edge.fact_kind".into()))?
                .as_str()?,
        )
        .ok_or_else(|| ExportError::InvalidValue("unknown fact kind".into()))?,
    })
}
