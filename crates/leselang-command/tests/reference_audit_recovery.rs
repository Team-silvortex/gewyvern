use leselang_command::reference_audit_recovery::*;
use leselang_command::{Cancellation, CancellationReason, ContinuationToken, Step};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn terminal(time: u64) -> Vec<u8> {
    serde_json::to_vec(&Step::Cancelled(Cancellation {
        continuation: ContinuationToken::from_wire("effect-1".into()),
        reason: CancellationReason::Requested,
        observed_at_ms: time,
    }))
    .unwrap()
}

struct Row<'a> {
    text: [String; 9],
    integers: [i64; 2],
    bytes: &'a [u8],
    trace: Rc<RefCell<Vec<usize>>>,
    failure: Option<(usize, Fault)>,
    unwind: Option<usize>,
}
impl<'a> Row<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            text: [
                "audit-1",
                "operator",
                "key-1",
                "gui",
                "session-1",
                "",
                "",
                "effect-1",
                "completed",
            ]
            .map(String::from),
            integers: [7, 42],
            bytes,
            trace: Rc::default(),
            failure: None,
            unwind: None,
        }
    }
    fn observe(&mut self, index: usize) -> Result<(), Fault> {
        self.trace.borrow_mut().push(index);
        assert_ne!(self.unwind, Some(index), "native row unwind");
        if self.failure.as_ref().is_some_and(|(at, _)| *at == index) {
            return Err(self.failure.take().unwrap().1);
        }
        Ok(())
    }
}
impl<'a> ReferenceAuditRow for Row<'a> {
    type Bytes = &'a [u8];
    type InvalidOrigin = &'static str;
    fn read_text(&mut self, column: AuditTextColumn) -> Result<String, Fault> {
        self.observe(column.index())?;
        Ok(self.text[column.index()].clone())
    }
    fn read_integer(&mut self, column: AuditIntegerColumn) -> Result<i64, Fault> {
        self.observe(column.index())?;
        Ok(self.integers[column.index() - 5])
    }
    fn read_terminal(&mut self) -> Result<&'a [u8], Fault> {
        self.observe(9)?;
        Ok(self.bytes)
    }
    fn invalid_origin(&mut self) -> &'static str {
        self.trace.borrow_mut().push(10);
        "native origin detail"
    }
}

#[test]
fn audit_rows_accept_exact_origins_and_signed_clock_boundaries_without_owned_bytes() {
    for origin in ["gui", "cli", "leselang", "model", "compatibility_adapter"] {
        for time in [0, 42, i64::MAX] {
            let bytes = terminal(time as u64);
            let mut row = Row::new(&bytes);
            row.text[3] = origin.into();
            row.integers = [i64::MAX, time];
            inspect_audit_record(&mut row).unwrap();
            assert_eq!(*row.trace.borrow(), (0..10).collect::<Vec<_>>());
            assert_eq!(row.bytes.as_ptr(), bytes.as_ptr());
        }
    }
}

#[test]
fn row_reads_stop_at_each_native_fault_and_move_the_original_fault_buffer() {
    let bytes = terminal(42);
    for at in 0..10 {
        let mut row = Row::new(&bytes);
        let error = Fault {
            code: "NATIVE".into(),
            message: "unchanged native detail".into(),
        };
        let pointer = error.message.as_ptr();
        row.failure = Some((at, error));
        let error = inspect_audit_record(&mut row).unwrap_err();
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(error.code, "NATIVE");
        assert_eq!(*row.trace.borrow(), (0..=at).collect::<Vec<_>>());
    }
}

#[test]
fn row_reads_never_continue_after_a_native_unwind() {
    let bytes = terminal(42);
    for at in 0..10 {
        let mut row = Row::new(&bytes);
        row.unwind = Some(at);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| inspect_audit_record(
                &mut row
            )))
            .is_err()
        );
        assert_eq!(*row.trace.borrow(), (0..=at).collect::<Vec<_>>());
    }
}

#[test]
fn semantic_refusals_keep_lazy_fault_precedence_and_do_not_read_later_columns() {
    let bytes = terminal(42);
    for (column, value, stop, message) in [
        (0, "", 0, "debugger audit command is invalid"),
        (1, "bad principal", 7, "debugger audit identity is invalid"),
        (2, "", 2, "debugger audit idempotency key is invalid"),
        (
            3,
            "GUI",
            3,
            "debugger audit origin is invalid: native origin detail",
        ),
        (4, "bad session", 7, "debugger audit identity is invalid"),
        (7, "bad token", 7, "debugger audit continuation is invalid"),
        (8, "pending", 8, "debugger audit continuation is invalid"),
    ] {
        let mut row = Row::new(&bytes);
        row.text[column] = value.into();
        row.failure = Some((
            stop + 1,
            Fault {
                code: "LATER".into(),
                message: "must not read".into(),
            },
        ));
        let error = inspect_audit_record(&mut row).unwrap_err();
        assert_eq!(error.code, "LSV4007");
        assert_eq!(error.message, message);
        let mut expected = (0..=stop).collect::<Vec<_>>();
        if column == 3 {
            expected.push(10);
        }
        assert_eq!(*row.trace.borrow(), expected);
    }
    for index in 0..2 {
        let mut row = Row::new(&bytes);
        row.integers[index] = -1;
        assert_eq!(
            inspect_audit_record(&mut row).unwrap_err().message,
            if index == 0 {
                "debugger audit revision is invalid"
            } else {
                "debugger audit clock is invalid"
            }
        );
        assert_eq!(*row.trace.borrow(), (0..=index + 5).collect::<Vec<_>>());
    }
    let mut row = Row::new(&bytes);
    row.text[1] = "bad principal".into();
    row.text[2] = "".into();
    assert_eq!(
        inspect_audit_record(&mut row).unwrap_err().message,
        "debugger audit idempotency key is invalid"
    );
}

#[test]
fn terminal_evidence_requires_the_original_requested_cancel_token_and_clock() {
    let mut values = Vec::new();
    for (token, reason, time) in [
        ("effect-2", CancellationReason::Requested, 42),
        ("effect-1", CancellationReason::Requested, 43),
        ("effect-1", CancellationReason::DeadlineExceeded, 42),
    ] {
        values.push(
            serde_json::to_vec(&Step::Cancelled(Cancellation {
                continuation: ContinuationToken::from_wire(token.into()),
                reason,
                observed_at_ms: time,
            }))
            .unwrap(),
        );
    }
    for bytes in values {
        assert_eq!(
            inspect_audit_record(&mut Row::new(&bytes))
                .unwrap_err()
                .message,
            "debugger audit does not match its terminal cancellation"
        );
    }
    let malformed = inspect_audit_record(&mut Row::new(b"{")).unwrap_err();
    assert_eq!(malformed.code, "LSV4007");
    assert!(malformed.message.starts_with("invalid journal JSON:"));
    let oversized = vec![b' '; leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES + 1];
    assert_eq!(
        inspect_audit_record(&mut Row::new(&oversized))
            .unwrap_err()
            .code,
        "LSV4006"
    );
}

struct Source {
    trace: Rc<RefCell<Vec<usize>>>,
    values: [u32; 3],
    failure: Option<(usize, Fault)>,
    unwind: Option<usize>,
}
impl Source {
    fn read(&mut self, index: usize) -> Result<u32, Fault> {
        self.trace.borrow_mut().push(index);
        assert_ne!(self.unwind, Some(index), "native stream unwind");
        if self.failure.as_ref().is_some_and(|(at, _)| *at == index) {
            return Err(self.failure.take().unwrap().1);
        }
        Ok(self.values[index])
    }
}
impl ReferenceAuditSource for Source {
    fn index_count(&mut self) -> Result<u32, Fault> {
        self.read(0)
    }
    fn audit_count(&mut self) -> Result<u32, Fault> {
        self.read(1)
    }
    fn inspect_records(&mut self) -> Result<u32, Fault> {
        self.read(2)
    }
}
fn source(values: [u32; 3]) -> Source {
    Source {
        trace: Rc::default(),
        values,
        failure: None,
        unwind: None,
    }
}

#[test]
fn complete_pass_keeps_schema_before_count_before_scan_and_checks_join_cardinality() {
    for values in [
        [0, 1, 1],
        [2, 1, 1],
        [1, 0, 0],
        [1, 3, 3],
        [1, 3, 2],
        [1, 0, 1],
    ] {
        let mut host = source(values);
        let result = validate_audit_records(&mut host as &mut dyn ReferenceAuditSource);
        if values[0] != 1 {
            assert_eq!(
                result.unwrap_err().message,
                "debugger audit schema is incomplete"
            );
            assert_eq!(*host.trace.borrow(), [0]);
        } else {
            assert_eq!(*host.trace.borrow(), [0, 1, 2]);
            if values[1] == values[2] {
                assert_eq!(result, Ok(()));
            } else {
                assert_eq!(
                    result.unwrap_err().message,
                    "debugger audit references a missing continuation"
                );
            }
        }
    }
}

#[test]
fn complete_pass_preserves_native_faults_and_unwinds_without_running_later_hooks() {
    for at in 0..3 {
        let mut host = source([1, 3, 3]);
        let error = Fault {
            code: "NATIVE".into(),
            message: "original scan error".into(),
        };
        let pointer = error.message.as_ptr();
        host.failure = Some((at, error));
        assert_eq!(
            validate_audit_records(&mut host)
                .unwrap_err()
                .message
                .as_ptr(),
            pointer
        );
        assert_eq!(*host.trace.borrow(), (0..=at).collect::<Vec<_>>());
        let mut host = source([1, 3, 3]);
        host.unwind = Some(at);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| validate_audit_records(
                &mut host
            )))
            .is_err()
        );
        assert_eq!(*host.trace.borrow(), (0..=at).collect::<Vec<_>>());
    }
}
