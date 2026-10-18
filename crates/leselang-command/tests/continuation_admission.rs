use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_command::{
    ContinuationAdmissionError as Error, ContinuationAdmissionLimits, ProjectedBinding,
    ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding, ResultBindingLimits,
    Value, continuation_schema::ContinuationSchemaError, validate_continuation_frame,
};
use leselang_hir::{
    Effect, Type, computation::Computation, host_call::HostOperation,
    projection_access::ProjectionAccessLimits,
};
use leselang_runtime_core::{
    CaptureValidationLimits, ContinuationFrame, GroupCaptureLimits, ResourceBudgetError,
    ResourceBudgetLimits, ScalarValue,
};

fn limits() -> ContinuationAdmissionLimits {
    ContinuationAdmissionLimits {
        binding: ResultBindingLimits {
            captures: CaptureValidationLimits {
                max_saved_bindings: 4,
                max_name_bytes: 16,
            },
            groups: GroupCaptureLimits {
                min_members: 1,
                max_members: 2,
            },
            accesses: ProjectionAccessLimits {
                max_nodes: 32,
                max_depth: 8,
            },
        },
        budget: ResourceBudgetLimits {
            max_fuel: 100,
            max_deadline_ms: 200,
            max_deadline_at_ms: 300,
            max_output_items: 4,
        },
        max_group_sequence: 9,
    }
}

fn frame<T, R>(token: T, revision: R) -> ContinuationFrame<T, R, Type, Effect, ResultBinding> {
    ContinuationFrame {
        schema_version: 1,
        token,
        program_counter: 1,
        expected_revision: Some(revision),
        result_type: HostOperation::UiFocus.result_type(),
        pending_effect: Effect::UiFocus {
            node_id: "private-node".into(),
        },
        fuel_remaining: 100,
        deadline_ms: 200,
        deadline_at_ms: Some(300),
        max_output_items: 4,
        result_binding: None,
        group_result: None,
    }
}

fn binding() -> ResultBinding {
    ResultBinding {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: Computation::Literal {
            value: ScalarValue::None,
        },
    }
}

fn check(image: &ContinuationFrame<String, u8, Type, Effect, ResultBinding>) -> Result<(), Error> {
    validate_continuation_frame(
        image,
        limits(),
        String::as_str,
        |token| token == "valid",
        |_| true,
    )
}

#[test]
fn conflicting_faults_keep_structure_schema_token_counter_signature_binding_budget_order() {
    let mut image = frame("private-token".into(), 7);
    image.schema_version = 99;
    image.program_counter = 0;
    image.pending_effect = Effect::Sequence { steps: vec![] };
    image.result_type = Type::Structured;
    image.fuel_remaining = 101;
    image.deadline_ms = 0;
    image.deadline_at_ms = Some(0);
    image.max_output_items = 5;
    let mut saved = binding();
    saved.name = "x".repeat(limits().binding.captures.max_name_bytes + 1);
    image.result_binding = Some(Box::new(saved));
    assert_eq!(check(&image), Err(Error::Binding));
    image.result_binding.as_mut().unwrap().name = "reply".into();
    assert_eq!(
        check(&image),
        Err(Error::Schema(ContinuationSchemaError::Version {
            actual: 99,
            expected: 2
        }))
    );
    image.schema_version = 2;
    assert_eq!(check(&image), Err(Error::Token));
    image.token = "valid".into();
    assert_eq!(check(&image), Err(Error::ProgramCounter));
    image.program_counter = 1;
    assert_eq!(check(&image), Err(Error::StructuredEffect));
    image.pending_effect = Effect::UiFocus {
        node_id: "private-node".into(),
    };
    assert_eq!(check(&image), Err(Error::ResultType));
    image.result_type = HostOperation::UiFocus.result_type();
    image.result_binding.as_mut().unwrap().body = Computation::Local {
        name: "missing".into(),
    };
    assert_eq!(check(&image), Err(Error::Binding));
    image.result_binding.as_mut().unwrap().body = binding().body;
    for (expected, repair) in [
        (ResourceBudgetError::Fuel, 0),
        (ResourceBudgetError::Deadline, 1),
        (ResourceBudgetError::AbsoluteDeadline, 2),
        (ResourceBudgetError::Output, 3),
    ] {
        let before = serde_json::to_vec(&image).unwrap();
        assert_eq!(check(&image), Err(Error::Budget(expected)));
        assert_eq!(serde_json::to_vec(&image).unwrap(), before);
        match repair {
            0 => image.fuel_remaining = 100,
            1 => image.deadline_ms = 200,
            2 => image.deadline_at_ms = Some(300),
            _ => image.max_output_items = 4,
        }
    }
    assert_eq!(check(&image), Ok(()));
}

#[test]
fn explicit_limits_preserve_zero_and_full_width_policy_without_vm_defaults() {
    let mut image = frame("valid".into(), 7);
    image.fuel_remaining = 0;
    image.max_output_items = 0;
    assert_eq!(check(&image), Ok(()));
    image.fuel_remaining = u64::MAX;
    image.deadline_ms = u64::MAX;
    image.deadline_at_ms = Some(u64::MAX);
    image.max_output_items = usize::MAX;
    let mut policy = limits();
    policy.budget = ResourceBudgetLimits {
        max_fuel: u64::MAX,
        max_deadline_ms: u64::MAX,
        max_deadline_at_ms: u64::MAX,
        max_output_items: usize::MAX,
    };
    assert_eq!(
        validate_continuation_frame(&image, policy, String::as_str, |_| true, |_| true),
        Ok(())
    );
    image.schema_version = 6;
    image.group_result = Some("merge-9".into());
    policy.max_group_sequence = 8;
    assert_eq!(
        validate_continuation_frame(&image, policy, String::as_str, |_| true, |_| true),
        Err(Error::Schema(ContinuationSchemaError::Group))
    );
}

struct LocalOwner {
    text: String,
    drops: Rc<Cell<usize>>,
}
impl Drop for LocalOwner {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn opaque_move_only_local_owners_are_borrowed_without_identity_or_thread_traits() {
    let drops = Rc::new(Cell::new(0));
    let owner = |text: &str| LocalOwner {
        text: text.into(),
        drops: drops.clone(),
    };
    let mut image = frame(owner("token"), owner("revision"));
    image.group_result = Some(owner("merge-1"));
    image.schema_version = 6;
    let token = &image.token as *const LocalOwner;
    let group = image.group_result.as_ref().unwrap() as *const LocalOwner;
    let revision = image.expected_revision.as_ref().unwrap() as *const LocalOwner;
    let events = RefCell::new(Vec::new());
    assert_eq!(
        validate_continuation_frame(
            &image,
            limits(),
            |owner| {
                assert_eq!(owner as *const LocalOwner, group);
                events.borrow_mut().push("owner");
                &owner.text
            },
            |owner| {
                assert_eq!(owner as *const LocalOwner, token);
                events.borrow_mut().push("token");
                true
            },
            |_| panic!("no saved groups")
        ),
        Ok(())
    );
    assert_eq!(*events.borrow(), ["owner", "token"]);
    assert_eq!(
        image.expected_revision.as_ref().unwrap() as *const LocalOwner,
        revision
    );
    assert_eq!(drops.get(), 0);
    drop(image);
    assert_eq!(drops.get(), 3);
}

#[test]
fn group_structure_hooks_precede_owner_and_schema_then_full_binding_hooks() {
    let mut image = frame(String::from("valid"), 7);
    let mut saved = binding();
    saved.groups.push(ProjectedGroupBinding {
        name: "group".into(),
        group: ProjectedGroup {
            members: vec![ProjectedBinding {
                name: "member".into(),
                result: ProjectedResult::capture_version(
                    HostOperation::UiFocus,
                    &Value::UiFocus {
                        node_id: "private".into(),
                    },
                    1,
                )
                .unwrap(),
            }],
        },
    });
    image.result_binding = Some(Box::new(saved));
    image.schema_version = 8;
    image.group_result = Some("merge-1".into());
    let events = RefCell::new(Vec::new());
    assert_eq!(
        validate_continuation_frame(
            &image,
            limits(),
            |owner| {
                events.borrow_mut().push("owner");
                owner.as_str()
            },
            |_| {
                events.borrow_mut().push("token");
                true
            },
            |_| {
                events.borrow_mut().push("name");
                true
            }
        ),
        Ok(())
    );
    let events = events.into_inner();
    let owner = events.iter().position(|event| *event == "owner").unwrap();
    let token = events.iter().position(|event| *event == "token").unwrap();
    assert!(owner > 0 && token == owner + 1 && token + 1 < events.len());
    assert!(
        events[..owner]
            .iter()
            .chain(events[token + 1..].iter())
            .all(|event| *event == "name")
    );
    assert_eq!(
        validate_continuation_frame(
            &image,
            limits(),
            |_| panic!("after invalid structure"),
            |_| panic!("after invalid structure"),
            |_| false
        ),
        Err(Error::Binding)
    );
}

#[test]
fn callback_unwind_is_not_retried_and_skipped_hook_captures_are_dropped() {
    let mut image = frame("private-token".into(), 7);
    image.schema_version = 6;
    image.group_result = Some("merge-1".into());
    let calls = Cell::new(0);
    let drops = Rc::new(Cell::new(0));
    let captured = LocalOwner {
        text: "capture".into(),
        drops: drops.clone(),
    };
    let before = serde_json::to_vec(&image).unwrap();
    let result = catch_unwind(AssertUnwindSafe(|| {
        validate_continuation_frame(
            &image,
            limits(),
            |_| {
                calls.set(calls.get() + 1);
                panic!("native hook");
            },
            move |_| {
                drop(captured);
                panic!("token hook must be skipped");
            },
            |_| true,
        )
    }));
    assert!(result.is_err());
    assert_eq!(calls.get(), 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(serde_json::to_vec(&image).unwrap(), before);
    image.schema_version = 99;
    assert!(matches!(
        validate_continuation_frame(
            &image,
            limits(),
            String::as_str,
            |_| panic!("schema first"),
            |_| true
        ),
        Err(Error::Schema(_))
    ));
}

#[test]
fn frame_consistency_neither_authenticates_payload_nor_leaks_it_in_diagnostics() {
    let mut image = frame("private-token".into(), 7);
    let error = check(&image).unwrap_err();
    assert_eq!(error, Error::Token);
    assert!(!format!("{error}: {error:?}").contains("private"));
    image.pending_effect = Effect::UiFocus {
        node_id: "\0noncanonical".into(),
    };
    assert_eq!(
        validate_continuation_frame(&image, limits(), String::as_str, |_| true, |_| true),
        Ok(())
    );
    // Canonical payload admission and authenticated grants belong to the caller.
    assert_eq!(image.expected_revision, Some(7));
}
