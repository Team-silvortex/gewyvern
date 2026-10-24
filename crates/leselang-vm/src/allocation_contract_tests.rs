use super::*;

fn occupy(vm: &mut Vm, sequence: u64, pending: bool) {
    let token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
    if pending {
        let mut image = crate::continuation_contract_tests::requests()
            .remove(0)
            .continuation;
        image.token = token.clone();
        vm.pending.insert(token, image);
    } else {
        vm.completed.insert(
            token,
            Step::Fault(Fault {
                code: "fixture".into(),
                message: "original cached result".into(),
            }),
        );
    }
}

fn seed(vm: &mut Vm, base: u64, mask: u8) {
    for offset in 0..2 {
        if mask & (1 << offset) != 0 {
            occupy(vm, base.saturating_add(offset), true);
        }
        if mask & (1 << (offset + 2)) != 0 {
            occupy(vm, base.saturating_add(offset), false);
        }
    }
    // Noncanonical spellings and group identities are not the generated token.
    for name in ["continuation-01", "merge-1", "opaque"] {
        vm.completed.insert(
            ContinuationToken::from_wire(name.into()),
            Step::Done(Value::Scalar {
                value: ScalarValue::Integer(7),
            }),
        );
    }
}

fn compare(actual: &mut Vm, expected: &mut Vm) -> Result<u64, Fault> {
    let before = (actual.pending.clone(), actual.completed.clone());
    let want = frozen_allocate_sequence(expected);
    let got = actual.allocate_sequence();
    assert_eq!(got, want);
    assert_eq!(
        serde_json::to_vec(&got).unwrap(),
        serde_json::to_vec(&want).unwrap()
    );
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!((actual.pending.clone(), actual.completed.clone()), before);
    got
}

#[test]
fn allocation_matches_frozen_memory_boundaries_and_both_cache_masks() {
    let mut comparisons = 0;
    for local in [
        0,
        1,
        2,
        MAX_EFFECT_SEQUENCE - 2,
        MAX_EFFECT_SEQUENCE,
        MAX_EFFECT_SEQUENCE + 1,
        u64::MAX,
    ] {
        for mask in 0..16 {
            let mut actual = Vm::new(0);
            let mut expected = Vm::new(0);
            for vm in [&mut actual, &mut expected] {
                vm.next_effect_id = local;
                seed(vm, local, mask);
            }
            let got = compare(&mut actual, &mut expected);
            if local == 0 || local > MAX_EFFECT_SEQUENCE {
                assert_eq!(got.unwrap_err().code, "LSV1003");
            } else if local < MAX_EFFECT_SEQUENCE {
                let expected_free = if mask & 1 == 0 && mask & 4 == 0 {
                    local
                } else if mask & 2 == 0 && mask & 8 == 0 {
                    local + 1
                } else {
                    local + 2
                };
                assert_eq!(got.unwrap(), expected_free);
            }
            let _ = compare(&mut actual, &mut expected);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 112);
}

#[test]
fn allocation_matches_frozen_long_collision_runs_and_does_not_spin_at_the_ceiling() {
    for near_limit in [false, true] {
        let start = if near_limit {
            MAX_EFFECT_SEQUENCE - 1023
        } else {
            1
        };
        let mut actual = Vm::new(0);
        let mut expected = Vm::new(0);
        for vm in [&mut actual, &mut expected] {
            vm.next_effect_id = start;
            for offset in 0..1024 {
                occupy(vm, start + offset, offset % 2 == 0);
            }
        }
        let got = compare(&mut actual, &mut expected);
        if near_limit {
            assert_eq!(got.unwrap_err().code, "LSV1003");
            assert_eq!(actual.next_effect_id, MAX_EFFECT_SEQUENCE + 1);
        } else {
            assert_eq!(got, Ok(1025));
            assert_eq!(actual.next_effect_id, 1026);
        }
    }
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::*;
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Scratch {
        fn new() -> Self {
            let dir = std::env::temp_dir().join(format!(
                "leselang-allocation-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            Self(dir)
        }
        fn db(&self, name: &str) -> std::path::PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn set_stored(path: &Path, stored: i64) {
        let db = Connection::open(path).unwrap();
        db.pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        db.execute("UPDATE vm_metadata SET next_sequence=?1", [stored])
            .unwrap();
    }
    fn stored(path: &Path) -> i64 {
        Connection::open(path)
            .unwrap()
            .query_row("SELECT next_sequence FROM vm_metadata", [], |row| {
                row.get(0)
            })
            .unwrap()
    }
    fn snapshot(path: &Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        tables
            .into_iter()
            .map(|table| {
                let mut stmt = db
                    .prepare(&format!(
                        "SELECT * FROM \"{}\" ORDER BY rowid",
                        table.replace('"', "\"\"")
                    ))
                    .unwrap();
                let columns = stmt.column_count();
                let rows = stmt
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<Result<Vec<SqlValue>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }

    #[test]
    fn allocation_sqlite_matches_frozen_native_frontiers_collisions_and_restart() {
        let mut scenarios = 0;
        for local in [
            0,
            1,
            9,
            MAX_EFFECT_SEQUENCE,
            MAX_EFFECT_SEQUENCE + 1,
            u64::MAX,
        ] {
            for frontier in [1, 5, MAX_EFFECT_SEQUENCE, MAX_EFFECT_SEQUENCE + 1] {
                for mask in [0, 1, 4, 9] {
                    let scratch = Scratch::new();
                    let a = scratch.db("actual.sqlite3");
                    let e = scratch.db("expected.sqlite3");
                    let mut actual = Vm::open_journal(&a, 0).unwrap();
                    let mut expected = Vm::open_journal(&e, 0).unwrap();
                    for (vm, path) in [(&mut actual, &a), (&mut expected, &e)] {
                        vm.next_effect_id = local;
                        set_stored(path, frontier as i64);
                        seed(vm, local.max(frontier), mask);
                    }
                    let before = snapshot(&a);
                    let got = compare(&mut actual, &mut expected);
                    assert_eq!(snapshot(&a), snapshot(&e));
                    if local > MAX_EFFECT_SEQUENCE || frontier > MAX_EFFECT_SEQUENCE {
                        assert_eq!(got.unwrap_err().code, "LSV4009");
                        assert_eq!(actual.next_effect_id, local);
                        assert_eq!(snapshot(&a), before);
                    } else if let Ok(sequence) = got {
                        assert_eq!(stored(&a) as u64, sequence + 1);
                        assert!(sequence >= local.max(frontier));
                    } else {
                        assert_eq!(got.unwrap_err().code, "LSV4009");
                        assert_eq!(local.max(frontier), MAX_EFFECT_SEQUENCE);
                        assert_eq!(stored(&a) as u64, MAX_EFFECT_SEQUENCE + 1);
                        assert_eq!(actual.next_effect_id, MAX_EFFECT_SEQUENCE + 1);
                    }
                    drop((actual, expected));
                    let mut actual = Vm::open_journal(&a, 0).unwrap();
                    let mut expected = Vm::open_journal(&e, 0).unwrap();
                    let _ = compare(&mut actual, &mut expected);
                    assert_eq!(snapshot(&a), snapshot(&e));
                    scenarios += 1;
                }
            }
        }
        assert_eq!(scenarios, 96);
    }

    #[test]
    fn allocation_sqlite_keeps_read_write_commit_faults_and_consumed_reservations_distinct() {
        for defect in 0..6 {
            let scratch = Scratch::new();
            let a = scratch.db("actual.sqlite3");
            let e = scratch.db("expected.sqlite3");
            let mut actual = Vm::open_journal(&a, 0).unwrap();
            let mut expected = Vm::open_journal(&e, 0).unwrap();
            for (vm, path) in [(&mut actual, &a), (&mut expected, &e)] {
                let db = Connection::open(path).unwrap();
                match defect {
                    0 => set_stored(path, -1),
                    1 => {
                        set_stored(path, 0);
                        vm.next_effect_id = 0;
                    }
                    2 => {
                        db.execute("DELETE FROM vm_metadata", []).unwrap();
                    }
                    3 | 4 => {
                        if defect == 4 {
                            occupy(vm, 1, true);
                        }
                        let threshold = if defect == 4 { 3 } else { 2 };
                        db.execute_batch(&format!("CREATE TRIGGER refuse_sequence BEFORE UPDATE ON vm_metadata WHEN NEW.next_sequence >= {threshold} BEGIN SELECT RAISE(ABORT, 'injected sequence write'); END;")).unwrap();
                    }
                    _ => {
                        db.execute_batch("CREATE TABLE test_parent (id INTEGER PRIMARY KEY); CREATE TABLE test_guard (id INTEGER REFERENCES test_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER fail_commit AFTER UPDATE ON vm_metadata BEGIN INSERT INTO test_guard VALUES (1); END;").unwrap();
                    }
                }
            }
            let before = snapshot(&a);
            let error = compare(&mut actual, &mut expected).unwrap_err();
            assert_eq!(snapshot(&a), snapshot(&e));
            assert_eq!(
                error.code,
                match defect {
                    0 => "LSV4007",
                    1 => "LSV1003",
                    _ => "LSV4008",
                }
            );
            if defect == 1 {
                assert_eq!(stored(&a), 1);
                assert_eq!(actual.next_effect_id, 0);
            } else if defect == 4 {
                assert_eq!(stored(&a), 2);
                assert_eq!(actual.next_effect_id, 2);
                assert!(
                    actual
                        .pending
                        .contains_key(&ContinuationToken::from_wire("continuation-1".into()))
                );
            } else {
                assert_eq!(snapshot(&a), before);
                assert_eq!(actual.next_effect_id, 1);
            }
            if defect == 5 {
                assert!(error.message.contains("failed to commit sequence"));
            }
            for path in [&a, &e] {
                let db = Connection::open(path).unwrap();
                db.execute_batch(
                    "DROP TRIGGER IF EXISTS refuse_sequence; DROP TRIGGER IF EXISTS fail_commit;",
                )
                .unwrap();
                if defect == 0 {
                    set_stored(path, 1);
                }
                if defect == 2 {
                    db.execute("INSERT INTO vm_metadata VALUES (1,1)", [])
                        .unwrap();
                }
            }
            drop((actual, expected));
            let mut actual = Vm::open_journal(&a, 0).unwrap();
            let mut expected = Vm::open_journal(&e, 0).unwrap();
            assert!(compare(&mut actual, &mut expected).is_ok());
            assert_eq!(snapshot(&a), snapshot(&e));
        }
    }

    #[test]
    fn allocation_four_shared_workers_keep_unique_reservations_without_effect_publication() {
        for frozen in [false, true] {
            let scratch = Scratch::new();
            let path = scratch.db("shared.sqlite3");
            let workers = (0..4)
                .map(|_| Vm::open_journal(&path, 0).unwrap())
                .collect::<Vec<_>>();
            let barrier = std::sync::Barrier::new(4);
            let mut allocated = std::thread::scope(|scope| {
                let handles = workers
                    .into_iter()
                    .map(|mut vm| {
                        let barrier = &barrier;
                        scope.spawn(move || {
                            barrier.wait();
                            let values = (0..16)
                                .map(|_| {
                                    if frozen {
                                        frozen_allocate_sequence(&mut vm).unwrap()
                                    } else {
                                        vm.allocate_sequence().unwrap()
                                    }
                                })
                                .collect::<Vec<_>>();
                            assert!(vm.pending.is_empty() && vm.completed.is_empty());
                            values
                        })
                    })
                    .collect::<Vec<_>>();
                handles
                    .into_iter()
                    .flat_map(|h| h.join().unwrap())
                    .collect::<Vec<_>>()
            });
            allocated.sort_unstable();
            assert_eq!(allocated, (1..=64).collect::<Vec<_>>());
            assert_eq!(stored(&path), 65);
            let mut reopened = Vm::open_journal(&path, 0).unwrap();
            assert_eq!(reopened.allocate_sequence().unwrap(), 65);
            assert!(reopened.claim_effect(1, 100).unwrap().is_none());
            assert!(reopened.pending.is_empty() && reopened.completed.is_empty());
        }
    }
}

// Frozen pre-extraction selection; native journal transactions remain shared.
fn frozen_allocate_sequence(vm: &mut Vm) -> Result<u64, Fault> {
    loop {
        let sequence = vm.journal.allocate_sequence(vm.next_effect_id)?;
        if sequence == 0 || sequence > MAX_EFFECT_SEQUENCE {
            return Err(effect_sequence_exhausted_fault());
        }
        vm.next_effect_id = SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
            .map_err(|_| effect_sequence_exhausted_fault())?
            .required_watermark();
        let token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
        if !vm.pending.contains_key(&token) && !vm.completed.contains_key(&token) {
            return Ok(sequence);
        }
    }
}
