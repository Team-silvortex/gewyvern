use leselang_command::continuation_schema::preflighted_result_binding_schema_version;
use leselang_command::reference_reply::CompletionLease;
use leselang_command::reference_request_build::build_effect_request;
use leselang_command::reference_successor::{self, ReferenceSuccessorHost, ReservedEffectInput};
use leselang_command::{
    ContinuationImage, ContinuationToken, DispatchLease, EffectRequest, ResultBinding, Step, Value,
};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, ScalarValue};
use leserpent_hir::{Effect, Type, computation::Computation};
use std::{cell::RefCell, rc::Rc};

#[path = "support/group_start_fixture.rs"]
mod fixture;

fn focus() -> Effect {
    Effect::UiFocus {
        node_id: "next".into(),
    }
}
fn raw() -> Value {
    Value::UiFocus {
        node_id: "parent".into(),
    }
}
fn parent() -> EffectRequest {
    let mut request = build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "parent".into(),
        },
        result_type: Type::UiFocus,
        sequence: 7,
        principal: Principal::new("original-operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation", "runtime.read"]),
        expected_revision: Some(Revision(42)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        fuel_remaining: 100,
    })
    .unwrap();
    request.continuation.max_output_items = 17;
    request.budget = request.continuation.resource_budget();
    request
}

struct Host {
    parent: Option<EffectRequest>,
    sequence: u64,
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail_at: Option<&'static str>,
    failure: Option<Fault>,
    unwind_at: Option<&'static str>,
    corrupt_builder: bool,
    binding_pointer: Option<*const ResultBinding>,
    raw_pointer: Option<*const u8>,
    image_pointer: Option<*const ContinuationImage>,
    lease_pointer: Option<*const DispatchLease>,
    next: Option<EffectRequest>,
    recorded: Option<Step>,
}
impl Host {
    fn new(parent: EffectRequest) -> Self {
        Self {
            parent: Some(parent),
            sequence: 20,
            trace: Rc::default(),
            fail_at: None,
            failure: None,
            unwind_at: None,
            corrupt_builder: false,
            binding_pointer: None,
            raw_pointer: None,
            image_pointer: None,
            lease_pointer: None,
            next: None,
            recorded: None,
        }
    }
    fn enter(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted owner unwind");
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        Ok(())
    }
    fn context(&self, image: &ContinuationImage, lease: CompletionLease<'_>) {
        if let Some(pointer) = self.image_pointer {
            assert_eq!(std::ptr::from_ref(image), pointer);
        }
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
impl ReferenceSuccessorHost for Host {
    fn effect_request(&mut self, token: &ContinuationToken) -> Result<EffectRequest, Fault> {
        self.enter("parent")?;
        assert_eq!(token.as_str(), "continuation-7");
        Ok(self.parent.take().unwrap())
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.enter("reserve")?;
        let current = self.sequence;
        self.sequence += 1;
        Ok(current)
    }
    fn build_effect_request(
        &mut self,
        input: ReservedEffectInput<'_>,
    ) -> Result<EffectRequest, Fault> {
        self.enter("build")?;
        assert_eq!(input.sequence, 20);
        assert_eq!(
            input.principal,
            Principal::new("original-operator").unwrap()
        );
        assert_eq!(
            input.capabilities,
            CapabilitySet::new(["ui.presentation", "runtime.read"])
        );
        assert_eq!(
            (
                input.expected_revision,
                input.deadline_ms,
                input.deadline_at_ms
            ),
            (Some(Revision(42)), 30_000, Some(31_000))
        );
        let mut next = build_effect_request(input)?;
        if self.corrupt_builder {
            next.budget.fuel_remaining += 1;
        }
        Ok(next)
    }
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        if let Err(error) = self.enter("record") {
            return Step::Fault(error);
        }
        self.context(image, lease);
        self.recorded = Some(step.clone());
        step
    }
    fn commit_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
    ) -> Step {
        if let Err(error) = self.enter("commit") {
            return Step::Fault(error);
        }
        self.context(image, lease);
        assert_eq!(&parent.continuation, image);
        if let Some(pointer) = self.binding_pointer {
            assert_eq!(
                std::ptr::from_ref(next.continuation.result_binding.as_deref().unwrap()),
                pointer
            );
        }
        if let Some(pointer) = self.raw_pointer {
            let Step::Done(Value::UiFocus { node_id }) = step else {
                panic!()
            };
            assert_eq!(node_id.as_ptr(), pointer);
        }
        self.next = Some(next.clone());
        Step::Done(Value::Scalar {
            value: ScalarValue::Integer(42),
        })
    }
}

#[test]
fn successor_keeps_authority_budget_schema_payload_owners_and_native_winner() {
    for kind in 0..=5 {
        for leased in [false, true] {
            let parent = parent();
            let image = parent.continuation.clone();
            let lease = DispatchLease {
                request: parent.clone(),
                attempt: 3,
                retry_count: 2,
                lease_expires_at_ms: 100,
            };
            let binding = fixture::binding(kind);
            let schema = binding
                .as_ref()
                .map(|b| preflighted_result_binding_schema_version(b))
                .unwrap_or(1);
            let value = raw();
            let Value::UiFocus { node_id } = &value else {
                panic!()
            };
            let mut host = Host::new(parent);
            host.raw_pointer = Some(node_id.as_ptr());
            host.binding_pointer = binding.as_deref().map(std::ptr::from_ref);
            host.image_pointer = Some(&image);
            host.lease_pointer = leased.then_some(&lease);
            let result = reference_successor::complete_successor(
                &image,
                leased.then_some((&lease, 12)),
                value,
                focus(),
                binding,
                55,
                &mut host as &mut dyn ReferenceSuccessorHost,
            );
            assert_eq!(
                result,
                Step::Done(Value::Scalar {
                    value: ScalarValue::Integer(42)
                })
            );
            assert_eq!(
                *host.trace.borrow(),
                ["parent", "reserve", "build", "commit"]
            );
            assert_eq!(host.sequence, 21);
            let next = host.next.unwrap();
            assert_eq!(next.continuation.schema_version, schema);
            assert_eq!(next.continuation.max_output_items, 17);
            assert_eq!(next.budget, next.continuation.resource_budget());
            assert_eq!(next.budget.fuel_remaining, 54);
            assert!(host.recorded.is_none());
        }
    }
}

#[test]
fn mismatched_parent_refuses_before_reservation_even_when_later_inputs_are_invalid() {
    let parent = parent();
    let mut image = parent.continuation.clone();
    image.fuel_remaining -= 1;
    let mut host = Host::new(parent);
    let result = reference_successor::complete_successor(
        &image,
        None,
        raw(),
        Effect::Sequence { steps: vec![] },
        None,
        0,
        &mut host,
    );
    assert!(matches!(result, Step::Fault(ref e) if e.code == "LSV1407"));
    assert_eq!(*host.trace.borrow(), ["parent"]);
    assert_eq!(host.sequence, 20);
    assert!(host.recorded.is_none());
}

#[test]
fn post_reservation_refusals_settle_once_without_refunding_the_sequence() {
    for defect in 0..5 {
        let parent = parent();
        let image = parent.continuation.clone();
        let mut host = Host::new(parent);
        let mut effect = focus();
        let mut binding = None;
        let mut fuel = 55;
        match defect {
            0 => effect = Effect::All { branches: vec![] },
            1 => fuel = 0,
            2 => {
                binding = fixture::binding(1);
                binding.as_mut().unwrap().name = "invalid name".into();
            }
            3 => {
                binding = fixture::binding(1);
                binding.as_mut().unwrap().body = Computation::Literal {
                    value: ScalarValue::String("x".repeat(70_000)),
                };
            }
            _ => host.corrupt_builder = true,
        }
        let result = reference_successor::complete_successor(
            &image,
            None,
            raw(),
            effect,
            binding,
            fuel,
            &mut host,
        );
        assert!(matches!(result, Step::Fault(_)), "{result:?}");
        assert_eq!(host.recorded, Some(result));
        assert!(host.next.is_none());
        assert_eq!(host.sequence, 21);
        let trace = host.trace.borrow();
        assert_eq!(trace.iter().filter(|s| **s == "record").count(), 1);
        assert_eq!(trace.last(), Some(&"record"));
        assert_eq!(trace.contains(&"build"), defect != 0);
    }
}

#[test]
fn caller_lease_never_replaces_saved_parent_authority() {
    let parent = parent();
    let image = parent.continuation.clone();
    let mut lease = DispatchLease {
        request: parent.clone(),
        attempt: 3,
        retry_count: 2,
        lease_expires_at_ms: 100,
    };
    let leselang_command::EffectOperation::Presentation(envelope) = &mut lease.request.operation
    else {
        panic!()
    };
    envelope.principal = Principal::new("substitute").unwrap();
    envelope.capabilities = CapabilitySet::new(["forged-capability"]);
    let before = serde_json::to_vec(&lease).unwrap();
    let mut host = Host::new(parent);
    host.lease_pointer = Some(&lease);
    reference_successor::complete_successor(
        &image,
        Some((&lease, 12)),
        raw(),
        focus(),
        None,
        55,
        &mut host,
    );
    assert_eq!(serde_json::to_vec(&lease).unwrap(), before);
    assert!(host.next.is_some());
    // This observation-only host does not authenticate leases; real journal tests
    // below the VM entry prove that the native commit fence refuses substitutes.
}

#[test]
fn native_faults_and_unwinds_preserve_once_only_hooks_and_prior_reservations() {
    for stage in ["parent", "reserve", "build", "record", "commit"] {
        for unwind in [false, true] {
            let parent = parent();
            let image = parent.continuation.clone();
            let mut host = Host::new(parent);
            let error = Fault {
                code: "owner-error".into(),
                message: "original owner refusal".into(),
            };
            let pointer = error.message.as_ptr();
            host.failure = Some(error);
            host.fail_at = (!unwind).then_some(stage);
            host.unwind_at = unwind.then_some(stage);
            let effect = if stage == "record" {
                Effect::Sequence { steps: vec![] }
            } else {
                focus()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                reference_successor::complete_successor(
                    &image,
                    None,
                    raw(),
                    effect,
                    None,
                    55,
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
            assert_eq!(
                host.sequence,
                if matches!(stage, "parent" | "reserve") {
                    20
                } else {
                    21
                }
            );
            let trace = host.trace.borrow();
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
            if !unwind && stage == "build" {
                assert_eq!(trace.last(), Some(&"record"));
            } else {
                assert_eq!(trace.last(), Some(&stage));
            }
        }
    }
}
