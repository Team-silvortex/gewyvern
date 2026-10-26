use leselang_command::reference_request::{
    continuation_admission_limits, validate_continuation_image, validate_effect_request,
};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectOperation, EffectRequest, PresentationEnvelope,
    PresentationOperation,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leserpent_hir::{Effect, Type};

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
fn concrete_request_checks_and_borrows_without_a_vm_or_conversion() {
    let request = request();
    assert_eq!(validate_effect_request(&request), Ok(()));
    let view = request.as_view();
    assert_eq!(view.effect_id.as_ptr(), request.effect_id.as_ptr());
    assert_eq!(
        view.required_capability.as_ptr(),
        request.required_capability.as_ptr()
    );
    assert!(std::ptr::eq(view.operation, &request.operation));
    assert!(std::ptr::eq(view.continuation, &request.continuation));
    assert!(std::ptr::eq(view.budget, &request.budget));
    let bytes = serde_json::to_vec(&request).unwrap();
    assert_eq!(
        serde_json::from_slice::<EffectRequest>(&bytes).unwrap(),
        request
    );
    assert_eq!(serde_json::to_vec(&request).unwrap(), bytes);
    let limits = continuation_admission_limits();
    assert_eq!(limits.budget.max_fuel, 1_000_000);
    assert_eq!(limits.budget.max_output_items, 10_000);
    assert_eq!(limits.budget.max_deadline_at_ms, i64::MAX as u64);
    assert_eq!(limits.binding.groups.max_members, 63);
}

#[test]
fn concrete_decode_preserves_ordered_faults_and_rechecks_public_mutations() {
    let mut changed = request();
    changed.continuation.schema_version = u32::MAX;
    changed.continuation.token = ContinuationToken::from_wire("bad/token".into());
    changed.continuation.program_counter = 2;
    changed.continuation.result_type = Type::Structured;
    changed.continuation.fuel_remaining = u64::MAX;
    changed.continuation.deadline_ms = 0;
    changed.continuation.deadline_at_ms = Some(u64::MAX);
    changed.continuation.max_output_items = 10_001;
    changed.effect_id = "wrong".into();
    changed.required_capability = "wrong".into();
    changed.budget.fuel_remaining = 0;
    for code in [
        "LSV2001", "LSV2006", "LSV2014", "LSV2016", "LSV2007", "LSV2008", "LSV2011", "LSV2009",
        "LSV2010", "LSV2011", "LSV2013",
    ] {
        let bytes = serde_json::to_vec(&changed).unwrap();
        let fault = validate_effect_request(&changed).unwrap_err();
        assert_eq!(fault.code, code);
        let decode = serde_json::from_slice::<EffectRequest>(&bytes)
            .unwrap_err()
            .to_string();
        assert!(
            decode.starts_with(&format!("{}: {}", fault.code, fault.message)),
            "{decode}"
        );
        assert_eq!(serde_json::to_vec(&changed).unwrap(), bytes);
        match code {
            "LSV2001" => changed.continuation.schema_version = 1,
            "LSV2006" => changed.continuation.token = request().continuation.token,
            "LSV2014" => changed.continuation.program_counter = 1,
            "LSV2016" => changed.continuation.result_type = Type::UiFocus,
            "LSV2007" => changed.continuation.fuel_remaining = 99,
            "LSV2008" => changed.continuation.deadline_ms = 30_000,
            "LSV2011" if changed.continuation.deadline_at_ms == Some(u64::MAX) => {
                changed.continuation.deadline_at_ms = Some(40_000);
            }
            "LSV2009" => changed.continuation.max_output_items = 10_000,
            "LSV2010" => changed.effect_id = "effect-7".into(),
            "LSV2011" => changed.required_capability = "ui.presentation".into(),
            "LSV2013" => changed.budget = changed.continuation.resource_budget(),
            _ => unreachable!(),
        }
    }
    assert_eq!(validate_effect_request(&changed), Ok(()));
    assert_eq!(changed, request());
    changed.continuation.max_output_items = 0;
    changed.budget.max_output_items = 0;
    assert_eq!(validate_effect_request(&changed), Ok(()));
}

#[test]
fn raw_frame_decode_and_lexical_consistency_do_not_replace_request_admission() {
    let mut request = request();
    request.continuation.token = ContinuationToken::from_wire("continuation-007".into());
    let bytes = serde_json::to_vec(&request.continuation).unwrap();
    let image: ContinuationImage = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(validate_continuation_image(&image), Ok(()));
    assert_eq!(
        validate_effect_request(&request).unwrap_err().code,
        "LSV2010"
    );
    assert!(
        serde_json::from_slice::<EffectRequest>(&serde_json::to_vec(&request).unwrap()).is_err()
    );
    request.continuation.token = ContinuationToken::from_wire("bad/token".into());
    let bytes = serde_json::to_vec(&request.continuation).unwrap();
    let image: ContinuationImage = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        validate_continuation_image(&image).unwrap_err().code,
        "LSV2006"
    );
}

#[test]
fn checked_request_decode_still_requires_external_ingress_bounds() {
    let request = request();
    let mut bytes = serde_json::to_vec(&request).unwrap();
    bytes.extend(std::iter::repeat_n(b' ', 64 * 1024));
    assert_eq!(
        serde_json::from_slice::<EffectRequest>(&bytes).unwrap(),
        request
    );
    assert!(matches!(
        leselang_command::bounded_json::decode::<EffectRequest>(&bytes, 64 * 1024),
        Err(leselang_command::bounded_json::BoundedJsonDecodeError::Limit { .. })
    ));
}
