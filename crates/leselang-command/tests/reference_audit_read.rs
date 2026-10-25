use leselang_command::reference_audit_read::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
enum NativeError {
    Read(String),
    Invalid,
}
struct Row {
    text: [Option<String>; 8],
    integer: [i64; 8],
    trace: Rc<RefCell<Vec<usize>>>,
    failure: Option<(usize, NativeError)>,
    unwind: Option<usize>,
}
impl Row {
    fn new(replay: bool) -> Self {
        let text = if replay {
            [
                "audit", "effect-1", "key", "operator", "gui", "session", "", "",
            ]
        } else {
            ["audit", "operator", "gui", "session", "", "", "", ""]
        }
        .map(|s| Some(s.into()));
        let mut integer = [7; 8];
        integer[if replay { 7 } else { 5 }] = 42;
        Self {
            text,
            integer,
            trace: Rc::default(),
            failure: None,
            unwind: None,
        }
    }
    fn enter(&mut self, index: usize) -> Result<(), NativeError> {
        self.trace.borrow_mut().push(index);
        assert_ne!(self.unwind, Some(index), "native read unwind");
        if self.failure.as_ref().is_some_and(|(at, _)| *at == index) {
            return Err(self.failure.take().unwrap().1);
        }
        Ok(())
    }
}
impl ReferenceAuditReadRow for Row {
    type Error = NativeError;
    fn text(&mut self, column: usize) -> Result<String, NativeError> {
        self.enter(column)?;
        Ok(self.text[column].take().unwrap())
    }
    fn integer(&mut self, column: usize) -> Result<i64, NativeError> {
        self.enter(column)?;
        Ok(self.integer[column])
    }
    fn invalid_data(&mut self) -> NativeError {
        self.trace.borrow_mut().push(99);
        assert_ne!(self.unwind, Some(99), "native mapping unwind");
        NativeError::Invalid
    }
}
fn run(
    row: &mut dyn ReferenceAuditReadRow<Error = NativeError>,
    replay: bool,
) -> Result<(), NativeError> {
    if replay {
        decode_replay(row).map(|_| ())
    } else {
        decode_record(row).map(|_| ())
    }
}

#[test]
fn both_projections_preserve_exact_origins_signed_boundaries_and_local_borrowed_owner() {
    for replay in [false, true] {
        for origin in ["gui", "cli", "leselang", "model", "compatibility_adapter"] {
            for clock in [0, i64::MAX] {
                let mut row = Row::new(replay);
                let origin_column = if replay { 4 } else { 2 };
                row.text[origin_column] = Some(origin.into());
                row.integer[origin_column + 2] = clock;
                row.integer[origin_column + 3] = clock;
                let record = if replay {
                    decode_replay(&mut row).unwrap().3
                } else {
                    decode_record(&mut row).unwrap()
                };
                assert_eq!(record.expected_revision.0, clock as u64);
                assert_eq!(record.observed_at_ms, clock as u64);
                assert_eq!(
                    leselang_command::debugger_audit::command_origin_label(record.origin),
                    origin
                );
                assert_eq!(
                    *row.trace.borrow(),
                    (0..if replay { 8 } else { 6 }).collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn projected_buffers_move_without_new_copies_except_the_original_replay_command_clone() {
    for replay in [false, true] {
        let mut row = Row::new(replay);
        let pointers = row.text.each_ref().map(|s| s.as_ref().unwrap().as_ptr());
        if replay {
            let (command, token, key, record) = decode_replay(&mut row).unwrap();
            assert_eq!(command.as_ptr(), pointers[0]);
            assert_eq!(token.as_ptr(), pointers[1]);
            assert_eq!(key.as_ptr(), pointers[2]);
            assert_eq!(record.principal_id.as_ptr(), pointers[3]);
            assert_eq!(record.session_id.as_ptr(), pointers[5]);
            assert_ne!(record.command_id.as_str().as_ptr(), command.as_ptr());
            assert_eq!(record.command_id.as_str(), command);
        } else {
            let record = decode_record(&mut row).unwrap();
            assert_eq!(record.command_id.as_str().as_ptr(), pointers[0]);
            assert_eq!(record.principal_id.as_ptr(), pointers[1]);
            assert_eq!(record.session_id.as_ptr(), pointers[3]);
        }
    }
}

#[test]
fn lookup_kind_retains_its_distinct_command_error_precedence() {
    for replay in [false, true] {
        let mut row = Row::new(replay);
        row.text[0] = Some(String::new());
        row.failure = Some((
            1,
            NativeError::Read("token read wins only in replay".into()),
        ));
        let result = run(&mut row, replay);
        if replay {
            assert!(matches!(result, Err(NativeError::Read(_))));
            assert_eq!(*row.trace.borrow(), [0, 1]);
        } else {
            assert!(matches!(result, Err(NativeError::Invalid)));
            assert_eq!(*row.trace.borrow(), [0, 99]);
        }
        let mut row = Row::new(replay);
        row.text[0] = Some(String::new());
        assert!(matches!(run(&mut row, replay), Err(NativeError::Invalid)));
        assert_eq!(
            *row.trace.borrow(),
            if replay {
                vec![0, 1, 2, 99]
            } else {
                vec![0, 99]
            }
        );
    }
}

#[test]
fn invalid_origins_and_negative_counters_stop_at_the_original_boundary() {
    for replay in [false, true] {
        let origin = if replay { 4 } else { 2 };
        for invalid in ["GUI", " gui", "gui ", "", "unknown"] {
            let mut row = Row::new(replay);
            row.text[origin] = Some(invalid.into());
            assert!(matches!(run(&mut row, replay), Err(NativeError::Invalid)));
            let mut expected = (0..=origin).collect::<Vec<_>>();
            expected.push(99);
            assert_eq!(*row.trace.borrow(), expected);
        }
        for column in [origin + 2, origin + 3] {
            let mut row = Row::new(replay);
            row.integer[column] = -1;
            assert!(matches!(run(&mut row, replay), Err(NativeError::Invalid)));
            let mut expected = (0..=column).collect::<Vec<_>>();
            expected.push(99);
            assert_eq!(*row.trace.borrow(), expected);
        }
    }
}

#[test]
fn projection_does_not_add_identity_token_or_key_admission() {
    for replay in [false, true] {
        let mut row = Row::new(replay);
        row.text[if replay { 3 } else { 1 }] = Some("bad principal".into());
        row.text[if replay { 5 } else { 3 }] = Some(String::new());
        if replay {
            row.text[1] = Some(String::new());
            row.text[2] = Some("bad key".into());
            let (_, token, key, record) = decode_replay(&mut row).unwrap();
            assert!(token.is_empty());
            assert_eq!(key, "bad key");
            assert_eq!(record.principal_id, "bad principal");
            assert!(record.session_id.is_empty());
        } else {
            let record = decode_record(&mut row).unwrap();
            assert_eq!(record.principal_id, "bad principal");
            assert!(record.session_id.is_empty());
        }
    }
}

#[test]
fn every_native_read_fault_moves_the_original_nonclone_error_and_stops() {
    for replay in [false, true] {
        for column in 0..if replay { 8 } else { 6 } {
            let mut row = Row::new(replay);
            let message = "original native read error".to_owned();
            let pointer = message.as_ptr();
            row.failure = Some((column, NativeError::Read(message)));
            let Err(NativeError::Read(message)) = run(&mut row, replay) else {
                panic!()
            };
            assert_eq!(message.as_ptr(), pointer);
            assert_eq!(*row.trace.borrow(), (0..=column).collect::<Vec<_>>());
        }
    }
}

#[test]
fn native_read_and_invalid_mapping_unwinds_propagate_without_retry() {
    for replay in [false, true] {
        for column in (0..if replay { 8 } else { 6 }).chain([99]) {
            let mut row = Row::new(replay);
            row.unwind = Some(column);
            if column == 99 {
                row.text[0] = Some(String::new());
            }
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(&mut row, replay)))
                    .is_err()
            );
            let mut expected = if column == 99 {
                (0..if replay { 3 } else { 1 }).collect()
            } else {
                (0..=column).collect::<Vec<_>>()
            };
            if column == 99 {
                expected.push(99);
            }
            assert_eq!(*row.trace.borrow(), expected);
        }
    }
}
