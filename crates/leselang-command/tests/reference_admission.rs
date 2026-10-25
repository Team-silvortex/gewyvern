use leselang_command::reference_admission::{
    AdmissionEnd, AdmissionPolicy, AdmissionPoll, ReferenceAdmissionHost, RootAdmission,
};
use leselang_command::{Step, Value};
use leselang_hir::{HirProgram, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, ScalarValue};
use leselang_syntax::parse;
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

#[derive(Default)]
struct Host {
    calls: Rc<RefCell<Vec<(u64, u64)>>>,
    outputs: VecDeque<Step>,
    program_name: Option<*const u8>,
    unwind: bool,
}

impl ReferenceAdmissionHost for Host {
    fn start_timed(
        &mut self,
        program: &HirProgram,
        principal: Principal,
        capabilities: CapabilitySet,
        expected_revision: Option<Revision>,
        now_ms: u64,
        timeout_ms: u64,
    ) -> Step {
        self.calls.borrow_mut().push((now_ms, timeout_ms));
        assert_eq!(principal, Principal::new("private-operator").unwrap());
        assert_eq!(capabilities, CapabilitySet::new(["ui.presentation"]));
        assert_eq!(expected_revision, Some(Revision(7)));
        if let Some(pointer) = self.program_name {
            assert_eq!(program.function.name.as_ptr(), pointer);
        }
        assert!(!self.unwind, "trusted host unwound");
        self.outputs.pop_front().expect("unexpected native replay")
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts: 3,
        base_delay_ms: 10,
        max_delay_ms: 20,
    }
}

fn root(policy: AdmissionPolicy) -> RootAdmission {
    RootAdmission::new(
        lower(&parse("fn main() = true")).unwrap(),
        Principal::new("private-operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        1_000,
        policy,
    )
    .unwrap()
}

fn fault(code: &str) -> Step {
    Step::Fault(Fault {
        code: code.into(),
        message: "original native refusal".into(),
    })
}

fn done() -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    })
}

#[test]
fn retry_borrows_original_program_and_preserves_deadline_authority_without_send_sync() {
    let program = lower(&parse("fn main() = true")).unwrap();
    let mut host = Host {
        program_name: Some(program.function.name.as_ptr()),
        outputs: [fault("LSV2501"), fault("LSV2501"), done()].into(),
        ..Host::default()
    };
    let mut admission = RootAdmission::new(
        program,
        Principal::new("private-operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        1_000,
        policy(),
    )
    .unwrap();
    for (now, retry, attempts) in [(100, 110, 1), (105, 110, 1), (110, 130, 2), (129, 130, 2)] {
        let AdmissionPoll::Waiting(wait) = admission
            .poll(&mut host as &mut dyn ReferenceAdmissionHost, now)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(
            (wait.retry_at_ms, wait.deadline_at_ms, wait.attempts),
            (retry, 1_100, attempts)
        );
    }
    assert_eq!(
        admission.poll(&mut host, 130).unwrap(),
        AdmissionPoll::Started(Box::new(done()))
    );
    assert_eq!(*host.calls.borrow(), [(100, 1_000), (110, 990), (130, 970)]);
    assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Started));
    assert!(!admission.cancel());
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
}

#[test]
fn only_exact_prepublication_pressure_retries_other_faults_move_unchanged() {
    for code in [
        "LSV2503",
        "LSH2001",
        "LSV2001",
        "SQLITE",
        "LSV2501-extra",
        "lsv2501",
    ] {
        let Step::Fault(error) = fault(code) else {
            unreachable!()
        };
        let pointer = error.message.as_ptr();
        let mut host = Host {
            outputs: [Step::Fault(error)].into(),
            ..Host::default()
        };
        let mut admission = root(policy());
        let AdmissionPoll::Rejected(error) = admission.poll(&mut host, 100).unwrap() else {
            panic!()
        };
        assert_eq!(error.code, code);
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Rejected));
        assert_eq!(
            admission.poll(&mut host, 101).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(host.calls.borrow().len(), 1);
    }
}

#[test]
fn accepted_payload_is_moved_without_cloning_or_replay() {
    let value = String::from("original-owned-result");
    let pointer = value.as_ptr();
    let mut host = Host {
        outputs: [Step::Done(Value::Scalar {
            value: ScalarValue::String(value),
        })]
        .into(),
        ..Host::default()
    };
    let mut admission = root(policy());
    let AdmissionPoll::Started(step) = admission.poll(&mut host, 100).unwrap() else {
        panic!()
    };
    let Step::Done(Value::Scalar {
        value: ScalarValue::String(value),
    }) = *step
    else {
        panic!()
    };
    assert_eq!(value.as_ptr(), pointer);
    assert_eq!(
        admission.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Finished
    );
}

#[test]
fn terminal_preflights_never_call_native_and_clock_errors_preserve_live_handle() {
    let mut host = Host::default();
    let mut admission = root(policy());
    let before = admission.status();
    assert_eq!(admission.poll(&mut host, 99).unwrap_err().code, "LSV2511");
    assert_eq!(admission.status(), before);
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap_err().code,
        "LSV2011"
    );
    assert_eq!(admission.status(), before);
    assert!(
        matches!(admission.poll(&mut host, 1_100).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
    );
    assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Expired));
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    let mut admission = root(policy());
    assert!(admission.cancel());
    assert!(!admission.cancel());
    assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Cancelled));
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert!(host.calls.borrow().is_empty());
}

#[test]
fn attempts_are_bounded_and_backpressure_can_be_cancelled_without_reentering_host() {
    for cancel in [false, true] {
        let mut admission = root(policy());
        let mut host = Host {
            outputs: [fault("LSV2501"), fault("LSV2501"), fault("LSV2501")].into(),
            ..Host::default()
        };
        assert!(matches!(
            admission.poll(&mut host, 100).unwrap(),
            AdmissionPoll::Waiting(_)
        ));
        if cancel {
            assert!(admission.cancel());
            assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Cancelled));
            assert_eq!(admission.attempts(), 1);
        } else {
            assert!(matches!(
                admission.poll(&mut host, 110).unwrap(),
                AdmissionPoll::Waiting(_)
            ));
            assert!(
                matches!(admission.poll(&mut host, 130).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2512")
            );
            assert_eq!(admission.terminal_reason(), Some(AdmissionEnd::Exhausted));
            assert_eq!(admission.attempts(), 3);
        }
        assert!(admission.is_finished());
        assert_eq!(
            admission.poll(&mut host, u64::MAX).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(host.calls.borrow().len(), admission.attempts() as usize);
    }
}

#[test]
fn uncertain_host_unwind_is_sealed_not_hidden_or_retried() {
    let mut admission = root(policy());
    let mut host = Host {
        unwind: true,
        ..Host::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || admission.poll(&mut host, 100)
        ))
        .is_err()
    );
    assert_eq!(
        admission.terminal_reason(),
        Some(AdmissionEnd::HostUncertain)
    );
    assert_eq!(admission.attempts(), 1);
    assert_eq!(
        admission.poll(&mut host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(host.calls.borrow().len(), 1);
}

#[test]
fn debug_is_redacted_and_core_conversion_preserves_wire_outcomes() {
    let admission = root(policy());
    for debug in [format!("{admission:?}"), format!("{admission:#?}")] {
        for private in ["private-operator", "ui.presentation", "main"] {
            assert!(!debug.contains(private));
        }
    }
    for core in [
        leselang_runtime_core::AdmissionPoll::Waiting(leselang_runtime_core::AdmissionWait {
            attempts: 1,
            retry_at_ms: 110,
            deadline_at_ms: 1_100,
        }),
        leselang_runtime_core::AdmissionPoll::Started(Box::new(done())),
        leselang_runtime_core::AdmissionPoll::Rejected(Fault {
            code: "NATIVE".into(),
            message: "original".into(),
        }),
        leselang_runtime_core::AdmissionPoll::Finished,
    ] {
        let wire = serde_json::to_vec(&core).unwrap();
        assert_eq!(
            serde_json::to_vec(&AdmissionPoll::from(core)).unwrap(),
            wire
        );
    }
}
