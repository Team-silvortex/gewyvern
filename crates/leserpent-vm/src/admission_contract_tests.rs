use super::*;

const FOCUS: &str = r#"bind(result: ui.focus(node_id: "target"), body: true)"#;
const FORMS: [&str; 6] = [
    "true",
    r#"ui.focus(node_id: "target")"#,
    FOCUS,
    r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    r#"all(left: ui.focus(node_id: "a"), right: ui.focus(node_id: "b"))"#,
    "runtime.list()",
];

fn program(expression: &str) -> HirProgram {
    leserpent_hir::lower(&leselang_syntax::parse(&format!(
        "fn main() = {expression}"
    )))
    .unwrap()
}

fn pair(expression: &str, policy: AdmissionPolicy) -> (RootAdmission, frozen::RootAdmission) {
    (
        RootAdmission::new(
            program(expression),
            Principal::new("operator").unwrap(),
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            Some(Revision(7)),
            100,
            200,
            policy,
        )
        .unwrap(),
        frozen::RootAdmission::new(
            program(expression),
            Principal::new("operator").unwrap(),
            CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
            Some(Revision(7)),
            100,
            200,
            policy,
        )
        .unwrap(),
    )
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts: 3,
        base_delay_ms: 10,
        max_delay_ms: 20,
    }
}

fn compare_handles(actual: &RootAdmission, expected: &frozen::RootAdmission) {
    assert_eq!(actual.status(), expected.status());
    assert_eq!(actual.attempts(), expected.attempts());
    assert_eq!(actual.is_finished(), expected.is_finished());
    assert_eq!(actual.terminal_reason(), expected.terminal_reason());
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    assert_eq!(format!("{actual:#?}"), format!("{expected:#?}"));
}

fn compare_vms(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(actual.scheduler_pressure(1), expected.scheduler_pressure(1));
}

fn run_case(expression: &str, mode: u8, actual: &mut Vm, expected: &mut Vm) {
    if matches!(mode, 1 | 2 | 4) {
        for vm in [&mut *actual, &mut *expected] {
            for _ in 0..vm.scheduler_limits.max_pending_dispatches {
                assert!(matches!(
                    vm.start_timed(
                        &program(FOCUS),
                        Principal::new("operator").unwrap(),
                        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                        Some(Revision(7)),
                        100,
                        if mode == 2 { 15 } else { 1_000 }
                    ),
                    Step::Effect(_)
                ));
            }
        }
    }
    if mode == 5 {
        actual.fuel_limit = 0;
        expected.fuel_limit = 0;
    }
    let (mut got, mut want) = pair(expression, policy());
    if mode == 3 {
        assert_eq!(got.cancel(), want.cancel());
    }
    compare_handles(&got, &want);
    for now in [99, 100, 105, 110, 120, 130, 299, 300, u64::MAX] {
        let outcome = got.poll(actual, now);
        let frozen_outcome = want.poll(expected, now);
        assert_eq!(
            serde_json::to_vec(&outcome).unwrap(),
            serde_json::to_vec(&frozen_outcome).unwrap(),
            "{expression} mode={mode} clock={now}"
        );
        compare_handles(&got, &want);
        compare_vms(actual, expected);
        if mode == 4 && now == 100 {
            assert_eq!(got.cancel(), want.cancel());
            compare_handles(&got, &want);
        }
    }
    assert_eq!(got.cancel(), want.cancel());
    // Accepted work stays native-owned even though the local handle is terminal.
    for _ in 0..3 {
        assert_eq!(
            actual.claim_effect(301, 100),
            expected.claim_effect(301, 100)
        );
    }
    compare_vms(actual, expected);
}

#[test]
fn root_admission_memory_matches_frozen_lifecycle_and_native_side_effects() {
    let mut cases = 0;
    for expression in FORMS {
        for mode in 0..6 {
            for pending in [1, 4] {
                let limits = SchedulerLimits {
                    max_pending_dispatches: pending,
                    max_active_leases: 1,
                };
                let mut actual = Vm::new_with_limits(100, limits).unwrap();
                let mut expected = Vm::new_with_limits(100, limits).unwrap();
                run_case(expression, mode, &mut actual, &mut expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 72);
}

#[test]
fn root_admission_constructor_matches_frozen_validation_precedence() {
    assert_eq!(MAX_ADMISSION_ATTEMPTS, frozen::MAX_ADMISSION_ATTEMPTS);
    assert_eq!(MAX_ADMISSION_DELAY_MS, frozen::MAX_ADMISSION_DELAY_MS);
    let mut cases = 0;
    for policy in [
        policy(),
        AdmissionPolicy {
            max_attempts: 0,
            ..policy()
        },
        AdmissionPolicy {
            base_delay_ms: 0,
            ..policy()
        },
        AdmissionPolicy {
            max_delay_ms: 1,
            ..policy()
        },
    ] {
        for (now, timeout) in [
            (100, 1_000),
            (100, 0),
            (100, MAX_EFFECT_DEADLINE_MS + 1),
            (i64::MAX as u64, 1),
            (u64::MAX, 1),
        ] {
            let got = RootAdmission::new(
                program("true"),
                Principal::new("operator").unwrap(),
                CapabilitySet::default(),
                None,
                now,
                timeout,
                policy,
            );
            let want = frozen::RootAdmission::new(
                program("true"),
                Principal::new("operator").unwrap(),
                CapabilitySet::default(),
                None,
                now,
                timeout,
                policy,
            );
            match (got, want) {
                (Ok(got), Ok(want)) => compare_handles(&got, &want),
                (Err(got), Err(want)) => assert_eq!(got, want),
                _ => panic!("constructor mismatch"),
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 20);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn root_admission_sqlite_matches_frozen_lifecycle_and_reopen() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let path = std::env::temp_dir().join(format!(
        "leselang-admission-parity-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).unwrap();
    let scratch = Scratch(path);
    let mut cases = 0;
    for expression in FORMS {
        for mode in 0..6 {
            for pending in [1, 4] {
                let limits = SchedulerLimits {
                    max_pending_dispatches: pending,
                    max_active_leases: 1,
                };
                let actual_path = scratch.0.join(format!("{cases}-actual.sqlite3"));
                let expected_path = scratch.0.join(format!("{cases}-expected.sqlite3"));
                let mut actual = Vm::open_journal_with_limits(&actual_path, 100, limits).unwrap();
                let mut expected =
                    Vm::open_journal_with_limits(&expected_path, 100, limits).unwrap();
                run_case(expression, mode, &mut actual, &mut expected);
                drop(actual);
                drop(expected);
                let mut actual = Vm::open_journal_with_limits(&actual_path, 100, limits).unwrap();
                let mut expected =
                    Vm::open_journal_with_limits(&expected_path, 100, limits).unwrap();
                compare_vms(&actual, &expected);
                assert_eq!(
                    actual.claim_effect(500, 100),
                    expected.claim_effect(500, 100)
                );
                compare_vms(&actual, &expected);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 72);
}

// Frozen pre-extraction adapter from 49648886. Native start/journal and the generic
// admission core are intentionally shared: this proves adapter parity, not an
// independent scheduler, storage implementation or host-neutral durable recovery.
mod frozen {
    use leselang_host_contract::{CapabilitySet, Principal, Revision};
    use leselang_runtime_core::{Admission, AdmissionAdapter, AdmissionAttempt};
    use leserpent_hir::HirProgram;
    use serde::Serialize;

    use crate::{Fault, Step, Vm};

    pub use leselang_runtime_core::{
        AdmissionEnd, AdmissionPolicy, AdmissionStatus, AdmissionWait, MAX_ADMISSION_ATTEMPTS,
        MAX_ADMISSION_DELAY_MS,
    };

    /// Concrete compatibility DTO; admission state and scheduling remain in the core.
    /// Ignoring accepted output never cancels the VM work it represents.
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// use leselang_vm::AdmissionPoll;
    /// fn poll_result() -> AdmissionPoll { AdmissionPoll::Finished }
    /// poll_result();
    /// ```
    #[must_use = "handle the admission outcome; accepted VM work is not cancelled by ignoring it"]
    #[derive(Debug, Eq, PartialEq, Serialize)]
    #[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
    pub enum AdmissionPoll {
        Waiting(AdmissionWait),
        Started(Box<Step>),
        Rejected(Fault),
        Finished,
    }

    impl From<leselang_runtime_core::AdmissionPoll<Box<Step>>> for AdmissionPoll {
        fn from(outcome: leselang_runtime_core::AdmissionPoll<Box<Step>>) -> Self {
            match outcome {
                leselang_runtime_core::AdmissionPoll::Waiting(wait) => Self::Waiting(wait),
                leselang_runtime_core::AdmissionPoll::Started(step) => Self::Started(step),
                leselang_runtime_core::AdmissionPoll::Rejected(error) => Self::Rejected(error),
                leselang_runtime_core::AdmissionPoll::Finished => Self::Finished,
            }
        }
    }

    struct AdmissionRoot {
        program: HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
    }

    /// The reference VM's authority-capturing adapter to the host-neutral lifecycle.
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// use leserpent_hir::lower;
    /// use leselang_host_contract::{CapabilitySet, Principal};
    /// use leselang_syntax::parse;
    /// use leselang_vm::{AdmissionPolicy, RootAdmission};
    /// let program = lower(&parse("fn main() = true")).unwrap();
    /// RootAdmission::new(
    ///     program, Principal::new("operator").unwrap(), CapabilitySet::default(),
    ///     None, 0, 1_000, AdmissionPolicy::default(),
    /// ).unwrap();
    /// ```
    #[must_use = "retain or poll the root admission; dropping it discards pending input"]
    #[derive(Debug)]
    pub struct RootAdmission {
        inner: Admission<AdmissionRoot>,
    }

    impl RootAdmission {
        /// Captures immutable input and authority without entering a VM or reserving identity.
        ///
        /// ```
        /// use leserpent_hir::lower;
        /// use leselang_host_contract::{CapabilitySet, Principal, Revision};
        /// use leselang_syntax::parse;
        /// use leselang_vm::{AdmissionPolicy, AdmissionPoll, RootAdmission, Step, Vm};
        ///
        /// let program = lower(&parse("fn main() = true")).unwrap();
        /// let mut admission = RootAdmission::new(
        ///     program, Principal::new("operator").unwrap(), CapabilitySet::default(),
        ///     Some(Revision(7)), 100, 1_000, AdmissionPolicy::default(),
        /// ).unwrap();
        /// let mut vm = Vm::default();
        /// assert!(matches!(admission.poll(&mut vm, 100).unwrap(),
        ///     AdmissionPoll::Started(step) if matches!(*step, Step::Done(_))));
        /// assert_eq!(admission.poll(&mut vm, 101).unwrap(), AdmissionPoll::Finished);
        /// ```
        pub fn new(
            program: HirProgram,
            principal: Principal,
            capabilities: CapabilitySet,
            expected_revision: Option<Revision>,
            now_ms: u64,
            timeout_ms: u64,
            policy: AdmissionPolicy,
        ) -> Result<Self, Fault> {
            let input = AdmissionRoot {
                program,
                principal,
                capabilities,
                expected_revision,
            };
            Ok(Self {
                inner: Admission::new(input, now_ms, timeout_ms, policy)?,
            })
        }

        pub fn attempts(&self) -> u32 {
            self.inner.attempts()
        }

        pub fn is_finished(&self) -> bool {
            self.inner.is_finished()
        }

        /// Observes scheduling metadata without evaluating HIR or entering the VM/journal.
        pub fn status(&self) -> AdmissionStatus {
            self.inner.status()
        }

        /// Observes only why admission ended, never accepted work or a replayable result.
        pub fn terminal_reason(&self) -> Option<AdmissionEnd> {
            self.inner.terminal_reason()
        }

        /// Cancels only a not-yet-admitted root; admitted work belongs to the VM/host.
        pub fn cancel(&mut self) -> bool {
            self.inner.cancel()
        }

        /// Clock errors leave the active handle unchanged; terminal outcomes consume it.
        pub fn poll(&mut self, vm: &mut Vm, now_ms: u64) -> Result<AdmissionPoll, Fault> {
            self.inner.poll(vm, now_ms).map(AdmissionPoll::from)
        }
    }

    impl AdmissionAdapter<AdmissionRoot> for Vm {
        type Started = Box<Step>;

        fn try_admit(
            &mut self,
            input: &AdmissionRoot,
            now_ms: u64,
            deadline_at_ms: u64,
        ) -> AdmissionAttempt<Self::Started> {
            let step = self.start_timed(
                &input.program,
                input.principal.clone(),
                input.capabilities.clone(),
                input.expected_revision,
                now_ms,
                deadline_at_ms - now_ms,
            );
            match step {
                // Only the VM's transactional, pre-publication capacity rejection is retryable.
                Step::Fault(error) if error.code == "LSV2501" => {
                    AdmissionAttempt::Backpressured(error)
                }
                Step::Fault(error) => AdmissionAttempt::Rejected(error),
                step => AdmissionAttempt::Started(Box::new(step)),
            }
        }
    }
}
