use leselang_command::reference_dispatch_recovery::{
    ReferenceDispatchSource, inspect_dispatch_record,
};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectError, EffectErrorClass, EffectOperation,
    EffectRequest, PresentationEnvelope, PresentationOperation,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{Fault, StoredDispatchMetadata};
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
            schema_version: 1,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            operation: PresentationOperation::Focus {
                node_id: "a".into(),
            },
        }),
        budget: continuation.resource_budget(),
        continuation,
    }
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

struct Source<'a> {
    request: &'a [u8],
    image: &'a [u8],
    retry: Option<&'a [u8]>,
    attempt: i64,
    ready: i64,
    retries: i64,
    acknowledged: bool,
    completed: bool,
    fail: Option<&'static str>,
    trace: Vec<&'static str>,
}

impl<'a> Source<'a> {
    fn new(request: &'a [u8], image: &'a [u8]) -> Self {
        Self {
            request,
            image,
            retry: None,
            attempt: 0,
            ready: 0,
            retries: 0,
            acknowledged: false,
            completed: false,
            fail: None,
            trace: vec![],
        }
    }
    fn hit(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.push(stage);
        if self.fail == Some(stage) {
            Err(fault("NATIVE", stage))
        } else {
            Ok(())
        }
    }
}

impl<'a> ReferenceDispatchSource for Source<'a> {
    type Bytes = &'a [u8];
    fn read_request(&mut self) -> Result<Self::Bytes, Fault> {
        self.hit("request")?;
        Ok(self.request)
    }
    fn read_image(&mut self) -> Result<Self::Bytes, Fault> {
        self.hit("image")?;
        Ok(self.image)
    }
    fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Self::Bytes>, Fault> {
        self.hit("metadata")?;
        Ok(StoredDispatchMetadata {
            attempt: self.attempt,
            ready_at_ms: self.ready,
            retry_count: self.retries,
            last_error: self.retry,
            acknowledged: self.acknowledged,
            effect_completed: self.completed,
        })
    }
}

// Frozen decisions/codec do not call the new adapter or the core phase runner.
fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8], limit: usize) -> Result<T, Fault> {
    if bytes.len() > limit {
        return Err(fault(
            "LSV4006",
            format!("journal entry exceeds {limit} bytes"),
        ));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| fault("LSV4007", format!("invalid journal JSON: {error}")))
}

fn frozen(source: &mut Source<'_>, token: &str) -> Result<(), Fault> {
    let request: EffectRequest = decode(source.read_request()?, 8 * 1024 * 1024)?;
    leselang_command::reference_request::validate_effect_request(&request)?;
    if request.continuation.token.as_str() != token {
        return Err(fault(
            "LSV4007",
            "dispatch token does not match effect request",
        ));
    }
    let original = source.read_image()?;
    let encoded = serde_json::to_vec(&request.continuation).unwrap();
    if encoded.len() > 65_536 {
        return Err(fault("LSV3002", "continuation exceeds 65536 bytes"));
    }
    if encoded != original {
        return Err(fault(
            "LSV4007",
            "dispatch request conflicts with continuation state",
        ));
    }
    let metadata = source.read_metadata()?;
    if metadata.attempt < 0 || metadata.attempt > 100 {
        return Err(fault("LSV4007", "dispatch attempt is invalid"));
    }
    if metadata.ready_at_ms < 0 || metadata.retry_count < 0 || metadata.retry_count > 16 {
        return Err(fault("LSV4007", "dispatch retry state is invalid"));
    }
    if (metadata.retry_count == 0) != metadata.last_error.is_none() {
        return Err(fault("LSV4007", "dispatch retry history is inconsistent"));
    }
    if let Some(bytes) = metadata.last_error {
        let error: EffectError = decode(bytes, 2_048)?;
        leselang_command::retry_policy::validate_effect_error(&error)
            .map_err(|error| fault("LSV2202", error.to_string()))?;
    }
    if metadata.acknowledged != metadata.effect_completed {
        return Err(fault(
            "LSV4007",
            "dispatch and continuation states are inconsistent",
        ));
    }
    Ok(())
}

fn compare(source: &mut Source<'_>, token: &str) {
    let expected = frozen(source, token);
    let expected_trace = std::mem::take(&mut source.trace);
    let dynamic: &mut dyn ReferenceDispatchSource<Bytes = &[u8]> = source;
    assert_eq!(inspect_dispatch_record(dynamic, token), expected);
    assert_eq!(source.trace, expected_trace);
}

#[test]
fn dispatch_policy_matches_frozen_fault_precedence_and_lazy_read_traces() {
    let original = request();
    let image = serde_json::to_vec(&original.continuation).unwrap();
    let mut requests = vec![serde_json::to_vec(&original).unwrap(), b"{".to_vec()];
    for defect in 0..4 {
        let mut value = original.clone();
        match defect {
            0 => value.continuation.schema_version = 0,
            1 => value.effect_id = "effect-8".into(),
            2 => value.required_capability = "wrong".into(),
            _ => value.budget.fuel_remaining += 1,
        }
        requests.push(serde_json::to_vec(&value).unwrap());
    }
    let images = [
        image.clone(),
        b"{".to_vec(),
        serde_json::to_vec_pretty(&original.continuation).unwrap(),
    ];
    let mut cases = 0;
    for bytes in &requests {
        for token in ["continuation-7", "wrong"] {
            for image in &images {
                for fail in [None, Some("request"), Some("image"), Some("metadata")] {
                    for attempt in [-1, 0, 101] {
                        let mut source = Source::new(bytes, image);
                        source.fail = fail;
                        source.attempt = attempt;
                        compare(&mut source, token);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 432);
}

#[test]
fn dispatch_numeric_history_error_and_completion_boundaries_match_frozen_policy() {
    let request = request();
    let raw = serde_json::to_vec(&request).unwrap();
    let image = serde_json::to_vec(&request.continuation).unwrap();
    let valid = serde_json::to_vec(&EffectError {
        class: EffectErrorClass::Transient,
        code: "busy".into(),
        message: "retry".into(),
    })
    .unwrap();
    let invalid = serde_json::to_vec(&EffectError {
        class: EffectErrorClass::Transient,
        code: "".into(),
        message: "retry".into(),
    })
    .unwrap();
    let mut boundary = valid.clone();
    boundary.resize(2_048, b' ');
    let over = vec![b'{'; 2_049];
    let errors = [
        None,
        Some(valid.as_slice()),
        Some(b"{".as_slice()),
        Some(invalid.as_slice()),
        Some(boundary.as_slice()),
        Some(over.as_slice()),
        Some(b"{} {}".as_slice()),
    ];
    let mut cases = 0;
    for attempt in [-1, 0, 1, 100, 101, i64::MAX] {
        for ready in [-1, 0, i64::MAX] {
            for retries in [-1, 0, 1, 16, 17, i64::MAX] {
                for error in errors {
                    for (acknowledged, completed) in
                        [(false, false), (false, true), (true, false), (true, true)]
                    {
                        let mut source = Source::new(&raw, &image);
                        source.attempt = attempt;
                        source.ready = ready;
                        source.retries = retries;
                        source.retry = error;
                        source.acknowledged = acknowledged;
                        source.completed = completed;
                        compare(&mut source, "continuation-7");
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 3024);
}

#[test]
fn request_bound_is_checked_before_parsing_and_image_comparison_is_byte_exact() {
    let request = request();
    let image = serde_json::to_vec(&request.continuation).unwrap();
    let mut raw = serde_json::to_vec(&request).unwrap();
    raw.resize(8 * 1024 * 1024, b' ');
    assert_eq!(
        inspect_dispatch_record(&mut Source::new(&raw, &image), "continuation-7"),
        Ok(())
    );
    raw.push(b' ');
    let mut source = Source::new(&raw, &image);
    assert_eq!(
        inspect_dispatch_record(&mut source, "continuation-7"),
        Err(fault("LSV4006", "journal entry exceeds 8388608 bytes"))
    );
    assert_eq!(source.trace, ["request"]);
    let raw = serde_json::to_vec(&request).unwrap();
    let mut changed = image;
    changed.push(b' ');
    let mut source = Source::new(&raw, &changed);
    assert_eq!(
        inspect_dispatch_record(&mut source, "continuation-7"),
        Err(fault(
            "LSV4007",
            "dispatch request conflicts with continuation state"
        ))
    );
    assert_eq!(source.trace, ["request", "image"]);
}

#[test]
fn local_move_only_byte_owners_and_each_unwind_stop_without_retry() {
    use std::cell::RefCell;
    use std::rc::Rc;
    type Trace = Rc<RefCell<Vec<&'static str>>>;
    struct Bytes {
        value: Vec<u8>,
        trace: Trace,
        borrow: &'static str,
        drop: &'static str,
        panic_at: Option<&'static str>,
    }
    impl AsRef<[u8]> for Bytes {
        fn as_ref(&self) -> &[u8] {
            self.trace.borrow_mut().push(self.borrow);
            assert_ne!(self.panic_at, Some(self.borrow));
            &self.value
        }
    }
    impl Drop for Bytes {
        fn drop(&mut self) {
            self.trace.borrow_mut().push(self.drop);
            assert_ne!(self.panic_at, Some(self.drop));
        }
    }
    struct Owned {
        request: Option<Bytes>,
        image: Option<Bytes>,
        retry: Option<Bytes>,
        trace: Trace,
        panic_at: Option<&'static str>,
    }
    impl Owned {
        fn hit(&self, stage: &'static str) {
            self.trace.borrow_mut().push(stage);
            assert_ne!(self.panic_at, Some(stage));
        }
    }
    impl ReferenceDispatchSource for Owned {
        type Bytes = Bytes;
        fn read_request(&mut self) -> Result<Bytes, Fault> {
            self.hit("request");
            Ok(self.request.take().unwrap())
        }
        fn read_image(&mut self) -> Result<Bytes, Fault> {
            self.hit("image");
            Ok(self.image.take().unwrap())
        }
        fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Bytes>, Fault> {
            self.hit("metadata");
            Ok(StoredDispatchMetadata {
                attempt: 1,
                ready_at_ms: 0,
                retry_count: 1,
                last_error: self.retry.take(),
                acknowledged: false,
                effect_completed: false,
            })
        }
    }
    let stages = [
        "request",
        "request.borrow",
        "request.drop",
        "image",
        "image.borrow",
        "image.drop",
        "metadata",
        "retry.borrow",
        "retry.drop",
    ];
    for panic_at in std::iter::once(None).chain(stages.map(Some)) {
        let trace = Rc::new(RefCell::new(vec![]));
        let bytes = |value, borrow, drop| Bytes {
            value,
            borrow,
            drop,
            trace: trace.clone(),
            panic_at,
        };
        let request = request();
        let mut source = Owned {
            request: Some(bytes(
                serde_json::to_vec(&request).unwrap(),
                "request.borrow",
                "request.drop",
            )),
            image: Some(bytes(
                serde_json::to_vec(&request.continuation).unwrap(),
                "image.borrow",
                "image.drop",
            )),
            retry: Some(bytes(
                serde_json::to_vec(&EffectError {
                    class: EffectErrorClass::Transient,
                    code: "busy".into(),
                    message: "retry".into(),
                })
                .unwrap(),
                "retry.borrow",
                "retry.drop",
            )),
            trace: trace.clone(),
            panic_at,
        };
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dynamic: &mut dyn ReferenceDispatchSource<Bytes = Bytes> = &mut source;
            inspect_dispatch_record(dynamic, "continuation-7")
        }));
        assert_eq!(result.is_err(), panic_at.is_some());
        let seen = trace.borrow().clone();
        if let Some(stage) = panic_at {
            let index = stages.iter().position(|item| *item == stage).unwrap();
            let cleanup = usize::from(stage.ends_with(".borrow"));
            assert_eq!(seen, stages[..=index + cleanup], "{stage}");
        } else {
            assert_eq!(result.unwrap(), Ok(()));
            assert_eq!(seen, stages);
        }
        // Unread owners remain with the caller; their later cleanup is not replay.
        source.panic_at = None;
        for bytes in [&mut source.request, &mut source.image, &mut source.retry]
            .into_iter()
            .flatten()
        {
            bytes.panic_at = None;
        }
    }
}
