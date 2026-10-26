use leselang_command::bounded_json::{BoundedJsonError, encode, measure};
use leselang_command::{CommandPlan, MAX_COMMAND_PLAN_BYTES, PlanCodecError};
use serde::{Serialize, Serializer, ser::SerializeSeq};
use std::io::{self, Write};

// Frozen released plan encoder, including its attempted-byte error field.
fn frozen_encode_plan(plan: &CommandPlan) -> Result<Vec<u8>, PlanCodecError> {
    plan.validate()
        .map_err(|error| PlanCodecError::InvalidPlan(error.to_string()))?;
    let mut writer = CommandPlanWriter::new();
    if let Err(error) = serde_json::to_writer(&mut writer, plan) {
        return if writer.overflowed {
            Err(PlanCodecError::Oversized {
                size: writer.attempted_size,
                limit: MAX_COMMAND_PLAN_BYTES,
            })
        } else {
            Err(PlanCodecError::InvalidJson(error.to_string()))
        };
    }
    Ok(writer.output)
}

struct CommandPlanWriter {
    output: Vec<u8>,
    attempted_size: usize,
    overflowed: bool,
}

impl CommandPlanWriter {
    fn new() -> Self {
        Self {
            output: Vec::with_capacity(MAX_COMMAND_PLAN_BYTES.min(4 * 1024)),
            attempted_size: 0,
            overflowed: false,
        }
    }
}

impl Write for CommandPlanWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.attempted_size = self.output.len().saturating_add(buffer.len());
        if self.attempted_size > MAX_COMMAND_PLAN_BYTES {
            self.overflowed = true;
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("command plan exceeds {MAX_COMMAND_PLAN_BYTES} encoded bytes"),
            ));
        }
        self.output.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn product_plan_encoding_keeps_frozen_bytes_attempted_sizes_and_validation_order() {
    use leselang_command::{LoweringContext, PlannedOperation, encode_plan, lower_effect};
    use leselang_host_contract::{
        CAPABILITY_RUNTIME_READ, CapabilitySet, CommandOrigin, Confirmation, Principal, Revision,
    };
    use leserpent_domain::{CommandId, IdempotencyKey};

    let mut count = 0;
    for origin in [
        CommandOrigin::Gui,
        CommandOrigin::Cli,
        CommandOrigin::Leselang,
        CommandOrigin::Model,
        CommandOrigin::CompatibilityAdapter,
    ] {
        let context = LoweringContext {
            principal: Principal {
                id: "codec-user".into(),
            },
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
            let size = encode_plan(&plan).unwrap().len();
            for extra in [
                0,
                1,
                MAX_COMMAND_PLAN_BYTES - size,
                MAX_COMMAND_PLAN_BYTES,
                MAX_COMMAND_PLAN_BYTES + 1,
            ] {
                for invalid in [false, true] {
                    let mut candidate = plan.clone();
                    if extra > 0 {
                        let PlannedOperation::Query(query) = &mut candidate.operation else {
                            unreachable!()
                        };
                        query.capabilities = CapabilitySet::new([
                            CAPABILITY_RUNTIME_READ.to_string(),
                            "x".repeat(extra),
                        ]);
                    }
                    if invalid {
                        candidate.required_capability = "wrong-capability".into();
                    }
                    let before = serde_json::to_vec(&candidate).unwrap();
                    assert_eq!(encode_plan(&candidate), frozen_encode_plan(&candidate));
                    assert_eq!(serde_json::to_vec(&candidate).unwrap(), before);
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 200);
}

#[test]
fn compact_bytes_and_measured_size_agree_at_every_boundary() {
    for value in [
        serde_json::json!(null),
        serde_json::json!([]),
        serde_json::json!({"escaped": "\"\\\n\u{0000}", "utf8": "\u{7fbd}\u{86c7}"}),
        serde_json::json!([u64::MAX, i64::MIN, true, {"nested": [1, 2]}]),
    ] {
        let expected = serde_json::to_vec(&value).unwrap();
        for limit in 0..=expected.len() + 1 {
            match (encode(&value, limit), measure(&value, limit)) {
                (Ok(bytes), Ok(size)) => {
                    assert_eq!(bytes, expected);
                    assert_eq!(size, expected.len());
                    assert!(size <= limit);
                }
                (
                    Err(BoundedJsonError::Limit {
                        attempted: encoded,
                        limit: a,
                    }),
                    Err(BoundedJsonError::Limit {
                        attempted: measured,
                        limit: b,
                    }),
                ) => {
                    assert_eq!((a, b), (limit, limit));
                    assert_eq!(encoded, measured);
                    assert!(encoded > limit);
                }
                other => panic!("inconsistent boundary: {other:?}"),
            }
        }
    }
    assert_eq!(
        encode("borrowed unsized str", usize::MAX).unwrap(),
        b"\"borrowed unsized str\""
    );
}

#[test]
fn borrowed_non_send_serializer_is_called_once_per_operation() {
    use std::{cell::Cell, rc::Rc};
    struct Borrowed<'a> {
        calls: Rc<Cell<usize>>,
        text: &'a str,
    }
    impl Serialize for Borrowed<'_> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.calls.set(self.calls.get() + 1);
            serializer.serialize_str(self.text)
        }
    }
    let calls = Rc::new(Cell::new(0));
    let text = String::from("borrowed");
    let value = Borrowed {
        calls: calls.clone(),
        text: &text,
    };
    assert_eq!(encode(&value, 10).unwrap(), b"\"borrowed\"");
    assert_eq!(calls.get(), 1);
    assert_eq!(measure(&value, 10).unwrap(), 10);
    assert_eq!(calls.get(), 2);
    assert!(encode(&value, 1).is_err());
    assert_eq!(calls.get(), 3);
}

struct Refuses;
impl Serialize for Refuses {
    fn serialize<S: Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("native serializer refusal"))
    }
}

#[test]
fn serializer_errors_remain_distinct_and_partial_buffers_are_not_returned() {
    for result in [
        encode(&Refuses, 0).map(|_| ()),
        measure(&Refuses, 0).map(|_| ()),
    ] {
        let Err(BoundedJsonError::Serialize(error)) = result else {
            panic!("lost serializer error")
        };
        assert_eq!(error.to_string(), "native serializer refusal");
    }
    assert!(matches!(
        encode(&(1, Refuses), 100),
        Err(BoundedJsonError::Serialize(_))
    ));
    assert!(matches!(
        encode(&(1, Refuses), 0),
        Err(BoundedJsonError::Limit { .. })
    ));
}

struct SwallowsWriteFailure;
impl Serialize for SwallowsWriteFailure {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(None)?;
        let _ = sequence.serialize_element(&"x".repeat(100));
        let _ = sequence.serialize_element(&0);
        sequence.end()
    }
}

#[test]
fn swallowed_overflow_is_sticky_and_retains_the_first_attempted_size() {
    for result in [
        encode(&SwallowsWriteFailure, 10).map(|_| ()),
        measure(&SwallowsWriteFailure, 10).map(|_| ()),
    ] {
        assert!(matches!(
            result,
            Err(BoundedJsonError::Limit {
                attempted: 102,
                limit: 10
            })
        ));
    }
}
