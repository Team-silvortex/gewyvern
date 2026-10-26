use super::*;

fn invoke(
    vm: &mut Vm,
    mode: usize,
    lease: &DispatchLease,
    now: u64,
    result: EffectResult,
    frozen: bool,
) -> Step {
    let authority = (mode == 2).then_some((lease, now));
    if frozen {
        frozen_complete_result(vm, &lease.request.continuation, authority, result)
    } else {
        vm.complete_result(&lease.request.continuation, authority, result)
    }
}

fn assert_state(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(12),
        expected.scheduler_pressure(12)
    );
}

#[test]
fn result_completion_matches_frozen_vm_across_operations_bindings_fuel_and_fences() {
    let mut comparisons = 0;
    let mut baseline_completions = 0;
    let mut baseline_refusals = 0;
    for (original, original_reply) in crate::reply_contract_tests::cases() {
        for kind in 0..5 {
            for mode in [1, 2] {
                for defect in 0..8 {
                    let mut request = original.clone();
                    let capabilities = CapabilitySet::new([
                        CAPABILITY_UI_PRESENTATION,
                        CAPABILITY_RUNTIME_READ,
                        CAPABILITY_RUNTIME_REFRESH,
                        CAPABILITY_RUNTIME_DEPLOY,
                        CAPABILITY_DEBUGGER_CONTROL,
                    ]);
                    match &mut request.operation {
                        EffectOperation::Query(envelope) => envelope.capabilities = capabilities,
                        EffectOperation::Command(envelope) => envelope.capabilities = capabilities,
                        EffectOperation::Presentation(envelope) => {
                            envelope.capabilities = capabilities
                        }
                    }
                    request.continuation.result_binding =
                        crate::group_start_fixture::binding(if kind == 4 { 5 } else { kind });
                    if let Some(binding) = &request.continuation.result_binding {
                        request.continuation.schema_version = binding.schema_version();
                    }
                    if matches!(defect, 1 | 2) {
                        request.continuation.fuel_remaining = defect as u64;
                    }
                    request.budget = request.continuation.resource_budget();
                    let mut actual = Vm::new(1);
                    let mut expected = Vm::new(1);
                    actual
                        .restore_request(request.clone())
                        .unwrap_or_else(|error| {
                            panic!(
                                "kind={kind}, mode={mode}, defect={defect}, type={:?}: {error:?}",
                                original.continuation.result_type
                            )
                        });
                    expected.restore_request(request.clone()).unwrap();
                    let mut lease = if mode == 2 {
                        let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                        assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                        lease
                    } else {
                        DispatchLease {
                            request,
                            attempt: 0,
                            retry_count: 0,
                            lease_expires_at_ms: 0,
                        }
                    };
                    let mut result = original_reply.clone();
                    match defect {
                        3 => {
                            result = EffectResult::Presentation(PresentationResult::Focus {
                                node_id: "wrong".into(),
                            })
                        }
                        4 => {
                            result = EffectResult::Presentation(PresentationResult::Focus {
                                node_id: "x".repeat(MAX_CONTINUATION_BYTES),
                            })
                        }
                        5 => lease.attempt += 1,
                        6 => {
                            actual.next_effect_id = u64::MAX;
                            expected.next_effect_id = u64::MAX;
                        }
                        7 => {
                            assert_eq!(
                                invoke(&mut actual, mode, &lease, 2, result.clone(), true),
                                invoke(&mut expected, mode, &lease, 2, result.clone(), true)
                            );
                        }
                        _ => {}
                    }
                    let before = serde_json::to_vec(&lease).unwrap();
                    let want = invoke(&mut expected, mode, &lease, 12, result.clone(), true);
                    let got = invoke(&mut actual, mode, &lease, 12, result, false);
                    assert_eq!(
                        got, want,
                        "kind={kind}, mode={mode}, defect={defect}, type={:?}",
                        original.continuation.result_type
                    );
                    assert_eq!(
                        serde_json::to_vec(&got).unwrap(),
                        serde_json::to_vec(&want).unwrap()
                    );
                    assert_eq!(serde_json::to_vec(&lease).unwrap(), before);
                    if defect == 0 {
                        if mode == 1
                            && matches!(
                                original.continuation.pending_effect,
                                Effect::RuntimeRefresh { .. }
                                    | Effect::RuntimeCapabilitiesRefresh { .. }
                                    | Effect::RuntimeDeploy { .. }
                                    | Effect::DebuggerCancel { .. }
                            )
                        {
                            assert!(
                                matches!(got, Step::Fault(ref error) if error.code == "LSV2103")
                            );
                            baseline_refusals += 1;
                        } else {
                            assert!(
                                !matches!(got, Step::Fault(_)),
                                "kind={kind}, mode={mode}, type={:?}: {got:?}",
                                original.continuation.result_type
                            );
                            baseline_completions += 1;
                        }
                    }
                    assert_state(&actual, &expected);
                    assert_eq!(actual.claim_effect(13, 100), expected.claim_effect(13, 100));
                    assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 5600);
    assert_eq!(baseline_completions, 680);
    assert_eq!(baseline_refusals, 20);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn result_completion_sqlite_matches_frozen_failures_peer_replay_restart_and_successor_drain() {
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
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
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
    fn reply(lease: &DispatchLease) -> EffectResult {
        let EffectOperation::Presentation(envelope) = &lease.request.operation else {
            panic!()
        };
        EffectResult::Presentation(
            serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
        )
    }
    let mut comparisons = 0;
    for source in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: 7)"#,
        r#"bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: "b"), body: 7))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(s: ui.focus(node_id: "c"), body: 7))"#,
    ] {
        let program =
            leserpent_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}")))
                .unwrap();
        for mode in [1, 2] {
            for defect in 0..4 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-result-completion-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual = Vm::open_journal(&actual_path, 100).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 100).unwrap();
                let start = |vm: &mut Vm| {
                    vm.start(
                        &program,
                        Principal::new("operator").unwrap(),
                        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                        None,
                    )
                };
                let started = start(&mut actual);
                assert_eq!(started, start(&mut expected));
                let mut peer_actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut peer_expected = Vm::open_journal(&expected_path, 1).unwrap();
                let lease = if mode == 2 {
                    let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                    assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                    lease
                } else {
                    let request = match started {
                        Step::Effect(request) => *request,
                        Step::Effects(mut batch) => batch.branches.remove(0).request,
                        _ => panic!(),
                    };
                    DispatchLease {
                        request,
                        attempt: 0,
                        retry_count: 0,
                        lease_expires_at_ms: 0,
                    }
                };
                if defect > 0 {
                    let (table, action) = match defect {
                        1 => ("vm_effects", "UPDATE"),
                        2 => ("vm_dispatches", "UPDATE"),
                        _ => ("vm_effects", "INSERT"),
                    };
                    for path in [&actual_path, &expected_path] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_reply BEFORE {action} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                let want = invoke(&mut expected, mode, &lease, 2, reply(&lease), true);
                let got = invoke(&mut actual, mode, &lease, 2, reply(&lease), false);
                assert_eq!(got, want, "{source}, mode={mode}, defect={defect}");
                if defect == 0 {
                    assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                if defect == 0 {
                    assert_eq!(
                        peer_actual.replay_completed(&lease.request.continuation.token),
                        peer_expected.replay_completed(&lease.request.continuation.token)
                    );
                    assert_state(&peer_actual, &peer_expected);
                }
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_reply")
                        .unwrap();
                }
                drop((actual, expected, peer_actual, peer_expected));
                let mut actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 1).unwrap();
                assert_state(&actual, &expected);
                let mut drained = 0;
                for now in [200, 202, 204, 206, 208] {
                    let lease = actual.claim_effect(now, 100).unwrap();
                    assert_eq!(lease, expected.claim_effect(now, 100).unwrap());
                    let Some(lease) = lease else { break };
                    let got = invoke(&mut actual, 2, &lease, now + 1, reply(&lease), false);
                    let want = invoke(&mut expected, 2, &lease, now + 1, reply(&lease), true);
                    assert_eq!(got, want);
                    assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                    drained += 1;
                }
                assert!(drained < 5);
                assert_eq!(actual.pending_count(), 0);
                assert_state(&actual, &expected);
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 64);
}

// Frozen result coordinator and materialization before product-policy extraction.
enum PreparedResult {
    Scalar(ScalarValue),
    Host {
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    },
}

fn materialize(image: &ContinuationImage, value: &Value) -> Result<PreparedResult, Fault> {
    let binding = image.result_binding.as_ref().ok_or_else(invalid)?;
    let mut fuel = Fuel::new(image.fuel_remaining);
    let operation = leserpent_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .ok_or_else(invalid)?;
    let (effect, binding) = match computation::resume_outcome(binding, value, operation, &mut fuel)?
    {
        computation::Outcome::Scalar(value) => return Ok(PreparedResult::Scalar(value)),
        computation::Outcome::Host(effect) => (effect, None),
        computation::Outcome::BoundHost { effect, binding } => (effect, Some(binding)),
        computation::Outcome::Result => return Err(invalid()),
    };
    if fuel.remaining() == 0 {
        return Err(Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before successor".into(),
        });
    }
    if leserpent_hir::host_call::HostOperation::for_effect(&effect).is_none() {
        return Err(invalid());
    }
    Ok(PreparedResult::Host {
        effect: effect.into_owned(),
        binding,
        fuel: fuel.remaining(),
    })
}

fn frozen_complete_result(
    vm: &mut Vm,
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    result: EffectResult,
) -> Step {
    use crate::result_conversion_contract_tests::frozen::{
        atomic_step_from_effect_result, step_from_effect_result,
    };
    let operation = lease.map(|(lease, _)| &lease.request.operation);
    let step = if matches!(
        image.schema_version,
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION
            | DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        let raw = atomic_step_from_effect_result(image, operation, result);
        if let Step::Done(value) = raw {
            match vm
                .journal
                .validate_result_output(image, &value)
                .and_then(|()| materialize(image, &value))
            {
                Ok(PreparedResult::Host {
                    effect,
                    binding,
                    fuel,
                }) => {
                    return super::assembly_contract_tests::frozen_complete_successor(
                        vm, image, lease, value, effect, binding, fuel,
                    );
                }
                Ok(PreparedResult::Scalar(value)) => Step::Done(Value::Scalar { value }),
                Err(error) => Step::Fault(error),
            }
        } else {
            raw
        }
    } else if matches!(
        image.schema_version,
        GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
            | PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
            | GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        atomic_step_from_effect_result(image, operation, result)
    } else {
        step_from_effect_result(image, operation, result)
    };
    super::settlement_contract_tests::frozen_record_result(vm, image, lease, step)
}
