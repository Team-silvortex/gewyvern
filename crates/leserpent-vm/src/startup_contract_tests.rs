use super::*;

fn policies() -> Vec<SchedulerLimits> {
    [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX]
        .into_iter()
        .flat_map(|pending| {
            [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX]
                .into_iter()
                .map(move |active| SchedulerLimits {
                    max_pending_dispatches: pending,
                    max_active_leases: active,
                })
        })
        .collect()
}
fn fuels() -> [u64; 6] {
    [
        0,
        1,
        DEFAULT_FUEL,
        MAX_EXECUTION_FUEL,
        MAX_EXECUTION_FUEL + 1,
        u64::MAX,
    ]
}
fn compare(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(actual.fuel_limit, expected.fuel_limit);
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.scheduler_limits, expected.scheduler_limits);
    assert_eq!(
        actual.scheduler_pressure(101),
        expected.scheduler_pressure(101)
    );
    assert_eq!(
        serde_json::to_vec(&(&actual.pending, &actual.completed)).unwrap(),
        serde_json::to_vec(&(&expected.pending, &expected.completed)).unwrap()
    );
}
fn start(vm: &mut Vm, expression: &str) -> Step {
    vm.start_timed(
        &leserpent_hir::lower(&leselang_syntax::parse(&format!(
            "fn main() = {expression}"
        )))
        .unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
        Some(Revision(7)),
        100,
        1_000,
    )
}

#[test]
fn startup_memory_matches_frozen_defaults_fuel_limits_and_first_execution() {
    compare(&Vm::default(), &frozen_new(DEFAULT_FUEL));
    let mut cases = 0;
    for fuel in fuels() {
        compare(&Vm::new(fuel), &frozen_new(fuel));
        for limits in policies() {
            match (
                Vm::new_with_limits(fuel, limits),
                frozen_new_with_limits(fuel, limits),
            ) {
                (Ok(mut actual), Ok(mut expected)) => {
                    compare(&actual, &expected);
                    for expression in ["true", r#"ui.focus(node_id: "target")"#] {
                        assert_eq!(
                            start(&mut actual, expression),
                            start(&mut expected, expression)
                        );
                        compare(&actual, &expected);
                    }
                }
                (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                _ => panic!("constructor outcome mismatch"),
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 294);
}

#[test]
fn startup_limit_validation_keeps_live_replacement_atomic() {
    for limits in policies() {
        let mut actual = Vm::new(100);
        let mut expected = frozen_new(100);
        for _ in 0..3 {
            assert_eq!(
                start(&mut actual, r#"ui.focus(node_id: "target")"#),
                start(&mut expected, r#"ui.focus(node_id: "target")"#)
            );
        }
        let old = actual.scheduler_limits;
        let want = frozen_validate(limits).map(|()| expected.scheduler_limits = limits);
        let got = actual.set_scheduler_limits(limits);
        assert_eq!(got, want);
        compare(&actual, &expected);
        assert_eq!(actual.pending.len(), 3);
        assert_eq!(
            actual.scheduler_limits,
            if got.is_ok() { limits } else { old }
        );
    }
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::*;
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::cell::Cell;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "leselang-startup-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn path(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    type DatabaseSnapshot = (u32, Vec<(String, Vec<Vec<SqlValue>>)>);
    fn snapshot(path: &Path) -> DatabaseSnapshot {
        let db = Connection::open(path).unwrap();
        let version = db
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        let rows = tables
            .into_iter()
            .map(|table| {
                let columns = db
                    .prepare(&format!("SELECT * FROM {table}"))
                    .unwrap()
                    .column_count();
                let order = (1..=columns)
                    .map(|i| i.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let mut query = db
                    .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
                    .unwrap();
                let rows = query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect();
        (version, rows)
    }
    fn receipt(request: &EffectRequest) -> EffectResult {
        let EffectOperation::Presentation(envelope) = &request.operation else {
            panic!()
        };
        EffectResult::Presentation(
            serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
        )
    }
    fn seed(path: &Path, mode: usize) -> Vec<DispatchLease> {
        let mut vm = frozen_open(path, 1000, SchedulerLimits::default()).unwrap();
        let mut leases = Vec::new();
        if mode == 1 {
            for _ in 0..4 {
                assert!(matches!(
                    start(
                        &mut vm,
                        r#"bind(result: ui.focus(node_id: "target"), body: true)"#
                    ),
                    Step::Effect(_)
                ));
            }
            let first = vm.claim_effect(101, 100).unwrap().unwrap();
            assert!(matches!(
                vm.acknowledge_effect(&first, 102, receipt(&first.request)),
                Step::Done(_)
            ));
            for _ in 0..2 {
                leases.push(vm.claim_effect(103, 100).unwrap().unwrap());
            }
        } else if mode == 2 {
            assert!(matches!(
                start(
                    &mut vm,
                    r#"bind(group: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")), body: bind(tail: ui.focus(node_id: "c"), body: true))"#
                ),
                Step::Effect(_)
            ));
            let first = vm.claim_effect(101, 100).unwrap().unwrap();
            let _ = vm.acknowledge_effect(&first, 102, receipt(&first.request));
        }
        leases
    }

    #[test]
    fn startup_recovery_matches_frozen_empty_leased_and_group_checkpoints() {
        let mut cases = 0;
        for mode in 0..3 {
            for fuel in fuels() {
                for limits in [
                    SchedulerLimits::default(),
                    SchedulerLimits {
                        max_pending_dispatches: 1,
                        max_active_leases: 1,
                    },
                    SchedulerLimits {
                        max_pending_dispatches: 2,
                        max_active_leases: 2,
                    },
                ] {
                    let scratch = Scratch::new();
                    let a = scratch.path("actual");
                    let b = scratch.path("frozen");
                    let leases = seed(&a, mode);
                    seed(&b, mode);
                    let before = snapshot(&a);
                    let mut actual = Vm::open_journal_with_limits(&a, fuel, limits).unwrap();
                    let mut expected = frozen_open(&b, fuel, limits).unwrap();
                    compare(&actual, &expected);
                    assert_eq!(
                        snapshot(&a),
                        before,
                        "startup must not reap, claim or cancel work"
                    );
                    assert_eq!(snapshot(&a), snapshot(&b));
                    for lease in leases {
                        let got = actual.acknowledge_effect(&lease, 104, receipt(&lease.request));
                        let want =
                            expected.acknowledge_effect(&lease, 104, receipt(&lease.request));
                        assert_eq!(got, want);
                        assert!(
                            matches!(got, Step::Done(_)),
                            "saved budgets must survive zero new-root fuel"
                        );
                    }
                    assert_eq!(
                        start(&mut actual, r#"ui.focus(node_id: "new")"#),
                        start(&mut expected, r#"ui.focus(node_id: "new")"#)
                    );
                    compare(&actual, &expected);
                    assert_eq!(snapshot(&a), snapshot(&b));
                    drop((actual, expected));
                    compare(
                        &Vm::open_journal(&a, 100).unwrap(),
                        &frozen_open(&b, 100, SchedulerLimits::default()).unwrap(),
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 54);
    }

    struct PathProbe<'a> {
        path: &'a Path,
        calls: &'a Cell<usize>,
        panic: bool,
    }
    impl AsRef<Path> for PathProbe<'_> {
        fn as_ref(&self) -> &Path {
            self.calls.set(self.calls.get() + 1);
            assert!(!self.panic, "path conversion unwind");
            self.path
        }
    }
    #[test]
    fn startup_policy_refusal_precedes_path_conversion_and_file_creation() {
        let scratch = Scratch::new();
        for limits in policies() {
            let calls = Cell::new(0);
            let path = scratch.path("must-not-create");
            let valid = frozen_validate(limits).is_ok();
            let got = Vm::open_journal_with_limits(
                PathProbe {
                    path: &path,
                    calls: &calls,
                    panic: !valid,
                },
                0,
                limits,
            );
            assert_eq!(calls.get(), usize::from(valid));
            assert_eq!(path.exists(), valid);
            if valid {
                drop(got.unwrap());
                std::fs::remove_file(path).unwrap();
            } else {
                assert_eq!(got.err(), frozen_validate(limits).err());
            }
        }
        let calls = Cell::new(0);
        let path = scratch.path("unwind");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Vm::open_journal_with_limits(
                PathProbe {
                    path: &path,
                    calls: &calls,
                    panic: true,
                },
                100,
                SchedulerLimits::default(),
            )
        }));
        assert!(result.is_err());
        assert_eq!(calls.get(), 1);
        assert!(!path.exists());
    }

    #[test]
    fn startup_native_failures_match_frozen_without_fallback_or_repair() {
        for mode in [
            "future",
            "image",
            "dispatch",
            "watermark",
            "image-before-watermark",
            "migrate-then-fail",
        ] {
            let scratch = Scratch::new();
            let a = scratch.path("actual");
            let b = scratch.path("frozen");
            for path in [&a, &b] {
                seed(path, 1);
                let db = Connection::open(path).unwrap();
                if mode == "future" {
                    db.execute_batch("PRAGMA user_version = 99;").unwrap();
                }
                if mode.contains("image") || mode == "migrate-then-fail" {
                    db.execute(
                        "UPDATE vm_effects SET image=x'7b' WHERE token='continuation-2'",
                        [],
                    )
                    .unwrap();
                }
                if mode == "dispatch" {
                    db.execute(
                        "UPDATE vm_dispatches SET request=x'7b' WHERE token='continuation-2'",
                        [],
                    )
                    .unwrap();
                }
                if mode.contains("watermark") {
                    db.execute("UPDATE vm_metadata SET next_sequence = 1", [])
                        .unwrap();
                    db.execute_batch("CREATE TRIGGER reject_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT, 'injected startup failure'); END;").unwrap();
                }
                if mode == "migrate-then-fail" {
                    db.execute_batch("PRAGMA user_version = 9;").unwrap();
                }
            }
            let before = snapshot(&a);
            let actual = Vm::open_journal(&a, 100)
                .err()
                .unwrap_or_else(|| panic!("expected refusal: {mode}"));
            let expected = frozen_open(&b, 100, SchedulerLimits::default())
                .err()
                .expect("expected frozen refusal");
            assert_eq!(actual, expected, "{mode}");
            assert_eq!(snapshot(&a), snapshot(&b), "{mode}");
            if mode == "migrate-then-fail" {
                assert_eq!(snapshot(&a).0, JOURNAL_SCHEMA_VERSION);
                assert_eq!(snapshot(&a).1, before.1);
            } else {
                assert_eq!(snapshot(&a), before, "{mode}");
            }
            assert_eq!(Vm::open_journal(&a, 100).err(), Some(actual));
        }
    }

    #[test]
    fn startup_keeps_native_watermark_repair_migration_exhaustion_and_bare_images() {
        for mode in ["repair", "migration", "exhaustion", "bare-image"] {
            let scratch = Scratch::new();
            let a = scratch.path("actual");
            let b = scratch.path("frozen");
            for path in [&a, &b] {
                seed(path, 1);
                let db = Connection::open(path).unwrap();
                let sql = match mode {
                    "repair" => "UPDATE vm_metadata SET next_sequence=1",
                    "migration" => "PRAGMA user_version=9",
                    "exhaustion" => "UPDATE vm_metadata SET next_sequence=9223372036854775807",
                    "bare-image" => "DELETE FROM vm_dispatches WHERE token='continuation-2'",
                    _ => unreachable!(),
                };
                db.execute_batch(sql).unwrap();
            }
            let limits = SchedulerLimits {
                max_pending_dispatches: 1,
                max_active_leases: 1,
            };
            let mut actual = Vm::open_journal_with_limits(&a, 0, limits).unwrap();
            let mut expected = frozen_open(&b, 0, limits).unwrap();
            compare(&actual, &expected);
            assert_eq!(actual.pending.len(), 3);
            assert_eq!(actual.completed.len(), 1);
            assert_eq!(
                actual.next_effect_id,
                if mode == "exhaustion" {
                    i64::MAX as u64
                } else {
                    5
                }
            );
            assert_eq!(snapshot(&a), snapshot(&b));
            assert_eq!(snapshot(&a).0, JOURNAL_SCHEMA_VERSION);
            assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
            compare(&actual, &expected);
            assert_eq!(snapshot(&a), snapshot(&b));
        }
    }

    #[test]
    fn startup_native_path_refusals_match_frozen() {
        let scratch = Scratch::new();
        let missing = scratch.path("missing/child");
        let target = scratch.path("target");
        let link = scratch.path("link");
        seed(&target, 1);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let paths = [
            Path::new(""),
            missing.as_path(),
            scratch.0.as_path(),
            link.as_path(),
        ];
        let before = snapshot(&target);
        for path in paths {
            #[cfg(not(unix))]
            if path == link {
                continue;
            }
            let got = Vm::open_journal(path, 100)
                .err()
                .expect("expected path refusal");
            assert_eq!(
                Some(got),
                frozen_open(path, 100, SchedulerLimits::default()).err()
            );
        }
        assert!(!missing.exists());
        assert_eq!(snapshot(&target), before);
    }
}

// Frozen pre-extraction constructors; journal recovery remains the shared native kernel.
fn frozen_validate(limits: SchedulerLimits) -> Result<(), Fault> {
    leselang_command::scheduler_policy::validate_scheduler_limits(limits).map_err(|error| Fault {
        code: "LSV2500".into(),
        message: error.to_string(),
    })
}
fn frozen_new(fuel_limit: u64) -> Vm {
    Vm {
        next_effect_id: 1,
        fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
        pending: BTreeMap::new(),
        completed: BTreeMap::new(),
        journal: Journal::ephemeral(),
        scheduler_limits: SchedulerLimits::default(),
    }
}
fn frozen_new_with_limits(fuel_limit: u64, limits: SchedulerLimits) -> Result<Vm, Fault> {
    frozen_validate(limits)?;
    let mut vm = frozen_new(fuel_limit);
    vm.scheduler_limits = limits;
    Ok(vm)
}
#[cfg(feature = "sqlite-journal")]
fn frozen_open(
    path: impl AsRef<Path>,
    fuel_limit: u64,
    limits: SchedulerLimits,
) -> Result<Vm, Fault> {
    frozen_validate(limits)?;
    let (journal, snapshot) = Journal::open(path.as_ref())?;
    Ok(Vm {
        next_effect_id: snapshot.next_sequence,
        fuel_limit: fuel_limit.min(MAX_EXECUTION_FUEL),
        pending: snapshot.pending,
        completed: snapshot.completed,
        journal,
        scheduler_limits: limits,
    })
}
