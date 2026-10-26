use super::settlement_contract_tests::frozen_record_result;
use super::*;

fn program(effect: Effect, kind: usize) -> HirProgram {
    use leselang_hir::computation::Computation;
    let scalar = || Computation::Literal {
        value: ScalarValue::Integer(7),
    };
    let host = Computation::Host {
        effect: Box::new(effect),
    };
    let bind = |name: &str, value, body| Computation::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    };
    let tail = || {
        bind(
            "tail_result",
            Computation::Host {
                effect: Box::new(Effect::UiFocus {
                    node_id: "tail".into(),
                }),
            },
            scalar(),
        )
    };
    let body = match kind {
        0 => host,
        1 => bind("child", host, scalar()),
        2 => bind("child", host, tail()),
        _ => Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(bind("child", host, tail())),
            otherwise: Box::new(scalar()),
        },
    };
    let effect = Effect::Compute {
        expression: Box::new(Computation::Bind {
            name: "parent".into(),
            value: Box::new(Computation::Host {
                effect: Box::new(Effect::UiFocus {
                    node_id: "parent".into(),
                }),
            }),
            body: Box::new(body),
        }),
    };
    leselang_hir::lower(&leselang_syntax::parse(&canonical_source(&effect).unwrap())).unwrap()
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
fn successor_assembly_matches_frozen_vm_for_seventy_operations_and_native_refusals() {
    let mut comparisons = 0;
    let mut baseline_completions = 0;
    for (original, _) in crate::reply_contract_tests::cases() {
        for kind in 0..4 {
            let program = program(original.continuation.pending_effect.clone(), kind);
            for leased in [false, true] {
                for defect in 0..13 {
                    let mut actual = Vm::new(1000);
                    let mut expected = Vm::new(1000);
                    let start = |vm: &mut Vm| {
                        vm.start(
                            &program,
                            Principal::new("operator").unwrap(),
                            CapabilitySet::new([
                                CAPABILITY_UI_PRESENTATION,
                                CAPABILITY_RUNTIME_READ,
                                CAPABILITY_RUNTIME_REFRESH,
                                CAPABILITY_RUNTIME_DEPLOY,
                                CAPABILITY_DEBUGGER_CONTROL,
                            ]),
                            Some(Revision(42)),
                        )
                    };
                    let started = start(&mut actual);
                    assert_eq!(started, start(&mut expected));
                    let Step::Effect(request) = started else {
                        panic!("{started:?}")
                    };
                    let mut lease = if leased {
                        let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                        assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                        lease
                    } else {
                        DispatchLease {
                            request: *request,
                            attempt: 0,
                            retry_count: 0,
                            lease_expires_at_ms: 0,
                        }
                    };
                    let value = Value::UiFocus {
                        node_id: "parent".into(),
                    };
                    let (mut effect, mut binding, mut fuel) =
                        match materialize(&lease.request.continuation, &value).unwrap() {
                            PreparedResult::Host {
                                effect,
                                binding,
                                fuel,
                            } => (effect, binding, fuel),
                            PreparedResult::Scalar(value) => {
                                panic!("successor expected, got {value:?}")
                            }
                        };
                    let mut value = value;
                    let mut now = 12;
                    match defect {
                        1 => {
                            actual = Vm::new(1);
                            expected = Vm::new(1);
                        }
                        2 => lease.request.continuation.deadline_ms += 1,
                        3 => {
                            actual.next_effect_id = u64::MAX;
                            expected.next_effect_id = u64::MAX;
                        }
                        4 => effect = Effect::Sequence { steps: vec![] },
                        5 => fuel = 0,
                        6 => {
                            binding
                                .get_or_insert_with(|| {
                                    crate::group_start_fixture::binding(1).unwrap()
                                })
                                .name = "bad name".into();
                        }
                        7 => {
                            binding
                                .get_or_insert_with(|| {
                                    crate::group_start_fixture::binding(1).unwrap()
                                })
                                .body = leselang_hir::computation::Computation::Literal {
                                value: ScalarValue::String("x".repeat(70_000)),
                            };
                        }
                        8 => lease.attempt += 1,
                        9 => now = 200,
                        10 => {
                            value = Value::UiFocus {
                                node_id: "wrong".into(),
                            }
                        }
                        11 => {
                            let authority = leased.then_some((&lease, now));
                            assert_eq!(
                                frozen_complete_successor(
                                    &mut actual,
                                    &lease.request.continuation,
                                    authority,
                                    value.clone(),
                                    effect.clone(),
                                    binding.clone(),
                                    fuel
                                ),
                                frozen_complete_successor(
                                    &mut expected,
                                    &lease.request.continuation,
                                    authority,
                                    value.clone(),
                                    effect.clone(),
                                    binding.clone(),
                                    fuel
                                )
                            );
                        }
                        12 => {
                            let EffectOperation::Presentation(envelope) =
                                &mut lease.request.operation
                            else {
                                panic!()
                            };
                            envelope.principal = Principal::new("substitute").unwrap();
                            envelope.capabilities = CapabilitySet::new(["forged-capability"]);
                        }
                        _ => {}
                    }
                    let before = serde_json::to_vec(&lease).unwrap();
                    let watermark = actual.next_effect_id;
                    let authority = leased.then_some((&lease, now));
                    let want = frozen_complete_successor(
                        &mut expected,
                        &lease.request.continuation,
                        authority,
                        value.clone(),
                        effect.clone(),
                        binding.clone(),
                        fuel,
                    );
                    let got = actual.complete_successor(
                        &lease.request.continuation,
                        authority,
                        value,
                        effect,
                        binding,
                        fuel,
                    );
                    assert_eq!(
                        got, want,
                        "kind={kind}, leased={leased}, defect={defect}, type={:?}",
                        original.continuation.result_type
                    );
                    assert_eq!(
                        serde_json::to_vec(&got).unwrap(),
                        serde_json::to_vec(&want).unwrap()
                    );
                    assert_eq!(serde_json::to_vec(&lease).unwrap(), before);
                    if defect == 12 && leased {
                        assert!(matches!(got, Step::Fault(ref error) if error.code == "LSV1407"));
                        assert_eq!(actual.next_effect_id, watermark + 1);
                        assert!(
                            actual
                                .pending
                                .contains_key(&lease.request.continuation.token)
                        );
                        assert!(
                            !actual
                                .completed
                                .contains_key(&lease.request.continuation.token)
                        );
                    }
                    if defect == 0 {
                        assert!(!matches!(got, Step::Fault(_)), "{got:?}");
                        baseline_completions += 1;
                    }
                    assert_state(&actual, &expected);
                    assert_eq!(
                        actual.claim_effect(201, 100),
                        expected.claim_effect(201, 100)
                    );
                    assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 7280);
    assert_eq!(baseline_completions, 560);
}

#[cfg(feature = "sqlite-journal")]
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

#[cfg(feature = "sqlite-journal")]
#[test]
fn successor_assembly_sqlite_matches_frozen_transaction_failures_and_restart() {
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
            leselang_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}"))).unwrap();
        for mode in [1, 2] {
            for defect in 0..7 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-successor-{}-{}",
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
                        3 => ("vm_effects", "INSERT"),
                        4 => ("vm_dispatches", "INSERT"),
                        5 => ("vm_merge_groups", "INSERT"),
                        _ => ("vm_merge_branches", "INSERT"),
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
    assert_eq!(comparisons, 112);
}

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
    let operation = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
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
    if leselang_hir::host_call::HostOperation::for_effect(&effect).is_none() {
        return Err(invalid());
    }
    Ok(PreparedResult::Host {
        effect: effect.into_owned(),
        binding,
        fuel: fuel.remaining(),
    })
}

#[cfg(feature = "sqlite-journal")]
fn frozen_complete_result(
    vm: &mut Vm,
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    result: EffectResult,
) -> Step {
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
                    return frozen_complete_successor(
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
    frozen_record_result(vm, image, lease, step)
}

// Frozen whole successor assembly and settlement before policy extraction.
pub(super) fn frozen_complete_successor(
    vm: &mut Vm,
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    value: Value,
    effect: Effect,
    binding: Option<Box<ResultBinding>>,
    fuel: u64,
) -> Step {
    let parent = match vm.journal.effect_request(&image.token) {
        Ok(request) if request.continuation == *image => request,
        Ok(_) => return Step::Fault(invalid()),
        Err(error) => return Step::Fault(error),
    };
    let sequence = match vm.allocate_sequence() {
        Ok(sequence) => sequence,
        Err(error) => return Step::Fault(error),
    };
    let (principal, capabilities) = authority(&parent.operation);
    let Some(operation) = leselang_hir::host_call::HostOperation::for_effect(&effect) else {
        return frozen_record_result(vm, image, lease, Step::Fault(invalid()));
    };
    let mut next = match Vm::build_effect_request(
        &effect,
        operation.result_type(),
        sequence,
        principal.clone(),
        capabilities.clone(),
        image.expected_revision,
        image.deadline_ms,
        image.deadline_at_ms,
        fuel,
    ) {
        Ok(request) => request,
        Err(error) => return frozen_record_result(vm, image, lease, Step::Fault(error)),
    };
    next.continuation.max_output_items = image.max_output_items;
    next.budget.max_output_items = image.max_output_items;
    if let Some(binding) = binding {
        next.continuation.schema_version = binding.schema_version();
        next.continuation.result_binding = Some(binding);
    }
    if let Err(error) =
        validate_effect_request(&next).and_then(|()| validate_continuation_size(&next.continuation))
    {
        return frozen_record_result(vm, image, lease, Step::Fault(error));
    }
    let step = Step::Done(value);
    let authoritative = match vm.journal.record_successor(&parent, &step, &next, lease) {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    vm.pending.remove(&image.token);
    vm.completed
        .insert(image.token.clone(), authoritative.clone());
    // The journal may have accepted a competing acknowledgement, so discover its winner.
    vm.visible_step(&image.token, authoritative)
}
