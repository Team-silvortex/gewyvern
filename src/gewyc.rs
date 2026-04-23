use crate::dsl::{DslError, compile_file, parse_str_unvalidated, validate_compiled_binding};
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
pub struct ParseStageReport {
    pub ok: bool,
    pub report: Option<BindingReport>,
    pub finding: Option<CompilerFinding>,
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
    pub message: String,
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
    let input = match crate::dsl::read_file(path) {
        Ok(input) => input,
        Err(err) => {
            return CompilerFindingsReport {
                findings: vec![finding_from_dsl_error(&err)],
            };
        }
    };
    compile_findings_report_str(&input)
}

pub fn compile_findings_report_str(input: &str) -> CompilerFindingsReport {
    compile_envelope_str(input).findings
}

pub fn compile_envelope_file(path: &str) -> Result<CompilerEnvelope, DslError> {
    let input = crate::dsl::read_file(path)?;
    Ok(compile_envelope_str(&input))
}

pub fn compile_envelope_str(input: &str) -> CompilerEnvelope {
    match parse_str_unvalidated(input) {
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

fn stages_text(report: &CompilerStagesReport) -> String {
    format!(
        "stage=parse\nok={}\nparse_finding={}\n{}\nstage=validation\nok={}\nregistry={}\nfragments={}\nprogram_rules={}\nreason_rules={}\nchecks={}\nsampled_payload_offsets={:?}\nrequired_payload_offsets={:?}\nunsupported_payload_offsets={:?}\nvalidation_finding={}\nstage=diagnostics\nok={}\ndiagnostics_finding={}\n{}",
        report.parse.ok,
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
        "{{\"parse\":{{\"ok\":{},\"finding\":{},\"report\":{}}},\"validation\":{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}],\"sampled_payload_offsets\":[{}],\"required_payload_offsets\":[{}],\"unsupported_payload_offsets\":[{}],\"finding\":{}}},\"diagnostics\":{{\"ok\":{},\"finding\":{},\"report\":{}}}}}",
        report.parse.ok,
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
    match finding.line {
        Some(line) => format!(
            "stage={} severity={} code={} line={} message={}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            finding.message
        ),
        None => format!(
            "stage={} severity={} code={} message={}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            finding.message
        ),
    }
}

fn finding_json_record(finding: &CompilerFinding) -> String {
    match finding.line {
        Some(line) => format!(
            "{{\"stage\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"line\":{},\"message\":\"{}\"}}",
            finding_stage_text(finding.stage),
            finding_severity_text(finding.severity),
            finding.code,
            line,
            json_escape(&finding.message),
        ),
        None => format!(
            "{{\"stage\":\"{}\",\"severity\":\"{}\",\"code\":\"{}\",\"line\":null,\"message\":\"{}\"}}",
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
        DslError::InvalidValue(_) => "GEWYC-PARSE-INVALID-VALUE",
        DslError::Registry(_) => "GEWYC-PARSE-REGISTRY",
        DslError::Io(_) => "GEWYC-PARSE-IO",
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
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
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
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
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
template=broken_offset_validation
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=broken_offset_validation_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:9:0xff:0xa0;datagram_observed;static:snmp seen;true
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
        assert_eq!(program_model.rules[0].unsupported_payload_offsets, vec![9]);
    }

    #[test]
    fn compile_findings_report_str_surfaces_parse_failures() {
        let report = compile_findings_report_str(
            r#"
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
"#,
        );
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].stage, CompilerFindingStage::Parse);
        assert_eq!(report.findings[0].code, "GEWYC-PARSE-INVALID-VALUE");
        assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
        assert_eq!(report.findings[0].line, Some(6));
        assert!(report.findings[0].message.contains("unknown DSL key"));
    }

    #[test]
    fn compile_findings_report_str_surfaces_validation_failures() {
        let report = compile_findings_report_str(
            r#"
template=broken_validation
window=default_5s
reason=udp_datagram_l1
fragment=route_meta_fragment
program_model=broken_validation_model
operation=dns_lookup
rule=datagram_observed:udp;datagram_observed;static:udp seen;true
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
template=broken_offset_validation
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
program_model=broken_offset_validation_model
operation=snmp_get
rule=datagram_observed:udp:remote:snmp:byte_at:9:0xff:0xa0;datagram_observed;static:snmp seen;true
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
    fn findings_json_includes_code_severity_and_line() {
        let report = compile_findings_report_str(
            r#"
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
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
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
"#,
        );
        let standalone = compile_findings_report_str(
            r#"
template=broken
window=default_5s
reason=udp_datagram_l1
fragment=udp_packet_meta_fragment
oops=true
"#,
        );
        let standalone_finding = standalone.findings.first().unwrap();
        let stages_json = render_stages_report(&stages, RenderFormat::Json);
        let expected = finding_json_record(standalone_finding);
        assert!(stages_json.contains(&format!("\"finding\":{expected}")));
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
    fn stages_json_includes_parse_and_diagnostics_sections() {
        let report = compile_stages_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let json = render_stages_report(&report, RenderFormat::Json);
        assert!(json.contains("\"parse\":{\"ok\":true"));
        assert!(json.contains("\"validation\":{\"ok\":true"));
        assert!(json.contains("\"registry\":\"builtin\""));
        assert!(json.contains(
            "\"checks\":[\"binding_schema\",\"fragment_params\",\"rule_evidence\",\"payload_offsets\"]"
        ));
        assert!(json.contains("\"sampled_payload_offsets\":[0,4,5,13]"));
        assert!(json.contains("\"required_payload_offsets\":[]"));
        assert!(json.contains("\"unsupported_payload_offsets\":[]"));
        assert!(json.contains("\"finding\":null"));
        assert!(json.contains("\"diagnostics\":{\"ok\":true"));
        assert!(json.contains("\"report\":"));
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    }

    #[test]
    fn stages_report_summarizes_payload_offset_support() {
        let report =
            compile_stages_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy")
                .unwrap();
        assert_eq!(report.validation.sampled_payload_offsets, vec![0, 4, 5, 13]);
        assert_eq!(report.validation.required_payload_offsets, vec![13]);
        assert_eq!(
            report.validation.unsupported_payload_offsets,
            Vec::<u16>::new()
        );
    }
}
