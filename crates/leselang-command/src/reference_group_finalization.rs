//! Reference group-finalization policy inside the caller's native settlement.

use crate::reference_completion::{sequence_stop, terminal_step_outcome, validate_terminal_step};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{BranchCompletion, ContinuationToken, MAX_CONTINUATION_BYTES, MergePlan, Step};
use leselang_runtime_core::Fault;

/// Reuse the original merge evaluator and successor authority. `None` means a
/// successor was admitted, not that the group may publish a terminal receipt.
pub trait GroupFinalizationOwner {
    fn merge_prefix(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault>;
    fn admit_successor(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        prefix: &Step,
    ) -> Result<Option<Step>, Fault>;
    fn finish(
        &mut self,
        plan: &MergePlan,
        completions: Vec<BranchCompletion>,
    ) -> Result<Step, Fault>;
}

pub struct MemoryGroupState<'a> {
    pub plan: &'a MergePlan,
    pub branches: &'a [ContinuationToken],
    pub completed: bool,
}
pub struct MemoryStoppedBranch<'a> {
    pub acknowledged: &'a mut bool,
    pub lease_expires_at_ms: &'a mut Option<u64>,
    pub terminal_step: &'a mut Option<Step>,
}
pub trait MemoryGroupFinalization: GroupFinalizationOwner {
    fn locate(&self, branch: &ContinuationToken) -> Option<ContinuationToken>;
    fn group(&self, token: &ContinuationToken) -> Option<MemoryGroupState<'_>>;
    fn branch_terminal(&self, token: &ContinuationToken) -> Option<&Step>;
    fn branch_mut(&mut self, token: &ContinuationToken) -> Option<MemoryStoppedBranch<'_>>;
    fn group_terminal_mut(&mut self, token: &ContinuationToken) -> Option<&mut Option<Step>>;
}

/// Completed groups replay without validating their receipt. Sequential stop
/// reads remain lazy; acknowledged/missing branches are not repaired. As before,
/// memory mutations are not rolled back if a later owner callback fails.
pub fn finalize_memory<Host: MemoryGroupFinalization + ?Sized>(
    host: &mut Host,
    branch: &ContinuationToken,
) -> Result<(), Fault> {
    let Some(token) = host.locate(branch) else {
        return Ok(());
    };
    let group = host.group(&token).ok_or_else(missing_group)?;
    if group.completed {
        return Ok(());
    }
    let plan = group.plan.clone();
    let branches = group.branches.to_vec();
    if plan.order.is_sequential()
        && let Some(stop) = sequence_stop(branches.iter().filter_map(|t| host.branch_terminal(t)))?
    {
        for t in &branches {
            if let Some(state) = host.branch_mut(t)
                && !*state.acknowledged
            {
                *state.acknowledged = true;
                *state.lease_expires_at_ms = None;
                *state.terminal_step = Some(stop.clone());
            }
        }
        *host
            .group_terminal_mut(&token)
            .ok_or_else(|| fault("LSV4007", "sequence disappeared"))? = Some(stop);
        return Ok(());
    }
    let mut completions = Vec::with_capacity(branches.len());
    for (name, t) in plan.branches.iter().zip(&branches) {
        let Some(step) = host.branch_terminal(t).cloned() else {
            return Ok(());
        };
        completions.push(BranchCompletion {
            branch: name.clone(),
            outcome: terminal_step_outcome(step)?,
        });
    }
    let Some(merged) = merge(host, &token, &plan, completions)? else {
        return Ok(());
    };
    *host.group_terminal_mut(&token).ok_or_else(missing_group)? = Some(merged);
    Ok(())
}

pub struct StoredFinalizingGroup {
    pub token: String,
    pub plan: Vec<u8>,
    pub state: String,
    pub terminal: Option<Vec<u8>>,
}
pub struct StoredFinalizingBranch {
    pub name: String,
    pub state: String,
    pub terminal: Option<Vec<u8>>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupTerminalEncoding {
    SequenceStop,
    MergeResult,
}

/// Borrow the existing settlement transaction. No begin/commit/retry capability
/// is exposed: the caller owns atomicity together with the triggering branch.
pub trait DurableGroupFinalization: GroupFinalizationOwner {
    fn locate(&mut self, branch: &str) -> Result<Option<StoredFinalizingGroup>, Fault>;
    fn branches(&mut self, token: &str) -> Result<Vec<StoredFinalizingBranch>, Fault>;
    fn encode_terminal(
        &mut self,
        step: &Step,
        kind: GroupTerminalEncoding,
    ) -> Result<Vec<u8>, Fault>;
    fn pending_tokens(&mut self, token: &str) -> Result<Vec<String>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete_branch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault>;
    fn stop_group(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault>;
    fn complete_group(&mut self, token: &str, bytes: &[u8]) -> Result<usize, Fault>;
}

/// Retain completed-receipt admission before plan decoding, branch reads before
/// plan decoding, and eager sequential byte decoding before stop selection. A
/// pending branch masks invalid later states on the ordinary merge route.
pub fn finalize_durable<Host: DurableGroupFinalization + ?Sized>(
    host: &mut Host,
    branch: &str,
) -> Result<(), Fault> {
    let Some(group) = host.locate(branch)? else {
        return Ok(());
    };
    if group.state == "completed" {
        let bytes = group
            .terminal
            .ok_or_else(|| fault("LSV4007", "completed merge group has no terminal step"))?;
        let terminal = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&terminal)?;
        return Ok(());
    }
    if group.state != "pending" || group.terminal.is_some() {
        return Err(fault("LSV4007", "merge group has an invalid durable state"));
    }
    let branches = host.branches(&group.token)?;
    let plan: MergePlan = decode_journal_value(&group.plan, MAX_CONTINUATION_BYTES)?;
    if plan.order.is_sequential() {
        let steps = branches
            .iter()
            .filter_map(|b| b.terminal.as_ref())
            .map(|bytes| decode_journal_value::<Step>(bytes, MAX_JOURNAL_ENTRY_BYTES))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(stop) = sequence_stop(steps.iter())? {
            let bytes = host.encode_terminal(&stop, GroupTerminalEncoding::SequenceStop)?;
            let pending = host.pending_tokens(&group.token)?;
            host.ensure_growth(bytes.len().saturating_mul(pending.len() + 1))?;
            for t in pending {
                host.complete_branch(&t, &bytes)?;
            }
            host.stop_group(&group.token, &bytes)?;
            return Ok(());
        }
    }
    if branches.iter().any(|b| b.state == "pending") {
        return Ok(());
    }
    if branches
        .iter()
        .any(|b| b.state != "completed" || b.terminal.is_none())
    {
        return Err(fault(
            "LSV4007",
            "merge branch has an invalid durable state",
        ));
    }
    let completions = branches
        .into_iter()
        .map(|b| {
            let bytes = b
                .terminal
                .ok_or_else(|| fault("LSV4007", "completed merge branch has no terminal step"))?;
            let step = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            Ok(BranchCompletion {
                branch: b.name,
                outcome: terminal_step_outcome(step)?,
            })
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    let token = ContinuationToken::from_wire(group.token);
    let Some(merged) = merge(host, &token, &plan, completions)? else {
        return Ok(());
    };
    validate_terminal_step(&merged)?;
    let bytes = host.encode_terminal(&merged, GroupTerminalEncoding::MergeResult)?;
    host.ensure_growth(bytes.len())?;
    if host.complete_group(token.as_str(), &bytes)? != 1 {
        return Err(fault(
            "LSV4033",
            "merge group completion lost its pending state",
        ));
    }
    Ok(())
}

fn merge<Host: GroupFinalizationOwner + ?Sized>(
    host: &mut Host,
    token: &ContinuationToken,
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
) -> Result<Option<Step>, Fault> {
    let needs_successor = plan.order.has_group_successor()
        && plan
            .result_binding
            .as_ref()
            .is_some_and(|binding| !binding.has_successor(plan));
    if needs_successor || plan.order.captures_group_successor() {
        let prefix = host.merge_prefix(plan, completions)?;
        host.admit_successor(token, plan, &prefix)
    } else {
        host.finish(plan, completions).map(Some)
    }
}
fn missing_group() -> Fault {
    fault("LSV4033", "located merge group disappeared")
}
fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
