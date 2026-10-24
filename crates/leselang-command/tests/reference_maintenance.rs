use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_settlement::maintenance::{
    self, JournalCompaction, ReferenceExpirationHost, ReferenceRetentionHost,
};
use leselang_command::{ContinuationImage, ContinuationToken, RetentionPolicy, Step};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;

fn token(s: &str) -> ContinuationToken {
    ContinuationToken::from_wire(s.into())
}
fn fault() -> Fault {
    Fault {
        code: "native-original".into(),
        message: "native-owned-fault".into(),
    }
}
fn step() -> Step {
    Step::Fault(fault())
}
fn payload(s: &Step) -> *const u8 {
    let Step::Fault(e) = s else { panic!() };
    e.message.as_ptr()
}
fn image(sequence: u64) -> ContinuationImage {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
        sequence,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: Some(10),
        fuel_remaining: 100,
    })
    .unwrap()
    .continuation
}
fn policy() -> RetentionPolicy {
    RetentionPolicy {
        max_completed_records: 1,
        max_delete_per_run: 2,
    }
}
fn report(tokens: Vec<ContinuationToken>) -> JournalCompaction {
    JournalCompaction {
        removed_records: 1,
        pruned_tokens: tokens,
        remaining_completed: 8,
        reclaimed_logical_bytes: 123,
    }
}

#[derive(Default)]
struct Host {
    trace: RefCell<Vec<String>>,
    snapshot: RefCell<Vec<(ContinuationToken, Step)>>,
    sorted: Vec<(String, *const u8)>,
    report: Option<JournalCompaction>,
    expired: Vec<(ContinuationImage, Step)>,
    expected_expired: Vec<(*const u8, *const u8)>,
    expected_pruned: Vec<*const u8>,
    pending: BTreeSet<ContinuationToken>,
    completed: BTreeMap<ContinuationToken, Step>,
    policy: Option<*const RetentionPolicy>,
    now: Option<u64>,
    error: Option<Fault>,
    fail: bool,
    unwind_at: Option<&'static str>,
    committed: bool,
    inserted: usize,
    removed: usize,
}
impl Host {
    fn event(&self, stage: &str) {
        self.trace.borrow_mut().push(stage.into());
    }
    fn commit(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.event(stage);
        self.committed = true;
        assert_ne!(
            self.unwind_at,
            Some(stage),
            "unwind after native side effect"
        );
        if self.fail {
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
    fn trace(&self) -> Vec<String> {
        self.trace.borrow().clone()
    }
}
impl ReferenceRetentionHost for Host {
    fn completed_snapshot(&self) -> Vec<(ContinuationToken, Step)> {
        self.event("snapshot");
        assert_ne!(self.unwind_at, Some("snapshot"), "snapshot unwind");
        self.snapshot.take()
    }
    fn compact_completed(
        &mut self,
        local: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        if let Some(pointer) = self.policy {
            assert_eq!(std::ptr::from_ref(policy), pointer);
        }
        assert_eq!(
            local
                .iter()
                .map(|(t, s)| (t.as_str().to_owned(), payload(s)))
                .collect::<Vec<_>>(),
            self.sorted
        );
        self.commit("compact")?;
        Ok(self.report.take().unwrap())
    }
    fn remove_completed(&mut self, token: &ContinuationToken) {
        assert!(self.committed);
        if !self.expected_pruned.is_empty() {
            assert_eq!(token.as_str().as_ptr(), self.expected_pruned[self.removed]);
        }
        self.event(&format!("evict:{}", token.as_str()));
        self.completed.remove(token);
        self.removed += 1;
        assert_ne!(self.unwind_at, Some("evict"), "eviction unwind");
    }
}
impl ReferenceExpirationHost for Host {
    fn expire_due(&mut self, now: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        if let Some(expected) = self.now {
            assert_eq!(now, expected);
        }
        self.commit("expire")?;
        Ok(std::mem::take(&mut self.expired))
    }
    fn remove_pending(&mut self, token: &ContinuationToken) {
        assert!(self.committed);
        if !self.expected_expired.is_empty() {
            assert_eq!(
                token.as_str().as_ptr(),
                self.expected_expired[self.inserted].0
            );
        }
        self.event(&format!("remove:{}", token.as_str()));
        self.pending.remove(token);
        assert_ne!(self.unwind_at, Some("remove"), "pending removal unwind");
    }
    fn insert_expired(&mut self, token: ContinuationToken, step: Step) {
        if !self.expected_expired.is_empty() {
            assert_eq!(
                (token.as_str().as_ptr(), payload(&step)),
                self.expected_expired[self.inserted]
            );
        }
        self.event(&format!("insert:{}", token.as_str()));
        self.completed.insert(token, step);
        self.inserted += 1;
        assert_ne!(
            self.unwind_at,
            Some("insert"),
            "completion insertion unwind"
        );
    }
}

#[test]
fn policy_rejection_precedes_snapshot_and_native_effects_at_all_bounds() {
    for retained in [0, 1, 10_000, 10_001, usize::MAX] {
        for batch in [0, 1, 1_000, 1_001, usize::MAX] {
            let policy = RetentionPolicy {
                max_completed_records: retained,
                max_delete_per_run: batch,
            };
            let mut host = Host {
                report: Some(report(vec![])),
                ..Host::default()
            };
            let result = maintenance::compact_journal(&policy, &mut host);
            if (1..=10_000).contains(&retained) && (1..=1_000).contains(&batch) {
                assert!(result.is_ok());
                assert_eq!(host.trace(), ["snapshot", "compact"]);
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    Fault {
                        code: "LSV2301".into(),
                        message: "journal retention policy exceeds runtime bounds".into()
                    }
                );
                assert!(host.trace().is_empty());
                assert!(!host.committed);
            }
        }
    }
}

#[test]
fn stable_age_order_preserves_snapshot_payloads_policy_borrow_and_native_report_counts() {
    let policy = policy();
    let snapshot = [
        "legacy-z",
        "continuation-10",
        "continuation-1",
        "continuation-01",
        "legacy-a",
    ]
    .map(|s| (token(s), step()));
    let sorted = [2, 3, 1, 4, 0]
        .map(|i| (snapshot[i].0.as_str().to_owned(), payload(&snapshot[i].1)))
        .to_vec();
    let native = report(vec![
        token("continuation-1"),
        token("continuation-10"),
        token("continuation-1"),
    ]);
    let mut host = Host {
        sorted,
        snapshot: RefCell::new(snapshot.into()),
        expected_pruned: native
            .pruned_tokens
            .iter()
            .map(|t| t.as_str().as_ptr())
            .collect(),
        report: Some(native),
        policy: Some(std::ptr::from_ref(&policy)),
        completed: BTreeMap::from([
            (token("continuation-1"), step()),
            (token("continuation-10"), step()),
            (token("untouched"), step()),
        ]),
        ..Host::default()
    };
    let result =
        maintenance::compact_journal(&policy, &mut host as &mut dyn ReferenceRetentionHost)
            .unwrap();
    assert_eq!(
        (
            result.removed_records,
            result.remaining_completed,
            result.reclaimed_logical_bytes
        ),
        (1, 8, 123)
    );
    assert_eq!(
        host.trace(),
        [
            "snapshot",
            "compact",
            "evict:continuation-1",
            "evict:continuation-10",
            "evict:continuation-1"
        ]
    );
    assert_eq!(
        host.completed.keys().collect::<Vec<_>>(),
        [&token("untouched")]
    );
}

#[test]
fn zero_removed_records_still_evicts_authoritative_stale_local_tokens() {
    let mut native = report(vec![token("stale")]);
    native.removed_records = 0;
    native.reclaimed_logical_bytes = 0;
    let mut host = Host {
        report: Some(native),
        completed: BTreeMap::from([(token("stale"), step())]),
        ..Host::default()
    };
    let result = maintenance::compact_journal(&policy(), &mut host).unwrap();
    assert_eq!(result.removed_records, 0);
    assert_eq!(result.remaining_completed, 8);
    assert_eq!(result.reclaimed_logical_bytes, 0);
    assert!(host.completed.is_empty());
    assert_eq!(host.trace(), ["snapshot", "compact", "evict:stale"]);
}

#[test]
fn expiration_clock_validation_precedes_hooks_and_empty_batches_do_not_publish() {
    for now in [0, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
        let mut host = Host {
            now: Some(now),
            ..Host::default()
        };
        let result = maintenance::expire_due(now, &mut host as &mut dyn ReferenceExpirationHost);
        assert_eq!(result, leselang_runtime_core::validate_clock(now));
        if now <= i64::MAX as u64 {
            assert_eq!(host.trace(), ["expire"]);
        } else {
            assert!(host.trace().is_empty());
        }
        assert!(host.pending.is_empty() && host.completed.is_empty());
    }
}

#[test]
fn expiration_moves_original_tokens_and_results_in_native_order_including_duplicates() {
    let expired = vec![(image(2), step()), (image(1), step()), (image(2), step())];
    let expected_expired = expired
        .iter()
        .map(|(i, s)| (i.token.as_str().as_ptr(), payload(s)))
        .collect::<Vec<_>>();
    let last_pointer = expected_expired[2];
    let mut host = Host {
        expired,
        expected_expired,
        now: Some(12),
        pending: BTreeSet::from([
            token("continuation-1"),
            token("continuation-2"),
            token("other"),
        ]),
        completed: BTreeMap::from([(token("continuation-2"), step()), (token("other"), step())]),
        ..Host::default()
    };
    maintenance::expire_due(12, &mut host).unwrap();
    assert_eq!(
        host.trace(),
        [
            "expire",
            "remove:continuation-2",
            "insert:continuation-2",
            "remove:continuation-1",
            "insert:continuation-1",
            "remove:continuation-2",
            "insert:continuation-2"
        ]
    );
    assert_eq!(host.pending, BTreeSet::from([token("other")]));
    assert_eq!(host.completed.len(), 3);
    assert_eq!(
        payload(&host.completed[&token("continuation-2")]),
        last_pointer.1
    );
}

#[test]
fn native_faults_keep_original_ownership_and_skip_cache_work_without_compensating_native_effects() {
    for retention in [false, true] {
        let error = fault();
        let pointer = error.message.as_ptr();
        let mut host = Host {
            fail: true,
            error: Some(error),
            pending: BTreeSet::from([token("x")]),
            completed: BTreeMap::from([(token("x"), step())]),
            ..Host::default()
        };
        let before = (host.pending.clone(), host.completed.clone());
        let error = if retention {
            maintenance::compact_journal(&policy(), &mut host).unwrap_err()
        } else {
            maintenance::expire_due(12, &mut host).unwrap_err()
        };
        assert_eq!(error.message.as_ptr(), pointer);
        assert!(host.committed);
        assert_eq!((host.pending.clone(), host.completed.clone()), before);
        assert_eq!(
            host.trace(),
            if retention {
                vec!["snapshot", "compact"]
            } else {
                vec!["expire"]
            }
        );
    }
}

#[test]
fn hook_unwinds_never_retry_or_roll_back_prior_native_or_cache_effects() {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    for stage in ["snapshot", "compact", "evict", "expire", "remove", "insert"] {
        let t = token("continuation-1");
        let mut host = Host {
            unwind_at: Some(stage),
            report: Some(report(vec![t.clone()])),
            expired: vec![(image(1), step())],
            pending: BTreeSet::from([t.clone()]),
            completed: BTreeMap::from([(t.clone(), step())]),
            ..Host::default()
        };
        let before_pointer = payload(&host.completed[&t]);
        let retention = matches!(stage, "snapshot" | "compact" | "evict");
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                if retention {
                    maintenance::compact_journal(&policy(), &mut host).unwrap();
                } else {
                    maintenance::expire_due(12, &mut host).unwrap();
                }
            }))
            .is_err()
        );
        assert_eq!(
            host.trace()
                .iter()
                .filter(|e| e.split(':').next() == Some(stage))
                .count(),
            1
        );
        assert_eq!(host.committed, stage != "snapshot");
        assert_eq!(
            host.pending.contains(&t),
            !matches!(stage, "remove" | "insert")
        );
        assert_eq!(host.completed.contains_key(&t), stage != "evict");
        if let Some(s) = host.completed.get(&t) {
            assert_eq!(payload(s) == before_pointer, stage != "insert");
        }
    }
}
