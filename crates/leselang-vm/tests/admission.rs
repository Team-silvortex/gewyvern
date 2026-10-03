use leselang_hir::{Effect, HirProgram, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    AdmissionEnd, AdmissionPolicy, AdmissionPoll, AdmissionStatus, AdmissionWait, EffectOperation,
    EffectRequest, EffectResult, MAX_ADMISSION_ATTEMPTS, MAX_ADMISSION_DELAY_MS,
    MAX_EFFECT_DEADLINE_MS, RootAdmission, SchedulerLimits, Step, Vm,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const FOCUS: &str = r#"fn main() = bind(result: ui.focus(node_id: "target"), body: true)"#;
const GROUP: &str =
    r#"fn main() = all(left: ui.focus(node_id: "a"), right: ui.focus(node_id: "b"))"#;

#[test]
fn admission_debug_does_not_disclose_script_authority_or_form_values() {
    let admission = RootAdmission::new(
        program(r#"fn main() = ui.set_form_value(node_id: "private-form-marker", field: "password", value: "PRIVATE_FORM_VALUE_MARKER")"#),
        Principal::new("private-operator-marker").unwrap(),
        CapabilitySet::new(["ui.presentation", "private.scope.marker"]),
        Some(Revision(7)),
        100,
        1_000,
        AdmissionPolicy::default(),
    ).unwrap();
    for debug in [format!("{admission:?}"), format!("{admission:#?}")] {
        for private in [
            "PRIVATE_FORM_VALUE_MARKER",
            "private-form-marker",
            "private-operator-marker",
            "private.scope.marker",
        ] {
            assert!(
                !debug.contains(private),
                "private input leaked in VM admission debug output"
            );
        }
    }
}

#[test]
fn admission_status_is_shared_metadata_without_vm_or_journal_side_effects() {
    let path = JournalPath::new();
    let mut vm = path.vm(true, 1, 50);
    let existing = effect(start(&mut vm, FOCUS, 100, 1_000));
    let mut handle = root(FOCUS, 100, 1_000, policy(3, 10, 20));
    let before = (path.records("vm_effects"), path.records("vm_dispatches"));
    let initial: leselang_runtime_core::AdmissionStatus = handle.status();
    assert_eq!(
        initial,
        AdmissionStatus::Pending(AdmissionWait {
            attempts: 0,
            retry_at_ms: 100,
            deadline_at_ms: 1_100,
        })
    );
    for _ in 0..100 {
        assert_eq!(handle.status(), initial);
        assert_eq!(handle.terminal_reason(), None);
        assert!(!format!("{handle:?}").contains("target"));
    }
    assert_eq!(
        (path.records("vm_effects"), path.records("vm_dispatches")),
        before
    );
    let wait = waiting(handle.poll(&mut vm, 100).unwrap());
    let snapshot = handle.status();
    assert_eq!(snapshot, AdmissionStatus::Pending(wait));
    assert_eq!(wait.retry_at_ms, 110);
    for _ in 0..100 {
        assert_eq!(handle.status(), snapshot);
        assert_eq!(handle.terminal_reason(), None);
    }
    assert_eq!(handle.poll(&mut vm, 99).unwrap_err().code, "LSV2511");
    assert_eq!(handle.status(), snapshot);
    assert_eq!(
        (path.records("vm_effects"), path.records("vm_dispatches")),
        before
    );
    assert!(handle.cancel());
    assert_eq!(handle.terminal_reason(), Some(AdmissionEnd::Cancelled));
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 1 });
    assert_eq!(
        handle.poll(&mut vm, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(handle.status(), AdmissionStatus::Finished { attempts: 1 });
    assert_eq!(
        (path.records("vm_effects"), path.records("vm_dispatches")),
        before
    );
    let lease = vm.claim_effect(101, 50).unwrap().unwrap();
    assert_eq!(lease.request.effect_id, existing.effect_id);
    assert!(!matches!(
        vm.acknowledge_effect(&lease, 102, receipt(&lease.request)),
        Step::Fault(_)
    ));
    let next = effect(start(&mut vm, FOCUS, 103, 1_000));
    assert_eq!(next.effect_id, "effect-2");
}

#[test]
fn reference_terminal_reasons_are_local_metadata_not_execution_completion() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 100);
        let mut pure = root("fn main() = true", 100, 1_000, policy(2, 10, 20));
        assert!(matches!(
            started(pure.poll(&mut vm, 100).unwrap()),
            Step::Done(_)
        ));
        assert_eq!(pure.terminal_reason(), Some(AdmissionEnd::Started));
        assert_eq!(vm.pending_count(), 0);

        let mut accepted = root(FOCUS, 100, 1_000, policy(2, 10, 20));
        let request = effect(started(accepted.poll(&mut vm, 100).unwrap()));
        let shared: leselang_runtime_core::AdmissionEnd = accepted.terminal_reason().unwrap();
        assert_eq!(shared, AdmissionEnd::Started);
        assert!(!accepted.cancel());
        assert_eq!(vm.pending_count(), 1);

        let mut cancelled = root(FOCUS, 100, 1_000, policy(2, 10, 20));
        waiting(cancelled.poll(&mut vm, 100).unwrap());
        assert_eq!(cancelled.terminal_reason(), None);
        assert!(cancelled.cancel());

        let mut expired = root(FOCUS, 100, 10, policy(2, 10, 20));
        rejected(expired.poll(&mut vm, 110).unwrap(), "LSV2513");

        let mut exhausted = root(FOCUS, 100, 1_000, policy(1, 10, 20));
        rejected(exhausted.poll(&mut vm, 100).unwrap(), "LSV2512");

        let mut denied = root("fn main() = runtime.list()", 100, 1_000, policy(2, 10, 20));
        rejected(denied.poll(&mut vm, 100).unwrap(), "LSH2001");

        let writer = durable.then(|| Connection::open(&path.0).unwrap());
        if let Some(writer) = &writer {
            writer.execute_batch("BEGIN IMMEDIATE").unwrap();
        }
        for (handle, expected) in [
            (&mut pure, AdmissionEnd::Started),
            (&mut accepted, AdmissionEnd::Started),
            (&mut cancelled, AdmissionEnd::Cancelled),
            (&mut expired, AdmissionEnd::Expired),
            (&mut exhausted, AdmissionEnd::Exhausted),
            (&mut denied, AdmissionEnd::Rejected),
        ] {
            for _ in 0..100 {
                assert_eq!(handle.terminal_reason(), Some(expected));
                assert!(matches!(handle.status(), AdmissionStatus::Finished { .. }));
            }
            assert!(!handle.cancel());
            assert_eq!(
                handle.poll(&mut vm, u64::MAX).unwrap(),
                AdmissionPoll::Finished
            );
            assert_eq!(handle.terminal_reason(), Some(expected));
        }
        if let Some(writer) = &writer {
            writer.execute_batch("ROLLBACK").unwrap();
        }
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&request.continuation)
        );
        assert_eq!(vm.completed_count(), 0);
        if durable {
            assert_eq!(path.records("vm_effects"), 1);
            assert_eq!(path.records("vm_dispatches"), 1);
            drop(vm);
            vm = path.vm(true, 1, 0);
            assert_eq!(
                vm.pending_continuations(),
                std::slice::from_ref(&request.continuation)
            );
        }
        let lease = vm.claim_effect(111, 50).unwrap().unwrap();
        assert_eq!(lease.request, request);
        assert!(matches!(
            vm.acknowledge_effect(&lease, 112, receipt(&lease.request)),
            Step::Done(_)
        ));
        assert_eq!(accepted.terminal_reason(), Some(AdmissionEnd::Started));
    }
}

#[test]
fn core_extraction_preserves_public_variant_imports_shared_types_and_wire_bytes() {
    use leselang_vm::AdmissionPoll::{Finished, Rejected, Started, Waiting};

    let fault: leselang_runtime_core::Fault = leselang_vm::Fault {
        code: "LSV2513".into(),
        message: "root expired before admission".into(),
    };
    let shared_policy: leselang_runtime_core::AdmissionPolicy = AdmissionPolicy::default();
    assert_eq!(
        shared_policy,
        leselang_runtime_core::AdmissionPolicy::default()
    );
    assert_eq!(
        MAX_EFFECT_DEADLINE_MS,
        leselang_runtime_core::MAX_EXECUTION_TIMEOUT_MS
    );
    let step = Box::new(Step::Done(leselang_vm::Value::Scalar {
        value: leselang_vm::ScalarValue::Boolean(true),
    }));
    let wait = AdmissionWait {
        attempts: 1,
        retry_at_ms: 110,
        deadline_at_ms: 1_100,
    };
    let outcomes = [
        (
            Waiting(wait),
            leselang_runtime_core::AdmissionPoll::Waiting(wait),
        ),
        (
            Rejected(fault.clone()),
            leselang_runtime_core::AdmissionPoll::Rejected(fault),
        ),
        (
            Started(step.clone()),
            leselang_runtime_core::AdmissionPoll::Started(step),
        ),
        (Finished, leselang_runtime_core::AdmissionPoll::Finished),
    ];
    for (reference, core) in outcomes {
        assert_eq!(
            serde_json::to_vec(&reference).unwrap(),
            serde_json::to_vec(&core).unwrap()
        );
        assert_eq!(AdmissionPoll::from(core), reference);
    }
}

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-admission-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
    fn vm(&self, durable: bool, pending: usize, fuel: u64) -> Vm {
        let limits = SchedulerLimits {
            max_pending_dispatches: pending,
            max_active_leases: 1,
        };
        if durable {
            Vm::open_journal_with_limits(&self.0, fuel, limits).unwrap()
        } else {
            Vm::new_with_limits(fuel, limits).unwrap()
        }
    }
    fn records(&self, table: &str) -> i64 {
        Connection::open(&self.0)
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn program(source: &str) -> HirProgram {
    lower(&parse(source)).unwrap()
}
fn policy(attempts: u32, base: u64, max: u64) -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts: attempts,
        base_delay_ms: base,
        max_delay_ms: max,
    }
}
fn root(source: &str, now: u64, timeout: u64, policy: AdmissionPolicy) -> RootAdmission {
    RootAdmission::new(
        program(source),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        now,
        timeout,
        policy,
    )
    .unwrap()
}
fn start(vm: &mut Vm, source: &str, now: u64, timeout: u64) -> Step {
    vm.start_timed(
        &program(source),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        now,
        timeout,
    )
}
fn effect(step: Step) -> EffectRequest {
    let Step::Effect(request) = step else {
        panic!("{step:?}")
    };
    *request
}
fn started(outcome: AdmissionPoll) -> Step {
    let AdmissionPoll::Started(step) = outcome else {
        panic!("{outcome:?}")
    };
    *step
}
fn waiting(outcome: AdmissionPoll) -> AdmissionWait {
    let AdmissionPoll::Waiting(wait) = outcome else {
        panic!("{outcome:?}")
    };
    wait
}
fn rejected(outcome: AdmissionPoll, code: &str) {
    assert!(
        matches!(&outcome, AdmissionPoll::Rejected(fault) if fault.code == code),
        "{outcome:?}"
    );
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

#[test]
fn invalid_policy_and_deadlines_fail_without_creating_a_journal() {
    let path = JournalPath::new();
    for invalid in [
        policy(0, 1, 1),
        policy(MAX_ADMISSION_ATTEMPTS + 1, 1, 1),
        policy(1, 0, 1),
        policy(1, 2, 1),
        policy(1, MAX_ADMISSION_DELAY_MS + 1, MAX_ADMISSION_DELAY_MS + 1),
        policy(1, 1, u64::MAX),
    ] {
        let preflight = invalid.validate().unwrap_err();
        assert_eq!(preflight.code, "LSV2510");
        assert!(!path.0.exists());
        assert_eq!(
            RootAdmission::new(
                program(FOCUS),
                Principal::new("operator").unwrap(),
                CapabilitySet::new(["ui.presentation"]),
                Some(Revision(7)),
                100,
                1000,
                invalid
            )
            .unwrap_err(),
            preflight
        );
    }
    for (now, timeout, code) in [
        (100, 0, "LSV2012"),
        (100, MAX_EFFECT_DEADLINE_MS + 1, "LSV2012"),
        (i64::MAX as u64, 1, "LSV2011"),
        (u64::MAX, 1, "LSV2011"),
    ] {
        assert_eq!(
            RootAdmission::new(
                program(FOCUS),
                Principal::new("operator").unwrap(),
                CapabilitySet::new(["ui.presentation"]),
                None,
                now,
                timeout,
                AdmissionPolicy::default()
            )
            .unwrap_err()
            .code,
            code
        );
    }
    assert!(!path.0.exists());
    assert!(
        serde_json::from_str::<AdmissionPolicy>(
            r#"{"max_attempts":2,"base_delay_ms":1,"max_delay_ms":2,"unbounded":true}"#
        )
        .is_err()
    );
}

#[test]
fn polling_before_retry_is_vm_free_and_does_not_reap_unrelated_deadlines() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        effect(start(&mut vm, FOCUS, 100, 5));
        let mut admission = root(FOCUS, 100, 100, policy(3, 10, 20));
        let wait = waiting(admission.poll(&mut vm, 100).unwrap());
        assert_eq!(
            wait,
            AdmissionWait {
                attempts: 1,
                retry_at_ms: 110,
                deadline_at_ms: 200
            }
        );
        assert_eq!(
            admission.poll(&mut vm, 109).unwrap(),
            AdmissionPoll::Waiting(wait)
        );
        let mut no_fuel = Vm::new(0);
        assert_eq!(
            admission.poll(&mut no_fuel, 109).unwrap(),
            AdmissionPoll::Waiting(wait)
        );
        assert_eq!(vm.scheduler_pressure(109).unwrap().pending_dispatches, 1);
        assert_eq!(admission.attempts(), 1);
        let request = effect(started(admission.poll(&mut vm, 110).unwrap()));
        assert_eq!(request.effect_id, "effect-2");
        assert_eq!(vm.scheduler_pressure(110).unwrap().pending_dispatches, 1);
        assert!(admission.is_finished());
    }
}

#[test]
fn accepted_work_has_the_original_absolute_deadline_authority_and_fuel() {
    let source = r#"fn main() = bind(target: concat(left: "node-", right: "a"), body: bind(result: ui.focus(node_id: target), body: true))"#;
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        let blocker = effect(start(&mut vm, FOCUS, 100, 1000));
        let mut admission = root(source, 100, 100, policy(4, 10, 20));
        waiting(admission.poll(&mut vm, 100).unwrap());
        assert!(matches!(
            vm.cancel_effect(&blocker.continuation, 105),
            Step::Cancelled(_)
        ));
        let accepted = effect(started(admission.poll(&mut vm, 110).unwrap()));
        let baseline = effect(start(&mut Vm::new(10000), source, 110, 90));
        assert_eq!(accepted.effect_id, "effect-2");
        assert_eq!(accepted.operation, baseline.operation);
        assert_eq!(accepted.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(
            accepted.budget.fuel_remaining,
            baseline.budget.fuel_remaining
        );
        assert_eq!(accepted.budget.deadline_ms, 90);
        assert_eq!(accepted.budget.deadline_at_ms, Some(200));
        assert_eq!(admission.attempts(), 2);
        assert!(!admission.cancel());
        for now in [110, 120, 1000] {
            assert_eq!(
                admission.poll(&mut vm, now).unwrap(),
                AdmissionPoll::Finished
            );
        }
        assert_eq!(vm.pending_count(), 1);
        let lease = vm.claim_effect(111, 50).unwrap().unwrap();
        assert_eq!(lease.request, accepted);
        assert!(matches!(
            vm.acknowledge_effect(&lease, 112, receipt(&accepted)),
            Step::Done(_)
        ));
        assert_eq!(
            admission.poll(&mut vm, 113).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn pure_completion_is_not_blocked_by_a_full_outbox() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        effect(start(&mut vm, FOCUS, 100, 1000));
        let mut admission = root(
            "fn main() = add(left: 2, right: 3)",
            100,
            100,
            AdmissionPolicy::default(),
        );
        assert!(matches!(
            started(admission.poll(&mut vm, 100).unwrap()),
            Step::Done(_)
        ));
        assert_eq!(admission.attempts(), 1);
        assert!(admission.is_finished());
        assert_eq!(vm.pending_count(), 1);
    }
}

#[test]
fn capped_exponential_backoff_stops_at_its_total_attempt_budget() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        let blocker = effect(start(&mut vm, FOCUS, 100, 1000));
        let mut admission = root(FOCUS, 100, 1000, policy(5, 10, 25));
        for (index, (now, next)) in [(100, 110), (110, 130), (130, 155), (155, 180)]
            .into_iter()
            .enumerate()
        {
            let wait = waiting(admission.poll(&mut vm, now).unwrap());
            assert_eq!(wait.attempts, index as u32 + 1);
            assert_eq!(wait.retry_at_ms, next);
            assert_eq!(
                admission.poll(&mut vm, now).unwrap(),
                AdmissionPoll::Waiting(wait)
            );
        }
        assert_eq!(
            admission.poll(&mut vm, 180).unwrap(),
            AdmissionPoll::Rejected(leselang_vm::Fault {
                code: "LSV2512".into(),
                message: "admission exhausted after 5 attempts".into(),
            })
        );
        assert_eq!(admission.attempts(), 5);
        assert!(admission.is_finished());
        vm.cancel_effect(&blocker.continuation, 181);
        assert_eq!(
            admission.poll(&mut vm, 182).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(
            effect(start(&mut vm, FOCUS, 182, 1000)).effect_id,
            "effect-2"
        );
    }
}

#[test]
fn one_attempt_policy_rejects_pressure_without_scheduling_a_retry() {
    let mut vm = Vm::new_with_limits(
        10000,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
    )
    .unwrap();
    effect(start(&mut vm, FOCUS, 100, 1000));
    let mut admission = root(FOCUS, 100, 100, policy(1, 1, 1));
    rejected(admission.poll(&mut vm, 100).unwrap(), "LSV2512");
    assert_eq!(admission.attempts(), 1);
    assert_eq!(
        admission.poll(&mut vm, 101).unwrap(),
        AdmissionPoll::Finished
    );
}

#[test]
fn maximum_attempt_policy_still_terminates_with_a_nonzero_not_before_bound() {
    let mut vm = Vm::new_with_limits(
        10000,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
    )
    .unwrap();
    effect(start(&mut vm, FOCUS, 100, 1000));
    let mut admission = root(FOCUS, 100, 1000, policy(MAX_ADMISSION_ATTEMPTS, 1, 1));
    for attempt in 1..MAX_ADMISSION_ATTEMPTS {
        let now = 99 + u64::from(attempt);
        let wait = waiting(admission.poll(&mut vm, now).unwrap());
        assert_eq!(wait.attempts, attempt);
        assert_eq!(wait.retry_at_ms, now + 1);
    }
    rejected(
        admission
            .poll(&mut vm, 99 + u64::from(MAX_ADMISSION_ATTEMPTS))
            .unwrap(),
        "LSV2512",
    );
    assert_eq!(admission.attempts(), MAX_ADMISSION_ATTEMPTS);
    assert!(admission.is_finished());
}

#[test]
fn wait_and_terminal_outcomes_have_machine_readable_kinds_without_serializing_the_handle() {
    let mut vm = Vm::new_with_limits(
        10000,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
    )
    .unwrap();
    effect(start(&mut vm, FOCUS, 100, 1000));
    let mut admission = root(FOCUS, 100, 100, policy(2, 10, 20));
    let waiting = serde_json::to_value(admission.poll(&mut vm, 100).unwrap()).unwrap();
    assert_eq!(waiting["kind"], "waiting");
    assert_eq!(waiting["payload"]["attempts"], 1);
    assert_eq!(waiting["payload"]["retry_at_ms"], 110);
    assert_eq!(waiting["payload"]["deadline_at_ms"], 200);
    let rejected = serde_json::to_value(admission.poll(&mut vm, 110).unwrap()).unwrap();
    assert_eq!(rejected["kind"], "rejected");
    assert_eq!(rejected["payload"]["code"], "LSV2512");
    assert_eq!(
        serde_json::to_value(admission.poll(&mut vm, 111).unwrap()).unwrap(),
        serde_json::json!({"kind":"finished"})
    );
}

#[test]
fn deadline_stops_even_pure_work_before_any_admission_attempt() {
    let mut vm = Vm::new(10000);
    let mut admission = root("fn main() = true", 100, 10, AdmissionPolicy::default());
    rejected(admission.poll(&mut vm, 110).unwrap(), "LSV2513");
    assert_eq!(admission.attempts(), 0);
    assert!(admission.is_finished());
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn waiting_reaches_the_pinned_deadline_without_overflow_or_extending_it() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        let now = i64::MAX as u64 - 100;
        effect(start(&mut vm, FOCUS, now, 100));
        let mut admission = root(
            FOCUS,
            now,
            100,
            policy(
                MAX_ADMISSION_ATTEMPTS,
                MAX_ADMISSION_DELAY_MS,
                MAX_ADMISSION_DELAY_MS,
            ),
        );
        let wait = waiting(admission.poll(&mut vm, now).unwrap());
        assert_eq!(wait.retry_at_ms, i64::MAX as u64);
        assert_eq!(wait.deadline_at_ms, i64::MAX as u64);
        assert_eq!(
            admission.poll(&mut vm, i64::MAX as u64 - 1).unwrap(),
            AdmissionPoll::Waiting(wait)
        );
        rejected(admission.poll(&mut vm, i64::MAX as u64).unwrap(), "LSV2513");
        assert_eq!(admission.attempts(), 1);
        assert_eq!(vm.pending_count(), 1);
    }
}

#[test]
fn invalid_or_backward_clocks_leave_an_active_handle_unchanged() {
    let mut vm = Vm::new_with_limits(
        10000,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
    )
    .unwrap();
    effect(start(&mut vm, FOCUS, 100, 1000));
    let mut admission = root(FOCUS, 100, 100, policy(3, 10, 20));
    assert_eq!(admission.poll(&mut vm, 99).unwrap_err().code, "LSV2511");
    assert_eq!(admission.attempts(), 0);
    let wait = waiting(admission.poll(&mut vm, 100).unwrap());
    assert_eq!(
        admission.poll(&mut vm, 105).unwrap(),
        AdmissionPoll::Waiting(wait)
    );
    assert_eq!(admission.poll(&mut vm, 104).unwrap_err().code, "LSV2511");
    assert_eq!(
        admission.poll(&mut vm, u64::MAX).unwrap_err().code,
        "LSV2011"
    );
    assert_eq!(
        admission.poll(&mut vm, 105).unwrap(),
        AdmissionPoll::Waiting(wait)
    );
    assert_eq!(admission.attempts(), 1);
    assert!(!admission.is_finished());
}

#[test]
fn local_cancellation_never_enters_the_vm_or_cancels_another_root() {
    for durable in [false, true] {
        for after_pressure in [false, true] {
            let path = JournalPath::new();
            let mut vm = path.vm(durable, 1, 10000);
            let blocker = effect(start(&mut vm, FOCUS, 100, 1000));
            let mut admission = root(FOCUS, 100, 100, policy(3, 10, 20));
            if after_pressure {
                waiting(admission.poll(&mut vm, 100).unwrap());
            }
            assert!(admission.cancel());
            assert!(!admission.cancel());
            assert_eq!(
                admission.poll(&mut vm, 110).unwrap(),
                AdmissionPoll::Finished
            );
            assert_eq!(vm.pending_count(), 1);
            let lease = vm.claim_effect(111, 50).unwrap().unwrap();
            assert_eq!(lease.request, blocker);
            assert!(matches!(
                vm.acknowledge_effect(&lease, 112, receipt(&blocker)),
                Step::Done(_)
            ));
        }
    }
}

#[test]
fn permanent_merge_validation_faults_are_not_retried_as_backpressure() {
    let mut malformed = program(GROUP);
    let Effect::All { branches } = &mut malformed.function.effect else {
        panic!()
    };
    branches[1].name = branches[0].name.clone();
    let mut admission = RootAdmission::new(
        malformed,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        1000,
        AdmissionPolicy::default(),
    )
    .unwrap();
    let mut vm = Vm::new(10000);
    rejected(admission.poll(&mut vm, 100).unwrap(), "LSV2401");
    assert_eq!(admission.attempts(), 1);
    assert_eq!(
        admission.poll(&mut vm, 101).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn authorization_fuel_and_computation_faults_are_terminal() {
    for (source, fuel, capabilities) in [
        (FOCUS, 10000, CapabilitySet::new([] as [&str; 0])),
        (FOCUS, 0, CapabilitySet::new(["ui.presentation"])),
        (
            "fn main() = div(left: 1, right: 0)",
            10000,
            CapabilitySet::new(["ui.presentation"]),
        ),
    ] {
        let mut baseline = Vm::new(fuel);
        let Step::Fault(expected) = baseline.start_timed(
            &program(source),
            Principal::new("operator").unwrap(),
            capabilities.clone(),
            None,
            100,
            1000,
        ) else {
            panic!()
        };
        let mut admission = RootAdmission::new(
            program(source),
            Principal::new("operator").unwrap(),
            capabilities,
            None,
            100,
            1000,
            AdmissionPolicy::default(),
        )
        .unwrap();
        let mut vm = Vm::new(fuel);
        assert_eq!(
            admission.poll(&mut vm, 100).unwrap(),
            AdmissionPoll::Rejected(expected)
        );
        assert_eq!(
            admission.poll(&mut vm, 110).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn journal_write_fault_is_terminal_and_does_not_hide_partial_publication() {
    let path = JournalPath::new();
    let mut vm = path.vm(true, 1, 10000);
    let connection = Connection::open(&path.0).unwrap();
    connection.execute_batch("CREATE TRIGGER reject_dispatch BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'injected failure'); END;").unwrap();
    let mut admission = root(FOCUS, 100, 1000, AdmissionPolicy::default());
    rejected(admission.poll(&mut vm, 100).unwrap(), "LSV4010");
    assert_eq!(path.records("vm_effects"), 0);
    assert_eq!(path.records("vm_dispatches"), 0);
    connection
        .execute_batch("DROP TRIGGER reject_dispatch;")
        .unwrap();
    assert_eq!(
        admission.poll(&mut vm, 110).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn busy_initial_batches_wait_as_a_whole_but_oversized_batches_are_terminal() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 1, 10000);
        let mut oversized = root(GROUP, 100, 1000, AdmissionPolicy::default());
        rejected(oversized.poll(&mut vm, 100).unwrap(), "LSV2503");
        let blocker = effect(start(&mut vm, FOCUS, 100, 1000));
        vm.set_scheduler_limits(SchedulerLimits {
            max_pending_dispatches: 2,
            max_active_leases: 1,
        })
        .unwrap();
        assert_eq!(
            oversized.poll(&mut vm, 101).unwrap(),
            AdmissionPoll::Finished
        );
        let mut busy = root(GROUP, 100, 100, policy(4, 10, 20));
        waiting(busy.poll(&mut vm, 100).unwrap());
        assert_eq!(vm.scheduler_pressure(100).unwrap().pending_dispatches, 1);
        if durable {
            assert_eq!(path.records("vm_merge_groups"), 0);
        }
        vm.cancel_effect(&blocker.continuation, 105);
        let Step::Effects(batch) = started(busy.poll(&mut vm, 110).unwrap()) else {
            panic!()
        };
        assert_eq!(batch.merge_token.as_str(), "merge-2");
        assert_eq!(
            batch
                .branches
                .iter()
                .map(|branch| branch.request.effect_id.as_str())
                .collect::<Vec<_>>(),
            ["effect-3", "effect-4"]
        );
        assert!(
            batch
                .branches
                .iter()
                .all(|branch| branch.request.budget.deadline_at_ms == Some(200))
        );
        assert_eq!(vm.scheduler_pressure(110).unwrap().pending_dispatches, 2);
    }
}

#[test]
fn waiting_admission_can_continue_on_a_reopened_worker_without_restarting_accepted_work() {
    let path = JournalPath::new();
    let mut original = path.vm(true, 1, 10000);
    let blocker = effect(start(&mut original, FOCUS, 100, 1000));
    let mut admission = root(FOCUS, 100, 100, policy(4, 10, 20));
    waiting(admission.poll(&mut original, 100).unwrap());
    original.cancel_effect(&blocker.continuation, 105);
    drop(original);
    let mut reopened = path.vm(true, 1, 10000);
    let request = effect(started(admission.poll(&mut reopened, 110).unwrap()));
    assert_eq!(request.effect_id, "effect-2");
    assert_eq!(request.budget.deadline_at_ms, Some(200));
    drop(reopened);
    let mut worker = path.vm(true, 1, 10000);
    assert_eq!(
        admission.poll(&mut worker, 111).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(path.records("vm_dispatches"), 2);
    assert_eq!(worker.pending_count(), 1);
}

#[test]
fn simultaneous_waiting_handles_recheck_shared_capacity_on_every_due_attempt() {
    let path = JournalPath::new();
    let mut draining = path.vm(true, 3, 10000);
    let workers = (0..4)
        .map(|_| {
            (
                path.vm(true, 3, 10000),
                root(GROUP, 100, 1000, policy(4, 10, 20)),
            )
        })
        .collect::<Vec<_>>();
    effect(start(&mut draining, FOCUS, 100, 1000));
    let barrier = std::sync::Barrier::new(4);
    let results = std::thread::scope(|scope| {
        workers
            .into_iter()
            .map(|(mut vm, mut admission)| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let result = admission.poll(&mut vm, 100).unwrap();
                    (vm, admission, result)
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results
            .iter()
            .filter(|(_, _, result)| matches!(result, AdmissionPoll::Started(_)))
            .count(),
        1
    );
    assert_eq!(
        draining.scheduler_pressure(100).unwrap().pending_dispatches,
        3
    );
    let mut waiters = Vec::new();
    for (mut vm, mut admission, result) in results {
        if let AdmissionPoll::Waiting(wait) = result {
            assert_eq!(
                admission.poll(&mut vm, 100).unwrap(),
                AdmissionPoll::Waiting(wait)
            );
            assert_eq!(admission.attempts(), 1);
            waiters.push((vm, admission));
        }
    }
    assert_eq!(waiters.len(), 3);
    while let Some(lease) = draining.claim_effect(101, 50).unwrap() {
        let step = draining.acknowledge_effect(&lease, 102, receipt(&lease.request));
        assert!(!matches!(step, Step::Fault(_)), "{step:?}");
    }
    let barrier = std::sync::Barrier::new(3);
    let results = std::thread::scope(|scope| {
        waiters
            .into_iter()
            .map(|(mut vm, mut admission)| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    let result = admission.poll(&mut vm, 110).unwrap();
                    assert_eq!(admission.attempts(), 2);
                    result
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, AdmissionPoll::Started(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(result, AdmissionPoll::Waiting(_)))
            .count(),
        2
    );
    assert_eq!(
        draining.scheduler_pressure(110).unwrap().pending_dispatches,
        2
    );
}
