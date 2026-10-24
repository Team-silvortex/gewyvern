use super::*;

use crate::group_start_fixture as fixture;

fn import(vm: &mut Vm, request: EffectRequest, full: bool, frozen: bool) -> Result<(), Fault> {
    match (full, frozen) {
        (true, true) => frozen_restore_request(vm, request),
        (true, false) => vm.restore_request(request),
        (false, true) => frozen_restore(vm, request.continuation),
        (false, false) => vm.restore(request.continuation),
    }
}

fn at(mut request: EffectRequest, sequence: u64) -> EffectRequest {
    request.effect_id = format!("effect-{sequence}");
    request.continuation.token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
    request
}

fn assert_state(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(actual.scheduler_pressure(1), expected.scheduler_pressure(1));
}

#[test]
fn restore_matches_frozen_whole_vm_import_for_seventy_operations_and_competing_faults() {
    let requests = crate::continuation_contract_tests::requests();
    let seed = at(
        requests
            .iter()
            .find(|r| r.continuation.result_type == Type::UiFocus)
            .unwrap()
            .clone(),
        999,
    );
    let mut comparisons = 0;
    for original in requests {
        for full in [false, true] {
            for defect in 0..16 {
                let mut request = original.clone();
                let mut actual = Vm::new(1);
                let mut expected = Vm::new(1);
                match defect {
                    1 => {
                        import(&mut actual, request.clone(), full, false).unwrap();
                        import(&mut expected, request.clone(), full, true).unwrap();
                    }
                    2 | 4 | 5 => {
                        let mut saved = request.continuation.clone();
                        if defect != 4 {
                            saved.fuel_remaining -= 1;
                        }
                        actual.pending.insert(saved.token.clone(), saved.clone());
                        expected.pending.insert(saved.token.clone(), saved);
                    }
                    6 => request.continuation.schema_version = 0,
                    7 => {
                        request.continuation.token =
                            ContinuationToken::from_wire("continuation-01".into())
                    }
                    8 => {
                        request.continuation.group_result =
                            Some(ContinuationToken::from_wire("merge-1".into()))
                    }
                    9 => {
                        request.continuation.token =
                            ContinuationToken::from_wire("x".repeat(MAX_CONTINUATION_BYTES))
                    }
                    10 => request.budget.fuel_remaining += 1,
                    11 => {
                        for (vm, frozen) in [(&mut actual, false), (&mut expected, true)] {
                            import(vm, seed.clone(), true, frozen).unwrap();
                            vm.set_scheduler_limits(SchedulerLimits {
                                max_pending_dispatches: 1,
                                ..SchedulerLimits::default()
                            })
                            .unwrap();
                        }
                    }
                    12 => {
                        actual.next_effect_id = 1000;
                        expected.next_effect_id = 1000;
                    }
                    13 => request = at(request, MAX_EFFECT_SEQUENCE),
                    14 => request.effect_id.clear(),
                    15 => request.continuation.deadline_ms = 0,
                    _ => {}
                }
                if matches!(defect, 3..=5) {
                    for vm in [&mut actual, &mut expected] {
                        vm.completed.insert(
                            request.continuation.token.clone(),
                            Step::Done(Value::Scalar {
                                value: ScalarValue::None,
                            }),
                        );
                    }
                }
                let bytes = serde_json::to_vec(&request).unwrap();
                let want = import(&mut expected, request.clone(), full, true);
                let got = import(&mut actual, request.clone(), full, false);
                assert_eq!(got, want, "full={full}, defect={defect}");
                if defect == 0 {
                    assert!(got.is_ok(), "{got:?}");
                }
                assert_eq!(
                    serde_json::to_vec(&got).unwrap(),
                    serde_json::to_vec(&want).unwrap()
                );
                assert_state(&actual, &expected);
                assert_eq!(actual.claim_effect(1, 100), expected.claim_effect(1, 100));
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(serde_json::to_vec(&request).unwrap(), bytes);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 2240);
}

fn schema_requests() -> Vec<EffectRequest> {
    let base = crate::continuation_contract_tests::requests()
        .into_iter()
        .find(|r| r.continuation.result_type == Type::UiFocus)
        .unwrap();
    let mut requests = Vec::new();
    for kind in [0, 1, 2, 3, 5] {
        let mut request = base.clone();
        request.continuation.result_binding = fixture::binding(kind);
        if let Some(binding) = &request.continuation.result_binding {
            request.continuation.schema_version =
                leselang_command::continuation_schema::preflighted_result_binding_schema_version(
                    binding,
                );
        }
        requests.push(request);
    }
    for version in 6..=11 {
        let mut request = base.clone();
        request.continuation.schema_version = version;
        request.continuation.group_result = Some(ContinuationToken::from_wire("merge-1".into()));
        requests.push(request);
    }
    requests
}

#[test]
fn restore_keeps_all_eleven_schemas_original_authority_and_group_refusals() {
    let mut seen = std::collections::BTreeSet::new();
    let mut comparisons = 0;
    for request in schema_requests() {
        let version = request.continuation.schema_version;
        seen.insert(version);
        validate_effect_request(&request).unwrap();
        for full in [false, true] {
            for state in 0..3 {
                let mut actual = Vm::new(1);
                let mut expected = Vm::new(1);
                for vm in [&mut actual, &mut expected] {
                    if state == 1 {
                        vm.pending.insert(
                            request.continuation.token.clone(),
                            request.continuation.clone(),
                        );
                    }
                    if state == 2 {
                        vm.completed.insert(
                            request.continuation.token.clone(),
                            Step::Done(Value::Scalar {
                                value: ScalarValue::None,
                            }),
                        );
                    }
                }
                let want = import(&mut expected, request.clone(), full, true);
                let got = import(&mut actual, request.clone(), full, false);
                assert_eq!(got, want, "schema={version}, full={full}, state={state}");
                if version >= 6 {
                    assert_eq!(got.unwrap_err().code, "LSV1409");
                } else if version >= 3 && !full {
                    assert_eq!(got.unwrap_err().code, "LSV1407");
                }
                assert_state(&actual, &expected);
                comparisons += 1;
            }
        }
    }
    assert_eq!(seen, (1..=11).collect());
    assert_eq!(comparisons, 66);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn restore_sqlite_matches_frozen_duplicate_authority_watermark_rollback_and_restart() {
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &std::path::Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap()
            .query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
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
    let request = at(schema_requests().remove(1), 20);
    let mut comparisons = 0;
    for full in [false, true] {
        for prior in 0..3 {
            for defect in 0..4 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-restore-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 1).unwrap();
                if prior > 0 {
                    import(&mut actual, request.clone(), prior == 2, false).unwrap();
                    import(&mut expected, request.clone(), prior == 2, true).unwrap();
                }
                if defect > 0 {
                    let (table, action) = match defect {
                        1 => ("vm_effects", "INSERT"),
                        2 => ("vm_dispatches", "INSERT"),
                        _ => ("vm_metadata", "UPDATE"),
                    };
                    for path in [&actual_path, &expected_path] {
                        let db = Connection::open(path).unwrap();
                        if defect == 3 {
                            db.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                                .unwrap();
                        }
                        db.execute_batch(&format!("CREATE TRIGGER reject_import BEFORE {action} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                let before = snapshot(&actual_path);
                let pending = actual.pending.clone();
                let watermark = actual.next_effect_id;
                let want = import(&mut expected, request.clone(), full, true);
                let got = import(&mut actual, request.clone(), full, false);
                assert_eq!(got, want, "full={full}, prior={prior}, defect={defect}");
                if full && prior == 1 {
                    assert_eq!(got.as_ref().unwrap_err().code, "LSV4011");
                } else if defect == 0 {
                    assert!(got.is_ok(), "full={full}, prior={prior}: {got:?}");
                }
                if got.is_err() {
                    assert_eq!(snapshot(&actual_path), before);
                    assert_eq!(actual.pending, pending);
                    assert_eq!(actual.next_effect_id, watermark);
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_import")
                        .unwrap();
                }
                drop(actual);
                drop(expected);
                let mut actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 1).unwrap();
                assert_state(&actual, &expected);
                let lease = actual.claim_effect(1, 100).unwrap();
                assert_eq!(lease, expected.claim_effect(1, 100).unwrap());
                if let Some(lease) = lease {
                    let EffectOperation::Presentation(envelope) = &lease.request.operation else {
                        panic!()
                    };
                    let result = EffectResult::Presentation(
                        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap())
                            .unwrap(),
                    );
                    let step = actual.acknowledge_effect(&lease, 2, result.clone());
                    assert_eq!(step, expected.acknowledge_effect(&lease, 2, result));
                    assert!(matches!(step, Step::Done(_)), "{step:?}");
                }
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 24);
}

// Frozen complete import entries before adapter extraction.
fn frozen_restore(vm: &mut Vm, image: ContinuationImage) -> Result<(), Fault> {
    frozen_restore_inner(vm, image, None)
}

/// Restores a continuation together with its original host-owned authority envelope.
fn frozen_restore_request(vm: &mut Vm, request: EffectRequest) -> Result<(), Fault> {
    validate_effect_request(&request)?;
    frozen_restore_inner(vm, request.continuation.clone(), Some(&request))
}

fn frozen_restore_inner(
    vm: &mut Vm,
    image: ContinuationImage,
    request: Option<&EffectRequest>,
) -> Result<(), Fault> {
    validate_continuation_size(&image)?;
    if image.group_result.is_some() {
        return Err(group_binding::invalid());
    }
    if matches!(
        image.schema_version,
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION
            | DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) && request.is_none()
    {
        return Err(successor::invalid());
    }
    if let Some(current) = vm.pending.get(&image.token) {
        return if current == &image {
            vm.journal
                .record_pending(&image, request, vm.scheduler_limits)?;
            Ok(())
        } else {
            Err(Fault {
                code: "LSV2002".to_string(),
                message: "continuation token conflicts with pending state".to_string(),
            })
        };
    }
    if vm.completed.contains_key(&image.token) {
        return Err(Fault {
            code: "LSV2003".to_string(),
            message: "continuation was already consumed".to_string(),
        });
    }
    let sequence = canonical_continuation_sequence(&image.token)
        .ok_or_else(invalid_continuation_sequence_fault)?;
    let next_sequence = SequenceWatermarkRequirement::after_observation(sequence, u64::MAX)
        .map_err(|_| effect_sequence_exhausted_fault())?
        .required_watermark();
    vm.journal
        .record_pending(&image, request, vm.scheduler_limits)?;
    vm.next_effect_id = vm.next_effect_id.max(next_sequence);
    vm.pending.insert(image.token.clone(), image);
    Ok(())
}
