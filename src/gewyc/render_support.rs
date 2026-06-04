use super::*;

pub(super) fn string_json_list(items: &[String]) -> String {
    items
        .iter()
        .map(|item| json_string(item))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn u16_json_list(items: &[u16]) -> String {
    items
        .iter()
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn finding_text(finding: Option<&CompilerFinding>) -> String {
    match finding {
        Some(finding) => finding_text_record(finding),
        None => "none".into(),
    }
}

pub(super) fn finding_json(finding: Option<&CompilerFinding>) -> String {
    match finding {
        Some(finding) => finding_json_record(finding),
        None => "null".into(),
    }
}

pub(super) fn finding_text_record(finding: &CompilerFinding) -> String {
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

pub(super) fn finding_json_record(finding: &CompilerFinding) -> String {
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

pub(super) fn reason_profile_report(profile: &ReasonProfile) -> ReasonProfileReport {
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

pub(super) fn model_diagnostics_report(model: &ModelDiagnostics) -> ModelDiagnosticsReport {
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

pub(super) fn validation_report(
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

pub(super) fn diagnostics_stage_report(
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

pub(super) fn empty_validation_report() -> ValidationReport {
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

pub(super) fn findings_from_stage_reports(
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

pub(super) fn fragment_param_report(value: &FragmentParamValue) -> ParamValueReport {
    match value {
        FragmentParamValue::Bool(value) => ParamValueReport::Bool(*value),
        FragmentParamValue::U64(value) => ParamValueReport::U64(*value),
        FragmentParamValue::String(value) => ParamValueReport::String(value.clone()),
    }
}

pub(super) fn reason_profile_text(profile: &ReasonProfileReport) -> String {
    match profile {
        ReasonProfileReport::Builtin { id } => id.clone(),
        ReasonProfileReport::Declarative { id, rules } => {
            format!("declarative:{id} rules={rules}")
        }
    }
}

pub(super) fn reason_profile_json(profile: &ReasonProfileReport) -> String {
    match profile {
        ReasonProfileReport::Builtin { id } => {
            format!("{{\"kind\":\"builtin\",\"id\":{}}}", json_string(id))
        }
        ReasonProfileReport::Declarative { id, rules } => format!(
            "{{\"kind\":\"declarative\",\"id\":{},\"rules\":{}}}",
            json_string(id),
            rules
        ),
    }
}

pub(super) fn program_operation_text(operation: &ProgramOperation) -> &str {
    match operation {
        ProgramOperation::ConnectFlow => "connect_flow",
        ProgramOperation::DatagramExchange => "datagram_exchange",
        ProgramOperation::Custom(id) => id.as_str(),
        ProgramOperation::Unknown => "unknown",
    }
}

pub(super) fn fragment_param_text(value: &ParamValueReport) -> String {
    match value {
        ParamValueReport::Bool(value) => value.to_string(),
        ParamValueReport::U64(value) => value.to_string(),
        ParamValueReport::String(value) => value.clone(),
    }
}

pub(super) fn fragment_param_json(value: &ParamValueReport) -> String {
    match value {
        ParamValueReport::Bool(value) => value.to_string(),
        ParamValueReport::U64(value) => value.to_string(),
        ParamValueReport::String(value) => json_string(value),
    }
}

pub(super) fn evidence_tier_text(tier: &EvidenceTier) -> &'static str {
    match tier {
        EvidenceTier::CoreRequirement => "core_requirement",
        EvidenceTier::OptionalEnhancement => "optional_enhancement",
    }
}

pub(super) fn rule_tier_text(tier: &RuleTier) -> &'static str {
    match tier {
        RuleTier::CoreRequirement => "core_requirement",
        RuleTier::OptionalEnhancement => "optional_enhancement",
        RuleTier::Unsupported => "unsupported",
    }
}

pub(super) fn finding_from_dsl_error(err: &DslError) -> CompilerFinding {
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

pub(super) fn finding_from_registry_error(
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

pub(super) fn dsl_error_message(err: &DslError) -> String {
    match err {
        DslError::Located { inner, .. } => dsl_error_message(inner),
        DslError::InvalidLine(line) => format!("invalid line: {line}"),
        DslError::MissingField(field) => format!("missing field: {field}"),
        DslError::InvalidValue(value) => value.clone(),
        DslError::Registry(err) => format!("{err:?}"),
        DslError::Io(err) => err.clone(),
    }
}

pub(super) fn dsl_error_code(err: &DslError) -> &'static str {
    match err {
        DslError::Located { inner, .. } => dsl_error_code(inner),
        DslError::InvalidLine(_) => "GEWYC-PARSE-INVALID-LINE",
        DslError::MissingField(_) => "GEWYC-PARSE-MISSING-FIELD",
        DslError::InvalidValue(value) => dsl_invalid_value_code(value),
        DslError::Registry(_) => "GEWYC-PARSE-REGISTRY",
        DslError::Io(_) => "GEWYC-PARSE-IO",
    }
}

pub(super) fn dsl_invalid_value_code(value: &str) -> &'static str {
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

pub(super) fn registry_error_code(err: &RegistryError) -> &'static str {
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

pub(super) fn finding_stage_text(stage: CompilerFindingStage) -> &'static str {
    match stage {
        CompilerFindingStage::Parse => "parse",
        CompilerFindingStage::Validation => "validation",
        CompilerFindingStage::Diagnostics => "diagnostics",
    }
}

pub(super) fn finding_severity_text(severity: CompilerFindingSeverity) -> &'static str {
    match severity {
        CompilerFindingSeverity::Error => "error",
        CompilerFindingSeverity::Warning => "warning",
    }
}

pub(super) fn json_escape(value: &str) -> String {
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
