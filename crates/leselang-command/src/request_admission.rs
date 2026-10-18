//! Complete product request consistency, without transport or execution authority.

use crate::{
    ContinuationAdmissionError, ContinuationAdmissionLimits, EffectRequestView,
    RequestCorrespondenceError, validate_continuation_frame, validate_preflighted_effect_request,
};

/// Preserve which ordered gate refused without retaining any request payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestAdmissionError {
    Frame(ContinuationAdmissionError),
    Correspondence(RequestCorrespondenceError),
}

impl std::fmt::Display for RequestAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frame(error) => std::fmt::Display::fmt(error, formatter),
            Self::Correspondence(error) => std::fmt::Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for RequestAdmissionError {}

/// Admit the complete frame before cold authority, identity, operation and budget
/// correspondence. Prefer this entry for requests without prior frame admission.
///
/// The caller bounds decoded ingress, pending payloads and native callback work.
/// Group-owner text, token syntax and canonical sequence are distinct native
/// policies, called at most once each in the original gate order. Name hooks
/// retain frame-validation traversal. All hooks must observe one stable snapshot;
/// refusal skips later hooks, unused captures drop, and unwind propagates without
/// retry or rollback. Opaque tokens need no Clone, Debug, serde or thread traits.
/// The canonical hook must reject zero, noncanonical and out-of-policy sequences;
/// neither identity hook allocates or authenticates an identity.
///
/// This composes existing checks, not a codec, evaluator or validation certificate.
/// It neither strengthens legacy pending-effect canonicality nor authenticates
/// declared authority/equal budgets. Canonical restore admission, authentic grants,
/// live deadlines, raw receipt provenance, leases and atomic settlement stay with
/// the caller. Mutable data must be checked again after changes or wire decoding.
pub fn validate_effect_request<Token>(
    request: EffectRequestView<'_, Token>,
    limits: ContinuationAdmissionLimits,
    group_owner_text: impl FnOnce(&Token) -> &str,
    validate_token: impl FnOnce(&Token) -> bool,
    validate_group_name: impl FnMut(&str) -> bool,
    canonical_sequence: impl FnOnce(&Token) -> Option<u64>,
) -> Result<(), RequestAdmissionError> {
    validate_continuation_frame(
        request.continuation,
        limits,
        group_owner_text,
        validate_token,
        validate_group_name,
    )
    .map_err(RequestAdmissionError::Frame)?;
    validate_preflighted_effect_request(request, canonical_sequence)
        .map_err(RequestAdmissionError::Correspondence)
}
