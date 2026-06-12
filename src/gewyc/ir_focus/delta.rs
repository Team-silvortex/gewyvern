use super::*;
use crate::gewyc::ir_focus::shape::{
    ir_model_shape_summary, ir_model_shape_summary_json, ir_model_shape_summary_text_lines,
    model_entries, supported_rule_count,
};
use crate::gewyc::ir_focus::support::{list_or_none, unique_strings};

pub(super) fn ir_lowering_delta(frontend: &FrontendReport, ir: &IrReport) -> IrLoweringDelta {
    let model_entries = model_entries(ir);
    let all_rules = model_entries
        .iter()
        .flat_map(|(_, model)| model.rules.iter());
    let lowered_modules = unique_strings(
        all_rules
            .clone()
            .filter_map(|rule| rule.module_name().map(str::to_string))
            .collect::<Vec<_>>(),
    );
    let lowered_phases = unique_strings(
        all_rules
            .clone()
            .filter_map(|rule| rule.phase_name().map(str::to_string))
            .collect::<Vec<_>>(),
    );
    let lowered_phase_kinds = unique_strings(
        all_rules
            .filter_map(|rule| rule.phase_kind_name().map(str::to_string))
            .collect::<Vec<_>>(),
    );
    let lowered_models = model_entries
        .iter()
        .map(|(label, model)| ir_model_shape_summary(label, model))
        .collect::<Vec<_>>();
    let supported_rule_count = model_entries
        .iter()
        .map(|(_, model)| supported_rule_count(model))
        .sum::<usize>();
    let total_rule_count: usize = model_entries
        .iter()
        .map(|(_, model)| model.rules.len())
        .sum();
    IrLoweringDelta {
        frontend_function_count: frontend.function_count,
        frontend_include_source_count: frontend.include_sources.len(),
        frontend_use_edge_count: frontend.use_edges.len(),
        frontend_graph_node_count: frontend.graph_nodes.len(),
        frontend_graph_edge_count: frontend.graph_edges.len(),
        lowered_program_rule_count: ir
            .program_model
            .as_ref()
            .map(|model| model.rules.len())
            .unwrap_or(0),
        lowered_reason_rule_count: ir
            .reason_model
            .as_ref()
            .map(|model| model.rules.len())
            .unwrap_or(0),
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
