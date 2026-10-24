use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, VecDeque};

use leselang_command::ContinuationToken;
use leselang_command::continuation_identity::MAX_EFFECT_SEQUENCE;
use leselang_command::reference_allocation::{self, ReferenceAllocationHost};
use leselang_runtime_core::Fault;

fn fault() -> Fault {
    Fault {
        code: "native-allocation".into(),
        message: "original-owned-error".into(),
    }
}
fn exhausted() -> Fault {
    Fault {
        code: "LSV1003".into(),
        message: "continuation sequence is exhausted".into(),
    }
}

#[derive(Default)]
struct Host {
    next: u64,
    allocations: VecDeque<Result<u64, Fault>>,
    pending: BTreeSet<u64>,
    completed: BTreeSet<u64>,
    trace: RefCell<Vec<String>>,
    last_token: Cell<Option<(*const ContinuationToken, *const u8)>>,
    native_effects: usize,
    unwind_at: Option<&'static str>,
}
impl Host {
    fn event(&self, stage: &'static str, value: u64) {
        self.trace.borrow_mut().push(format!("{stage}:{value}"));
        assert_ne!(self.unwind_at, Some(stage), "hook unwind");
    }
    fn sequence(&self, token: &ContinuationToken) -> u64 {
        let sequence = token
            .as_str()
            .strip_prefix("continuation-")
            .unwrap()
            .parse::<u64>()
            .unwrap();
        assert_eq!(token.as_str(), format!("continuation-{sequence}"));
        assert_eq!(self.next, sequence + 1, "publish must precede cache checks");
        sequence
    }
    fn trace(&self) -> Vec<String> {
        self.trace.borrow().clone()
    }
}
impl ReferenceAllocationHost for Host {
    fn local_next(&self) -> u64 {
        self.event("local", self.next);
        self.next
    }
    fn allocate_sequence(&mut self, next: u64) -> Result<u64, Fault> {
        assert_eq!(next, self.next);
        self.native_effects += 1;
        self.event("allocate", next);
        self.allocations
            .pop_front()
            .expect("unexpected extra reservation")
    }
    fn publish_next(&mut self, next: u64) {
        self.next = next;
        self.event("publish", next);
    }
    fn pending_contains(&self, token: &ContinuationToken) -> bool {
        let sequence = self.sequence(token);
        self.last_token
            .set(Some((std::ptr::from_ref(token), token.as_str().as_ptr())));
        self.event("pending", sequence);
        self.pending.contains(&sequence)
    }
    fn completed_contains(&self, token: &ContinuationToken) -> bool {
        let sequence = self.sequence(token);
        assert_eq!(
            self.last_token.get(),
            Some((std::ptr::from_ref(token), token.as_str().as_ptr()))
        );
        self.event("completed", sequence);
        self.completed.contains(&sequence)
    }
}

#[test]
fn collisions_consume_fresh_native_reservations_after_watermark_and_short_circuit_pending() {
    let mut host = Host {
        next: 5,
        allocations: [Ok(7), Ok(8), Ok(9), Ok(10)].into(),
        pending: BTreeSet::from([7, 9]),
        completed: BTreeSet::from([8, 9]),
        ..Host::default()
    };
    let before = (host.pending.clone(), host.completed.clone());
    assert_eq!(
        reference_allocation::allocate_sequence(&mut host as &mut dyn ReferenceAllocationHost),
        Ok(10)
    );
    assert_eq!(host.next, 11);
    assert_eq!(host.native_effects, 4);
    assert_eq!((host.pending.clone(), host.completed.clone()), before);
    assert_eq!(
        host.trace(),
        [
            "local:5",
            "allocate:5",
            "publish:8",
            "pending:7",
            "local:8",
            "allocate:8",
            "publish:9",
            "pending:8",
            "completed:8",
            "local:9",
            "allocate:9",
            "publish:10",
            "pending:9",
            "local:10",
            "allocate:10",
            "publish:11",
            "pending:10",
            "completed:10",
        ]
    );
}

#[test]
fn range_checks_follow_native_effects_but_precede_local_publication_or_cache_reads() {
    for sequence in [0, 1, MAX_EFFECT_SEQUENCE, MAX_EFFECT_SEQUENCE + 1, u64::MAX] {
        let mut host = Host {
            next: 5,
            allocations: [Ok(sequence)].into(),
            ..Host::default()
        };
        let got = reference_allocation::allocate_sequence(&mut host);
        assert_eq!(host.native_effects, 1);
        if sequence == 0 || sequence > MAX_EFFECT_SEQUENCE {
            assert_eq!(got, Err(exhausted()));
            assert_eq!(host.next, 5);
            assert_eq!(host.trace(), ["local:5", "allocate:5"]);
        } else {
            assert_eq!(got, Ok(sequence));
            assert_eq!(host.next, sequence + 1);
            assert_eq!(host.trace().len(), 5);
        }
    }
}

#[test]
fn local_hint_is_passed_verbatim_and_never_prevalidated_clamped_or_max_merged() {
    for local in [0, 1, MAX_EFFECT_SEQUENCE + 1, u64::MAX] {
        let mut host = Host {
            next: local,
            allocations: [Ok(7)].into(),
            ..Host::default()
        };
        assert_eq!(reference_allocation::allocate_sequence(&mut host), Ok(7));
        assert_eq!(host.next, 8);
        assert_eq!(
            host.trace()[..2],
            [format!("local:{local}"), format!("allocate:{local}")]
        );
    }
}

#[test]
fn native_errors_keep_original_fault_ownership_and_do_not_rewind_consumed_collisions() {
    for collision in [false, true] {
        let error = fault();
        let pointer = error.message.as_ptr();
        let mut host = Host {
            next: 7,
            pending: BTreeSet::from([7]),
            ..Host::default()
        };
        if collision {
            host.allocations.push_back(Ok(7));
        }
        host.allocations.push_back(Err(error));
        let error = reference_allocation::allocate_sequence(&mut host).unwrap_err();
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(host.next, if collision { 8 } else { 7 });
        assert_eq!(host.native_effects, if collision { 2 } else { 1 });
        assert_eq!(host.trace().len(), if collision { 6 } else { 2 });
        assert!(host.pending.contains(&7));
    }
}

#[test]
fn occupied_last_sequence_advances_to_exhaustion_without_wrapping_or_reusing_it() {
    for pending in [false, true] {
        let mut host = Host {
            next: MAX_EFFECT_SEQUENCE,
            allocations: [Ok(MAX_EFFECT_SEQUENCE), Ok(MAX_EFFECT_SEQUENCE + 1)].into(),
            ..Host::default()
        };
        if pending {
            host.pending.insert(MAX_EFFECT_SEQUENCE);
        } else {
            host.completed.insert(MAX_EFFECT_SEQUENCE);
        }
        assert_eq!(
            reference_allocation::allocate_sequence(&mut host),
            Err(exhausted())
        );
        assert_eq!(host.next, MAX_EFFECT_SEQUENCE + 1);
        assert_eq!(host.native_effects, 2);
        assert_eq!(
            host.trace()
                .iter()
                .filter(|s| s.starts_with("publish:"))
                .count(),
            1
        );
    }
}

#[test]
fn hook_unwinds_preserve_prior_native_effects_and_watermark_without_retry() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for stage in ["local", "allocate", "publish", "pending", "completed"] {
        let mut host = Host {
            next: 7,
            allocations: [Ok(7)].into(),
            unwind_at: Some(stage),
            ..Host::default()
        };
        assert!(
            catch_unwind(AssertUnwindSafe(
                || reference_allocation::allocate_sequence(&mut host)
            ))
            .is_err()
        );
        assert_eq!(host.native_effects, usize::from(stage != "local"));
        assert_eq!(
            host.next,
            if matches!(stage, "local" | "allocate") {
                7
            } else {
                8
            }
        );
        assert_eq!(
            host.trace()
                .iter()
                .filter(|s| s.starts_with(&format!("{stage}:")))
                .count(),
            1
        );
        assert!(host.pending.is_empty() && host.completed.is_empty());
    }
}
