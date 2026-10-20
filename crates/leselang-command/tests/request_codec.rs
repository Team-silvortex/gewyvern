use leselang_command::{DecodedEffectRequest, EffectOperation, decode_effect_request};
use leselang_host_contract::{CapabilitySet, Principal, RuntimeListFilter};
use leselang_runtime_core::ResourceBudget;
use leserpent_domain::{DOMAIN_SCHEMA_VERSION, Query, QueryEnvelope};
use serde::Deserialize;
use serde_json::{Value, json};

fn request(legacy: bool, continuation: Value) -> Value {
    let query = QueryEnvelope {
        schema_version: DOMAIN_SCHEMA_VERSION,
        principal: Principal::new("declared-only").unwrap(),
        capabilities: CapabilitySet::new(["runtime.read"]),
        query: Query::RuntimeList {
            filter: RuntimeListFilter::default(),
        },
    };
    let mut wire = json!({
        "effect_id": "effect-7",
        "required_capability": "runtime.read",
        "continuation": continuation,
        "budget": ResourceBudget {
            fuel_remaining: 9, deadline_ms: 30_000, deadline_at_ms: Some(40_000),
            max_output_items: 100,
        },
    });
    if legacy {
        wire["query"] = serde_json::to_value(query).unwrap();
    } else {
        wire["operation"] = serde_json::to_value(EffectOperation::Query(query)).unwrap();
    }
    wire
}

#[test]
fn closed_decoder_moves_a_host_owned_continuation_without_clone_debug_or_serialize() {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Native {
        token: Box<str>,
        generation: u64,
    }
    for legacy in [false, true] {
        let wire = request(
            legacy,
            json!({"token": "native-controller", "generation": 23}),
        );
        let decoded: DecodedEffectRequest<Native> = decode_effect_request(wire).unwrap();
        assert_eq!(&*decoded.continuation.token, "native-controller");
        assert_eq!(decoded.continuation.generation, 23);
        assert_eq!(decoded.effect_id, "effect-7");
        assert_eq!(decoded.required_capability, "runtime.read");
        assert_eq!(decoded.budget.fuel_remaining, 9);
        assert!(matches!(decoded.operation, EffectOperation::Query(_)));
    }
}

#[test]
fn closed_decoder_can_borrow_the_host_continuation_from_json_input() {
    for legacy in [false, true] {
        let text = request(legacy, json!("borrowed-native-token")).to_string();
        let mut deserializer = serde_json::Deserializer::from_str(&text);
        let decoded: DecodedEffectRequest<&str> = decode_effect_request(&mut deserializer).unwrap();
        deserializer.end().unwrap();
        assert_eq!(decoded.continuation, "borrowed-native-token");
        let start = text.as_ptr() as usize;
        let address = decoded.continuation.as_ptr() as usize;
        assert!((start..start + text.len()).contains(&address));
    }
}

#[test]
fn stream_end_validation_remains_with_the_ingress_owner() {
    for legacy in [false, true] {
        let text = format!("{} null", request(legacy, json!(7)));
        let mut deserializer = serde_json::Deserializer::from_str(&text);
        let decoded: DecodedEffectRequest<u64> = decode_effect_request(&mut deserializer).unwrap();
        assert_eq!(decoded.continuation, 7);
        assert!(deserializer.end().is_err());
    }
}

#[test]
fn closed_decoder_refuses_unknown_missing_duplicate_and_mixed_fields() {
    for legacy in [false, true] {
        let wire = request(legacy, json!(7));
        let mut malformed = Vec::new();
        let mut unknown = wire.clone();
        unknown["unexpected"] = json!(0);
        malformed.push(unknown.to_string());
        for name in wire.as_object().unwrap().keys() {
            let mut missing = wire.clone();
            missing.as_object_mut().unwrap().remove(name);
            malformed.push(missing.to_string());
            let mut duplicate = wire.to_string();
            duplicate.pop();
            duplicate.push_str(&format!(",{name:?}:{}}}", wire[name]));
            malformed.push(duplicate);
        }
        let mut mixed = wire;
        mixed[if legacy { "operation" } else { "query" }] = json!(null);
        malformed.push(mixed.to_string());
        for text in malformed {
            let mut decoder = serde_json::Deserializer::from_str(&text);
            let Err(error) = decode_effect_request::<_, u64>(&mut decoder) else {
                panic!("closed envelope accepted {text}")
            };
            assert!(
                error.to_string().starts_with(
                    "data did not match any variant of untagged enum EffectRequestWire",
                )
            );
        }
    }
}

#[test]
fn decoded_product_data_is_not_runtime_authority_or_budget_admission() {
    for legacy in [false, true] {
        let mut wire = request(legacy, json!(false));
        wire["effect_id"] = json!("not-a-canonical-id");
        wire["required_capability"] = json!("not-the-declared-capability");
        wire["budget"]["fuel_remaining"] = json!(u64::MAX);
        wire["budget"]["deadline_ms"] = json!(0);
        let decoded: DecodedEffectRequest<bool> = decode_effect_request(wire).unwrap();
        assert!(!decoded.continuation);
        assert_eq!(decoded.effect_id, "not-a-canonical-id");
        assert_eq!(decoded.budget.fuel_remaining, u64::MAX);
        assert_eq!(decoded.budget.deadline_ms, 0);
    }
}

#[test]
fn decoding_preserves_absent_absolute_deadlines_without_inventing_policy() {
    for legacy in [false, true] {
        let mut wire = request(legacy, json!(7));
        wire["budget"]
            .as_object_mut()
            .unwrap()
            .remove("deadline_at_ms");
        let decoded: DecodedEffectRequest<u64> = decode_effect_request(wire).unwrap();
        assert_eq!(decoded.budget.deadline_at_ms, None);
        assert_eq!(decoded.budget.deadline_ms, 30_000);
    }
}
