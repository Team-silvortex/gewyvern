//! Product group-progress projection over original memory state or one durable snapshot.

use crate::reference_completion::validate_terminal_step;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::reference_request::validate_effect_request;
pub use crate::reference_settlement::visibility::MergeProgress;
use crate::{ContinuationToken, EffectRequest, MAX_CONTINUATION_BYTES, MergePlan, Step};
use leselang_runtime_core::Fault;

pub struct MemoryProgressGroup<'a> {
    pub token: &'a ContinuationToken,
    pub plan: &'a MergePlan,
    pub branches: &'a [ContinuationToken],
    pub terminal: Option<&'a Step>,
}
pub struct MemoryProgressDispatch<'a> {
    pub request: &'a EffectRequest,
    pub acknowledged: bool,
    pub terminal: bool,
}
/// Borrow the first owning group and original dispatches, without admission,
/// eager copies or repair. Acknowledgement and terminal presence are distinct.
pub trait MemoryGroupProgress {
    fn locate(&self, branch: &ContinuationToken) -> Option<MemoryProgressGroup<'_>>;
    fn dispatch(&self, token: &ContinuationToken) -> Option<MemoryProgressDispatch<'_>>;
}

pub fn read_memory<Host: MemoryGroupProgress + ?Sized>(
    host: &Host,
    branch: &ContinuationToken,
) -> MergeProgress {
    let Some(group) = host.locate(branch) else {
        return MergeProgress::Standalone;
    };
    if let Some(step) = group.terminal {
        return MergeProgress::Completed {
            merge_token: group.token.clone(),
            step: Box::new(step.clone()),
            branch_tokens: group.branches.to_vec(),
        };
    }
    if (group.plan.order.is_sequential()
        || group
            .plan
            .result_binding
            .as_ref()
            .is_some_and(|b| b.has_successor(group.plan)))
        && let Some(dispatch) = group
            .branches
            .iter()
            .filter_map(|t| host.dispatch(t))
            .find(|d| !d.acknowledged)
    {
        return MergeProgress::Next {
            request: Box::new(dispatch.request.clone()),
            completed_tokens: group
                .branches
                .iter()
                .filter(|t| host.dispatch(t).is_some_and(|d| d.acknowledged))
                .cloned()
                .collect(),
        };
    }
    MergeProgress::Pending {
        merge_token: group.token.clone(),
        completed_branches: group
            .branches
            .iter()
            .filter(|t| host.dispatch(t).is_some_and(|d| d.terminal))
            .count(),
        total_branches: group.branches.len(),
    }
}

pub struct StoredGroupProgress {
    pub token: String,
    pub state: String,
    pub terminal: Option<Vec<u8>>,
    pub completed: i64,
    pub total: i64,
    pub order: String,
}
/// All hooks must read the same native snapshot, opened before `locate` and
/// held until this projection returns. Keep ordered SQL and storage diagnostics
/// native. No commit, fallback, retry or cache mutation is part of this surface.
pub trait DurableGroupProgress {
    fn locate(&mut self, branch: &str) -> Result<Option<StoredGroupProgress>, Fault>;
    fn plan(&mut self, owner: &str) -> Result<Vec<u8>, Fault>;
    fn next_request(&mut self, owner: &str) -> Result<Vec<u8>, Fault>;
    fn completed_tokens(&mut self, owner: &str) -> Result<Vec<ContinuationToken>, Fault>;
    fn branch_tokens(&mut self, owner: &str) -> Result<Vec<ContinuationToken>, Fault>;
}

pub fn read_durable<Host: DurableGroupProgress + ?Sized>(
    host: &mut Host,
    branch: &ContinuationToken,
) -> Result<MergeProgress, Fault> {
    let Some(record) = host.locate(branch.as_str())? else {
        return Ok(MergeProgress::Standalone);
    };
    let merge_token = ContinuationToken::from_wire(record.token);
    let completed_branches = usize::try_from(record.completed)
        .map_err(|_| invalid("merge completion count is invalid"))?;
    let total_branches =
        usize::try_from(record.total).map_err(|_| invalid("merge branch count is invalid"))?;
    match (record.state.as_str(), record.terminal) {
        ("pending", None) if completed_branches < total_branches => {
            let successor_ready =
                if record.order == "parallel" && completed_branches + 1 == total_branches {
                    let bytes = host.plan(merge_token.as_str())?;
                    let plan: MergePlan = decode_journal_value(&bytes, MAX_CONTINUATION_BYTES)?;
                    plan.order.has_group_successor()
                        && plan
                            .result_binding
                            .as_ref()
                            .is_some_and(|b| b.has_successor(&plan))
                } else {
                    false
                };
            if record.order == "sequential" || successor_ready {
                let bytes = host.next_request(merge_token.as_str())?;
                let request: EffectRequest = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                validate_effect_request(&request)?;
                let completed_tokens = host.completed_tokens(merge_token.as_str())?;
                return Ok(MergeProgress::Next {
                    request: Box::new(request),
                    completed_tokens,
                });
            }
            Ok(MergeProgress::Pending {
                merge_token,
                completed_branches,
                total_branches,
            })
        }
        ("completed", Some(bytes)) if completed_branches == total_branches => {
            let step: Step = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&step)?;
            let branch_tokens = host.branch_tokens(merge_token.as_str())?;
            Ok(MergeProgress::Completed {
                merge_token,
                step: Box::new(step),
                branch_tokens,
            })
        }
        _ => Err(invalid("merge progress has an invalid durable state")),
    }
}

fn invalid(message: &str) -> Fault {
    Fault {
        code: "LSV4007".into(),
        message: message.into(),
    }
}
