use crate::dsl::{
    DslError, compile_file, parse_file_unvalidated, parse_str_unvalidated, validate_compiled_binding,
};
use crate::flow::ProgramOperation;
use crate::fragment::{BindingDiagnostics, EvidenceTier, ModelDiagnostics, RegistryError, RuleTier, builtin_registry};
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
    Builtin {
        id: String,
    },
    Declarative {
        id: String,
        rules: usize,
    },
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
    pub parsed_binding: BindingReport,
    pub validation: ValidationReport,
    pub diagnostics: DiagnosticsReport,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationReport {
    pub ok: bool,
    pub registry: String,
    pub fragment_count: usize,
    pub program_rule_count: usize,
    pub reason_rule_count: usize,
    pub checks: Vec<String>,
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
    compile_binding_file(path).map(|binding| binding_report(&binding))
}

pub fn collect_binding_diagnostics(
    binding: &TemplateBinding,
) -> Result<BindingDiagnostics, RegistryError> {
    builtin_registry().binding_diagnostics(binding)
}

pub fn compile_diagnostics_report_file(path: &str) -> Result<DiagnosticsReport, CompileDiagnosticsError> {
    let binding = compile_binding_file(path)?;
    let diagnostics = collect_binding_diagnostics(&binding)?;
    Ok(diagnostics_report(&binding, &diagnostics))
}

pub fn compile_stages_report_file(path: &str) -> Result<CompilerStagesReport, CompileStagesError> {
    let binding = parse_file_unvalidated(path)?;
    let parsed_binding = binding_report(&binding);
    validate_compiled_binding(&binding).map_err(CompileStagesError::Validation)?;
    let diagnostics = collect_binding_diagnostics(&binding).map_err(CompileStagesError::Diagnostics)?;
    Ok(CompilerStagesReport {
        parsed_binding,
        validation: validation_report(&binding),
        diagnostics: diagnostics_report(&binding, &diagnostics),
    })
}

pub fn compile_findings_report_file(path: &str) -> CompilerFindingsReport {
    let input = match crate::dsl::read_file(path) {
        Ok(input) => input,
        Err(err) => return CompilerFindingsReport {
            findings: vec![finding_from_dsl_error(&err)],
        },
    };
    compile_findings_report_str(&input)
}

pub fn compile_findings_report_str(input: &str) -> CompilerFindingsReport {
    let binding = match parse_str_unvalidated(input) {
        Ok(binding) => binding,
        Err(err) => {
            return CompilerFindingsReport {
                findings: vec![finding_from_dsl_error(&err)],
            }
        }
    };

    if let Err(err) = validate_compiled_binding(&binding) {
        return CompilerFindingsReport {
            findings: vec![finding_from_registry_error(CompilerFindingStage::Validation, &err)],
        };
    }

    if let Err(err) = collect_binding_diagnostics(&binding) {
        return CompilerFindingsReport {
            findings: vec![finding_from_registry_error(CompilerFindingStage::Diagnostics, &err)],
        };
    }

    CompilerFindingsReport { findings: Vec::new() }
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
    Validation(RegistryError),
    Diagnostics(RegistryError),
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

pub fn binding_report(binding: &TemplateBinding) -> BindingReport {
    BindingReport {
        template_id: binding.template.id.to_string(),
        fragments: binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| (*fragment).to_string())
            .collect(),
        window: binding.template.window_profile.as_ref().map(|window| WindowReport {
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
        program_model: diagnostics.program_model.as_ref().map(model_diagnostics_report),
        reason_model: diagnostics.reason_model.as_ref().map(model_diagnostics_report),
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
            if let Some((_, entries)) = acc.iter_mut().find(|(fragment, _)| fragment == &param.fragment)
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
        .map(|finding| match finding.line {
            Some(line) => format!(
                "finding stage={} severity={} code={} line={} message={}",
                finding_stage_text(finding.stage),
                finding_severity_text(finding.severity),
                finding.code,
                line,
                finding.message
            ),
            None => format!(
                "finding stage={} severity={} code={} message={}",
                finding_stage_text(finding.stage),
                finding_severity_text(finding.severity),
                finding.code,
                finding.message
            ),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn findings_json(report: &CompilerFindingsReport) -> String {
    format!(
        "{{\"findings\":[{}]}}",
        report
            .findings
            .iter()
            .map(|finding| match finding.line {
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
            })
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn stages_text(report: &CompilerStagesReport) -> String {
    format!(
        "stage=parse\n{}\nstage=validation\nok={}\nregistry={}\nfragments={}\nprogram_rules={}\nreason_rules={}\nchecks={}\nstage=diagnostics\n{}",
        binding_text(&report.parsed_binding),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        report.validation.checks.join(","),
        diagnostics_text(&report.diagnostics)
    )
}

fn stages_json(report: &CompilerStagesReport) -> String {
    format!(
        "{{\"parse\":{},\"validation\":{{\"ok\":{},\"registry\":\"{}\",\"fragment_count\":{},\"program_rule_count\":{},\"reason_rule_count\":{},\"checks\":[{}]}},\"diagnostics\":{}}}",
        binding_json(&report.parsed_binding),
        report.validation.ok,
        report.validation.registry,
        report.validation.fragment_count,
        report.validation.program_rule_count,
        report.validation.reason_rule_count,
        string_json_list(&report.validation.checks),
        diagnostics_json(&report.diagnostics),
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
                required_facts: rule.required_facts.iter().map(|item| item.to_string()).collect(),
                supporting_fragments: rule.supporting_fragments.clone(),
                missing_facts: rule.missing_facts.iter().map(|item| item.to_string()).collect(),
                unsupported_payload_offsets: rule.unsupported_payload_offsets.clone(),
            })
            .collect(),
    }
}

fn validation_report(binding: &TemplateBinding) -> ValidationReport {
    let reason_rule_count = match binding.template.reason_profile.as_ref() {
        Some(ReasonProfile::Declarative(model)) => model.rules.len(),
        _ => 0,
    };
    ValidationReport {
        ok: true,
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
        ],
    }
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

fn finding_from_registry_error(stage: CompilerFindingStage, err: &RegistryError) -> CompilerFinding {
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
        RegistryError::UnknownFragmentParam { .. } => "GEWYC-VALIDATE-UNKNOWN-FRAGMENT-PARAM",
        RegistryError::InvalidFragmentParamType { .. } => "GEWYC-VALIDATE-INVALID-FRAGMENT-PARAM-TYPE",
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
        let binding = compile_binding_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let json = render_binding(&binding, RenderFormat::Json);
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
        assert!(json.contains("\"program_model\""));
    }

    #[test]
    fn diagnostics_text_mentions_program_rule() {
        let binding = compile_binding_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let diagnostics = collect_binding_diagnostics(&binding).unwrap();
        let text = render_diagnostics(&binding, &diagnostics, RenderFormat::Text);
        assert!(text.contains("program_model="));
        assert!(text.contains("program_rule["));
    }

    #[test]
    fn binding_report_is_owned_and_stable() {
        let binding = compile_binding_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let report = binding_report(&binding);
        assert_eq!(report.template_id, "udp_process_debug");
        assert!(report.fragments.contains(&"udp_packet_meta_fragment".to_string()));
        assert!(
            report
                .fragment_params
                .iter()
                .any(|param| param.fragment == "sock_lineage_fragment" && param.key == "capture_comm")
        );
    }

    #[test]
    fn compile_diagnostics_report_file_materializes_reason_and_program_models() {
        let report = compile_diagnostics_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert_eq!(report.template_id, "udp_process_debug");
        assert!(report.fragments.contains(&"udp_packet_meta_fragment".to_string()));
        assert!(report.program_model.is_some());
    }

    #[test]
    fn compile_stages_report_file_separates_binding_and_diagnostics_reports() {
        let report = compile_stages_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        assert_eq!(report.parsed_binding.template_id, "udp_process_debug");
        assert_eq!(report.diagnostics.template_id, "udp_process_debug");
        assert!(report.validation.ok);
        assert_eq!(report.validation.registry, "builtin");
        assert_eq!(report.validation.fragment_count, 3);
        assert!(report.validation.program_rule_count > 0);
        assert!(report.validation.checks.contains(&"rule_evidence".to_string()));
        assert!(report.parsed_binding.program_model.is_some());
        assert!(report.diagnostics.program_model.is_some());
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
        assert_eq!(report.findings[0].code, "GEWYC-VALIDATE-MISSING-RULE-EVIDENCE");
        assert_eq!(report.findings[0].severity, CompilerFindingSeverity::Error);
        assert_eq!(report.findings[0].line, None);
        assert!(report.findings[0].message.contains("MissingRuleEvidence"));
    }

    #[test]
    fn compile_findings_report_str_is_empty_when_pipeline_succeeds() {
        let input = crate::dsl::read_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
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
    fn stages_json_includes_parse_and_diagnostics_sections() {
        let report = compile_stages_report_file(
            "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
        )
        .unwrap();
        let json = render_stages_report(&report, RenderFormat::Json);
        assert!(json.contains("\"parse\":"));
        assert!(json.contains("\"validation\":{\"ok\":true"));
        assert!(json.contains("\"registry\":\"builtin\""));
        assert!(json.contains("\"checks\":[\"binding_schema\",\"fragment_params\",\"rule_evidence\"]"));
        assert!(json.contains("\"diagnostics\":"));
        assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    }
}
