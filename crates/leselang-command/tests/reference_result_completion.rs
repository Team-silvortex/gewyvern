use leselang_command::continuation_schema::*;
use leselang_command::reference_reply::CompletionLease;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_result_completion::{self, ReferenceCompletionHost};
use leselang_command::{
    ContinuationImage, DispatchLease, EffectOperation, EffectRequest, EffectResult,
    PresentationResult, ResultBinding, Step, Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{Fault, ScalarValue};
use leserpent_hir::{Effect, Type};
use std::{cell::RefCell, rc::Rc};

#[path = "support/group_start_fixture.rs"]
mod fixture;

fn request(schema: u32, kind: usize, fuel: u64) -> EffectRequest {
    let mut request = build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "target".into(),
        },
        result_type: Type::UiFocus,
        sequence: 7,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 30_000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap();
    request.continuation.schema_version = schema;
    request.continuation.result_binding = fixture::binding(kind);
    request.continuation.fuel_remaining = fuel;
    request
}

fn reply() -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: "target".into(),
    })
}

#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail_at: Option<&'static str>,
    failure: Option<Fault>,
    unwind_at: Option<&'static str>,
    converted: Option<Step>,
    successor: Option<(Effect, Option<Box<ResultBinding>>, u64)>,
    image_pointer: Option<*const ContinuationImage>,
    lease_pointer: Option<*const DispatchLease>,
    operation_pointer: Option<*const EffectOperation>,
}

impl Host {
    fn enter(&mut self, stage: &'static str, image: &ContinuationImage) -> Result<(), Fault> {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted hook unwind");
        if let Some(pointer) = self.image_pointer {
            assert_eq!(std::ptr::from_ref(image), pointer);
        }
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        Ok(())
    }

    fn convert(
        &mut self,
        stage: &'static str,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = self.enter(stage, image) {
            return Step::Fault(error);
        }
        assert_eq!(operation.map(std::ptr::from_ref), self.operation_pointer);
        if let Some(step) = self.converted.take() {
            return step;
        }
        let EffectResult::Presentation(PresentationResult::Focus { node_id }) = result else {
            panic!()
        };
        Step::Done(Value::UiFocus { node_id })
    }

    fn check_lease(&self, lease: CompletionLease<'_>) {
        assert_eq!(
            lease.map(|(l, _)| std::ptr::from_ref(l)),
            self.lease_pointer
        );
        if let Some((lease, now)) = lease {
            assert_eq!(
                (
                    lease.attempt,
                    lease.retry_count,
                    lease.lease_expires_at_ms,
                    now
                ),
                (3, 2, 100, 12)
            );
        }
    }
}

impl ReferenceCompletionHost for Host {
    fn atomic_step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        self.convert("atomic", image, operation, result)
    }
    fn step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        self.convert("bound", image, operation, result)
    }
    fn validate_result_output(
        &mut self,
        image: &ContinuationImage,
        _: &Value,
    ) -> Result<(), Fault> {
        self.enter("validate", image)
    }
    fn complete_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        value: Value,
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    ) -> Step {
        if let Err(error) = self.enter("successor", image) {
            return Step::Fault(error);
        }
        self.check_lease(lease);
        self.successor = Some((effect, binding, fuel));
        Step::Done(value)
    }
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        if let Err(error) = self.enter("record", image) {
            return Step::Fault(error);
        }
        self.check_lease(lease);
        step
    }
}

#[test]
fn schema_routes_keep_raw_group_values_and_original_reply_image_and_lease_owners() {
    for schema in 1..=11 {
        for leased in [false, true] {
            let lease = DispatchLease {
                request: request(schema, 3, 100),
                attempt: 3,
                retry_count: 2,
                lease_expires_at_ms: 100,
            };
            let image = &lease.request.continuation;
            let mut host = Host {
                image_pointer: Some(image),
                lease_pointer: leased.then_some(&lease),
                operation_pointer: leased.then_some(&lease.request.operation),
                ..Host::default()
            };
            let result = reply();
            let EffectResult::Presentation(PresentationResult::Focus { node_id }) = &result else {
                panic!()
            };
            let pointer = node_id.as_ptr();
            let step = reference_result_completion::complete_result(
                image,
                leased.then_some((&lease, 12)),
                result,
                &mut host as &mut dyn ReferenceCompletionHost,
            );
            let Step::Done(Value::UiFocus { node_id }) = step else {
                panic!("{step:?}")
            };
            assert_eq!(node_id.as_ptr(), pointer);
            let expected: &[&str] = match schema {
                3..=5 => &["atomic", "validate", "successor"],
                8..=11 => &["atomic", "record"],
                _ => &["bound", "record"],
            };
            assert_eq!(host.trace.borrow().as_slice(), expected);
            if let Some((effect, binding, fuel)) = host.successor {
                assert_eq!(
                    effect,
                    Effect::UiFocus {
                        node_id: "next".into()
                    }
                );
                assert!(binding.is_some());
                assert!(fuel < 100 && fuel > 0);
            }
        }
    }
}

#[test]
fn saved_fuel_and_scalar_host_and_bound_host_outcomes_match_the_shared_meter() {
    use leselang_command::{execution_outcome::ComputationOutcome, reference_computation};
    use leselang_runtime_core::Fuel;
    use leserpent_hir::host_call::HostOperation;
    for kind in 1..=5 {
        for fuel in (0..=12).chain([100]) {
            let request = request(SUCCESSOR_CONTINUATION_SCHEMA_VERSION, kind, fuel);
            let image = &request.continuation;
            let value = Value::UiFocus {
                node_id: "target".into(),
            };
            let mut meter = Fuel::new(fuel);
            let expected = reference_computation::resume_outcome(
                image.result_binding.as_ref().unwrap(),
                &value,
                HostOperation::for_effect(&image.pending_effect).unwrap(),
                &mut meter,
            );
            let mut host = Host::default();
            let got = reference_result_completion::complete_result(image, None, reply(), &mut host);
            match expected {
                Err(error) => assert_eq!(got, Step::Fault(error)),
                Ok(ComputationOutcome::Scalar(value)) => {
                    assert_eq!(got, Step::Done(Value::Scalar { value }))
                }
                Ok(ComputationOutcome::Host(_) | ComputationOutcome::BoundHost { .. })
                    if meter.remaining() == 0 =>
                {
                    assert!(
                        matches!(got, Step::Fault(ref e) if e.code == "LSV1001" && e.message == "execution fuel exhausted before successor")
                    );
                    assert!(host.successor.is_none());
                }
                Ok(ComputationOutcome::Host(effect)) => {
                    assert_eq!(got, Step::Done(value.clone()));
                    assert_eq!(
                        host.successor,
                        Some((effect.into_owned(), None, meter.remaining()))
                    );
                }
                Ok(ComputationOutcome::BoundHost { effect, binding }) => {
                    assert_eq!(got, Step::Done(value.clone()));
                    assert_eq!(
                        host.successor,
                        Some((effect.into_owned(), Some(binding), meter.remaining()))
                    );
                }
                Ok(ComputationOutcome::Result) => panic!(),
            }
            assert_eq!(
                host.trace
                    .borrow()
                    .iter()
                    .filter(|s| matches!(**s, "record" | "successor"))
                    .count(),
                1
            );
        }
    }
}

#[test]
fn raw_output_refusal_precedes_missing_binding_and_exhausted_fuel() {
    let request = request(SUCCESSOR_CONTINUATION_SCHEMA_VERSION, 0, 0);
    let error = Fault {
        code: "owner-rejected".into(),
        message: "original owner error".into(),
    };
    let pointer = error.message.as_ptr();
    let mut host = Host {
        fail_at: Some("validate"),
        failure: Some(error),
        ..Host::default()
    };
    let got = reference_result_completion::complete_result(
        &request.continuation,
        None,
        reply(),
        &mut host,
    );
    let Step::Fault(error) = got else { panic!() };
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(*host.trace.borrow(), ["atomic", "validate", "record"]);
    let mut host = Host::default();
    assert!(
        matches!(reference_result_completion::complete_result(&request.continuation, None, reply(), &mut host),
        Step::Fault(ref e) if e.code == "LSV1407")
    );
}

#[test]
fn converted_refusals_skip_evaluation_and_native_record_winner_is_returned() {
    for schema in [1, 3, 8] {
        let request = request(schema, 0, 0);
        let original = Fault {
            code: "raw-refused".into(),
            message: "raw reply refused".into(),
        };
        let mut host = Host {
            converted: Some(Step::Fault(original.clone())),
            ..Host::default()
        };
        assert_eq!(
            reference_result_completion::complete_result(
                &request.continuation,
                None,
                reply(),
                &mut host
            ),
            Step::Fault(original)
        );
        assert!(!host.trace.borrow().contains(&"validate"));
        let winner = Fault {
            code: "native-fence".into(),
            message: "native lease refused".into(),
        };
        host.fail_at = Some("record");
        host.failure = Some(winner.clone());
        host.converted = Some(Step::Done(Value::Scalar {
            value: ScalarValue::None,
        }));
        assert_eq!(
            reference_result_completion::complete_result(
                &request.continuation,
                None,
                reply(),
                &mut host
            ),
            Step::Fault(winner)
        );
    }
}

#[test]
fn trusted_hook_failures_and_unwinds_are_once_only_without_rollback() {
    for stage in ["atomic", "bound", "validate", "successor", "record"] {
        let schema = if stage == "bound" { 1 } else { 3 };
        let kind = if stage == "record" { 1 } else { 3 };
        let request = request(schema, kind, 100);
        for unwind in [false, true] {
            let error = Fault {
                code: "owner-error".into(),
                message: "retained owner fault".into(),
            };
            let pointer = error.message.as_ptr();
            let mut host = Host {
                fail_at: (!unwind).then_some(stage),
                failure: Some(error),
                unwind_at: unwind.then_some(stage),
                ..Host::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                reference_result_completion::complete_result(
                    &request.continuation,
                    None,
                    reply(),
                    &mut host,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                let Step::Fault(error) = result.unwrap() else {
                    panic!()
                };
                assert_eq!(error.message.as_ptr(), pointer);
            }
            let trace = host.trace.borrow();
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
            if unwind || matches!(stage, "successor" | "record") {
                assert_eq!(trace.last(), Some(&stage));
            } else {
                assert_eq!(trace.last(), Some(&"record"));
            }
        }
    }
}
