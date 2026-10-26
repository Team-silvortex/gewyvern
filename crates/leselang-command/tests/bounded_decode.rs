use leselang_command::bounded_json::{BoundedJsonDecodeError, decode};
use leselang_command::{
    COMMAND_PLAN_SCHEMA_VERSION, CommandPlan, MAX_COMMAND_PLAN_BYTES, PlanCodecError, decode_plan,
};
use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};
use std::{cell::Cell, error::Error};

#[test]
fn input_limit_precedes_json_parsing_and_retains_parser_diagnostics() {
    let cases = [
        b"null".to_vec(),
        b" \n[true,1,\"text\"]\t".to_vec(),
        b"{".to_vec(),
        b"null false".to_vec(),
        b"\"unterminated".to_vec(),
        b"\"\xff\"".to_vec(),
        b"\"\\uD800\"".to_vec(),
        b"1e9999".to_vec(),
        b"NaN".to_vec(),
        b"{\"a\":1,\"a\":2}".to_vec(),
        b"\n{\"a\":\nfalse,}".to_vec(),
        format!("{}0{}", "[".repeat(256), "]".repeat(256)).into_bytes(),
    ];
    let mut comparisons = 0;
    for bytes in cases {
        for limit in [0, bytes.len() - 1, bytes.len(), bytes.len() + 1, usize::MAX] {
            match decode::<Value>(&bytes, limit) {
                Err(BoundedJsonDecodeError::Limit { size, limit: bound }) => {
                    assert_eq!((size, bound), (bytes.len(), limit));
                    assert!(size > limit);
                }
                Err(BoundedJsonDecodeError::Deserialize(error)) => {
                    assert!(bytes.len() <= limit);
                    let expected = serde_json::from_slice::<Value>(&bytes).unwrap_err();
                    assert_eq!(error.to_string(), expected.to_string());
                    assert_eq!(error.classify(), expected.classify());
                    assert_eq!(
                        (error.line(), error.column()),
                        (expected.line(), expected.column())
                    );
                }
                Ok(value) => {
                    assert!(bytes.len() <= limit);
                    assert_eq!(value, serde_json::from_slice::<Value>(&bytes).unwrap());
                }
            }
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 60);
    let nested = format!("{}0{}", "[".repeat(256), "]".repeat(256));
    let error = decode::<Value>(nested.as_bytes(), usize::MAX).unwrap_err();
    assert!(error.to_string().contains("recursion limit exceeded"));
}

thread_local! {
    static CALLS: Cell<usize> = const { Cell::new(0) };
}

#[derive(Debug)]
struct Counted;

impl<'de> Deserialize<'de> for Counted {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        CALLS.with(|calls| calls.set(calls.get() + 1));
        Value::deserialize(deserializer)?;
        Ok(Self)
    }
}

#[test]
fn oversized_input_never_enters_custom_deserializers() {
    CALLS.with(|calls| calls.set(0));
    for bytes in [b"null".as_slice(), b"not json", b"null false"] {
        assert!(matches!(
            decode::<Counted>(bytes, bytes.len() - 1),
            Err(BoundedJsonDecodeError::Limit { .. })
        ));
    }
    assert_eq!(CALLS.with(Cell::get), 0);
    assert!(decode::<Counted>(b"null", 4).is_ok());
    assert_eq!(CALLS.with(Cell::get), 1);
    assert!(matches!(
        decode::<Counted>(b"null false", 10),
        Err(BoundedJsonDecodeError::Deserialize(_))
    ));
    assert_eq!(CALLS.with(Cell::get), 2);
    assert!(decode::<Counted>(b"{", 1).is_err());
    // A deserializer is trusted code, not a transaction rolled back by this codec.
    assert_eq!(CALLS.with(Cell::get), 3);
}

#[test]
fn borrowed_values_remain_borrowed_and_dto_shape_policy_is_not_replaced() {
    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct Borrowed<'a> {
        text: &'a str,
    }
    let bytes = br#"{"text":"borrowed"}"#;
    let value: Borrowed<'_> = decode(bytes, bytes.len()).unwrap();
    assert_eq!(value.text, "borrowed");
    assert_eq!(value.text.as_ptr(), bytes[9..].as_ptr());
    for invalid in [
        br#"{"text":"a","text":"b"}"#.as_slice(),
        br#"{"text":"a","extra":1}"#,
        br#"{}"#,
        br#"{"text":false}"#,
        br#"{"text":"a"} null"#,
    ] {
        let error = decode::<Borrowed<'_>>(invalid, usize::MAX).unwrap_err();
        let expected = serde_json::from_slice::<Borrowed<'_>>(invalid).unwrap_err();
        assert_eq!(error.to_string(), expected.to_string());
        assert_eq!(error.source().unwrap().to_string(), expected.to_string());
    }
    // The codec is not a blanket deny-unknown-fields layer.
    assert!(decode::<Value>(br#"{"extra":1}"#, 11).is_ok());
}

#[test]
fn limits_count_utf8_and_whitespace_bytes_not_characters_or_decoded_length() {
    let bytes = "\"\u{7fbd}\u{86c7}\" \n".as_bytes();
    assert_eq!(bytes.len(), 10);
    assert!(matches!(
        decode::<String>(bytes, 9),
        Err(BoundedJsonDecodeError::Limit { size: 10, limit: 9 })
    ));
    assert_eq!(decode::<String>(bytes, 10).unwrap(), "\u{7fbd}\u{86c7}");
    assert!(matches!(
        decode::<Value>(b"", 0),
        Err(BoundedJsonDecodeError::Deserialize(_))
    ));
    let error = decode::<Value>(b" ", 0).unwrap_err();
    assert_eq!(error.to_string(), "input JSON exceeds 0 bytes");
    assert!(error.source().is_none());
}

// Frozen released decoder, including schema-before-semantic validation.
fn frozen_decode_plan(bytes: &[u8]) -> Result<CommandPlan, PlanCodecError> {
    if bytes.len() > MAX_COMMAND_PLAN_BYTES {
        return Err(PlanCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_COMMAND_PLAN_BYTES,
        });
    }
    let plan: CommandPlan = serde_json::from_slice(bytes)
        .map_err(|error| PlanCodecError::InvalidJson(error.to_string()))?;
    if plan.schema_version != COMMAND_PLAN_SCHEMA_VERSION {
        return Err(PlanCodecError::UnsupportedSchema {
            actual: plan.schema_version,
            expected: COMMAND_PLAN_SCHEMA_VERSION,
        });
    }
    plan.validate()
        .map_err(|error| PlanCodecError::InvalidPlan(error.to_string()))?;
    Ok(plan)
}

#[test]
fn plan_decoding_preserves_frozen_bytes_errors_and_admission_order() {
    use leselang_command::{LoweringContext, lower_effect};
    use leselang_host_contract::{
        CAPABILITY_RUNTIME_READ, CapabilitySet, CommandOrigin, Confirmation, Principal, Revision,
    };
    use leserpent_domain::{CommandId, IdempotencyKey};

    let mut comparisons = 0;
    for origin in [
        CommandOrigin::Gui,
        CommandOrigin::Cli,
        CommandOrigin::Leselang,
        CommandOrigin::Model,
        CommandOrigin::CompatibilityAdapter,
    ] {
        let context = LoweringContext {
            principal: Principal::new("codec-user").unwrap(),
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            expected_revision: Some(Revision(7)),
            command_id: CommandId::new("codec-command").unwrap(),
            idempotency_key: IdempotencyKey::new("codec-key").unwrap(),
            origin,
            confirmation: Confirmation::Confirmed,
            dry_run: true,
        };
        for source in [
            "fn main() = runtime.list(role: \"edge\")",
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ] {
            let program = leserpent_hir::lower(&leselang_syntax::parse(source)).unwrap();
            let plan = lower_effect(&program.function.effect, &context).unwrap();
            for defect in 0..12 {
                let mut value = serde_json::to_value(&plan).unwrap();
                match defect {
                    5 => value["required_capability"] = json!("wrong"),
                    6 => value["schema_version"] = json!(0),
                    7 => {
                        value["schema_version"] = json!(0);
                        value["required_capability"] = json!("wrong");
                    }
                    8 => {
                        value.as_object_mut().unwrap().remove("operation");
                    }
                    9 => value["operation"] = json!(false),
                    _ => {}
                }
                let mut bytes = serde_json::to_vec(&value).unwrap();
                match defect {
                    1 => {
                        bytes.pop();
                    }
                    2 => bytes.extend_from_slice(b" false"),
                    3 => bytes.extend_from_slice(b" \n"),
                    4 => {
                        bytes.pop();
                        bytes.extend_from_slice(br#","schema_version":1}"#);
                    }
                    10 => bytes.resize(MAX_COMMAND_PLAN_BYTES, b' '),
                    11 => bytes.resize(MAX_COMMAND_PLAN_BYTES + 1, b' '),
                    _ => {}
                }
                let before = bytes.clone();
                let expected = frozen_decode_plan(&bytes);
                if matches!(defect, 0 | 3 | 10) {
                    assert_eq!(expected, Ok(plan.clone()));
                } else {
                    assert!(expected.is_err(), "defect {defect}");
                }
                assert_eq!(decode_plan(&bytes), expected, "{source}, {defect}");
                assert_eq!(bytes, before);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 240);
}
