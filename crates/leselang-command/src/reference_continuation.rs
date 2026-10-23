//! Released product continuation codec and static admission, without a VM.
//!
//! A decoded image is mutable protocol data, not permission to restore or execute.
//! Hosts still own authentication, complete group journals, live-state conflicts,
//! consumed receipts, clocks, scheduler admission and identity allocation.

use leselang_hir::canonical_source;
use leselang_runtime_core::Fault;

use crate::bounded_json::{self, BoundedJsonDecodeError, BoundedJsonError};
use crate::continuation_identity::canonical_continuation_sequence;
use crate::reference_request::{
    continuation_admission_fault, result_binding_limits, validate_continuation_image,
};
use crate::{ContinuationAdmissionError, ContinuationImage};

pub const MAX_CONTINUATION_BYTES: usize = 64 * 1024;

/// Check recursive structure and size before static admission, preserving wire bytes.
pub fn encode_continuation(image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
    validate_continuation_encoding_size(image)?;
    validate_restorable_image(image)?;
    bounded_json::encode(image, MAX_CONTINUATION_BYTES).map_err(encoding_fault)
}

/// Apply the encoding gates without retaining a serialized output buffer.
pub fn validate_continuation_size(image: &ContinuationImage) -> Result<(), Fault> {
    validate_continuation_encoding_size(image)?;
    validate_restorable_image(image)
}

/// Legacy static restoration gates, not live-state or complete-journal admission.
/// This stage alone does not bound the encoded size; prefer the codec at ingress.
pub fn validate_restorable_image(image: &ContinuationImage) -> Result<(), Fault> {
    validate_continuation_image(image)?;
    canonical_continuation_sequence(&image.token)
        .ok_or_else(invalid_continuation_sequence_fault)?;
    validate_pending_effect_contract(image)
}

/// Structural/size preflight only. Journal callers retain their later request gates.
pub fn validate_continuation_encoding_size(image: &ContinuationImage) -> Result<(), Fault> {
    if let Some(binding) = &image.result_binding {
        // Reject recursive native inputs before entering the serializer.
        let limits = result_binding_limits();
        crate::validate_result_binding_structure(binding, limits.captures, limits.groups, |name| {
            crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
        })
        .map_err(|_| continuation_admission_fault(ContinuationAdmissionError::Binding))?;
    }
    bounded_json::measure(image, MAX_CONTINUATION_BYTES)
        .map(|_| ())
        .map_err(encoding_fault)
}

/// Bound raw input before JSON parsing, then apply the released static frame gates.
/// Whitespace, legacy schemas and per-field serde policies remain unchanged.
pub fn decode_continuation(bytes: &[u8]) -> Result<ContinuationImage, Fault> {
    let image =
        bounded_json::decode(bytes, MAX_CONTINUATION_BYTES).map_err(|error| match error {
            BoundedJsonDecodeError::Limit { limit, .. } => Fault {
                code: "LSV3002".to_string(),
                message: format!("continuation exceeds {limit} bytes"),
            },
            BoundedJsonDecodeError::Deserialize(error) => Fault {
                code: "LSV3003".to_string(),
                message: format!("invalid continuation: {error}"),
            },
        })?;
    validate_restorable_image(&image)?;
    Ok(image)
}

fn validate_pending_effect_contract(image: &ContinuationImage) -> Result<(), Fault> {
    canonical_source(&image.pending_effect).map_err(|_| Fault {
        code: "LSV2017".to_string(),
        message: "continuation pending effect is not canonical".to_string(),
    })?;
    Ok(())
}

/// Shared diagnostic for static admission and the runtime's own restore boundary.
pub fn invalid_continuation_sequence_fault() -> Fault {
    Fault {
        code: "LSV2006".to_string(),
        message: "continuation token has a non-canonical or exhausted sequence".to_string(),
    }
}

fn encoding_fault(error: BoundedJsonError) -> Fault {
    match error {
        BoundedJsonError::Limit { limit, .. } => Fault {
            code: "LSV3002".to_string(),
            message: format!("continuation exceeds {limit} bytes"),
        },
        BoundedJsonError::Serialize(error) => Fault {
            code: "LSV3001".to_string(),
            message: format!("failed to encode continuation: {error}"),
        },
    }
}
