//! Product root-start coordination over the original HIR and computation policies.

use crate::continuation_schema::preflighted_result_binding_schema_version;
use crate::execution_outcome::ComputationOutcome as Outcome;
use crate::reference_request::{result_binding_limits, validate_effect_request};
use crate::reference_request_build::{ReservedEffectInput, build_effect_request};
use crate::{EffectRequest, ExecutionOrder, ResultBinding, Step, Value};
use leselang_hir::{Effect, HirBranch, HirProgram, Type, authorize, canonical_source};
use leselang_runtime_core::{Fault, Fuel};

/// Original authority, deadline and remaining root budget; not a fresh grant.
pub use crate::reference_group_start::GroupStartContext as StartContext;
use crate::reference_group_start::ReferenceGroupStartHost;

/// Trusted native owner. Inherited hooks keep live capacity and reservations on
/// that owner. Group startup and atomic commit must recheck current admission and
/// commit before publishing pending state or visible work. No hook is retried;
/// earlier reservations and native side effects are not rolled back here.
pub trait ReferenceStartHost: ReferenceGroupStartHost {
    fn start_group(
        &mut self,
        branches: &[HirBranch],
        order: ExecutionOrder,
        context: StartContext,
        binding: Option<Box<ResultBinding>>,
    ) -> Step;
    fn commit_atomic(&mut self, request: EffectRequest) -> Step;
}

/// Coordinate one root using the released fuel, authorization and error order.
/// The HIR remains borrowed and captures remain original owned results. Callers
/// bound ingress, supply the real principal/capability snapshot and remaining
/// budget, and (for timed starts) validate trusted time and expire due work before
/// entry. Static authorization is not live permission or journal authority.
/// Pure results do not visit native hooks; host work is published only by them.
pub fn start<Host: ReferenceStartHost + ?Sized>(
    program: &HirProgram,
    context: StartContext,
    host: &mut Host,
) -> Step {
    let StartContext {
        principal,
        capabilities,
        expected_revision,
        deadline_ms,
        deadline_at_ms,
        fuel_remaining: fuel_limit,
    } = context;
    if fuel_limit == 0 {
        return fault("LSV1001", "execution fuel exhausted");
    }
    if let Err(error) = authorize(program, &capabilities) {
        return fault(&error.code, error.message);
    }
    let mut fuel = Fuel::new(fuel_limit);
    let (effect, result_binding) = if let Effect::Compute { expression } = &program.function.effect
    {
        if !principal.is_valid() {
            return fault("LSV1402", "invalid computation principal");
        }
        match crate::reference_computation::evaluate(expression, &mut fuel) {
            Ok(Outcome::Scalar(value)) => {
                return Step::Done(Value::Scalar { value });
            }
            Ok(Outcome::Host(effect)) => (effect, None),
            Ok(Outcome::BoundHost { effect, binding }) => (effect, Some(binding)),
            Ok(Outcome::Result) => {
                return fault("LSV1402", "unexpected unprojected result");
            }
            Err(error) => return Step::Fault(error),
        }
    } else {
        (std::borrow::Cow::Borrowed(&program.function.effect), None)
    };
    let fuel_remaining = fuel.remaining();
    let effect = effect.as_ref();
    let effect_result_type = if let Some(binding) = &result_binding {
        if let Err(error) = validate_binding(binding, effect) {
            return Step::Fault(error);
        }
        match leselang_hir::host_call::HostOperation::for_effect(effect) {
            Some(operation) => operation.result_type(),
            None if matches!(effect, Effect::All { .. } | Effect::Sequence { .. }) => {
                Type::Structured
            }
            None => return Step::Fault(invalid_binding()),
        }
    } else {
        program.function.result_type
    };
    if fuel_remaining == 0 {
        return fault("LSV1001", "execution fuel exhausted before host operation");
    }
    if let Effect::Sequence { .. } = effect
        && canonical_source(effect).is_err()
    {
        return fault("LSV1002", "invalid sequential control flow");
    }
    if let Effect::All { branches } | Effect::Sequence { steps: branches } = effect {
        return host.start_group(
            branches,
            if matches!(effect, Effect::Sequence { .. }) {
                ExecutionOrder::Sequential
            } else {
                ExecutionOrder::Parallel
            },
            StartContext {
                principal,
                capabilities,
                expected_revision,
                deadline_ms,
                deadline_at_ms,
                fuel_remaining,
            },
            result_binding,
        );
    }

    if let Err(error) = host.check_admission(1) {
        return Step::Fault(error);
    }
    let sequence = match host.allocate_sequence() {
        Ok(sequence) => sequence,
        Err(error) => return Step::Fault(error),
    };
    let mut request = match build_effect_request(ReservedEffectInput {
        effect,
        result_type: effect_result_type,
        sequence,
        principal,
        capabilities,
        expected_revision,
        deadline_ms,
        deadline_at_ms,
        fuel_remaining,
    }) {
        Ok(request) => request,
        Err(error) => return Step::Fault(error),
    };
    if let Some(binding) = result_binding {
        request.continuation.schema_version = preflighted_result_binding_schema_version(&binding);
        request.continuation.result_binding = Some(binding);
        if let Err(error) = validate_effect_request(&request) {
            return Step::Fault(error);
        }
    }
    host.commit_atomic(request)
}

fn validate_binding(binding: &ResultBinding, effect: &Effect) -> Result<Type, Fault> {
    crate::validate_result_binding(binding, effect, result_binding_limits(), |name| {
        crate::valid_merge_branch_name(name, crate::MAX_MERGE_BRANCH_NAME_BYTES)
    })
    .map_err(|_| invalid_binding())
}

fn invalid_binding() -> Fault {
    crate::reference_request::continuation_admission_fault(
        crate::ContinuationAdmissionError::Binding,
    )
}

fn fault(code: &str, message: impl Into<String>) -> Step {
    Step::Fault(Fault {
        code: code.into(),
        message: message.into(),
    })
}
