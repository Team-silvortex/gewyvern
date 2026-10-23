//! Product data-only merge admission. No saved expression is evaluated here.
//!
//! Hosts retain successor authority and binding evaluation. This stage shares the
//! released completion kernel between live execution and unbound receipt checks.

use leselang_runtime_core::Fault;

use crate::bounded_json::{self, BoundedJsonError};
use crate::merge_completion::{
    MergeCompletionError, MergeCompletionHost, MergedCompletion, merge_preflighted_completions,
};
use crate::reference_merge::validate_merge_plan;
use crate::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
use crate::reference_request::{DEFAULT_MAX_OUTPUT_ITEMS, request_admission_fault};
use crate::reference_value::{validate_branch_outcome, validate_value};
use crate::{
    BranchCompletion, BranchOutcome, GroupResultBinding, MergePlan, RequestAdmissionError,
    RequestCorrespondenceError, Step, Value,
};

/// Admit plan, completions, member types, aggregate items/value and encoded bytes,
/// in that order. The borrowed member proof must not evaluate saved code. A host
/// finishing a successor group must separately prove its original authority first.
/// `finish_binding` selects the final-value projection, not expression execution.
pub fn assemble_merge(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
    max_output_items: usize,
    finish_binding: bool,
    validate_member: impl Fn(&GroupResultBinding, &str, &Value) -> Result<(), Fault>,
) -> Result<Step, Fault> {
    validate_merge_plan(plan)?;
    let merged = merge_preflighted_completions(
        plan,
        completions,
        max_output_items,
        DEFAULT_MAX_OUTPUT_ITEMS,
        finish_binding,
        &CompletionPolicy(validate_member),
    )
    .map_err(merge_completion_fault)?;
    let step = match merged {
        MergedCompletion::Value(value) => Step::Done(value),
        MergedCompletion::Terminal(step) => step,
    };
    if let Step::Done(value) = &step {
        validate_value(value, 1)?;
    }
    bounded_json::measure(&step, MAX_JOURNAL_ENTRY_BYTES).map_err(|error| match error {
        BoundedJsonError::Limit { limit, .. } => Fault {
            code: "LSV3002".into(),
            message: format!("structured merge output exceeds {limit} bytes"),
        },
        BoundedJsonError::Serialize(error) => Fault {
            code: "LSV3001".into(),
            message: format!("failed to encode structured merge output: {error}"),
        },
    })?;
    Ok(step)
}

struct CompletionPolicy<F>(F);

impl<F> MergeCompletionHost for CompletionPolicy<F>
where
    F: Fn(&GroupResultBinding, &str, &Value) -> Result<(), Fault>,
{
    type Completion = BranchCompletion;
    type Outcome = BranchOutcome;
    type Terminal = Step;
    type Error = Fault;

    fn into_parts(completion: BranchCompletion) -> (String, BranchOutcome) {
        (completion.branch, completion.outcome)
    }
    fn validate_outcome(&self, outcome: &BranchOutcome) -> Result<usize, Fault> {
        validate_branch_outcome(outcome)
    }
    fn validate_member(
        &self,
        binding: &GroupResultBinding,
        branch: &str,
        outcome: &BranchOutcome,
    ) -> Result<(), Fault> {
        if let BranchOutcome::Value(value) = outcome {
            (self.0)(binding, branch, value)?;
        }
        Ok(())
    }
    fn into_value_or_terminal(outcome: BranchOutcome) -> MergedCompletion<Step> {
        match outcome {
            BranchOutcome::Value(value) => MergedCompletion::Value(value),
            BranchOutcome::Cancelled(value) => MergedCompletion::Terminal(Step::Cancelled(value)),
            BranchOutcome::Failed(value) => MergedCompletion::Terminal(Step::Failed(value)),
            BranchOutcome::Fault(value) => MergedCompletion::Terminal(Step::Fault(value)),
        }
    }
}

/// Only returned-data limits become terminal receipts. Storage/authority failures
/// remain errors; this conversion itself does not persist or publish anything.
pub fn terminal_merge(plan: &MergePlan, result: Result<Step, Fault>) -> Result<Step, Fault> {
    match result {
        Err(error)
            if error.code == "LSV3002" || (plan.order.is_parallel() && error.code == "LSV2404") =>
        {
            Ok(Step::Fault(error))
        }
        result => result,
    }
}

fn merge_completion_fault(error: MergeCompletionError<Fault>) -> Fault {
    let (code, message) = match error {
        MergeCompletionError::Native(error) => return error,
        MergeCompletionError::OutputItems { items, limit } => {
            return merge_output_fault(items, limit);
        }
        MergeCompletionError::MissingFinalValue => {
            return request_admission_fault(RequestAdmissionError::Correspondence(
                RequestCorrespondenceError::SuccessorAuthority,
            ));
        }
        MergeCompletionError::OutputBudget => (
            "LSV2404",
            "structured merge output budget exceeds runtime bounds",
        ),
        MergeCompletionError::Incomplete => {
            ("LSV2402", "structured merge completion set is incomplete")
        }
        MergeCompletionError::Mismatch => (
            "LSV2402",
            "structured merge completion set does not match its plan",
        ),
    };
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

pub fn merge_output_fault(items: usize, limit: usize) -> Fault {
    Fault {
        code: "LSV2404".into(),
        message: format!("structured merge returned {items} items, limit is {limit}"),
    }
}
