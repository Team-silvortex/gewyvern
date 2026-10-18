//! Ordered product-frame validation without VM identity or recovery ownership.

use leselang_hir::{Effect, Type, host_call::HostOperation};
use leselang_runtime_core::{ContinuationFrame, ResourceBudgetError, ResourceBudgetLimits};

use crate::{
    ResultBinding, ResultBindingLimits,
    continuation_schema::{ContinuationSchemaError, validate_preflighted_continuation_schema},
    validate_result_binding, validate_result_binding_structure,
};

/// Caller-owned ceilings, not authenticated grants or implicit VM defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContinuationAdmissionLimits {
    pub binding: ResultBindingLimits,
    pub budget: ResourceBudgetLimits,
    pub max_group_sequence: u64,
}

/// Closed diagnostics never retain or format tokens, effects or saved payloads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContinuationAdmissionError {
    Binding,
    Schema(ContinuationSchemaError),
    Token,
    ProgramCounter,
    StructuredEffect,
    ResultType,
    Budget(ResourceBudgetError),
}

impl std::fmt::Display for ContinuationAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Binding => "invalid or oversized result-binding continuation",
            Self::Schema(error) => return std::fmt::Display::fmt(error, formatter),
            Self::Token => "invalid continuation token",
            Self::ProgramCounter => "continuation program counter is unsupported",
            Self::StructuredEffect => {
                "structured control flow cannot be restored as a single continuation"
            }
            Self::ResultType => "continuation result type does not match its pending effect",
            Self::Budget(ResourceBudgetError::Fuel) => "continuation fuel exceeds runtime limit",
            Self::Budget(ResourceBudgetError::Deadline) => {
                "continuation deadline exceeds runtime limit"
            }
            Self::Budget(ResourceBudgetError::AbsoluteDeadline) => {
                "continuation absolute deadline is out of range"
            }
            Self::Budget(ResourceBudgetError::Output) => {
                "continuation output limit exceeds runtime maximum"
            }
        })
    }
}

impl std::error::Error for ContinuationAdmissionError {}

/// Validate binding structure, schema, native token, program counter, pending
/// result signature, complete binding types, then saved budget bounds in order.
///
/// Callers bound decoded ingress, pending-effect payloads and native callback
/// work. The group-owner text hook runs at most once after structure preflight;
/// the token predicate runs at
/// most once after schema admission. Name hooks retain the original structure
/// and type-check traversals. Hooks must observe one stable snapshot: they are
/// trusted native code, and side effects/unwind are not retried or rolled back.
/// Tokens and revisions need no Clone, Debug, serde, equality or thread traits.
///
/// Success is product-frame consistency, not authentic ownership or eligibility
/// to resume. Bounded codecs, canonical pending-effect admission, canonical
/// request sequence policy, original authority/budgets, current deadlines,
/// receipts, leases and atomic durable settlement remain caller responsibilities.
/// No identity, meter, evaluator, journal or execution permission is created.
pub fn validate_continuation_frame<Token, Revision>(
    image: &ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>,
    limits: ContinuationAdmissionLimits,
    group_owner_text: impl FnOnce(&Token) -> &str,
    validate_token: impl FnOnce(&Token) -> bool,
    mut validate_group_name: impl FnMut(&str) -> bool,
) -> Result<(), ContinuationAdmissionError> {
    use ContinuationAdmissionError::{
        Binding, Budget, ProgramCounter, ResultType, Schema, StructuredEffect, Token,
    };

    if let Some(binding) = &image.result_binding {
        validate_result_binding_structure(
            binding,
            limits.binding.captures,
            limits.binding.groups,
            &mut validate_group_name,
        )
        .map_err(|_| Binding)?;
    }
    validate_preflighted_continuation_schema(
        image.result_binding.as_deref(),
        image.group_result.as_ref().map(group_owner_text),
        image.schema_version,
        limits.max_group_sequence,
    )
    .map_err(Schema)?;
    if !validate_token(&image.token) {
        return Err(Token);
    }
    if image.program_counter != 1 {
        return Err(ProgramCounter);
    }
    let expected_type = HostOperation::for_effect(&image.pending_effect)
        .map(|operation| operation.result_type())
        .ok_or(StructuredEffect)?;
    if image.result_type != expected_type {
        return Err(ResultType);
    }
    if let Some(binding) = &image.result_binding {
        validate_result_binding(
            binding,
            &image.pending_effect,
            limits.binding,
            &mut validate_group_name,
        )
        .map_err(|_| Binding)?;
    }
    image
        .resource_budget()
        .validate(limits.budget)
        .map_err(Budget)
}
