use leselang_command::reference_journal_schema::{JOURNAL_SCHEMA_VERSION, migrate};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn batches(version: u32) -> Vec<(&'static str, &'static str)> {
    let mut batches = Vec::new();
    migrate(version, |sql, context| {
        batches.push((sql, context));
        Ok(())
    })
    .unwrap();
    batches
}

#[test]
fn supported_versions_preserve_ordered_commits_and_current_version_index_repair() {
    assert_eq!(JOURNAL_SCHEMA_VERSION, 10);
    let mut total = 0;
    for version in 0..=10 {
        let steps = batches(version);
        let count = if version < 4 {
            8
        } else {
            (11 - version) as usize
        };
        assert_eq!(steps.len(), count);
        let last = steps.last().unwrap();
        assert!(
            last.0
                .starts_with("CREATE INDEX IF NOT EXISTS vm_dispatch_attempt_order_idx")
        );
        assert_eq!(last.1, "failed to index dispatch ordering");
        for (index, (sql, context)) in steps[..steps.len() - 1].iter().enumerate() {
            let target = if version < 4 {
                4 + index as u32
            } else {
                version + 1 + index as u32
            };
            assert!(sql.starts_with("BEGIN IMMEDIATE;"));
            assert!(sql.ends_with("COMMIT;"));
            assert!(sql.contains(&format!("PRAGMA user_version = {target};")));
            assert!(!sql.contains("ROLLBACK"));
            if version == 0 && index == 0 {
                assert_eq!(*context, "failed to initialize journal");
            }
        }
        total += steps.len();
    }
    assert_eq!(total, 60);
}

#[test]
fn unsupported_versions_refuse_before_calling_the_native_owner() {
    for version in [11, 99, u32::MAX] {
        let error =
            migrate(version, |_, _| panic!("unsupported schema reached storage")).unwrap_err();
        assert_eq!(
            error,
            Fault {
                code: "LSV4004".into(),
                message: format!("unsupported journal version {version}, expected 1 through 10")
            }
        );
    }
}

#[test]
fn each_native_failure_is_moved_unchanged_and_never_retried_or_compensated() {
    let mut cases = 0;
    for version in 0..=10 {
        let expected = batches(version);
        for failure in 0..expected.len() {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let error = Fault {
                code: "NATIVE".into(),
                message: "original failure buffer".into(),
            };
            let pointer = error.message.as_ptr();
            let mut error = Some(error);
            let got = migrate(version, |sql, context| {
                calls.borrow_mut().push((sql, context));
                if calls.borrow().len() == failure + 1 {
                    Err(error.take().unwrap())
                } else {
                    Ok(())
                }
            })
            .unwrap_err();
            assert_eq!(got.message.as_ptr(), pointer);
            assert_eq!(got.code, "NATIVE");
            assert_eq!(*calls.borrow(), expected[..=failure]);
            cases += 1;
        }
    }
    assert_eq!(cases, 60);
}

#[test]
fn each_native_unwind_propagates_without_replay_or_synthetic_rollback() {
    let mut cases = 0;
    for version in 0..=10 {
        let expected = batches(version);
        for failure in 0..expected.len() {
            let calls = Rc::new(RefCell::new(Vec::new()));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                migrate(version, |sql, context| {
                    calls.borrow_mut().push((sql, context));
                    assert_ne!(calls.borrow().len(), failure + 1, "native migration unwind");
                    Ok(())
                })
            }));
            assert!(result.is_err());
            assert_eq!(*calls.borrow(), expected[..=failure]);
            cases += 1;
        }
    }
    assert_eq!(cases, 60);
}

#[test]
fn trusted_callback_capture_cleanup_can_unwind_after_all_batches_commit() {
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            panic!("native capture cleanup");
        }
    }
    let calls = Rc::new(RefCell::new(Vec::new()));
    let observed = calls.clone();
    let cleanup = Cleanup;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        migrate(0, move |sql, context| {
            let _borrow = &cleanup;
            observed.borrow_mut().push((sql, context));
            Ok(())
        })
    }));
    assert!(result.is_err());
    assert_eq!(*calls.borrow(), batches(0));
}
