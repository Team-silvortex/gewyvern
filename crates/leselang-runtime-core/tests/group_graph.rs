use std::{
    cell::{Cell, RefCell},
    convert::Infallible,
    hash::{Hash, Hasher},
    rc::Rc,
};

use leselang_runtime_core::{
    CheckpointCommitError, CheckpointRecovery, GroupGraphError, GroupGraphMember,
    GroupGraphMismatch, RecoveryPayload, RecoveryRecord, validate_group_graph,
};

struct Row {
    name: String,
    position: Option<usize>,
    key: u64,
    local: Rc<Cell<usize>>,
}

fn rows() -> Vec<Row> {
    ["first", "second", "third"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| Row {
            name: name.into(),
            position: Some(index),
            key: index as u64 + 1,
            local: Rc::new(Cell::new(0)),
        })
        .collect()
}

fn view(row: &Row) -> GroupGraphMember<'_, u64> {
    GroupGraphMember {
        name: &row.name,
        position: row.position,
        identity: &row.key,
    }
}

const DECLARED: [&str; 3] = ["first", "second", "third"];

#[test]
fn original_move_only_rows_are_borrowed_once_in_order_without_conversion() {
    let rows = rows();
    let mut seen = Vec::new();
    validate_group_graph(&99, &DECLARED, &rows, 3, |index, row| {
        assert!(std::ptr::eq(row, &rows[index]));
        row.local.set(row.local.get() + 1);
        seen.push(index);
        Ok::<_, Infallible>(view(row))
    })
    .unwrap();
    assert_eq!(seen, [0, 1, 2]);
    assert!(rows.iter().all(|row| row.local.get() == 1));
}

#[test]
fn cold_limits_and_complete_count_precede_all_native_hooks() {
    let rows = rows();
    let called = Cell::new(0);
    for (declared, actual, limit, invalid_limit) in [
        (&DECLARED[..], &rows[..], 65, true),
        (&DECLARED[..], &rows[..], 2, false),
        (&DECLARED[..], &rows[..2], 3, false),
        (&DECLARED[..2], &rows[..], 3, false),
        (&DECLARED[..], &rows[..], 0, false),
    ] {
        let error = validate_group_graph(&99, declared, actual, limit, |_, row| {
            called.set(called.get() + 1);
            Ok::<_, Infallible>(view(row))
        })
        .unwrap_err();
        assert!(if invalid_limit {
            matches!(error, GroupGraphError::InvalidLimits)
        } else {
            matches!(error, GroupGraphError::Count)
        });
    }
    assert_eq!(called.get(), 0);
    validate_group_graph(&99, &[] as &[&str], &rows[..0], 0, |_, row| {
        Ok::<_, Infallible>(view(row))
    })
    .unwrap();
}

#[test]
fn full_inclusive_ceiling_does_not_truncate_or_sort_original_members() {
    let names = (0..64)
        .map(|index| format!("member_{index}"))
        .collect::<Vec<_>>();
    let identities = (0..64).collect::<Vec<u64>>();
    let mut called = 0;
    validate_group_graph(&64, &names, &identities, 64, |index, key| {
        called += 1;
        Ok::<_, Infallible>(GroupGraphMember {
            name: &names[index],
            position: Some(index),
            identity: key,
        })
    })
    .unwrap();
    assert_eq!(called, 64);
}

#[test]
fn position_name_owner_and_duplicate_refusals_stop_at_the_original_row() {
    for mismatch in [
        GroupGraphMismatch::Position,
        GroupGraphMismatch::Name,
        GroupGraphMismatch::Owner,
        GroupGraphMismatch::Duplicate,
    ] {
        let mut rows = rows();
        match mismatch {
            GroupGraphMismatch::Position => rows[1].position = None,
            GroupGraphMismatch::Name => rows[1].name = "different".into(),
            GroupGraphMismatch::Owner => rows[1].key = 99,
            GroupGraphMismatch::Duplicate => rows[1].key = rows[0].key,
        }
        let mut called = Vec::new();
        let error = validate_group_graph(&99, &DECLARED, &rows, 3, |index, row| {
            called.push(index);
            Ok::<_, Infallible>(view(row))
        })
        .unwrap_err();
        assert!(
            matches!(error, GroupGraphError::Member { index: 1, mismatch: found } if found == mismatch)
        );
        assert_eq!(called, [0, 1]);
    }
}

#[test]
fn identity_equality_is_not_pointer_equality_and_diagnostic_order_is_closed() {
    let mut rows = rows();
    rows[1].key = rows[0].key;
    assert!(!std::ptr::eq(&rows[0].key, &rows[1].key));
    rows[1].name = "wrong".into();
    rows[1].position = Some(9);
    let error = validate_group_graph(&99, &DECLARED, &rows, 3, |_, row| {
        Ok::<_, Infallible>(view(row))
    })
    .unwrap_err();
    assert!(matches!(
        error,
        GroupGraphError::Member {
            index: 1,
            mismatch: GroupGraphMismatch::Position
        }
    ));
    rows[1].position = Some(1);
    let error = validate_group_graph(&1, &DECLARED, &rows[1..], 3, |_, row| {
        Ok::<_, Infallible>(view(row))
    })
    .unwrap_err();
    assert!(matches!(error, GroupGraphError::Count));
    let error = validate_group_graph(&99, &DECLARED, &rows, 3, |_, row| {
        Ok::<_, Infallible>(view(row))
    })
    .unwrap_err();
    assert!(matches!(
        error,
        GroupGraphError::Member {
            index: 1,
            mismatch: GroupGraphMismatch::Name
        }
    ));
}

struct PrivateError {
    original: Box<[u8]>,
    drops: Rc<Cell<usize>>,
}
impl Drop for PrivateError {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn native_error_precedes_row_mismatch_and_moves_without_formatting_private_payload() {
    let mut rows = rows();
    rows[1].name = "wrong".into();
    let drops = Rc::new(Cell::new(0));
    let original = b"private-native-error".to_vec().into_boxed_slice();
    let pointer = original.as_ptr();
    let mut failure = Some(PrivateError {
        original,
        drops: drops.clone(),
    });
    let mut called = Vec::new();
    let error = validate_group_graph(&99, &DECLARED, &rows, 3, |index, row| {
        called.push(index);
        if index == 1 {
            return Err(failure.take().unwrap());
        }
        Ok(view(row))
    })
    .unwrap_err();
    assert_eq!(called, [0, 1]);
    assert!(!format!("{error:?}: {error}").contains("private-native-error"));
    assert!(std::error::Error::source(&error).is_none());
    let GroupGraphError::Native { index: 1, error } = error else {
        panic!()
    };
    assert_eq!(error.original.as_ptr(), pointer);
    assert_eq!(drops.get(), 0);
    drop(error);
    assert_eq!(drops.get(), 1);
}

#[test]
fn native_observation_unwind_never_retries_or_consumes_borrowed_rows() {
    let rows = rows();
    let seen = RefCell::new(Vec::new());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = validate_group_graph(&99, &DECLARED, &rows, 3, |index, row| {
            seen.borrow_mut().push(index);
            if index == 1 {
                panic!("native observation");
            }
            Ok::<_, Infallible>(view(row))
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(*seen.borrow(), [0, 1]);
    assert_eq!(rows[2].key, 3);
}

#[derive(Eq, PartialEq)]
struct NativeKey {
    value: u64,
    panic_hash: bool,
}
impl Hash for NativeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        assert!(!self.panic_hash, "native key hashing");
        self.value.hash(state);
        self.panic_hash.hash(state);
    }
}

#[test]
fn native_identity_hash_unwind_stops_later_observation() {
    let keys = [
        NativeKey {
            value: 1,
            panic_hash: false,
        },
        NativeKey {
            value: 2,
            panic_hash: true,
        },
        NativeKey {
            value: 3,
            panic_hash: false,
        },
    ];
    let owner = NativeKey {
        value: 99,
        panic_hash: false,
    };
    let seen = RefCell::new(Vec::new());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = validate_group_graph(&owner, &DECLARED, &keys, 3, |index, key| {
            seen.borrow_mut().push(index);
            Ok::<_, Infallible>(GroupGraphMember {
                name: DECLARED[index],
                position: Some(index),
                identity: key,
            })
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(*seen.borrow(), [0, 1]);
}

struct PanicOwner {
    error: Option<PrivateError>,
}
impl Drop for PanicOwner {
    fn drop(&mut self) {
        panic!("hook cleanup");
    }
}

#[test]
fn callback_cleanup_unwind_releases_unpublished_native_error_exactly_once() {
    let rows = rows();
    let drops = Rc::new(Cell::new(0));
    let mut owner = PanicOwner {
        error: Some(PrivateError {
            original: b"opaque".to_vec().into_boxed_slice(),
            drops: drops.clone(),
        }),
    };
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = validate_group_graph(&99, &DECLARED, &rows, 3, move |_, _| {
            let _borrow_complete_owner = &mut owner;
            Err(owner.error.take().unwrap())
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(drops.get(), 1);
    assert_eq!(rows.len(), 3);
}

struct SavedRows {
    owner: u64,
    declared: Vec<String>,
    rows: Vec<Row>,
    dropped: Rc<Cell<usize>>,
}
impl Drop for SavedRows {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

#[test]
fn complete_native_graph_gate_precedes_checkpoint_commit_and_snapshot_handoff() {
    let dropped = Rc::new(Cell::new(0));
    let commits = Cell::new(0);
    for corrupt in [true, false] {
        let mut group = SavedRows {
            owner: 99,
            declared: DECLARED.map(str::to_owned).into(),
            rows: rows(),
            dropped: dropped.clone(),
        };
        if corrupt {
            group.rows[2].key = group.rows[0].key;
        }
        let mut recovery = CheckpointRecovery::<u64, SavedRows, ()>::new(1, 10);
        recovery
            .push(RecoveryRecord {
                key: 99,
                sequence: 7,
                payload: RecoveryPayload::Pending(group),
            })
            .unwrap();
        let result = recovery.finish((), |_, snapshot| {
            let group = &snapshot.pending()[&99];
            validate_group_graph(&group.owner, &group.declared, &group.rows, 3, |_, row| {
                Ok::<_, Infallible>(view(row))
            })?;
            commits.set(commits.get() + 1);
            Ok(())
        });
        if corrupt {
            assert!(matches!(
                result,
                Err(CheckpointCommitError::Host(GroupGraphError::Member {
                    index: 2,
                    mismatch: GroupGraphMismatch::Duplicate
                }))
            ));
            assert_eq!(commits.get(), 0);
            assert_eq!(dropped.get(), 1);
        } else {
            let (snapshot, ()) = result.unwrap();
            assert_eq!(snapshot.highest_sequence(), 7);
            assert_eq!(snapshot.pending()[&99].rows.len(), 3);
            assert_eq!(commits.get(), 1);
            drop(snapshot);
            assert_eq!(dropped.get(), 2);
        }
    }
}

struct PanelRow {
    label: String,
    ordinal: i64,
    native_id: [u8; 16],
    widget: Rc<Cell<bool>>,
}

#[test]
fn unrelated_gui_native_identity_and_storage_position_schema_need_no_product_conversion() {
    let rows = [
        PanelRow {
            label: "window".into(),
            ordinal: 0,
            native_id: [1; 16],
            widget: Rc::new(Cell::new(true)),
        },
        PanelRow {
            label: "button".into(),
            ordinal: 1,
            native_id: [2; 16],
            widget: Rc::new(Cell::new(true)),
        },
    ];
    // Numeric counter identities above and these GUI IDs share only the protocol.
    validate_group_graph(&[99; 16], &["window", "button"], &rows, 2, |_, row| {
        assert!(row.widget.get());
        Ok::<_, Infallible>(GroupGraphMember {
            name: &row.label,
            position: usize::try_from(row.ordinal).ok(),
            identity: &row.native_id,
        })
    })
    .unwrap();
    rows[1].widget.set(false);
    assert!(!rows[1].widget.get()); // Structural success did not freeze current GUI policy.
}
