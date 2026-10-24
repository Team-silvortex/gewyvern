use leselang_command::reference_reply::CompletionLease;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_settlement::{self, ReferenceSettlementHost};
use leselang_command::{ContinuationImage, DispatchLease, EffectRequest, Step, Value};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;

fn request(sequence: u64) -> EffectRequest {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "target".into(),
        },
        result_type: Type::UiFocus,
        sequence,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
}

fn payload(step: &Step) -> *const u8 {
    match step {
        Step::Done(Value::UiFocus { node_id }) => node_id.as_ptr(),
        Step::Fault(error) => error.message.as_ptr(),
        _ => panic!(),
    }
}

struct Host {
    trace: Vec<&'static str>,
    image: *const ContinuationImage,
    parent: *const EffectRequest,
    next: *const EffectRequest,
    lease: Option<*const DispatchLease>,
    proposed: *const u8,
    winner: Option<Step>,
    winner_pointer: *const u8,
    failure: Option<Fault>,
    fail_at: Option<&'static str>,
    unwind_at: Option<&'static str>,
    committed: bool,
}

impl Host {
    fn check_lease(&self, lease: CompletionLease<'_>) {
        assert_eq!(
            lease.map(|(lease, _)| std::ptr::from_ref(lease)),
            self.lease
        );
        if let Some((lease, now)) = lease {
            assert_eq!(
                (
                    lease.attempt,
                    lease.retry_count,
                    lease.lease_expires_at_ms,
                    now
                ),
                (3, 2, 90, 12)
            );
        }
    }
    fn commit(&mut self, stage: &'static str, proposed: &Step) -> Result<Step, Fault> {
        self.trace.push(stage);
        assert_eq!(payload(proposed), self.proposed);
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        self.committed = true;
        assert_ne!(
            self.unwind_at,
            Some(stage),
            "commit hook may unwind after committing"
        );
        Ok(self.winner.take().unwrap())
    }
}

impl ReferenceSettlementHost for Host {
    fn record_completed(
        &mut self,
        image: &ContinuationImage,
        proposed: &Step,
    ) -> Result<Step, Fault> {
        assert_eq!(std::ptr::from_ref(image), self.image);
        assert!(self.lease.is_none());
        self.commit("completed", proposed)
    }
    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now: u64,
        proposed: &Step,
    ) -> Result<Step, Fault> {
        self.check_lease(Some((lease, now)));
        self.commit("dispatch", proposed)
    }
    fn record_successor(
        &mut self,
        parent: &EffectRequest,
        proposed: &Step,
        next: &EffectRequest,
        lease: CompletionLease<'_>,
    ) -> Result<Step, Fault> {
        assert_eq!(std::ptr::from_ref(parent), self.parent);
        assert_eq!(std::ptr::from_ref(next), self.next);
        self.check_lease(lease);
        self.commit("successor", proposed)
    }
    fn publish_completed(&mut self, image: &ContinuationImage, authoritative: Step) -> Step {
        self.trace.push("publish");
        assert!(self.committed);
        assert_eq!(std::ptr::from_ref(image), self.image);
        assert_eq!(payload(&authoritative), self.winner_pointer);
        assert_ne!(self.unwind_at, Some("publish"), "publication hook unwind");
        if self.fail_at == Some("publish") {
            return Step::Fault(self.failure.take().unwrap());
        }
        authoritative
    }
}

fn exercise(mode: usize, fail_at: Option<&'static str>, unwind_at: Option<&'static str>) {
    let parent = request(7);
    let next = request(8);
    let lease = DispatchLease {
        request: parent.clone(),
        attempt: 3,
        retry_count: 2,
        lease_expires_at_ms: 90,
    };
    let authority = (mode % 2 == 1).then_some((&lease, 12));
    let proposed = Step::Done(Value::UiFocus {
        node_id: "proposed".into(),
    });
    let winner = Step::Done(Value::UiFocus {
        node_id: "peer-winner".into(),
    });
    let failure = Fault {
        code: "native-refusal".into(),
        message: "original-owned-fault".into(),
    };
    let fault_pointer = failure.message.as_ptr();
    let mut host = Host {
        trace: vec![],
        image: std::ptr::from_ref(&parent.continuation),
        parent: std::ptr::from_ref(&parent),
        next: std::ptr::from_ref(&next),
        lease: authority.map(|(l, _)| std::ptr::from_ref(l)),
        proposed: payload(&proposed),
        winner_pointer: payload(&winner),
        winner: Some(winner),
        failure: Some(failure),
        fail_at,
        unwind_at,
        committed: false,
    };
    let saved = serde_json::to_vec(&(&parent, &next, &lease)).unwrap();
    let route = match mode {
        0 => "completed",
        1 => "dispatch",
        _ => "successor",
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let host: &mut dyn ReferenceSettlementHost = &mut host;
        if mode < 2 {
            reference_settlement::record_result(&parent.continuation, authority, proposed, host)
        } else {
            reference_settlement::commit_successor(
                &parent.continuation,
                authority,
                &parent,
                &proposed,
                &next,
                host,
            )
        }
    }));
    assert_eq!(
        serde_json::to_vec(&(&parent, &next, &lease)).unwrap(),
        saved
    );
    if unwind_at.is_some() {
        assert!(result.is_err());
        assert!(host.committed);
    } else {
        let step = result.unwrap();
        if fail_at.is_some() {
            assert_eq!(payload(&step), fault_pointer);
            assert!(matches!(step, Step::Fault(ref e) if e.code == "native-refusal"));
        } else {
            assert_eq!(payload(&step), host.winner_pointer);
            assert!(
                matches!(step, Step::Done(Value::UiFocus { ref node_id }) if node_id == "peer-winner")
            );
        }
        assert_eq!(host.committed, fail_at != Some(route));
    }
    let stops_at_commit = fail_at == Some(route) || unwind_at == Some(route);
    assert_eq!(
        host.trace,
        if stops_at_commit {
            vec![route]
        } else {
            vec![route, "publish"]
        }
    );
}

#[test]
fn original_borrows_and_owned_native_winner_survive_all_four_settlement_routes() {
    for mode in 0..4 {
        exercise(mode, None, None);
    }
}

#[test]
fn native_commit_refusal_preserves_original_fault_and_never_publishes_or_retries() {
    for (mode, route) in ["completed", "dispatch", "successor", "successor"]
        .into_iter()
        .enumerate()
    {
        exercise(mode, Some(route), None);
    }
}

#[test]
fn publication_fault_is_not_a_commit_failure_or_a_reason_to_retry() {
    for mode in 0..4 {
        exercise(mode, Some("publish"), None);
    }
}

#[test]
fn commit_hook_unwind_does_not_publish_retry_or_undo_native_side_effects() {
    for (mode, route) in ["completed", "dispatch", "successor", "successor"]
        .into_iter()
        .enumerate()
    {
        exercise(mode, None, Some(route));
    }
}

#[test]
fn publication_unwind_does_not_resettle_or_undo_committed_state() {
    for mode in 0..4 {
        exercise(mode, None, Some("publish"));
    }
}
