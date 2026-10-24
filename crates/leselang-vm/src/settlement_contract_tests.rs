use super::*;

// Frozen before settlement routing extraction; the original journal remains shared.
pub(super) fn frozen_record_result(
    vm: &mut Vm,
    image: &ContinuationImage,
    lease: CompletionLease<'_>,
    step: Step,
) -> Step {
    let recorded = match lease {
        Some((lease, now_ms)) => vm.journal.acknowledge_dispatch(lease, now_ms, &step),
        None => vm.journal.record_completed(image, &step),
    };
    let authoritative = match recorded {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    vm.pending.remove(&image.token);
    vm.completed
        .insert(image.token.clone(), authoritative.clone());
    vm.visible_step(&image.token, authoritative)
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn settlement_sqlite_publishes_peer_winner_instead_of_the_proposed_result() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let mut comparisons = 0;
    for leased in [false, true] {
        for fault_wins in [false, true] {
            let scratch = Scratch(std::env::temp_dir().join(format!(
                "leselang-settlement-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )));
            std::fs::create_dir(&scratch.0).unwrap();
            let actual_path = scratch.0.join("actual.sqlite3");
            let expected_path = scratch.0.join("expected.sqlite3");
            let mut actual = Vm::open_journal(&actual_path, 100).unwrap();
            let mut expected = Vm::open_journal(&expected_path, 100).unwrap();
            let program = leselang_hir::lower(&leselang_syntax::parse(
                r#"fn main() = ui.focus(node_id: "a")"#,
            ))
            .unwrap();
            let start = |vm: &mut Vm| {
                vm.start(
                    &program,
                    Principal::new("operator").unwrap(),
                    CapabilitySet::new(["ui.presentation"]),
                    None,
                )
            };
            let started = start(&mut actual);
            assert_eq!(started, start(&mut expected));
            let Step::Effect(request) = started else {
                panic!()
            };
            let lease = if leased {
                let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                Some(lease)
            } else {
                None
            };
            let authority = lease.as_ref().map(|l| (l, 12));
            let winner = if fault_wins {
                Step::Fault(Fault {
                    code: "LSV2103".into(),
                    message: "peer result".into(),
                })
            } else {
                Step::Done(Value::UiFocus {
                    node_id: "a".into(),
                })
            };
            for path in [&actual_path, &expected_path] {
                let mut peer = Vm::open_journal(path, 0).unwrap();
                assert_eq!(
                    peer.record_result(&request.continuation, authority, winner.clone()),
                    winner
                );
            }
            // These owners still have their original pending caches after the peer commits.
            assert_eq!(actual.pending_count(), 1);
            assert_eq!(expected.pending_count(), 1);
            let proposed = Step::Fault(Fault {
                code: "LSV1001".into(),
                message: "losing proposal".into(),
            });
            let want = frozen_record_result(
                &mut expected,
                &request.continuation,
                authority,
                proposed.clone(),
            );
            let got = actual.record_result(&request.continuation, authority, proposed);
            assert_eq!(got, winner);
            assert_eq!(got, want);
            assert_eq!(actual.pending, expected.pending);
            assert!(actual.pending.is_empty());
            assert_eq!(actual.completed, expected.completed);
            assert_eq!(
                actual.completed.get(&request.continuation.token),
                Some(&winner)
            );
            drop((actual, expected));
            let mut reopened = Vm::open_journal(&actual_path, 0).unwrap();
            assert_eq!(
                reopened
                    .replay_completed(&request.continuation.token)
                    .unwrap(),
                Some(winner)
            );
            assert!(reopened.claim_effect(200, 100).unwrap().is_none());
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 4);
}
