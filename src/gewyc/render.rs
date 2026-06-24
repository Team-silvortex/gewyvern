mod surfaces;

use super::render_support::*;
use super::*;

const GEWYC_JSON_SCHEMA_VERSION: usize = 1;
const GEWYC_JSON_STABILITY: &str = "candidate";
const GEWYC_JSON_COMPATIBILITY: &str = "grouped_payload_preferred";
const GEWYC_JSON_LEGACY_FIELDS: &str = "retained_in_payload";

pub fn render_binding(binding: &TemplateBinding, format: RenderFormat) -> String {
    render_binding_report(&binding_report(binding), format)
}

pub fn render_binding_report(report: &BindingReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => binding_text(report),
        RenderFormat::Json => gewyc_surface_json("binding", binding_json(report)),
    }
}

pub fn render_frontend_report(report: &FrontendReport, format: RenderFormat) -> String {
    render_frontend_report_with_options(report, format, None, false)
}

pub fn render_frontend_report_with_focus(
    report: &FrontendReport,
    format: RenderFormat,
    focus: Option<FrontendFocus>,
) -> String {
    render_frontend_report_with_options(report, format, focus, false)
}

pub fn render_frontend_report_with_options(
    report: &FrontendReport,
    format: RenderFormat,
    focus: Option<FrontendFocus>,
    compact: bool,
) -> String {
    match format {
        RenderFormat::Text if compact => frontend_report_text_compact(report, focus),
        RenderFormat::Text => frontend_report_text(report, focus),
        RenderFormat::Json => gewyc_surface_json("frontend", frontend_report_json(report, focus)),
    }
}

pub fn render_diagnostics(
    binding: &TemplateBinding,
    diagnostics: &BindingDiagnostics,
    format: RenderFormat,
) -> String {
    render_diagnostics_report(&diagnostics_report(binding, diagnostics), format)
}

pub fn render_diagnostics_report(report: &DiagnosticsReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => diagnostics_text(report),
        RenderFormat::Json => gewyc_surface_json("diagnostics", diagnostics_json(report)),
    }
}

pub fn render_findings_report(report: &CompilerFindingsReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => findings_text(report),
        RenderFormat::Json => gewyc_surface_json("findings", findings_json(report)),
    }
}

pub fn render_stages_report(report: &CompilerStagesReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => stages_text(report),
        RenderFormat::Json => gewyc_surface_json("stages", stages_json(report)),
    }
}

pub fn render_envelope_report(report: &CompilerEnvelope, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => envelope_text(report),
        RenderFormat::Json => gewyc_surface_json("envelope", envelope_json(report)),
    }
}

pub fn render_explain_report(report: &ExplainReport, format: RenderFormat) -> String {
    render_explain_report_with_options(report, format, None, false)
}

pub fn render_explain_report_with_focus(
    report: &ExplainReport,
    format: RenderFormat,
    focus: Option<ExplainFocus>,
) -> String {
    render_explain_report_with_options(report, format, focus, false)
}

pub fn render_explain_report_with_options(
    report: &ExplainReport,
    format: RenderFormat,
    focus: Option<ExplainFocus>,
    compact: bool,
) -> String {
    match format {
        RenderFormat::Text if compact => explain_text_compact(report, focus),
        RenderFormat::Text => explain_text(report, focus),
        RenderFormat::Json => gewyc_surface_json("explain", explain_json(report, focus)),
    }
}

pub fn render_ir_history_snapshot(report: &IrReport, format: RenderFormat) -> String {
    let snapshot = ir_history_snapshot(report);
    match format {
        RenderFormat::Text => ir_history_snapshot_text(&snapshot),
        RenderFormat::Json => {
            gewyc_surface_json("ir_history_snapshot", ir_history_snapshot_json(&snapshot))
        }
    }
}

fn gewyc_surface_json(surface: &str, body: String) -> String {
    let surface_id = format!("gewyc.{surface}");
    format!(
        "{{\"surface_id\":{},\"schema_hint\":{{\"family\":\"gewyc\",\"surface\":{},\"schema_version\":{}}},\"contract_hint\":{{\"stability\":{},\"compatibility\":{},\"legacy_fields\":{}}},\"payload\":{}}}",
        json_string(&surface_id),
        json_string(surface),
        GEWYC_JSON_SCHEMA_VERSION,
        json_string(GEWYC_JSON_STABILITY),
        json_string(GEWYC_JSON_COMPATIBILITY),
        json_string(GEWYC_JSON_LEGACY_FIELDS),
        body
    )
}

pub fn binding_report(binding: &TemplateBinding) -> BindingReport {
    BindingReport {
        template_id: binding.template.id.to_string(),
        fragments: binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| (*fragment).to_string())
            .collect(),
        window: binding
            .template
            .window_profile
            .as_ref()
            .map(|window| WindowReport {
                id: window.id.to_string(),
                duration_ms: window.duration_ms,
                lateness_ms: window.lateness_ms,
            }),
        reason_profile: binding
            .template
            .reason_profile
            .as_ref()
            .map(reason_profile_report),
        program_model: binding
            .template
            .program_model
            .as_ref()
            .map(|model| ProgramModelReport {
                id: model.id.to_string(),
                operation: program_operation_text(&model.operation).to_string(),
                rules: model.rules.len(),
            }),
        fragment_params: binding
            .fragment_params
            .iter()
            .flat_map(|(fragment, params)| {
                params.iter().map(|(key, value)| FragmentParamReport {
                    fragment: fragment.clone(),
                    key: key.clone(),
                    value: fragment_param_report(value),
                })
            })
            .collect(),
        evidence_overrides: binding
            .evidence_overrides
            .iter()
            .map(|(fact_kind, tier)| EvidenceOverrideReport {
                fact_kind: fact_kind.to_string(),
                tier: evidence_tier_text(tier).to_string(),
            })
            .collect(),
    }
}

pub fn diagnostics_report(
    binding: &TemplateBinding,
    diagnostics: &BindingDiagnostics,
) -> DiagnosticsReport {
    DiagnosticsReport {
        template_id: binding.template.id.to_string(),
        fragments: binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| (*fragment).to_string())
            .collect(),
        program_model: diagnostics
            .program_model
            .as_ref()
            .map(model_diagnostics_report),
        reason_model: diagnostics
            .reason_model
            .as_ref()
            .map(model_diagnostics_report),
    }
}

pub(super) fn binding_text(report: &BindingReport) -> String {
    surfaces::binding_text(report)
}

pub(super) fn binding_json(report: &BindingReport) -> String {
    surfaces::binding_json(report)
}

pub(super) fn diagnostics_text(report: &DiagnosticsReport) -> String {
    surfaces::diagnostics_text(report)
}

pub(super) fn diagnostics_json(report: &DiagnosticsReport) -> String {
    surfaces::diagnostics_json(report)
}

pub(super) fn findings_text(report: &CompilerFindingsReport) -> String {
    surfaces::findings_text(report)
}

pub(super) fn findings_json(report: &CompilerFindingsReport) -> String {
    surfaces::findings_json(report)
}

pub(super) fn envelope_text(report: &CompilerEnvelope) -> String {
    surfaces::envelope_text(report)
}

pub(super) fn envelope_json(report: &CompilerEnvelope) -> String {
    surfaces::envelope_json(report)
}

pub(super) fn stages_text(report: &CompilerStagesReport) -> String {
    surfaces::stages_text(report)
}

pub(super) fn stages_json(report: &CompilerStagesReport) -> String {
    surfaces::stages_json(report)
}
