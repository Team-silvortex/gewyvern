use leselang_command::continuation_identity::MAX_EFFECT_SEQUENCE;
use leselang_command::reference_checkpoint::{
    MAX_JOURNAL_TOTAL_BYTES, ReferenceCheckpoint, ReferenceCheckpointSource, recover_checkpoint,
};
use leselang_command::reference_recovery::RecoveredEffectRecord;
use leselang_command::retention_policy::MAX_JOURNAL_RECORDS;
use leselang_command::{ContinuationImage, ContinuationToken, Step, Value};
use leselang_host_contract::Revision;
use leselang_runtime_core::{
    CheckpointCommitError, CheckpointError, CheckpointFinalizer, CheckpointRecovery, Fault,
    RecoveryPayload, RecoveryRecord,
};
use leserpent_hir::{Effect, Type};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}

fn record(sequence: u64, pending: bool) -> RecoveredEffectRecord {
    let key = ContinuationToken::from_wire(format!("continuation-{sequence}"));
    let payload = if pending {
        RecoveryPayload::Pending(ContinuationImage {
            schema_version: 1,
            token: key.clone(),
            program_counter: 1,
            expected_revision: Some(Revision(3)),
            result_type: Type::UiFocus,
            pending_effect: Effect::UiFocus {
                node_id: "original-node".into(),
            },
            fuel_remaining: 99,
            deadline_ms: 30_000,
            deadline_at_ms: Some(31_000),
            max_output_items: 10_000,
            result_binding: None,
            group_result: None,
        })
    } else {
        RecoveryPayload::Completed(Step::Done(Value::UiFocus {
            node_id: "original-node".into(),
        }))
    };
    RecoveryRecord {
        key,
        sequence,
        payload,
    }
}

struct Source<'a> {
    trace: &'a RefCell<Vec<String>>,
    local: Rc<()>,
    records: VecDeque<RecoveredEffectRecord>,
    count: usize,
    bytes: usize,
    graph: u64,
    fail: Option<&'static str>,
    panic_at: Option<&'static str>,
    ignore_push_error: bool,
}

impl<'a> Source<'a> {
    fn new(trace: &'a RefCell<Vec<String>>, case: usize) -> Self {
        let records = match case {
            0 => vec![],
            1 => vec![record(7, true)],
            2 => vec![record(8, false)],
            3 => vec![record(7, true), record(3, false), record(11, true)],
            4 => vec![record(7, true), record(7, false)],
            5 => vec![record(7, true), record(0, false)],
            6 => vec![record(7, true), record(MAX_EFFECT_SEQUENCE + 1, true)],
            7 => vec![record(MAX_EFFECT_SEQUENCE, false)],
            _ => unreachable!(),
        };
        Self {
            trace,
            local: Rc::new(()),
            count: records.len(),
            bytes: 100,
            records: records.into(),
            graph: 0,
            fail: None,
            panic_at: None,
            ignore_push_error: false,
        }
    }

    fn hit(&self, event: &str) -> Result<(), Fault> {
        assert_eq!(Rc::strong_count(&self.local), 1);
        self.trace.borrow_mut().push(event.into());
        assert_ne!(self.panic_at, Some(event), "original owner unwind");
        if self.fail == Some(event) {
            Err(fault("NATIVE", event))
        } else {
            Ok(())
        }
    }
}

impl Drop for Source<'_> {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("drop".into());
        assert_ne!(self.panic_at, Some("drop"), "original owner cleanup unwind");
    }
}

impl ReferenceCheckpointSource for Source<'_> {
    fn record_count(&mut self) -> Result<usize, Fault> {
        self.hit("count")?;
        Ok(self.count)
    }
    fn payload_bytes(&mut self) -> Result<usize, Fault> {
        self.hit("bytes")?;
        Ok(self.bytes)
    }
    fn stage_records(
        &mut self,
        push: &mut dyn FnMut(RecoveredEffectRecord) -> Result<(), Fault>,
    ) -> Result<(), Fault> {
        self.hit("rows")?;
        let mut index = 0;
        while let Some(record) = self.records.pop_front() {
            self.hit(&format!("row:{index}"))?;
            let result = push(record);
            if !self.ignore_push_error {
                result?;
            }
            index += 1;
        }
        self.hit("end")
    }
}

impl CheckpointFinalizer for Source<'_> {
    type Commit = u64;
    type Error = Fault;
    fn validate_dispatches(&mut self) -> Result<(), Fault> {
        self.hit("dispatch")
    }
    fn validate_graphs(&mut self) -> Result<u64, Fault> {
        self.hit("graph")?;
        Ok(self.graph)
    }
    fn validate_audits(&mut self) -> Result<(), Fault> {
        self.hit("audit")
    }
    fn advance_watermark(&mut self, highest: u64) -> Result<u64, Fault> {
        self.hit("watermark")?;
        self.trace.borrow_mut().push(format!("highest:{highest}"));
        Ok(highest + 1)
    }
    fn commit(&mut self) -> Result<(), Fault> {
        self.hit("commit")
    }
}

// Frozen product coordination, intentionally not the new adapter or phase runner.
fn frozen(mut source: Source<'_>) -> Result<(ReferenceCheckpoint, u64), Fault> {
    let count = source.record_count()?;
    if count > MAX_JOURNAL_RECORDS {
        return Err(fault(
            "LSV4006",
            format!("journal has {count} records, limit is {MAX_JOURNAL_RECORDS}"),
        ));
    }
    let bytes = source.payload_bytes()?;
    if bytes > MAX_JOURNAL_TOTAL_BYTES {
        return Err(fault(
            "LSV4006",
            format!("journal payload is {bytes} bytes, limit is {MAX_JOURNAL_TOTAL_BYTES}"),
        ));
    }
    let mut recovery = CheckpointRecovery::new(MAX_JOURNAL_RECORDS, MAX_EFFECT_SEQUENCE);
    source.stage_records(&mut |record| recovery.push(record).map_err(frozen_fault))?;
    recovery
        .finish(source, |source, snapshot| {
            source.validate_dispatches()?;
            let highest = snapshot.highest_sequence().max(source.validate_graphs()?);
            source.validate_audits()?;
            let next = source.advance_watermark(highest)?;
            source.commit()?;
            Ok(next)
        })
        .map_err(|error| match error {
            CheckpointCommitError::Host(error) => error,
            CheckpointCommitError::Closed => frozen_fault(CheckpointError::Closed),
        })
}

fn frozen_fault(error: CheckpointError) -> Fault {
    match error {
        CheckpointError::RecordLimit => fault("LSV4006", "journal recovery record limit exceeded"),
        CheckpointError::InvalidSequence => {
            fault("LSV4007", "journal continuation sequence is invalid")
        }
        CheckpointError::DuplicateKey => fault("LSV4007", "duplicate journal continuation token"),
        CheckpointError::Closed => fault("LSV4007", "journal recovery pass is closed"),
    }
}

#[test]
fn checkpoint_policy_matches_frozen_faults_maps_read_order_and_finalization() {
    let budgets = [
        (0, 0),
        (MAX_JOURNAL_RECORDS, MAX_JOURNAL_TOTAL_BYTES),
        (MAX_JOURNAL_RECORDS + 1, 0),
        (0, MAX_JOURNAL_TOTAL_BYTES + 1),
        (MAX_JOURNAL_RECORDS + 1, MAX_JOURNAL_TOTAL_BYTES + 1),
    ];
    let failures = [
        None,
        Some("count"),
        Some("bytes"),
        Some("rows"),
        Some("row:0"),
        Some("row:1"),
        Some("row:2"),
        Some("end"),
        Some("dispatch"),
        Some("graph"),
        Some("audit"),
        Some("watermark"),
        Some("commit"),
    ];
    let mut comparisons = 0;
    for case in 0..8 {
        for (count, bytes) in budgets {
            for fail in failures {
                for graph in [0, 25, MAX_EFFECT_SEQUENCE] {
                    let actual_trace = RefCell::new(vec![]);
                    let expected_trace = RefCell::new(vec![]);
                    let mut actual = Source::new(&actual_trace, case);
                    let mut expected = Source::new(&expected_trace, case);
                    for source in [&mut actual, &mut expected] {
                        source.count = count;
                        source.bytes = bytes;
                        source.fail = fail;
                        source.graph = graph;
                    }
                    let result = recover_checkpoint(actual)
                        .map(|(snapshot, next)| (snapshot.into_parts(), next));
                    let reference =
                        frozen(expected).map(|(snapshot, next)| (snapshot.into_parts(), next));
                    assert_eq!(
                        result, reference,
                        "case={case}, count={count}, bytes={bytes}, fail={fail:?}, graph={graph}"
                    );
                    assert_eq!(actual_trace, expected_trace);
                    assert_eq!(actual_trace.borrow().last().unwrap(), "drop");
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 1560);
}

#[test]
fn checkpoint_uses_original_owned_payloads_and_cleans_up_before_publication() {
    let trace = RefCell::new(vec![]);
    let source = Source::new(&trace, 3);
    let RecoveryPayload::Pending(image) = &source.records[0].payload else {
        panic!()
    };
    let token_pointer = image.token.as_str().as_ptr();
    let RecoveryPayload::Completed(Step::Done(Value::UiFocus { node_id })) =
        &source.records[1].payload
    else {
        panic!()
    };
    let result_pointer = node_id.as_ptr();
    let (snapshot, next) = recover_checkpoint(source).unwrap();
    assert_eq!(
        *trace.borrow(),
        [
            "count",
            "bytes",
            "rows",
            "row:0",
            "row:1",
            "row:2",
            "end",
            "dispatch",
            "graph",
            "audit",
            "watermark",
            "highest:11",
            "commit",
            "drop"
        ]
    );
    assert_eq!(next, 12);
    let image = &snapshot.pending()[&ContinuationToken::from_wire("continuation-7".into())];
    assert_eq!(image.token.as_str().as_ptr(), token_pointer);
    assert_eq!(image.fuel_remaining, 99);
    assert_eq!(image.expected_revision, Some(Revision(3)));
    assert_eq!(image.deadline_at_ms, Some(31_000));
    let Step::Done(Value::UiFocus { node_id }) =
        &snapshot.completed()[&ContinuationToken::from_wire("continuation-3".into())]
    else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), result_pointer);
}

#[test]
fn checkpoint_rechecks_stream_limit_and_poisoned_staging_never_finalizes() {
    for length in [MAX_JOURNAL_RECORDS, MAX_JOURNAL_RECORDS + 1] {
        for ignore in [false, true] {
            let trace = RefCell::new(vec![]);
            let mut source = Source::new(&trace, 0);
            source.records = (1..=length).map(|i| record(i as u64, false)).collect();
            source.ignore_push_error = ignore;
            let result = recover_checkpoint(source);
            if length == MAX_JOURNAL_RECORDS {
                let (snapshot, next) = result.unwrap();
                assert_eq!(snapshot.completed().len(), length);
                assert_eq!(next, length as u64 + 1);
            } else {
                let error = result.err().unwrap();
                assert_eq!(
                    error,
                    frozen_fault(if ignore {
                        CheckpointError::Closed
                    } else {
                        CheckpointError::RecordLimit
                    })
                );
                assert!(!trace.borrow().iter().any(|s| s == "dispatch"));
            }
        }
    }
    for case in [4, 5, 6] {
        let trace = RefCell::new(vec![]);
        let mut source = Source::new(&trace, case);
        source.ignore_push_error = true;
        assert_eq!(
            recover_checkpoint(source).err().unwrap(),
            frozen_fault(CheckpointError::Closed)
        );
        assert!(!trace.borrow().iter().any(|s| s == "dispatch"));
    }
}

#[test]
fn checkpoint_unwind_at_every_owner_boundary_cannot_publish_or_retry() {
    let boundaries = [
        "count",
        "bytes",
        "rows",
        "row:0",
        "row:1",
        "row:2",
        "end",
        "dispatch",
        "graph",
        "audit",
        "watermark",
        "commit",
        "drop",
    ];
    for boundary in boundaries {
        let trace = RefCell::new(vec![]);
        let mut source = Source::new(&trace, 3);
        source.panic_at = Some(boundary);
        let mut published = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _checkpoint = recover_checkpoint(source);
            published = true;
        }));
        assert!(result.is_err(), "{boundary}");
        assert!(!published);
        let events = trace.borrow();
        assert_eq!(events.iter().filter(|s| *s == "drop").count(), 1);
        assert_eq!(events.iter().filter(|s| *s == boundary).count(), 1);
        assert!(events.iter().filter(|s| *s == "commit").count() <= 1);
    }
}
