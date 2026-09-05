use crate::{
    BindingReport, CompilerStageProjections, DiagnosticsReport, IrModelReport, IrReport,
    IrRuleReport, ModelDiagnosticsReport, ReasonProfileReport, RuleDiagnosticsReport,
};
use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

/// Version of the structural and cross-stage invariant rules in this module.
pub const IR_INVARIANT_CONTRACT_VERSION: u32 = 1;
/// Maximum violations retained from one validation pass.
pub const MAX_IR_INVARIANT_VIOLATIONS: usize = 256;
/// Maximum UTF-8 byte length retained for one violation detail.
pub const MAX_IR_INVARIANT_DETAIL_BYTES: usize = 512;

/// Stable machine-readable classes for malformed or incoherent IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum IrInvariantCode {
    EmptyValue,
    DuplicateValue,
    UnknownFragment,
    InvalidTier,
    ModelKindMismatch,
    OperationShapeMismatch,
    RuleIndexMismatch,
    PhaseShapeMismatch,
    MissingFactNotRequired,
    SupportStateMismatch,
    StageIdentityMismatch,
    StageShapeMismatch,
    StageRuleMismatch,
}

impl IrInvariantCode {
    /// Stable identifier suitable for logs, wire errors, and policy rules.
    pub const fn id(self) -> &'static str {
        match self {
            Self::EmptyValue => "GEWYLANG-IR-EMPTY-VALUE",
            Self::DuplicateValue => "GEWYLANG-IR-DUPLICATE-VALUE",
            Self::UnknownFragment => "GEWYLANG-IR-UNKNOWN-FRAGMENT",
            Self::InvalidTier => "GEWYLANG-IR-INVALID-TIER",
            Self::ModelKindMismatch => "GEWYLANG-IR-MODEL-KIND-MISMATCH",
            Self::OperationShapeMismatch => "GEWYLANG-IR-OPERATION-SHAPE-MISMATCH",
            Self::RuleIndexMismatch => "GEWYLANG-IR-RULE-INDEX-MISMATCH",
            Self::PhaseShapeMismatch => "GEWYLANG-IR-PHASE-SHAPE-MISMATCH",
            Self::MissingFactNotRequired => "GEWYLANG-IR-MISSING-FACT-NOT-REQUIRED",
            Self::SupportStateMismatch => "GEWYLANG-IR-SUPPORT-STATE-MISMATCH",
            Self::StageIdentityMismatch => "GEWYLANG-IR-STAGE-IDENTITY-MISMATCH",
            Self::StageShapeMismatch => "GEWYLANG-IR-STAGE-SHAPE-MISMATCH",
            Self::StageRuleMismatch => "GEWYLANG-IR-STAGE-RULE-MISMATCH",
        }
    }
}

impl fmt::Display for IrInvariantCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.id())
    }
}

/// One deterministic structural or cross-stage invariant violation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrInvariantViolation {
    pub code: IrInvariantCode,
    pub path: String,
    pub detail: String,
}

/// Bounded validation result. Validation accumulates deterministic findings
/// instead of stopping at the first malformed field.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IrValidationReport {
    pub violations: Vec<IrInvariantViolation>,
    pub truncated: bool,
}

impl IrValidationReport {
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty() && !self.truncated
    }

    pub fn into_result(self) -> Result<(), IrValidationErrors> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(IrValidationErrors {
                violations: self.violations,
                truncated: self.truncated,
            })
        }
    }

    fn push(&mut self, code: IrInvariantCode, path: impl Into<String>, detail: impl Into<String>) {
        if self.violations.len() >= MAX_IR_INVARIANT_VIOLATIONS {
            self.truncated = true;
            return;
        }
        self.violations.push(IrInvariantViolation {
            code,
            path: path.into(),
            detail: bounded_detail(detail.into()),
        });
    }

    fn extend_prefixed(&mut self, prefix: &str, report: Self) {
        for mut violation in report.violations {
            if self.violations.len() >= MAX_IR_INVARIANT_VIOLATIONS {
                self.truncated = true;
                break;
            }
            violation.path = format!("{prefix}.{}", violation.path);
            self.violations.push(violation);
        }
        self.truncated |= report.truncated;
    }
}

/// Fail-closed error returned by checked projections and wire decoders.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrValidationErrors {
    pub violations: Vec<IrInvariantViolation>,
    pub truncated: bool,
}

impl IrValidationErrors {
    pub fn first(&self) -> Option<&IrInvariantViolation> {
        self.violations.first()
    }
}

impl fmt::Display for IrValidationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.first() {
            Some(first) if self.violations.len() == 1 && !self.truncated => {
                write!(
                    formatter,
                    "{} at {}: {}",
                    first.code, first.path, first.detail
                )
            }
            Some(first) if self.truncated => write!(
                formatter,
                "at least {} IR invariant violations; first is {} at {}: {}",
                self.violations.len(),
                first.code,
                first.path,
                first.detail
            ),
            Some(first) => write!(
                formatter,
                "{} IR invariant violations; first is {} at {}: {}",
                self.violations.len(),
                first.code,
                first.path,
                first.detail
            ),
            None => formatter.write_str("IR validation error without violations"),
        }
    }
}

impl Error for IrValidationErrors {}

impl BindingReport {
    /// Validates one Binding IR value without consulting a product registry.
    pub fn validate_invariants(&self) -> IrValidationReport {
        validate_binding_ir(self)
    }
}

impl DiagnosticsReport {
    /// Validates one diagnostics projection independently of its host.
    pub fn validate_invariants(&self) -> IrValidationReport {
        validate_diagnostics_ir(self)
    }
}

impl IrReport {
    /// Validates one Analysis IR value independently of its host.
    pub fn validate_invariants(&self) -> IrValidationReport {
        validate_analysis_ir(self)
    }
}

impl<E> CompilerStageProjections<E> {
    /// Validates each available stage and all observable cross-stage links.
    pub fn validate_invariants(&self) -> IrValidationReport {
        validate_compiler_stages(self)
    }
}

pub fn validate_binding_ir(binding: &BindingReport) -> IrValidationReport {
    let mut report = IrValidationReport::default();
    require_text(&mut report, "template_id", &binding.template_id);
    validate_text_list(&mut report, "fragments", &binding.fragments);

    if let Some(window) = &binding.window {
        require_text(&mut report, "window.id", &window.id);
    }
    if let Some(reason) = &binding.reason_profile {
        let id = match reason {
            ReasonProfileReport::Builtin { id } | ReasonProfileReport::Declarative { id, .. } => id,
        };
        require_text(&mut report, "reason_profile.id", id);
    }
    if let Some(model) = &binding.program_model {
        require_text(&mut report, "program_model.id", &model.id);
        require_text(&mut report, "program_model.operation", &model.operation);
    }

    let fragments = binding
        .fragments
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut params = BTreeSet::new();
    for (index, param) in binding.fragment_params.iter().enumerate() {
        let base = format!("fragment_params[{index}]");
        require_text(&mut report, format!("{base}.fragment"), &param.fragment);
        require_text(&mut report, format!("{base}.key"), &param.key);
        if !param.fragment.trim().is_empty() && !fragments.contains(param.fragment.as_str()) {
            report.push(
                IrInvariantCode::UnknownFragment,
                format!("{base}.fragment"),
                format!("fragment '{}' is not present in fragments", param.fragment),
            );
        }
        if !params.insert((param.fragment.as_str(), param.key.as_str())) {
            report.push(
                IrInvariantCode::DuplicateValue,
                base,
                format!(
                    "fragment parameter '{}.{}' appears more than once",
                    param.fragment, param.key
                ),
            );
        }
    }

    let mut evidence_kinds = BTreeSet::new();
    for (index, evidence) in binding.evidence_overrides.iter().enumerate() {
        let base = format!("evidence_overrides[{index}]");
        require_text(
            &mut report,
            format!("{base}.fact_kind"),
            &evidence.fact_kind,
        );
        require_text(&mut report, format!("{base}.tier"), &evidence.tier);
        if !matches!(
            evidence.tier.as_str(),
            "core_requirement" | "optional_enhancement"
        ) {
            report.push(
                IrInvariantCode::InvalidTier,
                format!("{base}.tier"),
                format!("unknown evidence tier '{}'", evidence.tier),
            );
        }
        if !evidence_kinds.insert(evidence.fact_kind.as_str()) {
            report.push(
                IrInvariantCode::DuplicateValue,
                base,
                format!(
                    "evidence override for '{}' appears more than once",
                    evidence.fact_kind
                ),
            );
        }
    }
    report
}

pub fn validate_diagnostics_ir(diagnostics: &DiagnosticsReport) -> IrValidationReport {
    let mut report = IrValidationReport::default();
    require_text(&mut report, "template_id", &diagnostics.template_id);
    validate_text_list(&mut report, "fragments", &diagnostics.fragments);
    let fragments = diagnostics
        .fragments
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if let Some(model) = &diagnostics.program_model {
        validate_diagnostics_model(&mut report, "program_model", model, &fragments);
    }
    if let Some(model) = &diagnostics.reason_model {
        validate_diagnostics_model(&mut report, "reason_model", model, &fragments);
    }
    report
}

pub fn validate_analysis_ir(analysis: &IrReport) -> IrValidationReport {
    let mut report = IrValidationReport::default();
    require_text(&mut report, "template_id", &analysis.template_id);
    if let Some(model) = &analysis.program_model {
        validate_analysis_model(&mut report, "program_model", model, "program_model", true);
    }
    if let Some(model) = &analysis.reason_model {
        validate_analysis_model_kind(&mut report, "reason_model", model);
        validate_analysis_model(&mut report, "reason_model", model, &model.kind, false);
    }
    report
}

/// Validates a Binding/Analysis pair without requiring host diagnostics.
///
/// This is the boundary used by independently persisted IR snapshots: each
/// stage must be structurally valid, and the pair must describe one coherent
/// compile before a cross-snapshot operation such as diffing is allowed.
pub fn validate_binding_analysis_ir(
    binding: &BindingReport,
    analysis: &IrReport,
) -> IrValidationReport {
    let mut report = IrValidationReport::default();
    report.extend_prefixed("binding", validate_binding_ir(binding));
    report.extend_prefixed("analysis", validate_analysis_ir(analysis));
    validate_binding_analysis_links(&mut report, binding, analysis);
    report
}

pub fn validate_compiler_stages<E>(
    projections: &CompilerStageProjections<E>,
) -> IrValidationReport {
    let mut report = IrValidationReport::default();
    report.extend_prefixed("binding", validate_binding_ir(&projections.binding));
    report.extend_prefixed("analysis", validate_analysis_ir(&projections.analysis));
    if let Ok(diagnostics) = &projections.diagnostics {
        report.extend_prefixed("diagnostics", validate_diagnostics_ir(diagnostics));
    }

    validate_binding_analysis_links(&mut report, &projections.binding, &projections.analysis);

    if let Ok(diagnostics) = &projections.diagnostics {
        validate_diagnostics_stage_shape(
            &mut report,
            &projections.binding,
            diagnostics,
            &projections.analysis,
        );
    }
    report
}

fn validate_binding_analysis_links(
    report: &mut IrValidationReport,
    binding: &BindingReport,
    analysis: &IrReport,
) {
    if binding.template_id != analysis.template_id {
        report.push(
            IrInvariantCode::StageIdentityMismatch,
            "analysis.template_id",
            format!(
                "expected binding template_id '{}', found '{}'",
                binding.template_id, analysis.template_id
            ),
        );
    }
    validate_program_stage_shape(report, binding, analysis);
    validate_reason_stage_shape(report, binding, analysis);
    validate_analysis_fragment_references(report, binding, analysis);
}

fn validate_diagnostics_model(
    report: &mut IrValidationReport,
    path: &str,
    model: &ModelDiagnosticsReport,
    fragments: &BTreeSet<&str>,
) {
    require_text(report, format!("{path}.model"), &model.model);
    for (position, rule) in model.rules.iter().enumerate() {
        let rule_path = format!("{path}.rules[{position}]");
        validate_rule_index(report, &rule_path, position, rule.rule_index);
        require_text(report, format!("{rule_path}.tier"), &rule.tier);
        if !matches!(
            rule.tier.as_str(),
            "core_requirement" | "optional_enhancement" | "unsupported"
        ) {
            report.push(
                IrInvariantCode::InvalidTier,
                format!("{rule_path}.tier"),
                format!("unknown diagnostic tier '{}'", rule.tier),
            );
        }
        if rule.supported == (rule.tier == "unsupported") {
            report.push(
                IrInvariantCode::SupportStateMismatch,
                format!("{rule_path}.tier"),
                "supported rules cannot use the unsupported tier and unsupported rules must use it",
            );
        }
        validate_support_shape(
            report,
            &rule_path,
            rule.supported,
            &rule.required_facts,
            &rule.supporting_fragments,
            &rule.missing_facts,
            &rule.unsupported_payload_offsets,
        );
        for (index, fragment) in rule.supporting_fragments.iter().enumerate() {
            if !fragment.trim().is_empty() && !fragments.contains(fragment.as_str()) {
                report.push(
                    IrInvariantCode::UnknownFragment,
                    format!("{rule_path}.supporting_fragments[{index}]"),
                    format!("fragment '{fragment}' is not present in diagnostics.fragments"),
                );
            }
        }
    }
}

fn validate_analysis_model_kind(
    report: &mut IrValidationReport,
    path: &str,
    model: &IrModelReport,
) {
    if !matches!(
        model.kind.as_str(),
        "builtin_reason_profile" | "declarative_reason_model"
    ) {
        report.push(
            IrInvariantCode::ModelKindMismatch,
            format!("{path}.kind"),
            format!("unknown reason model kind '{}'", model.kind),
        );
    }
}

fn validate_analysis_model(
    report: &mut IrValidationReport,
    path: &str,
    model: &IrModelReport,
    expected_kind: &str,
    expects_operation: bool,
) {
    require_text(report, format!("{path}.kind"), &model.kind);
    require_text(report, format!("{path}.id"), &model.id);
    if model.kind != expected_kind {
        report.push(
            IrInvariantCode::ModelKindMismatch,
            format!("{path}.kind"),
            format!("expected '{expected_kind}', found '{}'", model.kind),
        );
    }
    match (&model.operation, expects_operation) {
        (Some(operation), true) => {
            require_text(report, format!("{path}.operation"), operation);
        }
        (None, true) => report.push(
            IrInvariantCode::OperationShapeMismatch,
            format!("{path}.operation"),
            "program models require an operation",
        ),
        (Some(_), false) => report.push(
            IrInvariantCode::OperationShapeMismatch,
            format!("{path}.operation"),
            "reason models cannot carry an operation",
        ),
        (None, false) => {}
    }
    for (position, rule) in model.rules.iter().enumerate() {
        validate_analysis_rule(report, &format!("{path}.rules[{position}]"), position, rule);
    }
}

fn validate_analysis_rule(
    report: &mut IrValidationReport,
    path: &str,
    position: usize,
    rule: &IrRuleReport,
) {
    validate_rule_index(report, path, position, rule.rule_index);
    require_text(report, format!("{path}.predicate"), &rule.predicate);
    require_text(report, format!("{path}.narrative"), &rule.narrative);
    require_optional_text(report, format!("{path}.signal"), rule.signal.as_deref());
    require_optional_text(report, format!("{path}.module"), rule.module.as_deref());
    require_optional_text(report, format!("{path}.phase"), rule.phase.as_deref());
    require_optional_text(
        report,
        format!("{path}.phase_kind"),
        rule.phase_kind.as_deref(),
    );
    if rule.phase_kind.is_some() && (rule.phase.is_none() || rule.signal.is_none()) {
        report.push(
            IrInvariantCode::PhaseShapeMismatch,
            format!("{path}.phase_kind"),
            "phase_kind requires both phase and signal",
        );
    }
    validate_support_shape(
        report,
        path,
        rule.supported,
        &rule.required_facts,
        &rule.supporting_fragments,
        &rule.missing_facts,
        &rule.unsupported_payload_offsets,
    );
}

fn validate_support_shape(
    report: &mut IrValidationReport,
    path: &str,
    supported: bool,
    required_facts: &[String],
    supporting_fragments: &[String],
    missing_facts: &[String],
    unsupported_payload_offsets: &[u16],
) {
    validate_text_list(report, &format!("{path}.required_facts"), required_facts);
    validate_text_list(
        report,
        &format!("{path}.supporting_fragments"),
        supporting_fragments,
    );
    validate_text_list(report, &format!("{path}.missing_facts"), missing_facts);
    validate_u16_list(
        report,
        &format!("{path}.unsupported_payload_offsets"),
        unsupported_payload_offsets,
    );
    let required = required_facts
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for (index, fact) in missing_facts.iter().enumerate() {
        if !fact.trim().is_empty() && !required.contains(fact.as_str()) {
            report.push(
                IrInvariantCode::MissingFactNotRequired,
                format!("{path}.missing_facts[{index}]"),
                format!("missing fact '{fact}' is absent from required_facts"),
            );
        }
    }
    let expected_supported = missing_facts.is_empty() && unsupported_payload_offsets.is_empty();
    if supported != expected_supported {
        report.push(
            IrInvariantCode::SupportStateMismatch,
            format!("{path}.supported"),
            format!("expected {expected_supported} from missing facts and unsupported offsets"),
        );
    }
}

fn validate_program_stage_shape(
    report: &mut IrValidationReport,
    binding: &BindingReport,
    analysis: &IrReport,
) {
    match (&binding.program_model, &analysis.program_model) {
        (Some(binding_model), Some(analysis_model)) => {
            compare_text(
                report,
                "analysis.program_model.id",
                &binding_model.id,
                &analysis_model.id,
            );
            compare_option_text(
                report,
                "analysis.program_model.operation",
                Some(binding_model.operation.as_str()),
                analysis_model.operation.as_deref(),
            );
            compare_count(
                report,
                "analysis.program_model.rules",
                binding_model.rules,
                analysis_model.rules.len(),
            );
        }
        (None, None) => {}
        _ => report.push(
            IrInvariantCode::StageShapeMismatch,
            "analysis.program_model",
            "program model presence differs from Binding IR",
        ),
    }
}

fn validate_reason_stage_shape(
    report: &mut IrValidationReport,
    binding: &BindingReport,
    analysis: &IrReport,
) {
    match (&binding.reason_profile, &analysis.reason_model) {
        (Some(ReasonProfileReport::Builtin { id }), Some(model)) => {
            compare_text(
                report,
                "analysis.reason_model.kind",
                "builtin_reason_profile",
                &model.kind,
            );
            compare_text(report, "analysis.reason_model.id", id, &model.id);
            compare_count(report, "analysis.reason_model.rules", 0, model.rules.len());
        }
        (Some(ReasonProfileReport::Declarative { id, rules }), Some(model)) => {
            compare_text(
                report,
                "analysis.reason_model.kind",
                "declarative_reason_model",
                &model.kind,
            );
            compare_text(report, "analysis.reason_model.id", id, &model.id);
            compare_count(
                report,
                "analysis.reason_model.rules",
                *rules,
                model.rules.len(),
            );
        }
        (None, None) => {}
        _ => report.push(
            IrInvariantCode::StageShapeMismatch,
            "analysis.reason_model",
            "reason model presence differs from Binding IR",
        ),
    }
}

fn validate_analysis_fragment_references(
    report: &mut IrValidationReport,
    binding: &BindingReport,
    analysis: &IrReport,
) {
    let fragments = binding
        .fragments
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    for (label, model) in analysis.model_entries() {
        for (rule_position, rule) in model.rules.iter().enumerate() {
            for (fragment_position, fragment) in rule.supporting_fragments.iter().enumerate() {
                if !fragment.trim().is_empty() && !fragments.contains(fragment.as_str()) {
                    report.push(
                        IrInvariantCode::UnknownFragment,
                        format!(
                            "analysis.{label}.rules[{rule_position}].supporting_fragments[{fragment_position}]"
                        ),
                        format!("fragment '{fragment}' is not present in Binding IR"),
                    );
                }
            }
        }
    }
}

fn validate_diagnostics_stage_shape(
    report: &mut IrValidationReport,
    binding: &BindingReport,
    diagnostics: &DiagnosticsReport,
    analysis: &IrReport,
) {
    compare_text(
        report,
        "diagnostics.template_id",
        &binding.template_id,
        &diagnostics.template_id,
    );
    if binding.fragments != diagnostics.fragments {
        report.push(
            IrInvariantCode::StageShapeMismatch,
            "diagnostics.fragments",
            "fragment sequence differs from Binding IR",
        );
    }
    validate_diagnostics_model_link(
        report,
        "program_model",
        binding
            .program_model
            .as_ref()
            .map(|model| (model.id.as_str(), model.rules)),
        diagnostics.program_model.as_ref(),
        analysis.program_model.as_ref(),
    );
    let expected_reason = match &binding.reason_profile {
        Some(ReasonProfileReport::Declarative { id, rules }) => Some((id.as_str(), *rules)),
        _ => None,
    };
    let analysis_reason = match &binding.reason_profile {
        Some(ReasonProfileReport::Declarative { .. }) => analysis.reason_model.as_ref(),
        _ => None,
    };
    validate_diagnostics_model_link(
        report,
        "reason_model",
        expected_reason,
        diagnostics.reason_model.as_ref(),
        analysis_reason,
    );
}

fn validate_diagnostics_model_link(
    report: &mut IrValidationReport,
    label: &str,
    expected: Option<(&str, usize)>,
    diagnostics: Option<&ModelDiagnosticsReport>,
    analysis: Option<&IrModelReport>,
) {
    match (expected, diagnostics, analysis) {
        (Some((expected_id, expected_rules)), Some(diagnostics), Some(analysis)) => {
            compare_text(
                report,
                &format!("diagnostics.{label}.model"),
                expected_id,
                &diagnostics.model,
            );
            compare_count(
                report,
                &format!("diagnostics.{label}.rules"),
                expected_rules,
                diagnostics.rules.len(),
            );
            for (position, (diagnostic_rule, analysis_rule)) in
                diagnostics.rules.iter().zip(&analysis.rules).enumerate()
            {
                compare_rule_support(report, label, position, diagnostic_rule, analysis_rule);
            }
        }
        (None, None, _) => {}
        _ => report.push(
            IrInvariantCode::StageShapeMismatch,
            format!("diagnostics.{label}"),
            format!("{label} presence differs across compiler stages"),
        ),
    }
}

fn compare_rule_support(
    report: &mut IrValidationReport,
    label: &str,
    position: usize,
    diagnostics: &RuleDiagnosticsReport,
    analysis: &IrRuleReport,
) {
    let path = format!("analysis.{label}.rules[{position}]");
    if diagnostics.rule_index != analysis.rule_index
        || diagnostics.supported != analysis.supported
        || diagnostics.required_facts != analysis.required_facts
        || diagnostics.supporting_fragments != analysis.supporting_fragments
        || diagnostics.missing_facts != analysis.missing_facts
        || diagnostics.unsupported_payload_offsets != analysis.unsupported_payload_offsets
    {
        report.push(
            IrInvariantCode::StageRuleMismatch,
            path,
            "supportability fields differ from diagnostics",
        );
    }
}

fn require_text(report: &mut IrValidationReport, path: impl Into<String>, value: &str) {
    if value.trim().is_empty() {
        report.push(
            IrInvariantCode::EmptyValue,
            path,
            "value must contain non-whitespace text",
        );
    }
}

fn require_optional_text(
    report: &mut IrValidationReport,
    path: impl Into<String>,
    value: Option<&str>,
) {
    if let Some(value) = value {
        require_text(report, path, value);
    }
}

fn validate_text_list(report: &mut IrValidationReport, path: &str, values: &[String]) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        require_text(report, format!("{path}[{index}]"), value);
        if !seen.insert(value.as_str()) {
            report.push(
                IrInvariantCode::DuplicateValue,
                format!("{path}[{index}]"),
                format!("value '{value}' appears more than once"),
            );
        }
    }
}

fn validate_u16_list(report: &mut IrValidationReport, path: &str, values: &[u16]) {
    let mut seen = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        if !seen.insert(*value) {
            report.push(
                IrInvariantCode::DuplicateValue,
                format!("{path}[{index}]"),
                format!("offset {value} appears more than once"),
            );
        }
    }
}

fn validate_rule_index(
    report: &mut IrValidationReport,
    path: &str,
    expected: usize,
    actual: usize,
) {
    if actual != expected {
        report.push(
            IrInvariantCode::RuleIndexMismatch,
            format!("{path}.rule_index"),
            format!("expected {expected}, found {actual}"),
        );
    }
}

fn compare_text(report: &mut IrValidationReport, path: &str, expected: &str, actual: &str) {
    if actual != expected {
        report.push(
            IrInvariantCode::StageIdentityMismatch,
            path,
            format!("expected '{expected}', found '{actual}'"),
        );
    }
}

fn compare_option_text(
    report: &mut IrValidationReport,
    path: &str,
    expected: Option<&str>,
    actual: Option<&str>,
) {
    if actual != expected {
        report.push(
            IrInvariantCode::StageIdentityMismatch,
            path,
            format!("expected {expected:?}, found {actual:?}"),
        );
    }
}

fn compare_count(report: &mut IrValidationReport, path: &str, expected: usize, actual: usize) {
    if actual != expected {
        report.push(
            IrInvariantCode::StageShapeMismatch,
            path,
            format!("expected {expected} entries, found {actual}"),
        );
    }
}

fn bounded_detail(mut detail: String) -> String {
    if detail.len() <= MAX_IR_INVARIANT_DETAIL_BYTES {
        return detail;
    }
    let mut end = MAX_IR_INVARIANT_DETAIL_BYTES.saturating_sub(3);
    while !detail.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    detail.truncate(end);
    detail.push_str("...");
    detail
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EvidenceOverrideReport, FragmentParamReport, ParamValueReport, ProgramModelReport,
    };

    fn valid_projection() -> CompilerStageProjections<()> {
        let rule = IrRuleReport {
            rule_index: 0,
            predicate: "packet_observed".into(),
            signal: Some("packet_observed".into()),
            narrative: "packet observed".into(),
            dedupe: true,
            module: Some("dns".into()),
            phase: Some("query".into()),
            phase_kind: Some("send".into()),
            required_facts: vec!["packet_meta".into()],
            supporting_fragments: vec!["packet".into()],
            missing_facts: Vec::new(),
            unsupported_payload_offsets: Vec::new(),
            supported: true,
        };
        CompilerStageProjections {
            binding: BindingReport {
                template_id: "dns_probe".into(),
                fragments: vec!["packet".into()],
                window: None,
                reason_profile: None,
                program_model: Some(ProgramModelReport {
                    id: "dns_model".into(),
                    operation: "dns_query".into(),
                    rules: 1,
                }),
                fragment_params: vec![FragmentParamReport {
                    fragment: "packet".into(),
                    key: "capture".into(),
                    value: ParamValueReport::Bool(true),
                }],
                evidence_overrides: vec![EvidenceOverrideReport {
                    fact_kind: "packet_meta".into(),
                    tier: "core_requirement".into(),
                }],
            },
            diagnostics: Ok(DiagnosticsReport {
                template_id: "dns_probe".into(),
                fragments: vec!["packet".into()],
                program_model: Some(ModelDiagnosticsReport {
                    model: "dns_model".into(),
                    rules: vec![RuleDiagnosticsReport {
                        rule_index: 0,
                        tier: "core_requirement".into(),
                        supported: true,
                        required_facts: vec!["packet_meta".into()],
                        supporting_fragments: vec!["packet".into()],
                        missing_facts: Vec::new(),
                        unsupported_payload_offsets: Vec::new(),
                    }],
                }),
                reason_model: None,
            }),
            analysis: IrReport {
                template_id: "dns_probe".into(),
                program_model: Some(IrModelReport {
                    kind: "program_model".into(),
                    id: "dns_model".into(),
                    operation: Some("dns_query".into()),
                    rules: vec![rule],
                }),
                reason_model: None,
            },
        }
    }

    #[test]
    fn valid_stage_set_has_no_violations() {
        assert!(valid_projection().validate_invariants().is_valid());
    }

    #[test]
    fn validation_accumulates_stable_codes_and_paths() {
        let mut projections = valid_projection();
        projections.binding.fragments.push("packet".into());
        projections.binding.fragment_params[0].fragment = "missing".into();
        let rule = &mut projections.analysis.program_model.as_mut().unwrap().rules[0];
        rule.rule_index = 4;
        rule.missing_facts.push("socket_state".into());

        let report = projections.validate_invariants();
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::DuplicateValue
                && violation.path == "binding.fragments[1]"
        }));
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::UnknownFragment
                && violation.path == "binding.fragment_params[0].fragment"
        }));
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::RuleIndexMismatch
                && violation.path == "analysis.program_model.rules[0].rule_index"
        }));
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::MissingFactNotRequired
                && violation.path == "analysis.program_model.rules[0].missing_facts[0]"
        }));
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::SupportStateMismatch
                && violation.path == "analysis.program_model.rules[0].supported"
        }));
    }

    #[test]
    fn cross_stage_drift_is_rejected() {
        let mut projections = valid_projection();
        projections.analysis.template_id = "other".into();
        projections
            .analysis
            .program_model
            .as_mut()
            .unwrap()
            .operation = Some("other".into());
        projections.diagnostics.as_mut().unwrap().program_model = None;

        let errors = projections.validate_invariants().into_result().unwrap_err();
        assert!(errors.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::StageIdentityMismatch
                && violation.path == "analysis.template_id"
        }));
        assert!(errors.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::StageIdentityMismatch
                && violation.path == "analysis.program_model.operation"
        }));
        assert!(errors.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::StageShapeMismatch
                && violation.path == "diagnostics.program_model"
        }));
    }

    #[test]
    fn reason_profile_kind_must_match_the_binding_variant() {
        let mut projections = valid_projection();
        projections.binding.reason_profile = Some(ReasonProfileReport::Builtin {
            id: "udp_datagram_l1".into(),
        });
        projections.analysis.reason_model = Some(IrModelReport {
            kind: "declarative_reason_model".into(),
            id: "udp_datagram_l1".into(),
            operation: None,
            rules: Vec::new(),
        });

        let report = projections.validate_invariants();
        assert!(report.violations.iter().any(|violation| {
            violation.code == IrInvariantCode::StageIdentityMismatch
                && violation.path == "analysis.reason_model.kind"
        }));
    }

    #[test]
    fn diagnostic_host_failure_does_not_invent_ir_violations() {
        let valid = valid_projection();
        let projections = CompilerStageProjections::<&'static str> {
            binding: valid.binding,
            diagnostics: Err("registry unavailable"),
            analysis: valid.analysis,
        };

        assert!(projections.validate_invariants().is_valid());
    }

    #[test]
    fn error_display_is_bounded_to_the_first_violation() {
        let mut report = IrValidationReport::default();
        report.push(IrInvariantCode::EmptyValue, "template_id", "empty");
        report.push(IrInvariantCode::DuplicateValue, "fragments[1]", "duplicate");

        assert_eq!(
            report.into_result().unwrap_err().to_string(),
            "2 IR invariant violations; first is GEWYLANG-IR-EMPTY-VALUE at template_id: empty"
        );
    }

    #[test]
    fn validation_report_caps_count_and_detail_allocation() {
        let mut binding = valid_projection().binding;
        binding.fragments = vec![" ".repeat(MAX_IR_INVARIANT_DETAIL_BYTES + 50); 400];

        let report = binding.validate_invariants();
        assert_eq!(report.violations.len(), MAX_IR_INVARIANT_VIOLATIONS);
        assert!(report.truncated);
        assert!(
            report
                .violations
                .iter()
                .all(|violation| violation.detail.len() <= MAX_IR_INVARIANT_DETAIL_BYTES)
        );
        assert!(
            report
                .into_result()
                .unwrap_err()
                .to_string()
                .starts_with("at least 256 IR invariant violations")
        );
    }
}
