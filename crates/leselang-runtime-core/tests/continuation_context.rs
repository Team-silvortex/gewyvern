use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::{
    ContinuationContextError, ContinuationContextSite, ResourceBudget, ResourceBudgetLimits,
    validate_inherited_context,
};

fn budget() -> ResourceBudget {
    ResourceBudget {
        fuel_remaining: 10,
        deadline_ms: 20,
        deadline_at_ms: Some(30),
        max_output_items: 40,
    }
}
fn compare(
    original: &ResourceBudget,
    saved: &ResourceBudget,
) -> Result<(), ContinuationContextError<Infallible>> {
    validate_inherited_context(original, saved, || Ok(true), || Ok(true))
}

#[test]
fn equal_context_calls_native_hooks_once_in_authority_then_revision_order() {
    let events = RefCell::new(vec![]);
    validate_inherited_context(
        &budget(),
        &budget(),
        || {
            events.borrow_mut().push("authority");
            Ok::<_, Infallible>(true)
        },
        || {
            events.borrow_mut().push("revision");
            Ok(true)
        },
    )
    .unwrap();
    assert_eq!(*events.borrow(), ["authority", "revision"]);
}

#[test]
fn authority_refusal_skips_revision_and_precedes_all_budget_differences() {
    let mut saved = budget();
    saved.deadline_ms = 0;
    saved.deadline_at_ms = None;
    saved.max_output_items = 0;
    let revisions = Cell::new(0);
    let error = validate_inherited_context(
        &budget(),
        &saved,
        || Ok::<_, Infallible>(false),
        || {
            revisions.set(revisions.get() + 1);
            Ok(true)
        },
    )
    .unwrap_err();
    assert!(matches!(error, ContinuationContextError::Authority));
    assert_eq!(revisions.get(), 0);
}

#[test]
fn revision_refusal_precedes_numeric_context_differences() {
    let mut saved = budget();
    saved.deadline_ms = 0;
    saved.deadline_at_ms = None;
    saved.max_output_items = 0;
    assert!(matches!(
        validate_inherited_context(
            &budget(),
            &saved,
            || Ok::<_, Infallible>(true),
            || Ok(false)
        ),
        Err(ContinuationContextError::Revision)
    ));
}

#[test]
fn relative_absolute_and_output_mismatches_keep_their_original_precedence() {
    let mut saved = budget();
    saved.deadline_ms = 0;
    saved.deadline_at_ms = None;
    saved.max_output_items = 0;
    assert!(matches!(
        compare(&budget(), &saved),
        Err(ContinuationContextError::Deadline)
    ));
    saved.deadline_ms = 20;
    assert!(matches!(
        compare(&budget(), &saved),
        Err(ContinuationContextError::AbsoluteDeadline)
    ));
    saved.deadline_at_ms = Some(30);
    assert!(matches!(
        compare(&budget(), &saved),
        Err(ContinuationContextError::Output)
    ));
}

#[test]
fn optional_deadlines_are_compared_exactly_without_default_or_normalization() {
    for original in [None, Some(0), Some(1), Some(u64::MAX)] {
        for saved in [None, Some(0), Some(1), Some(u64::MAX)] {
            let a = ResourceBudget {
                deadline_at_ms: original,
                ..budget()
            };
            let b = ResourceBudget {
                deadline_at_ms: saved,
                ..budget()
            };
            assert_eq!(compare(&a, &b).is_ok(), original == saved);
        }
    }
}

#[test]
fn output_limits_cannot_be_narrowed_or_expanded_by_an_inheritor() {
    for original in [0, 1, usize::MAX] {
        for saved in [0, 1, usize::MAX] {
            let a = ResourceBudget {
                max_output_items: original,
                ..budget()
            };
            let b = ResourceBudget {
                max_output_items: saved,
                ..budget()
            };
            assert_eq!(compare(&a, &b).is_ok(), original == saved);
        }
    }
}

#[test]
fn agreement_does_not_validate_budget_or_construct_a_fuel_grant() {
    let original = ResourceBudget {
        fuel_remaining: 0,
        deadline_ms: 0,
        deadline_at_ms: Some(u64::MAX),
        max_output_items: usize::MAX,
    };
    let saved = ResourceBudget {
        fuel_remaining: u64::MAX,
        ..original.clone()
    };
    compare(&original, &saved).unwrap();
    assert!(
        original
            .validate(ResourceBudgetLimits {
                max_fuel: 10,
                max_deadline_ms: 20,
                max_deadline_at_ms: 30,
                max_output_items: 40
            })
            .is_err()
    );
    assert_eq!(saved.fuel_remaining, u64::MAX);
    assert_eq!(original.fuel_remaining, 0);
}

struct OpaqueGuiIdentity {
    id: u64,
    _local: Rc<()>,
}
struct Cleanup {
    released: Rc<Cell<usize>>,
    panic: bool,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        self.released.set(self.released.get() + 1);
        assert!(!self.panic, "native cleanup panic");
    }
}
struct NativeFault {
    secret: Box<str>,
}

#[test]
fn gui_local_native_identity_and_revision_need_no_payload_traits_or_thread_bounds() {
    let original = OpaqueGuiIdentity {
        id: 7,
        _local: Rc::new(()),
    };
    let saved = OpaqueGuiIdentity {
        id: 7,
        _local: Rc::new(()),
    };
    let revision = OpaqueGuiIdentity {
        id: 3,
        _local: Rc::new(()),
    };
    let released = Rc::new(Cell::new(0));
    let authority_owner = Cleanup {
        released: released.clone(),
        panic: false,
    };
    let revision_owner = Cleanup {
        released: released.clone(),
        panic: false,
    };
    validate_inherited_context(
        &budget(),
        &budget(),
        move || {
            let _keep = &authority_owner;
            Ok::<_, NativeFault>(original.id == saved.id)
        },
        move || {
            let _keep = &revision_owner;
            Ok(revision.id == 3)
        },
    )
    .unwrap();
    assert_eq!(released.get(), 2);
}

#[test]
fn native_errors_keep_exact_payload_ownership_sites_and_redacted_diagnostics() {
    use std::error::Error;
    for site in [
        ContinuationContextSite::Authority,
        ContinuationContextSite::Revision,
    ] {
        let secret: Box<str> = "private-gui-identity".into();
        let address = secret.as_ptr();
        let mut error = Some(NativeFault { secret });
        let mut revision_error = if site == ContinuationContextSite::Revision {
            error.take()
        } else {
            None
        };
        let observed_revision = Cell::new(0);
        let result = validate_inherited_context(
            &budget(),
            &budget(),
            || match error.take() {
                Some(error) => Err(error),
                None => Ok(true),
            },
            || {
                observed_revision.set(observed_revision.get() + 1);
                match revision_error.take() {
                    Some(error) => Err(error),
                    None => Ok(true),
                }
            },
        );
        let failure = result.unwrap_err();
        assert!(!format!("{failure:?} {failure}").contains("private-gui-identity"));
        assert!(failure.source().is_none());
        let ContinuationContextError::Native {
            site: actual,
            error,
        } = failure
        else {
            panic!("native error expected")
        };
        assert_eq!(actual, site);
        assert_eq!(error.secret.as_ptr(), address);
        assert_eq!(
            observed_revision.get(),
            usize::from(site == ContinuationContextSite::Revision)
        );
    }
}

#[test]
fn unused_revision_hook_releases_before_refusal_handoff() {
    let released = Rc::new(Cell::new(0));
    let owner = Cleanup {
        released: released.clone(),
        panic: false,
    };
    let revision = move || {
        let _keep = &owner;
        panic!("revision must be skipped")
    };
    let result = validate_inherited_context(
        &budget(),
        &budget(),
        || Ok::<_, NativeFault>(false),
        revision,
    );
    assert!(matches!(result, Err(ContinuationContextError::Authority)));
    assert_eq!(released.get(), 1);
}

#[test]
fn authority_unwind_skips_revision_and_releases_both_hook_owners_once() {
    let released = Rc::new(Cell::new(0));
    let a = Cleanup {
        released: released.clone(),
        panic: false,
    };
    let r = Cleanup {
        released: released.clone(),
        panic: false,
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| validate_inherited_context(
            &budget(),
            &budget(),
            move || {
                let _keep = &a;
                panic!("authority observation panic")
            },
            move || {
                let _keep = &r;
                Ok::<_, NativeFault>(true)
            }
        )))
        .is_err()
    );
    assert_eq!(released.get(), 2);
}

#[test]
fn cleanup_unwind_prevents_a_refusal_from_being_handed_off() {
    let released = Rc::new(Cell::new(0));
    let owner = Cleanup {
        released: released.clone(),
        panic: true,
    };
    let handed_off = Cell::new(false);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _result = validate_inherited_context(
                &budget(),
                &budget(),
                || Ok::<_, NativeFault>(false),
                move || {
                    let _keep = &owner;
                    Ok(true)
                },
            );
            handed_off.set(true);
        }))
        .is_err()
    );
    assert!(!handed_off.get());
    assert_eq!(released.get(), 1);
}
