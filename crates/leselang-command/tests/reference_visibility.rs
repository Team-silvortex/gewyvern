use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_settlement::visibility::{
    self, EffectObservation, MergeProgress, ReferenceCompletionCacheHost,
    ReferenceGroupVisibilityHost,
};
use leselang_command::{ContinuationToken, EffectRequest, Step};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, Type};

fn token(n: u64) -> ContinuationToken {
    ContinuationToken::from_wire(format!("continuation-{n}"))
}
fn fault() -> Fault {
    Fault {
        code: "native-code".into(),
        message: "original-owned-result".into(),
    }
}
fn step() -> Step {
    Step::Fault(fault())
}
fn payload(step: &Step) -> *const u8 {
    let Step::Fault(e) = step else { panic!() };
    e.message.as_ptr()
}
fn request() -> Box<EffectRequest> {
    Box::new(
        build_effect_request(ReservedEffectInput {
            effect: &Effect::UiFocus {
                node_id: "original-node".into(),
            },
            result_type: Type::UiFocus,
            sequence: 7,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            expected_revision: None,
            deadline_ms: 1000,
            deadline_at_ms: None,
            fuel_remaining: 100,
        })
        .unwrap(),
    )
}

#[derive(Default)]
struct Host {
    trace: RefCell<Vec<String>>,
    pending: BTreeSet<ContinuationToken>,
    completed: BTreeMap<ContinuationToken, Step>,
    exists: bool,
    observation: Option<EffectObservation>,
    progress: Option<MergeProgress>,
    error: Option<Fault>,
    fail_at: Option<&'static str>,
    unwind_at: Option<&'static str>,
    original_token: Option<*const ContinuationToken>,
    original_request: Option<*const EffectRequest>,
    original_step: Option<*const u8>,
}
impl Host {
    fn event(&self, name: &str, token: &ContinuationToken) {
        self.trace
            .borrow_mut()
            .push(format!("{name}:{}", token.as_str()));
    }
    fn check_token(&self, token: &ContinuationToken) {
        if let Some(pointer) = self.original_token {
            assert_eq!(std::ptr::from_ref(token), pointer);
        }
    }
    fn read(&mut self, name: &'static str, token: &ContinuationToken) -> Result<(), Fault> {
        self.check_token(token);
        self.event(name, token);
        assert_ne!(self.unwind_at, Some(name), "read unwind");
        if self.fail_at == Some(name) {
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
    fn remove(&mut self, token: &ContinuationToken) -> bool {
        self.event("remove", token);
        let removed = self.pending.remove(token);
        assert_ne!(
            self.unwind_at,
            Some("remove"),
            "unwind after pending removal"
        );
        removed
    }
    fn trace(&self) -> Vec<String> {
        self.trace.borrow().clone()
    }
}
impl ReferenceCompletionCacheHost for Host {
    fn has_completed(&self, token: &ContinuationToken) -> bool {
        self.check_token(token);
        self.event("has", token);
        self.completed.contains_key(token)
    }
    fn completed_exists(&mut self, token: &ContinuationToken) -> Result<bool, Fault> {
        self.read("exists", token)?;
        Ok(self.exists)
    }
    fn observe_effect(&mut self, token: &ContinuationToken) -> Result<EffectObservation, Fault> {
        self.read("observe", token)?;
        Ok(self.observation.take().unwrap())
    }
    fn remove_completed(&mut self, token: &ContinuationToken) {
        self.check_token(token);
        self.event("evict", token);
        self.completed.remove(token);
        assert_ne!(self.unwind_at, Some("evict"), "unwind after eviction");
    }
    fn remove_pending(&mut self, token: &ContinuationToken) {
        self.check_token(token);
        self.remove(token);
    }
    fn insert_completed(&mut self, token: &ContinuationToken, step: Step) {
        self.check_token(token);
        self.event("insert", token);
        if let Some(pointer) = self.original_step {
            assert_eq!(payload(&step), pointer);
        }
        self.completed.insert(token.clone(), step);
        assert_ne!(self.unwind_at, Some("insert"), "unwind after insertion");
    }
}
impl ReferenceGroupVisibilityHost for Host {
    fn merge_progress(&mut self, token: &ContinuationToken) -> Result<MergeProgress, Fault> {
        self.read("merge", token)?;
        Ok(self.progress.take().unwrap())
    }
    fn remove_pending(&mut self, token: &ContinuationToken) -> bool {
        self.remove(token)
    }
    fn cache_next(&mut self, request: &EffectRequest) {
        if let Some(pointer) = self.original_request {
            assert_eq!(std::ptr::from_ref(request), pointer);
        }
        self.event("next", &request.continuation.token);
        self.pending.insert(request.continuation.token.clone());
        assert_ne!(self.unwind_at, Some("next"), "unwind after next insertion");
    }
    fn cache_merged_if_absent(&mut self, token: ContinuationToken, step: &Step) {
        if let Some(pointer) = self.original_step {
            assert_eq!(payload(step), pointer);
        }
        self.event("merged", &token);
        self.completed.entry(token).or_insert_with(|| step.clone());
        assert_ne!(
            self.unwind_at,
            Some("merged"),
            "unwind after merged insertion"
        );
    }
    fn debug_assert_merge_result(&mut self, token: &ContinuationToken) {
        self.event("debug", token);
        assert_ne!(
            self.unwind_at,
            Some("debug"),
            "diagnostic unwind after publication"
        );
    }
}

#[test]
fn cached_hit_checks_existence_without_refresh_or_observation_after_eviction() {
    let token = token(1);
    for exists in [false, true] {
        let cached = step();
        let pointer = payload(&cached);
        let mut host = Host {
            exists,
            original_token: Some(std::ptr::from_ref(&token)),
            pending: BTreeSet::from([token.clone()]),
            completed: BTreeMap::from([(token.clone(), cached)]),
            observation: Some(EffectObservation::Completed(Box::new(step()))),
            ..Host::default()
        };
        visibility::reconcile_completed(&token, &mut host as &mut dyn ReferenceCompletionCacheHost)
            .unwrap();
        assert!(host.pending.contains(&token));
        assert!(host.observation.is_some());
        assert_eq!(host.completed.contains_key(&token), exists);
        if exists {
            assert_eq!(payload(&host.completed[&token]), pointer);
            assert_eq!(
                host.trace(),
                ["has:continuation-1", "exists:continuation-1"]
            );
        } else {
            assert_eq!(
                host.trace(),
                [
                    "has:continuation-1",
                    "exists:continuation-1",
                    "evict:continuation-1"
                ]
            );
        }
    }
}

#[test]
fn uncached_observations_keep_missing_distinct_and_move_original_terminal_after_removal() {
    let token = token(1);
    for (index, observation) in [
        EffectObservation::Untracked,
        EffectObservation::Pending,
        EffectObservation::Missing,
        EffectObservation::Completed(Box::new(step())),
    ]
    .into_iter()
    .enumerate()
    {
        let pointer = if let EffectObservation::Completed(s) = &observation {
            Some(payload(s))
        } else {
            None
        };
        let mut host = Host {
            observation: Some(observation),
            pending: BTreeSet::from([token.clone()]),
            original_step: pointer,
            original_token: Some(std::ptr::from_ref(&token)),
            ..Host::default()
        };
        visibility::reconcile_completed(&token, &mut host).unwrap();
        assert_eq!(host.pending.contains(&token), index < 2);
        assert_eq!(host.completed.contains_key(&token), index == 3);
        let mut trace = vec!["has:continuation-1", "observe:continuation-1"];
        if index >= 2 {
            trace.push("remove:continuation-1");
        }
        if index == 3 {
            trace.push("insert:continuation-1");
        }
        assert_eq!(host.trace(), trace);
        if let Some(pointer) = pointer {
            assert_eq!(payload(&host.completed[&token]), pointer);
        }
    }
}

#[test]
fn native_read_faults_preserve_ownership_and_leave_both_caches_untouched() {
    let token = token(1);
    for stage in ["exists", "observe", "merge"] {
        let error = fault();
        let pointer = error.message.as_ptr();
        let mut host = Host {
            error: Some(error),
            fail_at: Some(stage),
            pending: BTreeSet::from([token.clone()]),
            ..Host::default()
        };
        if stage == "exists" {
            host.completed.insert(token.clone(), step());
        }
        let before = (host.pending.clone(), host.completed.clone());
        let error = if stage == "merge" {
            let Step::Fault(error) = visibility::visible_step(&token, step(), &mut host) else {
                panic!()
            };
            error
        } else {
            visibility::reconcile_completed(&token, &mut host).unwrap_err()
        };
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!((host.pending.clone(), host.completed.clone()), before);
        assert_eq!(host.trace().len(), if stage == "merge" { 1 } else { 2 });
    }
}

#[test]
fn standalone_and_waiting_preserve_owned_results_without_cache_changes() {
    let token = token(1);
    let original = step();
    let pointer = payload(&original);
    let mut host = Host {
        progress: Some(MergeProgress::Standalone),
        original_token: Some(std::ptr::from_ref(&token)),
        ..Host::default()
    };
    let got = visibility::visible_step(
        &token,
        original,
        &mut host as &mut dyn ReferenceGroupVisibilityHost,
    );
    assert_eq!(payload(&got), pointer);
    assert_eq!(host.trace(), ["merge:continuation-1"]);
    let merge = token.clone();
    let pointer = merge.as_str().as_ptr();
    host.progress = Some(MergeProgress::Pending {
        merge_token: merge,
        completed_branches: 2,
        total_branches: 5,
    });
    let Step::Waiting(wait) = visibility::visible_step(&token, step(), &mut host) else {
        panic!()
    };
    assert_eq!(wait.merge_token.as_str().as_ptr(), pointer);
    assert_eq!((wait.completed_branches, wait.total_branches), (2, 5));
    assert!(host.pending.is_empty() && host.completed.is_empty());
    assert_eq!(
        host.trace(),
        ["merge:continuation-1", "merge:continuation-1"]
    );
}

#[test]
fn next_preserves_request_allocation_and_order_including_duplicate_and_overlapping_tokens() {
    let request = request();
    let pointer = std::ptr::from_ref(request.as_ref());
    let mut host = Host {
        progress: Some(MergeProgress::Next {
            request,
            completed_tokens: vec![token(2), token(7), token(2)],
        }),
        pending: BTreeSet::from([token(2), token(7), token(9)]),
        original_request: Some(pointer),
        ..Host::default()
    };
    let Step::Effect(got) = visibility::visible_step(&token(1), step(), &mut host) else {
        panic!()
    };
    assert_eq!(std::ptr::from_ref(got.as_ref()), pointer);
    assert_eq!(host.pending, BTreeSet::from([token(7), token(9)]));
    assert!(host.completed.is_empty());
    assert_eq!(
        host.trace(),
        [
            "merge:continuation-1",
            "remove:continuation-2",
            "remove:continuation-7",
            "remove:continuation-2",
            "next:continuation-7"
        ]
    );
}

#[test]
fn completed_only_fills_removed_pending_members_and_never_overwrites_existing_results() {
    let original = step();
    let pointer = payload(&original);
    let existing = step();
    let existing_pointer = payload(&existing);
    let mut host = Host {
        progress: Some(MergeProgress::Completed {
            merge_token: token(10),
            step: Box::new(original),
            branch_tokens: vec![token(1), token(2), token(3), token(1)],
        }),
        pending: BTreeSet::from([token(1), token(2), token(9)]),
        completed: BTreeMap::from([(token(2), existing)]),
        original_step: Some(pointer),
        ..Host::default()
    };
    let got = visibility::visible_step(&token(1), step(), &mut host);
    assert_eq!(payload(&got), pointer);
    assert_eq!(host.pending, BTreeSet::from([token(9)]));
    assert_eq!(host.completed.len(), 2);
    assert_eq!(payload(&host.completed[&token(2)]), existing_pointer);
    assert_eq!(host.completed[&token(1)], got);
    assert_eq!(
        host.trace(),
        [
            "merge:continuation-1",
            "remove:continuation-1",
            "merged:continuation-1",
            "remove:continuation-2",
            "merged:continuation-2",
            "remove:continuation-3",
            "remove:continuation-1",
            "debug:continuation-10"
        ]
    );
}

#[test]
fn hook_unwinds_do_not_retry_or_undo_already_applied_cache_changes() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for stage in [
        "observe", "remove", "insert", "evict", "next", "merged", "debug",
    ] {
        let mut host = Host {
            unwind_at: Some(stage),
            observation: Some(EffectObservation::Completed(Box::new(step()))),
            pending: BTreeSet::from([token(1)]),
            ..Host::default()
        };
        if stage == "evict" {
            host.completed.insert(token(1), step());
        }
        host.progress = Some(if stage == "next" {
            MergeProgress::Next {
                request: request(),
                completed_tokens: vec![token(1)],
            }
        } else {
            MergeProgress::Completed {
                merge_token: token(10),
                step: Box::new(step()),
                branch_tokens: vec![token(1)],
            }
        });
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if matches!(stage, "next" | "merged" | "debug") {
                    visibility::visible_step(&token(1), step(), &mut host);
                } else {
                    visibility::reconcile_completed(&token(1), &mut host).unwrap();
                }
            }))
            .is_err()
        );
        assert_eq!(
            host.trace()
                .iter()
                .filter(|e| e.starts_with(&format!("{stage}:")))
                .count(),
            1
        );
        assert_eq!(
            host.pending.contains(&token(1)),
            matches!(stage, "observe" | "evict")
        );
        assert_eq!(
            host.completed.contains_key(&token(1)),
            matches!(stage, "insert" | "merged" | "debug")
        );
        assert_eq!(host.pending.contains(&token(7)), stage == "next");
    }
}
