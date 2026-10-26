//! Released product merge-plan policy, without execution or recovery ownership.
//!
//! Static admission never authenticates saved fuel, reserved identities, original
//! authority or receipts. Hosts still own bounded ingress, complete request graphs,
//! live state and durable settlement. Recheck mutable data after changes.

use leselang_runtime_core::Fault;
use leserpent_hir::Type;

use crate::bounded_json::{self, BoundedJsonError};
use crate::continuation_identity::MAX_EFFECT_SEQUENCE;
use crate::reference_request::{
    MAX_EXECUTION_FUEL, continuation_admission_fault, request_admission_fault,
    result_binding_limits,
};
use crate::{
    ContinuationAdmissionError, GroupBindingError, GroupBindingLimits, GroupPlanLimits,
    GroupResultBinding, MAX_CONTINUATION_BYTES, MAX_MERGE_BRANCH_NAME_BYTES, MAX_MERGE_BRANCHES,
    MergePlan, MergePlanAdmissionError, MergePlanAdmissionLimits, MergePlanShapeError,
    MergePlanShapeLimits, RequestAdmissionError, RequestCorrespondenceError,
};

pub fn group_binding_admission_limits() -> GroupBindingLimits {
    GroupBindingLimits {
        plan: GroupPlanLimits {
            max_branches: MAX_MERGE_BRANCHES,
            max_effect_sequence: MAX_EFFECT_SEQUENCE,
            max_fuel: MAX_EXECUTION_FUEL,
        },
        binding: result_binding_limits(),
    }
}

pub fn merge_plan_admission_limits() -> MergePlanAdmissionLimits {
    MergePlanAdmissionLimits {
        shape: MergePlanShapeLimits {
            max_branches: MAX_MERGE_BRANCHES,
            max_name_bytes: MAX_MERGE_BRANCH_NAME_BYTES,
        },
        group: group_binding_admission_limits(),
    }
}

/// Outer shape, saved structure, correspondence, bounded encoding, then typing.
/// Selects the original binding from the plan; never executes its saved body.
pub fn validate_merge_plan(plan: &MergePlan) -> Result<(), Fault> {
    crate::validate_merge_plan(
        plan,
        merge_plan_admission_limits(),
        valid_merge_branch_name,
        validate_plan_encoding,
    )
    .map(|_| ())
    .map_err(merge_plan_admission_fault)
}

/// Lower-level binding stage for already shape-admitted plans. The caller must
/// select the original binding and bound all other recursive plan data before
/// entry; prefer `validate_merge_plan` for a whole unchecked plan.
pub fn validate_group_binding(group: &GroupResultBinding, plan: &MergePlan) -> Result<Type, Fault> {
    crate::validate_group_binding(
        group,
        plan,
        group_binding_admission_limits(),
        valid_merge_branch_name,
        validate_plan_encoding,
    )
    .map_err(group_binding_admission_fault)
}

/// Encoding callback only, not a standalone admission gate. Callers must bound
/// recursive structures first. Measures compact bytes without an output buffer.
pub fn validate_plan_encoding(plan: &MergePlan) -> Result<(), Fault> {
    bounded_json::measure(plan, MAX_CONTINUATION_BYTES)
        .map(|_| ())
        .map_err(|error| match error {
            BoundedJsonError::Limit { limit, .. } => Fault {
                code: "LSV3002".into(),
                message: format!("group-result plan exceeds {limit} bytes"),
            },
            BoundedJsonError::Serialize(error) => Fault {
                code: "LSV3001".into(),
                message: format!("failed to encode group-result plan: {error}"),
            },
        })
}

pub fn group_binding_admission_fault(error: GroupBindingError<Fault>) -> Fault {
    match error {
        GroupBindingError::Binding => {
            continuation_admission_fault(ContinuationAdmissionError::Binding)
        }
        GroupBindingError::Plan => invalid_group_binding(),
        GroupBindingError::Encoding(fault) => fault,
    }
}

pub fn merge_plan_admission_fault(error: MergePlanAdmissionError<Fault>) -> Fault {
    match error {
        MergePlanAdmissionError::Binding(error) => group_binding_admission_fault(error),
        MergePlanAdmissionError::Shape(error) => match error {
            MergePlanShapeError::GroupBinding => invalid_group_binding(),
            MergePlanShapeError::Successor => {
                request_admission_fault(RequestAdmissionError::Correspondence(
                    RequestCorrespondenceError::SuccessorAuthority,
                ))
            }
            MergePlanShapeError::Branches => Fault {
                code: "LSV2401".into(),
                message: "structured merge plan is invalid or exceeds runtime bounds".into(),
            },
        },
    }
}

pub fn invalid_group_binding() -> Fault {
    continuation_admission_fault(ContinuationAdmissionError::Schema(
        crate::continuation_schema::ContinuationSchemaError::Group,
    ))
}

fn valid_merge_branch_name(name: &str) -> bool {
    crate::valid_merge_branch_name(name, MAX_MERGE_BRANCH_NAME_BYTES)
}
