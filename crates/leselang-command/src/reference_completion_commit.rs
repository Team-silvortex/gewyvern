//! Reference ordinary-result completion policy over native journal ownership.

use crate::reference_completion::validate_terminal_step;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{ContinuationImage, ContinuationToken, Step};
use leselang_runtime_core::Fault;

pub use crate::reference_recovery::{StoredDispatch, StoredEffect};

pub struct MemoryCompletionState<'a> {
    pub acknowledged: &'a mut bool,
    pub lease_expires_at_ms: Option<u64>,
    pub terminal_step: &'a mut Option<Step>,
}

pub trait MemoryCompletionJournal {
    fn sequence_ready(&self, token: &ContinuationToken) -> bool;
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryCompletionState<'_>>;
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault>;
}

/// The public entry owns admission and winner replay. This primitive checks the
/// sequence first, accepts absent dispatches, and replaces acknowledged terminals
/// without clearing their lease field. Finalization may fail after mutation.
pub fn complete_memory<Host: MemoryCompletionJournal + ?Sized>(
    host: &mut Host,
    image: &ContinuationImage,
    step: &Step,
) -> Result<Step, Fault> {
    if !host.sequence_ready(&image.token) {
        return Err(crate::reference_sequence_gate::predecessor_pending());
    }
    if let Some(state) = host.dispatch(&image.token) {
        if state.lease_expires_at_ms.is_some() && !*state.acknowledged {
            return Err(leased_fault());
        }
        *state.acknowledged = true;
        *state.terminal_step = Some(step.clone());
    }
    host.finalize_group(&image.token)?;
    Ok(step.clone())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionCommit {
    Replay,
    Completion,
}

/// One native immediate transaction, opened after image/proposal admission and
/// bounded encoding. Reads retain native history/counter checks. Native storage
/// owns rollback; consuming commit is never retried or compensated.
pub trait DurableCompletionTransaction {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn dispatch(&mut self, token: &str) -> Result<Option<StoredDispatch>, Fault>;
    fn ensure_sequence_ready(&mut self, token: &str) -> Result<(), Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn complete_effect(&mut self, token: &str, step_bytes: &[u8]) -> Result<(), Fault>;
    fn acknowledge_dispatch(&mut self, token: &str) -> Result<(), Fault>;
    fn finalize_group(&mut self, token: &str) -> Result<(), Fault>;
    fn commit(self, kind: CompletionCommit) -> Result<(), Fault>;
}

/// Encodings must come from the owner's admitted inputs. Replay precedes dispatch
/// and sequence reads; for pending records the lease check precedes sequence
/// readiness (unlike memory). No request decoding or new recovery admission is
/// introduced here, and a missing dispatch keeps its original accepted branch.
pub fn complete_durable<Host: DurableCompletionTransaction>(
    mut host: Host,
    image: &ContinuationImage,
    step: &Step,
    image_bytes: &[u8],
    step_bytes: &[u8],
) -> Result<Step, Fault> {
    let token = image.token.as_str();
    let existing = host.effect(token)?.ok_or_else(|| {
        fault(
            "LSV4013",
            "pending continuation is missing from durable journal",
        )
    })?;
    if existing.image != image_bytes {
        return Err(fault(
            "LSV4014",
            "continuation image conflicts with durable journal state",
        ));
    }
    if existing.state == "completed" {
        let bytes = existing
            .terminal_step
            .ok_or_else(|| fault("LSV4007", "completed journal record has no terminal step"))?;
        let authoritative = decode_journal_value(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_terminal_step(&authoritative)?;
        host.commit(CompletionCommit::Replay)?;
        return Ok(authoritative);
    }
    if let Some(dispatch) = host.dispatch(token)?
        && dispatch.state == "leased"
    {
        return Err(leased_fault());
    }
    host.ensure_sequence_ready(token)?;
    host.ensure_growth(step_bytes.len())?;
    host.complete_effect(token, step_bytes)?;
    host.acknowledge_dispatch(token)?;
    host.finalize_group(token)?;
    host.commit(CompletionCommit::Completion)?;
    Ok(step.clone())
}

fn leased_fault() -> Fault {
    fault(
        "LSV4024",
        "leased effect must be completed through dispatch acknowledgement",
    )
}
fn fault(code: &str, message: &str) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
