mod ir;

pub use ir::*;

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
    pub module_doc: Option<String>,
    pub template_doc: Option<String>,
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
    pub doc: Option<String>,
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
