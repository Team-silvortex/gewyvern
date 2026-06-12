use super::*;
use crate::fragment::{BindingDiagnostics, ModelDiagnostics, RuleDiagnostics};
use crate::ir::{FlowPredicate, NarrativeTemplate, phase_kind};
use crate::program::ProgramRule;
use crate::reason::{ReasonProfile, ReasonRule};
use crate::template::TemplateBinding;

mod build;
mod delta;
mod render;
mod shape;
mod support;

pub(super) fn ir_report_from_binding(
    binding: &TemplateBinding,
    diagnostics: Option<&BindingDiagnostics>,
) -> IrReport {
    build::ir_report_from_binding(binding, diagnostics)
}

pub(super) fn ir_text(report: &IrReport) -> String {
    render::ir_text(report)
}

pub(super) fn ir_json(report: &IrReport) -> String {
    render::ir_json(report)
}

pub(super) fn ir_history_snapshot(report: &IrReport) -> IrHistorySnapshot {
    report.history_snapshot()
}

pub(super) fn ir_history_snapshot_text(snapshot: &IrHistorySnapshot) -> String {
    render::ir_history_snapshot_text(snapshot)
}

pub(super) fn ir_history_snapshot_json(snapshot: &IrHistorySnapshot) -> String {
    render::ir_history_snapshot_json(snapshot)
}

pub(super) fn ir_lowering_delta(frontend: &FrontendReport, ir: &IrReport) -> IrLoweringDelta {
    delta::ir_lowering_delta(frontend, ir)
}

pub(super) fn ir_lowering_delta_text_lines(delta: &IrLoweringDelta) -> Vec<String> {
    delta::ir_lowering_delta_text_lines(delta)
}

pub(super) fn ir_lowering_delta_json(delta: &IrLoweringDelta) -> String {
    delta::ir_lowering_delta_json(delta)
}

pub(super) fn ir_shape_note_from_delta(delta: &IrLoweringDelta) -> String {
    delta::ir_shape_note_from_delta(delta)
}
