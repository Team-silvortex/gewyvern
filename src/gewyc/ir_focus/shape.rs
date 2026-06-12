use super::*;
use crate::gewyc::ir_focus::support::list_or_none;

pub(super) fn model_entries(report: &IrReport) -> Vec<(&'static str, &IrModelReport)> {
    report.model_entries()
}

pub(super) fn supported_rule_count(model: &IrModelReport) -> usize {
    model.supported_rule_count()
}

pub(super) fn ir_model_shape_summary(label: &str, model: &IrModelReport) -> IrModelShapeSummary {
    IrModelShapeSummary {
        label: label.to_string(),
        id: model.id.clone(),
        kind: model.kind.clone(),
        rule_count: model.rules.len(),
        supported_rule_count: model.supported_rule_count(),
        unsupported_rule_count: model.unsupported_rule_count(),
        modules: model.modules(),
        phases: model.phases(),
    }
}

pub(super) fn ir_model_shape_summary_text_lines(summary: &IrModelShapeSummary) -> Vec<String> {
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

pub(super) fn ir_model_shape_summary_json(summary: &IrModelShapeSummary) -> String {
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
