use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::helper_body::{HelperBody, HelperBodyLimits, LoweredHelperBody};
use leselang_hir::helper_body_source::{
    HelperBodyObservation, HelperBodySourceLimits, lower_helper_body,
};
use leselang_hir::helper_declarations::{
    HelperDeclarationLimits, HelperDeclarations, accept_helper_declarations,
};
use leselang_hir::helper_dependencies::{HelperDependencyError, HelperDependencyLimits};
use leselang_hir::helper_instance::{HelperInstanceLimits, SelectedHelper};
use leselang_hir::helper_instance_finish::{HelperInstanceFinisher, finish_helper_instance};
use leselang_hir::helper_registry::*;
use leselang_hir::helper_source::{HelperParameterError, helper_parameters};
use leselang_hir::helper_templates::HelperTemplateLimits;
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::{SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Function, SyntaxTree, parse};

const TEMPLATE: HelperTemplateLimits = HelperTemplateLimits {
    max_nodes: 128,
    max_depth: 16,
    max_bindings: 16,
    max_parameters: 8,
};
const COST: SourceCostLimits = SourceCostLimits {
    max_nodes: 128,
    max_depth: 16,
};
const LIMITS: HelperRegistryLimits = HelperRegistryLimits {
    dependencies: HelperDependencyLimits {
        max_helpers: 31,
        max_source_nodes: 128,
        max_source_depth: 16,
    },
    template: TEMPLATE,
    source_cost: COST,
};
fn declarations<'source>(tree: &'source SyntaxTree, entry: &str) -> HelperDeclarations<'source> {
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    let rows = tree
        .function
        .iter()
        .chain(&tree.helpers)
        .collect::<Vec<_>>();
    accept_helper_declarations(
        &rows,
        entry,
        HelperDeclarationLimits {
            max_functions: 32,
            max_parameters: 8,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
        |_| Ok::<_, Infallible>(false),
    )
    .unwrap()
}
fn denied<State, L, R>(result: HelperRegistryResult<State, L, R>) -> HelperRegistryError<L, R> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected registry denial"),
    }
}
struct PrivateError(&'static str);

// State, native slots and native errors are move-only, without Debug/serde/Send.
#[derive(Default)]
struct Audit {
    events: RefCell<Vec<String>>,
    charges: Cell<usize>,
    native_drops: Cell<usize>,
    state_drops: Cell<usize>,
}
struct Native {
    bytes: Vec<u8>,
    audit: Rc<Audit>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.audit
            .native_drops
            .set(self.audit.native_drops.get() + 1);
    }
}
type NativeNode = Computation<Native, Native, Native, Native>;
struct State {
    marker: Native,
    rows: Vec<(String, HelperBody<NativeNode, Native>)>,
}
impl Drop for State {
    fn drop(&mut self) {
        self.marker
            .audit
            .state_drops
            .set(self.marker.audit.state_drops.get() + 1);
    }
}
fn native(audit: &Rc<Audit>) -> Native {
    Native {
        bytes: vec![7; 31],
        audit: audit.clone(),
    }
}
fn state(audit: &Rc<Audit>) -> State {
    State {
        marker: native(audit),
        rows: vec![],
    }
}
fn body(
    parameters: Vec<(String, ScalarType)>,
    audit: &Rc<Audit>,
    extra: SourceCostExtra,
) -> HelperBody<NativeNode, Native> {
    LoweredHelperBody {
        parameters,
        expression: NativeNode::Host {
            effect: Box::new(native(audit)),
        },
        result_type: native(audit),
        scalar_result: None,
    }
    .prepare(
        HelperBodyLimits {
            template: TEMPLATE,
            source_cost: COST,
        },
        |_, _, _, _| Ok::<_, PrivateError>(()),
        |_| Ok::<_, PrivateError>(extra),
    )
    .unwrap()
}
fn prepare(
    function: &Function,
    state: &mut State,
) -> Result<HelperBody<NativeNode, Native>, PrivateError> {
    let audit = &state.marker.audit;
    audit
        .events
        .borrow_mut()
        .push(format!("prepare:{}", function.name));
    audit.charges.set(audit.charges.get() + 1);
    let parameters = helper_parameters(function, 8)
        .unwrap()
        .into_iter()
        .map(|row| (row.name.to_owned(), row.domain))
        .collect();
    Ok(body(
        parameters,
        audit,
        SourceCostExtra { nodes: 0, depth: 0 },
    ))
}
fn register(
    function: &Function,
    body: HelperBody<NativeNode, Native>,
    state: &mut State,
) -> Result<(), PrivateError> {
    state
        .marker
        .audit
        .events
        .borrow_mut()
        .push(format!("register:{}", function.name));
    state.rows.push((function.name.clone(), body));
    Ok(())
}
fn forbid(
    declarations: &HelperDeclarations<'_>,
    limits: HelperRegistryLimits,
    state: State,
) -> HelperRegistryResult<State, PrivateError, PrivateError> {
    assemble_helper_registry(
        declarations,
        limits,
        state,
        |_, _| -> Result<HelperBody<NativeNode, Native>, PrivateError> {
            panic!("prepare must not run")
        },
        |_, _, _| panic!("register must not run"),
    )
}

#[test]
fn borrowed_ready_order_registers_each_helper_before_the_next_without_compiling_the_entry() {
    let tree = parse(
        "fn boot() = top()\nfn top() = leaf()\nfn zebra() = 7\nfn leaf() = 1\nfn alpha() = leaf()",
    );
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    let state = state(&audit);
    let marker_buffer = state.marker.bytes.as_ptr();
    let mut original_state: *mut State = std::ptr::null_mut();
    let returned = assemble_helper_registry(
        &declarations,
        LIMITS,
        state,
        |function, state| {
            if original_state.is_null() {
                original_state = state as *mut State;
            }
            assert_eq!(state as *mut State, original_state);
            assert!(
                declarations
                    .helpers()
                    .iter()
                    .any(|original| std::ptr::eq(*original, function))
            );
            if function.name == "top" || function.name == "alpha" {
                assert!(state.rows.iter().any(|(name, _)| name == "leaf"));
            }
            assert!(state.rows.iter().all(|(name, _)| name != &function.name));
            prepare(function, state)
        },
        register,
    )
    .unwrap();
    assert_eq!(returned.marker.bytes.as_ptr(), marker_buffer);
    assert_eq!(
        *audit.events.borrow(),
        [
            "prepare:leaf",
            "register:leaf",
            "prepare:alpha",
            "register:alpha",
            "prepare:top",
            "register:top",
            "prepare:zebra",
            "register:zebra"
        ]
    );
    assert_eq!(
        returned
            .rows
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["leaf", "alpha", "top", "zebra"]
    );
    assert_eq!(audit.charges.get(), 4);
    assert_eq!(audit.native_drops.get(), 0);
    drop(returned);
    assert_eq!(audit.state_drops.get(), 1);
    assert_eq!(audit.native_drops.get(), 9);
}

#[test]
fn every_dependency_template_and_cost_ceiling_precedes_native_callbacks() {
    let tree = parse("fn boot() = 0\nfn work() = 1");
    let declarations = declarations(&tree, "boot");
    let mut invalid = Vec::new();
    let mut row = LIMITS;
    row.dependencies.max_helpers = 32;
    invalid.push(row);
    let mut row = LIMITS;
    row.dependencies.max_source_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.dependencies.max_source_depth = 65;
    invalid.push(row);
    let mut row = LIMITS;
    row.template.max_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.template.max_depth = 65;
    invalid.push(row);
    let mut row = LIMITS;
    row.template.max_bindings = 1_025;
    invalid.push(row);
    let mut row = LIMITS;
    row.template.max_parameters = 9;
    invalid.push(row);
    let mut row = LIMITS;
    row.source_cost.max_nodes = 16_385;
    invalid.push(row);
    let mut row = LIMITS;
    row.source_cost.max_depth = 65;
    invalid.push(row);
    for limits in invalid {
        let audit = Rc::new(Audit::default());
        assert!(matches!(
            denied(forbid(&declarations, limits, state(&audit))),
            HelperRegistryError::InvalidLimits
        ));
        assert!(audit.events.borrow().is_empty());
        assert_eq!(audit.charges.get(), 0);
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.native_drops.get(), 1);
    }
}

#[test]
fn whole_helper_source_and_cold_entry_calls_fail_before_any_preparation() {
    for source in [
        "fn boot() = 0\nfn work() = choose(when: true, then: 0, otherwise: boot())",
        "fn boot() = 0\nfn work() = add(left: 1, right: 2)",
    ] {
        let tree = parse(source);
        let declarations = declarations(&tree, "boot");
        let audit = Rc::new(Audit::default());
        let limits = if source.contains("add(") {
            HelperRegistryLimits {
                dependencies: HelperDependencyLimits {
                    max_source_nodes: 2,
                    ..LIMITS.dependencies
                },
                ..LIMITS
            }
        } else {
            LIMITS
        };
        assert!(matches!(
            denied(forbid(&declarations, limits, state(&audit))),
            HelperRegistryError::Dependency(
                HelperDependencyError::EntryCall { .. } | HelperDependencyError::Source { .. }
            )
        ));
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.charges.get(), 0);
    }
}

#[test]
fn current_parameter_and_unused_prefix_limits_precede_the_selected_native_prepare() {
    let tree = parse("fn boot() = 0\nfn work(unused: integer) = 1");
    let declarations = declarations(&tree, "boot");
    for bindings in [false, true] {
        let audit = Rc::new(Audit::default());
        let mut limits = LIMITS;
        if bindings {
            limits.template.max_bindings = 0;
        } else {
            limits.template.max_parameters = 0;
        }
        assert!(matches!(
            denied(forbid(&declarations, limits, state(&audit))),
            HelperRegistryError::ParameterScopeLimit { .. }
                | HelperRegistryError::Parameters {
                    error: HelperParameterError::ParameterLimit,
                    ..
                }
        ));
        assert_eq!(audit.charges.get(), 0);
        assert_eq!(audit.state_drops.get(), 1);
    }
}

#[test]
fn prepared_signatures_cannot_substitute_reorder_rename_or_change_domains_before_registration() {
    let tree = parse("fn boot() = 0\nfn work(first: integer, second: string) = first");
    let declarations = declarations(&tree, "boot");
    for parameters in [
        vec![],
        vec![
            ("second".into(), ScalarType::String),
            ("first".into(), ScalarType::Integer),
        ],
        vec![
            ("first".into(), ScalarType::Boolean),
            ("second".into(), ScalarType::String),
        ],
        vec![
            ("invented".into(), ScalarType::Integer),
            ("second".into(), ScalarType::String),
        ],
    ] {
        let audit = Rc::new(Audit::default());
        let mut parameters = Some(parameters);
        let error = denied(assemble_helper_registry(
            &declarations,
            LIMITS,
            state(&audit),
            |_, state| {
                state.marker.audit.charges.set(1);
                Ok::<_, PrivateError>(body(
                    parameters.take().unwrap(),
                    &audit,
                    SourceCostExtra { nodes: 0, depth: 0 },
                ))
            },
            |_, _, _| -> Result<(), PrivateError> {
                panic!("must reject signature before registration")
            },
        ));
        assert!(
            matches!(error, HelperRegistryError::Signature { span } if span == declarations.helpers()[0].span)
        );
        assert_eq!(audit.charges.get(), 1);
        assert_eq!(audit.native_drops.get(), 3);
        assert_eq!(audit.state_drops.get(), 1);
    }
}

#[test]
fn current_template_and_cached_cost_bounds_precede_native_storage_with_depth_first_priority() {
    let tree = parse("fn boot() = 0\nfn work() = 1");
    let declarations = declarations(&tree, "boot");
    for stage in 0..3 {
        let audit = Rc::new(Audit::default());
        let mut limits = LIMITS;
        match stage {
            0 => limits.template.max_nodes = 0,
            1 => {
                limits.source_cost.max_nodes = 0;
                limits.source_cost.max_depth = 0;
            }
            _ => limits.source_cost.max_nodes = 2,
        }
        let error = denied(assemble_helper_registry(
            &declarations,
            limits,
            state(&audit),
            |_, state| {
                state.marker.audit.charges.set(1);
                Ok::<_, PrivateError>(body(vec![], &audit, SourceCostExtra { nodes: 2, depth: 1 }))
            },
            |_, _, _| -> Result<(), PrivateError> { panic!("bounded output must precede storage") },
        ));
        match stage {
            0 => assert!(matches!(error, HelperRegistryError::Template { .. })),
            1 => assert!(matches!(
                error,
                HelperRegistryError::SourceCost {
                    error: StructureError::DepthLimit,
                    ..
                }
            )),
            _ => assert!(matches!(
                error,
                HelperRegistryError::SourceCost {
                    error: StructureError::NodeLimit,
                    ..
                }
            )),
        }
        assert_eq!(audit.charges.get(), 1);
        assert_eq!(audit.native_drops.get(), 3);
        assert_eq!(audit.state_drops.get(), 1);
    }
}

#[test]
fn original_body_boxes_signature_result_buffers_and_cost_move_into_native_storage_once() {
    let tree = parse("fn boot() = 0\nfn work(n: integer) = n");
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    let prepared = body(
        vec![("n".into(), ScalarType::Integer)],
        &audit,
        SourceCostExtra { nodes: 1, depth: 1 },
    );
    let signature = prepared.template().parameters().as_ptr();
    let result = prepared.template().result_type().bytes.as_ptr();
    let NativeNode::Host { effect } = prepared.template().body() else {
        panic!()
    };
    let original_box = &**effect as *const Native;
    let buffer = effect.bytes.as_ptr();
    let mut prepared = Some(prepared);
    let returned = assemble_helper_registry(
        &declarations,
        LIMITS,
        state(&audit),
        |_, _| Ok::<_, PrivateError>(prepared.take().unwrap()),
        |original, prepared, state| {
            assert!(std::ptr::eq(original, declarations.helpers()[0]));
            assert_eq!(prepared.template().parameters().as_ptr(), signature);
            assert_eq!(prepared.template().result_type().bytes.as_ptr(), result);
            let NativeNode::Host { effect } = prepared.template().body() else {
                panic!()
            };
            assert_eq!(&**effect as *const Native, original_box);
            assert_eq!(effect.bytes.as_ptr(), buffer);
            assert_eq!(prepared.source_cost().nodes, 2);
            assert_eq!(prepared.source_cost().depth, 1);
            register(original, prepared, state)
        },
    )
    .unwrap();
    assert_eq!(
        returned.rows[0].1.template().parameters().as_ptr(),
        signature
    );
    assert_eq!(audit.native_drops.get(), 0);
    drop(returned);
    assert_eq!(audit.native_drops.get(), 3);
}

#[test]
fn ready_native_failure_precedes_a_later_cycle_without_eager_order_collection() {
    let tree = parse("fn boot() = 0\nfn cycle() = cycle()\nfn ready() = 1");
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    let error = denied(assemble_helper_registry(
        &declarations,
        LIMITS,
        state(&audit),
        |function, state| -> Result<HelperBody<NativeNode, Native>, PrivateError> {
            assert_eq!(function.name, "ready");
            state.marker.audit.charges.set(7);
            Err(PrivateError("private-ready"))
        },
        register,
    ));
    assert!(matches!(
        error,
        HelperRegistryError::Lowering {
            error: PrivateError("private-ready"),
            ..
        }
    ));
    assert_eq!(audit.charges.get(), 7);
    assert_eq!(audit.state_drops.get(), 1);
}

#[test]
fn late_cycle_drops_already_registered_owned_state_without_returning_a_partial_registry() {
    let tree = parse("fn boot() = 0\nfn cycle() = cycle()\nfn ready() = 1");
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    let error = denied(assemble_helper_registry(
        &declarations,
        LIMITS,
        state(&audit),
        prepare,
        register,
    ));
    assert!(matches!(
        error,
        HelperRegistryError::Dependency(HelperDependencyError::Cycle { remaining: 1 })
    ));
    assert_eq!(*audit.events.borrow(), ["prepare:ready", "register:ready"]);
    assert_eq!(audit.charges.get(), 1);
    assert_eq!(audit.state_drops.get(), 1);
    assert_eq!(audit.native_drops.get(), 3);
}

#[test]
fn later_native_errors_and_unwind_drop_state_and_inputs_without_retry_refund_or_payload_formatting()
{
    let tree = parse("fn boot() = 0\nfn a() = 1\nfn b() = 2");
    let declarations = declarations(&tree, "boot");
    for stage in 0..4 {
        let audit = Rc::new(Audit::default());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            assemble_helper_registry(
                &declarations,
                LIMITS,
                state(&audit),
                |function, state| {
                    if function.name == "b" && (stage == 0 || stage == 2) {
                        state.marker.audit.charges.set(9);
                        if stage == 2 {
                            panic!("native-prepare-unwind");
                        }
                        return Err(PrivateError("private-prepare"));
                    }
                    prepare(function, state)
                },
                |function, body, state| {
                    register(function, body, state)?;
                    if function.name == "b" {
                        if stage == 3 {
                            panic!("native-register-unwind");
                        }
                        return Err(PrivateError("private-register"));
                    }
                    Ok(())
                },
            )
        }));
        if stage < 2 {
            let error = denied(outcome.unwrap());
            assert!(!format!("{error:?}: {error}").contains("private-"));
            assert!(error.source().is_none());
            assert!(matches!(
                error,
                HelperRegistryError::Lowering {
                    error: PrivateError("private-prepare"),
                    ..
                } | HelperRegistryError::Registration {
                    error: PrivateError("private-register"),
                    ..
                }
            ));
        } else {
            assert!(outcome.is_err());
        }
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(
            audit.native_drops.get(),
            if stage == 0 || stage == 2 { 3 } else { 5 }
        );
        assert_eq!(
            audit.charges.get(),
            if stage == 0 || stage == 2 { 9 } else { 2 }
        );
    }
}

#[test]
fn empty_registry_returns_original_state_under_explicit_zero_policies_without_callbacks() {
    let tree = parse("fn boot() = 0");
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    let state = state(&audit);
    let buffer = state.marker.bytes.as_ptr();
    let limits = HelperRegistryLimits {
        dependencies: HelperDependencyLimits {
            max_helpers: 0,
            max_source_nodes: 0,
            max_source_depth: 0,
        },
        template: HelperTemplateLimits {
            max_nodes: 0,
            max_depth: 0,
            max_bindings: 0,
            max_parameters: 0,
        },
        source_cost: SourceCostLimits {
            max_nodes: 0,
            max_depth: 0,
        },
    };
    let returned = forbid(&declarations, limits, state).unwrap();
    assert_eq!(returned.marker.bytes.as_ptr(), buffer);
    assert_eq!(audit.state_drops.get(), 0);
    drop(returned);
    assert_eq!(audit.state_drops.get(), 1);
}

#[test]
fn helper_count_limits_and_unknown_calls_do_not_grant_native_lookup_or_skip_unused_helpers() {
    let tree = parse("fn boot() = 0\nfn work() = unknown.call()");
    let declarations = declarations(&tree, "boot");
    let audit = Rc::new(Audit::default());
    assert!(matches!(
        denied(forbid(
            &declarations,
            HelperRegistryLimits {
                dependencies: HelperDependencyLimits {
                    max_helpers: 0,
                    ..LIMITS.dependencies
                },
                ..LIMITS
            },
            state(&audit)
        )),
        HelperRegistryError::Dependency(HelperDependencyError::HelperLimit)
    ));
    let error = denied(assemble_helper_registry(
        &declarations,
        LIMITS,
        state(&audit),
        |function, _| -> Result<HelperBody<NativeNode, Native>, PrivateError> {
            assert_eq!(function.name, "work");
            Err(PrivateError("native-unknown-call"))
        },
        register,
    ));
    assert!(matches!(
        error,
        HelperRegistryError::Lowering {
            error: PrivateError("native-unknown-call"),
            ..
        }
    ));
    assert_eq!(audit.state_drops.get(), 2);
}

type Data = Computation<(), (), (), ()>;
struct ScalarState {
    rows: Vec<(String, HelperBody<Data, ScalarType>)>,
    charges: Vec<(String, usize)>,
}
struct ScalarHost;
impl PureTypeEnvironment<(), ()> for ScalarHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<(), ()> for ScalarHost {
    type Result = ();
    type Error = ();
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, ()> {
        Err(())
    }
    fn member(&self, _: &(), _: &str, _: &()) -> Result<(), ()> {
        Err(())
    }
}
impl HelperInstanceFinisher<(), (), (), (), ScalarType> for ScalarHost {
    type Error = ();
    fn admit_argument(&mut self, _: &Data, _: ScalarType, _: usize) -> Result<(), ()> {
        Err(())
    }
    fn argument_depth(&mut self, _: &Data) -> Result<usize, ()> {
        Err(())
    }
    fn materialize(&mut self, body: &Data) -> Result<Data, ()> {
        Ok(body.clone())
    }
    fn fresh_name(&mut self) -> Result<String, ()> {
        Err(())
    }
    fn admit(&mut self, body: &Data, result: &ScalarType) -> Result<(), ()> {
        let ty = infer_pure_type(
            body,
            &[],
            &ScalarHost,
            TypeInferenceLimits {
                max_nodes: 128,
                max_depth: 16,
                max_bindings: 16,
            },
        )
        .map_err(|_| ())?;
        if ty == PureType::Scalar(*result) {
            Ok(())
        } else {
            Err(())
        }
    }
}
fn scalar_prepare(
    function: &Function,
    state: &mut ScalarState,
) -> Result<HelperBody<Data, ScalarType>, PrivateError> {
    let source = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let mut used = 0;
    let result = lower_helper_body(
        function,
        HelperBodySourceLimits {
            max_source_nodes: 128,
            max_source_depth: 16,
            max_arguments: 64,
            body: HelperBodyLimits {
                template: TEMPLATE,
                source_cost: COST,
            },
        },
        &mut used,
        |function, parameters, used| {
            let mut pending = vec![&function.body];
            while let Some(expression) = pending.pop() {
                *used += 1;
                if let Expression::Call { arguments, .. } = expression {
                    pending.extend(arguments.iter().map(|row| &row.value));
                }
            }
            let prefix = parameters
                .iter()
                .map(|row| (row.name, row.domain))
                .collect::<Vec<_>>();
            let (expression, ty) = lower_scalar_source_with_scope(
                &function.body,
                ScalarSourceLimits {
                    source,
                    max_bindings: 16,
                },
                &prefix,
                |expression, scope| {
                    let Expression::Call {
                        callee, arguments, ..
                    } = expression
                    else {
                        return Err(());
                    };
                    if !arguments.is_empty() || !scope.is_empty() {
                        return Err(());
                    }
                    let body = state
                        .rows
                        .iter()
                        .find(|(name, _)| name == callee)
                        .map(|(_, body)| body)
                        .ok_or(())?;
                    let (value, result) = finish_helper_instance(
                        SelectedHelper {
                            name: callee,
                            body,
                            reserved_names: &[],
                            caller_depth: 0,
                        },
                        vec![],
                        HelperInstanceLimits {
                            source,
                            max_parameters: 8,
                            max_bindings: 16,
                            max_reserved_names: 128,
                        },
                        used,
                        &mut ScalarHost,
                    )
                    .map_err(|_| ())?;
                    Ok((value, Some(*result)))
                },
            )
            .map_err(|_| PrivateError("scalar-lower"))?;
            Ok::<_, PrivateError>(HelperBodyObservation {
                expression,
                result_type: ty,
                scalar_result: Some(ty),
            })
        },
        |_, _, result, scalar| {
            if scalar == Some(*result) {
                Ok::<_, PrivateError>(())
            } else {
                Err(PrivateError("scalar-result"))
            }
        },
        |_| -> Result<SourceCostExtra, PrivateError> {
            panic!("pure scalar source must not query native cost")
        },
    )
    .map_err(|_| PrivateError("scalar-body"))?;
    state.charges.push((function.name.clone(), used));
    Ok(result)
}

#[test]
fn unrelated_scalar_registry_composes_declarations_bodies_instances_and_entry_with_exact_fuel() {
    let tree = parse("fn boot() = plus()\nfn plus() = add(left: base(), right: 1)\nfn base() = 41");
    let declarations = declarations(&tree, "boot");
    let mut state = assemble_helper_registry(
        &declarations,
        LIMITS,
        ScalarState {
            rows: vec![],
            charges: vec![],
        },
        scalar_prepare,
        |function, body, state| {
            state.rows.push((function.name.clone(), body));
            Ok::<_, PrivateError>(())
        },
    )
    .unwrap();
    assert_eq!(
        state
            .rows
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["base", "plus"]
    );
    let entry = scalar_prepare(declarations.entry(), &mut state)
        .unwrap_or_else(|_| panic!("entry must compile"));
    assert_eq!(
        state.charges,
        [("base".into(), 1), ("plus".into(), 4), ("boot".into(), 4)]
    );
    assert_eq!(entry.source_cost().nodes, 3);
    let mut bindings = Vec::new();
    let mut fuel = Fuel::new(100);
    let value = evaluate_pure_in_scope(
        entry.template().body(),
        &mut ScopeFrame::new(&mut bindings),
        &ScalarHost,
        &mut fuel,
        PureEvaluationLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
        },
    )
    .unwrap();
    assert!(matches!(value, PureValue::Scalar(ScalarValue::Integer(42))));
    assert_eq!(fuel.remaining(), 97);
    assert!(bindings.is_empty());
}

#[test]
fn reference_registry_delegation_preserves_nested_helper_wire_authority_and_ready_error_precedence()
{
    let source = "fn wrap(n: integer) = add(left: base(n: n), right: 1)\nfn main() = ui.focus(node_id: to_string(value: wrap(n: 40)))\nfn base(n: integer) = add(left: n, right: 1)";
    let program = leselang_hir::lower(&parse(source)).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let again = leselang_hir::lower(&parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&again).unwrap()
    );
    assert!(
        leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
            .is_err()
    );
    let source = "fn cycle() = cycle()\nfn ready() = missing\nfn main() = 0";
    let errors = leselang_hir::lower(&parse(source)).unwrap_err();
    assert!(errors.iter().any(|row| row.code == "LSH1403"));
    assert!(errors.iter().all(|row| row.code != "LSH1502"));
    assert!(
        errors
            .iter()
            .all(|row| row.span.is_some_and(|span| span.end <= source.len()))
    );
}
