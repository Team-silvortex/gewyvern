mod render;

use super::explain_support::*;
use super::*;

pub(super) fn explain_text(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    render::explain_text(report, focus)
}

pub(super) fn explain_text_compact(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    render::explain_text_compact(report, focus)
}

pub(super) fn explain_json(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    render::explain_json(report, focus)
}

pub(super) fn stages_validation_json(report: &ValidationReport) -> String {
    render::stages_validation_json(report)
}

pub(super) fn explain_report(envelope: CompilerEnvelope, source: Option<&str>) -> ExplainReport {
    let ok = envelope.stages.parse.ok
        && envelope.stages.validation.ok
        && envelope.stages.diagnostics.ok
        && envelope.findings.findings.is_empty();
    let parse_source_excerpt = source.and_then(|source| {
        envelope
            .stages
            .parse
            .finding
            .as_ref()
            .and_then(|finding| source_excerpt_for_finding(source, finding))
    });
    let validation_excerpt = envelope
        .diagnostics
        .as_ref()
        .and_then(validation_excerpt_from_diagnostics);
    let diagnostics_excerpt = envelope
        .diagnostics
        .as_ref()
        .and_then(diagnostics_excerpt_from_diagnostics);
    let lowered_binding_summary = envelope
        .binding
        .as_ref()
        .map(lowered_binding_summary_from_binding);
    let frontend_lowering_delta = envelope
        .stages
        .parse
        .frontend
        .as_ref()
        .zip(lowered_binding_summary.as_ref())
        .map(|(frontend, lowered)| frontend_lowering_delta(frontend, lowered));
    let binding_shape_note = frontend_lowering_delta
        .as_ref()
        .map(binding_shape_note_from_delta);
    let ir_lowering_delta = envelope
        .stages
        .parse
        .frontend
        .as_ref()
        .zip(envelope.ir_report.as_ref())
        .map(|(frontend, ir_report)| ir_lowering_delta(frontend, ir_report));
    let ir_shape_note = ir_lowering_delta.as_ref().map(ir_shape_note_from_delta);
    let validation_shape_note = validation_excerpt
        .as_ref()
        .map(validation_shape_note_from_excerpt);
    let diagnostics_shape_note = diagnostics_excerpt
        .as_ref()
        .map(diagnostics_shape_note_from_excerpt);
    ExplainReport {
        ok,
        binding: envelope.binding,
        frontend: envelope.stages.parse.frontend.clone(),
        diagnostics: envelope.diagnostics,
        findings: envelope.findings,
        stages: envelope.stages,
        ir_report: envelope.ir_report,
        lowered_binding_summary,
        frontend_lowering_delta,
        binding_shape_note,
        ir_lowering_delta,
        ir_shape_note,
        validation_shape_note,
        diagnostics_shape_note,
        parse_source_excerpt,
        validation_excerpt,
        diagnostics_excerpt,
    }
}
