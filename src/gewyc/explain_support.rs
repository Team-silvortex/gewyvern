use super::*;

mod analysis;
mod focus;

pub(super) fn explain_focus_text(focus: ExplainFocus) -> &'static str {
    focus::explain_focus_text(focus)
}

pub(super) fn explain_focus_text_lines(report: &ExplainReport, focus: ExplainFocus) -> Vec<String> {
    focus::explain_focus_text_lines(report, focus)
}

pub(super) fn explain_focus_json(report: &ExplainReport, focus: ExplainFocus) -> String {
    focus::explain_focus_json(report, focus)
}

pub(super) fn lowered_binding_summary_from_binding(
    binding: &BindingReport,
) -> LoweredBindingSummary {
    analysis::lowered_binding_summary_from_binding(binding)
}

pub(super) fn lowered_binding_summary_json(summary: &LoweredBindingSummary) -> String {
    analysis::lowered_binding_summary_json(summary)
}

pub(super) fn frontend_lowering_delta(
    frontend: &FrontendReport,
    lowered: &LoweredBindingSummary,
) -> FrontendLoweringDelta {
    analysis::frontend_lowering_delta(frontend, lowered)
}

pub(super) fn frontend_lowering_delta_json(delta: &FrontendLoweringDelta) -> String {
    analysis::frontend_lowering_delta_json(delta)
}

pub(super) fn binding_shape_note_from_delta(delta: &FrontendLoweringDelta) -> String {
    analysis::binding_shape_note_from_delta(delta)
}

pub(super) fn validation_shape_note_from_excerpt(excerpt: &ValidationExcerpt) -> String {
    analysis::validation_shape_note_from_excerpt(excerpt)
}

pub(super) fn diagnostics_shape_note_from_excerpt(excerpt: &DiagnosticsExcerpt) -> String {
    analysis::diagnostics_shape_note_from_excerpt(excerpt)
}

pub(super) fn explain_next_step_hint(report: &ExplainReport) -> &'static str {
    analysis::explain_next_step_hint(report)
}

pub(super) fn source_excerpt_for_finding(
    source: &str,
    finding: &CompilerFinding,
) -> Option<SourceExcerpt> {
    analysis::source_excerpt_for_finding(source, finding)
}

pub(super) fn source_excerpt_json(excerpt: &SourceExcerpt) -> String {
    analysis::source_excerpt_json(excerpt)
}

pub(super) fn json_string(value: &str) -> String {
    analysis::json_string(value)
}

pub(super) fn validation_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<ValidationExcerpt> {
    analysis::validation_excerpt_from_diagnostics(diagnostics)
}

pub(super) fn validation_excerpt_json(excerpt: &ValidationExcerpt) -> String {
    analysis::validation_excerpt_json(excerpt)
}

pub(super) fn diagnostics_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<DiagnosticsExcerpt> {
    analysis::diagnostics_excerpt_from_diagnostics(diagnostics)
}

pub(super) fn diagnostics_excerpt_json(excerpt: &DiagnosticsExcerpt) -> String {
    analysis::diagnostics_excerpt_json(excerpt)
}

pub(super) fn json_escape_string(value: &str) -> String {
    analysis::json_escape_string(value)
}
