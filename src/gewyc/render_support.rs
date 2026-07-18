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

pub(super) fn findings_next_step_hint(report: &CompilerFindingsReport) -> &'static str {
    match report.findings.first().map(|finding| finding.stage) {
        Some(CompilerFindingStage::Parse) => {
            "fix the parse finding first, then rerun `gewyc findings` or `gewyc explain`"
        }
        Some(CompilerFindingStage::Validation) => {
            "inspect fragment coverage, rule evidence, or payload offsets, then rerun"
        }
        Some(CompilerFindingStage::Diagnostics) => {
            "inspect unsupported rules and missing facts in `gewyc diagnostics`, then rerun"
        }
        None => "findings are clear; continue with `gewyc explain` or runtime verification",
    }
}

pub(super) fn stages_finding_count(report: &CompilerStagesReport) -> usize {
    usize::from(report.parse.finding.is_some())
        + usize::from(report.validation.finding.is_some())
        + usize::from(report.diagnostics.finding.is_some())
}

pub(super) fn stages_next_step_hint(report: &CompilerStagesReport) -> &'static str {
    if report.parse.finding.is_some() {
        "fix the parse finding first, then rerun `gewyc stages` or `gewyc explain`"
    } else if report.validation.finding.is_some() {
        "inspect fragment coverage, rule evidence, or payload offsets, then rerun"
    } else if report.diagnostics.finding.is_some() {
        "inspect unsupported rules and missing facts in `gewyc diagnostics`, then rerun"
    } else {
        "parse, validation, and diagnostics are healthy; continue with `gewyc explain` or runtime verification"
    }
}

pub(super) fn envelope_next_step_hint(report: &CompilerEnvelope) -> &'static str {
    if !report.findings.findings.is_empty() {
        findings_next_step_hint(&report.findings)
    } else {
        stages_next_step_hint(&report.stages)
    }
}

pub(super) fn model_rule_support_counts(
    model: Option<&ModelDiagnosticsReport>,
) -> (usize, usize, usize) {
    match model {
        Some(model) => {
            let total = model.rules.len();
            let supported = model.rules.iter().filter(|rule| rule.supported).count();
            let unsupported = total.saturating_sub(supported);
            (total, supported, unsupported)
        }
        None => (0, 0, 0),
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
    if value.starts_with("unknown pipeline DSL step '") {
        "GEWYC-PARSE-UNKNOWN-PIPELINE-STEP"
    } else if value.starts_with("unknown pipeline function '") {
        "GEWYC-PARSE-UNKNOWN-PIPELINE-FUNCTION"
    } else if value.starts_with("unknown pipeline parameter kind '") {
        "GEWYC-PARSE-UNKNOWN-PARAMETER-KIND"
    } else if value.starts_with("pipeline parameter '")
        && (value.contains(" declares kind '") || value.contains(" is inferred inconsistently "))
    {
        "GEWYC-PARSE-PARAMETER-KIND-CONFLICT"
    } else if value.starts_with("unknown pipeline placeholder '$") {
        "GEWYC-PARSE-UNKNOWN-PLACEHOLDER"
    } else if value.starts_with("invalid pipeline placeholder '") {
        "GEWYC-PARSE-INVALID-PLACEHOLDER"
    } else if value.starts_with("invalid pipeline source literal '") {
        "GEWYC-PARSE-INVALID-LITERAL"
    } else if value.starts_with("pipeline string literal cannot interpolate placeholder '") {
        "GEWYC-PARSE-STRING-INTERPOLATION"
    } else if value.starts_with("unclosed pipeline placeholder in '") {
        "GEWYC-PARSE-UNCLOSED-PLACEHOLDER"
    } else if value.starts_with("pipeline placeholder expansion exceeded 32 substitutions while ") {
        "GEWYC-PARSE-PLACEHOLDER-EXPANSION-LIMIT"
    } else if value.starts_with("pipeline function call does not match ")
        && value.contains("unknown named parameter '")
    {
        "GEWYC-PARSE-UNKNOWN-NAMED-ARGUMENT"
    } else if value.contains("received both positional and named values")
        || value.starts_with("pipeline step 'use' received duplicate named argument '")
    {
        "GEWYC-PARSE-DUPLICATE-ARGUMENT"
    } else if value.contains("cannot place positional arguments after named arguments") {
        "GEWYC-PARSE-ARGUMENT-ORDER"
    } else if value.starts_with("pipeline function call does not match ")
        && (value.contains(": expected ") || value.contains(": missing required parameter '"))
    {
        "GEWYC-PARSE-FUNCTION-ARITY"
    } else if value.starts_with("invalid function signature '") {
        "GEWYC-PARSE-INVALID-FUNCTION-SIGNATURE"
    } else if value.starts_with("invalid pipeline function name '") {
        "GEWYC-PARSE-INVALID-FUNCTION-NAME"
    } else if value.starts_with("duplicate pipeline function '") {
        "GEWYC-PARSE-DUPLICATE-FUNCTION"
    } else if value.starts_with("duplicate pipeline parameter '") {
        "GEWYC-PARSE-DUPLICATE-PARAMETER"
    } else if value.starts_with("duplicate pipeline local binding '") {
        "GEWYC-PARSE-DUPLICATE-LOCAL-BINDING"
    } else if value.starts_with("pipeline function required parameter '")
        && value.ends_with(" cannot follow a defaulted parameter")
    {
        "GEWYC-PARSE-INVALID-PARAMETER-ORDER"
    } else if value.starts_with("pipeline parameter '")
        && value.ends_with(" requires a default value after '='")
    {
        "GEWYC-PARSE-MISSING-PARAMETER-DEFAULT"
    } else if value == "pipeline parameter name cannot be empty"
        || value.starts_with("invalid pipeline parameter name '")
    {
        "GEWYC-PARSE-INVALID-PARAMETER-NAME"
    } else if value == "unclosed pipeline string literal" {
        "GEWYC-PARSE-UNCLOSED-STRING"
    } else if value.starts_with("invalid pipeline string escape '")
        || value == "pipeline string literal cannot end with an escape"
    {
        "GEWYC-PARSE-INVALID-STRING-ESCAPE"
    } else if value.starts_with("invalid pipeline string character U+") {
        "GEWYC-PARSE-INVALID-STRING-CHARACTER"
    } else if value.starts_with("invalid pipeline atom '") {
        "GEWYC-PARSE-INVALID-ATOM"
    } else if value.starts_with("pipeline step '")
        && value.contains(" received invalid keyword field name '")
    {
        "GEWYC-PARSE-INVALID-KEYWORD-NAME"
    } else if value.starts_with("invalid let binding '")
        || value.starts_with("pipeline let binding '")
    {
        "GEWYC-PARSE-INVALID-LET-BINDING"
    } else if value.starts_with("invalid pipeline call 'template")
        || value == "pipeline step 'template' expects exactly one argument"
    {
        "GEWYC-PARSE-INVALID-TEMPLATE-HEAD"
    } else if value.starts_with("invalid pipeline call '") {
        "GEWYC-PARSE-INVALID-PIPELINE-CALL"
    } else if value.starts_with("pipeline rule received unknown field '") {
        "GEWYC-PARSE-UNKNOWN-RULE-FIELD"
    } else if value.starts_with("pipeline rule received duplicate field '")
        || value.contains("received duplicate rule field")
    {
        "GEWYC-PARSE-DUPLICATE-RULE-FIELD"
    } else if value.starts_with("pipeline step 'window' received duplicate field '") {
        "GEWYC-PARSE-DUPLICATE-WINDOW-FIELD"
    } else if value.starts_with("pipeline step 'window' received unknown field '") {
        "GEWYC-PARSE-UNKNOWN-WINDOW-FIELD"
    } else if value.starts_with("unknown reason profile '") {
        "GEWYC-PARSE-UNKNOWN-REASON-PROFILE"
    } else if value.starts_with("unknown stage '") {
        "GEWYC-PARSE-UNKNOWN-STAGE"
    } else if value.starts_with("unknown reason key event '") {
        "GEWYC-PARSE-UNKNOWN-KEY-EVENT"
    } else if value.starts_with("unknown evidence fact kind '") {
        "GEWYC-PARSE-UNKNOWN-EVIDENCE-FACT-KIND"
    } else if value.starts_with("unknown evidence tier '") {
        "GEWYC-PARSE-UNKNOWN-EVIDENCE-TIER"
    } else if value.starts_with("invalid param target '") {
        "GEWYC-PARSE-INVALID-FRAGMENT-PARAM-TARGET"
    } else if value.starts_with("unknown window profile '") {
        "GEWYC-PARSE-UNKNOWN-WINDOW-PROFILE"
    } else if value.starts_with("invalid bool '") {
        "GEWYC-PARSE-INVALID-BOOLEAN"
    } else if value.starts_with("invalid u64 for '") {
        "GEWYC-PARSE-INVALID-INTEGER"
    } else if value.starts_with("pipeline step '")
        && (value.contains(" expects exactly one argument")
            || value.contains(" expects at least one argument")
            || value.contains(" expects target and value")
            || value.contains(" expects fact kind and tier")
            || value.contains(" positional shorthand expects exactly ")
            || value.contains(" positional shorthand accepts at most "))
    {
        "GEWYC-PARSE-INVALID-STEP-ARITY"
    } else if (value.starts_with("pipeline step '")
        && (value.contains(" expected keyword argument, got '")
            || value.contains(" expected named argument, got '")
            || value.contains(" named argument '") && value.ends_with(" requires a value")))
        || (value.starts_with("pipeline keyword argument '")
            && value.ends_with(" requires a value"))
    {
        "GEWYC-PARSE-MALFORMED-ARGUMENT"
    } else if value == "pipeline argument list contains an empty argument" {
        "GEWYC-PARSE-EMPTY-ARGUMENT"
    } else if value == "unclosed pipeline block comment" {
        "GEWYC-PARSE-UNCLOSED-BLOCK-COMMENT"
    } else if value == "pipeline let binding contains multiple assignment separators"
        || value == "pipeline parameter contains multiple default separators"
    {
        "GEWYC-PARSE-MULTIPLE-ASSIGNMENT-SEPARATORS"
    } else if value.starts_with("pipeline rule phase '") && value.ends_with(" requires module") {
        "GEWYC-PARSE-RULE-PHASE-WITHOUT-MODULE"
    } else if value == "pipeline DSL must start with template(...) or template :name" {
        "GEWYC-PARSE-MISSING-TEMPLATE-HEAD"
    } else if value == "pipeline DSL supports exactly one template head" {
        "GEWYC-PARSE-DUPLICATE-TEMPLATE-HEAD"
    } else if value == "pipeline DSL steps after template must start with '|>'" {
        "GEWYC-PARSE-MISSING-PIPELINE-PREFIX"
    } else if value.starts_with("pipeline include cycle detected at '") {
        "GEWYC-PARSE-INCLUDE-CYCLE"
    } else if value.starts_with("pipeline use cycle detected at function '") {
        "GEWYC-PARSE-USE-CYCLE"
    } else if value.starts_with("unknown package source '") {
        "GEWYC-PARSE-UNKNOWN-PACKAGE-SOURCE"
    } else if value.starts_with("invalid source dependency '") {
        "GEWYC-PARSE-INVALID-SOURCE-DEPENDENCY"
    } else if value.starts_with("included path '") && value.contains(" escapes package root '") {
        "GEWYC-PARSE-INCLUDE-ESCAPES-PACKAGE"
    } else if value == "gewylang now only supports the pipeline stable subset" {
        "GEWYC-PARSE-UNSUPPORTED-SYNTAX"
    } else if value.starts_with("gewylang source exceeds ") && value.ends_with(" bytes") {
        "GEWYC-PARSE-SOURCE-TOO-LARGE"
    } else if value.starts_with("unknown datagram proto '")
        || value.starts_with("unknown packet proto '")
    {
        "GEWYC-PARSE-UNKNOWN-TRANSPORT-PROTOCOL"
    } else if value.starts_with("unknown predicate '") {
        "GEWYC-PARSE-UNKNOWN-PREDICATE"
    } else if (value.starts_with("missing ") && value.ends_with(" qualifier"))
        || (value.contains(" requires a ") && value.ends_with(" qualifier"))
    {
        "GEWYC-PARSE-MISSING-PREDICATE-QUALIFIER"
    } else if value.starts_with("unknown QUIC ")
        || (value.starts_with("unknown ")
            && (value.contains(" port '")
                || value.contains(" predicate suffix '")
                || value.contains(" state qualifier '")))
        || (value.starts_with("unexpected ") && value.contains(" suffix '"))
        || (value.starts_with("invalid ")
            && [
                "packet_observed",
                "datagram_observed",
                "quic_packet_observed",
                "quic_frame_observed",
                "socket_state_observed",
            ]
            .iter()
            .any(|predicate| value.contains(predicate)))
    {
        "GEWYC-PARSE-INVALID-PREDICATE-QUALIFIER"
    } else if value.contains(" expects ")
        && (value.contains("-compatible value") || value.contains("atom-like identifier value"))
    {
        "GEWYC-PARSE-ARGUMENT-TYPE-MISMATCH"
    } else if value.starts_with("unknown package dependency '") {
        "GEWYC-PARSE-UNKNOWN-PACKAGE-DEPENDENCY"
    } else if value == "pipeline include() requires a filesystem-backed entry file" {
        "GEWYC-PARSE-INCLUDE-NONFILESYSTEM-ENTRY"
    } else if value == "pipeline include() should be resolved before lowering" {
        "GEWYC-PARSE-UNRESOLVED-INCLUDE"
    } else if value == "pipeline function bodies must contain '|>' steps"
        || value == "pipeline function bodies may only contain 'let' bindings or '|>' steps"
        || value == "pipeline function expressions must contain '|>' steps"
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
