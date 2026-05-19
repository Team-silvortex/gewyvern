use crate::dsl::{
    DslError, FrontendDslKind, FrontendExpansionPreview, FrontendFunctionNode, FrontendGraphEdge,
    FrontendGraphEdgeKind, FrontendGraphNode, FrontendGraphNodeKind, FrontendModuleSummary,
    FrontendUseEdge, compile_file, parse_file_unvalidated, parse_str_unvalidated,
    summarize_frontend_file, summarize_frontend_str, validate_compiled_binding,
};
use crate::flow::ProgramOperation;
use crate::fragment::{
    BindingDiagnostics, EvidenceTier, ModelDiagnostics, PayloadOffsetSupportSummary, RegistryError,
    RuleTier, builtin_registry,
};
use crate::reason::ReasonProfile;
use crate::template::{FragmentParamValue, TemplateBinding};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderFormat {
    Text,
    Json,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BindingReport {
    pub template_id: String,
    pub fragments: Vec<String>,
    pub window: Option<WindowReport>,
    pub reason_profile: Option<ReasonProfileReport>,
    pub program_model: Option<ProgramModelReport>,
    pub fragment_params: Vec<FragmentParamReport>,
    pub evidence_overrides: Vec<EvidenceOverrideReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowReport {
    pub id: String,
    pub duration_ms: u64,
    pub lateness_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReasonProfileReport {
    Builtin { id: String },
    Declarative { id: String, rules: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramModelReport {
    pub id: String,
    pub operation: String,
    pub rules: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FragmentParamReport {
    pub fragment: String,
    pub key: String,
    pub value: ParamValueReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParamValueReport {
    Bool(bool),
    U64(u64),
    String(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidenceOverrideReport {
    pub fact_kind: String,
    pub tier: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsReport {
    pub template_id: String,
    pub fragments: Vec<String>,
    pub program_model: Option<ModelDiagnosticsReport>,
    pub reason_model: Option<ModelDiagnosticsReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelDiagnosticsReport {
    pub model: String,
    pub rules: Vec<RuleDiagnosticsReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleDiagnosticsReport {
    pub rule_index: usize,
    pub tier: String,
    pub supported: bool,
    pub required_facts: Vec<String>,
    pub supporting_fragments: Vec<String>,
    pub missing_facts: Vec<String>,
    pub unsupported_payload_offsets: Vec<u16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerStagesReport {
    pub parse: ParseStageReport,
    pub validation: ValidationReport,
    pub diagnostics: DiagnosticsStageReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerEnvelope {
    pub binding: Option<BindingReport>,
    pub diagnostics: Option<DiagnosticsReport>,
    pub findings: CompilerFindingsReport,
    pub stages: CompilerStagesReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplainReport {
    pub ok: bool,
    pub binding: Option<BindingReport>,
    pub frontend: Option<FrontendReport>,
    pub diagnostics: Option<DiagnosticsReport>,
    pub findings: CompilerFindingsReport,
    pub stages: CompilerStagesReport,
    pub lowered_binding_summary: Option<LoweredBindingSummary>,
    pub frontend_lowering_delta: Option<FrontendLoweringDelta>,
    pub binding_shape_note: Option<String>,
    pub validation_shape_note: Option<String>,
    pub diagnostics_shape_note: Option<String>,
    pub parse_source_excerpt: Option<SourceExcerpt>,
    pub validation_excerpt: Option<ValidationExcerpt>,
    pub diagnostics_excerpt: Option<DiagnosticsExcerpt>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExplainFocus {
    Parse,
    Frontend,
    Binding,
    Validation,
    Diagnostics,
    Findings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoweredBindingSummary {
    pub fragment_count: usize,
    pub has_window: bool,
    pub has_reason_profile: bool,
    pub has_program_model: bool,
    pub program_rule_count: usize,
    pub fragment_param_count: usize,
    pub evidence_override_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendLoweringDelta {
    pub frontend_function_count: usize,
    pub frontend_merged_step_count: usize,
    pub frontend_use_edge_count: usize,
    pub frontend_include_source_count: usize,
    pub lowered_fragment_count: usize,
    pub lowered_program_rule_count: usize,
    pub lowered_fragment_param_count: usize,
    pub lowered_evidence_override_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrontendFocus {
    Functions,
    Includes,
    Graph,
    Expansion,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseStageReport {
    pub ok: bool,
    pub frontend: Option<FrontendReport>,
    pub report: Option<BindingReport>,
    pub finding: Option<CompilerFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendReport {
    pub kind: String,
    pub function_count: usize,
    pub function_nodes: Vec<FrontendFunctionReport>,
    pub merged_step_count: usize,
    pub include_sources: Vec<String>,
    pub use_edges: Vec<FrontendUseEdgeReport>,
    pub graph_nodes: Vec<FrontendGraphNodeReport>,
    pub graph_edges: Vec<FrontendGraphEdgeReport>,
    pub expansion_previews: Vec<FrontendExpansionPreviewReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionReport {
    pub name: String,
    pub step_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendUseEdgeReport {
    pub from: String,
    pub to: String,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphNodeReport {
    pub id: String,
    pub kind: String,
    pub step_count: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendGraphEdgeReport {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendExpansionPreviewReport {
    pub scope: String,
    pub local_bindings: Vec<String>,
    pub steps: Vec<String>,
    pub use_targets: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationReport {
    pub ok: bool,
    pub registry: String,
    pub fragment_count: usize,
    pub program_rule_count: usize,
    pub reason_rule_count: usize,
    pub checks: Vec<String>,
    pub sampled_payload_offsets: Vec<u16>,
    pub required_payload_offsets: Vec<u16>,
    pub unsupported_payload_offsets: Vec<u16>,
    pub finding: Option<CompilerFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsStageReport {
    pub ok: bool,
    pub report: Option<DiagnosticsReport>,
    pub finding: Option<CompilerFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFindingsReport {
    pub findings: Vec<CompilerFinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerFinding {
    pub stage: CompilerFindingStage,
    pub code: String,
    pub severity: CompilerFindingSeverity,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceExcerpt {
    pub line: usize,
    pub column: Option<usize>,
    pub line_text: String,
    pub marker: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationExcerpt {
    pub model: String,
    pub rule_index: usize,
    pub unsupported_payload_offsets: Vec<u16>,
    pub supporting_fragments: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiagnosticsExcerpt {
    pub model: String,
    pub rule_index: usize,
    pub missing_facts: Vec<String>,
    pub unsupported_payload_offsets: Vec<u16>,
    pub supporting_fragments: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerFindingStage {
    Parse,
    Validation,
    Diagnostics,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerFindingSeverity {
    Error,
    Warning,
}

pub fn compile_binding_file(path: &str) -> Result<TemplateBinding, DslError> {
    compile_file(path)
}

pub fn compile_binding_report_file(path: &str) -> Result<BindingReport, DslError> {
    compile_envelope_file(path).and_then(|envelope| {
        envelope
            .binding
            .ok_or_else(|| DslError::InvalidValue("binding report unavailable".into()))
    })
}

pub fn compile_frontend_report_file(path: &str) -> Result<FrontendReport, DslError> {
    summarize_frontend_file(path).map(frontend_report)
}

pub fn compile_frontend_report_str(input: &str) -> Result<FrontendReport, DslError> {
    summarize_frontend_str(input).map(frontend_report)
}

pub fn collect_binding_diagnostics(
    binding: &TemplateBinding,
) -> Result<BindingDiagnostics, RegistryError> {
    builtin_registry().binding_diagnostics(binding)
}

pub fn compile_diagnostics_report_file(
    path: &str,
) -> Result<DiagnosticsReport, CompileDiagnosticsError> {
    let envelope = compile_envelope_file(path)?;
    envelope.diagnostics.ok_or_else(|| {
        let finding = envelope
            .findings
            .findings
            .first()
            .map(|finding| finding.message.clone())
            .unwrap_or_else(|| "diagnostics report unavailable".into());
        CompileDiagnosticsError::Dsl(DslError::InvalidValue(finding))
    })
}

pub fn compile_stages_report_file(path: &str) -> Result<CompilerStagesReport, CompileStagesError> {
    let envelope = compile_envelope_file(path)?;
    Ok(envelope.stages)
}

pub fn compile_stages_report_str(input: &str) -> CompilerStagesReport {
    compile_envelope_str(input).stages
}

pub fn compile_findings_report_file(path: &str) -> CompilerFindingsReport {
    match compile_envelope_file(path) {
        Ok(envelope) => envelope.findings,
        Err(err) => CompilerFindingsReport {
            findings: vec![finding_from_dsl_error(&err)],
        },
    }
}

pub fn compile_findings_report_str(input: &str) -> CompilerFindingsReport {
    compile_envelope_str(input).findings
}

pub fn compile_explain_report_file(path: &str) -> Result<ExplainReport, DslError> {
    let source = std::fs::read_to_string(path).ok();
    compile_envelope_file(path).map(|envelope| explain_report(envelope, source.as_deref()))
}

pub fn compile_explain_report_str(input: &str) -> ExplainReport {
    explain_report(compile_envelope_str(input), Some(input))
}

pub fn compile_envelope_file(path: &str) -> Result<CompilerEnvelope, DslError> {
    let frontend = summarize_frontend_file(path).ok().map(frontend_report);
    Ok(compile_envelope_from_parse_result(
        parse_file_unvalidated(path),
        frontend,
    ))
}

pub fn compile_envelope_str(input: &str) -> CompilerEnvelope {
    let frontend = summarize_frontend_str(input).ok().map(frontend_report);
    compile_envelope_from_parse_result(parse_str_unvalidated(input), frontend)
}

fn compile_envelope_from_parse_result(
    parsed: Result<TemplateBinding, DslError>,
    frontend: Option<FrontendReport>,
) -> CompilerEnvelope {
    match parsed {
        Ok(binding) => {
            let binding_report = binding_report(&binding);
            let diagnostics_result = collect_binding_diagnostics(&binding);
            let validation_result = validate_compiled_binding(&binding);
            let validation = validation_report(
                &binding,
                diagnostics_result.as_ref().ok(),
                validation_result.err().as_ref(),
            );
            let diagnostics_stage = diagnostics_stage_report(&binding, diagnostics_result);
            let diagnostics = diagnostics_stage.report.clone();
            let parse = ParseStageReport {
                ok: true,
                frontend,
                report: Some(binding_report.clone()),
                finding: None,
            };
            let findings = findings_from_stage_reports(&parse, &validation, &diagnostics_stage);
            let stages = CompilerStagesReport {
                parse,
                validation,
                diagnostics: diagnostics_stage,
            };
            CompilerEnvelope {
                binding: Some(binding_report),
                diagnostics,
                findings,
                stages,
            }
        }
        Err(err) => {
            let parse = ParseStageReport {
                ok: false,
                frontend,
                report: None,
                finding: Some(finding_from_dsl_error(&err)),
            };
            let validation = empty_validation_report();
            let diagnostics = DiagnosticsStageReport {
                ok: false,
                report: None,
                finding: None,
            };
            CompilerEnvelope {
                binding: None,
                diagnostics: None,
                findings: findings_from_stage_reports(&parse, &validation, &diagnostics),
                stages: CompilerStagesReport {
                    parse,
                    validation,
                    diagnostics,
                },
            }
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CompileDiagnosticsError {
    Dsl(DslError),
    Registry(RegistryError),
}

impl From<DslError> for CompileDiagnosticsError {
    fn from(value: DslError) -> Self {
        Self::Dsl(value)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum CompileStagesError {
    Dsl(DslError),
}

impl From<DslError> for CompileStagesError {
    fn from(value: DslError) -> Self {
        Self::Dsl(value)
    }
}

impl From<RegistryError> for CompileDiagnosticsError {
    fn from(value: RegistryError) -> Self {
        Self::Registry(value)
    }
}

pub fn render_binding(binding: &TemplateBinding, format: RenderFormat) -> String {
    render_binding_report(&binding_report(binding), format)
}

pub fn render_binding_report(report: &BindingReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => binding_text(report),
        RenderFormat::Json => binding_json(report),
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
        RenderFormat::Json => frontend_report_json(report, focus),
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
        RenderFormat::Json => diagnostics_json(report),
    }
}

pub fn render_findings_report(report: &CompilerFindingsReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => findings_text(report),
        RenderFormat::Json => findings_json(report),
    }
}

pub fn render_stages_report(report: &CompilerStagesReport, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => stages_text(report),
        RenderFormat::Json => stages_json(report),
    }
}

pub fn render_envelope_report(report: &CompilerEnvelope, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => envelope_text(report),
        RenderFormat::Json => envelope_json(report),
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
        RenderFormat::Json => explain_json(report, focus),
    }
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

fn binding_text(report: &BindingReport) -> String {
    let mut lines = vec![
        format!("template={}", report.template_id),
        format!("fragments={}", report.fragments.join(",")),
    ];

    if let Some(window) = &report.window {
        lines.push(format!(
            "window={} duration_ms={} lateness_ms={}",
            window.id, window.duration_ms, window.lateness_ms
        ));
    }

    if let Some(reason) = &report.reason_profile {
        lines.push(format!("reason={}", reason_profile_text(reason)));
    }

    if let Some(model) = &report.program_model {
        lines.push(format!(
            "program_model={} operation={} rules={}",
            model.id, model.operation, model.rules
        ));
    }

    for param in &report.fragment_params {
        lines.push(format!(
            "param={}.{}={}",
            param.fragment,
            param.key,
            fragment_param_text(&param.value)
        ));
    }

    for evidence in &report.evidence_overrides {
        lines.push(format!("evidence={}:{}", evidence.fact_kind, evidence.tier));
    }

    lines.join("\n")
}

fn binding_json(report: &BindingReport) -> String {
    let fragment_params = report
        .fragment_params
        .iter()
        .fold(Vec::<(String, Vec<String>)>::new(), |mut acc, param| {
            if let Some((_, entries)) = acc
                .iter_mut()
                .find(|(fragment, _)| fragment == &param.fragment)
            {
                entries.push(format!(
                    "\"{}\":{}",
                    param.key,
                    fragment_param_json(&param.value)
                ));
            } else {
                acc.push((
                    param.fragment.clone(),
                    vec![format!(
                        "\"{}\":{}",
                        param.key,
                        fragment_param_json(&param.value)
                    )],
                ));
            }
            acc
        })
        .into_iter()
        .map(|(fragment, entries)| format!("\"{fragment}\":{{{}}}", entries.join(",")))
        .collect::<Vec<_>>()
        .join(",");
    let evidence_overrides = report
        .evidence_overrides
        .iter()
        .map(|evidence| format!("\"{}\":\"{}\"", evidence.fact_kind, evidence.tier))
        .collect::<Vec<_>>()
        .join(",");

    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"fragments\":[{}],",
            "\"window\":{},",
            "\"reason_profile\":{},",
            "\"program_model\":{},",
            "\"fragment_params\":{{{}}},",
            "\"evidence_overrides\":{{{}}}",
            "}}"
        ),
        report.template_id,
        report
            .fragments
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        report
            .window
            .as_ref()
            .map_or("null".into(), |window| format!(
                "{{\"id\":\"{}\",\"duration_ms\":{},\"lateness_ms\":{}}}",
                window.id, window.duration_ms, window.lateness_ms
            )),
        report
            .reason_profile
            .as_ref()
            .map_or("null".into(), reason_profile_json),
        report
            .program_model
            .as_ref()
            .map_or("null".into(), |model| format!(
                "{{\"id\":\"{}\",\"operation\":\"{}\",\"rules\":{}}}",
                model.id, model.operation, model.rules
            )),
        fragment_params,
        evidence_overrides
    )
}

fn diagnostics_text(report: &DiagnosticsReport) -> String {
    let mut lines = vec![
        format!("template={}", report.template_id),
        format!("fragments={}", report.fragments.join(",")),
    ];

    if let Some(model) = &report.program_model {
        lines.push(format!("program_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?} unsupported_offsets={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts,
                rule.unsupported_payload_offsets,
            ));
        }
    }

    if let Some(model) = &report.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?} unsupported_offsets={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts,
                rule.unsupported_payload_offsets,
            ));
        }
    }

    lines.join("\n")
}

fn diagnostics_json(report: &DiagnosticsReport) -> String {
    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"fragments\":[{}],",
            "\"program_model\":{},",
            "\"reason_model\":{}",
            "}}"
        ),
        report.template_id,
        report
            .fragments
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        report
            .program_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
        report
            .reason_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
    )
}

fn findings_text(report: &CompilerFindingsReport) -> String {
    if report.findings.is_empty() {
        return "findings=none".into();
    }

    report
        .findings
        .iter()
        .map(finding_text_record)
        .map(|finding| format!("finding {finding}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn findings_json(report: &CompilerFindingsReport) -> String {
    format!(
        "{{\"findings\":[{}]}}",
        report
            .findings
            .iter()
            .map(finding_json_record)
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn envelope_text(report: &CompilerEnvelope) -> String {
    let mut sections = Vec::new();
    sections.push("surface=binding".to_string());
    sections.push(
        report
            .binding
            .as_ref()
            .map_or_else(|| "binding=none".to_string(), binding_text),
    );
    sections.push("surface=diagnostics".to_string());
    sections.push(
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "diagnostics=none".to_string(), diagnostics_text),
    );
    sections.push("surface=findings".to_string());
    sections.push(findings_text(&report.findings));
    sections.push("surface=stages".to_string());
    sections.push(stages_text(&report.stages));
    sections.join("\n")
}

fn envelope_json(report: &CompilerEnvelope) -> String {
    format!(
        "{{\"binding\":{},\"diagnostics\":{},\"findings\":{},\"stages\":{}}}",
        report
            .binding
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
        findings_json(&report.findings),
        stages_json(&report.stages),
    )
}

fn explain_text(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let next_step = explain_next_step_hint(report);
    let mut lines = vec![
        "surface=explain".to_string(),
        format!("ok={}", report.ok),
        format!("parse_ok={}", report.stages.parse.ok),
        format!("validation_ok={}", report.stages.validation.ok),
        format!("diagnostics_ok={}", report.stages.diagnostics.ok),
        format!("next_step={next_step}"),
    ];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", explain_focus_text(focus)));
    }

    if let Some(focus) = focus {
        lines.extend(explain_focus_text_lines(report, focus));
        return lines.join("\n");
    }

    if let Some(excerpt) = &report.parse_source_excerpt {
        lines.push(format!("parse_source_excerpt={}", excerpt.line_text));
        lines.push(format!("parse_source_marker={}", excerpt.marker));
    }

    match &report.binding {
        Some(binding) => {
            lines.push(format!("template={}", binding.template_id));
            if let Some(model) = &binding.program_model {
                lines.push(format!("operation={}", model.operation));
                lines.push(format!("program_rules={}", model.rules));
            } else {
                lines.push("operation=none".into());
            }
            lines.push(format!("fragments={}", binding.fragments.join(",")));
            if let Some(summary) = &report.lowered_binding_summary {
                lines.push(format!(
                    "lowered_binding_summary=fragments:{} window:{} reason:{} program_model:{} program_rules:{} params:{} evidence:{}",
                    summary.fragment_count,
                    summary.has_window,
                    summary.has_reason_profile,
                    summary.has_program_model,
                    summary.program_rule_count,
                    summary.fragment_param_count,
                    summary.evidence_override_count
                ));
            }
            if let Some(delta) = &report.frontend_lowering_delta {
                lines.push(format!(
                    "frontend_lowering_delta=functions:{} merged_steps:{} use_edges:{} includes:{} => fragments:{} program_rules:{} params:{} evidence:{}",
                    delta.frontend_function_count,
                    delta.frontend_merged_step_count,
                    delta.frontend_use_edge_count,
                    delta.frontend_include_source_count,
                    delta.lowered_fragment_count,
                    delta.lowered_program_rule_count,
                    delta.lowered_fragment_param_count,
                    delta.lowered_evidence_override_count
                ));
            }
            if let Some(note) = &report.binding_shape_note {
                lines.push(format!("binding_shape_note={note}"));
            }
        }
        None => {
            lines.push("template=none".into());
            lines.push("operation=none".into());
            lines.push("fragments=none".into());
            lines.push("lowered_binding_summary=none".into());
            lines.push("frontend_lowering_delta=none".into());
            lines.push("binding_shape_note=none".into());
        }
    }

    if let Some(frontend) = &report.frontend {
        lines.push("frontend:".into());
        lines.push(format!("- kind={}", frontend.kind));
        lines.push(format!("- function_count={}", frontend.function_count));
        lines.push(format!(
            "- merged_step_count={}",
            frontend.merged_step_count
        ));
        lines.push(format!(
            "- include_sources={}",
            frontend.include_sources.len()
        ));
        lines.push(format!("- use_edges={}", frontend.use_edges.len()));
        lines.push(format!("- graph_nodes={}", frontend.graph_nodes.len()));
        lines.push(format!("- graph_edges={}", frontend.graph_edges.len()));
    } else {
        lines.push("frontend=none".into());
    }

    lines.push("validation:".into());
    lines.push(format!("- registry={}", report.stages.validation.registry));
    lines.push(format!(
        "- fragments={}",
        report.stages.validation.fragment_count
    ));
    lines.push(format!(
        "- program_rules={}",
        report.stages.validation.program_rule_count
    ));
    lines.push(format!(
        "- reason_rules={}",
        report.stages.validation.reason_rule_count
    ));
    lines.push(format!(
        "- unsupported_payload_offsets={:?}",
        report.stages.validation.unsupported_payload_offsets
    ));
    if let Some(excerpt) = &report.validation_excerpt {
        lines.push(format!(
            "- validation_excerpt=model:{} rule:{} offsets:{:?} supporting_fragments:{}",
            excerpt.model,
            excerpt.rule_index,
            excerpt.unsupported_payload_offsets,
            excerpt.supporting_fragments.join(",")
        ));
    }
    if let Some(note) = &report.validation_shape_note {
        lines.push(format!("- validation_note={note}"));
    }

    match &report.diagnostics {
        Some(diagnostics) => {
            lines.push("diagnostics:".into());
            lines.push(format!("- template={}", diagnostics.template_id));
            lines.push(format!(
                "- program_model_rules={}",
                diagnostics
                    .program_model
                    .as_ref()
                    .map(|model| model.rules.len())
                    .unwrap_or(0)
            ));
            lines.push(format!(
                "- reason_model_rules={}",
                diagnostics
                    .reason_model
                    .as_ref()
                    .map(|model| model.rules.len())
                    .unwrap_or(0)
            ));
            if let Some(excerpt) = &report.diagnostics_excerpt {
                lines.push(format!(
                    "- diagnostics_excerpt=model:{} rule:{} missing_facts:{} offsets:{:?} supporting_fragments:{}",
                    excerpt.model,
                    excerpt.rule_index,
                    excerpt.missing_facts.join(","),
                    excerpt.unsupported_payload_offsets,
                    excerpt.supporting_fragments.join(",")
                ));
            }
            if let Some(note) = &report.diagnostics_shape_note {
                lines.push(format!("- diagnostics_note={note}"));
            }
        }
        None => lines.push("diagnostics=none".into()),
    }

    if report.findings.findings.is_empty() {
        lines.push("findings=none".into());
    } else {
        lines.push("findings:".into());
        lines.extend(
            report
                .findings
                .findings
                .iter()
                .map(|finding| format!("- {}", finding_text_record(finding))),
        );
    }

    lines.join("\n")
}

fn explain_text_compact(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let mut lines = vec![format!(
        "surface=explain ok={} parse_ok={} validation_ok={} diagnostics_ok={} findings={} next_step={}",
        report.ok,
        report.stages.parse.ok,
        report.stages.validation.ok,
        report.stages.diagnostics.ok,
        report.findings.findings.len(),
        explain_next_step_hint(report),
    )];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", explain_focus_text(focus)));
        match focus {
            ExplainFocus::Parse => {
                lines.push(format!(
                    "parse_finding={}",
                    finding_text(report.stages.parse.finding.as_ref())
                ));
            }
            ExplainFocus::Frontend => {
                if let Some(frontend) = &report.frontend {
                    lines.push(format!(
                        "frontend kind={} functions={} includes={} use_edges={} graph_nodes={} graph_edges={}",
                        frontend.kind,
                        frontend.function_count,
                        frontend.include_sources.len(),
                        frontend.use_edges.len(),
                        frontend.graph_nodes.len(),
                        frontend.graph_edges.len()
                    ));
                } else {
                    lines.push("frontend=none".into());
                }
            }
            ExplainFocus::Binding => {
                if let Some(summary) = &report.lowered_binding_summary {
                    lines.push(format!(
                        "binding fragments={} window={} reason={} program_model={} rules={} params={} evidence={}",
                        summary.fragment_count,
                        summary.has_window,
                        summary.has_reason_profile,
                        summary.has_program_model,
                        summary.program_rule_count,
                        summary.fragment_param_count,
                        summary.evidence_override_count
                    ));
                } else {
                    lines.push("binding=none".into());
                }
                if let Some(delta) = &report.frontend_lowering_delta {
                    lines.push(format!(
                        "binding_delta frontend_functions={} frontend_steps={} frontend_use_edges={} frontend_includes={} lowered_fragments={} lowered_rules={} lowered_params={} lowered_evidence={}",
                        delta.frontend_function_count,
                        delta.frontend_merged_step_count,
                        delta.frontend_use_edge_count,
                        delta.frontend_include_source_count,
                        delta.lowered_fragment_count,
                        delta.lowered_program_rule_count,
                        delta.lowered_fragment_param_count,
                        delta.lowered_evidence_override_count
                    ));
                }
                if let Some(note) = &report.binding_shape_note {
                    lines.push(format!("binding_note={note}"));
                }
            }
            ExplainFocus::Validation => {
                lines.push(format!(
                    "validation registry={} unsupported_payload_offsets={:?}",
                    report.stages.validation.registry,
                    report.stages.validation.unsupported_payload_offsets
                ));
                if let Some(note) = &report.validation_shape_note {
                    lines.push(format!("validation_note={note}"));
                }
            }
            ExplainFocus::Diagnostics => {
                lines.push(format!(
                    "diagnostics excerpt={}",
                    report
                        .diagnostics_excerpt
                        .as_ref()
                        .map(|excerpt| format!(
                            "{}#{} missing={} offsets={:?}",
                            excerpt.model,
                            excerpt.rule_index,
                            excerpt.missing_facts.join(","),
                            excerpt.unsupported_payload_offsets
                        ))
                        .unwrap_or_else(|| "none".into())
                ));
                if let Some(note) = &report.diagnostics_shape_note {
                    lines.push(format!("diagnostics_note={note}"));
                }
            }
            ExplainFocus::Findings => {
                lines.push(format!("findings={}", report.findings.findings.len()));
            }
        }
        return lines.join("\n");
    }

    lines.push(format!(
        "template={} operation={} fragments={}",
        report
            .binding
            .as_ref()
            .map(|binding| binding.template_id.as_str())
            .unwrap_or("none"),
        report
            .binding
            .as_ref()
            .and_then(|binding| binding.program_model.as_ref())
            .map(|model| model.operation.as_str())
            .unwrap_or("none"),
        report
            .binding
            .as_ref()
            .map(|binding| binding.fragments.len().to_string())
            .unwrap_or_else(|| "0".into())
    ));
    if let Some(summary) = &report.lowered_binding_summary {
        lines.push(format!(
            "lowered=window:{} reason:{} program_model:{} rules:{} params:{} evidence:{}",
            summary.has_window,
            summary.has_reason_profile,
            summary.has_program_model,
            summary.program_rule_count,
            summary.fragment_param_count,
            summary.evidence_override_count
        ));
    }
    if let Some(delta) = &report.frontend_lowering_delta {
        lines.push(format!(
            "delta=frontend_functions:{} frontend_steps:{} lowered_fragments:{} lowered_rules:{}",
            delta.frontend_function_count,
            delta.frontend_merged_step_count,
            delta.lowered_fragment_count,
            delta.lowered_program_rule_count
        ));
    }
    if let Some(note) = &report.binding_shape_note {
        lines.push(format!("binding_note={note}"));
    }
    if let Some(excerpt) = &report.parse_source_excerpt {
        lines.push(format!(
            "parse_source={} {}",
            excerpt.line_text, excerpt.marker
        ));
    }
    if let Some(excerpt) = &report.validation_excerpt {
        lines.push(format!(
            "validation_excerpt={}#{} offsets={:?}",
            excerpt.model, excerpt.rule_index, excerpt.unsupported_payload_offsets
        ));
    }
    if let Some(note) = &report.validation_shape_note {
        lines.push(format!("validation_note={note}"));
    }
    if let Some(excerpt) = &report.diagnostics_excerpt {
        lines.push(format!(
            "diagnostics_excerpt={}#{} missing={} offsets={:?}",
            excerpt.model,
            excerpt.rule_index,
            excerpt.missing_facts.join(","),
            excerpt.unsupported_payload_offsets
        ));
    }
    if let Some(note) = &report.diagnostics_shape_note {
        lines.push(format!("diagnostics_note={note}"));
    }
    lines.join("\n")
}

fn explain_json(report: &ExplainReport, focus: Option<ExplainFocus>) -> String {
    let next_step = explain_next_step_hint(report);
    let template_id = report
        .binding
        .as_ref()
        .map(|binding| format!("\"{}\"", binding.template_id))
        .unwrap_or_else(|| "null".into());
    let operation = report
        .binding
        .as_ref()
        .and_then(|binding| binding.program_model.as_ref())
        .map(|model| format!("\"{}\"", model.operation))
        .unwrap_or_else(|| "null".into());
    let focus_json = focus
        .map(|focus| format!("\"{}\"", explain_focus_text(focus)))
        .unwrap_or_else(|| "null".into());
    let focused_report_json = focus
        .map(|focus| explain_focus_json(report, focus))
        .unwrap_or_else(|| "null".into());
    let parse_source_excerpt_json = report
        .parse_source_excerpt
        .as_ref()
        .map(source_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let validation_excerpt_json = report
        .validation_excerpt
        .as_ref()
        .map(validation_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let diagnostics_excerpt_json = report
        .diagnostics_excerpt
        .as_ref()
        .map(diagnostics_excerpt_json)
        .unwrap_or_else(|| "null".into());
    let lowered_binding_summary_json = report
        .lowered_binding_summary
        .as_ref()
        .map(lowered_binding_summary_json)
        .unwrap_or_else(|| "null".into());
    let frontend_lowering_delta_json = report
        .frontend_lowering_delta
        .as_ref()
        .map(frontend_lowering_delta_json)
        .unwrap_or_else(|| "null".into());
    let binding_shape_note_json = report
        .binding_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    let validation_shape_note_json = report
        .validation_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    let diagnostics_shape_note_json = report
        .diagnostics_shape_note
        .as_ref()
        .map(|note| format!("\"{}\"", json_escape_string(note)))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"ok\":{},\"summary\":{{\"parse_ok\":{},\"validation_ok\":{},\"diagnostics_ok\":{},\"template_id\":{},\"operation\":{},\"finding_count\":{},\"next_step\":\"{}\",\"focus\":{},\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{},\"binding_shape_note\":{},\"validation_shape_note\":{},\"diagnostics_shape_note\":{},\"parse_source_excerpt\":{},\"validation_excerpt\":{},\"diagnostics_excerpt\":{}}},\"focused_report\":{},\"frontend\":{},\"binding\":{},\"validation\":{},\"diagnostics\":{},\"findings\":{}}}",
        report.ok,
        report.stages.parse.ok,
        report.stages.validation.ok,
        report.stages.diagnostics.ok,
        template_id,
        operation,
        report.findings.findings.len(),
        next_step,
        focus_json,
        lowered_binding_summary_json,
        frontend_lowering_delta_json,
        binding_shape_note_json,
        validation_shape_note_json,
        diagnostics_shape_note_json,
        parse_source_excerpt_json,
        validation_excerpt_json,
        diagnostics_excerpt_json,
        focused_report_json,
        frontend_json(report.frontend.as_ref()),
        report
            .binding
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        stages_validation_json(&report.stages.validation),
        report
            .diagnostics
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
        findings_json(&report.findings),
    )
}

fn stages_validation_json(report: &ValidationReport) -> String {
    format!(
        "{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}],\"sampled_payload_offsets\":[{}],\"required_payload_offsets\":[{}],\"unsupported_payload_offsets\":[{}],\"finding\":{}}}",
        report.ok,
        report.registry,
        report.fragment_count,
        report.program_rule_count,
        report.reason_rule_count,
        string_json_list(&report.checks),
        u16_json_list(&report.sampled_payload_offsets),
        u16_json_list(&report.required_payload_offsets),
        u16_json_list(&report.unsupported_payload_offsets),
        finding_json(report.finding.as_ref()),
    )
}

fn explain_report(envelope: CompilerEnvelope, source: Option<&str>) -> ExplainReport {
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
        lowered_binding_summary,
        frontend_lowering_delta,
        binding_shape_note,
        validation_shape_note,
        diagnostics_shape_note,
        parse_source_excerpt,
        validation_excerpt,
        diagnostics_excerpt,
    }
}

fn explain_focus_text(focus: ExplainFocus) -> &'static str {
    match focus {
        ExplainFocus::Parse => "parse",
        ExplainFocus::Frontend => "frontend",
        ExplainFocus::Binding => "binding",
        ExplainFocus::Validation => "validation",
        ExplainFocus::Diagnostics => "diagnostics",
        ExplainFocus::Findings => "findings",
    }
}

fn explain_focus_text_lines(report: &ExplainReport, focus: ExplainFocus) -> Vec<String> {
    match focus {
        ExplainFocus::Parse => {
            let mut lines = vec![
                format!("parse_ok={}", report.stages.parse.ok),
                format!(
                    "parse_finding={}",
                    finding_text(report.stages.parse.finding.as_ref())
                ),
            ];
            if let Some(excerpt) = &report.parse_source_excerpt {
                lines.push(format!("parse_source_excerpt={}", excerpt.line_text));
                lines.push(format!("parse_source_marker={}", excerpt.marker));
            }
            lines
        }
        ExplainFocus::Frontend => match &report.frontend {
            Some(frontend) => {
                let mut lines = vec!["frontend:".to_string()];
                lines.extend(
                    frontend_report_text(frontend, None)
                        .lines()
                        .map(|line| line.to_string()),
                );
                lines
            }
            None => vec!["frontend=none".into()],
        },
        ExplainFocus::Binding => {
            let mut lines = vec![format!("binding_present={}", report.binding.is_some())];
            if let Some(summary) = &report.lowered_binding_summary {
                lines.push(format!(
                    "lowered_binding_summary=fragments:{} window:{} reason:{} program_model:{} program_rules:{} params:{} evidence:{}",
                    summary.fragment_count,
                    summary.has_window,
                    summary.has_reason_profile,
                    summary.has_program_model,
                    summary.program_rule_count,
                    summary.fragment_param_count,
                    summary.evidence_override_count
                ));
            } else {
                lines.push("lowered_binding_summary=none".into());
            }
            if let Some(delta) = &report.frontend_lowering_delta {
                lines.push(format!(
                    "binding_delta frontend_functions={} frontend_steps={} frontend_use_edges={} frontend_includes={} lowered_fragments={} lowered_rules={} lowered_params={} lowered_evidence={}",
                    delta.frontend_function_count,
                    delta.frontend_merged_step_count,
                    delta.frontend_use_edge_count,
                    delta.frontend_include_source_count,
                    delta.lowered_fragment_count,
                    delta.lowered_program_rule_count,
                    delta.lowered_fragment_param_count,
                    delta.lowered_evidence_override_count
                ));
            }
            if let Some(note) = &report.binding_shape_note {
                lines.push(format!("binding_note={note}"));
            }
            if let Some(binding) = &report.binding {
                lines.extend(binding_text(binding).lines().map(|line| line.to_string()));
            } else {
                lines.push("binding=none".into());
            }
            lines
        }
        ExplainFocus::Validation => {
            vec![
            format!("validation_ok={}", report.stages.validation.ok),
            format!("registry={}", report.stages.validation.registry),
            format!("checks={}", report.stages.validation.checks.join(",")),
            format!(
                "unsupported_payload_offsets={:?}",
                report.stages.validation.unsupported_payload_offsets
            ),
            report
                .validation_excerpt
                .as_ref()
                .map(|excerpt| format!(
                    "validation_excerpt=model:{} rule:{} offsets:{:?} supporting_fragments:{}",
                    excerpt.model,
                    excerpt.rule_index,
                    excerpt.unsupported_payload_offsets,
                    excerpt.supporting_fragments.join(",")
                ))
                .unwrap_or_else(|| "validation_excerpt=none".into()),
            report
                .validation_shape_note
                .as_ref()
                .map(|note| format!("validation_note={note}"))
                .unwrap_or_else(|| "validation_note=none".into()),
            format!(
                "validation_finding={}",
                finding_text(report.stages.validation.finding.as_ref())
            ),
        ]
        }
        ExplainFocus::Diagnostics => match &report.diagnostics {
            Some(diagnostics) => {
                let mut lines = vec![format!("diagnostics_ok={}", report.stages.diagnostics.ok)];
                lines.push(
                    report
                        .diagnostics_excerpt
                        .as_ref()
                        .map(|excerpt| format!(
                            "diagnostics_excerpt=model:{} rule:{} missing_facts:{} offsets:{:?} supporting_fragments:{}",
                            excerpt.model,
                            excerpt.rule_index,
                            excerpt.missing_facts.join(","),
                            excerpt.unsupported_payload_offsets,
                            excerpt.supporting_fragments.join(",")
                        ))
                        .unwrap_or_else(|| "diagnostics_excerpt=none".into()),
                );
                lines.push(
                    report
                        .diagnostics_shape_note
                        .as_ref()
                        .map(|note| format!("diagnostics_note={note}"))
                        .unwrap_or_else(|| "diagnostics_note=none".into()),
                );
                lines.extend(
                    diagnostics_text(diagnostics)
                        .lines()
                        .map(|line| line.to_string()),
                );
                lines
            }
            None => vec![
                format!("diagnostics_ok={}", report.stages.diagnostics.ok),
                "diagnostics=none".into(),
            ],
        },
        ExplainFocus::Findings => {
            let mut lines = vec![format!("finding_count={}", report.findings.findings.len())];
            if report.findings.findings.is_empty() {
                lines.push("findings=none".into());
            } else {
                lines.push("findings:".into());
                lines.extend(
                    report
                        .findings
                        .findings
                        .iter()
                        .map(|finding| format!("- {}", finding_text_record(finding))),
                );
            }
            lines
        }
    }
}

fn explain_focus_json(report: &ExplainReport, focus: ExplainFocus) -> String {
    match focus {
        ExplainFocus::Parse => format!(
            "{{\"kind\":\"parse\",\"ok\":{},\"finding\":{},\"source_excerpt\":{}}}",
            report.stages.parse.ok,
            finding_json(report.stages.parse.finding.as_ref()),
            report
                .parse_source_excerpt
                .as_ref()
                .map(source_excerpt_json)
                .unwrap_or_else(|| "null".to_string())
        ),
        ExplainFocus::Frontend => format!(
            "{{\"kind\":\"frontend\",\"report\":{}}}",
            frontend_json(report.frontend.as_ref())
        ),
        ExplainFocus::Binding => format!(
            "{{\"kind\":\"binding\",\"lowered_binding_summary\":{},\"frontend_lowering_delta\":{},\"binding_shape_note\":{},\"report\":{}}}",
            report
                .lowered_binding_summary
                .as_ref()
                .map(lowered_binding_summary_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .frontend_lowering_delta
                .as_ref()
                .map(frontend_lowering_delta_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .binding_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string()),
            report
                .binding
                .as_ref()
                .map_or_else(|| "null".to_string(), binding_json)
        ),
        ExplainFocus::Validation => format!(
            "{{\"kind\":\"validation\",\"report\":{},\"validation_excerpt\":{},\"validation_shape_note\":{}}}",
            stages_validation_json(&report.stages.validation),
            report
                .validation_excerpt
                .as_ref()
                .map(validation_excerpt_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .validation_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string())
        ),
        ExplainFocus::Diagnostics => format!(
            "{{\"kind\":\"diagnostics\",\"ok\":{},\"diagnostics_excerpt\":{},\"diagnostics_shape_note\":{},\"report\":{}}}",
            report.stages.diagnostics.ok,
            report
                .diagnostics_excerpt
                .as_ref()
                .map(diagnostics_excerpt_json)
                .unwrap_or_else(|| "null".to_string()),
            report
                .diagnostics_shape_note
                .as_ref()
                .map(|note| format!("\"{}\"", json_escape_string(note)))
                .unwrap_or_else(|| "null".to_string()),
            report
                .diagnostics
                .as_ref()
                .map_or_else(|| "null".to_string(), diagnostics_json)
        ),
        ExplainFocus::Findings => format!(
            "{{\"kind\":\"findings\",\"report\":{}}}",
            findings_json(&report.findings)
        ),
    }
}

fn lowered_binding_summary_from_binding(binding: &BindingReport) -> LoweredBindingSummary {
    LoweredBindingSummary {
        fragment_count: binding.fragments.len(),
        has_window: binding.window.is_some(),
        has_reason_profile: binding.reason_profile.is_some(),
        has_program_model: binding.program_model.is_some(),
        program_rule_count: binding
            .program_model
            .as_ref()
            .map(|model| model.rules)
            .unwrap_or(0),
        fragment_param_count: binding.fragment_params.len(),
        evidence_override_count: binding.evidence_overrides.len(),
    }
}

fn lowered_binding_summary_json(summary: &LoweredBindingSummary) -> String {
    format!(
        "{{\"fragment_count\":{},\"has_window\":{},\"has_reason_profile\":{},\"has_program_model\":{},\"program_rule_count\":{},\"fragment_param_count\":{},\"evidence_override_count\":{}}}",
        summary.fragment_count,
        summary.has_window,
        summary.has_reason_profile,
        summary.has_program_model,
        summary.program_rule_count,
        summary.fragment_param_count,
        summary.evidence_override_count
    )
}

fn frontend_lowering_delta(
    frontend: &FrontendReport,
    lowered: &LoweredBindingSummary,
) -> FrontendLoweringDelta {
    FrontendLoweringDelta {
        frontend_function_count: frontend.function_count,
        frontend_merged_step_count: frontend.merged_step_count,
        frontend_use_edge_count: frontend.use_edges.len(),
        frontend_include_source_count: frontend.include_sources.len(),
        lowered_fragment_count: lowered.fragment_count,
        lowered_program_rule_count: lowered.program_rule_count,
        lowered_fragment_param_count: lowered.fragment_param_count,
        lowered_evidence_override_count: lowered.evidence_override_count,
    }
}

fn frontend_lowering_delta_json(delta: &FrontendLoweringDelta) -> String {
    format!(
        "{{\"frontend_function_count\":{},\"frontend_merged_step_count\":{},\"frontend_use_edge_count\":{},\"frontend_include_source_count\":{},\"lowered_fragment_count\":{},\"lowered_program_rule_count\":{},\"lowered_fragment_param_count\":{},\"lowered_evidence_override_count\":{}}}",
        delta.frontend_function_count,
        delta.frontend_merged_step_count,
        delta.frontend_use_edge_count,
        delta.frontend_include_source_count,
        delta.lowered_fragment_count,
        delta.lowered_program_rule_count,
        delta.lowered_fragment_param_count,
        delta.lowered_evidence_override_count
    )
}

fn binding_shape_note_from_delta(delta: &FrontendLoweringDelta) -> String {
    let mut reasons = Vec::new();
    if delta.frontend_use_edge_count > 0 {
        reasons.push("use(...) edges inline reusable function bodies into one binding");
    }
    if delta.frontend_include_source_count > 0 {
        reasons.push("include(...) pulls filesystem-backed modules into the same compiled entry");
    }
    if delta.lowered_program_rule_count > 0 {
        reasons.push("program_rule(...) calls lower into explicit program-model rules");
    }
    if delta.lowered_fragment_param_count > 0 || delta.lowered_evidence_override_count > 0 {
        reasons.push("param(...) and evidence(...) survive as binding-level overrides");
    }

    if reasons.is_empty() {
        "frontend and lowered binding are close in shape; there are no extra use/include or override layers to explain".into()
    } else {
        format!(
            "lowered binding looks different because {}",
            reasons.join("; ")
        )
    }
}

fn validation_shape_note_from_excerpt(excerpt: &ValidationExcerpt) -> String {
    let offset_note = if excerpt.unsupported_payload_offsets.is_empty() {
        "rule support failed without explicit payload offsets".to_string()
    } else {
        format!(
            "the first failing rule asks for payload offsets {:?} that current fragment coverage does not sample",
            excerpt.unsupported_payload_offsets
        )
    };
    if excerpt.supporting_fragments.is_empty() {
        offset_note
    } else {
        format!(
            "{}; current support comes from fragments [{}]",
            offset_note,
            excerpt.supporting_fragments.join(", ")
        )
    }
}

fn diagnostics_shape_note_from_excerpt(excerpt: &DiagnosticsExcerpt) -> String {
    let mut reasons = Vec::new();
    if !excerpt.missing_facts.is_empty() {
        reasons.push(format!(
            "the first unsupported rule still misses facts [{}]",
            excerpt.missing_facts.join(", ")
        ));
    }
    if !excerpt.unsupported_payload_offsets.is_empty() {
        reasons.push(format!(
            "it also references unsampled payload offsets {:?}",
            excerpt.unsupported_payload_offsets
        ));
    }
    if !excerpt.supporting_fragments.is_empty() {
        reasons.push(format!(
            "current support comes from fragments [{}]",
            excerpt.supporting_fragments.join(", ")
        ));
    }
    if reasons.is_empty() {
        "the first unsupported rule still lacks enough fragment-backed evidence to be supported"
            .into()
    } else {
        reasons.join("; ")
    }
}

fn explain_next_step_hint(report: &ExplainReport) -> &'static str {
    if !report.stages.parse.ok {
        if report.frontend.is_some() {
            return "fix the parse finding first, then inspect the standalone frontend graph with `gewyc frontend`";
        }
        return "fix the parse finding first, then rerun `gewyc explain`";
    }

    if !report.stages.validation.ok {
        if !report
            .stages
            .validation
            .unsupported_payload_offsets
            .is_empty()
        {
            return "inspect `unsupported_payload_offsets` and adjust fragment coverage or payload matchers before rerunning";
        }
        return "inspect the validation section and binding fragments before rerunning";
    }

    if !report.stages.diagnostics.ok {
        return "inspect the diagnostics section and rule support details before rerunning";
    }

    if !report.findings.findings.is_empty() {
        return "inspect the findings list first, then drill into `frontend` or `stages` for the failing phase";
    }

    "binding, frontend, validation, and diagnostics are all healthy; continue with runtime/demo verification"
}

fn source_excerpt_for_finding(source: &str, finding: &CompilerFinding) -> Option<SourceExcerpt> {
    let line = finding.line?;
    let line_text = source.lines().nth(line.saturating_sub(1))?.to_string();
    let marker_column = finding.column.unwrap_or(1).max(1);
    let marker = format!("{}^", " ".repeat(marker_column.saturating_sub(1)));
    Some(SourceExcerpt {
        line,
        column: finding.column,
        line_text,
        marker,
    })
}

fn source_excerpt_json(excerpt: &SourceExcerpt) -> String {
    format!(
        "{{\"line\":{},\"column\":{},\"line_text\":\"{}\",\"marker\":\"{}\"}}",
        excerpt.line,
        excerpt
            .column
            .map(|column| column.to_string())
            .unwrap_or_else(|| "null".to_string()),
        json_escape_string(&excerpt.line_text),
        json_escape_string(&excerpt.marker),
    )
}

fn validation_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<ValidationExcerpt> {
    diagnostics
        .program_model
        .as_ref()
        .into_iter()
        .chain(diagnostics.reason_model.as_ref())
        .find_map(|model| {
            model.rules.iter().find_map(|rule| {
                if rule.unsupported_payload_offsets.is_empty() {
                    None
                } else {
                    Some(ValidationExcerpt {
                        model: model.model.clone(),
                        rule_index: rule.rule_index,
                        unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
                        supporting_fragments: rule.supporting_fragments.clone(),
                    })
                }
            })
        })
}

fn validation_excerpt_json(excerpt: &ValidationExcerpt) -> String {
    format!(
        "{{\"model\":\"{}\",\"rule_index\":{},\"unsupported_payload_offsets\":[{}],\"supporting_fragments\":[{}]}}",
        json_escape_string(&excerpt.model),
        excerpt.rule_index,
        u16_json_list(&excerpt.unsupported_payload_offsets),
        string_json_list(&excerpt.supporting_fragments),
    )
}

fn diagnostics_excerpt_from_diagnostics(
    diagnostics: &DiagnosticsReport,
) -> Option<DiagnosticsExcerpt> {
    diagnostics
        .program_model
        .as_ref()
        .into_iter()
        .chain(diagnostics.reason_model.as_ref())
        .find_map(|model| {
            model.rules.iter().find_map(|rule| {
                if rule.supported {
                    None
                } else {
                    Some(DiagnosticsExcerpt {
                        model: model.model.clone(),
                        rule_index: rule.rule_index,
                        missing_facts: rule.missing_facts.clone(),
                        unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
                        supporting_fragments: rule.supporting_fragments.clone(),
                    })
                }
            })
        })
}

fn diagnostics_excerpt_json(excerpt: &DiagnosticsExcerpt) -> String {
    format!(
        "{{\"model\":\"{}\",\"rule_index\":{},\"missing_facts\":[{}],\"unsupported_payload_offsets\":[{}],\"supporting_fragments\":[{}]}}",
        json_escape_string(&excerpt.model),
        excerpt.rule_index,
        string_json_list(&excerpt.missing_facts),
        u16_json_list(&excerpt.unsupported_payload_offsets),
        string_json_list(&excerpt.supporting_fragments),
    )
}

fn json_escape_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn stages_text(report: &CompilerStagesReport) -> String {
    format!(
        "stage=parse\nok={}\nfrontend={}\nparse_finding={}\n{}\nstage=validation\nok={}\nregistry={}\nfragments={}\nprogram_rules={}\nreason_rules={}\nchecks={}\nsampled_payload_offsets={:?}\nrequired_payload_offsets={:?}\nunsupported_payload_offsets={:?}\nvalidation_finding={}\nstage=diagnostics\nok={}\ndiagnostics_finding={}\n{}",
        report.parse.ok,
        frontend_text(report.parse.frontend.as_ref()),
        finding_text(report.parse.finding.as_ref()),
        report
            .parse
            .report
            .as_ref()
            .map_or_else(String::new, binding_text),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        report.validation.checks.join(","),
        report.validation.sampled_payload_offsets,
        report.validation.required_payload_offsets,
        report.validation.unsupported_payload_offsets,
        finding_text(report.validation.finding.as_ref()),
        report.diagnostics.ok,
        finding_text(report.diagnostics.finding.as_ref()),
        report
            .diagnostics
            .report
            .as_ref()
            .map_or_else(String::new, diagnostics_text)
    )
}

fn stages_json(report: &CompilerStagesReport) -> String {
    format!(
        "{{\"parse\":{{\"ok\":{},\"frontend\":{},\"finding\":{},\"report\":{}}},\"validation\":{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}],\"sampled_payload_offsets\":[{}],\"required_payload_offsets\":[{}],\"unsupported_payload_offsets\":[{}],\"finding\":{}}},\"diagnostics\":{{\"ok\":{},\"finding\":{},\"report\":{}}}}}",
        report.parse.ok,
        frontend_json(report.parse.frontend.as_ref()),
        finding_json(report.parse.finding.as_ref()),
        report
            .parse
            .report
            .as_ref()
            .map_or_else(|| "null".to_string(), binding_json),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        string_json_list(&report.validation.checks),
        u16_json_list(&report.validation.sampled_payload_offsets),
        u16_json_list(&report.validation.required_payload_offsets),
        u16_json_list(&report.validation.unsupported_payload_offsets),
        finding_json(report.validation.finding.as_ref()),
        report.diagnostics.ok,
        finding_json(report.diagnostics.finding.as_ref()),
        report
            .diagnostics
            .report
            .as_ref()
            .map_or_else(|| "null".to_string(), diagnostics_json),
    )
}

fn model_diagnostics_json(model: &ModelDiagnosticsReport) -> String {
    format!(
        "{{\"model\":\"{}\",\"rules\":[{}]}}",
        model.model,
        model
            .rules
            .iter()
            .map(|rule| format!(
                "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":[{}],\"supporting_fragments\":[{}],\"missing_facts\":[{}],\"unsupported_payload_offsets\":[{}]}}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                string_json_list(&rule.required_facts),
                string_json_list(&rule.supporting_fragments),
                string_json_list(&rule.missing_facts),
                rule
                    .unsupported_payload_offsets
                    .iter()
                    .map(|offset| offset.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn string_json_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn u16_json_list(items: &[u16]) -> String {
    items
        .iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

fn frontend_report(summary: FrontendModuleSummary) -> FrontendReport {
    FrontendReport {
        kind: frontend_kind_text(summary.kind).to_string(),
        function_count: summary.function_count,
        function_nodes: summary
            .function_nodes
            .into_iter()
            .map(frontend_function_report)
            .collect(),
        merged_step_count: summary.merged_step_count,
        include_sources: summary.include_sources,
        use_edges: summary
            .use_edges
            .into_iter()
            .map(frontend_use_edge_report)
            .collect(),
        graph_nodes: summary
            .graph_nodes
            .into_iter()
            .map(frontend_graph_node_report)
            .collect(),
        graph_edges: summary
            .graph_edges
            .into_iter()
            .map(frontend_graph_edge_report)
            .collect(),
        expansion_previews: summary
            .expansion_previews
            .into_iter()
            .map(frontend_expansion_preview_report)
            .collect(),
    }
}

fn frontend_function_report(node: FrontendFunctionNode) -> FrontendFunctionReport {
    FrontendFunctionReport {
        name: node.name,
        step_count: node.step_count,
    }
}

fn frontend_use_edge_report(edge: FrontendUseEdge) -> FrontendUseEdgeReport {
    FrontendUseEdgeReport {
        from: edge.from,
        to: edge.to,
        line: edge.line,
    }
}

fn frontend_graph_node_report(node: FrontendGraphNode) -> FrontendGraphNodeReport {
    FrontendGraphNodeReport {
        id: node.id,
        kind: frontend_graph_node_kind_text(node.kind).to_string(),
        step_count: node.step_count,
    }
}

fn frontend_graph_edge_report(edge: FrontendGraphEdge) -> FrontendGraphEdgeReport {
    FrontendGraphEdgeReport {
        from: edge.from,
        to: edge.to,
        kind: frontend_graph_edge_kind_text(edge.kind).to_string(),
        line: edge.line,
    }
}

fn frontend_expansion_preview_report(
    preview: FrontendExpansionPreview,
) -> FrontendExpansionPreviewReport {
    FrontendExpansionPreviewReport {
        scope: preview.scope,
        local_bindings: preview.local_bindings,
        steps: preview.steps,
        use_targets: preview.use_targets,
    }
}

fn frontend_kind_text(kind: FrontendDslKind) -> &'static str {
    match kind {
        FrontendDslKind::Pipeline => "pipeline",
    }
}

fn frontend_graph_node_kind_text(kind: FrontendGraphNodeKind) -> &'static str {
    match kind {
        FrontendGraphNodeKind::Entry => "entry",
        FrontendGraphNodeKind::File => "file",
        FrontendGraphNodeKind::Function => "function",
    }
}

fn frontend_graph_edge_kind_text(kind: FrontendGraphEdgeKind) -> &'static str {
    match kind {
        FrontendGraphEdgeKind::Include => "include",
        FrontendGraphEdgeKind::Use => "use",
    }
}

fn frontend_text(frontend: Option<&FrontendReport>) -> String {
    match frontend {
        Some(frontend) => format!(
            "kind={} functions={} function_nodes={} merged_steps={} include_sources={} use_edges={} expansions={} graph_nodes={} graph_edges={}",
            frontend.kind,
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!("{}:{}", node.name, node.step_count))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            frontend.include_sources.join(","),
            frontend
                .use_edges
                .iter()
                .map(|edge| format!("{}->{}@{}", edge.from, edge.to, edge.line))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .expansion_previews
                .iter()
                .map(|preview| format!("{}:{}", preview.scope, preview.steps.join(" -> ")))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_nodes
                .iter()
                .map(|node| match node.step_count {
                    Some(step_count) => format!("{}:{}:{}", node.id, node.kind, step_count),
                    None => format!("{}:{}", node.id, node.kind),
                })
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_edges
                .iter()
                .map(|edge| format!("{}-{}->{}@{}", edge.from, edge.kind, edge.to, edge.line))
                .collect::<Vec<_>>()
                .join(",")
        ),
        None => "none".into(),
    }
}

fn frontend_report_text(report: &FrontendReport, focus: Option<FrontendFocus>) -> String {
    let mut lines = vec![
        format!("kind={}", report.kind),
        format!("function_count={}", report.function_count),
        format!("merged_step_count={}", report.merged_step_count),
    ];

    if let Some(focus) = focus {
        lines.push(format!("focus={}", frontend_focus_text(focus)));
        lines.extend(frontend_focus_text_lines(report, focus));
        return lines.join("\n");
    }

    if report.include_sources.is_empty() {
        lines.push("include_sources=none".into());
    } else {
        lines.push("include_sources:".into());
        lines.extend(
            report
                .include_sources
                .iter()
                .map(|source| format!("- {source}")),
        );
    }

    if report.function_nodes.is_empty() {
        lines.push("function_nodes=none".into());
    } else {
        lines.push("function_nodes:".into());
        lines.extend(
            report
                .function_nodes
                .iter()
                .map(|node| format!("- {} (steps={})", node.name, node.step_count)),
        );
    }

    if report.use_edges.is_empty() {
        lines.push("use_edges=none".into());
    } else {
        lines.push("use_edges:".into());
        lines.extend(
            report
                .use_edges
                .iter()
                .map(|edge| format!("- {} -> {} @ line {}", edge.from, edge.to, edge.line)),
        );
    }

    if report.expansion_previews.is_empty() {
        lines.push("expansion_previews=none".into());
    } else {
        lines.push("expansion_previews:".into());
        lines.extend(
            report
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_text),
        );
    }

    if report.graph_nodes.is_empty() {
        lines.push("graph_nodes=none".into());
    } else {
        lines.push("graph_nodes:".into());
        lines.extend(report.graph_nodes.iter().map(|node| match node.step_count {
            Some(step_count) => format!("- {} [{}] steps={}", node.id, node.kind, step_count),
            None => format!("- {} [{}]", node.id, node.kind),
        }));
    }

    if report.graph_edges.is_empty() {
        lines.push("graph_edges=none".into());
    } else {
        lines.push("graph_edges:".into());
        lines.extend(report.graph_edges.iter().map(|edge| {
            format!(
                "- {} -{}-> {} @ line {}",
                edge.from, edge.kind, edge.to, edge.line
            )
        }));
    }

    lines.join("\n")
}

fn frontend_report_text_compact(report: &FrontendReport, focus: Option<FrontendFocus>) -> String {
    let mut lines = vec![format!(
        "kind={} function_count={} merged_step_count={}",
        report.kind, report.function_count, report.merged_step_count
    )];
    if let Some(focus) = focus {
        lines.push(format!("focus={}", frontend_focus_text(focus)));
        match focus {
            FrontendFocus::Functions => lines.push(format!(
                "functions={}",
                if report.function_nodes.is_empty() {
                    "none".into()
                } else {
                    report
                        .function_nodes
                        .iter()
                        .map(|node| format!("{}:{}", node.name, node.step_count))
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
            FrontendFocus::Includes => lines.push(format!(
                "include_sources={}",
                if report.include_sources.is_empty() {
                    "none".into()
                } else {
                    report.include_sources.join(",")
                }
            )),
            FrontendFocus::Graph => lines.push(format!(
                "graph_nodes={} graph_edges={}",
                report.graph_nodes.len(),
                report.graph_edges.len()
            )),
            FrontendFocus::Expansion => lines.push(format!(
                "expansion_previews={}",
                if report.expansion_previews.is_empty() {
                    "none".into()
                } else {
                    report
                        .expansion_previews
                        .iter()
                        .map(|preview| format!("{}:{}", preview.scope, preview.steps.len()))
                        .collect::<Vec<_>>()
                        .join(",")
                }
            )),
        }
        return lines.join("\n");
    }
    lines.push(format!(
        "includes={} use_edges={} expansion_previews={} graph_nodes={} graph_edges={}",
        report.include_sources.len(),
        report.use_edges.len(),
        report.expansion_previews.len(),
        report.graph_nodes.len(),
        report.graph_edges.len()
    ));
    lines.join("\n")
}

fn frontend_report_json(report: &FrontendReport, focus: Option<FrontendFocus>) -> String {
    let focus_json = focus
        .map(|focus| format!("\"{}\"", frontend_focus_text(focus)))
        .unwrap_or_else(|| "null".into());
    let focused_report_json = focus
        .map(|focus| frontend_focus_json(report, focus))
        .unwrap_or_else(|| "null".into());
    format!(
        "{{\"summary\":{{\"kind\":\"{}\",\"function_count\":{},\"merged_step_count\":{},\"focus\":{}}},\"focused_report\":{},\"report\":{}}}",
        report.kind,
        report.function_count,
        report.merged_step_count,
        focus_json,
        focused_report_json,
        frontend_json(Some(report)),
    )
}

fn frontend_focus_text(focus: FrontendFocus) -> &'static str {
    match focus {
        FrontendFocus::Functions => "functions",
        FrontendFocus::Includes => "includes",
        FrontendFocus::Graph => "graph",
        FrontendFocus::Expansion => "expansion",
    }
}

fn frontend_focus_text_lines(report: &FrontendReport, focus: FrontendFocus) -> Vec<String> {
    match focus {
        FrontendFocus::Functions => {
            let mut lines = vec!["function_nodes:".into()];
            if report.function_nodes.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(
                    report
                        .function_nodes
                        .iter()
                        .map(|node| format!("- {} (steps={})", node.name, node.step_count)),
                );
            }
            lines
        }
        FrontendFocus::Includes => {
            let mut lines = vec!["include_sources:".into()];
            if report.include_sources.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(
                    report
                        .include_sources
                        .iter()
                        .map(|source| format!("- {source}")),
                );
            }
            lines
        }
        FrontendFocus::Graph => {
            let mut lines = vec!["graph_nodes:".into()];
            if report.graph_nodes.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(report.graph_nodes.iter().map(|node| match node.step_count {
                    Some(step_count) => {
                        format!("- {} [{}] steps={}", node.id, node.kind, step_count)
                    }
                    None => format!("- {} [{}]", node.id, node.kind),
                }));
            }
            lines.push("graph_edges:".into());
            if report.graph_edges.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(report.graph_edges.iter().map(|edge| {
                    format!(
                        "- {} -{}-> {} @ line {}",
                        edge.from, edge.kind, edge.to, edge.line
                    )
                }));
            }
            lines
        }
        FrontendFocus::Expansion => {
            let mut lines = vec!["expansion_previews:".into()];
            if report.expansion_previews.is_empty() {
                lines.push("- none".into());
            } else {
                lines.extend(
                    report
                        .expansion_previews
                        .iter()
                        .map(frontend_expansion_preview_text),
                );
            }
            lines
        }
    }
}

fn frontend_focus_json(report: &FrontendReport, focus: FrontendFocus) -> String {
    match focus {
        FrontendFocus::Functions => format!(
            "{{\"kind\":\"functions\",\"function_nodes\":[{}]}}",
            report
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":\"{}\",\"step_count\":{}}}",
                    node.name, node.step_count
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Includes => format!(
            "{{\"kind\":\"includes\",\"include_sources\":[{}]}}",
            string_json_list(&report.include_sources)
        ),
        FrontendFocus::Graph => format!(
            "{{\"kind\":\"graph\",\"graph_nodes\":[{}],\"graph_edges\":[{}]}}",
            report
                .graph_nodes
                .iter()
                .map(|node| match node.step_count {
                    Some(step_count) => format!(
                        "{{\"id\":\"{}\",\"kind\":\"{}\",\"step_count\":{}}}",
                        node.id, node.kind, step_count
                    ),
                    None => format!(
                        "{{\"id\":\"{}\",\"kind\":\"{}\",\"step_count\":null}}",
                        node.id, node.kind
                    ),
                })
                .collect::<Vec<_>>()
                .join(","),
            report
                .graph_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":\"{}\",\"to\":\"{}\",\"kind\":\"{}\",\"line\":{}}}",
                    edge.from, edge.to, edge.kind, edge.line
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        FrontendFocus::Expansion => format!(
            "{{\"kind\":\"expansion\",\"expansion_previews\":[{}]}}",
            report
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
    }
}

fn frontend_json(frontend: Option<&FrontendReport>) -> String {
    match frontend {
        Some(frontend) => format!(
            "{{\"kind\":\"{}\",\"function_count\":{},\"function_nodes\":[{}],\"merged_step_count\":{},\"include_sources\":[{}],\"use_edges\":[{}],\"graph_nodes\":[{}],\"graph_edges\":[{}],\"expansion_previews\":[{}]}}",
            frontend.kind,
            frontend.function_count,
            frontend
                .function_nodes
                .iter()
                .map(|node| format!(
                    "{{\"name\":\"{}\",\"step_count\":{}}}",
                    node.name, node.step_count
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend.merged_step_count,
            string_json_list(&frontend.include_sources),
            frontend
                .use_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":\"{}\",\"to\":\"{}\",\"line\":{}}}",
                    edge.from, edge.to, edge.line
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_nodes
                .iter()
                .map(|node| match node.step_count {
                    Some(step_count) => format!(
                        "{{\"id\":\"{}\",\"kind\":\"{}\",\"step_count\":{}}}",
                        node.id, node.kind, step_count
                    ),
                    None => format!(
                        "{{\"id\":\"{}\",\"kind\":\"{}\",\"step_count\":null}}",
                        node.id, node.kind
                    ),
                })
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .graph_edges
                .iter()
                .map(|edge| format!(
                    "{{\"from\":\"{}\",\"to\":\"{}\",\"kind\":\"{}\",\"line\":{}}}",
                    edge.from, edge.to, edge.kind, edge.line
                ))
                .collect::<Vec<_>>()
                .join(","),
            frontend
                .expansion_previews
                .iter()
                .map(frontend_expansion_preview_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        None => "null".into(),
    }
}

fn frontend_expansion_preview_text(preview: &FrontendExpansionPreviewReport) -> String {
    let bindings = if preview.local_bindings.is_empty() {
        "none".to_string()
    } else {
        preview.local_bindings.join(", ")
    };
    let uses = if preview.use_targets.is_empty() {
        "none".to_string()
    } else {
        preview.use_targets.join(", ")
    };
    let steps = if preview.steps.is_empty() {
        "none".to_string()
    } else {
        preview.steps.join(" -> ")
    };
    format!(
        "- {} bindings=[{}] uses=[{}] steps=[{}]",
        preview.scope, bindings, uses, steps
    )
}

fn frontend_expansion_preview_json(preview: &FrontendExpansionPreviewReport) -> String {
    format!(
        "{{\"scope\":\"{}\",\"local_bindings\":[{}],\"steps\":[{}],\"use_targets\":[{}]}}",
        preview.scope,
        string_json_list(&preview.local_bindings),
        string_json_list(&preview.steps),
        string_json_list(&preview.use_targets),
    )
}

fn finding_text(finding: Option<&CompilerFinding>) -> String {
    match finding {
        Some(finding) => finding_text_record(finding),
        None => "none".into(),
    }
}

fn finding_json(finding: Option<&CompilerFinding>) -> String {
    match finding {
        Some(finding) => finding_json_record(finding),
        None => "null".into(),
    }
}

fn finding_text_record(finding: &CompilerFinding) -> String {
    match (finding.line, finding.column) {
        (Some(line), Some(column)) => format!(
            "stage={} severity={} code={} line={} column={} message={}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            column,
            finding.message
        ),
        (Some(line), None) => format!(
            "stage={} severity={} code={} line={} message={}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            finding.message
        ),
        (None, _) => format!(
            "stage={} severity={} code={} message={}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            finding.message
        ),
    }
}

fn finding_json_record(finding: &CompilerFinding) -> String {
    match (finding.line, finding.column) {
        (Some(line), Some(column)) => format!(
            "{{\"stage\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"line\":{},\"column\":{},\"message\":\"{}\"}}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            column,
            json_escape(&finding.message),
        ),
        (Some(line), None) => format!(
            "{{\"stage\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"line\":{},\"column\":null,\"message\":\"{}\"}}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            json_escape(&finding.message),
        ),
        (None, _) => format!(
            "{{\"stage\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"line\":null,\"column\":null,\"message\":\"{}\"}}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            json_escape(&finding.message),
        ),
    }
}

fn reason_profile_report(profile: &ReasonProfile) -> ReasonProfileReport {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => ReasonProfileReport::Builtin {
            id: profile.id().to_string(),
        },
        ReasonProfile::Declarative(model) => ReasonProfileReport::Declarative {
            id: model.id.to_string(),
            rules: model.rules.len(),
        },
    }
}

fn model_diagnostics_report(model: &ModelDiagnostics) -> ModelDiagnosticsReport {
    ModelDiagnosticsReport {
        model: model.model.to_string(),
        rules: model
            .rules
            .iter()
            .map(|rule| RuleDiagnosticsReport {
                rule_index: rule.rule_index,
                tier: rule_tier_text(&rule.tier).to_string(),
                supported: rule.supported,
                required_facts: rule
                    .required_facts
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                supporting_fragments: rule.supporting_fragments.clone(),
                missing_facts: rule
                    .missing_facts
                    .iter()
                    .map(|item| item.to_string())
                    .collect(),
                unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
            })
            .collect(),
    }
}

fn validation_report(
    binding: &TemplateBinding,
    diagnostics: Option<&BindingDiagnostics>,
    validation_error: Option<&RegistryError>,
) -> ValidationReport {
    let reason_rule_count = match binding.template.reason_profile.as_ref() {
        Some(ReasonProfile::Declarative(model)) => model.rules.len(),
        _ => 0,
    };
    let PayloadOffsetSupportSummary {
        sampled_offsets: sampled_payload_offsets,
        required_offsets: required_payload_offsets,
        unsupported_offsets: unsupported_payload_offsets,
    } = match diagnostics {
        Some(diagnostics) => {
            builtin_registry().payload_offset_support_summary(binding, diagnostics)
        }
        None => {
            let registry = builtin_registry();
            PayloadOffsetSupportSummary {
                sampled_offsets: binding
                    .template
                    .fragment_set
                    .iter()
                    .filter_map(|fragment_id| registry.descriptor(fragment_id))
                    .flat_map(|descriptor| descriptor.sampled_payload_offsets.iter().copied())
                    .collect(),
                required_offsets: Vec::new(),
                unsupported_offsets: Vec::new(),
            }
        }
    };
    ValidationReport {
        ok: validation_error.is_none(),
        registry: "builtin".into(),
        fragment_count: binding.template.fragment_set.len(),
        program_rule_count: binding
            .template
            .program_model
            .as_ref()
            .map_or(0, |model| model.rules.len()),
        reason_rule_count,
        checks: vec![
            "binding_schema".into(),
            "fragment_params".into(),
            "rule_evidence".into(),
            "payload_offsets".into(),
        ],
        sampled_payload_offsets,
        required_payload_offsets,
        unsupported_payload_offsets,
        finding: validation_error
            .map(|err| finding_from_registry_error(CompilerFindingStage::Validation, err)),
    }
}

fn diagnostics_stage_report(
    binding: &TemplateBinding,
    diagnostics: Result<BindingDiagnostics, RegistryError>,
) -> DiagnosticsStageReport {
    match diagnostics {
        Ok(diagnostics) => DiagnosticsStageReport {
            ok: true,
            report: Some(diagnostics_report(binding, &diagnostics)),
            finding: None,
        },
        Err(err) => DiagnosticsStageReport {
            ok: false,
            report: None,
            finding: Some(finding_from_registry_error(
                CompilerFindingStage::Diagnostics,
                &err,
            )),
        },
    }
}

fn empty_validation_report() -> ValidationReport {
    ValidationReport {
        ok: false,
        registry: "builtin".into(),
        fragment_count: 0,
        program_rule_count: 0,
        reason_rule_count: 0,
        checks: vec![
            "binding_schema".into(),
            "fragment_params".into(),
            "rule_evidence".into(),
            "payload_offsets".into(),
        ],
        sampled_payload_offsets: Vec::new(),
        required_payload_offsets: Vec::new(),
        unsupported_payload_offsets: Vec::new(),
        finding: None,
    }
}

fn findings_from_stage_reports(
    parse: &ParseStageReport,
    validation: &ValidationReport,
    diagnostics: &DiagnosticsStageReport,
) -> CompilerFindingsReport {
    let mut findings = Vec::new();
    if let Some(finding) = &parse.finding {
        findings.push(finding.clone());
    }
    if let Some(finding) = &validation.finding {
        findings.push(finding.clone());
    }
    if let Some(finding) = &diagnostics.finding {
        findings.push(finding.clone());
    }
    CompilerFindingsReport { findings }
}

fn fragment_param_report(value: &FragmentParamValue) -> ParamValueReport {
    match value {
        FragmentParamValue::Bool(value) => ParamValueReport::Bool(*value),
        FragmentParamValue::U64(value) => ParamValueReport::U64(*value),
        FragmentParamValue::String(value) => ParamValueReport::String(value.clone()),
    }
}

fn reason_profile_text(profile: &ReasonProfileReport) -> String {
    match profile {
        ReasonProfileReport::Builtin { id } => id.clone(),
        ReasonProfileReport::Declarative { id, rules } => {
            format!("declarative:{id} rules={rules}")
        }
    }
}

fn reason_profile_json(profile: &ReasonProfileReport) -> String {
    match profile {
        ReasonProfileReport::Builtin { id } => {
            format!("{{\"kind\":\"builtin\",\"id\":\"{id}\"}}")
        }
        ReasonProfileReport::Declarative { id, rules } => format!(
            "{{\"kind\":\"declarative\",\"id\":\"{}\",\"rules\":{}}}",
            id, rules
        ),
    }
}

fn program_operation_text(operation: &ProgramOperation) -> &str {
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(id) => id.as_str(),
        ProgramOperation::Unknown => "unknown",
    }
}

fn fragment_param_text(value: &ParamValueReport) -> String {
    match value {
        ParamValueReport::Bool(value) => value.to_string(),
        ParamValueReport::U64(value) => value.to_string(),
        ParamValueReport::String(value) => value.clone(),
    }
}

fn fragment_param_json(value: &ParamValueReport) -> String {
    match value {
        ParamValueReport::Bool(value) => value.to_string(),
        ParamValueReport::U64(value) => value.to_string(),
        ParamValueReport::String(value) => format!("\"{value}\""),
    }
}

fn evidence_tier_text(tier: &EvidenceTier) -> &'static str {
    match tier {
        EvidenceTier::CoreRequirement => "core_requirement",
        EvidenceTier::OptionalEnhancement => "optional_enhancement",
    }
}

fn rule_tier_text(tier: &RuleTier) -> &'static str {
    match tier {
        RuleTier::CoreRequirement => "core_requirement",
        RuleTier::OptionalEnhancement => "optional_enhancement",
        RuleTier::Unsupported => "unsupported",
    }
}

fn finding_from_dsl_error(err: &DslError) -> CompilerFinding {
    let root = err.root();
    CompilerFinding {
        stage: CompilerFindingStage::Parse,
        code: dsl_error_code(root).to_string(),
        severity: CompilerFindingSeverity::Error,
        line: err.line(),
        column: err.column(),
        message: dsl_error_message(root),
    }
}

fn finding_from_registry_error(
    stage: CompilerFindingStage,
    err: &RegistryError,
) -> CompilerFinding {
    CompilerFinding {
        stage,
        code: registry_error_code(err).to_string(),
        severity: CompilerFindingSeverity::Error,
        line: None,
        column: None,
        message: format!("{err:?}"),
    }
}

fn dsl_error_message(err: &DslError) -> String {
    match err {
        DslError::Located { inner, .. } => dsl_error_message(inner),
        DslError::InvalidLine(line) => format!("invalid line: {line}"),
        DslError::MissingField(field) => format!("missing field: {field}"),
        DslError::InvalidValue(value) => value.clone(),
        DslError::Registry(err) => format!("{err:?}"),
        DslError::Io(err) => err.clone(),
    }
}

fn dsl_error_code(err: &DslError) -> &'static str {
    match err {
        DslError::Located { inner, .. } => dsl_error_code(inner),
        DslError::InvalidLine(_) => "GEWYC-PARSE-INVALID-LINE",
        DslError::MissingField(_) => "GEWYC-PARSE-MISSING-FIELD",
        DslError::InvalidValue(value) => dsl_invalid_value_code(value),
        DslError::Registry(_) => "GEWYC-PARSE-REGISTRY",
        DslError::Io(_) => "GEWYC-PARSE-IO",
    }
}

fn dsl_invalid_value_code(value: &str) -> &'static str {
    if value.starts_with("unknown pipeline function '") {
        "GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION"
    } else if value.starts_with("unknown package dependency '") {
        "GEWYC-PARSE-UNKNOWN-PACKAGE-DEPENDENCY"
    } else if value == "pipeline include() requires a filesystem-backed entry file" {
        "GEWYC-PARSE-INCLUDE-NONFILESYSTEM-ENTRY"
    } else if value == "pipeline function bodies must contain '|>' steps"
        || value == "pipeline function bodies may only contain 'let' bindings or '|>' steps"
    {
        "GEWYC-PARSE-INVALID-FUNCTION-BODY"
    } else if value == "unclosed pipeline function block" {
        "GEWYC-PARSE-UNCLOSED-FUNCTION-BLOCK"
    } else {
        "GEWYC-PARSE-INVALID-VALUE"
    }
}

fn registry_error_code(err: &RegistryError) -> &'static str {
    match err {
        RegistryError::DuplicateFragmentId(_) => "GEWYC-VALIDATE-DUPLICATE-FRAGMENT-ID",
        RegistryError::MissingFragment(_) => "GEWYC-VALIDATE-MISSING-FRAGMENT",
        RegistryError::HookConflict(_) => "GEWYC-VALIDATE-HOOK-CONFLICT",
        RegistryError::FactConflict(_) => "GEWYC-VALIDATE-FACT-CONFLICT",
        RegistryError::MissingCoverage { .. } => "GEWYC-VALIDATE-MISSING-COVERAGE",
        RegistryError::MissingRuleEvidence { .. } => "GEWYC-VALIDATE-MISSING-RULE-EVIDENCE",
        RegistryError::UnsupportedRulePayloadOffsets { .. } => {
            "GEWYC-VALIDATE-UNSUPPORTED-PAYLOAD-OFFSETS"
        }
        RegistryError::UnknownFragmentParam { .. } => "GEWYC-VALIDATE-UNKNOWN-FRAGMENT-PARAM",
        RegistryError::InvalidFragmentParamType { .. } => {
            "GEWYC-VALIDATE-INVALID-FRAGMENT-PARAM-TYPE"
        }
    }
}

fn finding_stage_text(stage: CompilerFindingStage) -> &'static str {
    match stage {
        CompilerFindingStage::Parse => "parse",
        CompilerFindingStage::Validation => "validation",
        CompilerFindingStage::Diagnostics => "diagnostics",
    }
}

fn finding_severity_text(severity: CompilerFindingSeverity) -> &'static str {
    match severity {
        CompilerFindingSeverity::Error => "error",
        CompilerFindingSeverity::Warning => "warning",
    }
}

fn json_escape(value: &str) -> String {
    value
        .chars()
        .flat_map(|ch| match ch {
            '\\' => "\\\\".chars().collect::<Vec<_>>(),
            '"' => "\\\"".chars().collect::<Vec<_>>(),
            '\n' => "\\n".chars().collect::<Vec<_>>(),
            '\r' => "\\r".chars().collect::<Vec<_>>(),
            '\t' => "\\t".chars().collect::<Vec<_>>(),
            other => vec![other],
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_json_mentions_template_id() {
        let binding =
            compile_binding_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let json = render_binding(&binding, RenderFormat::Json);
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
        assert!(json.contains("\"program_model\""));
    }

    #[test]
    fn diagnostics_text_mentions_program_rule() {
        let binding =
            compile_binding_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let diagnostics = collect_binding_diagnostics(&binding).unwrap();
        let text = render_diagnostics(&binding, &diagnostics, RenderFormat::Text);
        assert!(text.contains("program_model="));
        assert!(text.contains("program_rule["));
    }

    #[test]
    fn binding_report_is_owned_and_stable() {
        let binding =
            compile_binding_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let report = binding_report(&binding);
        assert_eq!(report.template_id, "udp_process_debug");
        assert!(
            report
                .fragments
                .contains(&"udp_packet_meta_fragment".to_string())
        );
        assert!(
            report
                .fragment_params
                .iter()
                .any(|param| param.fragment == "sock_lineage_fragment"
                    && param.key == "capture_comm")
        );
    }

    #[test]
    fn compile_diagnostics_report_file_materializes_reason_and_program_models() {
        let report = compile_diagnostics_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert_eq!(report.template_id, "udp_process_debug");
        assert!(
            report
                .fragments
                .contains(&"udp_packet_meta_fragment".to_string())
        );
        assert!(report.program_model.is_some());
    }

    #[test]
    fn compile_envelope_str_collects_all_frontend_surfaces() {
        let input =
            crate::dsl::read_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let envelope = compile_envelope_str(&input);
        assert_eq!(
            envelope
                .binding
                .as_ref()
                .map(|report| report.template_id.as_str()),
            Some("udp_process_debug")
        );
        assert_eq!(
            envelope
                .diagnostics
                .as_ref()
                .and_then(|report| report.program_model.as_ref())
                .map(|_| true),
            Some(true)
        );
        assert!(envelope.findings.findings.is_empty());
        assert!(envelope.stages.parse.ok);
        assert!(envelope.stages.validation.ok);
        assert!(envelope.stages.diagnostics.ok);
    }

    #[test]
    fn compile_envelope_str_keeps_findings_and_stages_in_sync_for_parse_failure() {
        let envelope = compile_envelope_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        assert!(envelope.binding.is_none());
        assert!(envelope.diagnostics.is_none());
        assert_eq!(envelope.findings.findings.len(), 1);
        assert_eq!(
            envelope.findings.findings[0],
            envelope.stages.parse.finding.clone().unwrap()
        );
    }

    #[test]
    fn compile_stages_report_file_separates_binding_and_diagnostics_reports() {
        let report = compile_stages_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert!(report.parse.ok);
        assert!(report.parse.finding.is_none());
        assert_eq!(
            report.parse.report.as_ref().unwrap().template_id,
            "udp_process_debug"
        );
        assert!(report.diagnostics.ok);
        assert_eq!(
            report.diagnostics.report.as_ref().unwrap().template_id,
            "udp_process_debug"
        );
        assert!(report.validation.ok);
        assert!(report.validation.finding.is_none());
        assert_eq!(report.validation.registry, "builtin");
        assert_eq!(report.validation.fragment_count, 3);
        assert!(report.validation.program_rule_count > 0);
        assert!(
            report
                .validation
                .checks
                .contains(&"rule_evidence".to_string())
        );
        assert!(
            report
                .parse
                .report
                .as_ref()
                .unwrap()
                .program_model
                .is_some()
        );
        assert!(
            report
                .diagnostics
                .report
                .as_ref()
                .unwrap()
                .program_model
                .is_some()
        );
    }

    #[test]
    fn compile_stages_report_str_keeps_parse_failure_as_stage_finding() {
        let report = compile_stages_report_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        assert!(!report.parse.ok);
        assert!(report.parse.report.is_none());
        assert_eq!(
            report
                .parse
                .finding
                .as_ref()
                .map(|finding| finding.code.as_str()),
            Some("GEWYC-PARSE-INVALID-VALUE")
        );
        assert!(!report.validation.ok);
        assert!(report.validation.finding.is_none());
        assert!(!report.diagnostics.ok);
        assert!(report.diagnostics.finding.is_none());
    }

    #[test]
    fn compile_stages_report_file_keeps_partial_report_on_validation_failure() {
        let path = "/tmp/gewyc-validation-failure.gewy";
        std::fs::write(
            path,
            r#"
template(:broken_offset_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:broken_offset_validation_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: "static:snmp seen", dedupe: true)
"#,
        )
        .unwrap();
        let report = compile_stages_report_file(path).unwrap();
        assert!(!report.validation.ok);
        assert_eq!(
            report
                .validation
                .finding
                .as_ref()
                .map(|finding| finding.code.as_str()),
            Some("GEWYC-VALIDATE-UNSUPPORTED-PAYLOAD-OFFSETS")
        );
        assert!(report.diagnostics.ok);
        let diagnostics = report.diagnostics.report.as_ref().unwrap();
        let program_model = diagnostics.program_model.as_ref().unwrap();
        assert_eq!(program_model.rules[0].unsupported_payload_offsets, vec![8]);
    }

    #[test]
    fn compile_findings_report_str_surfaces_parse_failures() {
        let report = compile_findings_report_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].stage, CompilerFindingStage::Parse);
        assert_eq!(report.findings[0].code, "GEWYC-PARSE-INVALID-VALUE");
        assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
        assert_eq!(report.findings[0].line, Some(6));
        assert!(
            report.findings[0]
                .message
                .contains("unknown pipeline DSL step 'oops'")
        );
    }

    #[test]
    fn compile_findings_report_str_surfaces_validation_failures() {
        let report = compile_findings_report_str(
            r#"
template(:broken_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:route_meta_fragment)
|> program_model(:broken_validation_model)
|> operation(:dns_lookup)
|> program_rule(predicate: "datagram_observed:udp", stage: :datagram_observed, narrative: "static:udp seen", dedupe: true)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].stage, CompilerFindingStage::Validation);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-VALIDATE-MISSING-RULE-EVIDENCE"
        );
        assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
        assert_eq!(report.findings[0].line, None);
        assert!(report.findings[0].message.contains("MissingRuleEvidence"));
    }

    #[test]
    fn compile_findings_report_str_surfaces_unsupported_payload_offset_failures() {
        let report = compile_findings_report_str(
            r#"
template(:broken_offset_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:broken_offset_validation_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: "static:snmp seen", dedupe: true)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].stage, CompilerFindingStage::Validation);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-VALIDATE-UNSUPPORTED-PAYLOAD-OFFSETS"
        );
        assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
        assert_eq!(report.findings[0].line, None);
        assert!(
            report.findings[0]
                .message
                .contains("UnsupportedRulePayloadOffsets")
        );
    }

    #[test]
    fn compile_findings_report_str_is_empty_when_pipeline_succeeds() {
        let input =
            crate::dsl::read_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let report = compile_findings_report_str(&input);
        assert!(report.findings.is_empty());
    }

    #[test]
    fn compile_findings_report_str_uses_specific_code_for_unknown_pipeline_function() {
        let report = compile_findings_report_str(
            r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION"
        );
        assert_eq!(report.findings[0].line, Some(5));
        assert!(
            report.findings[0]
                .message
                .contains("unknown pipeline function 'missing_core'")
        );
    }

    #[test]
    fn compile_findings_report_file_uses_specific_code_for_unknown_package_dependency() {
        let package_dir =
            std::env::temp_dir().join(format!("gewyc-missing-dependency-{}", std::process::id()));
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(
            package_dir.join("gewy.pkg"),
            "name=missing_dependency_pkg\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        std::fs::write(
            package_dir.join("main.gewy"),
            r#"
template(:missing_dependency_pkg)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("missing_dep:module.gewy")
"#,
        )
        .unwrap();

        let report = compile_findings_report_file(package_dir.to_str().unwrap());
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-PARSE-UNKNOWN-PACKAGE-DEPENDENCY"
        );
        assert_eq!(report.findings[0].line, Some(5));
        assert!(
            report.findings[0]
                .message
                .contains("unknown package dependency 'missing_dep'")
        );
    }

    #[test]
    fn compile_findings_report_str_uses_specific_code_for_nonfilesystem_include() {
        let report = compile_findings_report_str(
            r#"
template(:include_without_package)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-PARSE-INCLUDE-NONFILESYSTEM-ENTRY"
        );
        assert_eq!(report.findings[0].line, Some(5));
    }

    #[test]
    fn compile_findings_report_str_uses_specific_code_for_invalid_function_body() {
        let report = compile_findings_report_str(
            r#"
fn udp_core() {
  fragment(:udp_packet_meta_fragment)
}

template(:invalid_function_body)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].code, "GEWYC-PARSE-INVALID-FUNCTION-BODY");
        assert_eq!(report.findings[0].line, Some(3));
    }

    #[test]
    fn compile_findings_report_str_uses_specific_code_for_unclosed_function_block() {
        let report = compile_findings_report_str(
            r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].code,
            "GEWYC-PARSE-UNCLOSED-FUNCTION-BLOCK"
        );
        assert_eq!(report.findings[0].line, Some(2));
    }

    #[test]
    fn findings_json_includes_code_severity_and_line() {
        let report = compile_findings_report_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        let json = render_findings_report(&report, RenderFormat::Json);
        assert!(json.contains("\"code\":\"GEWYC-PARSE-INVALID-VALUE\""));
        assert!(json.contains("\"severity\":\"error\""));
        assert!(json.contains("\"line\":6"));
    }

    #[test]
    fn stage_local_finding_json_matches_standalone_findings_shape() {
        let stages = compile_stages_report_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        let standalone = compile_findings_report_str(
            r#"
template(:broken)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> oops(:true)
"#,
        );
        let standalone_finding = standalone.findings.first().unwrap();
        let stages_json = render_stages_report(&stages, RenderFormat::Json);
        let expected = finding_json_record(standalone_finding);
        assert!(stages_json.contains(&format!("\"finding\":{expected}")));
    }

    #[test]
    fn stage_local_finding_keeps_specific_frontend_parse_code() {
        let stages = compile_stages_report_str(
            r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
        );
        assert_eq!(
            stages
                .parse
                .finding
                .as_ref()
                .map(|finding| finding.code.as_str()),
            Some("GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION")
        );
        assert_eq!(
            stages
                .parse
                .finding
                .as_ref()
                .and_then(|finding| finding.column),
            None
        );
    }

    #[test]
    fn parse_findings_surface_column_for_invalid_function_signature() {
        let report = compile_findings_report_str(
            r#"
fn broken =
template(:broken)
|> use(:broken)
"#,
        );
        let finding = report.findings.first().expect("parse finding");
        assert_eq!(finding.line, Some(2));
        assert_eq!(finding.column, Some(10));
        let text = render_findings_report(&report, RenderFormat::Text);
        let json = render_findings_report(&report, RenderFormat::Json);
        assert!(text.contains("line=2 column=10"));
        assert!(json.contains("\"line\":2"));
        assert!(json.contains("\"column\":10"));
    }

    #[test]
    fn parse_findings_surface_column_for_invalid_let_binding() {
        let report = compile_findings_report_str(
            r#"
fn demo() =
  let op
template(:demo)
|> use(:demo)
"#,
        );
        let finding = report.findings.first().expect("parse finding");
        assert_eq!(finding.line, Some(3));
        assert_eq!(finding.column, Some(9));
        let text = render_findings_report(&report, RenderFormat::Text);
        let json = render_findings_report(&report, RenderFormat::Json);
        assert!(text.contains("line=3 column=9"));
        assert!(json.contains("\"line\":3"));
        assert!(json.contains("\"column\":9"));
    }

    #[test]
    fn parse_findings_surface_column_for_window_keyword_error() {
        let report = compile_findings_report_str(
            r#"
template(:demo)
|> window(duration_ms: 5000)
|> reason(:udp_datagram_l1)
"#,
        );
        let finding = report.findings.first().expect("parse finding");
        assert_eq!(finding.line, Some(3));
        assert_eq!(finding.column, Some(4));
    }

    #[test]
    fn parse_findings_surface_column_for_program_rule_keyword_error() {
        let report = compile_findings_report_str(
            r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "process_bound", stage: :connect_flow, dedupe: true)
"#,
        );
        let finding = report.findings.first().expect("parse finding");
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(4));
    }

    #[test]
    fn parse_findings_surface_column_for_program_rule_invalid_stage_value() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "process_bound", stage: :not_a_stage, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find(":not_a_stage").unwrap() + 1;
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_program_rule_invalid_predicate_value() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "packet_observed:tcp:remote:notaport", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("\"packet_observed").unwrap() + 1;
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_packet_byte_at_qualifier_error() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("byte_at").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_datagram_bytes_at_missing_sequence() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:bytes_at:8", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("bytes_at").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_socket_state_invalid_port() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "socket_state_observed:remote:notaport", stage: :socket_state_transition, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("notaport").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_quic_packet_invalid_type() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "quic_packet_observed:remote:quic:type:not_a_type", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("not_a_type").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_quic_frame_byte_at_error() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "quic_frame_observed:remote:quic:frame:crypto:byte_at:not_u16:0xff:0xa0", stage: :datagram_observed, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("byte_at").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_all_predicate_child_error() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "all(process_bound, packet_observed:tcp:remote:mysql:byte_at:not_u16:255:1)", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("byte_at").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn parse_findings_surface_column_for_any_predicate_child_error() {
        let input = r#"
template(:demo)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:datagram_exchange)
|> program_model(:demo_model)
|> program_rule(predicate: "any(process_bound, quic_packet_observed:remote:quic:type:not_a_type)", stage: :connect_flow, narrative: "static:test", dedupe: true)
"#;
        let report = compile_findings_report_str(input);
        let finding = report.findings.first().expect("parse finding");
        let line = input.lines().nth(7).unwrap();
        let expected_column = line.find("not_a_type").unwrap();
        assert_eq!(finding.line, Some(8));
        assert_eq!(finding.column, Some(expected_column));
    }

    #[test]
    fn stage_local_finding_without_column_stays_shape_compatible() {
        let stages = compile_stages_report_str(
            r#"
template(:broken_pipeline_use)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:missing_core)
"#,
        );
        assert_eq!(
            stages
                .parse
                .finding
                .as_ref()
                .and_then(|finding| finding.line),
            Some(5)
        );
    }

    #[test]
    fn envelope_json_contains_all_frontend_surfaces() {
        let input =
            crate::dsl::read_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
                .unwrap();
        let envelope = compile_envelope_str(&input);
        let json = render_envelope_report(&envelope, RenderFormat::Json);
        assert!(json.contains("\"binding\":"));
        assert!(json.contains("\"diagnostics\":"));
        assert!(json.contains("\"findings\":{\"findings\":[]}"));
        assert!(json.contains("\"stages\":"));
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    }

    #[test]
    fn compile_frontend_report_file_materializes_pipeline_summary() {
        let report = compile_frontend_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert_eq!(report.kind, "pipeline");
        assert!(!report.function_nodes.is_empty());
        assert!(!report.graph_nodes.is_empty());
        assert!(!report.graph_edges.is_empty());
        assert!(!report.expansion_previews.is_empty());
        assert_eq!(report.expansion_previews[0].scope, "entry");
        assert!(
            report.expansion_previews[0]
                .steps
                .iter()
                .any(|step| step.starts_with("use("))
        );
    }

    #[test]
    fn compile_explain_report_file_materializes_human_summary_surface() {
        let report = compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert!(report.ok);
        assert!(report.binding.is_some());
        assert!(report.frontend.is_some());
        assert!(report.findings.findings.is_empty());
        let text = render_explain_report(&report, RenderFormat::Text);
        let json = render_explain_report(&report, RenderFormat::Json);
        assert!(text.contains("surface=explain"));
        assert!(text.contains("validation:"));
        assert!(text.contains("next_step="));
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"next_step\""));
    }

    #[test]
    fn explain_report_suggests_frontend_for_parse_failure() {
        let report = compile_explain_report_str(
            r#"
template(:broken_parse)
|> window(:default_5s)
|> use(:missing_function)
"#,
        );
        let text = render_explain_report(&report, RenderFormat::Text);
        assert!(text.contains("next_step=fix the parse finding first"));
        assert!(text.contains("gewyc frontend"));
    }

    #[test]
    fn explain_report_includes_parse_source_excerpt() {
        let report = compile_explain_report_str(
            r#"
template(:demo)
fn broken( =
  |> fragment(:udp_packet_meta_fragment)
"#,
        );
        let text = render_explain_report(&report, RenderFormat::Text);
        let json = render_explain_report(&report, RenderFormat::Json);
        assert!(text.contains("parse_source_excerpt=fn broken( ="));
        assert!(text.contains("parse_source_marker="));
        assert!(json.contains("\"parse_source_excerpt\""));
        assert!(json.contains("\"line_text\":\"fn broken( =\""));
    }

    #[test]
    fn explain_report_suggests_unsupported_offsets_for_validation_failure() {
        let report = compile_explain_report_str(
            r#"
template(:broken_offsets)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> operation(:snmp_query)
|> program_model(:broken_offsets_model)
|> program_rule(predicate: "packet_observed:tcp:remote:mysql:byte_at:42:255:1", stage: :packet_observed, narrative: "static:test", dedupe: true)
"#,
        );
        let text = render_explain_report(&report, RenderFormat::Text);
        let json = render_explain_report(&report, RenderFormat::Json);
        assert!(text.contains("unsupported_payload_offsets"));
        assert!(text.contains("validation_excerpt=model:broken_offsets_model rule:0"));
        assert!(text.contains("validation_note="));
        assert!(text.contains("adjust fragment coverage or payload matchers"));
        assert!(json.contains("unsupported_payload_offsets"));
        assert!(json.contains("\"validation_excerpt\""));
        assert!(json.contains("\"validation_shape_note\""));
        assert!(json.contains("\"model\":\"broken_offsets_model\""));
    }

    #[test]
    fn explain_report_includes_diagnostics_excerpt_for_rule_support_failures() {
        let report = compile_explain_report_str(
            r#"
template(:broken_offset_validation)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> fragment(:udp_packet_meta_fragment)
|> program_model(:broken_offset_validation_model)
|> operation(:snmp_get)
|> program_rule(predicate: "datagram_observed:udp:remote:snmp:byte_at:8:0xff:0xa0", stage: :datagram_observed, narrative: "static:snmp seen", dedupe: true)
"#,
        );
        let text = render_explain_report_with_focus(
            &report,
            RenderFormat::Text,
            Some(ExplainFocus::Diagnostics),
        );
        let json = render_explain_report(&report, RenderFormat::Json);
        assert!(text.contains("diagnostics_excerpt=model:broken_offset_validation_model"));
        assert!(text.contains("diagnostics_note="));
        assert!(text.contains("offsets:[8]"));
        assert!(json.contains("\"diagnostics_excerpt\""));
        assert!(json.contains("\"diagnostics_shape_note\""));
        assert!(json.contains("\"model\":\"broken_offset_validation_model\""));
    }

    #[test]
    fn stages_json_includes_parse_and_diagnostics_sections() {
        let report = compile_stages_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let json = render_stages_report(&report, RenderFormat::Json);
        assert!(json.contains("\"parse\":{\"ok\":true"));
        assert!(json.contains("\"frontend\":"));
        assert!(json.contains("\"function_nodes\""));
        assert!(json.contains("\"use_edges\""));
        assert!(json.contains("\"graph_nodes\""));
        assert!(json.contains("\"graph_edges\""));
        assert!(json.contains("\"validation\":{\"ok\":true"));
        assert!(json.contains("\"registry\":\"builtin\""));
        assert!(json.contains(
            "\"checks\":[\"binding_schema\",\"fragment_params\",\"rule_evidence\",\"payload_offsets\"]"
        ));
        assert!(json.contains("\"sampled_payload_offsets\":[0,1,4,5,9,10,13]"));
        assert!(json.contains("\"required_payload_offsets\":[]"));
        assert!(json.contains("\"unsupported_payload_offsets\":[]"));
        assert!(json.contains("\"finding\":null"));
        assert!(json.contains("\"diagnostics\":{\"ok\":true"));
        assert!(json.contains("\"report\":"));
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    }

    #[test]
    fn stages_report_includes_pipeline_frontend_summary() {
        let report = compile_stages_report_str(
            r#"
fn udp_rules() {
  |> operation(:datagram_exchange)
  |> program_model(:frontend_summary_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_summary, phase: :bind)
}

fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> use(:udp_rules)
}

template(:frontend_summary)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
        );
        let frontend = report.parse.frontend.as_ref().unwrap();
        assert_eq!(frontend.kind, "pipeline");
        assert_eq!(frontend.function_count, 2);
        assert_eq!(
            frontend.function_nodes,
            vec![
                FrontendFunctionReport {
                    name: "udp_core".to_string(),
                    step_count: 3,
                },
                FrontendFunctionReport {
                    name: "udp_rules".to_string(),
                    step_count: 3,
                }
            ]
        );
        assert_eq!(frontend.merged_step_count, 9);
        assert_eq!(
            frontend.use_edges,
            vec![
                FrontendUseEdgeReport {
                    from: "entry".to_string(),
                    to: "udp_core".to_string(),
                    line: 17,
                },
                FrontendUseEdgeReport {
                    from: "udp_core".to_string(),
                    to: "udp_rules".to_string(),
                    line: 11,
                }
            ]
        );
        assert_eq!(
            frontend.graph_nodes,
            vec![
                FrontendGraphNodeReport {
                    id: "entry".to_string(),
                    kind: "entry".to_string(),
                    step_count: Some(3),
                },
                FrontendGraphNodeReport {
                    id: "fn:udp_core".to_string(),
                    kind: "function".to_string(),
                    step_count: Some(3),
                },
                FrontendGraphNodeReport {
                    id: "fn:udp_rules".to_string(),
                    kind: "function".to_string(),
                    step_count: Some(3),
                }
            ]
        );
        assert_eq!(
            frontend.graph_edges,
            vec![
                FrontendGraphEdgeReport {
                    from: "entry".to_string(),
                    to: "fn:udp_core".to_string(),
                    kind: "use".to_string(),
                    line: 17,
                },
                FrontendGraphEdgeReport {
                    from: "fn:udp_core".to_string(),
                    to: "fn:udp_rules".to_string(),
                    kind: "use".to_string(),
                    line: 11,
                }
            ]
        );
    }

    #[test]
    fn stages_report_lists_include_sources_in_parse_frontend_summary() {
        let package_dir =
            std::env::temp_dir().join(format!("gewyc-frontend-summary-{}", std::process::id()));
        std::fs::create_dir_all(&package_dir).unwrap();
        std::fs::write(
            package_dir.join("gewy.pkg"),
            "name=frontend_summary_pkg\nversion=0.1.0\nentry=main.gewy\n",
        )
        .unwrap();
        std::fs::write(
            package_dir.join("main.gewy"),
            r#"
template(:frontend_summary_pkg)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:udp_core)
"#,
        )
        .unwrap();
        std::fs::write(
            package_dir.join("module.gewy"),
            r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:frontend_summary_pkg_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_summary_pkg, phase: :bind)
}
"#,
        )
        .unwrap();

        let report = compile_stages_report_file(package_dir.to_str().unwrap()).unwrap();
        let frontend = report.parse.frontend.as_ref().unwrap();
        assert_eq!(frontend.kind, "pipeline");
        assert_eq!(frontend.function_count, 1);
        assert_eq!(
            frontend.function_nodes,
            vec![FrontendFunctionReport {
                name: "udp_core".to_string(),
                step_count: 5,
            }]
        );
        assert_eq!(frontend.include_sources.len(), 1);
        assert!(frontend.include_sources[0].ends_with("module.gewy"));
        assert_eq!(
            frontend.use_edges,
            vec![FrontendUseEdgeReport {
                from: "entry".to_string(),
                to: "udp_core".to_string(),
                line: 6,
            }]
        );
        assert!(frontend.graph_nodes.iter().any(|node| node.kind == "entry"));
        assert!(frontend.graph_nodes.iter().any(|node| node.kind == "file"));
        assert!(
            frontend
                .graph_edges
                .iter()
                .any(|edge| edge.kind == "include" && edge.line == 5)
        );
    }

    #[test]
    fn stages_report_summarizes_payload_offset_support() {
        let report =
            compile_stages_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy")
                .unwrap();
        assert_eq!(
            report.validation.sampled_payload_offsets,
            vec![0, 1, 4, 5, 9, 10, 13]
        );
        assert_eq!(report.validation.required_payload_offsets, vec![13]);
        assert_eq!(
            report.validation.unsupported_payload_offsets,
            Vec::<u16>::new()
        );
    }
}
