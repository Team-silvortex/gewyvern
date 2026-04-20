use crate::dsl::{DslError, compile_file};
use crate::flow::ProgramOperation;
use crate::fragment::{BindingDiagnostics, EvidenceTier, ModelDiagnostics, RegistryError, RuleTier, builtin_registry};
use crate::ledger::FactKindTag;
use crate::reason::ReasonProfile;
use crate::template::{FragmentParamValue, TemplateBinding};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderFormat {
    Text,
    Json,
}

pub fn compile_binding_file(path: &str) -> Result<TemplateBinding, DslError> {
    compile_file(path)
}

pub fn collect_binding_diagnostics(
    binding: &TemplateBinding,
) -> Result<BindingDiagnostics, RegistryError> {
    builtin_registry().binding_diagnostics(binding)
}

pub fn render_binding(binding: &TemplateBinding, format: RenderFormat) -> String {
    match format {
        RenderFormat::Text => binding_text(binding),
        RenderFormat::Json => binding_json(binding),
    }
}

pub fn render_diagnostics(
    binding: &TemplateBinding,
    diagnostics: &BindingDiagnostics,
    format: RenderFormat,
) -> String {
    match format {
        RenderFormat::Text => diagnostics_text(binding, diagnostics),
        RenderFormat::Json => diagnostics_json(binding, diagnostics),
    }
}

fn binding_text(binding: &TemplateBinding) -> String {
    let mut lines = vec![
        format!("template={}", binding.template.id),
        format!("fragments={}", binding.template.fragment_set.join(",")),
    ];

    if let Some(window) = &binding.template.window_profile {
        lines.push(format!(
            "window={} duration_ms={} lateness_ms={}",
            window.id, window.duration_ms, window.lateness_ms
        ));
    }

    if let Some(reason) = &binding.template.reason_profile {
        lines.push(format!("reason={}", reason_profile_text(reason)));
    }

    if let Some(model) = &binding.template.program_model {
        lines.push(format!(
            "program_model={} operation={} rules={}",
            model.id,
            program_operation_text(&model.operation),
            model.rules.len()
        ));
    }

    for (fragment, params) in &binding.fragment_params {
        for (key, value) in params {
            lines.push(format!(
                "param={fragment}.{key}={}",
                fragment_param_text(value)
            ));
        }
    }

    for (fact_kind, tier) in &binding.evidence_overrides {
        lines.push(format!("evidence={fact_kind}:{}", evidence_tier_text(tier)));
    }

    lines.join("\n")
}

fn binding_json(binding: &TemplateBinding) -> String {
    let fragment_params = binding
        .fragment_params
        .iter()
        .map(|(fragment, params)| {
            format!(
                "\"{fragment}\":{{{}}}",
                params
                    .iter()
                    .map(|(key, value)| format!("\"{key}\":{}", fragment_param_json(value)))
                    .collect::<Vec<_>>()
                    .join(",")
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let evidence_overrides = binding
        .evidence_overrides
        .iter()
        .map(|(fact_kind, tier)| format!("\"{fact_kind}\":\"{}\"", evidence_tier_text(tier)))
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
        binding.template.id,
        binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        binding
            .template
            .window_profile
            .as_ref()
            .map_or("null".into(), |window| format!(
                "{{\"id\":\"{}\",\"duration_ms\":{},\"lateness_ms\":{}}}",
                window.id, window.duration_ms, window.lateness_ms
            )),
        binding
            .template
            .reason_profile
            .as_ref()
            .map_or("null".into(), reason_profile_json),
        binding
            .template
            .program_model
            .as_ref()
            .map_or("null".into(), |model| format!(
                "{{\"id\":\"{}\",\"operation\":\"{}\",\"rules\":{}}}",
                model.id,
                program_operation_text(&model.operation),
                model.rules.len()
            )),
        fragment_params,
        evidence_overrides
    )
}

fn diagnostics_text(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    let mut lines = vec![
        format!("template={}", binding.template.id),
        format!("fragments={}", binding.template.fragment_set.join(",")),
    ];

    if let Some(model) = &diagnostics.program_model {
        lines.push(format!("program_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    if let Some(model) = &diagnostics.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    lines.join("\n")
}

fn diagnostics_json(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    format!(
        concat!(
            "{{",
            "\"template_id\":\"{}\",",
            "\"fragments\":[{}],",
            "\"program_model\":{},",
            "\"reason_model\":{}",
            "}}"
        ),
        binding.template.id,
        binding
            .template
            .fragment_set
            .iter()
            .map(|fragment| format!("\"{fragment}\""))
            .collect::<Vec<_>>()
            .join(","),
        diagnostics
            .program_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
        diagnostics
            .reason_model
            .as_ref()
            .map_or("null".into(), model_diagnostics_json),
    )
}

fn model_diagnostics_json(model: &ModelDiagnostics) -> String {
    format!(
        "{{\"model\":\"{}\",\"rules\":[{}]}}",
        model.model,
        model
            .rules
            .iter()
            .map(|rule| format!(
                "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":[{}],\"supporting_fragments\":[{}],\"missing_facts\":[{}]}}",
                rule.rule_index,
                rule_tier_text(&rule.tier),
                rule.supported,
                fact_tag_json_list(&rule.required_facts),
                string_json_list(&rule.supporting_fragments),
                fact_tag_json_list(&rule.missing_facts),
            ))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn fact_tag_json_list(items: &[FactKindTag]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn string_json_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",")
}

fn reason_profile_text(profile: &ReasonProfile) -> String {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => profile.id().into(),
        ReasonProfile::Declarative(model) => {
            format!("declarative:{} rules={}", model.id, model.rules.len())
        }
    }
}

fn reason_profile_json(profile: &ReasonProfile) -> String {
    match profile {
        ReasonProfile::HandshakeL1 | ReasonProfile::UdpDatagramL1 => {
            format!("{{\"kind\":\"builtin\",\"id\":\"{}\"}}", profile.id())
        }
        ReasonProfile::Declarative(model) => format!(
            "{{\"kind\":\"declarative\",\"id\":\"{}\",\"rules\":{}}}",
            model.id,
            model.rules.len()
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

fn fragment_param_text(value: &FragmentParamValue) -> String {
    match value {
        FragmentParamValue::Bool(value) => value.to_string(),
        FragmentParamValue::U64(value) => value.to_string(),
        FragmentParamValue::String(value) => value.clone(),
    }
}

fn fragment_param_json(value: &FragmentParamValue) -> String {
    match value {
        FragmentParamValue::Bool(value) => value.to_string(),
        FragmentParamValue::U64(value) => value.to_string(),
        FragmentParamValue::String(value) => format!("\"{value}\""),
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
}
