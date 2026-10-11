use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CaptureScopeError as Error, CaptureScopeSite as Site, CapturedBindings, ProjectedBinding,
    ProjectedGroupBinding, ScalarBinding, ScalarValue, ScopeFrame, with_captured_scope,
};

type Capture = CapturedBindings<u8, u8, u8>;
fn capture() -> Capture {
    CapturedBindings {
        name: "pending".into(),
        locals: vec![
            ScalarBinding {
                name: "s0".into(),
                value: ScalarValue::Integer(7),
            },
            ScalarBinding {
                name: "s1".into(),
                value: ScalarValue::Integer(8),
            },
        ],
        results: vec![
            ProjectedBinding {
                name: "r0".into(),
                result: 20,
            },
            ProjectedBinding {
                name: "r1".into(),
                result: 21,
            },
        ],
        groups: vec![
            ProjectedGroupBinding {
                name: "g0".into(),
                group: 30,
            },
            ProjectedGroupBinding {
                name: "g1".into(),
                group: 31,
            },
        ],
        body: 50,
    }
}
fn scalar_id(value: &ScalarValue) -> u8 {
    let ScalarValue::Integer(n) = value else {
        panic!("integer fixture expected")
    };
    u8::try_from(*n).unwrap()
}
#[derive(Clone, Copy)]
enum Stop {
    None,
    Error(u8),
    Panic(u8),
}
struct Tracked {
    id: u8,
    drops: Rc<RefCell<Vec<u8>>>,
    panic_on_drop: bool,
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.borrow_mut().push(self.id);
        assert!(!self.panic_on_drop, "native destructor unwind");
    }
}
fn tracked(id: u8, drops: &Rc<RefCell<Vec<u8>>>) -> Tracked {
    Tracked {
        id,
        drops: Rc::clone(drops),
        panic_on_drop: false,
    }
}
fn prepare(
    id: u8,
    stop: Stop,
    events: &RefCell<Vec<u8>>,
    drops: &Rc<RefCell<Vec<u8>>>,
) -> Result<Tracked, ()> {
    events.borrow_mut().push(id);
    match stop {
        Stop::Error(target) if target == id => Err(()),
        Stop::Panic(target) if target == id => panic!("native hook unwind"),
        _ => Ok(tracked(id, drops)),
    }
}
fn run<'a>(
    f: &'a Capture,
    parent: &mut ScopeFrame<'_, 'a, Tracked>,
    stop: Stop,
    events: &RefCell<Vec<u8>>,
    drops: &Rc<RefCell<Vec<u8>>>,
) -> Result<Box<u8>, Error<()>> {
    with_captured_scope(
        f,
        parent,
        |v| prepare(scalar_id(v), stop, events, drops),
        |v| prepare(*v, stop, events, drops),
        |v| prepare(*v, stop, events, drops),
        || prepare(40, stop, events, drops),
        |v, scope| {
            events.borrow_mut().push(*v);
            assert_eq!(scope.local_len(), 7);
            assert_eq!(scope.get("parent").unwrap().id, 0);
            match stop {
                Stop::Error(50) => Err(()),
                Stop::Panic(50) => panic!("body unwind"),
                _ => Ok(Box::new(*v)),
            }
        },
    )
}

#[test]
fn original_scalar_result_group_pending_order_and_once_only_body_share_the_parent_stack() {
    let f = capture();
    let events = RefCell::new(vec![]);
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = Vec::with_capacity(8);
    bindings.push(("parent", tracked(0, &drops)));
    let buffer = bindings.as_ptr();
    let mut parent = ScopeFrame::new(&mut bindings);
    let output = run(&f, &mut parent, Stop::None, &events, &drops).unwrap();
    assert_eq!(*output, 50);
    assert_eq!(*events.borrow(), [7, 8, 20, 21, 30, 31, 40, 50]);
    assert_eq!(*drops.borrow(), [40, 31, 30, 21, 20, 8, 7]);
    assert_eq!(parent.local_len(), 0);
    assert_eq!(parent.bindings().as_ptr(), buffer);
    assert_eq!(parent.get("parent").unwrap().id, 0);
    drop(parent);
    drop(bindings);
    assert_eq!(*drops.borrow(), [40, 31, 30, 21, 20, 8, 7, 0]);
}

#[test]
fn every_saved_namespace_and_pending_collision_prepares_once_then_restores_original_prefix() {
    for (position, prior, site) in [
        (0, "parent", Site::Scalar { index: 0 }),
        (1, "s0", Site::Scalar { index: 1 }),
        (2, "s0", Site::Result { index: 0 }),
        (3, "r0", Site::Result { index: 1 }),
        (4, "s0", Site::Group { index: 0 }),
        (5, "g0", Site::Group { index: 1 }),
        (6, "parent", Site::Pending),
        (6, "s0", Site::Pending),
        (6, "r0", Site::Pending),
        (6, "g0", Site::Pending),
    ] {
        let mut f = capture();
        match position {
            0..=1 => f.locals[position].name = prior.into(),
            2..=3 => f.results[position - 2].name = prior.into(),
            4..=5 => f.groups[position - 4].name = prior.into(),
            _ => f.name = prior.into(),
        }
        let bytes = serde_json::to_vec(&f).unwrap();
        let events = RefCell::new(vec![]);
        let drops = Rc::new(RefCell::new(vec![]));
        let mut bindings = vec![("parent", tracked(0, &drops))];
        let mut parent = ScopeFrame::new(&mut bindings);
        assert_eq!(
            run(&f, &mut parent, Stop::None, &events, &drops),
            Err(Error::Duplicate { site })
        );
        let visited = &[7, 8, 20, 21, 30, 31, 40][..=position];
        assert_eq!(*events.borrow(), visited);
        assert_eq!(
            *drops.borrow(),
            visited.iter().rev().copied().collect::<Vec<_>>()
        );
        assert_eq!(parent.len(), 1);
        assert_eq!(parent.get("parent").unwrap().id, 0);
        assert_eq!(serde_json::to_vec(&f).unwrap(), bytes);
    }
}

#[test]
fn native_refusal_precedes_same_row_collision_and_preserves_interleaved_error_position() {
    let mut f = capture();
    f.results[0].name = "s0".into();
    let events = RefCell::new(vec![]);
    let drops = Rc::new(RefCell::new(vec![]));
    let mut bindings = vec![("parent", tracked(0, &drops))];
    let mut parent = ScopeFrame::new(&mut bindings);
    assert_eq!(
        run(&f, &mut parent, Stop::Error(20), &events, &drops),
        Err(Error::Native {
            site: Site::Result { index: 0 },
            error: (),
        })
    );
    assert_eq!(*events.borrow(), [7, 8, 20]);
    assert_eq!(*drops.borrow(), [8, 7]);
    assert_eq!(parent.len(), 1);
}

#[test]
fn every_native_error_and_unwind_cleans_only_local_values_without_replay_or_mutation_rollback() {
    let f = capture();
    for (id, site, count) in [
        (7, Site::Scalar { index: 0 }, 1),
        (8, Site::Scalar { index: 1 }, 2),
        (20, Site::Result { index: 0 }, 3),
        (21, Site::Result { index: 1 }, 4),
        (30, Site::Group { index: 0 }, 5),
        (31, Site::Group { index: 1 }, 6),
        (40, Site::Pending, 7),
        (50, Site::Body, 8),
    ] {
        for panic in [false, true] {
            let events = RefCell::new(vec![]);
            let drops = Rc::new(RefCell::new(vec![]));
            let mut bindings = vec![("parent", tracked(0, &drops))];
            let mut parent = ScopeFrame::new(&mut bindings);
            let stop = if panic {
                Stop::Panic(id)
            } else {
                Stop::Error(id)
            };
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                run(&f, &mut parent, stop, &events, &drops)
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert_eq!(outcome.unwrap(), Err(Error::Native { site, error: () }));
            }
            let visited = &[7, 8, 20, 21, 30, 31, 40, 50][..count];
            assert_eq!(*events.borrow(), visited);
            let prepared = if id == 50 {
                &visited[..7]
            } else {
                &visited[..count - 1]
            };
            assert_eq!(
                *drops.borrow(),
                prepared.iter().rev().copied().collect::<Vec<_>>()
            );
            assert_eq!(parent.len(), 1);
            assert_eq!(parent.local_len(), 0);
        }
    }
}

#[test]
fn borrowed_move_only_gui_local_payloads_names_and_outputs_need_no_native_traits() {
    struct Native(Rc<Cell<usize>>);
    enum Slot<'a> {
        Scalar(&'a ScalarValue),
        Result(&'a Native),
        Group(&'a Native),
        Pending(Box<Native>),
    }
    let state = Rc::new(Cell::new(0));
    let native = || Native(Rc::clone(&state));
    let f = CapturedBindings {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "scalar".into(),
            value: ScalarValue::String("original".into()),
        }],
        results: vec![ProjectedBinding {
            name: "result".into(),
            result: native(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: native(),
        }],
        body: native(),
    };
    let pending = Box::new(native());
    let pending_pointer = std::ptr::from_ref(pending.as_ref());
    let output = Box::new(native());
    let output_pointer = std::ptr::from_ref(output.as_ref());
    let mut bindings = vec![];
    let mut parent = ScopeFrame::new(&mut bindings);
    let answer = with_captured_scope(
        &f,
        &mut parent,
        |v| Ok::<_, ()>(Slot::Scalar(v)),
        |v| Ok(Slot::Result(v)),
        |v| Ok(Slot::Group(v)),
        || Ok(Slot::Pending(pending)),
        |body, scope| {
            for (actual, original) in scope.bindings().iter().zip([
                f.locals[0].name.as_str(),
                f.results[0].name.as_str(),
                f.groups[0].name.as_str(),
                f.name.as_str(),
            ]) {
                assert_eq!(actual.0.as_ptr(), original.as_ptr());
            }
            let Slot::Scalar(v) = scope.get("scalar").unwrap() else {
                panic!()
            };
            assert!(std::ptr::eq(*v, &f.locals[0].value));
            let Slot::Result(v) = scope.get("result").unwrap() else {
                panic!()
            };
            assert!(std::ptr::eq(*v, &f.results[0].result));
            let Slot::Group(v) = scope.get("group").unwrap() else {
                panic!()
            };
            assert!(std::ptr::eq(*v, &f.groups[0].group));
            let Slot::Pending(v) = scope.get("pending").unwrap() else {
                panic!()
            };
            assert_eq!(std::ptr::from_ref(v.as_ref()), pending_pointer);
            body.0.set(body.0.get() + 1);
            Ok(output)
        },
    )
    .unwrap();
    assert_eq!(std::ptr::from_ref(answer.as_ref()), output_pointer);
    assert_eq!(state.get(), 1);
    assert!(parent.is_empty());
}

#[test]
fn opaque_owned_and_borrowed_native_failures_keep_original_ownership_and_redacted_diagnostics() {
    struct NativeError {
        value: Box<u8>,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for NativeError {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let f = capture();
    let drops = Rc::new(Cell::new(0));
    let error = NativeError {
        value: Box::new(5),
        drops: Rc::clone(&drops),
    };
    let pointer = std::ptr::from_ref(error.value.as_ref());
    let mut bindings = vec![];
    let mut parent = ScopeFrame::new(&mut bindings);
    let outcome = with_captured_scope(
        &f,
        &mut parent,
        |_| Ok(()),
        |_| Ok(()),
        |_| Ok(()),
        || Ok(()),
        |_, _| Err::<(), _>(error),
    );
    let Err(Error::Native { site, error }) = outcome else {
        panic!()
    };
    assert_eq!(site, Site::Body);
    assert_eq!(std::ptr::from_ref(error.value.as_ref()), pointer);
    assert_eq!(drops.get(), 0);
    drop(error);
    assert_eq!(drops.get(), 1);
    let borrowed = with_captured_scope(
        &f,
        &mut parent,
        |_| Ok(()),
        |_| Err(&f.results[0].result),
        |_| Ok(()),
        || Ok(()),
        |_, _| Ok(()),
    );
    let Err(Error::Native { site, error }) = borrowed else {
        panic!()
    };
    assert_eq!(site, Site::Result { index: 0 });
    assert!(std::ptr::eq(error, &f.results[0].result));
    assert!(parent.is_empty());
    struct Formatter;
    impl std::fmt::Debug for Formatter {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("native debug")
        }
    }
    impl std::fmt::Display for Formatter {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("native display")
        }
    }
    let error = Error::Native {
        site: Site::Body,
        error: Formatter,
    };
    assert_eq!(format!("{error}"), "native captured scope hook rejected");
    assert_eq!(format!("{error:?}"), "Native { site: Body, .. }");
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(
        format!(
            "{}",
            Error::<Formatter>::Duplicate {
                site: Site::Pending
            }
        ),
        "captured scope duplicates an active name"
    );
}

#[test]
fn nested_capture_entry_keeps_outer_names_live_and_reuses_closed_inner_names() {
    let f = capture();
    let inner: CapturedBindings<(), (), ()> = CapturedBindings {
        name: "inner".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: (),
    };
    let mut bindings = vec![("parent", 1_u8)];
    let mut parent = ScopeFrame::new(&mut bindings);
    with_captured_scope(
        &f,
        &mut parent,
        |_| Ok::<_, ()>(2),
        |_| Ok(3),
        |_| Ok(4),
        || Ok(5),
        |_, scope| {
            for _ in 0..2 {
                with_captured_scope(
                    &inner,
                    scope,
                    |_| Ok::<_, ()>(0),
                    |_| Ok(0),
                    |_| Ok(0),
                    || Ok(6),
                    |_, nested| {
                        assert_eq!(nested.get("s0"), Some(&2));
                        assert_eq!(nested.local_len(), 1);
                        Ok(())
                    },
                )
                .unwrap();
                assert!(scope.get("inner").is_none());
                assert_eq!(scope.get("pending"), Some(&5));
            }
            let mut nested = scope.nested();
            assert!(nested.push("pending", 9).is_err());
            nested.push("local", 9).unwrap();
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(parent.bindings(), [("parent", 1)]);
}

#[test]
fn parent_existing_duplicate_prefix_and_unchecked_bounds_are_not_silently_validated_or_repaired() {
    let mut f: CapturedBindings<(), (), ()> = CapturedBindings {
        name: "not a normalized name".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: (),
    };
    f.locals.push(ScalarBinding {
        name: "".into(),
        value: ScalarValue::String("x".repeat(4097)),
    });
    let mut bindings = vec![("parent", 1), ("parent", 2)];
    let mut parent = ScopeFrame::new(&mut bindings);
    with_captured_scope(
        &f,
        &mut parent,
        |_| Ok::<_, ()>(3),
        |_| Ok(0),
        |_| Ok(0),
        || Ok(4),
        |_, scope| {
            assert_eq!(scope.get("parent"), Some(&1));
            assert_eq!(scope.get(""), Some(&3));
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(parent.bindings(), [("parent", 1), ("parent", 2)]);
    assert!(!f.locals[0].value.is_bounded());
}

#[test]
fn second_unrelated_native_schema_maps_data_explicitly_without_inferred_projection_or_authority() {
    struct TextBody(String);
    struct Visibility(bool);
    enum Slot {
        Visible(bool),
        Windows(usize),
        Pending,
    }
    let f = CapturedBindings {
        name: "ready".into(),
        locals: vec![],
        results: vec![ProjectedBinding {
            name: "target".into(),
            result: Visibility(true),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "windows".into(),
            group: vec!["a", "b"],
        }],
        body: TextBody("original".into()),
    };
    let mut bindings = vec![];
    let mut parent = ScopeFrame::new(&mut bindings);
    let value = with_captured_scope(
        &f,
        &mut parent,
        |_| Ok::<_, ()>(Slot::Pending),
        |v| Ok(Slot::Visible(v.0)),
        |v| Ok(Slot::Windows(v.len())),
        || Ok(Slot::Pending),
        |body, scope| {
            assert_eq!(body.0, "original");
            assert!(matches!(scope.get("target"), Some(Slot::Visible(true))));
            assert!(matches!(scope.get("windows"), Some(Slot::Windows(2))));
            Ok(body.0.as_str())
        },
    )
    .unwrap();
    assert!(std::ptr::eq(value.as_ptr(), f.body.0.as_ptr()));
    assert!(parent.is_empty());
}

#[test]
fn destructor_unwind_detaches_the_whole_local_suffix_and_preserves_parent_and_once_only_body() {
    let f = capture();
    for failure in [false, true] {
        let drops = Rc::new(RefCell::new(vec![]));
        let calls = Cell::new(0);
        let mut bindings = vec![("parent", tracked(0, &drops))];
        let mut parent = ScopeFrame::new(&mut bindings);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_captured_scope(
                &f,
                &mut parent,
                |v| Ok::<_, Tracked>(tracked(scalar_id(v), &drops)),
                |v| {
                    let mut value = tracked(*v, &drops);
                    value.panic_on_drop = *v == 21;
                    Ok(value)
                },
                |v| Ok(tracked(*v, &drops)),
                || Ok(tracked(40, &drops)),
                |_, _| {
                    calls.set(calls.get() + 1);
                    if failure {
                        Err(tracked(90, &drops))
                    } else {
                        Ok(tracked(90, &drops))
                    }
                },
            )
        }));
        assert!(outcome.is_err());
        assert_eq!(calls.get(), 1);
        let mut actual = drops.borrow().clone();
        actual.sort_unstable();
        assert_eq!(actual, [7, 8, 20, 21, 30, 31, 40, 90]);
        assert_eq!(parent.len(), 1);
        assert_eq!(parent.get("parent").unwrap().id, 0);
    }
}

#[test]
fn early_native_error_is_released_if_previously_prepared_scope_cleanup_unwinds() {
    let f = capture();
    let drops = Rc::new(RefCell::new(vec![]));
    let calls = RefCell::new(vec![]);
    let mut bindings = vec![("parent", tracked(0, &drops))];
    let mut parent = ScopeFrame::new(&mut bindings);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        with_captured_scope(
            &f,
            &mut parent,
            |v| {
                calls.borrow_mut().push(scalar_id(v));
                Ok::<_, Tracked>(tracked(scalar_id(v), &drops))
            },
            |v| {
                calls.borrow_mut().push(*v);
                if *v == 21 {
                    Err(tracked(90, &drops))
                } else {
                    let mut value = tracked(*v, &drops);
                    value.panic_on_drop = true;
                    Ok(value)
                }
            },
            |_| panic!("group cold"),
            || panic!("pending cold"),
            |_, _| -> Result<(), Tracked> { panic!("body cold") },
        )
    }));
    assert!(outcome.is_err());
    assert_eq!(*calls.borrow(), [7, 8, 20, 21]);
    let mut actual = drops.borrow().clone();
    actual.sort_unstable();
    assert_eq!(actual, [7, 8, 20, 90]);
    assert_eq!(parent.len(), 1);
    assert_eq!(parent.get("parent").unwrap().id, 0);
}
