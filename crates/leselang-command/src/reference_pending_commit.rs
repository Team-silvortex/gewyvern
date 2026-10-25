//! Reference pending-record registration over native journal ownership.

use crate::reference_continuation::validate_continuation_encoding_size;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::reference_request::validate_effect_request;
use crate::retention_policy::MAX_JOURNAL_RECORDS;
use crate::{ContinuationImage, ContinuationToken, EffectRequest, SchedulerLimits};
use leselang_runtime_core::Fault;

pub use crate::reference_recovery::{StoredDispatch, StoredEffect};

pub trait MemoryPendingJournal {
    fn request(&self, token: &ContinuationToken) -> Option<&EffectRequest>;
    fn check_capacity(&self, limits: SchedulerLimits) -> Result<(), Fault>;
    fn insert_pending(&mut self, token: ContinuationToken, request: &EffectRequest);
}

/// No request means no work in memory. Otherwise static request/encoding checks
/// precede lookup. Identical requests bypass capacity regardless of dispatch
/// phase; this primitive never resets their lease or terminal. Public entry owns
/// live replay/adoption. The native map clones the payload only for insertion.
pub fn register_memory<Host: MemoryPendingJournal + ?Sized>(
    host: &mut Host,
    request: Option<&EffectRequest>,
    limits: SchedulerLimits,
) -> Result<(), Fault> {
    let Some(request) = request else {
        return Ok(());
    };
    validate_effect_request(request)?;
    validate_continuation_encoding_size(&request.continuation)?;
    let token = request.continuation.token.clone();
    if let Some(existing) = host.request(&token) {
        return if existing == request {
            Ok(())
        } else {
            Err(fault(
                "LSV4011",
                "effect request conflicts with pending state",
            ))
        };
    }
    host.check_capacity(limits)?;
    host.insert_pending(token, request);
    Ok(())
}

/// Exact bounded buffers and canonical sequence admitted by the native entry,
/// not arbitrary serialized input or permission to register/dispatch work.
pub struct PendingEncoding<'a> {
    pub image: &'a [u8],
    pub request: Option<&'a [u8]>,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingCommit {
    Existing,
    New,
}

/// One immediate transaction after native image/request/correspondence admission,
/// bounded encoding and sequence parsing. Counts, quota, signed deadline storage,
/// writes and watermark reconciliation retain native diagnostics and atomicity.
/// Consuming commit is never retried or compensated after an uncertain outcome.
pub trait DurablePendingTransaction {
    fn effect(&mut self, token: &str) -> Result<Option<StoredEffect>, Fault>;
    fn dispatch(&mut self, token: &str) -> Result<Option<StoredDispatch>, Fault>;
    fn record_count(&mut self) -> Result<usize, Fault>;
    fn check_capacity(&mut self, limits: SchedulerLimits) -> Result<(), Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn insert_effect(&mut self, image: &ContinuationImage, bytes: &[u8]) -> Result<(), Fault>;
    fn insert_dispatch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault>;
    fn advance_watermark(&mut self, sequence: u64) -> Result<(), Fault>;
    fn commit(self, kind: PendingCommit) -> Result<(), Fault>;
}

/// Existing pending images must match exactly. Only callers with a request check
/// the original dispatch, including native history/counter reads and bounded
/// request decoding. Replay repairs the watermark but bypasses capacity and quota.
/// Fresh bare images bypass dispatch capacity, not record/quota limits. The owner
/// must supply matching request/encoding options; no new recovery admission or
/// missing-dispatch repair is introduced by these trusted hooks.
pub fn register_durable<Host: DurablePendingTransaction>(
    mut host: Host,
    image: &ContinuationImage,
    request: Option<&EffectRequest>,
    limits: SchedulerLimits,
    encoding: PendingEncoding<'_>,
) -> Result<(), Fault> {
    let token = image.token.as_str();
    if let Some(existing) = host.effect(token)? {
        if existing.state == "pending" && existing.image == encoding.image {
            if let Some(request) = request {
                let dispatch = host.dispatch(token)?.ok_or_else(|| {
                    fault("LSV4011", "pending effect is missing its durable dispatch")
                })?;
                let durable_request: EffectRequest =
                    decode_journal_value(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
                if durable_request != *request {
                    return Err(fault(
                        "LSV4011",
                        "effect request conflicts with durable dispatch state",
                    ));
                }
            }
            host.advance_watermark(encoding.sequence)?;
            return host.commit(PendingCommit::Existing);
        }
        return Err(fault(
            "LSV4011",
            "continuation token conflicts with durable journal state",
        ));
    }
    if host.record_count()? >= MAX_JOURNAL_RECORDS {
        return Err(fault(
            "LSV4006",
            format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
        ));
    }
    if request.is_some() {
        host.check_capacity(limits)?;
    }
    host.ensure_growth(encoding.image.len() + encoding.request.map_or(0, <[u8]>::len))?;
    host.insert_effect(image, encoding.image)?;
    if let Some(bytes) = encoding.request {
        host.insert_dispatch(token, bytes)?;
    }
    host.advance_watermark(encoding.sequence)?;
    host.commit(PendingCommit::New)
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
