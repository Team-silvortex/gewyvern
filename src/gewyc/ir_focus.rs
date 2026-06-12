use super::*;
use crate::fragment::{BindingDiagnostics, ModelDiagnostics, RuleDiagnostics};
use crate::ir::{FlowPredicate, NarrativeTemplate, phase_kind};
use crate::program::ProgramRule;
use crate::reason::{ReasonProfile, ReasonRule};
use crate::template::TemplateBinding;

pub(super) fn ir_report_from_binding(
    binding: &TemplateBinding,
    diagnostics: Option<&BindingDiagnostics>,
) -> IrReport {
    IrReport {
        template_id: binding.template.id.to_string(),
        program_model: binding
            .template
            .program_model
            .as_ref()
            .map(|model| IrModelReport {
                kind: "program_model".into(),
                id: model.id.to_string(),
                operation: Some(program_operation_text(&model.operation).to_string()),
                rules: model
                    .rules
                    .iter()
                    .enumerate()
                    .map(|(rule_index, rule)| {
                        ir_rule_report(
                            rule_index,
                            rule,
                            diagnostics.and_then(|all| all.program_model.as_ref()),
                        )
                    })
                    .collect(),
            }),
        reason_model: binding.template.reason_profile.as_ref().map(|profile| {
            let (kind, id, rules): (&str, &str, &[ReasonRule]) = match profile {
                ReasonProfile::HandshakeL1 => ("builtin_reason_profile", profile.id(), &[]),
                ReasonProfile::UdpDatagramL1 => ("builtin_reason_profile", profile.id(), &[]),
                ReasonProfile::Declarative(model) => {
                    ("declarative_reason_model", model.id, model.rules.as_slice())
                }
            };
            IrModelReport {
                kind: kind.into(),
                id: id.into(),
                operation: None,
                rules: rules
                    .iter()
                    .enumerate()
                    .map(|(rule_index, rule)| {
                        ir_rule_report(
                            rule_index,
                            rule,
                            diagnostics.and_then(|all| all.reason_model.as_ref()),
                        )
                    })
                    .collect(),
            }
        }),
    }
}

pub(super) fn ir_text(report: &IrReport) -> String {
    let mut lines = vec![format!("template={}", report.template_id)];
    if let Some(model) = &report.program_model {
        lines.extend(ir_model_text_lines(model, "program_model"));
    } else {
        lines.push("program_model=none".into());
    }
    if let Some(model) = &report.reason_model {
        lines.extend(ir_model_text_lines(model, "reason_model"));
    } else {
        lines.push("reason_model=none".into());
    }
    lines.join("\n")
}

pub(super) fn ir_json(report: &IrReport) -> String {
    format!(
        "{{\"template_id\":{},\"program_model\":{},\"reason_model\":{}}}",
        json_string(&report.template_id),
        report
            .program_model
            .as_ref()
            .map(ir_model_json)
            .unwrap_or_else(|| "null".into()),
        report
            .reason_model
            .as_ref()
            .map(ir_model_json)
            .unwrap_or_else(|| "null".into()),
    )
}

pub(super) fn ir_lowering_delta(frontend: &FrontendReport, ir: &IrReport) -> IrLoweringDelta {
    let program_rules = ir
        .program_model
        .as_ref()
        .map(|model| model.rules.as_slice())
        .unwrap_or(&[]);
    let reason_rules = ir
        .reason_model
        .as_ref()
        .map(|model| model.rules.as_slice())
        .unwrap_or(&[]);
    let all_rules = program_rules.iter().chain(reason_rules.iter());
    let lowered_modules = unique_strings(
        all_rules
            .clone()
            .filter_map(|rule| rule.module.clone())
            .collect::<Vec<_>>(),
    );
    let lowered_phases = unique_strings(
        all_rules
            .clone()
            .filter_map(|rule| rule.phase.clone())
            .collect::<Vec<_>>(),
    );
    let lowered_phase_kinds = unique_strings(
        all_rules
            .filter_map(|rule| rule.phase_kind.clone())
            .collect::<Vec<_>>(),
    );
    let lowered_models = [
        ir.program_model
            .as_ref()
            .map(|model| ir_model_shape_summary("program_model", model)),
        ir.reason_model
            .as_ref()
            .map(|model| ir_model_shape_summary("reason_model", model)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let supported_rule_count = program_rules
        .iter()
        .chain(reason_rules.iter())
        .filter(|rule| rule.supported)
        .count();
    let total_rule_count = program_rules.len() + reason_rules.len();
    IrLoweringDelta {
        frontend_function_count: frontend.function_count,
        frontend_include_source_count: frontend.include_sources.len(),
        frontend_use_edge_count: frontend.use_edges.len(),
        frontend_graph_node_count: frontend.graph_nodes.len(),
        frontend_graph_edge_count: frontend.graph_edges.len(),
        lowered_program_rule_count: program_rules.len(),
        lowered_reason_rule_count: reason_rules.len(),
        lowered_supported_rule_count: supported_rule_count,
        lowered_unsupported_rule_count: total_rule_count.saturating_sub(supported_rule_count),
        lowered_modules,
        lowered_phases,
        lowered_phase_kinds,
        lowered_models,
    }
}

pub(super) fn ir_lowering_delta_text_lines(delta: &IrLoweringDelta) -> Vec<String> {
    vec![
        format!(
            "ir_delta.frontend_functions={}",
            delta.frontend_function_count
        ),
        format!(
            "ir_delta.frontend_includes={}",
            delta.frontend_include_source_count
        ),
        format!(
            "ir_delta.frontend_use_edges={}",
            delta.frontend_use_edge_count
        ),
        format!(
            "ir_delta.frontend_graph_nodes={}",
            delta.frontend_graph_node_count
        ),
        format!(
            "ir_delta.frontend_graph_edges={}",
            delta.frontend_graph_edge_count
        ),
        format!(
            "ir_delta.lowered_program_rules={}",
            delta.lowered_program_rule_count
        ),
        format!(
            "ir_delta.lowered_reason_rules={}",
            delta.lowered_reason_rule_count
        ),
        format!(
            "ir_delta.lowered_supported_rules={}",
            delta.lowered_supported_rule_count
        ),
        format!(
            "ir_delta.lowered_unsupported_rules={}",
            delta.lowered_unsupported_rule_count
        ),
        format!(
            "ir_delta.lowered_modules={}",
            list_or_none(&delta.lowered_modules)
        ),
        format!(
            "ir_delta.lowered_phases={}",
            list_or_none(&delta.lowered_phases)
        ),
        format!(
            "ir_delta.lowered_phase_kinds={}",
            list_or_none(&delta.lowered_phase_kinds)
        ),
    ]
    .into_iter()
    .chain(
        delta
            .lowered_models
            .iter()
            .flat_map(ir_model_shape_summary_text_lines),
    )
    .collect()
}

pub(super) fn ir_lowering_delta_json(delta: &IrLoweringDelta) -> String {
    format!(
        concat!(
            "{{",
            "\"frontend_function_count\":{},",
            "\"frontend_include_source_count\":{},",
            "\"frontend_use_edge_count\":{},",
            "\"frontend_graph_node_count\":{},",
            "\"frontend_graph_edge_count\":{},",
            "\"lowered_program_rule_count\":{},",
            "\"lowered_reason_rule_count\":{},",
            "\"lowered_supported_rule_count\":{},",
            "\"lowered_unsupported_rule_count\":{},",
            "\"lowered_modules\":[{}],",
            "\"lowered_phases\":[{}],",
            "\"lowered_phase_kinds\":[{}],",
            "\"lowered_models\":[{}]",
            "}}"
        ),
        delta.frontend_function_count,
        delta.frontend_include_source_count,
        delta.frontend_use_edge_count,
        delta.frontend_graph_node_count,
        delta.frontend_graph_edge_count,
        delta.lowered_program_rule_count,
        delta.lowered_reason_rule_count,
        delta.lowered_supported_rule_count,
        delta.lowered_unsupported_rule_count,
        string_json_list(&delta.lowered_modules),
        string_json_list(&delta.lowered_phases),
        string_json_list(&delta.lowered_phase_kinds),
        delta
            .lowered_models
            .iter()
            .map(ir_model_shape_summary_json)
            .collect::<Vec<_>>()
            .join(","),
    )
}

pub(super) fn ir_shape_note_from_delta(delta: &IrLoweringDelta) -> String {
    if delta.lowered_unsupported_rule_count > 0 {
        return "lowered IR still contains unsupported rules; inspect required facts, supporting fragments, and payload offsets before treating the module as runtime-ready".into();
    }
    if delta.lowered_program_rule_count + delta.lowered_reason_rule_count == 0 {
        return "front-end shape resolved, but the lowered IR is still effectively empty; check whether the package only references builtin reason profiles or has not materialized declarative rules yet".into();
    }
    if delta.frontend_function_count > 0
        && delta.lowered_program_rule_count + delta.lowered_reason_rule_count
            > delta.frontend_function_count
    {
        return "lowered IR is denser than the front-end module graph because pipeline steps collapsed into multiple rule-sized entries with explicit module and phase metadata".into();
    }
    "front-end and lowered IR look structurally aligned; inspect modules, phases, and reason-model provenance when you need finer rule-level detail".into()
}

fn ir_model_text_lines(model: &IrModelReport, label: &str) -> Vec<String> {
    let mut lines = vec![format!(
        "{}={} kind={} rules={}",
        label,
        model.id,
        model.kind,
        model.rules.len()
    )];
    if let Some(operation) = &model.operation {
        lines.push(format!("{label}_operation={operation}"));
    }
    if model.rules.is_empty() {
        lines.push(format!("{label}_rules=none"));
        return lines;
    }
    lines.push(format!("{label}_rules:"));
    lines.extend(model.rules.iter().map(|rule| {
        format!(
            "- rule[{index}] predicate={predicate} signal={signal} narrative={narrative} dedupe={dedupe} module={module} phase={phase} phase_kind={phase_kind} required={required} supporting={supporting} missing={missing} unsupported_offsets={offsets:?} supported={supported}",
            index = rule.rule_index,
            predicate = rule.predicate,
            signal = rule.signal.as_deref().unwrap_or("none"),
            narrative = rule.narrative,
            dedupe = rule.dedupe,
            module = rule.module.as_deref().unwrap_or("none"),
            phase = rule.phase.as_deref().unwrap_or("none"),
            phase_kind = rule.phase_kind.as_deref().unwrap_or("none"),
            required = list_or_none(&rule.required_facts),
            supporting = list_or_none(&rule.supporting_fragments),
            missing = list_or_none(&rule.missing_facts),
            offsets = rule.unsupported_payload_offsets,
            supported = rule.supported,
        )
    }));
    lines
}

fn ir_model_json(model: &IrModelReport) -> String {
    format!(
        "{{\"kind\":{},\"id\":{},\"operation\":{},\"rules\":[{}]}}",
        json_string(&model.kind),
        json_string(&model.id),
        model
            .operation
            .as_ref()
            .map(|operation| json_string(operation))
            .unwrap_or_else(|| "null".into()),
        model
            .rules
            .iter()
            .map(ir_rule_json)
            .collect::<Vec<_>>()
            .join(","),
    )
}

fn ir_rule_json(rule: &IrRuleReport) -> String {
    format!(
        concat!(
            "{{",
            "\"rule_index\":{},",
            "\"predicate\":{},",
            "\"signal\":{},",
            "\"narrative\":{},",
            "\"dedupe\":{},",
            "\"module\":{},",
            "\"phase\":{},",
            "\"phase_kind\":{},",
            "\"required_facts\":[{}],",
            "\"supporting_fragments\":[{}],",
            "\"missing_facts\":[{}],",
            "\"unsupported_payload_offsets\":[{}],",
            "\"supported\":{}",
            "}}"
        ),
        rule.rule_index,
        json_string(&rule.predicate),
        rule.signal
            .as_ref()
            .map(|signal| json_string(signal))
            .unwrap_or_else(|| "null".into()),
        json_string(&rule.narrative),
        rule.dedupe,
        rule.module
            .as_ref()
            .map(|module| json_string(module))
            .unwrap_or_else(|| "null".into()),
        rule.phase
            .as_ref()
            .map(|phase| json_string(phase))
            .unwrap_or_else(|| "null".into()),
        rule.phase_kind
            .as_ref()
            .map(|phase_kind| json_string(phase_kind))
            .unwrap_or_else(|| "null".into()),
        string_json_list(&rule.required_facts),
        string_json_list(&rule.supporting_fragments),
        string_json_list(&rule.missing_facts),
        u16_json_list(&rule.unsupported_payload_offsets),
        rule.supported,
    )
}

fn ir_rule_report(
    rule_index: usize,
    rule: &ProgramRule,
    diagnostics: Option<&ModelDiagnostics>,
) -> IrRuleReport {
    let diagnostics = diagnostics.and_then(|model| rule_diagnostics(model, rule_index));
    let required_facts = rule
        .predicate
        .required_fact_kinds()
        .into_iter()
        .map(|fact| fact.to_string())
        .collect::<Vec<_>>();
    IrRuleReport {
        rule_index,
        predicate: predicate_summary(&rule.predicate),
        signal: rule.signal.as_ref().map(|signal| signal.id().to_string()),
        narrative: narrative_summary(&rule.narrative),
        dedupe: rule.dedupe,
        module: rule.module.clone(),
        phase: rule.phase.clone(),
        phase_kind: rule
            .signal
            .as_ref()
            .and_then(|signal| phase_kind(signal, rule.phase.as_deref()))
            .map(str::to_string),
        required_facts: diagnostics
            .map(|diagnostics| {
                diagnostics
                    .required_facts
                    .iter()
                    .map(|fact| fact.to_string())
                    .collect()
            })
            .unwrap_or(required_facts),
        supporting_fragments: diagnostics
            .map(|diagnostics| diagnostics.supporting_fragments.clone())
            .unwrap_or_default(),
        missing_facts: diagnostics
            .map(|diagnostics| {
                diagnostics
                    .missing_facts
                    .iter()
                    .map(|fact| fact.to_string())
                    .collect()
            })
            .unwrap_or_default(),
        unsupported_payload_offsets: diagnostics
            .map(|diagnostics| diagnostics.unsupported_payload_offsets.clone())
            .unwrap_or_default(),
        supported: diagnostics
            .map(|diagnostics| diagnostics.supported)
            .unwrap_or(true),
    }
}

fn rule_diagnostics(diagnostics: &ModelDiagnostics, rule_index: usize) -> Option<&RuleDiagnostics> {
    diagnostics
        .rules
        .iter()
        .find(|diagnostics| diagnostics.rule_index == rule_index)
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "none".into()
    } else {
        items.join(",")
    }
}

fn ir_model_shape_summary(label: &str, model: &IrModelReport) -> IrModelShapeSummary {
    let supported_rule_count = model.rules.iter().filter(|rule| rule.supported).count();
    let modules = unique_strings(
        model
            .rules
            .iter()
            .filter_map(|rule| rule.module.clone())
            .collect::<Vec<_>>(),
    );
    let phases = unique_strings(
        model
            .rules
            .iter()
            .filter_map(|rule| rule.phase.clone())
            .collect::<Vec<_>>(),
    );
    IrModelShapeSummary {
        label: label.to_string(),
        id: model.id.clone(),
        kind: model.kind.clone(),
        rule_count: model.rules.len(),
        supported_rule_count,
        unsupported_rule_count: model.rules.len().saturating_sub(supported_rule_count),
        modules,
        phases,
    }
}

fn ir_model_shape_summary_text_lines(summary: &IrModelShapeSummary) -> Vec<String> {
    vec![
        format!("ir_delta.model.{}.id={}", summary.label, summary.id),
        format!("ir_delta.model.{}.kind={}", summary.label, summary.kind),
        format!(
            "ir_delta.model.{}.rules={}",
            summary.label, summary.rule_count
        ),
        format!(
            "ir_delta.model.{}.supported_rules={}",
            summary.label, summary.supported_rule_count
        ),
        format!(
            "ir_delta.model.{}.unsupported_rules={}",
            summary.label, summary.unsupported_rule_count
        ),
        format!(
            "ir_delta.model.{}.modules={}",
            summary.label,
            list_or_none(&summary.modules)
        ),
        format!(
            "ir_delta.model.{}.phases={}",
            summary.label,
            list_or_none(&summary.phases)
        ),
    ]
}

fn ir_model_shape_summary_json(summary: &IrModelShapeSummary) -> String {
    format!(
        concat!(
            "{{",
            "\"label\":{},",
            "\"id\":{},",
            "\"kind\":{},",
            "\"rule_count\":{},",
            "\"supported_rule_count\":{},",
            "\"unsupported_rule_count\":{},",
            "\"modules\":[{}],",
            "\"phases\":[{}]",
            "}}"
        ),
        json_string(&summary.label),
        json_string(&summary.id),
        json_string(&summary.kind),
        summary.rule_count,
        summary.supported_rule_count,
        summary.unsupported_rule_count,
        string_json_list(&summary.modules),
        string_json_list(&summary.phases),
    )
}

fn unique_strings(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items.dedup();
    items
}

fn narrative_summary(narrative: &NarrativeTemplate) -> String {
    match narrative {
        NarrativeTemplate::None => "none".into(),
        NarrativeTemplate::Static(line) => format!("static:{line}"),
        NarrativeTemplate::ProcessBound => "process_bound".into(),
        NarrativeTemplate::PacketObserved => "packet_observed".into(),
        NarrativeTemplate::TransportPayloadSent => "transport_payload_sent".into(),
        NarrativeTemplate::TransportPayloadReceived => "transport_payload_received".into(),
        NarrativeTemplate::TcpStateTransition => "tcp_state_transition".into(),
        NarrativeTemplate::RouteChanged => "route_changed".into(),
        NarrativeTemplate::UdpDatagramObserved => "udp_datagram_observed".into(),
        NarrativeTemplate::UdpDatagramSent => "udp_datagram_sent".into(),
        NarrativeTemplate::UdpDatagramReceived => "udp_datagram_received".into(),
    }
}

fn predicate_summary(predicate: &FlowPredicate) -> String {
    match predicate {
        FlowPredicate::ProcessBound => "process_bound".into(),
        FlowPredicate::SocketStateObserved {
            local_port,
            remote_port,
            min_new_state,
        } => format!(
            "socket_state_observed(local_port={},remote_port={},min_new_state={})",
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u8(*min_new_state),
        ),
        FlowPredicate::PacketObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            ..
        } => format!(
            "packet_observed(l4_proto={},dir={},local_port={},remote_port={},payload_offsets={:?})",
            l4_proto,
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::DatagramObserved {
            l4_proto,
            dir,
            local_port,
            remote_port,
            min_len,
            ..
        } => format!(
            "datagram_observed(l4_proto={},dir={},local_port={},remote_port={},min_len={},payload_offsets={:?})",
            l4_proto,
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u32(*min_len),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::QuicPacketObserved {
            dir,
            local_port,
            remote_port,
            min_len,
            long_header,
            packet_type,
        } => format!(
            "quic_packet_observed(dir={},local_port={},remote_port={},min_len={},long_header={},packet_type={})",
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            optional_u32(*min_len),
            optional_bool(*long_header),
            packet_type
                .map(|item| format!("{item:?}"))
                .unwrap_or_else(|| "none".into()),
        ),
        FlowPredicate::QuicFrameObserved {
            dir,
            local_port,
            remote_port,
            packet_type,
            frame_type,
            ..
        } => format!(
            "quic_frame_observed(dir={},local_port={},remote_port={},packet_type={},frame_type={frame_type:?},payload_offsets={:?})",
            optional_dir(*dir),
            optional_u16(*local_port),
            optional_u16(*remote_port),
            packet_type
                .map(|item| format!("{item:?}"))
                .unwrap_or_else(|| "none".into()),
            predicate.required_payload_offsets(),
        ),
        FlowPredicate::RouteResolved => "route_resolved".into(),
        FlowPredicate::All(predicates) => format!(
            "all({})",
            predicates
                .iter()
                .map(predicate_summary)
                .collect::<Vec<_>>()
                .join(" && ")
        ),
        FlowPredicate::Any(predicates) => format!(
            "any({})",
            predicates
                .iter()
                .map(predicate_summary)
                .collect::<Vec<_>>()
                .join(" || ")
        ),
    }
}

fn optional_bool(value: Option<bool>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_dir(value: Option<crate::ledger::PacketDir>) -> String {
    value
        .map(|value| format!("{value:?}").to_lowercase())
        .unwrap_or_else(|| "none".into())
}

fn optional_u8(value: Option<u8>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_u16(value: Option<u16>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}

fn optional_u32(value: Option<u32>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "none".into())
}
