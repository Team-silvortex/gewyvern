//! Product-independent GewyLang expansion and semantic lowering.

use gewylang_syntax::{PipelineValueKind, SyntaxError};

mod lowering;

pub use lowering::{lower_pipeline_module, substitute_pipeline_arg};

/// Parse and lower an in-memory GewyLang module through an explicit semantic host.
pub fn compile_str<H: SemanticHost>(
    input: &str,
    host: &H,
) -> Result<Vec<CanonicalAssignment<H>>, SyntaxError> {
    let module = gewylang_syntax::parse_str(input)?;
    lower_pipeline_module(&module, host, true)
}

/// Load, expand, and lower a GewyLang package entry through an explicit semantic host.
pub fn compile_file<H: SemanticHost>(
    path: &str,
    host: &H,
) -> Result<Vec<CanonicalAssignment<H>>, SyntaxError> {
    let module = gewylang_syntax::parse_file(path)?;
    lower_pipeline_module(&module, host, true)
}

/// Product semantics required while lowering a GewyLang module.
///
/// The compiler owns source expansion, argument binding, and diagnostics. A
/// host supplies only the runtime-specific values that cross the language
/// boundary.
pub trait SemanticHost {
    type WindowProfile;
    type ReasonProfile;
    type ReasonRule;
    type ProgramOperation;
    type ProgramRule;
    type FragmentParamValue;
    type FactKind;
    type EvidenceTier;

    fn validate_pipeline_param_value_kind(
        &self,
        raw_value: &str,
        kind: PipelineValueKind,
        context: &str,
    ) -> Result<(), SyntaxError>;

    fn parse_window_profile(&self, value: &str) -> Result<Self::WindowProfile, SyntaxError>;
    fn parse_reason_profile(&self, value: &str) -> Result<Self::ReasonProfile, SyntaxError>;
    fn parse_operation(&self, value: &str) -> Self::ProgramOperation;
    fn parse_fragment_param_value(
        &self,
        value: &str,
    ) -> Result<Self::FragmentParamValue, SyntaxError>;
    fn parse_fact_kind(&self, value: &str) -> Result<Self::FactKind, SyntaxError>;
    fn parse_evidence_tier(&self, value: &str) -> Result<Self::EvidenceTier, SyntaxError>;
    fn build_program_rule(
        &self,
        input: ProgramRuleInput<'_>,
    ) -> Result<Self::ProgramRule, SyntaxError>;
    fn build_reason_rule(
        &self,
        input: ReasonRuleInput<'_>,
    ) -> Result<Self::ReasonRule, SyntaxError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LocatedSemanticValue<'a> {
    pub value: &'a str,
    pub column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuleScope {
    pub module: Option<String>,
    pub phase: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProgramRuleInput<'a> {
    pub predicate: LocatedSemanticValue<'a>,
    pub stage: LocatedSemanticValue<'a>,
    pub narrative: &'a str,
    pub dedupe: bool,
    pub scope: RuleScope,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReasonRuleInput<'a> {
    pub predicate: LocatedSemanticValue<'a>,
    pub key_event: LocatedSemanticValue<'a>,
    pub narrative: &'a str,
    pub dedupe: bool,
    pub scope: RuleScope,
}

pub struct CanonicalAssignment<H: SemanticHost> {
    pub value: CanonicalAssignmentValue<H>,
    pub line_no: usize,
}

pub enum CanonicalAssignmentValue<H: SemanticHost> {
    Template(String),
    Window(H::WindowProfile),
    WindowDuration(u64),
    WindowLateness(u64),
    Reason(H::ReasonProfile),
    ReasonModel(String),
    ReasonRule(H::ReasonRule),
    Fragment(String),
    ProgramModel(String),
    Operation(H::ProgramOperation),
    ProgramRule(H::ProgramRule),
    FragmentParam {
        fragment_id: String,
        key: String,
        value: H::FragmentParamValue,
    },
    EvidenceOverride {
        fact_kind: H::FactKind,
        tier: H::EvidenceTier,
    },
}

impl<H: SemanticHost> CanonicalAssignment<H> {
    fn new(value: CanonicalAssignmentValue<H>, line_no: usize) -> Self {
        Self { value, line_no }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHost;

    impl SemanticHost for TestHost {
        type WindowProfile = String;
        type ReasonProfile = String;
        type ReasonRule = String;
        type ProgramOperation = String;
        type ProgramRule = String;
        type FragmentParamValue = String;
        type FactKind = String;
        type EvidenceTier = String;

        fn validate_pipeline_param_value_kind(
            &self,
            _raw_value: &str,
            _kind: PipelineValueKind,
            _context: &str,
        ) -> Result<(), SyntaxError> {
            Ok(())
        }

        fn parse_window_profile(&self, value: &str) -> Result<String, SyntaxError> {
            Ok(value.to_string())
        }

        fn parse_reason_profile(&self, value: &str) -> Result<String, SyntaxError> {
            Ok(value.to_string())
        }

        fn parse_operation(&self, value: &str) -> String {
            value.to_string()
        }

        fn parse_fragment_param_value(&self, value: &str) -> Result<String, SyntaxError> {
            Ok(value.to_string())
        }

        fn parse_fact_kind(&self, value: &str) -> Result<String, SyntaxError> {
            Ok(value.to_string())
        }

        fn parse_evidence_tier(&self, value: &str) -> Result<String, SyntaxError> {
            Ok(value.to_string())
        }

        fn build_program_rule(&self, input: ProgramRuleInput<'_>) -> Result<String, SyntaxError> {
            Ok(format!(
                "{}:{}:{}",
                input.predicate.value, input.stage.value, input.dedupe
            ))
        }

        fn build_reason_rule(&self, input: ReasonRuleInput<'_>) -> Result<String, SyntaxError> {
            Ok(format!(
                "{}:{}:{}",
                input.predicate.value, input.key_event.value, input.dedupe
            ))
        }
    }

    #[test]
    fn standalone_compiler_lowers_with_a_non_product_host() {
        let assignments = compile_str(
            r#"
fn common(model_name, op_name) {
  |> fragment(:socket)
  |> operation($op_name)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true)
}

template(:standalone)
|> window(:default_5s)
|> reason(:network_timeout)
|> use(:common, :standalone_model, :connect_flow)
|> param(:socket.capture, true)
|> evidence(:packet, :core_requirement)
"#,
            &TestHost,
        )
        .unwrap();
        assert!(matches!(
            assignments.first().map(|item| &item.value),
            Some(CanonicalAssignmentValue::Template(id)) if id == "standalone"
        ));
        assert!(assignments.iter().any(|item| matches!(
            &item.value,
            CanonicalAssignmentValue::Operation(operation) if operation == "connect_flow"
        )));
        assert!(assignments.iter().any(|item| matches!(
            &item.value,
            CanonicalAssignmentValue::ProgramRule(rule)
                if rule == "process_bound:process_bound:true"
        )));
    }
}
