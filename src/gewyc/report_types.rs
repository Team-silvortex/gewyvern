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
    pub ir_report: Option<IrReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExplainReport {
    pub ok: bool,
    pub binding: Option<BindingReport>,
    pub frontend: Option<FrontendReport>,
    pub diagnostics: Option<DiagnosticsReport>,
    pub findings: CompilerFindingsReport,
    pub stages: CompilerStagesReport,
    pub ir_report: Option<IrReport>,
    pub lowered_binding_summary: Option<LoweredBindingSummary>,
    pub frontend_lowering_delta: Option<FrontendLoweringDelta>,
    pub binding_shape_note: Option<String>,
    pub ir_lowering_delta: Option<IrLoweringDelta>,
    pub ir_shape_note: Option<String>,
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
    Ir,
    Validation,
    Diagnostics,
    Findings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrReport {
    pub template_id: String,
    pub program_model: Option<IrModelReport>,
    pub reason_model: Option<IrModelReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrModelReport {
    pub kind: String,
    pub id: String,
    pub operation: Option<String>,
    pub rules: Vec<IrRuleReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrRuleReport {
    pub rule_index: usize,
    pub predicate: String,
    pub signal: Option<String>,
    pub narrative: String,
    pub dedupe: bool,
    pub module: Option<String>,
    pub phase: Option<String>,
    pub phase_kind: Option<String>,
    pub required_facts: Vec<String>,
    pub supporting_fragments: Vec<String>,
    pub missing_facts: Vec<String>,
    pub unsupported_payload_offsets: Vec<u16>,
    pub supported: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrRuleSupportShape<'a> {
    pub required_facts: &'a [String],
    pub supporting_fragments: &'a [String],
    pub missing_facts: &'a [String],
    pub unsupported_payload_offsets: &'a [u16],
    pub supported: bool,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrLoweringDelta {
    pub frontend_function_count: usize,
    pub frontend_include_source_count: usize,
    pub frontend_use_edge_count: usize,
    pub frontend_graph_node_count: usize,
    pub frontend_graph_edge_count: usize,
    pub lowered_program_rule_count: usize,
    pub lowered_reason_rule_count: usize,
    pub lowered_supported_rule_count: usize,
    pub lowered_unsupported_rule_count: usize,
    pub lowered_modules: Vec<String>,
    pub lowered_phases: Vec<String>,
    pub lowered_phase_kinds: Vec<String>,
    pub lowered_models: Vec<IrModelShapeSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrModelShapeSummary {
    pub label: String,
    pub id: String,
    pub kind: String,
    pub rule_count: usize,
    pub supported_rule_count: usize,
    pub unsupported_rule_count: usize,
    pub modules: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IrModelCompareSummary {
    pub program_rule_count: usize,
    pub reason_rule_count: usize,
    pub rule_count_delta: isize,
    pub program_supported_rule_count: usize,
    pub reason_supported_rule_count: usize,
    pub supported_rule_count_delta: isize,
    pub shared_modules: Vec<String>,
    pub program_only_modules: Vec<String>,
    pub reason_only_modules: Vec<String>,
    pub shared_phases: Vec<String>,
    pub program_only_phases: Vec<String>,
    pub reason_only_phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistorySnapshot {
    pub template_id: String,
    pub operation: Option<String>,
    pub program_model: Option<IrHistoryModelSnapshot>,
    pub reason_model: Option<IrHistoryModelSnapshot>,
    pub model_compare: Option<IrHistoryCompareSnapshot>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistoryModelSnapshot {
    pub id: String,
    pub kind: String,
    pub rule_count: usize,
    pub supported_rule_count: usize,
    pub unsupported_rule_count: usize,
    pub modules: Vec<String>,
    pub phases: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IrHistoryCompareSnapshot {
    pub rule_count_delta: isize,
    pub supported_rule_count_delta: isize,
    pub shared_modules: Vec<String>,
    pub program_only_modules: Vec<String>,
    pub reason_only_modules: Vec<String>,
    pub shared_phases: Vec<String>,
    pub program_only_phases: Vec<String>,
    pub reason_only_phases: Vec<String>,
}

impl IrReport {
    pub(crate) fn model_entries(&self) -> Vec<(&'static str, &IrModelReport)> {
        let mut entries = Vec::with_capacity(2);
        if let Some(model) = self.program_model.as_ref() {
            entries.push(("program_model", model));
        }
        if let Some(model) = self.reason_model.as_ref() {
            entries.push(("reason_model", model));
        }
        entries
    }

    pub(crate) fn compare_models(&self) -> Option<IrModelCompareSummary> {
        let program = self.program_model.as_ref()?;
        let reason = self.reason_model.as_ref()?;
        let program_modules = program.modules();
        let reason_modules = reason.modules();
        let program_phases = program.phases();
        let reason_phases = reason.phases();
        Some(IrModelCompareSummary {
            program_rule_count: program.rules.len(),
            reason_rule_count: reason.rules.len(),
            rule_count_delta: program.rules.len() as isize - reason.rules.len() as isize,
            program_supported_rule_count: program.supported_rule_count(),
            reason_supported_rule_count: reason.supported_rule_count(),
            supported_rule_count_delta: program.supported_rule_count() as isize
                - reason.supported_rule_count() as isize,
            shared_modules: shared_sorted_strings(&program_modules, &reason_modules),
            program_only_modules: difference_sorted_strings(&program_modules, &reason_modules),
            reason_only_modules: difference_sorted_strings(&reason_modules, &program_modules),
            shared_phases: shared_sorted_strings(&program_phases, &reason_phases),
            program_only_phases: difference_sorted_strings(&program_phases, &reason_phases),
            reason_only_phases: difference_sorted_strings(&reason_phases, &program_phases),
        })
    }

    pub(crate) fn history_snapshot(&self) -> IrHistorySnapshot {
        IrHistorySnapshot {
            template_id: self.template_id.clone(),
            operation: self
                .program_model
                .as_ref()
                .and_then(|model| model.operation.clone()),
            program_model: self
                .program_model
                .as_ref()
                .map(IrModelReport::history_snapshot),
            reason_model: self
                .reason_model
                .as_ref()
                .map(IrModelReport::history_snapshot),
            model_compare: self
                .compare_models()
                .map(|compare| compare.history_snapshot()),
        }
    }
}

impl IrModelReport {
    pub(crate) fn supported_rule_count(&self) -> usize {
        self.rules.iter().filter(|rule| rule.supported).count()
    }

    pub(crate) fn unsupported_rule_count(&self) -> usize {
        self.rules.len().saturating_sub(self.supported_rule_count())
    }

    pub(crate) fn modules(&self) -> Vec<String> {
        unique_sorted_strings(
            self.rules
                .iter()
                .filter_map(|rule| rule.module_name().map(str::to_string))
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn phases(&self) -> Vec<String> {
        unique_sorted_strings(
            self.rules
                .iter()
                .filter_map(|rule| rule.phase_name().map(str::to_string))
                .collect::<Vec<_>>(),
        )
    }

    pub(crate) fn history_snapshot(&self) -> IrHistoryModelSnapshot {
        IrHistoryModelSnapshot {
            id: self.id.clone(),
            kind: self.kind.clone(),
            rule_count: self.rules.len(),
            supported_rule_count: self.supported_rule_count(),
            unsupported_rule_count: self.unsupported_rule_count(),
            modules: self.modules(),
            phases: self.phases(),
        }
    }
}

impl IrModelCompareSummary {
    pub(crate) fn history_snapshot(self) -> IrHistoryCompareSnapshot {
        IrHistoryCompareSnapshot {
            rule_count_delta: self.rule_count_delta,
            supported_rule_count_delta: self.supported_rule_count_delta,
            shared_modules: self.shared_modules,
            program_only_modules: self.program_only_modules,
            reason_only_modules: self.reason_only_modules,
            shared_phases: self.shared_phases,
            program_only_phases: self.program_only_phases,
            reason_only_phases: self.reason_only_phases,
        }
    }
}

impl IrRuleReport {
    pub(crate) fn module_name(&self) -> Option<&str> {
        self.module.as_deref()
    }

    pub(crate) fn phase_name(&self) -> Option<&str> {
        self.phase.as_deref()
    }

    pub(crate) fn signal_name(&self) -> Option<&str> {
        self.signal.as_deref()
    }

    pub(crate) fn phase_kind_name(&self) -> Option<&str> {
        self.phase_kind.as_deref()
    }

    pub(crate) fn has_unsupported_payload_offsets(&self) -> bool {
        !self.unsupported_payload_offsets.is_empty()
    }

    pub(crate) fn support_shape(&self) -> IrRuleSupportShape<'_> {
        IrRuleSupportShape {
            required_facts: &self.required_facts,
            supporting_fragments: &self.supporting_fragments,
            missing_facts: &self.missing_facts,
            unsupported_payload_offsets: &self.unsupported_payload_offsets,
            supported: self.supported,
        }
    }
}

fn unique_sorted_strings(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items.dedup();
    items
}

fn shared_sorted_strings(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|item| right.contains(item))
        .cloned()
        .collect::<Vec<_>>()
}

fn difference_sorted_strings(left: &[String], right: &[String]) -> Vec<String> {
    left.iter()
        .filter(|item| !right.contains(item))
        .cloned()
        .collect::<Vec<_>>()
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
    pub include_sources: Vec<FrontendIncludeSourceReport>,
    pub use_edges: Vec<FrontendUseEdgeReport>,
    pub graph_nodes: Vec<FrontendGraphNodeReport>,
    pub graph_edges: Vec<FrontendGraphEdgeReport>,
    pub expansion_previews: Vec<FrontendExpansionPreviewReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendIncludeSourceReport {
    pub request: String,
    pub resolved_path: String,
    pub kind: String,
    pub dependency: Option<String>,
    pub package_scope: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionReport {
    pub name: String,
    pub signature: String,
    pub step_count: usize,
    pub source_id: String,
    pub package_scope: String,
    pub params: Vec<FrontendFunctionParamReport>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FrontendFunctionParamReport {
    pub name: String,
    pub has_default: bool,
    pub declared_kind: Option<String>,
    pub effective_kind: Option<String>,
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
    pub label: String,
    pub package_scope: String,
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
