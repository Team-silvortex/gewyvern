//! Product continuation import coordination, without storage or execution ownership.

use crate::continuation_identity::canonical_continuation_sequence;
use crate::continuation_schema::{
    CONDITIONAL_CONTINUATION_SCHEMA_VERSION, DATAFLOW_CONTINUATION_SCHEMA_VERSION,
    SUCCESSOR_CONTINUATION_SCHEMA_VERSION,
};
use crate::reference_continuation::{
    invalid_continuation_sequence_fault, validate_continuation_size,
};
use crate::reference_request::validate_effect_request;
use crate::{ContinuationImage, ContinuationToken, EffectRequest, Step};
use leselang_runtime_core::{Fault, SequenceWatermarkRequirement};
use std::collections::BTreeMap;

/// Borrowed reference-cache state, not journal authority or a recovery snapshot.
/// Import uses the existing preflight, then a trusted native commit that must
/// recheck current authority and capacity. No local state changes before commit;
/// success advances the watermark before moving the original image into cache.
/// Native side effects on refusal/unwind are not retried or compensated. Cache
/// publication and native storage are not one atomic transaction.
/// Callback-capture cleanup is trusted and can unwind after successful publication.
pub struct ReferenceRestoreCache<'a> {
    pub pending: &'a mut BTreeMap<ContinuationToken, ContinuationImage>,
    pub completed: &'a BTreeMap<ContinuationToken, Step>,
    pub next_sequence: &'a mut u64,
}

impl ReferenceRestoreCache<'_> {
    pub fn restore(
        self,
        image: ContinuationImage,
        commit: impl FnMut(&ContinuationImage, Option<&EffectRequest>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        restore(
            image,
            &mut CacheImport {
                cache: self,
                commit,
            },
        )
    }

    pub fn restore_request(
        self,
        request: EffectRequest,
        commit: impl FnMut(&ContinuationImage, Option<&EffectRequest>) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        restore_request(
            request,
            &mut CacheImport {
                cache: self,
                commit,
            },
        )
    }
}

struct CacheImport<'a, Commit> {
    cache: ReferenceRestoreCache<'a>,
    commit: Commit,
}

impl<Commit> ReferenceRestoreHost for CacheImport<'_, Commit>
where
    Commit: FnMut(&ContinuationImage, Option<&EffectRequest>) -> Result<(), Fault>,
{
    fn pending(&self, token: &ContinuationToken) -> Option<&ContinuationImage> {
        self.cache.pending.get(token)
    }
    fn is_completed(&self, token: &ContinuationToken) -> bool {
        self.cache.completed.contains_key(token)
    }
    fn commit_existing(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        (self.commit)(image, request)
    }
    fn commit_new(
        &mut self,
        image: ContinuationImage,
        request: Option<&EffectRequest>,
        next_sequence: u64,
    ) -> Result<(), Fault> {
        (self.commit)(&image, request)?;
        *self.cache.next_sequence = (*self.cache.next_sequence).max(next_sequence);
        self.cache.pending.insert(image.token.clone(), image);
        Ok(())
    }
}

/// Trusted owner of local observations and transactional import. Observations are
/// not durable authority: both commit hooks must validate against current journal
/// state and enforce capacity. New imports publish the watermark and original
/// image only after commit. Duplicate imports still enter the journal, but never
/// replace the local image or advance the local watermark. No hook is retried.
pub trait ReferenceRestoreHost {
    fn pending(&self, token: &ContinuationToken) -> Option<&ContinuationImage>;
    fn is_completed(&self, token: &ContinuationToken) -> bool;
    fn commit_existing(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault>;
    fn commit_new(
        &mut self,
        image: ContinuationImage,
        request: Option<&EffectRequest>,
        next_sequence: u64,
    ) -> Result<(), Fault>;
}

/// Import a bare image using the released bounds, schema and conflict order.
/// Callers authenticate provenance and bound ingress. Static validation does not
/// authenticate saved fuel, original authority or receipts, or execute saved code.
pub fn restore<Host: ReferenceRestoreHost + ?Sized>(
    image: ContinuationImage,
    host: &mut Host,
) -> Result<(), Fault> {
    restore_inner(image, None, host)
}

/// Import the original authority envelope. Preserve request validation before
/// image cloning/preflight; the request remains borrowed by once-only commit.
pub fn restore_request<Host: ReferenceRestoreHost + ?Sized>(
    request: EffectRequest,
    host: &mut Host,
) -> Result<(), Fault> {
    validate_effect_request(&request)?;
    restore_inner(request.continuation.clone(), Some(&request), host)
}

fn restore_inner<Host: ReferenceRestoreHost + ?Sized>(
    image: ContinuationImage,
    request: Option<&EffectRequest>,
    host: &mut Host,
) -> Result<(), Fault> {
    validate_continuation_size(&image)?;
    if image.group_result.is_some() {
        return Err(crate::reference_merge::invalid_group_binding());
    }
    if matches!(
        image.schema_version,
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION
            | DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) && request.is_none()
    {
        return Err(Fault {
            code: "LSV1407".into(),
            message: "invalid result-driven successor or missing original effect authority".into(),
        });
    }
    if let Some(current) = host.pending(&image.token) {
        return if current == &image {
            host.commit_existing(&image, request)
        } else {
            Err(Fault {
                code: "LSV2002".into(),
                message: "continuation token conflicts with pending state".into(),
            })
        };
    }
    if host.is_completed(&image.token) {
        return Err(Fault {
            code: "LSV2003".into(),
            message: "continuation was already consumed".into(),
        });
    }
    let sequence = canonical_continuation_sequence(&image.token)
        .ok_or_else(invalid_continuation_sequence_fault)?;
    let next_sequence = SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
        .map_err(|_| Fault {
            code: "LSV1003".into(),
            message: "continuation sequence is exhausted".into(),
        })?
        .required_watermark();
    host.commit_new(image, request, next_sequence)
}
