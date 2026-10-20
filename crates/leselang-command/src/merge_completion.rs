//! Product completion-set admission and declared-order assembly, without VM state.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ExecutionOrder, GroupResultBinding, MAX_MERGE_BRANCH_NAME_BYTES, MergePlan, StructuredField,
    Value, valid_merge_branch_name,
};

/// Native outcome ownership stays with the host, including terminal values/errors.
/// Hooks validate existing data; they must not dispatch or commit work.
pub trait MergeCompletionHost {
    type Completion;
    type Outcome;
    type Terminal;
    type Error;

    fn into_parts(completion: Self::Completion) -> (String, Self::Outcome);
    fn validate_outcome(&self, outcome: &Self::Outcome) -> Result<usize, Self::Error>;
    fn validate_member(
        &self,
        binding: &GroupResultBinding,
        branch: &str,
        outcome: &Self::Outcome,
    ) -> Result<(), Self::Error>;
    fn into_value_or_terminal(outcome: Self::Outcome) -> MergedCompletion<Self::Terminal>;
}

pub enum MergedCompletion<Terminal> {
    Value(Value),
    Terminal(Terminal),
}

#[derive(Debug, Eq, PartialEq)]
pub enum MergeCompletionError<Native> {
    OutputBudget,
    Incomplete,
    Mismatch,
    Native(Native),
    OutputItems { items: usize, limit: usize },
    MissingFinalValue,
}

/// Assemble a fully admitted product plan without running its saved binding.
/// Callers must first bound and validate the plan and its complete binding.
/// Completion count and names precede native outcome checks; every outcome and
/// member binding is checked before aggregate output bounds and terminal choice.
/// Terminal precedence and field order follow the declaration, not arrival order.
///
/// Owned outcomes move without Clone/serde/Send bounds. This returns uncommitted
/// data, not an authentic receipt, authority, a recovered graph or execution.
/// Callers still validate the final value/encoding, finish the saved binding and
/// atomically publish through their original owner. No fuel or journal is created.
pub fn merge_preflighted_completions<H: MergeCompletionHost>(
    plan: &MergePlan,
    completions: Vec<H::Completion>,
    max_output_items: usize,
    max_allowed_output_items: usize,
    finish_binding: bool,
    host: &H,
) -> Result<MergedCompletion<H::Terminal>, MergeCompletionError<H::Error>> {
    if max_output_items > max_allowed_output_items {
        return Err(MergeCompletionError::OutputBudget);
    }
    if completions.len() != plan.branches.len() {
        return Err(MergeCompletionError::Incomplete);
    }
    let expected = plan
        .branches
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mut by_branch = BTreeMap::new();
    let mut output_items = 0usize;
    for completion in completions {
        let (branch, outcome) = H::into_parts(completion);
        if !valid_merge_branch_name(&branch, MAX_MERGE_BRANCH_NAME_BYTES)
            || !expected.contains(branch.as_str())
            || by_branch.contains_key(&branch)
        {
            return Err(MergeCompletionError::Mismatch);
        }
        output_items = output_items.saturating_add(
            host.validate_outcome(&outcome)
                .map_err(MergeCompletionError::Native)?,
        );
        if let Some(binding) = &plan.result_binding {
            host.validate_member(binding, &branch, &outcome)
                .map_err(MergeCompletionError::Native)?;
        }
        by_branch.insert(branch, outcome);
    }
    if output_items > max_output_items {
        return Err(MergeCompletionError::OutputItems {
            items: output_items,
            limit: max_output_items,
        });
    }
    let mut fields = Vec::with_capacity(plan.branches.len());
    let mut terminal = None;
    for branch in &plan.branches {
        let outcome = by_branch
            .remove(branch)
            .ok_or(MergeCompletionError::Incomplete)?;
        match H::into_value_or_terminal(outcome) {
            MergedCompletion::Value(value) => fields.push(StructuredField {
                name: branch.clone(),
                value: Box::new(value),
            }),
            MergedCompletion::Terminal(stop) => {
                terminal = Some(stop);
                break;
            }
        }
    }
    if let Some(terminal) = terminal {
        Ok(MergedCompletion::Terminal(terminal))
    } else if matches!(
        plan.order,
        ExecutionOrder::ResultChain
            | ExecutionOrder::Dataflow
            | ExecutionOrder::GroupTail
            | ExecutionOrder::GroupCapture
            | ExecutionOrder::ParallelGroupTail
            | ExecutionOrder::ParallelGroupCapture
            | ExecutionOrder::GroupDataflow
            | ExecutionOrder::ParallelGroupDataflow
            | ExecutionOrder::GroupConditional
            | ExecutionOrder::ParallelGroupConditional
    ) && finish_binding
    {
        Ok(MergedCompletion::Value(
            *fields
                .pop()
                .ok_or(MergeCompletionError::MissingFinalValue)?
                .value,
        ))
    } else {
        Ok(MergedCompletion::Value(Value::Structured { fields }))
    }
}
