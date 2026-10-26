use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_result::{
    accept_bound_value, atomic_step_from_effect_result, step_from_effect_result,
    validate_bound_value,
};
use leselang_command::{
    EffectRequest, EffectResult, PresentationResult, Step, StructuredField, Value,
};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, Fuel, ScalarValue};
use leserpent_hir::{Effect, Type, computation::Computation, host_call::HostOperation};

#[path = "support/group_start_fixture.rs"]
mod fixture;

fn request() -> EffectRequest {
    build_effect_request(ReservedEffectInput {
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
    .unwrap()
}

fn reply() -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: "target".into(),
    })
}

fn code(step: Step) -> String {
    let Step::Fault(error) = step else {
        panic!("expected refusal: {step:?}")
    };
    error.code
}

#[test]
fn unbound_conversion_moves_the_original_payload_without_fuel_or_admission() {
    let mut request = request();
    request.continuation.fuel_remaining = 0;
    request.continuation.max_output_items = 0;
    // Conversion is not a lease/token gate. The actual owner must authenticate first.
    request.continuation.token =
        leselang_command::ContinuationToken::from_wire("not-an-admitted-token".into());
    let before = serde_json::to_vec(&request).unwrap();
    let node_id = String::from("target");
    let pointer = node_id.as_ptr();
    let Step::Done(Value::UiFocus { node_id }) = step_from_effect_result(
        &request.continuation,
        Some(&request.operation),
        EffectResult::Presentation(PresentationResult::Focus { node_id }),
    ) else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), pointer);
    assert_eq!(serde_json::to_vec(&request).unwrap(), before);
}

#[test]
fn bound_resume_uses_only_saved_fuel_and_the_existing_scalar_evaluator() {
    let mut request = request();
    request.continuation.result_binding = fixture::binding(1);
    request.continuation.result_binding.as_mut().unwrap().body = Computation::Bind {
        name: "x".into(),
        value: Box::new(Computation::Literal {
            value: ScalarValue::String("saved".into()),
        }),
        body: Box::new(Computation::Local { name: "x".into() }),
    };
    let mut accepted = 0;
    let mut exhausted = 0;
    for saved in 0..20 {
        request.continuation.fuel_remaining = saved;
        let before = serde_json::to_vec(&request).unwrap();
        let mut meter = Fuel::new(saved);
        let expected = match leselang_command::reference_computation::resume(
            request.continuation.result_binding.as_ref().unwrap(),
            &Value::UiFocus {
                node_id: "target".into(),
            },
            HostOperation::UiFocus,
            &mut meter,
        ) {
            Ok(value) => {
                accepted += 1;
                Step::Done(Value::Scalar { value })
            }
            Err(error) => {
                exhausted += 1;
                Step::Fault(error)
            }
        };
        assert_eq!(
            step_from_effect_result(&request.continuation, Some(&request.operation), reply()),
            expected
        );
        assert_eq!(serde_json::to_vec(&request).unwrap(), before);
    }
    assert!(accepted > 0 && exhausted > 0);
    request.continuation.result_binding = fixture::binding(2);
    assert!(matches!(
        step_from_effect_result(&request.continuation, None, reply()),
        Step::Fault(_)
    ));
}

#[test]
fn conversion_and_bound_gates_precede_exhausted_fuel_without_changing_diagnostics() {
    let mut request = request();
    request.continuation.result_binding = fixture::binding(1);
    request.continuation.fuel_remaining = 0;
    request.continuation.max_output_items = 0;
    let wrong = EffectResult::Presentation(PresentationResult::Focus {
        node_id: "wrong".into(),
    });
    assert_eq!(
        code(step_from_effect_result(&request.continuation, None, wrong)),
        "LSV2103"
    );
    assert_eq!(
        code(step_from_effect_result(
            &request.continuation,
            None,
            reply()
        )),
        "LSV2102"
    );
    request.continuation.max_output_items = 1;
    assert_eq!(
        code(step_from_effect_result(
            &request.continuation,
            None,
            reply()
        )),
        "LSV1001"
    );
}

#[test]
fn query_type_revision_and_zero_item_policies_keep_exact_faults() {
    let mut image = request().continuation;
    image.pending_effect = Effect::RuntimeList {
        filter: Default::default(),
    };
    image.result_type = Type::RuntimeList;
    image.expected_revision = Some(Revision(7));
    image.max_output_items = 0;
    let reply = || {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(8),
            runtimes: vec![],
        })
    };
    assert_eq!(
        atomic_step_from_effect_result(&image, None, reply()),
        Step::Fault(Fault {
            code: "LSV2101".into(),
            message: "effect revision conflict: expected 7, actual 8".into(),
        })
    );
    image.result_type = Type::UiFocus;
    assert_eq!(
        code(atomic_step_from_effect_result(&image, None, reply())),
        "LSV2103"
    );
    image.result_type = Type::RuntimeList;
    image.expected_revision = Some(Revision(8));
    assert!(matches!(
        atomic_step_from_effect_result(&image, None, reply()),
        Step::Done(_)
    ));
    image.result_binding = fixture::binding(1);
    assert_eq!(
        step_from_effect_result(&image, None, reply()),
        Step::Fault(Fault {
            code: "LSV2102".into(),
            message: "bound result exceeds the output item limit".into(),
        })
    );
}

#[test]
fn bound_observation_keeps_original_owners_and_type_tree_item_precedence() {
    let mut image = request().continuation;
    let reply = Value::UiFocus {
        node_id: "target".into(),
    };
    let (frame, value) = accept_bound_value(&image, &reply).unwrap();
    assert!(std::ptr::eq(frame, &image));
    assert!(std::ptr::eq(value, &reply));
    let invalid = Value::UiFocus {
        node_id: "bad id".into(),
    };
    image.max_output_items = 0;
    image.result_type = Type::RuntimeList;
    assert_eq!(
        validate_bound_value(&image, &invalid).unwrap_err().code,
        "LSV2103"
    );
    image.result_type = Type::UiFocus;
    assert_eq!(
        validate_bound_value(&image, &invalid).unwrap_err().code,
        "LSV2403"
    );
    assert_eq!(
        validate_bound_value(&image, &reply).unwrap_err().code,
        "LSV2102"
    );
}

#[test]
fn encoded_size_is_checked_after_shape_and_items_without_copying_or_changing_input() {
    fn structure(values: impl Iterator<Item = Value>) -> Value {
        Value::Structured {
            fields: values
                .enumerate()
                .map(|(i, value)| StructuredField {
                    name: format!("field{i}"),
                    value: Box::new(value),
                })
                .collect(),
        }
    }
    let mut image = request().continuation;
    image.result_type = Type::Structured;
    let value = structure((0..64).map(|_| {
        structure((0..34).map(|_| Value::Scalar {
            value: ScalarValue::String("x".repeat(4096)),
        }))
    }));
    let before = serde_json::to_vec(&value).unwrap();
    assert_eq!(
        validate_bound_value(&image, &value),
        Err(Fault {
            code: "LSV3002".into(),
            message: "bound host result exceeds 8388608 bytes".into(),
        })
    );
    image.max_output_items = 0;
    assert_eq!(
        validate_bound_value(&image, &value).unwrap_err().code,
        "LSV2102"
    );
    assert_eq!(serde_json::to_vec(&value).unwrap(), before);
}
