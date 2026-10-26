use super::*;

use leselang_command::reference_start::StartContext;
use leserpent_hir::computation::Computation;

fn parse_program(expression: &str) -> HirProgram {
    leserpent_hir::lower(&leselang_syntax::parse(&format!(
        "fn main() = {expression}"
    )))
    .unwrap()
}

fn context() -> StartContext {
    StartContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new([
            CAPABILITY_UI_PRESENTATION,
            CAPABILITY_RUNTIME_READ,
            CAPABILITY_RUNTIME_REFRESH,
            CAPABILITY_RUNTIME_DEPLOY,
            CAPABILITY_DEBUGGER_CONTROL,
        ]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_100),
        fuel_remaining: 100,
    }
}

fn compare_start(
    program: &HirProgram,
    mut actual_vm: Vm,
    mut expected_vm: Vm,
    context: StartContext,
) -> Step {
    actual_vm.fuel_limit = context.fuel_remaining;
    expected_vm.fuel_limit = context.fuel_remaining;
    let before = serde_json::to_vec(program).unwrap();
    let expected = frozen_start_inner(
        &mut expected_vm,
        program,
        context.principal.clone(),
        context.capabilities.clone(),
        context.expected_revision,
        context.deadline_ms,
        context.deadline_at_ms,
    );
    let actual = actual_vm.start_inner(
        program,
        context.principal,
        context.capabilities,
        context.expected_revision,
        context.deadline_ms,
        context.deadline_at_ms,
    );
    assert_eq!(actual, expected);
    assert_eq!(
        serde_json::to_vec(&actual).unwrap(),
        serde_json::to_vec(&expected).unwrap()
    );
    assert_eq!(actual_vm.pending, expected_vm.pending);
    assert_eq!(actual_vm.completed, expected_vm.completed);
    assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
    if matches!(&program.function.effect, Effect::Compute { expression } if expression.is_pure()) {
        assert_eq!(actual_vm.next_effect_id, 1);
        assert_eq!(actual_vm.pending_count(), 0);
    }
    assert_eq!(
        actual_vm.scheduler_pressure(1),
        expected_vm.scheduler_pressure(1)
    );
    for _ in 0..3 {
        assert_eq!(
            actual_vm.claim_effect(1, 100),
            expected_vm.claim_effect(1, 100)
        );
    }
    assert_eq!(
        actual_vm.allocate_sequence(),
        expected_vm.allocate_sequence()
    );
    assert_eq!(serde_json::to_vec(program).unwrap(), before);
    actual
}

fn forms(effect: Effect, ty: Type) -> Vec<HirProgram> {
    let branches = ["a", "b"]
        .map(|name| leserpent_hir::HirBranch {
            name: name.into(),
            effect: effect.clone(),
            result_type: ty,
        })
        .to_vec();
    let sequence = Effect::Sequence {
        steps: branches.clone(),
    };
    let parallel = Effect::All { branches };
    let scalar = || Computation::Literal {
        value: ScalarValue::Integer(7),
    };
    let bound = |effect, body| Effect::Compute {
        expression: Box::new(Computation::Bind {
            name: "result".into(),
            value: Box::new(Computation::Host {
                effect: Box::new(effect),
            }),
            body: Box::new(body),
        }),
    };
    vec![
        effect.clone(),
        bound(effect, scalar()),
        sequence.clone(),
        parallel.clone(),
        bound(sequence, scalar()),
        bound(
            parallel,
            Computation::Host {
                effect: Box::new(Effect::UiFocus {
                    node_id: "tail".into(),
                }),
            },
        ),
    ]
    .into_iter()
    .map(|effect| {
        leserpent_hir::lower(&leselang_syntax::parse(&canonical_source(&effect).unwrap())).unwrap()
    })
    .collect()
}

#[test]
fn root_start_matches_frozen_whole_vm_entry_for_seventy_operations_and_six_routes() {
    let mut comparisons = 0;
    let mut valid_routes = 0;
    for request in crate::continuation_contract_tests::requests() {
        for original in forms(
            request.continuation.pending_effect,
            request.continuation.result_type,
        ) {
            for defect in 0..20 {
                let mut program = original.clone();
                let mut context = context();
                let mut actual_vm = Vm::new(100);
                let mut expected_vm = Vm::new(100);
                match defect {
                    1..=5 => context.fuel_remaining = [0, 1, 2, 3, 8][defect - 1],
                    6 => context.principal.id.clear(),
                    7 => context.capabilities = CapabilitySet::default(),
                    8 => program
                        .function
                        .required_capabilities
                        .push("unknown".into()),
                    9 => program.function.result_type = Type::Structured,
                    10 => context.deadline_ms = 0,
                    11 => context.deadline_ms = MAX_EFFECT_DEADLINE_MS + 1,
                    12 => context.deadline_at_ms = Some(u64::MAX),
                    13 => program
                        .function
                        .required_capabilities
                        .push(program.function.required_capabilities[0].clone()),
                    14 => {
                        for vm in [&mut actual_vm, &mut expected_vm] {
                            assert!(matches!(
                                vm.start(
                                    &parse_program(r#"ui.focus(node_id: "existing")"#),
                                    Principal::new("operator").unwrap(),
                                    CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                                    None
                                ),
                                Step::Effect(_)
                            ));
                            vm.set_scheduler_limits(SchedulerLimits {
                                max_pending_dispatches: 1,
                                ..SchedulerLimits::default()
                            })
                            .unwrap();
                        }
                    }
                    15 => {
                        actual_vm.next_effect_id = MAX_EFFECT_SEQUENCE;
                        expected_vm.next_effect_id = MAX_EFFECT_SEQUENCE;
                    }
                    16 => {
                        program.function.effect = Effect::Sequence { steps: vec![] };
                        program.function.required_capabilities.clear();
                    }
                    17 => {
                        program.function.effect = Effect::UiFocus {
                            node_id: "bad id".into(),
                        };
                        program.function.result_type = Type::UiFocus;
                        program.function.required_capabilities =
                            vec![CAPABILITY_UI_PRESENTATION.into()];
                    }
                    18 => {
                        context.fuel_remaining = 0;
                        context.principal.id.clear();
                        program
                            .function
                            .required_capabilities
                            .push("unknown".into());
                    }
                    19 => context.deadline_at_ms = None,
                    _ => {}
                }
                let result = compare_start(&program, actual_vm, expected_vm, context);
                if defect == 0 {
                    assert!(!matches!(result, Step::Fault(_)), "{result:?}");
                    valid_routes += 1;
                }
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 8400);
    assert_eq!(valid_routes, 420);
}

#[test]
fn root_start_pure_and_lazy_entries_keep_frozen_precedence_and_never_allocate() {
    let mut comparisons = 0;
    for expression in [
        "7",
        "none",
        "true",
        r#""text""#,
        "add(left: 2, right: 3)",
        "div(left: 1, right: 0)",
        "bind(x: 2, body: mul(left: x, right: 3))",
        "choose(when: true, then: 7, otherwise: div(left: 1, right: 0))",
        r#"parse_integer(value: "invalid")"#,
        r#"concat(left: "a", right: "b")"#,
    ] {
        let original = parse_program(expression);
        for fuel in [0, 1, 2, 3, 4, 8, 32, 100] {
            for defect in 0..4 {
                let mut program = original.clone();
                let mut context = context();
                context.fuel_remaining = fuel;
                match defect {
                    1 => context.principal.id.clear(),
                    2 => program
                        .function
                        .required_capabilities
                        .push("unknown".into()),
                    3 => program.function.result_type = Type::Structured,
                    _ => {}
                }
                let actual = compare_start(&program, Vm::new(fuel), Vm::new(fuel), context);
                assert!(matches!(actual, Step::Done(_) | Step::Fault(_)));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 320);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn root_start_sqlite_atomic_and_group_publication_matches_frozen_failed_write_restarts() {
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
    let mut comparisons = 0;
    for expression in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: field(value: r, name: "node_id"))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: 7)"#,
        r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "tail"))"#,
        "7",
        "choose(when: true, then: 7, otherwise: div(left: 1, right: 0))",
    ] {
        let program = parse_program(expression);
        let pure = matches!(&program.function.effect, Effect::Compute { expression } if expression.is_pure());
        for defect in 0..3 {
            let scratch = Scratch(std::env::temp_dir().join(format!(
                "leselang-root-start-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )));
            std::fs::create_dir(&scratch.0).unwrap();
            let actual_path = scratch.0.join("actual.sqlite3");
            let expected_path = scratch.0.join("expected.sqlite3");
            let mut actual_vm = Vm::open_journal(&actual_path, 100).unwrap();
            let mut expected_vm = Vm::open_journal(&expected_path, 100).unwrap();
            if defect != 0 {
                let table = if defect == 1 {
                    "vm_effects"
                } else {
                    "vm_dispatches"
                };
                for path in [&actual_path, &expected_path] {
                    Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_root_start BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                }
            }
            let context = context();
            let expected = frozen_start_inner(
                &mut expected_vm,
                &program,
                context.principal.clone(),
                context.capabilities.clone(),
                context.expected_revision,
                context.deadline_ms,
                context.deadline_at_ms,
            );
            let actual = actual_vm.start_inner(
                &program,
                context.principal,
                context.capabilities,
                context.expected_revision,
                context.deadline_ms,
                context.deadline_at_ms,
            );
            assert_eq!(actual, expected);
            assert_eq!(matches!(actual, Step::Fault(_)), defect != 0 && !pure);
            assert_eq!(actual_vm.pending, expected_vm.pending);
            assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
            assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
            if pure {
                assert_eq!(actual_vm.next_effect_id, 1);
            } else {
                assert!(actual_vm.next_effect_id > 1);
            }
            drop(actual_vm);
            drop(expected_vm);
            let mut actual_vm = Vm::open_journal(&actual_path, 100).unwrap();
            let mut expected_vm = Vm::open_journal(&expected_path, 100).unwrap();
            assert_eq!(actual_vm.pending, expected_vm.pending);
            assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
            if defect != 0 || pure {
                assert_eq!(actual_vm.pending_count(), 0);
                assert!(actual_vm.claim_effect(1, 100).unwrap().is_none());
            }
            assert_eq!(
                actual_vm.claim_effect(1, 100),
                expected_vm.claim_effect(1, 100)
            );
            assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 24);
}

// Frozen whole root entry before its product coordination moved to the adapter.
fn frozen_start_inner(
    vm: &mut Vm,
    program: &HirProgram,
    principal: Principal,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
    deadline_ms: u64,
    deadline_at_ms: Option<u64>,
) -> Step {
    if vm.fuel_limit == 0 {
        return fault("LSV1001", "execution fuel exhausted");
    }
    if let Err(error) = authorize(program, &capabilities) {
        return fault(&error.code, error.message);
    }
    let mut fuel = Fuel::new(vm.fuel_limit);
    let (effect, result_binding) = if let Effect::Compute { expression } = &program.function.effect
    {
        if !principal.is_valid() {
            return fault("LSV1402", "invalid computation principal");
        }
        match computation::evaluate(expression, &mut fuel) {
            Ok(computation::Outcome::Scalar(value)) => {
                return Step::Done(Value::Scalar { value });
            }
            Ok(computation::Outcome::Host(effect)) => (effect, None),
            Ok(computation::Outcome::BoundHost { effect, binding }) => (effect, Some(binding)),
            Ok(computation::Outcome::Result) => {
                return fault("LSV1402", "unexpected unprojected result");
            }
            Err(error) => return Step::Fault(error),
        }
    } else {
        (std::borrow::Cow::Borrowed(&program.function.effect), None)
    };
    let fuel_remaining = fuel.remaining();
    let effect = effect.as_ref();
    let effect_result_type = if let Some(binding) = &result_binding {
        if let Err(error) = binding.validate(effect) {
            return Step::Fault(error);
        }
        match leserpent_hir::host_call::HostOperation::for_effect(effect) {
            Some(operation) => operation.result_type(),
            None if matches!(effect, Effect::All { .. } | Effect::Sequence { .. }) => {
                Type::Structured
            }
            None => return Step::Fault(result_binding::invalid()),
        }
    } else {
        program.function.result_type
    };
    if fuel_remaining == 0 {
        return fault("LSV1001", "execution fuel exhausted before host operation");
    }
    if let Effect::Sequence { .. } = effect
        && canonical_source(effect).is_err()
    {
        return fault("LSV1002", "invalid sequential control flow");
    }
    if let Effect::All { branches } | Effect::Sequence { steps: branches } = effect {
        return vm.start_group(
            branches,
            if matches!(effect, Effect::Sequence { .. }) {
                ExecutionOrder::Sequential
            } else {
                ExecutionOrder::Parallel
            },
            principal,
            capabilities,
            expected_revision,
            deadline_ms,
            deadline_at_ms,
            fuel_remaining,
            result_binding,
        );
    }

    if let Err(error) = vm.journal.check_admission(1, vm.scheduler_limits) {
        return Step::Fault(error);
    }
    let sequence = match vm.allocate_sequence() {
        Ok(sequence) => sequence,
        Err(error) => return Step::Fault(error),
    };
    let mut request = match Vm::build_effect_request(
        effect,
        effect_result_type,
        sequence,
        principal,
        capabilities,
        expected_revision,
        deadline_ms,
        deadline_at_ms,
        fuel_remaining,
    ) {
        Ok(request) => request,
        Err(error) => return Step::Fault(error),
    };
    if let Some(binding) = result_binding {
        request.continuation.schema_version = binding.schema_version();
        request.continuation.result_binding = Some(binding);
        if let Err(error) = validate_effect_request(&request) {
            return Step::Fault(error);
        }
    }
    if let Err(error) =
        vm.journal
            .record_pending(&request.continuation, Some(&request), vm.scheduler_limits)
    {
        return Step::Fault(error);
    }
    vm.pending.insert(
        request.continuation.token.clone(),
        request.continuation.clone(),
    );
    Step::Effect(Box::new(request))
}
