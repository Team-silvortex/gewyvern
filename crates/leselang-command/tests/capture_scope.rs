use std::{
    cell::{Cell, RefCell},
    marker::PhantomData,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_command::{
    EffectPreparationError, PreflightedResultEnvironment, ProjectedBinding, ProjectedGroup,
    ProjectedGroupBinding, ProjectedResult, ResultBinding, ResultCaptureHost, ResultCaptureLimits,
    ResultExecutionHost, ResultExecutionLimits, ResultView, ResultViewError, StructuredField,
    Value, resume_preflighted_result_binding_in_scope, snapshot_preflighted_result_scope,
};
use leselang_hir_core::{
    call_evaluation::CallEvaluationError,
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationFault, EffectEvaluationLimits,
        EffectEvaluationOutcome, evaluate_effects_in_scope,
    },
    pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault,
        PureEvaluationLimits, PureValue,
    },
};
use leselang_runtime_core::{
    CalculationFailure, CaptureScopeError, CaptureSnapshotError, CaptureSnapshotLimits, Fuel,
    GroupCaptureLimits, ScalarBinding, ScalarValue, ScopeFrame,
};
use leserpent_hir::{
    Effect, HirBranch, Type, computation::Computation, host_call::HostOperation,
    result_field::ResultField,
};

const LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 64,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 4,
    max_branches: 4,
};
const CAPTURE: ResultCaptureLimits = ResultCaptureLimits {
    snapshot: CaptureSnapshotLimits {
        max_bindings: 8,
        max_name_bytes: 16,
    },
    groups: GroupCaptureLimits {
        min_members: 1,
        max_members: 4,
    },
};

// Native payloads deliberately have no Clone, Debug, Display or serde contract.
struct Native {
    kind: &'static str,
    payload: Box<u64>,
    dropped: Option<Rc<Cell<bool>>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        if let Some(flag) = &self.dropped {
            flag.set(true);
        }
    }
}
fn native(kind: &'static str) -> Native {
    Native {
        kind,
        payload: Box::new(91),
        dropped: None,
    }
}

struct Host {
    events: RefCell<Vec<&'static str>>,
    fail: Option<&'static str>,
    unwind: bool,
    saved: bool,
    original: RefCell<Option<Native>>,
}
impl Host {
    fn new() -> Self {
        Self {
            events: RefCell::new(vec![]),
            fail: None,
            unwind: false,
            saved: true,
            original: RefCell::new(Some(native("private-native-error"))),
        }
    }
    fn event(&self, name: &'static str) -> Result<(), Native> {
        self.events.borrow_mut().push(name);
        if self.fail == Some(name) {
            assert!(!self.unwind, "native hook unwind");
            return Err(self.original.borrow_mut().take().unwrap());
        }
        Ok(())
    }
    fn debit(&self, name: &'static str, fuel: &mut Fuel) -> Result<(), Native> {
        self.event(name)?;
        fuel.charge(1).map_err(|_| native("fuel"))
    }
}
impl ResultCaptureHost for Host {
    type Error = Native;
    fn charge_scalar(&self, _: &ScalarValue, fuel: &mut Fuel) -> Result<(), Native> {
        self.debit("scalar", fuel)
    }
    fn charge_result(&self, _: &ProjectedResult, fuel: &mut Fuel) -> Result<(), Native> {
        self.debit("result", fuel)
    }
    fn charge_group(&self, _: &ProjectedGroup, fuel: &mut Fuel) -> Result<(), Native> {
        self.debit("group", fuel)
    }
    fn projection_error(&self, _: ResultViewError) -> Native {
        self.events.borrow_mut().push("projection");
        native("projection")
    }
    fn allow_saved_projection(&self) -> Result<(), Native> {
        self.event("allow")?;
        if self.saved {
            Ok(())
        } else {
            Err(native("saved-refused"))
        }
    }
}

fn pure_failure(error: PureEvaluationFailure<Native>) -> CalculationFailure<Native> {
    match error {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(PureEvaluationFault::Native(error)) => error.into(),
        CalculationFailure::External(PureEvaluationFault::FuelExhausted) => native("fuel").into(),
        _ => native("contract").into(),
    }
}

impl ResultExecutionHost for Host {
    fn preflight_effect(&self, _: &Computation) -> Result<(), Native> {
        self.event("preflight")
    }
    fn before_prepare_effect(&self, _: &Computation) -> Result<(), Native> {
        self.event("prepare")
    }
    fn prepared_effect(&self, effect: &Effect) -> Result<(), Native> {
        self.event(
            if matches!(effect, Effect::Sequence { .. } | Effect::All { .. }) {
                "group-prepared"
            } else {
                "prepared"
            },
        )
    }
    fn before_projection(&self, _: ResultField) -> Result<(), Native> {
        self.event("field-policy")
    }
    fn valid_group_name(&self, name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 16
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    }
    fn preparation_failure(
        &self,
        error: EffectPreparationError<Native>,
    ) -> CalculationFailure<Native> {
        match error {
            EffectPreparationError::Evaluation(error) => match error {
                CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
                CalculationFailure::External(EffectEvaluationFault::Native(error)) => error.into(),
                CalculationFailure::External(EffectEvaluationFault::Pure(error)) => {
                    pure_failure(CalculationFailure::External(error))
                }
                _ => native("contract").into(),
            },
            EffectPreparationError::Call { failure, .. } => match failure {
                CallEvaluationError::Evaluation { failure, .. } => pure_failure(failure),
                CallEvaluationError::Scope(error) => {
                    pure_failure(CalculationFailure::External(error))
                }
                CallEvaluationError::FuelExhausted => native("fuel").into(),
                _ => native("contract").into(),
            },
            _ => native("contract").into(),
        }
    }
    fn capture_failure(&self, error: CaptureSnapshotError<Native>) -> Native {
        match error {
            CaptureSnapshotError::Native { error, .. } => error,
            _ => native("capture-contract"),
        }
    }
}

fn execution_environment<'value, 'host>(
    host: &'host Host,
) -> PreflightedResultEnvironment<'value, 'host, dyn ResultExecutionHost<Error = Native> + 'host> {
    PreflightedResultEnvironment::new(
        host,
        ResultExecutionLimits {
            effects: LIMITS,
            captures: CAPTURE,
        },
    )
}

fn computed_group(mode: &str) -> Computation {
    let source = format!(
        "fn main() = bind(prefix: \"p-\", body: {mode}(a: ui.focus(node_id: concat(left: prefix, right: \"a\")), b: ui.focus(node_id: concat(left: prefix, right: \"b\"))))"
    );
    let program = leserpent_hir::lower(&leselang_syntax::parse(&source)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    expression.validate_in_scope(&[]).unwrap();
    *expression
}

#[test]
fn shared_environment_borrows_one_unsized_host_for_recursive_group_preparation() {
    for mode in ["seq", "all"] {
        let expression = computed_group(mode);
        let before = serde_json::to_vec(&expression).unwrap();
        let host = Host::new();
        let environment = execution_environment(&host);
        assert_eq!(format!("{environment:?}"), "PreflightedResultEnvironment");
        assert!(host.events.borrow().is_empty());
        let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
        let mut scope = ScopeFrame::new(&mut rows);
        let mut fuel = Fuel::new(1000);
        let outcome = evaluate_effects_in_scope(
            &expression,
            &mut scope,
            &environment,
            &mut fuel,
            environment.limits().effects,
        )
        .unwrap();
        let EffectEvaluationOutcome::Request(request) = outcome else {
            panic!()
        };
        let (Effect::Sequence { steps } | Effect::All { branches: steps }) = request.as_ref()
        else {
            panic!()
        };
        assert_eq!(
            steps
                .iter()
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            steps[0].effect,
            Effect::UiFocus {
                node_id: "p-a".into()
            }
        );
        assert_eq!(
            steps[1].effect,
            Effect::UiFocus {
                node_id: "p-b".into()
            }
        );
        let events = host.events.borrow();
        let first_prepare = events.iter().position(|event| *event == "prepare").unwrap();
        assert!(first_prepare >= 3);
        assert!(
            events[..first_prepare]
                .iter()
                .all(|event| *event == "preflight")
        );
        assert_eq!(
            events.iter().filter(|event| **event == "prepared").count(),
            2
        );
        assert_eq!(events.last(), Some(&"group-prepared"));
        assert_eq!(scope.len(), 1);
        assert_eq!(scope.bindings()[0].0, "parent");
        assert_eq!(serde_json::to_vec(&expression).unwrap(), before);
        let spent = 1000 - fuel.remaining();
        for grant in 0..=spent {
            let host = Host::new();
            let environment = execution_environment(&host);
            let mut fuel = Fuel::new(grant);
            let outcome = evaluate_effects_in_scope(
                &expression,
                &mut scope,
                &environment,
                &mut fuel,
                environment.limits().effects,
            );
            if grant == spent {
                assert!(matches!(outcome, Ok(EffectEvaluationOutcome::Request(_))));
                assert_eq!(fuel.remaining(), 0);
            } else {
                assert!(matches!(
                    outcome,
                    Err(CalculationFailure::External(
                        EffectEvaluationFault::Pure(PureEvaluationFault::FuelExhausted)
                            | EffectEvaluationFault::Native(Native { kind: "fuel", .. })
                    ))
                ));
            }
            assert_eq!(scope.len(), 1);
        }
    }
}

#[test]
fn shared_environment_refusals_preserve_native_ownership_and_completed_work() {
    for mode in ["seq", "all"] {
        let expression = computed_group(mode);
        for fail in ["preflight", "prepare", "prepared", "group-prepared"] {
            let host = Host {
                fail: Some(fail),
                ..Host::new()
            };
            let pointer =
                std::ptr::from_ref(host.original.borrow().as_ref().unwrap().payload.as_ref());
            let environment = execution_environment(&host);
            let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
            let mut scope = ScopeFrame::new(&mut rows);
            let mut fuel = Fuel::new(1000);
            let error = evaluate_effects_in_scope(
                &expression,
                &mut scope,
                &environment,
                &mut fuel,
                environment.limits().effects,
            )
            .unwrap_err();
            let CalculationFailure::External(EffectEvaluationFault::Native(error)) = error else {
                panic!()
            };
            assert_eq!(std::ptr::from_ref(error.payload.as_ref()), pointer);
            assert_eq!(scope.len(), 1);
            let events = host.events.borrow();
            assert_eq!(events.last(), Some(&fail));
            if fail == "preflight" {
                assert_eq!(events.as_slice(), ["preflight"]);
                assert_eq!(fuel.remaining(), 1000);
            } else {
                assert!(fuel.remaining() < 1000);
            }
            assert_eq!(
                events.iter().filter(|event| **event == "prepared").count(),
                match fail {
                    "prepared" => 1,
                    "group-prepared" => 2,
                    _ => 0,
                }
            );
        }
    }
}

#[test]
fn shared_environment_projection_policy_is_not_scalar_recovery() {
    let mut saved = binding();
    saved.body = Computation::Recover {
        value: Box::new(field("saved")),
        fallback: Box::new(Computation::Literal {
            value: ScalarValue::String("fallback".into()),
        }),
    };
    let before = serde_json::to_vec(&saved).unwrap();
    let host = Host {
        fail: Some("field-policy"),
        ..Host::new()
    };
    let pointer = std::ptr::from_ref(host.original.borrow().as_ref().unwrap().payload.as_ref());
    let environment = execution_environment(&host);
    let value = Value::UiFocus {
        node_id: "pending".into(),
    };
    let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
    let mut scope = ScopeFrame::new(&mut rows);
    let error = resume_preflighted_result_binding_in_scope(
        &saved,
        &mut scope,
        &environment,
        &mut Fuel::new(100),
        environment.limits().effects,
        pending(&value),
        &host,
    )
    .unwrap_err();
    let CaptureScopeError::Native {
        error:
            CalculationFailure::External(EffectEvaluationFault::Pure(PureEvaluationFault::Native(
                error,
            ))),
        ..
    } = error
    else {
        panic!()
    };
    assert_eq!(std::ptr::from_ref(error.payload.as_ref()), pointer);
    assert_eq!(host.events.borrow().last(), Some(&"field-policy"));
    assert_eq!(scope.len(), 1);
    assert_eq!(serde_json::to_vec(&saved).unwrap(), before);
}

#[test]
fn shared_environment_unwind_keeps_the_original_meter_and_cleans_local_prefix() {
    let expression = computed_group("seq");
    let host = Host {
        fail: Some("prepared"),
        unwind: true,
        ..Host::new()
    };
    let environment = execution_environment(&host);
    let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
    let mut scope = ScopeFrame::new(&mut rows);
    let mut fuel = Fuel::new(1000);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = evaluate_effects_in_scope(
                &expression,
                &mut scope,
                &environment,
                &mut fuel,
                environment.limits().effects,
            );
        }))
        .is_err()
    );
    assert!(fuel.remaining() < 1000);
    assert_eq!(scope.len(), 1);
    assert_eq!(scope.bindings()[0].0, "parent");
    assert_eq!(
        host.events
            .borrow()
            .iter()
            .filter(|event| **event == "prepared")
            .count(),
        1
    );
    assert_eq!(host.events.borrow().last(), Some(&"prepared"));
}

struct Environment<'value, 'host> {
    host: &'host Host,
    seen: RefCell<Vec<usize>>,
    marker: PhantomData<ResultView<'value>>,
}
impl<'a> PureEvaluationEnvironment<ResultField, HostOperation> for Environment<'a, '_> {
    type Result = ResultView<'a>;
    type Error = Native;
    fn field(&self, value: &Self::Result, field: &ResultField) -> Result<ScalarValue, Native> {
        self.host.event("field")?;
        if let ResultView::Projected(value) = value {
            self.seen
                .borrow_mut()
                .push(std::ptr::from_ref(*value).addr());
        }
        value
            .project_scalar(*field)
            .map_err(|error| self.host.projection_error(error))
    }
    fn member(
        &self,
        value: &Self::Result,
        name: &str,
        operation: &HostOperation,
    ) -> Result<Self::Result, Native> {
        value
            .select_member(name, *operation)
            .map_err(|error| self.host.projection_error(error))
    }
}
impl<'a> EffectEvaluationEnvironment<'a, ResultField, HostOperation, Effect, Type>
    for Environment<'a, '_>
{
    type Request = ();
    type Capture = ();
    fn preflight_effect(&self, _: &'a Computation) -> Result<(), Native> {
        Err(native("unexpected-effect"))
    }
    fn prepare_effect(
        &self,
        _: &'a Computation,
        _: &mut ScopeFrame<'_, 'a, PureValue<Self::Result>>,
        _: &mut Fuel,
    ) -> Result<(), CalculationFailure<Native>> {
        Err(native("unexpected-effect").into())
    }
    fn capture(
        &self,
        _: &'a str,
        _: &'a Computation,
        _: &ScopeFrame<'_, 'a, PureValue<Self::Result>>,
        _: &mut Fuel,
    ) -> Result<(), Native> {
        Err(native("unexpected-capture"))
    }
}

fn projected(node: &str, version: u32) -> ProjectedResult {
    ProjectedResult::capture_version(
        HostOperation::UiFocus,
        &Value::UiFocus {
            node_id: node.into(),
        },
        version,
    )
    .unwrap()
}
fn field(name: &str) -> Computation {
    Computation::Field {
        value: Box::new(Computation::Local { name: name.into() }),
        field: ResultField::NodeId,
    }
}
fn binding() -> ResultBinding {
    ResultBinding {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "n".into(),
            value: ScalarValue::Integer(3),
        }],
        results: vec![ProjectedBinding {
            name: "saved".into(),
            result: projected("a", 1),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "batch".into(),
            group: ProjectedGroup {
                members: vec![ProjectedBinding {
                    name: "member".into(),
                    result: projected("b", 5),
                }],
            },
        }],
        body: field("saved"),
    }
}
fn pending(value: &Value) -> ResultView<'_> {
    ResultView::Raw {
        value,
        operation: HostOperation::UiFocus,
    }
}

#[test]
fn snapshot_joins_mixed_views_with_explicit_accounting_and_closed_saved_bytes() {
    let saved = binding();
    let raw = Value::UiFocus {
        node_id: "raw".into(),
    };
    let grouped = Value::Structured {
        fields: vec![StructuredField {
            name: "member".into(),
            value: Box::new(raw.clone()),
        }],
    };
    let branches = [HirBranch {
        name: "member".into(),
        effect: Effect::UiFocus {
            node_id: "raw".into(),
        },
        result_type: Type::UiFocus,
    }];
    let mut rows: Vec<(&'static str, PureValue<ResultView<'_>>)> = vec![
        ("raw", PureValue::Result(pending(&raw))),
        ("n", PureValue::Scalar(ScalarValue::Integer(3))),
        (
            "group",
            PureValue::Result(ResultView::Group {
                value: &grouped,
                branches: &branches,
            }),
        ),
        (
            "saved",
            PureValue::Result(ResultView::Projected(&saved.results[0].result)),
        ),
        (
            "batch",
            PureValue::Result(ResultView::ProjectedGroup(&saved.groups[0].group)),
        ),
    ];
    let scope = ScopeFrame::new(&mut rows);
    let host = Host::new();
    let mut fuel = Fuel::new(5);
    let mut names = vec![];
    let result = snapshot_preflighted_result_scope(
        "pending",
        &saved.body,
        &scope,
        &mut fuel,
        CAPTURE,
        &host,
        |name| {
            names.push(name.to_owned());
            true
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 0);
    assert_eq!(
        *host.events.borrow(),
        ["result", "scalar", "group", "result", "group"]
    );
    assert_eq!(names, ["member"]); // Saved groups bypass raw capture policy.
    assert_eq!(result.locals, saved.locals);
    assert_eq!(result.results[1], saved.results[0]);
    assert_eq!(result.groups[1], saved.groups[0]);
    assert_eq!(result.body, saved.body);
    assert_eq!(scope.len(), 5);
    assert_eq!(
        serde_json::to_vec(&result.results[1]).unwrap(),
        serde_json::to_vec(&saved.results[0]).unwrap()
    );
}

#[test]
fn snapshot_preflight_and_projection_refusal_preserve_native_charge_order() {
    let body = field("pending");
    let bad = Value::Scalar {
        value: ScalarValue::None,
    };
    let mut rows = vec![
        ("n", PureValue::Scalar(ScalarValue::None)),
        ("raw", PureValue::Result(pending(&bad))),
    ];
    let scope = ScopeFrame::new(&mut rows);
    for limits in [
        ResultCaptureLimits {
            snapshot: CaptureSnapshotLimits {
                max_bindings: 1,
                ..CAPTURE.snapshot
            },
            ..CAPTURE
        },
        ResultCaptureLimits {
            snapshot: CaptureSnapshotLimits {
                max_name_bytes: 0,
                ..CAPTURE.snapshot
            },
            ..CAPTURE
        },
    ] {
        let host = Host::new();
        let mut fuel = Fuel::new(100);
        assert!(
            snapshot_preflighted_result_scope(
                "pending",
                &body,
                &scope,
                &mut fuel,
                limits,
                &host,
                |_| panic!("name hook before bounds")
            )
            .is_err()
        );
        assert_eq!(fuel.remaining(), 100);
        assert!(host.events.borrow().is_empty());
    }
    let host = Host::new();
    let mut fuel = Fuel::new(100);
    let error = snapshot_preflighted_result_scope(
        "pending",
        &body,
        &scope,
        &mut fuel,
        CAPTURE,
        &host,
        |_| true,
    )
    .unwrap_err();
    assert_eq!(fuel.remaining(), 99);
    assert_eq!(*host.events.borrow(), ["scalar", "projection"]);
    assert!(matches!(
        error,
        CaptureSnapshotError::Native {
            error: Native {
                kind: "projection",
                ..
            },
            ..
        }
    ));
    assert_eq!(scope.len(), 2);
}

#[test]
fn reentry_borrows_original_projections_and_preserves_parent_scope_and_wire() {
    for grouped in [false, true] {
        let mut saved = binding();
        if grouped {
            saved.body = Computation::Field {
                value: Box::new(Computation::Member {
                    group: "batch".into(),
                    name: "member".into(),
                    operation: HostOperation::UiFocus,
                }),
                field: ResultField::NodeId,
            };
        }
        let before = serde_json::to_vec(&saved).unwrap();
        let host = Host::new();
        let environment = Environment {
            host: &host,
            seen: RefCell::new(vec![]),
            marker: PhantomData,
        };
        let value = Value::UiFocus {
            node_id: "pending".into(),
        };
        let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
        let mut scope = ScopeFrame::new(&mut rows);
        let outcome = resume_preflighted_result_binding_in_scope(
            &saved,
            &mut scope,
            &environment,
            &mut Fuel::new(100),
            LIMITS,
            pending(&value),
            &host,
        )
        .unwrap();
        let leselang_hir_core::effect_reentry::ResumableEffectOutcome::Value(PureValue::Scalar(
            output,
        )) = outcome
        else {
            panic!()
        };
        assert_eq!(
            output,
            ScalarValue::String(if grouped { "b" } else { "a" }.into())
        );
        assert_eq!(
            *host.events.borrow(),
            ["scalar", "allow", "result", "allow", "group", "field"]
        );
        let original = if grouped {
            &saved.groups[0].group.members[0].result
        } else {
            &saved.results[0].result
        };
        assert_eq!(
            *environment.seen.borrow(),
            [std::ptr::from_ref(original).addr()]
        );
        assert_eq!(scope.len(), 1);
        assert_eq!(scope.bindings()[0].0, "parent");
        assert_eq!(serde_json::to_vec(&saved).unwrap(), before);
    }
}

#[test]
fn reentry_refusals_keep_row_order_native_payloads_and_duplicate_cleanup() {
    let value = Value::UiFocus {
        node_id: "pending".into(),
    };
    for fail in ["scalar", "result", "group", "allow", "field"] {
        let saved = binding();
        let host = Host {
            fail: Some(fail),
            ..Host::new()
        };
        let pointer = std::ptr::from_ref(host.original.borrow().as_ref().unwrap().payload.as_ref());
        let environment = Environment {
            host: &host,
            seen: RefCell::new(vec![]),
            marker: PhantomData,
        };
        let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
        let mut scope = ScopeFrame::new(&mut rows);
        let error = resume_preflighted_result_binding_in_scope(
            &saved,
            &mut scope,
            &environment,
            &mut Fuel::new(100),
            LIMITS,
            pending(&value),
            &host,
        )
        .unwrap_err();
        assert!(!format!("{error:?}").contains("private-native-error"));
        let CaptureScopeError::Native {
            error: CalculationFailure::External(fault),
            ..
        } = error
        else {
            panic!()
        };
        let native = match fault {
            EffectEvaluationFault::Native(error)
            | EffectEvaluationFault::Pure(
                leselang_hir_core::pure_evaluation::PureEvaluationFault::Native(error),
            ) => error,
            _ => panic!(),
        };
        assert_eq!(std::ptr::from_ref(native.payload.as_ref()), pointer);
        assert_eq!(scope.len(), 1);
        assert_eq!(host.events.borrow().last(), Some(&fail));
    }
    for duplicate in [false, true] {
        let mut saved = binding();
        let host = Host {
            saved: duplicate,
            ..Host::new()
        };
        if duplicate {
            saved.name = "saved".into();
        }
        let environment = Environment {
            host: &host,
            seen: RefCell::new(vec![]),
            marker: PhantomData,
        };
        let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
        let mut scope = ScopeFrame::new(&mut rows);
        let mut fuel = Fuel::new(100);
        let error = resume_preflighted_result_binding_in_scope(
            &saved,
            &mut scope,
            &environment,
            &mut fuel,
            LIMITS,
            pending(&value),
            &host,
        )
        .unwrap_err();
        if duplicate {
            assert!(matches!(error, CaptureScopeError::Duplicate { .. }));
            assert_eq!(fuel.remaining(), 97);
        } else {
            assert!(matches!(
                error,
                CaptureScopeError::Native {
                    error: CalculationFailure::External(EffectEvaluationFault::Native(Native {
                        kind: "saved-refused",
                        ..
                    })),
                    ..
                }
            ));
            assert_eq!(fuel.remaining(), 99);
            assert_eq!(*host.events.borrow(), ["scalar", "allow"]);
        }
        assert_eq!(scope.len(), 1);
        assert_eq!(scope.bindings()[0].0, "parent");
    }
}

#[test]
fn native_unwind_never_refills_retries_or_leaks_restored_rows() {
    let saved = binding();
    let value = Value::UiFocus {
        node_id: "pending".into(),
    };
    let host = Host {
        fail: Some("group"),
        unwind: true,
        ..Host::new()
    };
    let environment = Environment {
        host: &host,
        seen: RefCell::new(vec![]),
        marker: PhantomData,
    };
    let mut rows = vec![("parent", PureValue::Scalar(ScalarValue::Integer(42)))];
    let mut scope = ScopeFrame::new(&mut rows);
    let mut fuel = Fuel::new(100);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = resume_preflighted_result_binding_in_scope(
                &saved,
                &mut scope,
                &environment,
                &mut fuel,
                LIMITS,
                pending(&value),
                &host,
            );
        }))
        .is_err()
    );
    assert_eq!(fuel.remaining(), 98);
    assert_eq!(
        *host.events.borrow(),
        ["scalar", "allow", "result", "allow", "group"]
    );
    assert_eq!(scope.len(), 1);
    assert_eq!(scope.bindings()[0].0, "parent");
    let mut rows = vec![
        ("n", PureValue::Scalar(ScalarValue::Integer(3))),
        (
            "batch",
            PureValue::Result(ResultView::ProjectedGroup(&saved.groups[0].group)),
        ),
    ];
    let scope = ScopeFrame::new(&mut rows);
    host.events.borrow_mut().clear();
    let mut fuel = Fuel::new(100);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = snapshot_preflighted_result_scope(
                "pending",
                &saved.body,
                &scope,
                &mut fuel,
                CAPTURE,
                &host,
                |_| true,
            );
        }))
        .is_err()
    );
    assert_eq!(fuel.remaining(), 99);
    assert_eq!(*host.events.borrow(), ["scalar", "group"]);
    assert_eq!(scope.len(), 2);
}

#[test]
fn predicate_cleanup_unwind_drops_the_locally_owned_native_error_before_handoff() {
    struct Cleanup;
    impl Drop for Cleanup {
        fn drop(&mut self) {
            panic!("predicate cleanup unwind");
        }
    }
    let dropped = Rc::new(Cell::new(false));
    let host = Host {
        fail: Some("scalar"),
        original: RefCell::new(Some(Native {
            kind: "private-native-error",
            payload: Box::new(91),
            dropped: Some(dropped.clone()),
        })),
        ..Host::new()
    };
    let mut rows = vec![("n", PureValue::Scalar(ScalarValue::Integer(3)))];
    let scope = ScopeFrame::new(&mut rows);
    let mut fuel = Fuel::new(100);
    let cleanup = Cleanup;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = snapshot_preflighted_result_scope(
                "pending",
                &field("pending"),
                &scope,
                &mut fuel,
                CAPTURE,
                &host,
                move |_| {
                    let _keep_alive = &cleanup;
                    true
                },
            );
        }))
        .is_err()
    );
    assert!(
        dropped.get(),
        "callback cleanup must not strand the native error in a return place"
    );
    assert!(host.original.borrow().is_none());
    assert_eq!(*host.events.borrow(), ["scalar"]);
    assert_eq!(fuel.remaining(), 100);
    assert_eq!(scope.len(), 1);
}
