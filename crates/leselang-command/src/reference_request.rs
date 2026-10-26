//! Released product request data and static consistency policy, without execution ownership.
//!
//! Checked deserialization is not authorization, bounded ingress, canonical source
//! admission or permission to restore. Public mutable fields must be checked again
//! after changes. Live grants, clocks, receipts, identities and durable settlement
//! remain the embedding runtime's responsibility.

use crate::continuation_identity::{
    MAX_EFFECT_SEQUENCE, canonical_continuation_sequence, valid_continuation_token,
};
use crate::{ContinuationToken, EffectOperation, ResultBinding};
use leselang_hir_core::projection_access::ProjectionAccessLimits;
use leselang_host_contract::Revision;
use leselang_runtime_core::{
    CaptureValidationLimits, Fault, GroupCaptureLimits, ResourceBudget, ResourceBudgetError,
    ResourceBudgetLimits,
};
use leserpent_hir::{Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, Type};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

pub const MAX_EXECUTION_FUEL: u64 = 1_000_000;
pub const MAX_EFFECT_DEADLINE_MS: u64 = leselang_runtime_core::MAX_EXECUTION_TIMEOUT_MS;
pub const DEFAULT_MAX_OUTPUT_ITEMS: usize = 10_000;

/// Exact legacy specialization; native saved-frame ownership belongs to runtime-core.
pub type ContinuationImage = leselang_runtime_core::ContinuationFrame<
    ContinuationToken,
    Revision,
    Type,
    Effect,
    ResultBinding,
>;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct EffectRequest {
    pub effect_id: String,
    pub required_capability: String,
    pub operation: EffectOperation,
    pub continuation: ContinuationImage,
    pub budget: ResourceBudget,
}

impl EffectRequest {
    /// Borrow the same mutable-data representation, not a validation certificate.
    pub fn as_view(&self) -> crate::EffectRequestView<'_, ContinuationToken> {
        crate::EffectRequestView {
            effect_id: &self.effect_id,
            required_capability: &self.required_capability,
            operation: &self.operation,
            continuation: &self.continuation,
            budget: &self.budget,
        }
    }
}

impl<'de> Deserialize<'de> for EffectRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let decoded = crate::decode_effect_request(deserializer)?;
        let request = Self {
            effect_id: decoded.effect_id,
            required_capability: decoded.required_capability,
            operation: decoded.operation,
            continuation: decoded.continuation,
            budget: decoded.budget,
        };
        validate_effect_request(&request)
            .map_err(|fault| D::Error::custom(format!("{}: {}", fault.code, fault.message)))?;
        Ok(request)
    }
}

/// Released product ceilings; independent hosts may supply their own generic policy.
pub fn result_binding_limits() -> crate::ResultBindingLimits {
    crate::ResultBindingLimits {
        captures: CaptureValidationLimits {
            max_saved_bindings: MAX_EFFECT_NESTING_DEPTH,
            max_name_bytes: MAX_BRANCH_NAME_BYTES,
        },
        groups: GroupCaptureLimits {
            min_members: 1,
            max_members: crate::MAX_MERGE_BRANCHES - 1,
        },
        accesses: ProjectionAccessLimits {
            max_nodes: leserpent_hir::computation::MAX_COMPUTATION_NODES,
            max_depth: MAX_EFFECT_NESTING_DEPTH,
        },
    }
}

/// Check frame consistency only, not canonical source or restoration eligibility.
pub fn validate_continuation_image(image: &ContinuationImage) -> Result<(), Fault> {
    crate::validate_continuation_frame(
        image,
        continuation_admission_limits(),
        ContinuationToken::as_str,
        valid_continuation_token,
        valid_merge_branch_name,
    )
    .map_err(continuation_admission_fault)
}

/// The released product's static ceilings, not an authenticated resource grant.
pub fn continuation_admission_limits() -> crate::ContinuationAdmissionLimits {
    crate::ContinuationAdmissionLimits {
        binding: result_binding_limits(),
        budget: ResourceBudgetLimits {
            max_fuel: MAX_EXECUTION_FUEL,
            max_deadline_ms: MAX_EFFECT_DEADLINE_MS,
            max_deadline_at_ms: i64::MAX as u64,
            max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        },
        max_group_sequence: MAX_EFFECT_SEQUENCE,
    }
}

/// Preserve released product diagnostics for all frame-admission callers.
pub fn continuation_admission_fault(error: crate::ContinuationAdmissionError) -> Fault {
    use crate::ContinuationAdmissionError;
    use crate::continuation_schema::ContinuationSchemaError;
    let code = match error {
        ContinuationAdmissionError::Binding => "LSV1405",
        ContinuationAdmissionError::Schema(ContinuationSchemaError::Group) => {
            return Fault {
                code: "LSV1409".into(),
                message: "invalid group-result binding or missing complete group journal".into(),
            };
        }
        ContinuationAdmissionError::Schema(ContinuationSchemaError::Version { .. }) => "LSV2001",
        ContinuationAdmissionError::Token => "LSV2006",
        ContinuationAdmissionError::ProgramCounter => "LSV2014",
        ContinuationAdmissionError::StructuredEffect => "LSV2015",
        ContinuationAdmissionError::ResultType => "LSV2016",
        ContinuationAdmissionError::Budget(ResourceBudgetError::Fuel) => "LSV2007",
        ContinuationAdmissionError::Budget(ResourceBudgetError::Deadline) => "LSV2008",
        ContinuationAdmissionError::Budget(ResourceBudgetError::AbsoluteDeadline) => "LSV2011",
        ContinuationAdmissionError::Budget(ResourceBudgetError::Output) => "LSV2009",
    };
    Fault {
        code: code.into(),
        message: error.to_string(),
    }
}

/// Validate the complete frame before identity, operation and budget correspondence.
/// This neither authenticates declared capabilities nor allocates or restores work.
pub fn validate_effect_request(request: &EffectRequest) -> Result<(), Fault> {
    crate::validate_effect_request(
        request.as_view(),
        continuation_admission_limits(),
        ContinuationToken::as_str,
        valid_continuation_token,
        valid_merge_branch_name,
        canonical_continuation_sequence,
    )
    .map_err(request_admission_fault)
}

/// Map ordered generic refusals without changing the released fault vocabulary.
pub fn request_admission_fault(error: crate::RequestAdmissionError) -> Fault {
    match error {
        crate::RequestAdmissionError::Frame(error) => continuation_admission_fault(error),
        crate::RequestAdmissionError::Correspondence(error) => request_correspondence_fault(error),
    }
}

fn request_correspondence_fault(error: crate::RequestCorrespondenceError) -> Fault {
    use crate::{OperationRequestError, RequestCorrespondenceError};
    let code = match error {
        RequestCorrespondenceError::SuccessorAuthority => "LSV1407",
        RequestCorrespondenceError::ContinuationIdentity
        | RequestCorrespondenceError::EffectIdentity => "LSV2010",
        RequestCorrespondenceError::Operation(OperationRequestError::Capability) => "LSV2011",
        RequestCorrespondenceError::Operation(OperationRequestError::Envelope) => "LSV2012",
        RequestCorrespondenceError::Operation(OperationRequestError::Mismatch)
        | RequestCorrespondenceError::Budget => "LSV2013",
    };
    Fault {
        code: code.into(),
        message: error.to_string(),
    }
}

fn valid_merge_branch_name(name: &str) -> bool {
    crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
}
