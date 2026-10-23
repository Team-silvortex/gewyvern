//! Product single-group recovery decisions, without storage or execution authority.
//!
//! The owner must admit the original plan and token, bound/read one snapshot and
//! check cross-group reservations before entry. Bound receipt/record proofs still
//! belong to the product runtime. Unbound correspondence is data-only. Inspection never commits,
//! executes saved code or grants permission to publish a recovered checkpoint.

use leselang_runtime_core::{
    Fault, GroupGraphError, GroupGraphMember, GroupReceiptError, GroupRecoveryError,
    GroupRecoveryHost, GroupRecoveryPolicy, SequenceFrontierError, SequentialMemberState,
    StoredGroupState, inspect_group_recovery, inspect_sequential_frontier,
};

use crate::reference_completion::{sequence_stop, terminal_step_outcome, validate_terminal_step};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{
    BranchCompletion, ContinuationToken, ExecutionOrder, MAX_MERGE_BRANCHES, MergePlan, Step,
};

/// Original name, signed position, state, optional receipt bytes and effect token.
/// Unchecked product observations, not an admitted graph or portable storage ABI.
pub type RecoveredMergeBranch = (String, i64, String, Option<Vec<u8>>, String);

/// Proofs over the same original owner/plan/rows passed to inspection. Hooks must
/// bound work and preserve native faults, without dispatch, repair or commit.
/// Receipt correspondence must compare original data without evaluating saved DSL.
/// No Clone/Send requirement is imposed on a borrowed, possibly unsized owner.
pub trait ReferenceGroupProofs {
    fn validate_chain(&mut self) -> Result<(), Fault>;
    fn validate_blocked_dispatches(&mut self, first_pending: usize) -> Result<(), Fault>;
    fn validate_unbound_receipt(&mut self, terminal: &Step) -> Result<(), Fault>;
    fn validate_records(&mut self, terminal: Option<&Step>) -> Result<(), Fault>;
}

/// Preserve graph -> chain -> sequence -> state/receipt -> records ordering.
/// Plan admission is deliberately not repeated here: moving it earlier/later would
/// change the released batch's token/collision/reservation/order fault precedence.
pub fn inspect_group_record<Proofs: ReferenceGroupProofs + ?Sized, Bytes: AsRef<[u8]>>(
    proofs: &mut Proofs,
    token: &ContinuationToken,
    plan: &MergePlan,
    branches: &[RecoveredMergeBranch],
    state: &str,
    terminal_bytes: Option<Bytes>,
) -> Result<(), Fault> {
    inspect_group_recovery(
        token.as_str(),
        &plan.branches,
        branches,
        GroupRecoveryPolicy {
            max_members: MAX_MERGE_BRANCHES,
            require_chain: matches!(
                plan.order,
                ExecutionOrder::ResultChain | ExecutionOrder::Dataflow
            ),
            require_pending_sequence: plan.order.is_sequential() && state == "pending",
        },
        terminal_bytes,
        &mut GroupPolicy {
            proofs,
            state,
            encoded: std::marker::PhantomData::<fn() -> Bytes>,
        },
    )
    .map(|_| ())
    .map_err(|error| match error {
        GroupRecoveryError::Graph(GroupGraphError::Native { error, .. })
        | GroupRecoveryError::Chain(error)
        | GroupRecoveryError::Sequence(error)
        | GroupRecoveryError::State(error)
        | GroupRecoveryError::Records(error)
        | GroupRecoveryError::Receipt(GroupReceiptError::Native(error)) => error,
        GroupRecoveryError::Graph(_) => invalid_graph(),
        GroupRecoveryError::Receipt(GroupReceiptError::State) => invalid_group_receipt_state(),
    })
}

struct GroupPolicy<'a, Proofs: ReferenceGroupProofs + ?Sized, Bytes> {
    proofs: &'a mut Proofs,
    state: &'a str,
    encoded: std::marker::PhantomData<fn() -> Bytes>,
}

impl<Proofs: ReferenceGroupProofs + ?Sized, Bytes: AsRef<[u8]>>
    GroupRecoveryHost<RecoveredMergeBranch, str> for GroupPolicy<'_, Proofs, Bytes>
{
    type Encoded = Bytes;
    type Receipt = Step;
    type Error = Fault;

    fn member<'a>(
        &mut self,
        _: usize,
        branch: &'a RecoveredMergeBranch,
    ) -> Result<GroupGraphMember<'a, str>, Fault>
    where
        str: 'a,
    {
        Ok(GroupGraphMember {
            name: &branch.0,
            position: usize::try_from(branch.1).ok(),
            identity: branch.4.as_str(),
        })
    }
    fn is_completed(&mut self, branch: &RecoveredMergeBranch) -> bool {
        branch.2 == "completed"
    }
    fn validate_chain(&mut self) -> Result<(), Fault> {
        self.proofs.validate_chain()
    }
    fn validate_pending_sequence(
        &mut self,
        branches: &[RecoveredMergeBranch],
    ) -> Result<(), Fault> {
        inspect_pending_sequence(branches, |first_pending| {
            self.proofs.validate_blocked_dispatches(first_pending)
        })
    }
    fn stored_state(&mut self) -> Result<StoredGroupState, Fault> {
        match self.state {
            "pending" => Ok(StoredGroupState::Pending),
            "completed" => Ok(StoredGroupState::Completed),
            _ => Err(invalid_group_receipt_state()),
        }
    }
    fn admit_receipt(&mut self, bytes: Bytes) -> Result<Step, Fault> {
        let terminal = decode_journal_value(bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&terminal)?;
        self.proofs.validate_unbound_receipt(&terminal)?;
        Ok(terminal)
    }
    fn validate_records(&mut self, terminal: Option<&Step>) -> Result<(), Fault> {
        self.proofs.validate_records(terminal)
    }
}

/// Inspect the original completed prefix before querying blocked dispatches.
/// All prefix receipts decode before stop/outcome checks, including entries after
/// an early stop. The active frontier is excluded from the blocked suffix query.
pub fn inspect_pending_sequence(
    branches: &[RecoveredMergeBranch],
    validate_blocked: impl FnOnce(usize) -> Result<(), Fault>,
) -> Result<(), Fault> {
    inspect_sequential_frontier(
        branches,
        MAX_MERGE_BRANCHES,
        |_, branch| {
            Ok(if branch.2 == "pending" {
                SequentialMemberState::Pending
            } else {
                SequentialMemberState::Completed
            })
        },
        |prefix| {
            let terminal_steps = prefix
                .iter()
                .map(|branch| {
                    decode_journal_value::<Step>(
                        branch
                            .3
                            .as_deref()
                            .ok_or_else(|| fault("completed sequence step has no result"))?,
                        MAX_JOURNAL_ENTRY_BYTES,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if sequence_stop(terminal_steps.iter())?.is_some() {
                return Err(fault("failed sequence still has pending successors"));
            }
            Ok(())
        },
        |first_pending, _| validate_blocked(first_pending),
    )
    .map_err(|error| match error {
        SequenceFrontierError::Native { error, .. } => error,
        SequenceFrontierError::OutOfOrder { .. } => fault("sequence completion is out of order"),
        _ => invalid_graph(),
    })
}

/// Compare an unbound final receipt with original branch data only. Bound plans
/// deliberately skip this policy; their owner must validate binding records.
/// This is correspondence, not authenticity, graph admission or publication.
pub fn validate_unbound_group_receipt(
    plan: &MergePlan,
    branches: &[RecoveredMergeBranch],
    terminal: &Step,
) -> Result<(), Fault> {
    if plan.result_binding.is_some() {
        return Ok(());
    }
    let invalid = || fault("merge group receipt does not match original branch results");
    if branches.len() != plan.branches.len() || branches.len() > MAX_MERGE_BRANCHES {
        return Err(invalid());
    }
    let steps = branches
        .iter()
        .map(|branch| {
            decode_journal_value::<Step>(
                branch.3.as_deref().ok_or_else(invalid)?,
                MAX_JOURNAL_ENTRY_BYTES,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    // Reproduce the original data-only merge, including sequential early stops.
    // Bound expressions are validated separately; no DSL or saved body runs here.
    let stop = if plan.order.is_sequential() {
        sequence_stop(steps.iter())?
    } else {
        None
    };
    let expected = if let Some(stop) = stop {
        stop
    } else {
        let completions = branches
            .iter()
            .zip(steps)
            .map(|(branch, step)| {
                Ok(BranchCompletion {
                    branch: branch.0.clone(),
                    outcome: terminal_step_outcome(step)?,
                })
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        // Unbound plans have no successor authority. Keep the live finalization
        // refusal before plan admission, without exposing any evaluator hook.
        let result = if plan.order.has_group_successor() {
            Err(crate::reference_merge::invalid_group_binding())
        } else {
            crate::reference_merge_output::assemble_merge(
                plan,
                completions,
                crate::reference_request::DEFAULT_MAX_OUTPUT_ITEMS,
                true,
                |_, _, _| Err(crate::reference_merge::invalid_group_binding()),
            )
        };
        crate::reference_merge_output::terminal_merge(plan, result)?
    };
    if &expected != terminal {
        return Err(invalid());
    }
    Ok(())
}

fn invalid_graph() -> Fault {
    fault("merge branches do not match their declared plan")
}
fn invalid_group_receipt_state() -> Fault {
    fault("merge group state conflicts with its branch graph")
}
fn fault(message: &str) -> Fault {
    Fault {
        code: "LSV4007".into(),
        message: message.into(),
    }
}
