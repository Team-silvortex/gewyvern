use leselang_command::{
    ContinuationImage, ContinuationToken, DispatchClaim, DispatchLease, EffectOperation,
    EffectRequest, MergeWait, NamedEffectRequest, PresentationEnvelope, PresentationOperation,
    RetryDisposition, Step, StructuredEffectBatch, Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leserpent_hir::{Effect, Type};
use serde_json::json;

fn request() -> EffectRequest {
    let continuation = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: None,
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(40_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    };
    EffectRequest {
        effect_id: "effect-7".into(),
        required_capability: "ui.presentation".into(),
        operation: EffectOperation::Presentation(PresentationEnvelope {
            schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
            principal: Principal::new("declared-owner").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            operation: PresentationOperation::Focus {
                node_id: "a".into(),
            },
        }),
        budget: continuation.resource_budget(),
        continuation,
    }
}

#[test]
fn concrete_variants_keep_owned_boxes_and_no_copy_protocol_handoffs() {
    use leselang_command::DispatchClaim::Leased;
    use leselang_command::RetryDisposition::Terminal;
    use leselang_command::Step::Effect;
    let request = Box::new(request());
    let address = &*request as *const EffectRequest;
    let text = request.effect_id.as_ptr();
    let observation = Terminal(Effect(request));
    let Terminal(Effect(request)) = observation else {
        panic!("wrong observation")
    };
    assert_eq!(&*request as *const EffectRequest, address);
    assert_eq!(request.effect_id.as_ptr(), text);
    let lease = Box::new(DispatchLease {
        request: *request,
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 20,
    });
    let address = &*lease as *const DispatchLease;
    let Leased(lease) = Leased(lease) else {
        panic!("wrong claim")
    };
    assert_eq!(&*lease as *const DispatchLease, address);
    assert_eq!(lease.request.effect_id.as_ptr(), text);
}

#[test]
fn data_decoding_keeps_permissive_observations_but_closed_claim_envelopes() {
    let wait = MergeWait {
        merge_token: ContinuationToken::from_wire("invalid/token".into()),
        completed_branches: 3,
        total_branches: 0,
    };
    let step = Step::Waiting(wait.clone());
    let mut wire = serde_json::to_value(&step).unwrap();
    wire["extra"] = json!(true);
    wire["payload"]["extra"] = json!(true);
    assert_eq!(serde_json::from_value::<Step>(wire).unwrap(), step);
    let retry = RetryDisposition::Terminal(Step::Waiting(wait));
    let mut wire = serde_json::to_value(&retry).unwrap();
    wire["extra"] = json!(true);
    assert_eq!(
        serde_json::from_value::<RetryDisposition>(wire).unwrap(),
        retry
    );
    assert!(serde_json::from_value::<DispatchClaim>(json!({"kind":"idle","extra":true})).is_err());
    let empty = StructuredEffectBatch {
        merge_token: ContinuationToken::from_wire(String::new()),
        branches: vec![],
    };
    assert_eq!(
        serde_json::from_value::<StructuredEffectBatch>(serde_json::to_value(&empty).unwrap())
            .unwrap(),
        empty
    );
    assert_eq!(
        serde_json::from_value::<Step>(
            json!({"kind":"done","payload":{"kind":"ui_focus","node_id":"a"}})
        )
        .ok(),
        Some(Step::Done(Value::UiFocus {
            node_id: "a".into()
        }))
    );
}

#[test]
fn nested_effects_still_decode_through_the_checked_request_not_raw_data() {
    let mut request = request();
    request.effect_id = "wrong".into();
    let batch = StructuredEffectBatch {
        merge_token: ContinuationToken::from_wire("merge-1".into()),
        branches: vec![NamedEffectRequest {
            branch: "a".into(),
            request: request.clone(),
        }],
    };
    for step in [
        Step::Effect(Box::new(request.clone())),
        Step::Effects(Box::new(batch)),
    ] {
        let wire = serde_json::to_vec(&step).unwrap();
        assert!(
            serde_json::from_slice::<Step>(&wire)
                .unwrap_err()
                .to_string()
                .contains("LSV2010")
        );
        let wire = serde_json::to_vec(&RetryDisposition::Terminal(step)).unwrap();
        assert!(
            serde_json::from_slice::<RetryDisposition>(&wire)
                .unwrap_err()
                .to_string()
                .contains("LSV2010")
        );
    }
    request.continuation.token = ContinuationToken::from_wire("invalid/token".into());
    let raw = Step::Yield(request.continuation);
    assert_eq!(
        serde_json::from_slice::<Step>(&serde_json::to_vec(&raw).unwrap()).unwrap(),
        raw
    );
}

#[test]
fn lease_alias_preserves_omitted_retry_defaults_without_validating_counters() {
    let lease = DispatchLease {
        request: request(),
        attempt: 0,
        retry_count: 0,
        lease_expires_at_ms: u64::MAX,
    };
    let mut wire = serde_json::to_value(&lease).unwrap();
    wire.as_object_mut().unwrap().remove("retry_count");
    wire["extra"] = json!(true);
    assert_eq!(
        serde_json::from_value::<DispatchLease>(wire).unwrap(),
        lease
    );
}
