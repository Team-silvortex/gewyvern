use super::*;

use serde_json::{Value as Json, json};

// Frozen released decoder: compare closed shapes, normalization and exact errors
// independently of the command adapter now used by EffectRequest.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CurrentEffectRequestWire {
    effect_id: String,
    required_capability: String,
    operation: EffectOperation,
    continuation: ContinuationImage,
    budget: ResourceBudget,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyQueryEffectRequestWire {
    effect_id: String,
    required_capability: String,
    query: QueryEnvelope,
    continuation: ContinuationImage,
    budget: ResourceBudget,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum EffectRequestWire {
    Current(CurrentEffectRequestWire),
    LegacyQuery(LegacyQueryEffectRequestWire),
}
struct Frozen(EffectRequest);
impl<'de> Deserialize<'de> for Frozen {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let request = match EffectRequestWire::deserialize(deserializer)? {
            EffectRequestWire::Current(wire) => EffectRequest {
                effect_id: wire.effect_id,
                required_capability: wire.required_capability,
                operation: wire.operation,
                continuation: wire.continuation,
                budget: wire.budget,
            },
            EffectRequestWire::LegacyQuery(wire) => EffectRequest {
                effect_id: wire.effect_id,
                required_capability: wire.required_capability,
                operation: EffectOperation::Query(wire.query),
                continuation: wire.continuation,
                budget: wire.budget,
            },
        };
        operation_request_tests::frozen_validate_effect_request(&request)
            .map_err(|fault| D::Error::custom(format!("{}: {}", fault.code, fault.message)))?;
        Ok(Self(request))
    }
}

fn variants(request: &EffectRequest) -> Vec<Json> {
    let current = serde_json::to_value(request).unwrap();
    let mut variants = vec![current.clone()];
    if let EffectOperation::Query(query) = &request.operation {
        let mut legacy = current;
        legacy.as_object_mut().unwrap().remove("operation");
        legacy["query"] = serde_json::to_value(query).unwrap();
        variants.push(legacy);
    }
    variants
}

fn compare(input: &str) -> Result<Vec<u8>, String> {
    let expected = serde_json::from_str::<Frozen>(input)
        .map(|request| serde_json::to_vec(&request.0).unwrap())
        .map_err(|error| error.to_string());
    let actual = serde_json::from_str::<EffectRequest>(input)
        .map(|request| serde_json::to_vec(&request).unwrap())
        .map_err(|error| error.to_string());
    assert_eq!(actual, expected, "{input}");
    actual
}

#[test]
fn request_codec_migration_keeps_all_operation_and_legacy_query_bytes() {
    let mut count = 0;
    for request in continuation_contract_tests::requests() {
        let canonical = serde_json::to_vec(&request).unwrap();
        for wire in variants(&request) {
            assert_eq!(compare(&wire.to_string()).unwrap(), canonical);
            count += 1;
        }
    }
    assert_eq!(count, 74);
}

#[test]
fn request_codec_migration_keeps_closed_shapes_duplicates_and_exact_errors() {
    let mut count = 0;
    for request in continuation_contract_tests::requests() {
        for wire in variants(&request) {
            let mut invalid = wire.clone();
            invalid["unknown"] = json!(true);
            assert!(compare(&invalid.to_string()).is_err());
            count += 1;
            for name in wire.as_object().unwrap().keys() {
                let mut missing = wire.clone();
                missing.as_object_mut().unwrap().remove(name);
                assert!(compare(&missing.to_string()).is_err());
                let mut duplicate = wire.to_string();
                duplicate.pop();
                duplicate.push_str(&format!(",{name:?}:{}}}", wire[name]));
                assert!(compare(&duplicate).is_err());
                count += 2;
            }
            let mut mixed = wire.clone();
            mixed[if wire.get("query").is_some() {
                "operation"
            } else {
                "query"
            }] = json!(null);
            assert!(compare(&mixed.to_string()).is_err());
            count += 1;
            for path in ["continuation", "budget"] {
                let mut unknown = wire.clone();
                unknown[path]["unknown"] = json!(true);
                assert!(compare(&unknown.to_string()).is_err());
                let mut wrong_type = wire.clone();
                wrong_type[path] = json!("not-an-object");
                assert!(compare(&wrong_type.to_string()).is_err());
                count += 2;
            }
            let text = wire.to_string();
            assert!(compare(&format!("{text} null")).is_err());
            assert!(compare(&text[..text.len() - 1]).is_err());
            count += 2;
        }
    }
    assert_eq!(count, 74 * 18);
}

#[test]
fn request_codec_migration_still_runs_full_admission_after_shape_decode() {
    for request in continuation_contract_tests::requests() {
        for wire in variants(&request) {
            for fault in 0..8 {
                let mut invalid = wire.clone();
                match fault {
                    0 => invalid["effect_id"] = json!("wrong-effect"),
                    1 => invalid["required_capability"] = json!("wrong-capability"),
                    2 => invalid["continuation"]["program_counter"] = json!(2),
                    3 => invalid["continuation"]["token"] = json!("invalid-token"),
                    4 => invalid["budget"]["fuel_remaining"] = json!(u64::MAX),
                    5 => invalid["continuation"]["schema_version"] = json!(u32::MAX),
                    6 => invalid["budget"]["deadline_ms"] = json!(0),
                    _ => invalid["budget"]["max_output_items"] = json!(0),
                }
                let error = compare(&invalid.to_string()).unwrap_err();
                assert!(error.starts_with("LSV"), "{error}");
            }
        }
    }
}
