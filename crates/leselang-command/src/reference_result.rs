//! Released product reply conversion and bound-result observation, without a VM.
//!
//! Trusted composition after caller-owned admission: these functions neither validate
//! arbitrary continuation bindings nor settle journals, issue leases or publish state.
//! They reuse the shared value gates and evaluator; no alternate interpreter is added.

use std::convert::Infallible;

use crate::{ResultValueDomain, ResultValueError, ResultValueHost};
use leselang_runtime_core::{HostResultError, PendingReply, ReplyAcceptanceError, ReplyAuthority};

use crate::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
use crate::reference_request::continuation_admission_fault;
use crate::{
    ContinuationAdmissionError, ContinuationImage, EffectOperation, EffectResult, Step, Value,
};
use leselang_runtime_core::{Fault, Fuel};

struct BoundResultHost;

impl ResultValueHost for BoundResultHost {
    type Error = Fault;

    fn valid_field_name(&self, name: &str) -> bool {
        crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
    }

    fn validate_encoded_value(&self, reply: &Value) -> Result<(), Fault> {
        crate::bounded_json::measure(reply, MAX_JOURNAL_ENTRY_BYTES)
            .map(|_| ())
            .map_err(|error| match error {
                crate::bounded_json::BoundedJsonError::Limit { limit, .. } => Fault {
                    code: "LSV3002".into(),
                    message: format!("bound host result exceeds {limit} bytes"),
                },
                crate::bounded_json::BoundedJsonError::Serialize(error) => Fault {
                    code: "LSV3001".into(),
                    message: format!("failed to encode bound host result: {error}"),
                },
            })
    }
}

struct ResultObservation;

impl ReplyAuthority<&str> for ResultObservation {
    type Error = Infallible;

    fn authorize(&self, _: &&str) -> Result<(), Infallible> {
        // Durable identity/lease/revision checks precede raw result construction.
        // This observation-only checkpoint grants no new execution authority.
        Ok(())
    }
}

/// Observe a bounded reply without copying its frame or payload.
/// This does not authenticate tokens, leases, revisions or durable receipts.
pub fn accept_bound_value<'a>(
    image: &'a ContinuationImage,
    value: &'a Value,
) -> Result<(&'a ContinuationImage, &'a Value), Fault> {
    let declaration = ResultValueDomain {
        result_type: image.result_type,
        value_limits: crate::reference_value::value_validation_limits(),
        max_output_items: image.max_output_items,
        host: &BoundResultHost,
    };
    let mut pending = PendingReply::new(image.token.as_str(), image, &declaration);
    let accepted = pending
        .try_accept(&image.token.as_str(), value, &ResultObservation)
        .map_err(|rejected| match rejected.error {
            ReplyAcceptanceError::Result(HostResultError::TypeMismatch) => Fault {
                code: "LSV2103".into(),
                message: "effect result does not match pending effect".into(),
            },
            ReplyAcceptanceError::Result(HostResultError::InvalidValue(error)) => match error {
                ResultValueError::Bounds(error) => {
                    crate::reference_value::value_bounds_fault(error)
                }
                ResultValueError::OutputLimit { .. } => Fault {
                    code: "LSV2102".into(),
                    message: "bound result exceeds the output item limit".into(),
                },
                ResultValueError::Encoding(error) => error,
            },
            ReplyAcceptanceError::Authority(error) => match error {},
            ReplyAcceptanceError::Closed(_) | ReplyAcceptanceError::IdentityMismatch => {
                continuation_admission_fault(ContinuationAdmissionError::Binding)
            }
        })?;
    let (_, image, _, value) = accepted.into_parts();
    Ok((image, value))
}

/// Apply the same observation gates without transferring either owner.
pub fn validate_bound_value(image: &ContinuationImage, value: &Value) -> Result<(), Fault> {
    accept_bound_value(image, value).map(|_| ())
}

/// Convert an admitted reply and resume its scalar binding with the saved fuel.
/// Successor/group routing and durable settlement remain the caller's responsibility.
pub fn step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    let step = atomic_step_from_effect_result(image, operation, result);
    let Some(binding) = &image.result_binding else {
        return step;
    };
    let Step::Done(value) = step else {
        return step;
    };
    let (image, value) = match accept_bound_value(image, &value) {
        Ok(accepted) => accepted,
        Err(error) => return Step::Fault(error),
    };
    let mut fuel = Fuel::new(image.fuel_remaining);
    let Some(operation) = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
    else {
        return Step::Fault(continuation_admission_fault(
            ContinuationAdmissionError::Binding,
        ));
    };
    match crate::reference_computation::resume(binding, value, operation, &mut fuel) {
        Ok(value) => Step::Done(Value::Scalar { value }),
        Err(error) => Step::Fault(error),
    }
}

/// Convert an admitted owned reply, retaining the released diagnostic precedence.
/// Correspondence checking is not dispatch or completion authority.
pub fn atomic_step_from_effect_result(
    image: &ContinuationImage,
    operation: Option<&EffectOperation>,
    result: EffectResult,
) -> Step {
    match result.into_effect_value(crate::EffectValueContext {
        effect: &image.pending_effect,
        result_type: image.result_type,
        operation,
        expected_revision: image.expected_revision,
        max_output_items: image.max_output_items,
    }) {
        Ok(value) => Step::Done(value),
        Err(crate::EffectValueError::Mismatch) => {
            fault("LSV2103", "effect result does not match pending effect")
        }
        Err(crate::EffectValueError::RevisionConflict { expected, actual }) => fault(
            "LSV2101",
            format!(
                "effect revision conflict: expected {}, actual {}",
                expected.0, actual.0
            ),
        ),
        Err(crate::EffectValueError::OutputLimit { count, limit }) => fault(
            "LSV2102",
            format!("effect returned {count} items, limit is {limit}"),
        ),
    }
}

fn fault(code: &str, message: impl Into<String>) -> Step {
    Step::Fault(Fault {
        code: code.into(),
        message: message.into(),
    })
}
