//! Product completion-cache reconciliation and visible group-result selection.
//!
//! Observations come from an already admitted native journal. These transient
//! values are not a wire format, durable receipt, dispatch lease or authority to
//! execute saved code. The native owner retains bounded reads and cache storage.

use crate::{ContinuationToken, EffectRequest, MergeWait, Step};
use leselang_runtime_core::Fault;

pub enum EffectObservation {
    /// An image-only memory restore has no outbox row; retain its local pending state.
    Untracked,
    /// A durable row is absent, including after retention; discard stale pending state.
    Missing,
    Pending,
    Completed(Box<Step>),
}

pub enum MergeProgress {
    Standalone,
    Next {
        request: Box<EffectRequest>,
        completed_tokens: Vec<ContinuationToken>,
    },
    Pending {
        merge_token: ContinuationToken,
        completed_branches: usize,
        total_branches: usize,
    },
    Completed {
        merge_token: ContinuationToken,
        step: Box<Step>,
        branch_tokens: Vec<ContinuationToken>,
    },
}

/// Original native cache operations, without eager copies or fallback reads.
/// Hooks are called once in order; a fault/unwind does not retry or undo earlier
/// effects. After successful reconciliation the owner returns its original cached
/// borrow; the adapter neither clones it nor extends the owner's borrowing scope.
pub trait ReferenceCompletionCacheHost {
    fn has_completed(&self, token: &ContinuationToken) -> bool;
    fn completed_exists(&mut self, token: &ContinuationToken) -> Result<bool, Fault>;
    fn observe_effect(&mut self, token: &ContinuationToken) -> Result<EffectObservation, Fault>;
    fn remove_completed(&mut self, token: &ContinuationToken);
    fn remove_pending(&mut self, token: &ContinuationToken);
    fn insert_completed(&mut self, token: &ContinuationToken, step: Step);
}

/// Reconcile only the original branch: a cached hit checks existence, never
/// refreshes its payload or falls through to observation after eviction.
pub fn reconcile_completed<Host: ReferenceCompletionCacheHost + ?Sized>(
    token: &ContinuationToken,
    host: &mut Host,
) -> Result<(), Fault> {
    if host.has_completed(token) {
        if !host.completed_exists(token)? {
            host.remove_completed(token);
        }
    } else {
        match host.observe_effect(token)? {
            EffectObservation::Missing => host.remove_pending(token),
            EffectObservation::Untracked | EffectObservation::Pending => {}
            EffectObservation::Completed(step) => {
                host.remove_pending(token);
                host.insert_completed(token, *step);
            }
        }
    }
    Ok(())
}

/// Native publication primitives. Next-request caching copies the original
/// continuation only after removals. Merged completion uses lazy entry insertion
/// so an existing result is never replaced or needlessly copied. The final debug
/// hook retains the owner's diagnostic-only receipt check, not an admission gate.
pub trait ReferenceGroupVisibilityHost {
    fn merge_progress(&mut self, token: &ContinuationToken) -> Result<MergeProgress, Fault>;
    fn remove_pending(&mut self, token: &ContinuationToken) -> bool;
    fn cache_next(&mut self, request: &EffectRequest);
    fn cache_merged_if_absent(&mut self, token: ContinuationToken, step: &Step);
    fn debug_assert_merge_result(&mut self, merge_token: &ContinuationToken);
}

/// Publish exactly the native observation, preserving token order and duplicate
/// handling. No additional journal read, commit or cache scan is introduced.
pub fn visible_step<Host: ReferenceGroupVisibilityHost + ?Sized>(
    branch_token: &ContinuationToken,
    branch_step: Step,
    host: &mut Host,
) -> Step {
    match host.merge_progress(branch_token) {
        Ok(MergeProgress::Standalone) => branch_step,
        Ok(MergeProgress::Next {
            request,
            completed_tokens,
        }) => {
            for token in completed_tokens {
                host.remove_pending(&token);
            }
            host.cache_next(&request);
            Step::Effect(request)
        }
        Ok(MergeProgress::Pending {
            merge_token,
            completed_branches,
            total_branches,
        }) => Step::Waiting(MergeWait {
            merge_token,
            completed_branches,
            total_branches,
        }),
        Ok(MergeProgress::Completed {
            merge_token,
            step,
            branch_tokens,
        }) => {
            let step = *step;
            for token in branch_tokens {
                if host.remove_pending(&token) {
                    host.cache_merged_if_absent(token, &step);
                }
            }
            host.debug_assert_merge_result(&merge_token);
            step
        }
        Err(error) => Step::Fault(error),
    }
}
