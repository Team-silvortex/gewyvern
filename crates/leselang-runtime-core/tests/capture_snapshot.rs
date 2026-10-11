use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CaptureScopeHooks, CaptureSnapshotError, CaptureSnapshotHooks, CaptureSnapshotLimits,
    CaptureSnapshotSite, CapturedBindings, CapturedValue, Fuel, ScalarValue, ScopeFrame,
    snapshot_captured_scope_state, with_captured_scope_state,
};

fn limits() -> CaptureSnapshotLimits {
    CaptureSnapshotLimits {
        max_bindings: 8,
        max_name_bytes: 32,
    }
}

struct Native {
    id: usize,
    drops: Rc<RefCell<Vec<usize>>>,
    poison: bool,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.poison, "native destructor unwind");
    }
}
fn native(id: usize, drops: &Rc<RefCell<Vec<usize>>>) -> Native {
    Native {
        id,
        drops: drops.clone(),
        poison: false,
    }
}
struct Context {
    fuel: Fuel,
    original: *const Fuel,
    seen: Rc<RefCell<Vec<usize>>>,
    drops: Rc<RefCell<Vec<usize>>>,
}
impl Context {
    fn charge(&mut self, id: usize, cost: u64) -> Result<(), Native> {
        assert_eq!(self.original, &raw const self.fuel);
        self.seen.borrow_mut().push(id);
        self.fuel.charge(cost).map_err(|_| native(90, &self.drops))
    }
}
fn context(grant: u64) -> Context {
    Context {
        fuel: Fuel::new(grant),
        original: std::ptr::null(),
        seen: Rc::new(RefCell::new(vec![])),
        drops: Rc::new(RefCell::new(vec![])),
    }
}
fn snapshot(
    scope: &ScopeFrame<'_, '_, usize>,
    state: &mut Context,
) -> Result<CapturedBindings<Native, Native, Native>, CaptureSnapshotError<Native>> {
    snapshot_captured_scope_state(
        "reply",
        &(),
        scope,
        state,
        limits(),
        CaptureSnapshotHooks {
            row: |_: &str, id: &usize, s: &mut Context| {
                let cost = match id {
                    0 => 2,
                    1 => 3,
                    _ => 5,
                };
                s.charge(*id, cost)?;
                Ok(match id {
                    0 => CapturedValue::Scalar(ScalarValue::Integer(7)),
                    1 => CapturedValue::Result(native(*id, &s.drops)),
                    _ => CapturedValue::Group(native(*id, &s.drops)),
                })
            },
            body: |_: &(), s: &mut Context| {
                s.charge(3, 7)?;
                Ok(native(3, &s.drops))
            },
        },
    )
}

#[test]
fn ordered_native_preparation_uses_original_gui_local_context_and_moves_category_rows() {
    let mut values = vec![("group", 2), ("scalar", 0), ("result", 1), ("last", 0)];
    let scope = ScopeFrame::new(&mut values);
    let mut state = context(19);
    state.original = &raw const state.fuel;
    let saved = snapshot(&scope, &mut state).unwrap();
    assert_eq!(*state.seen.borrow(), [2, 0, 1, 0, 3]);
    assert_eq!(state.fuel.remaining(), 0);
    assert_eq!(
        saved
            .locals
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>(),
        ["scalar", "last"]
    );
    assert_eq!(saved.results[0].result.id, 1);
    assert_eq!(saved.groups[0].group.id, 2);
    assert_eq!(
        scope.bindings(),
        [("group", 2), ("scalar", 0), ("result", 1), ("last", 0)]
    );
    drop(saved);
    assert_eq!(*state.drops.borrow(), [1, 2, 3]);
}

#[test]
fn every_original_budget_boundary_stops_before_later_rows_without_refund() {
    let mut values = vec![("scalar", 0), ("result", 1), ("group", 2)];
    let scope = ScopeFrame::new(&mut values);
    for grant in 0..=20 {
        let mut state = context(grant);
        state.original = &raw const state.fuel;
        let output = snapshot(&scope, &mut state);
        let costs = [2, 3, 5, 7];
        let mut remaining = grant;
        let mut calls = 0;
        for cost in costs {
            calls += 1;
            if remaining < cost {
                break;
            }
            remaining -= cost;
        }
        assert_eq!(state.fuel.remaining(), remaining, "grant {grant}");
        assert_eq!(*state.seen.borrow(), (0..calls).collect::<Vec<_>>());
        if grant < 17 {
            let site = if calls == 4 {
                CaptureSnapshotSite::Body
            } else {
                CaptureSnapshotSite::Binding { index: calls - 1 }
            };
            assert!(
                matches!(output, Err(CaptureSnapshotError::Native { site: actual, .. }) if actual == site)
            );
        } else {
            assert!(output.is_ok());
        }
        assert_eq!(scope.len(), 3);
    }
}

#[test]
fn complete_count_and_utf8_name_preflight_precedes_every_native_hook() {
    for (rows, name, policy, expected) in [
        (
            vec![("a", 0), ("b", 1)],
            "long",
            CaptureSnapshotLimits {
                max_bindings: 1,
                max_name_bytes: 1,
            },
            "BindingCount",
        ),
        (
            vec![("a", 0)],
            "long",
            CaptureSnapshotLimits {
                max_bindings: 1,
                max_name_bytes: 1,
            },
            "PendingName",
        ),
        (
            vec![("a", 0), ("\u{00e9}", 1)],
            "r",
            CaptureSnapshotLimits {
                max_bindings: 2,
                max_name_bytes: 1,
            },
            "BindingName { index: 1 }",
        ),
    ] {
        let mut rows = rows;
        let scope = ScopeFrame::new(&mut rows);
        let mut called = 0;
        let output = snapshot_captured_scope_state(
            name,
            &(),
            &scope,
            &mut called,
            policy,
            CaptureSnapshotHooks {
                row: |_: &str, _: &usize, s: &mut usize| {
                    *s += 1;
                    Ok::<CapturedValue<(), ()>, ()>(CapturedValue::Scalar(ScalarValue::None))
                },
                body: |_: &(), s: &mut usize| {
                    *s += 1;
                    Ok(())
                },
            },
        );
        assert_eq!(format!("{:?}", output.unwrap_err()), expected);
        assert_eq!(called, 0);
    }
    let mut empty: Vec<(&str, ())> = vec![];
    let saved = snapshot_captured_scope_state(
        "",
        &(),
        &ScopeFrame::new(&mut empty),
        &mut (),
        CaptureSnapshotLimits {
            max_bindings: 0,
            max_name_bytes: 0,
        },
        CaptureSnapshotHooks {
            row: |_: &str, _: &(), _: &mut ()| {
                Ok::<CapturedValue<(), ()>, ()>(CapturedValue::Scalar(ScalarValue::None))
            },
            body: |_: &(), _: &mut ()| Ok(()),
        },
    )
    .unwrap();
    assert!(saved.locals.is_empty());
}

#[test]
fn native_errors_keep_original_position_ownership_and_redacted_diagnostics() {
    struct Secret;
    impl std::fmt::Debug for Secret {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("secret formatter")
        }
    }
    impl std::fmt::Display for Secret {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("secret formatter")
        }
    }
    let error = CaptureSnapshotError::Native {
        site: CaptureSnapshotSite::Binding { index: 4 },
        error: Secret,
    };
    assert_eq!(format!("{error}"), "native capture snapshot hook rejected");
    assert_eq!(
        format!("{error:?}"),
        "Native { site: Binding { index: 4 }, .. }"
    );
    let CaptureSnapshotError::Native {
        error: original, ..
    } = error
    else {
        panic!()
    };
    let _ = original;
}

#[test]
fn unsized_context_and_borrowed_native_values_need_no_clone_or_thread_traits() {
    struct View<'a>(&'a str, Rc<()>);
    let source = String::from("native body");
    let payload = String::from("native result");
    let mut rows = vec![("view", View(payload.as_str(), Rc::new(())))];
    let scope = ScopeFrame::new(&mut rows);
    let mut state = [0_u8; 2];
    fn row<'a>(_: &str, v: &View<'a>, s: &mut [u8]) -> Result<CapturedValue<View<'a>, ()>, ()> {
        s[0] += 1;
        Ok(CapturedValue::Result(View(v.0, v.1.clone())))
    }
    fn body<'a>(v: &'a str, s: &mut [u8]) -> Result<&'a str, ()> {
        s[1] += 1;
        Ok(v)
    }
    let saved = snapshot_captured_scope_state(
        "reply",
        source.as_str(),
        &scope,
        &mut state[..],
        limits(),
        CaptureSnapshotHooks { row, body },
    )
    .unwrap();
    assert_eq!(saved.body.as_ptr(), source.as_ptr());
    assert_eq!(saved.results[0].result.0.as_ptr(), payload.as_ptr());
    assert_eq!(state, [1, 1]);
}

#[test]
fn native_hook_unwind_preserves_scope_and_context_without_rollback_or_retry() {
    let mut values = vec![("first", 1), ("second", 2)];
    let scope = ScopeFrame::new(&mut values);
    let mut state = context(20);
    state.original = &raw const state.fuel;
    let result = catch_unwind(AssertUnwindSafe(|| {
        snapshot_captured_scope_state(
            "r",
            &(),
            &scope,
            &mut state,
            limits(),
            CaptureSnapshotHooks {
                row: |_: &str, id: &usize, s: &mut Context| {
                    assert!(s.charge(*id, 3).is_ok());
                    assert_ne!(*id, 2, "row unwind");
                    Ok::<CapturedValue<Native, ()>, Native>(CapturedValue::Result(native(
                        *id, &s.drops,
                    )))
                },
                body: |_: &(), _: &mut Context| -> Result<(), Native> {
                    panic!("must not enter body")
                },
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(state.fuel.remaining(), 14);
    assert_eq!(*state.seen.borrow(), [1, 2]);
    assert_eq!(*state.drops.borrow(), [1]);
    assert_eq!(scope.bindings(), [("first", 1), ("second", 2)]);
}

#[test]
fn uncalled_body_cleanup_unwind_releases_already_owned_row_error() {
    let mut values = vec![("secret", ())];
    let scope = ScopeFrame::new(&mut values);
    let drops = Rc::new(RefCell::new(vec![]));
    let poison = Native {
        id: 99,
        drops: drops.clone(),
        poison: true,
    };
    let row_drops = drops.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        snapshot_captured_scope_state(
            "r",
            &(),
            &scope,
            &mut (),
            limits(),
            CaptureSnapshotHooks {
                row: |_: &str, _: &(), _: &mut ()| -> Result<CapturedValue<(), ()>, Native> {
                    Err(native(90, &row_drops))
                },
                body: move |_: &(), _: &mut ()| {
                    let _whole_capture = &poison;
                    Ok(())
                },
            },
        )
    }));
    assert!(result.is_err());
    assert!(drops.borrow().contains(&90));
    assert!(drops.borrow().contains(&99));
}

#[test]
fn partial_row_cleanup_unwind_releases_later_row_and_body_errors() {
    for body_error in [false, true] {
        let mut values = vec![("first", 1), ("second", 2)];
        let scope = ScopeFrame::new(&mut values);
        let drops = Rc::new(RefCell::new(vec![]));
        let result = catch_unwind(AssertUnwindSafe(|| {
            snapshot_captured_scope_state(
                "r",
                &(),
                &scope,
                &mut (),
                limits(),
                CaptureSnapshotHooks {
                    row: |_: &str, id: &usize, _: &mut ()| {
                        if *id == 2 && !body_error {
                            return Err(native(90, &drops));
                        }
                        Ok::<CapturedValue<Native, ()>, Native>(CapturedValue::Result(Native {
                            id: *id,
                            drops: drops.clone(),
                            poison: *id == 1,
                        }))
                    },
                    body: |_: &(), _: &mut ()| -> Result<(), Native> { Err(native(90, &drops)) },
                },
            )
        }));
        assert!(result.is_err());
        assert!(drops.borrow().contains(&90));
        assert!(drops.borrow().contains(&1));
    }
}

#[test]
fn remaining_row_hook_cleanup_unwind_releases_successful_capture_or_body_error() {
    for body_error in [false, true] {
        let mut values = vec![("first", 1)];
        let scope = ScopeFrame::new(&mut values);
        let drops = Rc::new(RefCell::new(vec![]));
        let poison = Native {
            id: 99,
            drops: drops.clone(),
            poison: true,
        };
        let row_drops = drops.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            snapshot_captured_scope_state(
                "r",
                &(),
                &scope,
                &mut (),
                limits(),
                CaptureSnapshotHooks {
                    row: move |_: &str, _: &usize, _: &mut ()| {
                        let _whole_capture = &poison;
                        Ok::<CapturedValue<Native, ()>, Native>(CapturedValue::Result(native(
                            1, &row_drops,
                        )))
                    },
                    body: |_: &(), _: &mut ()| {
                        if body_error {
                            Err(native(90, &drops))
                        } else {
                            Ok(native(3, &drops))
                        }
                    },
                },
            )
        }));
        assert!(result.is_err());
        assert!(drops.borrow().contains(&1));
        assert!(drops.borrow().contains(&99));
        assert!(drops.borrow().contains(if body_error { &90 } else { &3 }));
    }
}

#[test]
fn storage_preflight_is_not_name_uniqueness_scalar_or_native_authority_validation() {
    let name = String::from("not a lexical identifier");
    let mut values = vec![(name.as_str(), ()), (name.as_str(), ())];
    let scope = ScopeFrame::new(&mut values);
    let saved = snapshot_captured_scope_state(
        &name,
        &(),
        &scope,
        &mut (),
        limits(),
        CaptureSnapshotHooks {
            row: |_: &str, _: &(), _: &mut ()| {
                Ok::<CapturedValue<(), ()>, ()>(CapturedValue::Scalar(ScalarValue::String(
                    "x".repeat(leselang_runtime_core::MAX_SCALAR_STRING_BYTES + 1),
                )))
            },
            body: |_: &(), _: &mut ()| Ok(()),
        },
    )
    .unwrap();
    assert_eq!(saved.locals.len(), 2);
    assert_eq!(saved.locals[0].name, name);
    assert_ne!(saved.locals[0].name.as_ptr(), name.as_ptr());
    assert!(
        saved.locals[0].value.text().unwrap().len()
            > leselang_runtime_core::MAX_SCALAR_STRING_BYTES
    );
}

#[test]
fn snapshot_includes_visible_parent_prefix_without_mutating_either_scope() {
    let mut rows = vec![("parent", 0)];
    let mut parent = ScopeFrame::new(&mut rows);
    let mut nested = parent.nested();
    nested.push("child", 1).unwrap();
    let mut state = context(20);
    state.original = &raw const state.fuel;
    let saved = snapshot(&nested, &mut state).unwrap();
    assert_eq!(saved.locals[0].name, "parent");
    assert_eq!(saved.results[0].name, "child");
    assert_eq!(nested.local_len(), 1);
    drop(nested);
    assert_eq!(parent.bindings(), [("parent", 0)]);
}

#[test]
fn native_snapshot_and_reentry_round_trip_share_original_meter_without_fresh_grant() {
    let mut rows = vec![("n", ScalarValue::Integer(7))];
    let source = ScopeFrame::new(&mut rows);
    let mut fuel = Fuel::new(4);
    let saved = snapshot_captured_scope_state(
        "reply",
        &(),
        &source,
        &mut fuel,
        limits(),
        CaptureSnapshotHooks {
            row: |_: &str, v: &ScalarValue, f: &mut Fuel| {
                f.charge(1)?;
                Ok::<CapturedValue<(), ()>, leselang_runtime_core::FuelExhausted>(
                    CapturedValue::Scalar(v.clone()),
                )
            },
            body: |_: &(), _: &mut Fuel| Ok(()),
        },
    )
    .unwrap();
    let mut restored = vec![];
    let answer = with_captured_scope_state(
        &saved,
        &mut ScopeFrame::new(&mut restored),
        &mut fuel,
        CaptureScopeHooks {
            scalar: |v: &ScalarValue, f: &mut Fuel| {
                f.charge(1)?;
                Ok::<_, leselang_runtime_core::FuelExhausted>(v.clone())
            },
            result: |_: &(), _: &mut Fuel| Ok(ScalarValue::None),
            group: |_: &(), _: &mut Fuel| Ok(ScalarValue::None),
            pending: |f: &mut Fuel| {
                f.charge(1)?;
                Ok(ScalarValue::Boolean(true))
            },
            body: |_: &(), scope: &mut ScopeFrame<'_, '_, ScalarValue>, f: &mut Fuel| {
                f.charge(1)?;
                Ok(scope.get("n").cloned())
            },
        },
    )
    .unwrap();
    assert_eq!(answer, Some(ScalarValue::Integer(7)));
    assert_eq!(fuel.remaining(), 0);
    assert!(restored.is_empty());
    assert_eq!(source.len(), 1);
}
