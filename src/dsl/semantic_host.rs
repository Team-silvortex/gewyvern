use super::{
    DslError,
    function_types::validate_pipeline_param_value_kind,
    materializer::build_binding_from_canonical_assignments,
    parse_bool, parse_flow_predicate, parse_reason_key_event,
    pipeline::CanonicalAssignment,
    predicate::{parse_narrative_template, parse_reason_narrative},
    semantic_values::{parse_operation, parse_stage, parse_window_profile},
};
use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::fragment::EvidenceTier;
use crate::ir::FlowPredicate;
use crate::ledger::FactKindTag;
use crate::program::ProgramRule;
use crate::reason::{ReasonProfile, ReasonRule};
use crate::template::{FragmentParamValue, TemplateBinding, WindowProfile};
use gewylang_compiler::{BindingMaterializer, ProgramRuleInput, ReasonRuleInput, SemanticHost};
use gewylang_syntax::{PipelineValueKind, SyntaxError};

pub(super) struct GewyvernSemanticHost;

impl SemanticHost for GewyvernSemanticHost {
    type WindowProfile = WindowProfile;
    type ReasonProfile = ReasonProfile;
    type ReasonRule = ReasonRule;
    type ProgramOperation = ProgramOperation;
    type ProgramRule = ProgramRule;
    type FragmentParamValue = FragmentParamValue;
    type FactKind = FactKindTag;
    type EvidenceTier = EvidenceTier;

    fn validate_pipeline_param_value_kind(
        &self,
        raw_value: &str,
        kind: PipelineValueKind,
        context: &str,
    ) -> Result<(), SyntaxError> {
        validate_pipeline_param_value_kind(raw_value, kind, context).map_err(to_syntax_error)
    }

    fn parse_window_profile(&self, value: &str) -> Result<WindowProfile, SyntaxError> {
        parse_window_profile(value).map_err(to_syntax_error)
    }

    fn parse_reason_profile(&self, value: &str) -> Result<ReasonProfile, SyntaxError> {
        ReasonProfile::from_id(value)
            .ok_or_else(|| SyntaxError::InvalidValue(format!("unknown reason profile '{value}'")))
    }

    fn parse_operation(&self, value: &str) -> ProgramOperation {
        parse_operation(value)
    }

    fn parse_fragment_param_value(&self, value: &str) -> Result<FragmentParamValue, SyntaxError> {
        if matches!(value, "true" | "false") {
            return parse_bool(value)
                .map(FragmentParamValue::Bool)
                .map_err(to_syntax_error);
        }
        Ok(value.parse::<u64>().map_or_else(
            |_| FragmentParamValue::String(value.to_string()),
            FragmentParamValue::U64,
        ))
    }

    fn parse_fact_kind(&self, value: &str) -> Result<FactKindTag, SyntaxError> {
        FactKindTag::from_str(value).ok_or_else(|| {
            SyntaxError::InvalidValue(format!("unknown evidence fact kind '{value}'"))
        })
    }

    fn parse_evidence_tier(&self, value: &str) -> Result<EvidenceTier, SyntaxError> {
        match value {
            "core_requirement" => Ok(EvidenceTier::CoreRequirement),
            "optional_enhancement" => Ok(EvidenceTier::OptionalEnhancement),
            other => Err(SyntaxError::InvalidValue(format!(
                "unknown evidence tier '{other}'"
            ))),
        }
    }

    fn build_program_rule(&self, input: ProgramRuleInput<'_>) -> Result<ProgramRule, SyntaxError> {
        Ok(ProgramRule {
            predicate: parse_host_predicate(input.predicate.value, input.predicate.column)?,
            signal: parse_host_stage(input.stage.value, input.stage.column)?,
            narrative: parse_narrative_template(input.narrative),
            dedupe: input.dedupe,
            module: input.scope.module,
            phase: input.scope.phase,
        })
    }

    fn build_reason_rule(&self, input: ReasonRuleInput<'_>) -> Result<ReasonRule, SyntaxError> {
        Ok(ReasonRule {
            predicate: parse_host_predicate(input.predicate.value, input.predicate.column)?,
            signal: parse_reason_key_event(input.key_event.value)
                .map_err(to_syntax_error)
                .map_err(|err| err.reanchor_line_column(0, input.key_event.column))?,
            narrative: parse_reason_narrative(input.narrative),
            dedupe: input.dedupe,
            module: input.scope.module,
            phase: input.scope.phase,
        })
    }
}

impl BindingMaterializer for GewyvernSemanticHost {
    type Binding = TemplateBinding;
    type Error = DslError;

    fn materialize_binding(
        &self,
        assignments: Vec<CanonicalAssignment>,
    ) -> Result<Self::Binding, Self::Error> {
        build_binding_from_canonical_assignments(assignments)
    }
}

fn parse_host_predicate(value: &str, column: usize) -> Result<FlowPredicate, SyntaxError> {
    parse_flow_predicate(value)
        .map_err(to_syntax_error)
        .map_err(|err| err.reanchor_line_column(0, column))
}

fn parse_host_stage(value: &str, column: usize) -> Result<Option<ProgramStageKind>, SyntaxError> {
    parse_stage(value)
        .map_err(to_syntax_error)
        .map_err(|err| err.reanchor_line_column(0, column))
}

fn to_syntax_error(error: DslError) -> SyntaxError {
    match error {
        DslError::Located {
            line,
            column,
            inner,
        } => SyntaxError::Located {
            line,
            column,
            inner: Box::new(to_syntax_error(*inner)),
        },
        DslError::InvalidLine(line) => SyntaxError::InvalidLine(line),
        DslError::MissingField(field) => SyntaxError::MissingField(field),
        DslError::InvalidValue(value) => SyntaxError::InvalidValue(value),
        DslError::Registry(error) => {
            SyntaxError::InvalidValue(format!("Gewyvern semantic registry failure: {error:?}"))
        }
        DslError::Io(error) => SyntaxError::Io(error),
    }
}
