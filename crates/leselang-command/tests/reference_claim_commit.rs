use leselang_command::reference_claim_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, DispatchClaim, EffectRequest, SchedulerLimits,
    SchedulerPressure,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn request(n: u64) -> EffectRequest {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
        sequence: n,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<String>>>,
    fail: Option<usize>,
    unwind: bool,
    error: RefCell<Option<Fault>>,
}
impl Hooks {
    fn enter(&self, name: impl Into<String>) -> Result<(), Fault> {
        let index = self.trace.borrow().len();
        self.trace.borrow_mut().push(name.into());
        if self.fail == Some(index) {
            assert!(!self.unwind, "native unwind");
            return Err(self.error.borrow_mut().take().unwrap());
        }
        Ok(())
    }
}
struct Entry {
    request: EffectRequest,
    attempt: u32,
    retry_count: u32,
    expiry: Option<u64>,
    ready: u64,
    ack: bool,
    blocked: bool,
}
impl Entry {
    fn new(n: u64) -> Self {
        Self {
            request: request(n),
            attempt: 0,
            retry_count: 0,
            expiry: None,
            ready: 0,
            ack: false,
            blocked: false,
        }
    }
}
#[derive(Default)]
struct Memory {
    hooks: Hooks,
    entries: Vec<Entry>,
    full: bool,
    missing: bool,
}
impl MemoryClaimJournal for Memory {
    fn candidates(&self) -> impl Iterator<Item = MemoryClaimCandidate<'_>> {
        self.entries.iter().map(|e| MemoryClaimCandidate {
            token: &e.request.continuation.token,
            attempt: e.attempt,
            acknowledged: e.ack,
            ready_at_ms: e.ready,
            lease_expires_at_ms: e.expiry,
        })
    }
    fn sequence_ready(&self, t: &ContinuationToken) -> bool {
        self.hooks.enter(format!("graph:{}", t.as_str())).unwrap();
        !self
            .entries
            .iter()
            .find(|e| &e.request.continuation.token == t)
            .unwrap()
            .blocked
    }
    fn pressure(&self, _: u64, limits: SchedulerLimits) -> SchedulerPressure {
        self.hooks.enter("pressure").unwrap();
        SchedulerPressure {
            pending_dispatches: self.entries.len(),
            active_leases: if self.full {
                limits.max_active_leases
            } else {
                0
            },
            limits,
        }
    }
    fn dispatch(&mut self, t: &ContinuationToken) -> Option<MemoryLeaseState<'_>> {
        self.hooks.enter("dispatch").unwrap();
        if self.missing {
            return None;
        }
        self.entries
            .iter_mut()
            .find(|e| &e.request.continuation.token == t)
            .map(|e| MemoryLeaseState {
                request: &e.request,
                attempt: &mut e.attempt,
                retry_count: e.retry_count,
                lease_expires_at_ms: &mut e.expiry,
            })
    }
}
#[test]
fn memory_selects_attempt_then_numeric_age_and_skips_uncompetitive_graph_reads() {
    let mut h = Memory {
        entries: vec![
            Entry::new(10),
            Entry::new(2),
            Entry::new(3),
            Entry::new(4),
            Entry::new(5),
        ],
        ..Memory::default()
    };
    h.entries[2].attempt = 1;
    h.entries[3].ack = true;
    h.entries[4].expiry = Some(11);
    let source = h.entries[1].request.effect_id.as_ptr();
    let DispatchClaim::Leased(lease) =
        claim_memory(&mut h, 10, 20, SchedulerLimits::default()).unwrap()
    else {
        panic!()
    };
    assert_eq!(lease.request, request(2));
    assert_eq!(lease.attempt, 1);
    assert_eq!(
        *h.hooks.trace.borrow(),
        [
            "graph:continuation-10",
            "graph:continuation-2",
            "pressure",
            "dispatch"
        ]
    );
    assert_eq!(h.entries[1].request.effect_id.as_ptr(), source);
    assert_eq!(h.entries[1].expiry, Some(20));
    assert!(h.entries[0].expiry.is_none());
    assert_eq!(h.entries[2].attempt, 1);
}
#[test]
fn memory_idle_precedes_pressure_and_busy_owner_never_reads_the_request() {
    let mut h = Memory {
        entries: vec![Entry::new(1)],
        full: true,
        ..Memory::default()
    };
    h.entries[0].ready = 11;
    assert!(matches!(
        claim_memory(&mut h, 10, 20, SchedulerLimits::default()).unwrap(),
        DispatchClaim::Idle
    ));
    assert!(h.hooks.trace.borrow().is_empty());
    h.entries[0].ready = 10;
    h.entries[0].request.required_capability = "invalid".into();
    assert!(matches!(
        claim_memory(&mut h, 10, 20, SchedulerLimits::default()).unwrap(),
        DispatchClaim::Backpressured(_)
    ));
    assert_eq!(
        *h.hooks.trace.borrow(),
        ["graph:continuation-1", "pressure"]
    );
    assert_eq!(h.entries[0].attempt, 0);
}
#[test]
fn memory_blocked_minimum_does_not_hide_ready_work_and_expiry_is_inclusive() {
    let mut h = Memory {
        entries: vec![Entry::new(1), Entry::new(2)],
        ..Memory::default()
    };
    h.entries[0].blocked = true;
    h.entries[1].expiry = Some(10);
    h.entries[1].retry_count = 2;
    let DispatchClaim::Leased(l) =
        claim_memory(&mut h, 10, 20, SchedulerLimits::default()).unwrap()
    else {
        panic!()
    };
    assert_eq!(l.request, request(2));
    assert_eq!(l.retry_count, 2);
    assert_eq!(
        *h.hooks.trace.borrow(),
        [
            "graph:continuation-1",
            "graph:continuation-2",
            "pressure",
            "dispatch"
        ]
    );
}
#[test]
fn memory_missing_or_exhausted_candidate_never_mutates_lease_fields() {
    for missing in [true, false] {
        let mut h = Memory {
            entries: vec![Entry::new(1)],
            missing,
            ..Memory::default()
        };
        h.entries[0].attempt = u32::MAX;
        h.entries[0].expiry = Some(10);
        let err = claim_memory(&mut h, 10, 20, SchedulerLimits::default()).unwrap_err();
        assert_eq!(err.code, if missing { "LSV4007" } else { "LSV4017" });
        assert_eq!(h.entries[0].attempt, u32::MAX);
        assert_eq!(h.entries[0].expiry, Some(10));
    }
}
struct Durable {
    hooks: Hooks,
    full: bool,
    eligible: bool,
    candidate: Option<StoredClaimCandidate>,
}
impl Durable {
    fn new(route: usize) -> Self {
        let r = request(7);
        Self {
            hooks: Hooks::default(),
            full: route < 2,
            eligible: route == 1,
            candidate: if route == 3 {
                Some(StoredClaimCandidate {
                    token: r.continuation.token.as_str().into(),
                    request: serde_json::to_vec(&r).unwrap(),
                    attempt: 2,
                    retry_count: 1,
                    image: serde_json::to_vec(&r.continuation).unwrap(),
                })
            } else {
                None
            },
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop".into());
    }
}
impl DurableClaimTransaction for Durable {
    fn capacity_exhausted(&mut self, _: usize) -> Result<bool, Fault> {
        self.hooks.enter("capacity")?;
        Ok(self.full)
    }
    fn has_candidate(&mut self) -> Result<bool, Fault> {
        self.hooks.enter("exists")?;
        Ok(self.eligible)
    }
    fn pressure(&mut self, _: u64, limits: SchedulerLimits) -> Result<SchedulerPressure, Fault> {
        self.hooks.enter("pressure")?;
        Ok(SchedulerPressure {
            pending_dispatches: 2,
            active_leases: limits.max_active_leases,
            limits,
        })
    }
    fn candidate(&mut self) -> Result<Option<StoredClaimCandidate>, Fault> {
        self.hooks.enter("candidate")?;
        Ok(self.candidate.take())
    }
    fn encode_image(&mut self, image: &ContinuationImage) -> Result<Vec<u8>, Fault> {
        self.hooks.enter("encode")?;
        Ok(serde_json::to_vec(image).unwrap())
    }
    fn lease(&mut self, token: &str, attempt: u32) -> Result<(), Fault> {
        assert_eq!(token, "continuation-7");
        assert_eq!(attempt, 3);
        self.hooks.enter("lease")
    }
    fn commit(self, reason: ClaimCommit) -> Result<(), Fault> {
        self.hooks.enter(format!("commit:{reason:?}"))
    }
}
fn run(h: Durable) -> Result<DispatchClaim, Fault> {
    claim_durable(h, 10, 20, SchedulerLimits::default())
}
fn trace(route: usize) -> Vec<String> {
    match route {
        0 => vec!["capacity", "exists", "commit:CapacityLimited", "drop"],
        1 => vec![
            "capacity",
            "exists",
            "pressure",
            "commit:CapacityLimited",
            "drop",
        ],
        2 => vec!["capacity", "candidate", "commit:Idle", "drop"],
        _ => vec![
            "capacity",
            "candidate",
            "encode",
            "lease",
            "commit:Leased",
            "drop",
        ],
    }
    .into_iter()
    .map(str::to_owned)
    .collect()
}
#[test]
fn durable_all_routes_keep_commit_reason_and_capacity_fast_path_cold() {
    for route in 0..4 {
        let h = Durable::new(route);
        let seen = h.hooks.trace.clone();
        let result = run(h).unwrap();
        assert_eq!(*seen.borrow(), trace(route));
        match route {
            0 | 2 => assert_eq!(result, DispatchClaim::Idle),
            1 => assert!(matches!(result, DispatchClaim::Backpressured(_))),
            _ => {
                let DispatchClaim::Leased(l) = result else {
                    panic!()
                };
                assert_eq!(l.request, request(7));
                assert_eq!(
                    (l.attempt, l.retry_count, l.lease_expires_at_ms),
                    (3, 1, 20)
                );
            }
        }
    }
}
#[test]
fn durable_decode_identity_image_and_counter_faults_precede_updates() {
    for kind in 0..9 {
        let mut h = Durable::new(3);
        let c = h.candidate.as_mut().unwrap();
        match kind {
            0 => c.request = b"{".to_vec(),
            1 => {
                let mut r = request(7);
                r.required_capability = "invalid".into();
                c.request = serde_json::to_vec(&r).unwrap();
            }
            2 => c.token = "continuation-8".into(),
            3 => c.image.clear(),
            4 => c.attempt = -1,
            5 => c.attempt = i64::MAX,
            6 => c.attempt = 100,
            7 => c.retry_count = -1,
            8 => c.retry_count = i64::MAX,
            _ => unreachable!(),
        }
        let seen = h.hooks.trace.clone();
        assert!(run(h).is_err());
        let mut expected = vec!["capacity", "candidate"];
        if kind >= 3 {
            expected.push("encode");
        }
        expected.push("drop");
        assert_eq!(*seen.borrow(), expected);
    }
}
#[test]
fn durable_attempt_exhaustion_precedes_invalid_retry_count() {
    let mut h = Durable::new(3);
    let c = h.candidate.as_mut().unwrap();
    c.attempt = 100;
    c.retry_count = -1;
    assert_eq!(run(h).unwrap_err().code, "LSV4017");
}
#[test]
fn durable_each_hook_fault_or_unwind_drops_once_without_retry_or_lease_publication() {
    for route in 0..4 {
        let expected = trace(route);
        for at in 0..expected.len() - 1 {
            for unwind in [false, true] {
                let mut h = Durable::new(route);
                h.hooks.fail = Some(at);
                h.hooks.unwind = unwind;
                h.hooks.error.replace(Some(Fault {
                    code: "NATIVE".into(),
                    message: "original fault".into(),
                }));
                let pointer = h.hooks.error.borrow().as_ref().unwrap().message.as_ptr();
                let seen = h.hooks.trace.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(h)));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                }
                let mut prefix = expected[..=at].to_vec();
                prefix.push("drop".into());
                assert_eq!(*seen.borrow(), prefix);
            }
        }
    }
}
