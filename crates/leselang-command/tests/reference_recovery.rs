use leselang_command::reference_recovery::{
    MAX_JOURNAL_ENTRY_BYTES, RecoveredEffectRecord, ReferenceRecordSource, decode_journal_value,
    inspect_effect_record,
};
use leselang_command::{ContinuationImage, ContinuationToken, MergeWait, Step, Value};
use leselang_host_contract::Revision;
use leselang_runtime_core::{Fault, RecoveryPayload, RecoveryRecord};
use leserpent_hir::{Effect, Type};

fn image() -> ContinuationImage {
    ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: Some(Revision(3)),
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    }
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

struct Source<'a> {
    image: &'a [u8],
    receipt: &'a [u8],
    deadline: Option<i64>,
    fail: Option<&'static str>,
    panic_at: Option<&'static str>,
    trace: Vec<&'static str>,
    image_token: usize,
    receipt_text: usize,
    local_owner: std::rc::Rc<()>,
}

impl<'a> Source<'a> {
    fn new(image: &'a [u8], receipt: &'a [u8], deadline: Option<i64>) -> Self {
        Self {
            image,
            receipt,
            deadline,
            fail: None,
            panic_at: None,
            trace: vec![],
            image_token: 0,
            receipt_text: 0,
            local_owner: std::rc::Rc::new(()),
        }
    }
    fn hit(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.push(stage);
        assert_ne!(self.panic_at, Some(stage), "native hook unwind");
        if self.fail == Some(stage) {
            return Err(fault("NATIVE", stage));
        }
        Ok(())
    }
}

impl<'a> ReferenceRecordSource for Source<'a> {
    type Bytes = &'a [u8];

    fn read_image(&mut self) -> Result<Self::Bytes, Fault> {
        self.hit("image")?;
        Ok(self.image)
    }
    fn read_deadline(&mut self) -> Result<Option<i64>, Fault> {
        self.hit("deadline")?;
        Ok(self.deadline)
    }
    fn validate_owner(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        self.hit("owner")?;
        self.image_token = image.token.as_str().as_ptr() as usize;
        Ok(())
    }
    fn read_completed(&mut self) -> Result<Self::Bytes, Fault> {
        self.hit("receipt")?;
        Ok(self.receipt)
    }
    fn validate_successor(
        &mut self,
        _image: &ContinuationImage,
        completed: Option<&Step>,
    ) -> Result<(), Fault> {
        self.hit("successor")?;
        if let Some(Step::Done(Value::UiFocus { node_id })) = completed {
            self.receipt_text = node_id.as_ptr() as usize;
        }
        Ok(())
    }
}

// Frozen record policy uses the unchanged image/terminal validators, but not the
// new adapter, its codec or the shared phase runner being wired into it.
fn frozen_decode<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, Fault> {
    if bytes.len() > limit {
        return Err(fault(
            "LSV4006",
            format!("journal entry exceeds {limit} bytes"),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| fault("LSV4007", format!("invalid journal JSON: {error}")))
}

fn frozen(
    source: &mut Source<'_>,
    token: &str,
    state: &str,
) -> Result<RecoveredEffectRecord, Fault> {
    let image: ContinuationImage = frozen_decode(source.read_image()?, 65_536)?;
    leselang_command::reference_request::validate_continuation_image(&image)?;
    let sequence =
        leselang_command::continuation_identity::canonical_continuation_sequence(&image.token)
            .ok_or_else(|| fault("LSV4007", "journal continuation sequence is invalid"))?;
    if image.token.as_str() != token {
        return Err(fault(
            "LSV4007",
            "journal token does not match continuation image",
        ));
    }
    let deadline = source
        .read_deadline()?
        .map(|n| u64::try_from(n).map_err(|_| fault("LSV4007", "effect deadline is invalid")))
        .transpose()?;
    if deadline != image.deadline_at_ms {
        return Err(fault(
            "LSV4007",
            "journal deadline does not match continuation image",
        ));
    }
    source.validate_owner(&image)?;
    let (key, payload) = match state {
        "pending" => {
            source.validate_successor(&image, None)?;
            (image.token.clone(), RecoveryPayload::Pending(image))
        }
        "completed" => {
            let step = frozen_decode(source.read_completed()?, 8 * 1024 * 1024)?;
            leselang_command::reference_completion::validate_terminal_step(&step)?;
            source.validate_successor(&image, Some(&step))?;
            (image.token, RecoveryPayload::Completed(step))
        }
        _ => return Err(fault("LSV4007", "invalid journal record state")),
    };
    Ok(RecoveryRecord {
        key,
        sequence,
        payload,
    })
}

fn comparable(record: RecoveredEffectRecord) -> (String, u64, serde_json::Value) {
    let payload = match record.payload {
        RecoveryPayload::Pending(image) => serde_json::json!({"pending": image}),
        RecoveryPayload::Completed(step) => serde_json::json!({"completed": step}),
    };
    (record.key.as_str().into(), record.sequence, payload)
}

#[test]
fn record_admission_matches_frozen_fault_precedence_payloads_and_lazy_read_traces() {
    let original = image();
    let mut images = vec![
        serde_json::to_vec(&original).unwrap(),
        b"{".to_vec(),
        vec![b'{'; 65_537],
    ];
    for defect in 0..3 {
        let mut value = original.clone();
        match defect {
            0 => value.schema_version = 0,
            1 => value.result_type = Type::Structured,
            _ => value.token = ContinuationToken::from_wire("continuation-007".into()),
        }
        images.push(serde_json::to_vec(&value).unwrap());
    }
    let receipts = [
        serde_json::to_vec(&Step::Done(Value::UiFocus {
            node_id: "a".into(),
        }))
        .unwrap(),
        b"{".to_vec(),
        serde_json::to_vec(&Step::Waiting(MergeWait {
            merge_token: original.token.clone(),
            completed_branches: 0,
            total_branches: 1,
        }))
        .unwrap(),
        serde_json::to_vec(&Step::Fault(fault("", "bad"))).unwrap(),
    ];
    let mut cases = 0;
    for raw in &images {
        for token in ["continuation-7", "wrong"] {
            for deadline in [None, Some(-1), Some(31_000), Some(31_001)] {
                for state in ["pending", "completed", "wrong"] {
                    for receipt in &receipts {
                        for fail in [
                            None,
                            Some("image"),
                            Some("deadline"),
                            Some("owner"),
                            Some("receipt"),
                            Some("successor"),
                        ] {
                            let mut actual = Source::new(raw, receipt, deadline);
                            let mut expected = Source::new(raw, receipt, deadline);
                            actual.fail = fail;
                            expected.fail = fail;
                            assert_eq!(
                                inspect_effect_record(&mut actual, token, state).map(comparable),
                                frozen(&mut expected, token, state).map(comparable)
                            );
                            assert_eq!(actual.trace, expected.trace);
                            cases += 1;
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3456);
}

#[test]
fn borrowed_unsized_sources_move_admitted_payloads_and_never_decode_pending_receipts() {
    let raw = serde_json::to_vec(&image()).unwrap();
    let receipt = serde_json::to_vec(&Step::Done(Value::UiFocus {
        node_id: "original-text".into(),
    }))
    .unwrap();
    for state in ["pending", "completed"] {
        let mut source = Source::new(&raw, &receipt, Some(31_000));
        let owner = source.local_owner.clone();
        let erased: &mut dyn ReferenceRecordSource<Bytes = &[u8]> = &mut source;
        let record = inspect_effect_record(erased, "continuation-7", state).unwrap();
        assert_eq!(record.sequence, 7);
        match record.payload {
            RecoveryPayload::Pending(image) => {
                assert_eq!(source.image_token, image.token.as_str().as_ptr() as usize);
                assert_eq!(source.trace, ["image", "deadline", "owner", "successor"]);
            }
            RecoveryPayload::Completed(Step::Done(Value::UiFocus { node_id })) => {
                assert_eq!(source.image_token, record.key.as_str().as_ptr() as usize);
                assert_eq!(source.receipt_text, node_id.as_ptr() as usize);
                assert_eq!(
                    source.trace,
                    ["image", "deadline", "owner", "receipt", "successor"]
                );
            }
            _ => panic!("wrong record payload"),
        }
        assert_eq!(std::rc::Rc::strong_count(&owner), 2);
    }
    let mut source = Source::new(&raw, b"invalid receipt must stay cold", Some(31_000));
    source.fail = Some("receipt");
    assert!(inspect_effect_record(&mut source, "continuation-7", "pending").is_ok());
    assert!(!source.trace.contains(&"receipt"));
}

#[test]
fn each_native_hook_unwind_stops_without_retry_or_later_reads() {
    let raw = serde_json::to_vec(&image()).unwrap();
    let receipt = serde_json::to_vec(&Step::Done(Value::UiFocus {
        node_id: "a".into(),
    }))
    .unwrap();
    for state in ["pending", "completed"] {
        let stages: &[&str] = if state == "pending" {
            &["image", "deadline", "owner", "successor"]
        } else {
            &["image", "deadline", "owner", "receipt", "successor"]
        };
        for (index, stage) in stages.iter().enumerate() {
            let mut source = Source::new(&raw, &receipt, Some(31_000));
            source.panic_at = Some(stage);
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                inspect_effect_record(&mut source, "continuation-7", state)
            }));
            assert!(result.is_err());
            assert_eq!(source.trace, stages[..=index]);
        }
    }
}

#[test]
fn bounded_journal_codec_keeps_literal_limits_and_does_not_admit_semantic_values() {
    assert_eq!(MAX_JOURNAL_ENTRY_BYTES, 8 * 1024 * 1024);
    for bytes in [
        b"{}".to_vec(),
        b"{} null".to_vec(),
        b"\"\xff\"".to_vec(),
        vec![b'['; 256],
    ] {
        for limit in [0, bytes.len() - 1, bytes.len(), bytes.len() + 1] {
            assert_eq!(
                decode_journal_value::<serde_json::Value>(&bytes, limit),
                frozen_decode(&bytes, limit)
            );
        }
    }
    let mut invalid = image();
    invalid.schema_version = 0;
    let bytes = serde_json::to_vec(&invalid).unwrap();
    assert_eq!(
        decode_journal_value::<ContinuationImage>(&bytes, bytes.len()),
        Ok(invalid)
    );
    let mut source = Source::new(&bytes, b"{", Some(-1));
    assert_eq!(
        inspect_effect_record(&mut source, "wrong", "wrong")
            .err()
            .unwrap()
            .code,
        "LSV2001"
    );
    assert_eq!(source.trace, ["image"]);
}
