use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_runtime_core::{
    CaptureStructureError as Error, CaptureValidationLimits, CaptureValidationSite as Site,
    CapturedBindings, GroupCaptureLimits, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    OptionalStringValue, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ScalarBinding,
    ScalarValue, StringListValue, validate_captured_bindings, validate_projected_group,
};

type Frame = CapturedBindings<u8, u8, u8>;

fn frame() -> Frame {
    CapturedBindings {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: 9,
    }
}
fn limits(count: usize, bytes: usize) -> CaptureValidationLimits {
    CaptureValidationLimits {
        max_saved_bindings: count,
        max_name_bytes: bytes,
    }
}
fn scalar(name: &str, value: ScalarValue) -> ScalarBinding {
    ScalarBinding {
        name: name.into(),
        value,
    }
}
fn result(name: &str, result: u8) -> ProjectedBinding<u8> {
    ProjectedBinding {
        name: name.into(),
        result,
    }
}
fn group_binding(name: &str, group: u8) -> ProjectedGroupBinding<u8> {
    ProjectedGroupBinding {
        name: name.into(),
        group,
    }
}
fn check(frame: &Frame, limits: CaptureValidationLimits) -> Result<(), Error<()>> {
    validate_captured_bindings(frame, limits, |_| Ok(()), |_| Ok(()), |_| Ok(()))
}
fn members(names: &[&str]) -> ProjectedGroup<u8> {
    ProjectedGroup {
        members: names
            .iter()
            .enumerate()
            .map(|(i, name)| result(name, i as u8))
            .collect(),
    }
}

#[test]
fn explicit_zero_and_inclusive_limits_do_not_count_the_pending_name_or_repair_policy() {
    let mut f = frame();
    assert_eq!(check(&f, limits(0, 5)), Ok(()));
    assert_eq!(check(&f, limits(0, 4)), Err(Error::PendingName));
    f.name.clear();
    assert_eq!(check(&f, limits(0, 0)), Ok(()));
    f.locals.push(scalar("", ScalarValue::None));
    f.results.push(result("", 1));
    f.groups.push(group_binding("", 2));
    assert_eq!(check(&f, limits(3, 0)), Ok(()));
    assert_eq!(check(&f, limits(2, 0)), Err(Error::BindingCount));
    assert_eq!(check(&f, limits(0, 0)), Err(Error::BindingCount));
}

#[test]
fn name_limits_use_original_utf8_bytes_at_every_saved_position() {
    let name = "\u{754c}";
    let mut f = frame();
    f.name = name.into();
    f.locals.push(scalar(name, ScalarValue::None));
    f.results.push(result(name, 1));
    f.groups.push(group_binding(name, 2));
    assert_eq!(check(&f, limits(3, 3)), Ok(()));
    assert_eq!(check(&f, limits(3, 2)), Err(Error::PendingName));
    f.name.clear();
    assert_eq!(check(&f, limits(3, 2)), Err(Error::ScalarName { index: 0 }));
    f.locals[0].name.clear();
    assert_eq!(check(&f, limits(3, 2)), Err(Error::ResultName { index: 0 }));
    f.results[0].name.clear();
    assert_eq!(check(&f, limits(3, 2)), Err(Error::GroupName { index: 0 }));
}

#[test]
fn all_six_scalar_types_keep_inclusive_language_bounds_and_list_aggregate_limits() {
    for value in [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
        ScalarValue::None,
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES)),
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::OptionalString(OptionalStringValue(Some(String::new()))),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "x".repeat(MAX_SCALAR_STRING_BYTES),
        ))),
        ScalarValue::StringList(StringListValue(vec![String::new(); MAX_STRING_LIST_ITEMS])),
        ScalarValue::StringList(StringListValue(vec!["x".repeat(MAX_SCALAR_STRING_BYTES)])),
    ] {
        let mut f = frame();
        f.locals.push(scalar("x", value));
        assert_eq!(check(&f, limits(1, 5)), Ok(()));
    }
    for value in [
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
        ScalarValue::StringList(StringListValue(vec![
            String::new();
            MAX_STRING_LIST_ITEMS + 1
        ])),
        ScalarValue::StringList(StringListValue(vec![
            "x".repeat(MAX_SCALAR_STRING_BYTES),
            "y".into(),
        ])),
    ] {
        let mut f = frame();
        f.locals.push(scalar("x", ScalarValue::None));
        f.locals.push(scalar("y", value));
        assert_eq!(
            check(&f, limits(2, 5)),
            Err(Error::ScalarValue { index: 1 })
        );
    }
}

#[test]
fn count_pending_and_scalar_refusals_call_no_native_hook_in_legacy_order() {
    let mut f = frame();
    f.name = "too long".into();
    f.locals.push(scalar(
        "too long",
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
    ));
    f.results.push(result("too long", 1));
    f.groups.push(group_binding("too long", 2));
    let calls = Cell::new(0);
    let run = |f: &Frame, policy| {
        let hook = |_: &u8| -> Result<(), ()> {
            calls.set(calls.get() + 1);
            Err(())
        };
        validate_captured_bindings(f, policy, hook, hook, hook)
    };
    assert_eq!(run(&f, limits(2, 5)), Err(Error::BindingCount));
    assert_eq!(run(&f, limits(3, 5)), Err(Error::PendingName));
    f.name.clear();
    assert_eq!(run(&f, limits(3, 5)), Err(Error::ScalarName { index: 0 }));
    f.locals[0].name.clear();
    assert_eq!(run(&f, limits(3, 5)), Err(Error::ScalarValue { index: 0 }));
    assert_eq!(calls.get(), 0);
}

#[test]
fn result_and_group_names_interleave_with_once_only_native_calls_and_body_runs_last() {
    let mut f = frame();
    f.results = vec![result("a", 1), result("too long", 2)];
    f.groups = vec![group_binding("g", 3), group_binding("too long", 4)];
    let calls = RefCell::new(vec![]);
    let run = |f: &Frame| {
        validate_captured_bindings(
            f,
            limits(4, 5),
            |v| {
                calls.borrow_mut().push(*v);
                Ok::<_, ()>(())
            },
            |v| {
                calls.borrow_mut().push(*v);
                Ok(())
            },
            |v| {
                calls.borrow_mut().push(*v);
                Ok(())
            },
        )
    };
    assert_eq!(run(&f), Err(Error::ResultName { index: 1 }));
    assert_eq!(*calls.borrow(), [1]);
    calls.borrow_mut().clear();
    f.results[1].name = "b".into();
    assert_eq!(run(&f), Err(Error::GroupName { index: 1 }));
    assert_eq!(*calls.borrow(), [1, 2, 3]);
    calls.borrow_mut().clear();
    f.groups[1].name = "h".into();
    assert_eq!(run(&f), Ok(()));
    assert_eq!(*calls.borrow(), [1, 2, 3, 4, 9]);
}

#[test]
fn native_failures_retain_original_channel_position_and_stop_before_later_rows() {
    let mut f = frame();
    f.results = vec![result("a", 1), result("b", 2)];
    f.groups = vec![group_binding("g", 3), group_binding("h", 4)];
    for (failure, site, expected) in [
        (1, Site::Result { index: 0 }, vec![1]),
        (2, Site::Result { index: 1 }, vec![1, 2]),
        (3, Site::Group { index: 0 }, vec![1, 2, 3]),
        (4, Site::Group { index: 1 }, vec![1, 2, 3, 4]),
        (9, Site::Body, vec![1, 2, 3, 4, 9]),
    ] {
        let calls = RefCell::new(vec![]);
        let original = match site {
            Site::Result { index } => &f.results[index].result,
            Site::Group { index } => &f.groups[index].group,
            Site::Body => &f.body,
            Site::Member { .. } => unreachable!(),
        };
        let hook = |v: &u8| {
            calls.borrow_mut().push(*v);
            if *v == failure { Err(original) } else { Ok(()) }
        };
        let outcome = validate_captured_bindings(&f, limits(4, 5), hook, hook, hook);
        let Err(Error::Native {
            site: actual,
            error,
        }) = outcome
        else {
            panic!("native refusal expected")
        };
        assert_eq!(actual, site);
        assert_eq!(*error, failure);
        assert!(std::ptr::eq(error, original));
        assert_eq!(*calls.borrow(), expected);
    }
}

#[test]
fn groups_admit_zero_inclusive_and_inverted_bounds_without_implicit_policy() {
    let empty = members(&[]);
    let one = members(&["a"]);
    let two = members(&["a", "b"]);
    let calls = Cell::new(0);
    let run = |g: &ProjectedGroup<u8>, min_members, max_members| {
        validate_projected_group(
            g,
            GroupCaptureLimits {
                min_members,
                max_members,
            },
            |_| {
                calls.set(calls.get() + 1);
                true
            },
            |_| {
                calls.set(calls.get() + 1);
                Ok::<_, ()>(())
            },
        )
    };
    assert_eq!(run(&empty, 0, 0), Ok(()));
    assert_eq!(run(&empty, 1, 2), Err(Error::MemberCount));
    assert_eq!(run(&one, 0, 0), Err(Error::MemberCount));
    for g in [&empty, &one, &two] {
        assert_eq!(run(g, 2, 1), Err(Error::MemberCount));
    }
    assert_eq!(calls.get(), 0);
    assert_eq!(run(&one, 1, 2), Ok(()));
    assert_eq!(run(&two, 1, 2), Ok(()));
    assert_eq!(calls.get(), 6);
}

#[test]
fn group_names_are_borrowed_and_all_preflight_before_results_without_normalization() {
    let g = members(&["a", "A", "\u{754c}"]);
    let events = RefCell::new(vec![]);
    assert_eq!(
        validate_projected_group(
            &g,
            GroupCaptureLimits {
                min_members: 3,
                max_members: 3
            },
            |name| {
                let index = events.borrow().len();
                assert_eq!(name.as_ptr(), g.members[index].name.as_ptr());
                events.borrow_mut().push(format!("name:{name}"));
                true
            },
            |v| {
                events.borrow_mut().push(format!("result:{v}"));
                Ok::<_, ()>(())
            }
        ),
        Ok(())
    );
    assert_eq!(
        *events.borrow(),
        [
            "name:a",
            "name:A",
            "name:\u{754c}",
            "result:0",
            "result:1",
            "result:2"
        ]
    );
}

#[test]
fn invalid_group_names_precede_same_row_duplicates_and_never_run_result_hooks() {
    let g = members(&["a", "a", "b"]);
    let names = Cell::new(0);
    let results = Cell::new(0);
    let policy = GroupCaptureLimits {
        min_members: 1,
        max_members: 3,
    };
    let run = |reject_second| {
        validate_projected_group(
            &g,
            policy,
            |_| {
                names.set(names.get() + 1);
                !(reject_second && names.get() == 2)
            },
            |_| {
                results.set(results.get() + 1);
                Ok::<_, ()>(())
            },
        )
    };
    assert_eq!(run(true), Err(Error::MemberName { index: 1 }));
    assert_eq!(names.get(), 2);
    names.set(0);
    assert_eq!(run(false), Err(Error::DuplicateMember { index: 1 }));
    assert_eq!(names.get(), 2);
    assert_eq!(results.get(), 0);
}

#[test]
fn group_result_errors_keep_member_position_and_do_not_repeat_preflight_or_later_results() {
    let g = members(&["a", "b", "c"]);
    let names = Cell::new(0);
    let results = RefCell::new(vec![]);
    let outcome = validate_projected_group(
        &g,
        GroupCaptureLimits {
            min_members: 1,
            max_members: 3,
        },
        |_| {
            names.set(names.get() + 1);
            true
        },
        |v| {
            results.borrow_mut().push(*v);
            if *v == 1 { Err("native") } else { Ok(()) }
        },
    );
    assert_eq!(
        outcome,
        Err(Error::Native {
            site: Site::Member { index: 1 },
            error: "native"
        })
    );
    assert_eq!(names.get(), 3);
    assert_eq!(*results.borrow(), [0, 1]);
}

#[test]
fn opaque_move_only_thread_local_payloads_keep_buffers_and_error_ownership_without_trait_bounds() {
    struct Native {
        calls: Rc<Cell<usize>>,
    }
    struct NativeError {
        secret: String,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for NativeError {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let calls = Rc::new(Cell::new(0));
    let drops = Rc::new(Cell::new(0));
    let native = || Native {
        calls: Rc::clone(&calls),
    };
    let f = CapturedBindings {
        name: "original".into(),
        locals: vec![scalar("x", ScalarValue::String("saved".into()))],
        results: vec![ProjectedBinding {
            name: "r".into(),
            result: native(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "g".into(),
            group: members(&["member"]),
        }],
        body: native(),
    };
    let name_pointer = f.name.as_ptr();
    let scalar_pointer = f.locals[0].value.text().unwrap().as_ptr();
    let error = NativeError {
        secret: "native secret".into(),
        drops: Rc::clone(&drops),
    };
    let secret_pointer = error.secret.as_ptr();
    let mut original_error = Some(error);
    let outcome = validate_captured_bindings(
        &f,
        limits(3, 8),
        |v| {
            v.calls.set(v.calls.get() + 1);
            Err(original_error.take().unwrap())
        },
        |_| -> Result<(), NativeError> { panic!("group must remain cold") },
        |_| -> Result<(), NativeError> { panic!("body must remain cold") },
    );
    let Err(Error::Native { site, error }) = outcome else {
        panic!("opaque error expected")
    };
    assert_eq!(site, Site::Result { index: 0 });
    assert_eq!(error.secret.as_ptr(), secret_pointer);
    assert_eq!(f.name.as_ptr(), name_pointer);
    assert_eq!(f.locals[0].value.text().unwrap().as_ptr(), scalar_pointer);
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 0);
    drop(error);
    assert_eq!(drops.get(), 1);
}

#[test]
fn diagnostics_never_invoke_native_formatters_or_expose_original_payloads() {
    struct NativeError;
    impl std::fmt::Debug for NativeError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("native formatter")
        }
    }
    impl std::fmt::Display for NativeError {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("native formatter")
        }
    }
    let error = Error::Native {
        site: Site::Body,
        error: NativeError,
    };
    assert_eq!(format!("{error}"), "native capture validation rejected");
    assert_eq!(format!("{error:?}"), "Native { site: Body, .. }");
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(
        format!("{:?}", Error::<NativeError>::DuplicateMember { index: 3 }),
        "captured group member name is duplicated"
    );
}

#[test]
fn callback_unwind_propagates_without_replay_rollback_or_core_field_mutation() {
    let mut f = frame();
    f.results = vec![result("a", 1), result("b", 2)];
    let wire = serde_json::to_vec(&f).unwrap();
    let calls = Cell::new(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        validate_captured_bindings(
            &f,
            limits(2, 5),
            |_| -> Result<(), ()> {
                calls.set(calls.get() + 1);
                panic!("native unwind")
            },
            |_| panic!("group cold"),
            |_| panic!("body cold"),
        )
    }));
    assert!(outcome.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(serde_json::to_vec(&f).unwrap(), wire);
    let g = members(&["a", "b"]);
    let names = Cell::new(0);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        validate_projected_group(
            &g,
            GroupCaptureLimits {
                min_members: 1,
                max_members: 2,
            },
            |_| {
                names.set(names.get() + 1);
                panic!("predicate unwind")
            },
            |_| -> Result<(), ()> { panic!("results cold") },
        )
    }));
    assert!(outcome.is_err());
    assert_eq!(names.get(), 1);
    assert_eq!(g.members[0].name, "a");
}

#[test]
fn unrelated_native_body_and_group_schemas_remain_opaque_and_not_execution_certificates() {
    struct TextBody(String);
    struct Visibility(bool);
    let f = CapturedBindings {
        name: "visible".into(),
        locals: vec![],
        results: vec![ProjectedBinding {
            name: "target".into(),
            result: Visibility(true),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "windows".into(),
            group: vec![17_u32, 4],
        }],
        body: TextBody("original".into()),
    };
    assert_eq!(
        validate_captured_bindings(
            &f,
            limits(2, 7),
            |v| if v.0 { Ok::<_, ()>(()) } else { Err(()) },
            |v| if v == &[17, 4] { Ok(()) } else { Err(()) },
            |v| if v.0 == "original" { Ok(()) } else { Err(()) }
        ),
        Ok(())
    );
    let mut mutable = frame();
    assert_eq!(check(&mutable, limits(1, 5)), Ok(()));
    mutable.locals.push(scalar(
        "x",
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
    ));
    assert_eq!(
        check(&mutable, limits(1, 5)),
        Err(Error::ScalarValue { index: 0 })
    );
    let mut mutable_group = members(&["a", "b"]);
    let policy = GroupCaptureLimits {
        min_members: 1,
        max_members: 2,
    };
    assert_eq!(
        validate_projected_group(&mutable_group, policy, |_| true, |_| Ok::<_, ()>(())),
        Ok(())
    );
    mutable_group.members[1].name = "a".into();
    assert_eq!(
        validate_projected_group(&mutable_group, policy, |_| true, |_| Ok::<_, ()>(())),
        Err(Error::DuplicateMember { index: 1 })
    );
}
