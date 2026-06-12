use super::*;
use crate::gewyc::ir_focus::support::list_or_none;

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
    if let Some(compare) = report.compare_models() {
        lines.extend(ir_compare_text_lines(&compare));
    }
    lines.join("\n")
}

pub(super) fn ir_json(report: &IrReport) -> String {
    format!(
        "{{\"template_id\":{},\"program_model\":{},\"reason_model\":{},\"model_compare\":{},\"history_snapshot\":{}}}",
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
        report
            .compare_models()
            .map(|compare| ir_compare_json(&compare))
            .unwrap_or_else(|| "null".into()),
        ir_history_snapshot_json(&report.history_snapshot()),
    )
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
        let support = rule.support_shape();
        let unsupported_offsets = if rule.has_unsupported_payload_offsets() {
            format!("{:?}", support.unsupported_payload_offsets)
        } else {
            "[]".into()
        };
        format!(
            "- rule[{index}] predicate={predicate} signal={signal} narrative={narrative} dedupe={dedupe} module={module} phase={phase} phase_kind={phase_kind} required={required} supporting={supporting} missing={missing} unsupported_offsets={offsets} supported={supported}",
            index = rule.rule_index,
            predicate = rule.predicate,
            signal = rule.signal_name().unwrap_or("none"),
            narrative = rule.narrative,
            dedupe = rule.dedupe,
            module = rule.module_name().unwrap_or("none"),
            phase = rule.phase_name().unwrap_or("none"),
            phase_kind = rule.phase_kind_name().unwrap_or("none"),
            required = list_or_none(support.required_facts),
            supporting = list_or_none(support.supporting_fragments),
            missing = list_or_none(support.missing_facts),
            offsets = unsupported_offsets,
            supported = support.supported,
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
    let support = rule.support_shape();
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
        rule.signal_name()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        json_string(&rule.narrative),
        rule.dedupe,
        rule.module_name()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        rule.phase_name()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        rule.phase_kind_name()
            .map(json_string)
            .unwrap_or_else(|| "null".into()),
        string_json_list(support.required_facts),
        string_json_list(support.supporting_fragments),
        string_json_list(support.missing_facts),
        u16_json_list(support.unsupported_payload_offsets),
        support.supported,
    )
}

fn ir_compare_text_lines(compare: &IrModelCompareSummary) -> Vec<String> {
    vec![
        format!("ir_compare.program_rules={}", compare.program_rule_count),
        format!("ir_compare.reason_rules={}", compare.reason_rule_count),
        format!("ir_compare.rule_delta={}", compare.rule_count_delta),
        format!(
            "ir_compare.program_supported_rules={}",
            compare.program_supported_rule_count
        ),
        format!(
            "ir_compare.reason_supported_rules={}",
            compare.reason_supported_rule_count
        ),
        format!(
            "ir_compare.supported_rule_delta={}",
            compare.supported_rule_count_delta
        ),
        format!(
            "ir_compare.shared_modules={}",
            list_or_none(&compare.shared_modules)
        ),
        format!(
            "ir_compare.program_only_modules={}",
            list_or_none(&compare.program_only_modules)
        ),
        format!(
            "ir_compare.reason_only_modules={}",
            list_or_none(&compare.reason_only_modules)
        ),
        format!(
            "ir_compare.shared_phases={}",
            list_or_none(&compare.shared_phases)
        ),
        format!(
            "ir_compare.program_only_phases={}",
            list_or_none(&compare.program_only_phases)
        ),
        format!(
            "ir_compare.reason_only_phases={}",
            list_or_none(&compare.reason_only_phases)
        ),
    ]
}

fn ir_compare_json(compare: &IrModelCompareSummary) -> String {
    format!(
        concat!(
            "{{",
            "\"program_rule_count\":{},",
            "\"reason_rule_count\":{},",
            "\"rule_count_delta\":{},",
            "\"program_supported_rule_count\":{},",
            "\"reason_supported_rule_count\":{},",
            "\"supported_rule_count_delta\":{},",
            "\"shared_modules\":[{}],",
            "\"program_only_modules\":[{}],",
            "\"reason_only_modules\":[{}],",
            "\"shared_phases\":[{}],",
            "\"program_only_phases\":[{}],",
            "\"reason_only_phases\":[{}]",
            "}}"
        ),
        compare.program_rule_count,
        compare.reason_rule_count,
        compare.rule_count_delta,
        compare.program_supported_rule_count,
        compare.reason_supported_rule_count,
        compare.supported_rule_count_delta,
        string_json_list(&compare.shared_modules),
        string_json_list(&compare.program_only_modules),
        string_json_list(&compare.reason_only_modules),
        string_json_list(&compare.shared_phases),
        string_json_list(&compare.program_only_phases),
        string_json_list(&compare.reason_only_phases),
    )
}

pub(super) fn ir_history_snapshot_text(snapshot: &IrHistorySnapshot) -> String {
    let mut lines = vec![format!("template={}", snapshot.template_id)];
    lines.push(format!(
        "operation={}",
        snapshot.operation.as_deref().unwrap_or("none")
    ));
    if let Some(model) = &snapshot.program_model {
        lines.extend(ir_history_model_snapshot_text_lines(model, "program_model"));
    } else {
        lines.push("program_model=none".into());
    }
    if let Some(model) = &snapshot.reason_model {
        lines.extend(ir_history_model_snapshot_text_lines(model, "reason_model"));
    } else {
        lines.push("reason_model=none".into());
    }
    if let Some(compare) = &snapshot.model_compare {
        lines.extend(ir_history_compare_snapshot_text_lines(compare));
    } else {
        lines.push("model_compare=none".into());
    }
    lines.join("\n")
}

pub(super) fn ir_history_snapshot_json(snapshot: &IrHistorySnapshot) -> String {
    format!(
        concat!(
            "{{",
            "\"template_id\":{},",
            "\"operation\":{},",
            "\"program_model\":{},",
            "\"reason_model\":{},",
            "\"model_compare\":{}",
            "}}"
        ),
        json_string(&snapshot.template_id),
        snapshot
            .operation
            .as_ref()
            .map(|operation| json_string(operation))
            .unwrap_or_else(|| "null".into()),
        snapshot
            .program_model
            .as_ref()
            .map(ir_history_model_snapshot_json)
            .unwrap_or_else(|| "null".into()),
        snapshot
            .reason_model
            .as_ref()
            .map(ir_history_model_snapshot_json)
            .unwrap_or_else(|| "null".into()),
        snapshot
            .model_compare
            .as_ref()
            .map(ir_history_compare_snapshot_json)
            .unwrap_or_else(|| "null".into()),
    )
}

fn ir_history_model_snapshot_text_lines(
    snapshot: &IrHistoryModelSnapshot,
    label: &str,
) -> Vec<String> {
    vec![
        format!("{label}.id={}", snapshot.id),
        format!("{label}.kind={}", snapshot.kind),
        format!("{label}.rule_count={}", snapshot.rule_count),
        format!(
            "{label}.supported_rule_count={}",
            snapshot.supported_rule_count
        ),
        format!(
            "{label}.unsupported_rule_count={}",
            snapshot.unsupported_rule_count
        ),
        format!("{label}.modules={}", list_or_none(&snapshot.modules)),
        format!("{label}.phases={}", list_or_none(&snapshot.phases)),
    ]
}

fn ir_history_model_snapshot_json(snapshot: &IrHistoryModelSnapshot) -> String {
    format!(
        concat!(
            "{{",
            "\"id\":{},",
            "\"kind\":{},",
            "\"rule_count\":{},",
            "\"supported_rule_count\":{},",
            "\"unsupported_rule_count\":{},",
            "\"modules\":[{}],",
            "\"phases\":[{}]",
            "}}"
        ),
        json_string(&snapshot.id),
        json_string(&snapshot.kind),
        snapshot.rule_count,
        snapshot.supported_rule_count,
        snapshot.unsupported_rule_count,
        string_json_list(&snapshot.modules),
        string_json_list(&snapshot.phases),
    )
}

fn ir_history_compare_snapshot_text_lines(snapshot: &IrHistoryCompareSnapshot) -> Vec<String> {
    vec![
        format!(
            "model_compare.rule_count_delta={}",
            snapshot.rule_count_delta
        ),
        format!(
            "model_compare.supported_rule_count_delta={}",
            snapshot.supported_rule_count_delta
        ),
        format!(
            "model_compare.shared_modules={}",
            list_or_none(&snapshot.shared_modules)
        ),
        format!(
            "model_compare.program_only_modules={}",
            list_or_none(&snapshot.program_only_modules)
        ),
        format!(
            "model_compare.reason_only_modules={}",
            list_or_none(&snapshot.reason_only_modules)
        ),
        format!(
            "model_compare.shared_phases={}",
            list_or_none(&snapshot.shared_phases)
        ),
        format!(
            "model_compare.program_only_phases={}",
            list_or_none(&snapshot.program_only_phases)
        ),
        format!(
            "model_compare.reason_only_phases={}",
            list_or_none(&snapshot.reason_only_phases)
        ),
    ]
}

fn ir_history_compare_snapshot_json(snapshot: &IrHistoryCompareSnapshot) -> String {
    format!(
        concat!(
            "{{",
            "\"rule_count_delta\":{},",
            "\"supported_rule_count_delta\":{},",
            "\"shared_modules\":[{}],",
            "\"program_only_modules\":[{}],",
            "\"reason_only_modules\":[{}],",
            "\"shared_phases\":[{}],",
            "\"program_only_phases\":[{}],",
            "\"reason_only_phases\":[{}]",
            "}}"
        ),
        snapshot.rule_count_delta,
        snapshot.supported_rule_count_delta,
        string_json_list(&snapshot.shared_modules),
        string_json_list(&snapshot.program_only_modules),
        string_json_list(&snapshot.reason_only_modules),
        string_json_list(&snapshot.shared_phases),
        string_json_list(&snapshot.program_only_phases),
        string_json_list(&snapshot.reason_only_phases),
    )
}
