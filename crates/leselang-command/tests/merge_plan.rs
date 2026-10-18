use leselang_command::{
    ExecutionOrder, GroupResultBinding, MAX_MERGE_BRANCH_NAME_BYTES, MAX_MERGE_BRANCHES, MergePlan,
    MergePlanShapeError, MergePlanShapeLimits, ResultBinding, valid_merge_branch_name,
};
use leselang_command::{GroupPlanLimits, InvalidGroupPlan};
use leselang_hir::{Effect, HirBranch, Type, computation::Computation};
use leselang_runtime_core::ScalarValue;

#[cfg(test)]
mod group_admission {
    use super::*;
    use leselang_command::{GroupBindingError, GroupBindingLimits, validate_group_binding};
    use leselang_hir::projection_access::ProjectionAccessLimits;
    use leselang_runtime_core::{CaptureValidationLimits, GroupCaptureLimits};
    use std::{cell::Cell, rc::Rc};

    pub(super) fn policy() -> GroupBindingLimits {
        GroupBindingLimits {
            plan: group_limits(),
            binding: leselang_command::ResultBindingLimits {
                captures: CaptureValidationLimits {
                    max_saved_bindings: 16,
                    max_name_bytes: 64,
                },
                groups: GroupCaptureLimits {
                    min_members: 1,
                    max_members: 4,
                },
                accesses: ProjectionAccessLimits {
                    max_nodes: 256,
                    max_depth: 32,
                },
            },
        }
    }

    #[test]
    fn complete_admission_borrows_each_original_plan_once_and_retains_all_ten_result_types() {
        for parallel in [false, true] {
            for kind in 0..5 {
                let (group, plan) = group_plan_fixture(parallel, kind);
                let before = serde_json::to_vec(&(&group, &plan)).unwrap();
                let calls = Cell::new(0);
                let result = validate_group_binding(
                    &group,
                    &plan,
                    policy(),
                    |_| true,
                    |borrowed| {
                        assert!(std::ptr::eq(borrowed, &plan));
                        calls.set(calls.get() + 1);
                        Ok::<_, ()>(())
                    },
                );
                let expected = if kind == 1 {
                    Type::UiFocus
                } else {
                    Type::Scalar(leselang_hir::computation::ScalarType::None)
                };
                assert_eq!(result, Ok(expected));
                assert_eq!(calls.get(), 1);
                assert_eq!(serde_json::to_vec(&(&group, &plan)).unwrap(), before);
            }
        }
    }

    #[test]
    fn complete_admission_preserves_structure_plan_encoding_type_and_scalar_refusal_order() {
        for defect in 0..6 {
            let (mut group, mut plan) = group_plan_fixture(false, 0);
            group.binding.body = Computation::Local {
                name: "missing".into(),
            };
            match defect {
                0 => {
                    group.binding.name = "x".repeat(65);
                    group.fuel_remaining = 101;
                }
                1 => group.fuel_remaining = 101,
                2 | 3 => {}
                4 => {
                    group.binding.body = Computation::Member {
                        group: "group".into(),
                        name: "a".into(),
                        operation: leselang_hir::host_call::HostOperation::UiFocus,
                    }
                }
                5 => {
                    group.binding.body = Computation::Literal {
                        value: ScalarValue::None,
                    }
                }
                _ => unreachable!(),
            }
            plan.result_binding = Some(Box::new(group.clone()));
            let calls = Cell::new(0);
            let result = validate_group_binding(
                &group,
                &plan,
                policy(),
                |_| true,
                |_| {
                    calls.set(calls.get() + 1);
                    if defect <= 2 {
                        Err("encoding refused")
                    } else {
                        Ok(())
                    }
                },
            );
            let expected = match defect {
                0 | 3 | 4 => Err(GroupBindingError::Binding),
                1 => Err(GroupBindingError::Plan),
                2 => Err(GroupBindingError::Encoding("encoding refused")),
                5 => Ok(Type::Scalar(leselang_hir::computation::ScalarType::None)),
                _ => unreachable!(),
            };
            assert_eq!(result, expected, "defect {defect}");
            assert_eq!(calls.get(), usize::from(defect >= 2));
        }
    }

    #[test]
    fn complete_admission_moves_non_debug_local_encoding_errors_without_exposing_payloads() {
        struct NativeError(Rc<Cell<usize>>);
        impl Drop for NativeError {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let (group, plan) = group_plan_fixture(false, 0);
        let drops = Rc::new(Cell::new(0));
        let payload = Box::new(NativeError(drops.clone()));
        let pointer = &*payload as *const NativeError;
        let result =
            validate_group_binding(&group, &plan, policy(), |_| true, move |_| Err(payload));
        let error = result.unwrap_err();
        assert_eq!(format!("{error}"), "group plan encoding rejected");
        assert_eq!(format!("{error:?}"), "group plan encoding rejected");
        assert!(std::error::Error::source(&error).is_none());
        let GroupBindingError::Encoding(original) = error else {
            panic!("lost encoding failure")
        };
        assert_eq!(&*original as *const NativeError, pointer);
        assert_eq!(drops.get(), 0);
        drop(original);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn complete_admission_keeps_name_hooks_before_plan_refusal_and_never_retries_unwind() {
        let (mut group, plan) = group_plan_fixture(false, 0);
        group
            .binding
            .groups
            .push(leselang_command::ProjectedGroupBinding {
                name: "prior".into(),
                group: leselang_command::ProjectedGroup {
                    members: vec![leselang_command::ProjectedBinding {
                        name: "member".into(),
                        result: leselang_command::ProjectedResult::capture(
                            leselang_hir::host_call::HostOperation::UiFocus,
                            &leselang_command::Value::UiFocus {
                                node_id: "node".into(),
                            },
                        )
                        .unwrap(),
                    }],
                },
            });
        let names = Cell::new(0);
        for accept_name in [false, true] {
            names.set(0);
            let result = validate_group_binding(
                &group,
                &plan,
                policy(),
                |_| {
                    names.set(names.get() + 1);
                    accept_name
                },
                |_| -> Result<(), ()> { panic!("encoding ran before rejected captured rows") },
            );
            assert_eq!(
                result,
                Err(if accept_name {
                    GroupBindingError::Plan
                } else {
                    GroupBindingError::Binding
                })
            );
            assert!(names.get() > 0);
        }
        let (group, plan) = group_plan_fixture(false, 0);
        let called = Cell::new(0);
        let before = serde_json::to_vec(&(&group, &plan)).unwrap();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validate_group_binding(
                &group,
                &plan,
                policy(),
                |_| true,
                |_| -> Result<(), ()> {
                    called.set(called.get() + 1);
                    panic!("native encoding failed");
                },
            )
        }));
        assert!(unwind.is_err());
        assert_eq!(called.get(), 1);
        assert_eq!(serde_json::to_vec(&(&group, &plan)).unwrap(), before);
    }

    #[test]
    fn complete_admission_drops_uncalled_encoder_captures_and_keeps_outer_shape_external() {
        struct Capture(Rc<Cell<usize>>);
        impl Drop for Capture {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let (mut group, mut plan) = group_plan_fixture(false, 0);
        group.binding.name = "x".repeat(65);
        let drops = Rc::new(Cell::new(0));
        let capture = Capture(drops.clone());
        let result = validate_group_binding(
            &group,
            &plan,
            policy(),
            |_| true,
            move |_| -> Result<(), ()> {
                drop(capture);
                panic!("must not encode structurally rejected input");
            },
        );
        assert_eq!(result, Err(GroupBindingError::Binding));
        assert_eq!(drops.get(), 1);
        group.binding.name = "group".into();
        plan.result_binding = None;
        assert!(plan.validate_shape(limits()).is_err());
        assert!(
            validate_group_binding(&group, &plan, policy(), |_| true, |_| Ok::<_, ()>(())).is_ok()
        );
    }
}

mod group_requests {
    use super::*;
    use leselang_command::{
        EffectOperation, EffectRequestView, GroupRequestContext, InvalidGroupRequests,
        validate_group_binding, validate_preflighted_group_requests,
    };
    use leselang_host_contract::{CapabilitySet, Principal, Revision};
    use leselang_runtime_core::{ContinuationFrame, ResourceBudget};
    use std::{cell::Cell, rc::Rc};

    struct LocalToken(String, Rc<Cell<usize>>);
    impl PartialEq for LocalToken {
        fn eq(&self, other: &Self) -> bool {
            self.0 == other.0
        }
    }
    impl Drop for LocalToken {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }
    struct Row {
        effect_id: String,
        frame: ContinuationFrame<LocalToken, Revision, Type, Effect, ResultBinding>,
        operation: EffectOperation,
        budget: ResourceBudget,
    }
    impl Row {
        fn view(&self) -> EffectRequestView<'_, LocalToken> {
            EffectRequestView {
                effect_id: &self.effect_id,
                required_capability: "ui.presentation",
                operation: &self.operation,
                continuation: &self.frame,
                budget: &self.budget,
            }
        }
    }
    fn rows(
        group: &GroupResultBinding,
        plan: &MergePlan,
        drops: &Rc<Cell<usize>>,
    ) -> Vec<(&'static str, Row)> {
        group
            .plan_branches()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, branch)| {
                let (_, operation) = leselang_command::lower_effect_operation(
                    &branch.effect,
                    Principal::new("fixture").unwrap(),
                    CapabilitySet::new(["ui.presentation"]),
                    None,
                    || panic!("GUI must not request a command identity"),
                )
                .unwrap();
                let frame = ContinuationFrame {
                    schema_version: group.continuation_schema(),
                    token: LocalToken(format!("continuation-{}", index + 2), Rc::clone(drops)),
                    program_counter: 1,
                    expected_revision: None,
                    result_type: branch.result_type,
                    pending_effect: branch.effect.clone(),
                    fuel_remaining: group.fuel_remaining + 1
                        - if plan.order.is_sequential() {
                            index as u64
                        } else {
                            0
                        },
                    deadline_ms: 1000,
                    deadline_at_ms: None,
                    max_output_items: 4,
                    result_binding: None,
                    group_result: Some(LocalToken("merge-1".into(), Rc::clone(drops))),
                };
                let budget = frame.resource_budget();
                (
                    if index == 0 { "a" } else { "b" },
                    Row {
                        effect_id: format!("effect-{}", index + 2),
                        frame,
                        operation,
                        budget,
                    },
                )
            })
            .collect()
    }
    fn context<'a>(
        owner: &'a LocalToken,
        plan: &'a MergePlan,
        group: &GroupResultBinding,
    ) -> GroupRequestContext<'a, LocalToken> {
        let result_type = validate_group_binding(
            group,
            plan,
            group_admission::policy(),
            |name| valid_merge_branch_name(name, 64),
            |_| Ok::<_, ()>(()),
        )
        .unwrap();
        GroupRequestContext {
            owner,
            plan,
            result_type,
            max_successor_names: 64,
            binding_limits: group_admission::policy().binding,
        }
    }

    mod complete_graph {
        use super::*;
        use leselang_command::{
            ContinuationAdmissionLimits, GroupBindingError, MergeGraphAdmissionError as Error,
            MergeGraphAdmissionHost, MergeGraphAdmissionLimits, MergePlanAdmissionError,
            MergePlanAdmissionLimits, validate_merge_graph, validate_merge_plan,
        };
        use leselang_runtime_core::{GroupGraphError, GroupGraphMismatch, ResourceBudgetLimits};
        use std::{
            cell::RefCell,
            panic::{AssertUnwindSafe, catch_unwind},
        };

        fn policy() -> MergeGraphAdmissionLimits {
            MergeGraphAdmissionLimits {
                plan: MergePlanAdmissionLimits {
                    shape: limits(),
                    group: group_admission::policy(),
                },
                frame: ContinuationAdmissionLimits {
                    binding: group_admission::policy().binding,
                    budget: ResourceBudgetLimits {
                        max_fuel: 101,
                        max_deadline_ms: 1000,
                        max_deadline_at_ms: 2000,
                        max_output_items: 4,
                    },
                    max_group_sequence: 101,
                },
            }
        }

        struct NativeError(Rc<Cell<usize>>);
        impl Drop for NativeError {
            fn drop(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        struct Host<'a> {
            plan: &'a MergePlan,
            events: RefCell<Vec<String>>,
            panic_at: Cell<usize>,
            encodings: Cell<usize>,
            fail_encoding: Cell<usize>,
            error_drops: Rc<Cell<usize>>,
        }
        impl<'a> Host<'a> {
            fn new(plan: &'a MergePlan) -> Self {
                Self {
                    plan,
                    events: RefCell::new(vec![]),
                    panic_at: Cell::new(0),
                    encodings: Cell::new(0),
                    fail_encoding: Cell::new(0),
                    error_drops: Rc::new(Cell::new(0)),
                }
            }
            fn event(&self, value: String) {
                self.events.borrow_mut().push(value);
                assert_ne!(
                    self.events.borrow().len(),
                    self.panic_at.get(),
                    "native unwind"
                );
            }
        }
        fn sequence(token: &str) -> Option<u64> {
            let text = token.strip_prefix("continuation-")?;
            let value = text.parse::<u64>().ok()?;
            ((1..=101).contains(&value) && value.to_string() == text).then_some(value)
        }
        impl MergeGraphAdmissionHost<LocalToken> for Host<'_> {
            type Error = NativeError;
            fn token_text<'a>(&self, token: &'a LocalToken) -> &'a str {
                self.event(format!("text:{}", token.0));
                &token.0
            }
            fn valid_token(&self, token: &LocalToken) -> bool {
                self.event(format!("token:{}", token.0));
                token.0 == "merge-1" || sequence(&token.0).is_some()
            }
            fn canonical_sequence(&self, token: &LocalToken) -> Option<u64> {
                self.event(format!("sequence:{}", token.0));
                sequence(&token.0)
            }
            fn valid_group_name(&self, name: &str) -> bool {
                self.event(format!("name:{name}"));
                valid_merge_branch_name(name, 64)
            }
            fn validate_plan_encoding(&self, plan: &MergePlan) -> Result<(), NativeError> {
                assert!(std::ptr::eq(plan, self.plan));
                self.event("encoding".into());
                self.encodings.set(self.encodings.get() + 1);
                if self.encodings.get() == self.fail_encoding.get() {
                    Err(NativeError(self.error_drops.clone()))
                } else {
                    Ok(())
                }
            }
        }
        fn check(
            owner: &LocalToken,
            rows: &[(&str, Row)],
            host: &Host<'_>,
        ) -> Result<(), Error<NativeError>> {
            let object: &dyn MergeGraphAdmissionHost<LocalToken, Error = NativeError> = host;
            validate_merge_graph(
                owner,
                host.plan,
                rows,
                policy(),
                |row| {
                    host.event(format!("view:{}", row.effect_id));
                    let view = row.view();
                    assert!(std::ptr::eq(view.continuation, &row.frame));
                    view
                },
                object,
            )
        }

        #[test]
        fn complete_graph_borrows_local_opaque_rows_and_rechecks_all_ten_bound_plan_forms() {
            for parallel in [false, true] {
                for kind in 0..5 {
                    let (group, plan) = group_plan_fixture(parallel, kind);
                    let before = serde_json::to_vec(&plan).unwrap();
                    let drops = Rc::new(Cell::new(0));
                    let owner = LocalToken("merge-1".into(), drops.clone());
                    let requests = rows(&group, &plan, &drops);
                    let host = Host::new(&plan);
                    let result = validate_merge_plan(
                        &plan,
                        policy().plan,
                        |_| true,
                        |plan| host.validate_plan_encoding(plan),
                    )
                    .unwrap();
                    assert_eq!(
                        result,
                        Some(if kind == 1 {
                            Type::UiFocus
                        } else {
                            Type::Scalar(leselang_hir::computation::ScalarType::None)
                        })
                    );
                    assert_eq!(host.encodings.get(), 1);
                    host.encodings.set(0);
                    check(&owner, &requests, &host).unwrap();
                    assert_eq!(host.encodings.get(), 2);
                    assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
                    assert_eq!(requests[0].1.frame.fuel_remaining, 101);
                    assert_eq!(
                        requests[1].1.frame.fuel_remaining,
                        if parallel { 101 } else { 100 }
                    );
                    assert_eq!(drops.get(), 0);
                    drop(requests);
                    assert_eq!(drops.get(), 4);
                    drop(owner);
                    assert_eq!(drops.get(), 5);
                }
            }
        }

        #[test]
        fn complete_graph_keeps_plan_owner_count_request_identity_and_correspondence_order() {
            for defect in 0..7 {
                let (group, mut plan) = group_plan_fixture(false, 0);
                let drops = Rc::new(Cell::new(0));
                let mut owner = LocalToken("merge-1".into(), drops.clone());
                let mut requests = rows(&group, &plan, &drops);
                if defect <= 2 {
                    requests.pop();
                }
                if defect <= 1 {
                    owner.0 = "bad".into();
                }
                if defect == 0 {
                    plan.branches.clear();
                }
                if defect == 3 {
                    requests[0].0 = "wrong";
                    requests[0].1.frame.program_counter = 0;
                }
                if defect == 4 {
                    requests[0].0 = "wrong";
                }
                if defect == 5 {
                    requests[1].1.effect_id = "effect-2".into();
                    requests[1].1.frame.token.0 = "continuation-2".into();
                }
                if defect == 6 {
                    requests[0].1.frame.fuel_remaining = 99;
                    requests[0].1.budget = requests[0].1.frame.resource_budget();
                }
                let host = Host::new(&plan);
                let error = check(&owner, &requests, &host).unwrap_err();
                assert!(
                    match defect {
                        0 => matches!(error, Error::Plan(MergePlanAdmissionError::Shape(_))),
                        1 => matches!(error, Error::Owner),
                        2 => matches!(error, Error::Graph(GroupGraphError::Count)),
                        3 => matches!(
                            error,
                            Error::Graph(GroupGraphError::Native { index: 0, .. })
                        ),
                        4 => matches!(
                            error,
                            Error::Graph(GroupGraphError::Member {
                                index: 0,
                                mismatch: GroupGraphMismatch::Name,
                            })
                        ),
                        5 => matches!(
                            error,
                            Error::Graph(GroupGraphError::Member {
                                index: 1,
                                mismatch: GroupGraphMismatch::Duplicate,
                            })
                        ),
                        6 => matches!(error, Error::Correspondence),
                        _ => unreachable!(),
                    },
                    "defect {defect}: {error:?}"
                );
                assert_eq!(
                    host.encodings.get(),
                    if defect == 0 {
                        0
                    } else if defect == 6 {
                        2
                    } else {
                        1
                    }
                );
                if defect <= 2 {
                    assert!(
                        !host
                            .events
                            .borrow()
                            .iter()
                            .any(|event| event.starts_with("view:"))
                    );
                }
            }
        }

        #[test]
        fn complete_graph_preserves_both_owned_encoding_failures_and_each_unwind_prefix() {
            let (group, plan) = group_plan_fixture(false, 1);
            let drops = Rc::new(Cell::new(0));
            let owner = LocalToken("merge-1".into(), drops.clone());
            let requests = rows(&group, &plan, &drops);
            for pass in [1, 2] {
                let host = Host::new(&plan);
                host.fail_encoding.set(pass);
                let error = check(&owner, &requests, &host).unwrap_err();
                assert!(std::error::Error::source(&error).is_none());
                assert_eq!(format!("{error:?}"), format!("{error}"));
                let original = match error {
                    Error::Plan(MergePlanAdmissionError::Binding(GroupBindingError::Encoding(
                        e,
                    ))) if pass == 1 => e,
                    Error::Binding(GroupBindingError::Encoding(e)) if pass == 2 => e,
                    _ => panic!("lost native encoding error"),
                };
                assert!(Rc::ptr_eq(&original.0, &host.error_drops));
                assert_eq!(host.error_drops.get(), 0);
                drop(original);
                assert_eq!(host.error_drops.get(), 1);
                assert_eq!(host.encodings.get(), pass);
            }
            let host = Host::new(&plan);
            check(&owner, &requests, &host).unwrap();
            let expected = host.events.into_inner();
            for cutoff in 1..=expected.len() {
                let host = Host::new(&plan);
                host.panic_at.set(cutoff);
                assert!(
                    catch_unwind(AssertUnwindSafe(|| check(&owner, &requests, &host))).is_err()
                );
                assert_eq!(*host.events.borrow(), expected[..cutoff]);
                assert_eq!(drops.get(), 0);
                assert_eq!(requests[0].1.frame.fuel_remaining, 101);
            }
        }

        #[test]
        fn complete_unbound_plan_skips_encoding_and_rejects_bound_members_without_authorizing_them()
        {
            let (group, mut plan) = group_plan_fixture(false, 0);
            plan.result_binding = None;
            plan.order = ExecutionOrder::Sequential;
            assert_eq!(
                validate_merge_plan(
                    &plan,
                    policy().plan,
                    |_| panic!("unbound group name hook"),
                    |_| -> Result<(), ()> { panic!("unbound encoding hook") }
                ),
                Ok(None)
            );
            for bad_shape in [false, true] {
                let mut candidate = plan.clone();
                if bad_shape {
                    candidate.branches.clear();
                }
                let dropped = Rc::new(Cell::new(0));
                let capture = NativeError(dropped.clone());
                let result = validate_merge_plan(
                    &candidate,
                    policy().plan,
                    |_| panic!("uncalled name hook"),
                    move |_| -> Result<(), ()> {
                        drop(capture);
                        panic!("uncalled encoding hook");
                    },
                );
                assert_eq!(result.is_err(), bad_shape);
                assert_eq!(dropped.get(), 1);
            }
            let drops = Rc::new(Cell::new(0));
            let owner = LocalToken("merge-1".into(), drops.clone());
            let mut requests = rows(&group, &plan, &drops);
            for (_, row) in &mut requests {
                row.frame.group_result = None;
                row.frame.schema_version = 1;
            }
            let host = Host::new(&plan);
            check(&owner, &requests, &host).unwrap();
            assert_eq!(host.encodings.get(), 0);
            requests[0].1.frame.group_result = Some(LocalToken("merge-1".into(), drops.clone()));
            requests[0].1.frame.schema_version = group.continuation_schema();
            assert!(matches!(
                check(&owner, &requests, &host),
                Err(Error::UnexpectedGroupOwner)
            ));
            assert_eq!(host.encodings.get(), 0);
        }
    }

    #[test]
    fn group_requests_borrow_opaque_local_rows_without_clones_or_identity_allocations() {
        for parallel in [false, true] {
            for kind in [0, 1] {
                let (group, plan) = group_plan_fixture(parallel, kind);
                let drops = Rc::new(Cell::new(0));
                let owner = LocalToken("merge-1".into(), Rc::clone(&drops));
                let requests = rows(&group, &plan, &drops);
                let calls = Cell::new(0);
                let sequences = Cell::new(0);
                assert_eq!(
                    validate_preflighted_group_requests(
                        &group,
                        context(&owner, &plan, &group),
                        &requests,
                        |row| {
                            calls.set(calls.get() + 1);
                            let borrowed = row.view();
                            assert!(std::ptr::eq(borrowed.continuation, &row.frame));
                            borrowed
                        },
                        |_| panic!("no successor token text is needed for a prefix"),
                        |token| {
                            sequences.set(sequences.get() + 1);
                            token.0.strip_prefix("continuation-").unwrap().parse().ok()
                        },
                        |_| panic!("no successor capture names are needed for a prefix")
                    ),
                    Ok(())
                );
                assert_eq!(calls.get(), if kind == 0 { 3 } else { 5 });
                assert_eq!(sequences.get(), if kind == 0 { 0 } else { 2 });
                assert_eq!(drops.get(), 0);
                drop(requests);
                assert_eq!(drops.get(), 4);
                drop(owner);
                assert_eq!(drops.get(), 5);
            }
        }
    }

    #[test]
    fn group_requests_skip_late_hooks_on_refusal_and_propagate_native_unwind_once() {
        let (group, plan) = group_plan_fixture(false, 1);
        let drops = Rc::new(Cell::new(0));
        let owner = LocalToken("merge-1".into(), Rc::clone(&drops));
        let mut requests = rows(&group, &plan, &drops);
        requests[0].0 = "wrong-name";
        let calls = Cell::new(0);
        let error = validate_preflighted_group_requests(
            &group,
            context(&owner, &plan, &group),
            &requests,
            |row| {
                calls.set(calls.get() + 1);
                row.view()
            },
            |_| panic!("token text hook ran after prefix refusal"),
            |_| panic!("sequence hook ran after prefix refusal"),
            |_| panic!("name hook ran after prefix refusal"),
        )
        .unwrap_err();
        assert_eq!(error, InvalidGroupRequests);
        assert_eq!(calls.get(), 2);
        assert_eq!(format!("{error}"), "invalid group request correspondence");
        assert!(std::error::Error::source(&error).is_none());
        requests[0].0 = "a";
        let calls = Cell::new(0);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            validate_preflighted_group_requests(
                &group,
                context(&owner, &plan, &group),
                &requests,
                Row::view,
                |_| panic!("no appended request"),
                |_| {
                    calls.set(calls.get() + 1);
                    panic!("trusted token policy unwind")
                },
                |_| true,
            )
        }));
        assert!(unwind.is_err());
        assert_eq!(calls.get(), 1);
        assert_eq!(drops.get(), 0);
        assert_eq!(requests[0].1.frame.fuel_remaining, 101);
        assert_eq!(requests[1].1.frame.fuel_remaining, 100);
    }

    #[test]
    fn shared_group_capture_type_lookup_keeps_cold_alternatives_and_pure_prefixes() {
        for parallel in [false, true] {
            for kind in 0..5 {
                for pure_prefix in [false, true] {
                    let (mut group, _) = group_plan_fixture(parallel, kind);
                    if pure_prefix {
                        group.binding.body = Computation::Bind {
                            name: "local".into(),
                            value: Box::new(Computation::Literal {
                                value: ScalarValue::None,
                            }),
                            body: Box::new(group.binding.body.clone()),
                        };
                    }
                    for position in 0..4 {
                        assert_eq!(
                            group.preflighted_capture_accepts_type_at(position, Type::UiFocus),
                            (position == 0 && kind >= 2) || (position == 1 && kind == 3)
                        );
                        assert!(
                            !group.preflighted_capture_accepts_type_at(
                                position,
                                Type::UiAssertVisible
                            )
                        );
                    }
                }
            }
        }
    }
}

fn group_plan_fixture(parallel: bool, kind: usize) -> (GroupResultBinding, MergePlan) {
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let (body, order) = match (parallel, kind) {
        (p, 0) => (
            pure.clone(),
            if p {
                ExecutionOrder::BoundParallel
            } else {
                ExecutionOrder::BoundSequential
            },
        ),
        (p, 1) => (
            host.clone(),
            if p {
                ExecutionOrder::ParallelGroupTail
            } else {
                ExecutionOrder::GroupTail
            },
        ),
        (p, 2) => (
            capture.clone(),
            if p {
                ExecutionOrder::ParallelGroupCapture
            } else {
                ExecutionOrder::GroupCapture
            },
        ),
        (p, 3) => (
            Computation::Bind {
                name: "first".into(),
                value: Box::new(host),
                body: Box::new(capture.clone()),
            },
            if p {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            },
        ),
        (p, 4) => (
            Computation::Choose {
                when: Box::new(Computation::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                then: Box::new(capture),
                otherwise: Box::new(pure),
            },
            if p {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            },
        ),
        _ => unreachable!(),
    };
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            result_type: Type::UiFocus,
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
        })
        .collect();
    let binding = GroupResultBinding {
        pending: if parallel {
            Effect::All { branches }
        } else {
            Effect::Sequence { steps: branches }
        },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body,
        }),
        fuel_remaining: 100,
        successor_sequence: (kind != 0).then_some(100),
        additional_successor_sequences: if kind == 3 { vec![101] } else { vec![] },
    };
    let plan = MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: Some(Box::new(binding.clone())),
    };
    (binding, plan)
}

fn limits() -> MergePlanShapeLimits {
    MergePlanShapeLimits {
        max_branches: 64,
        max_name_bytes: 64,
    }
}

fn group_limits() -> GroupPlanLimits {
    GroupPlanLimits {
        max_branches: 64,
        max_effect_sequence: 101,
        max_fuel: 100,
    }
}

#[test]
fn group_plan_correspondence_accepts_all_ten_forms_and_each_saved_successor_prefix() {
    let mut cases = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            let (binding, mut plan) = group_plan_fixture(parallel, kind);
            binding.binding.body.validate_structure().unwrap();
            let bound = binding.reserved_sequences().count();
            let members = binding.plan_branches().unwrap();
            let original_ptr = match &binding.pending {
                Effect::All { branches } | Effect::Sequence { steps: branches } => {
                    branches.as_ptr()
                }
                _ => unreachable!(),
            };
            assert_eq!(members.as_ptr(), original_ptr);
            for suffix in 0..=bound {
                plan.branches.truncate(members.len());
                for position in 0..suffix {
                    plan.branches
                        .push(binding.plan_successor_name_at(position, 64).unwrap());
                }
                let before = serde_json::to_vec(&(&binding, &plan)).unwrap();
                assert_eq!(
                    binding.validate_preflighted_plan(&plan, group_limits()),
                    Ok(())
                );
                assert_eq!(serde_json::to_vec(&(&binding, &plan)).unwrap(), before);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 20);
}

#[test]
fn group_plan_limits_are_explicit_inclusive_and_never_repair_saved_resources() {
    let (binding, plan) = group_plan_fixture(false, 3);
    let exact = GroupPlanLimits {
        max_branches: 4,
        max_effect_sequence: 101,
        max_fuel: 100,
    };
    assert_eq!(binding.validate_preflighted_plan(&plan, exact), Ok(()));
    for policy in [
        GroupPlanLimits {
            max_branches: 3,
            ..exact
        },
        GroupPlanLimits {
            max_effect_sequence: 100,
            ..exact
        },
        GroupPlanLimits {
            max_fuel: 99,
            ..exact
        },
        GroupPlanLimits {
            max_branches: 0,
            max_effect_sequence: 0,
            max_fuel: 0,
        },
    ] {
        assert_eq!(
            binding.validate_preflighted_plan(&plan, policy),
            Err(InvalidGroupPlan)
        );
    }
    let (mut pure, pure_plan) = group_plan_fixture(false, 0);
    pure.fuel_remaining = 0;
    assert_eq!(
        pure.validate_preflighted_plan(
            &pure_plan,
            GroupPlanLimits {
                max_branches: 2,
                max_effect_sequence: 0,
                max_fuel: 0,
            }
        ),
        Ok(())
    );
    let before = serde_json::to_vec(&binding).unwrap();
    assert_eq!(
        binding.validate_preflighted_plan(
            &plan,
            GroupPlanLimits {
                max_branches: usize::MAX,
                max_effect_sequence: u64::MAX,
                max_fuel: u64::MAX,
            }
        ),
        Ok(())
    );
    assert_eq!(serde_json::to_vec(&binding).unwrap(), before);
}

#[test]
fn group_plan_names_skip_original_collisions_without_extending_the_search_window() {
    let (mut binding, mut plan) = group_plan_fixture(false, 3);
    if let Effect::Sequence { steps } = &mut binding.pending {
        steps[0].name = "successor".into();
        steps[1].name = "successor_1".into();
    }
    plan.branches = ["successor", "successor_1", "successor_2", "successor_3"]
        .map(String::from)
        .into();
    assert_eq!(
        binding.plan_successor_name_at(0, 4),
        Ok("successor_2".into())
    );
    assert_eq!(
        binding.plan_successor_name_at(1, 4),
        Ok("successor_3".into())
    );
    for (position, ceiling) in [(0, 0), (0, 2), (1, 3), (usize::MAX, 4)] {
        assert_eq!(
            binding.plan_successor_name_at(position, ceiling),
            Err(InvalidGroupPlan)
        );
    }
    assert_eq!(
        binding.validate_preflighted_plan(&plan, group_limits()),
        Ok(())
    );
    plan.branches[2] = "successor_02".into();
    assert_eq!(
        binding.validate_preflighted_plan(&plan, group_limits()),
        Err(InvalidGroupPlan)
    );
}

#[test]
fn group_plan_rejects_inconsistent_reservations_signatures_and_pending_shapes() {
    let (binding, plan) = group_plan_fixture(false, 3);
    for (first, rest) in [
        (None, vec![100, 101]),
        (Some(0), vec![101]),
        (Some(100), vec![]),
        (Some(100), vec![100]),
        (Some(100), vec![99]),
        (Some(100), vec![102]),
        (Some(100), vec![101, 102]),
    ] {
        let mut broken = binding.clone();
        broken.successor_sequence = first;
        broken.additional_successor_sequences = rest;
        assert_eq!(
            broken.validate_preflighted_plan(&plan, group_limits()),
            Err(InvalidGroupPlan)
        );
    }
    for defect in 0..4 {
        let mut broken = binding.clone();
        let Effect::Sequence { steps } = &mut broken.pending else {
            unreachable!()
        };
        match defect {
            0 => steps[0].result_type = Type::UiAssertVisible,
            1 => steps[0].effect = Effect::All { branches: vec![] },
            2 => steps[0].name = "other".into(),
            3 => steps.reverse(),
            _ => unreachable!(),
        }
        assert_eq!(
            broken.validate_preflighted_plan(&plan, group_limits()),
            Err(InvalidGroupPlan)
        );
    }
    let broken = unchecked_binding();
    assert_eq!(broken.plan_branches(), Err(InvalidGroupPlan));
    assert_eq!(broken.plan_successor_name_at(0, 64), Err(InvalidGroupPlan));
    assert_eq!(
        broken.validate_preflighted_plan(&plan, group_limits()),
        Err(InvalidGroupPlan)
    );
}

#[test]
fn group_plan_policy_is_not_outer_plan_type_or_recovery_admission() {
    let (mut binding, mut plan) = group_plan_fixture(false, 0);
    binding.binding.body = Computation::Local {
        name: "unresolved".into(),
    };
    binding.binding.body.validate_structure().unwrap();
    plan.result_binding = None;
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::GroupBinding)
    );
    assert_eq!(
        binding.validate_preflighted_plan(&plan, group_limits()),
        Ok(())
    );
    plan.result_binding = Some(unchecked_binding());
    // The chosen binding and original plan must be associated by the caller.
    assert_eq!(
        binding.validate_preflighted_plan(&plan, group_limits()),
        Ok(())
    );
    let error = InvalidGroupPlan;
    assert_eq!(error.to_string(), "invalid group plan correspondence");
    assert_eq!(format!("{error:?}"), "InvalidGroupPlan");
    assert!(std::error::Error::source(&error).is_none());
}

fn unchecked_binding() -> Box<GroupResultBinding> {
    Box::new(GroupResultBinding {
        pending: Effect::UiFocus {
            node_id: "not-a-group".into(),
        },
        binding: Box::new(ResultBinding {
            name: "invalid name".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body: Computation::Literal {
                value: ScalarValue::None,
            },
        }),
        fuel_remaining: u64::MAX,
        successor_sequence: Some(0),
        additional_successor_sequences: vec![0],
    })
}

#[test]
fn execution_order_keeps_fourteen_wire_tags_flags_and_default_omission() {
    use ExecutionOrder::*;
    for (order, wire, parallel, successor, capture) in [
        (Parallel, "parallel", true, false, false),
        (Sequential, "sequential", false, false, false),
        (ResultChain, "result_chain", false, false, false),
        (Dataflow, "dataflow", false, false, false),
        (BoundParallel, "bound_parallel", true, false, false),
        (BoundSequential, "bound_sequential", false, false, false),
        (GroupTail, "group_tail", false, true, false),
        (GroupCapture, "group_capture", false, true, true),
        (ParallelGroupTail, "parallel_group_tail", true, true, false),
        (
            ParallelGroupCapture,
            "parallel_group_capture",
            true,
            true,
            true,
        ),
        (GroupDataflow, "group_dataflow", false, true, true),
        (
            ParallelGroupDataflow,
            "parallel_group_dataflow",
            true,
            true,
            true,
        ),
        (GroupConditional, "group_conditional", false, true, true),
        (
            ParallelGroupConditional,
            "parallel_group_conditional",
            true,
            true,
            true,
        ),
    ] {
        assert_eq!(
            serde_json::to_string(&order).unwrap(),
            format!("\"{wire}\"")
        );
        assert_eq!(
            serde_json::from_value::<ExecutionOrder>(wire.into()).unwrap(),
            order
        );
        assert_eq!(order.is_parallel(), parallel);
        assert_eq!(order.is_sequential(), !parallel);
        assert_eq!(order.has_group_successor(), successor);
        assert_eq!(order.captures_group_successor(), capture);
    }
    assert_eq!(ExecutionOrder::default(), Parallel);
    assert_eq!((MAX_MERGE_BRANCHES, MAX_MERGE_BRANCH_NAME_BYTES), (64, 64));
    let plan = MergePlan::new(["a", "b"]).unwrap();
    assert_eq!(
        serde_json::to_string(&plan).unwrap(),
        r#"{"branches":["a","b"]}"#
    );
    assert_eq!(
        serde_json::from_str::<MergePlan>(
            r#"{"branches":["a","b"],"order":"parallel","result_binding":null}"#
        )
        .unwrap(),
        plan
    );
}

#[test]
fn plan_shape_keeps_binding_before_successor_before_count_and_name_faults() {
    let mut plan = MergePlan {
        branches: vec!["invalid name".into()],
        order: ExecutionOrder::Dataflow,
        result_binding: Some(unchecked_binding()),
    };
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::GroupBinding)
    );
    plan.result_binding = None;
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Successor)
    );
    plan.branches = vec!["step_1".into()];
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Branches)
    );
    plan.branches.push("step_2".into());
    assert_eq!(plan.validate_shape(limits()), Ok(()));
    plan.order = ExecutionOrder::ResultChain;
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Successor)
    );
    plan.branches = vec!["result".into(), "successor".into()];
    assert_eq!(plan.validate_shape(limits()), Ok(()));
    plan.order = ExecutionOrder::Parallel;
    plan.branches = vec!["same".into(), "same".into()];
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Branches)
    );
}

#[test]
fn shape_limits_are_explicit_and_do_not_admit_or_traverse_saved_bindings() {
    let mut plan = MergePlan {
        branches: vec!["a".into()],
        order: ExecutionOrder::BoundSequential,
        result_binding: Some(unchecked_binding()),
    };
    let before = serde_json::to_vec(&plan).unwrap();
    assert_eq!(
        plan.validate_shape(MergePlanShapeLimits {
            max_branches: 1,
            max_name_bytes: 1
        }),
        Ok(())
    );
    for policy in [
        MergePlanShapeLimits {
            max_branches: 0,
            max_name_bytes: 1,
        },
        MergePlanShapeLimits {
            max_branches: 1,
            max_name_bytes: 0,
        },
    ] {
        assert_eq!(
            plan.validate_shape(policy),
            Err(MergePlanShapeError::Branches)
        );
    }
    assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
    plan.order = ExecutionOrder::BoundParallel;
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Branches)
    );
    plan.branches = (0..65).map(|i| format!("member_{i}")).collect();
    assert_eq!(
        plan.validate_shape(limits()),
        Err(MergePlanShapeError::Branches)
    );
    assert_eq!(
        plan.validate_shape(MergePlanShapeLimits {
            max_branches: 65,
            max_name_bytes: 64
        }),
        Ok(())
    );
    // Independent shape policy cannot widen the released constructor's bounds.
    assert!(MergePlan::new(plan.branches).is_err());
}

#[test]
fn branch_spelling_and_closed_codec_never_normalize_or_expose_private_names() {
    for name in ["", "a b", "a:b", "a.b", "\u{e9}", "a\n"] {
        assert!(!valid_merge_branch_name(name, usize::MAX));
    }
    assert!(valid_merge_branch_name("a-A_0", 5));
    assert!(!valid_merge_branch_name("a-A_0", 4));
    assert!(!valid_merge_branch_name("a", 0));
    for wire in [
        r#"{"branches":[],"extra":true}"#,
        r#"{"branches":[],"order":"other"}"#,
        r#"{"branches":[],"branches":[]}"#,
        r#"{"order":"parallel"}"#,
    ] {
        assert!(serde_json::from_str::<MergePlan>(wire).is_err());
    }
    let mut wire = serde_json::to_value(MergePlan {
        branches: vec!["a".into()],
        order: ExecutionOrder::BoundSequential,
        result_binding: Some(unchecked_binding()),
    })
    .unwrap();
    wire["result_binding"]["unknown"] = true.into();
    assert!(serde_json::from_value::<MergePlan>(wire).is_err());
    for error in [
        MergePlanShapeError::GroupBinding,
        MergePlanShapeError::Successor,
        MergePlanShapeError::Branches,
    ] {
        assert!(!error.to_string().contains("invalid name"));
        assert!(std::error::Error::source(&error).is_none());
    }
}

#[test]
fn constructor_preserves_original_buffers_and_converts_all_input_before_refusal() {
    use std::{cell::Cell, rc::Rc};
    struct Input {
        text: Option<String>,
        converted: Rc<Cell<usize>>,
        dropped: Rc<Cell<usize>>,
        panic: bool,
    }
    impl Drop for Input {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    impl From<Input> for String {
        fn from(mut input: Input) -> String {
            input.converted.set(input.converted.get() + 1);
            assert!(!input.panic, "conversion failed");
            input.text.take().unwrap()
        }
    }
    let a = String::from("a");
    let b = String::from("b");
    let pointers = [a.as_ptr(), b.as_ptr()];
    let plan = MergePlan::new([a, b]).unwrap();
    assert_eq!(
        [plan.branches[0].as_ptr(), plan.branches[1].as_ptr()],
        pointers
    );
    for panic in [false, true] {
        let converted = Rc::new(Cell::new(0));
        let dropped = Rc::new(Cell::new(0));
        let values = ["same", "same", "tail"]
            .into_iter()
            .enumerate()
            .map(|(i, value)| Input {
                text: Some(value.into()),
                converted: converted.clone(),
                dropped: dropped.clone(),
                panic: panic && i == 1,
            })
            .collect::<Vec<_>>();
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| MergePlan::new(values)));
        if panic {
            assert!(result.is_err());
            assert_eq!(converted.get(), 2);
        } else {
            let fault = result.unwrap().unwrap_err();
            assert_eq!(fault.code, "LSV2401");
            assert_eq!(
                fault.message,
                "structured merge plan is invalid or exceeds runtime bounds"
            );
            assert_eq!(converted.get(), 3);
        }
        assert_eq!(dropped.get(), 3);
    }
}
