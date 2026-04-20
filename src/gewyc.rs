use crate::dsl::{DslError, compile_file};
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
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    if let Some(model) = &report.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
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

fn model_diagnostics_json(model: &ModelDiagnosticsReport) -> String {
    format!(
        "{{\"model\":\"{}\",\"rules\":[{}]}}",
        model.model,
        model
            .rules
            .iter()
            .map(|rule| format!(
                "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":[{}],\"supporting_fragments\":[{}],\"missing_facts\":[{}]}}",
                rule.rule_index,
                rule.tier,
                rule.supported,
                string_json_list(&rule.required_facts),
                string_json_list(&rule.supporting_fragments),
                string_json_list(&rule.missing_facts),
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
            })
            .collect(),
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
}
