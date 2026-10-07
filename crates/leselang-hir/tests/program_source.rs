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
use leselang_hir::helper_registry::{HelperRegistryError, HelperRegistryLimits};
use leselang_hir::helper_templates::HelperTemplateLimits;
use leselang_hir::ir::Computation;
use leselang_hir::program_source::*;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::{SourceCallLimits, SourceShapeError};
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
const LIMITS: ProgramSourceLimits = ProgramSourceLimits {
    helpers: HelperRegistryLimits {
        dependencies: HelperDependencyLimits {
            max_helpers: 31,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
        template: TEMPLATE,
        source_cost: COST,
    },
    max_entry_source_nodes: 128,
    max_entry_source_depth: 16,
    max_entry_arguments: 64,
};
fn declarations<'source>(tree: &'source SyntaxTree) -> HelperDeclarations<'source> {
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    let rows = tree
        .function
        .iter()
        .chain(&tree.helpers)
        .collect::<Vec<_>>();
    accept_helper_declarations(
        &rows,
        "boot",
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
fn denied<'source, Output, State, L, R, E, A>(
    result: ProgramSourceResult<'source, Output, State, L, R, E, A>,
) -> ProgramSourceError<L, R, E, A> {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected denial"),
    }
}
struct PrepareError(&'static str);
struct RegisterError(&'static str);
struct EntryError(&'static str);
struct AdmissionError(&'static str);

#[derive(Default)]
struct Audit {
    events: RefCell<Vec<String>>,
    charges: Cell<usize>,
    native_drops: Cell<usize>,
    state_drops: Cell<usize>,
    output_drops: Cell<usize>,
}
// All owned native slots/state/output/errors are move-only and GUI-local.
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
struct Output {
    blob: Native,
    result: Native,
    revision: u64,
}
impl Drop for Output {
    fn drop(&mut self) {
        self.blob
            .audit
            .output_drops
            .set(self.blob.audit.output_drops.get() + 1);
    }
}
fn native(audit: &Rc<Audit>) -> Native {
    Native {
        bytes: vec![7; 37],
        audit: audit.clone(),
    }
}
fn state(audit: &Rc<Audit>) -> State {
    State {
        marker: native(audit),
        rows: vec![],
    }
}
fn prepare(
    function: &Function,
    state: &mut State,
) -> Result<HelperBody<NativeNode, Native>, PrepareError> {
    let audit = &state.marker.audit;
    audit
        .events
        .borrow_mut()
        .push(format!("prepare:{}", function.name));
    audit.charges.set(audit.charges.get() + 1);
    LoweredHelperBody {
        parameters: vec![],
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
        |_, _, _, _| Ok::<_, PrepareError>(()),
        |_| Ok::<_, PrepareError>(SourceCostExtra { nodes: 0, depth: 0 }),
    )
    .map_err(|_| PrepareError("prepare-admission"))
}
fn register(
    function: &Function,
    body: HelperBody<NativeNode, Native>,
    state: &mut State,
) -> Result<(), RegisterError> {
    state
        .marker
        .audit
        .events
        .borrow_mut()
        .push(format!("register:{}", function.name));
    state.rows.push((function.name.clone(), body));
    Ok(())
}
fn entry(function: &Function, state: &mut State) -> Result<Output, EntryError> {
    let audit = &state.marker.audit;
    audit
        .events
        .borrow_mut()
        .push(format!("entry:{}", function.name));
    audit.charges.set(audit.charges.get() + 1);
    Ok(Output {
        blob: native(audit),
        result: native(audit),
        revision: 1,
    })
}
fn admit(function: &Function, output: &Output, state: &mut State) -> Result<(), AdmissionError> {
    state
        .marker
        .audit
        .events
        .borrow_mut()
        .push(format!("admit:{}", function.name));
    if output.revision != 1 {
        return Err(AdmissionError("private-stale-output"));
    }
    Ok(())
}
fn forbid<'source>(
    declarations: &HelperDeclarations<'source>,
    limits: ProgramSourceLimits,
    state: State,
) -> ProgramSourceResult<
    'source,
    Output,
    State,
    PrepareError,
    RegisterError,
    EntryError,
    AdmissionError,
> {
    assemble_program(
        declarations,
        limits,
        state,
        |_, _| -> Result<HelperBody<NativeNode, Native>, PrepareError> {
            panic!("prepare must not run")
        },
        |_, _, _| -> Result<(), RegisterError> { panic!("register must not run") },
        |_, _| -> Result<Output, EntryError> { panic!("entry must not run") },
        |_, _, _| -> Result<(), AdmissionError> { panic!("admit must not run") },
    )
}

#[test]
fn original_custom_entry_runs_after_every_ready_helper_then_whole_output_admission_once() {
    let tree = parse("fn boot() = top()\nfn top() = base()\nfn unused() = 9\nfn base() = 41");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let state = state(&audit);
    let buffer = state.marker.bytes.as_ptr();
    let original_state = Cell::new(std::ptr::null_mut::<State>());
    let expected_entry = declarations.entry();
    let mut output_buffer = std::ptr::null();
    let result = assemble_program(
        &declarations,
        LIMITS,
        state,
        |function, state| {
            if original_state.get().is_null() {
                original_state.set(state);
            }
            assert_eq!(state as *mut State, original_state.get());
            if function.name == "top" {
                assert!(state.rows.iter().any(|(name, _)| name == "base"));
            }
            prepare(function, state)
        },
        register,
        |function, state| {
            assert!(std::ptr::eq(expected_entry, function));
            assert_eq!(state.marker.bytes.as_ptr(), buffer);
            assert_eq!(state.rows.len(), 3);
            let output = entry(function, state)?;
            output_buffer = output.blob.bytes.as_ptr();
            Ok::<_, EntryError>(output)
        },
        admit,
    )
    .unwrap();
    assert!(std::ptr::eq(result.entry(), expected_entry));
    assert_eq!(result.output().blob.bytes.as_ptr(), output_buffer);
    assert_eq!(result.state().marker.bytes.as_ptr(), buffer);
    assert_eq!(
        *audit.events.borrow(),
        [
            "prepare:base",
            "register:base",
            "prepare:top",
            "register:top",
            "prepare:unused",
            "register:unused",
            "entry:boot",
            "admit:boot"
        ]
    );
    assert_eq!(audit.charges.get(), 4);
    assert_eq!(audit.native_drops.get(), 0);
    drop(result);
    assert_eq!(audit.output_drops.get(), 1);
    assert_eq!(audit.state_drops.get(), 1);
    assert_eq!(audit.native_drops.get(), 9);
}

#[test]
fn all_entry_source_ceilings_precede_helper_entry_and_admission_callbacks() {
    let tree = parse("fn boot() = 0\nfn base() = 1");
    let declarations = declarations(&tree);
    for limits in [
        ProgramSourceLimits {
            max_entry_source_nodes: 16_385,
            ..LIMITS
        },
        ProgramSourceLimits {
            max_entry_source_depth: 65,
            ..LIMITS
        },
        ProgramSourceLimits {
            max_entry_arguments: 65,
            ..LIMITS
        },
    ] {
        let audit = Rc::new(Audit::default());
        assert!(matches!(
            denied(forbid(&declarations, limits, state(&audit))),
            ProgramSourceError::InvalidLimits
        ));
        assert!(audit.events.borrow().is_empty());
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.native_drops.get(), 1);
    }
}

#[test]
fn cold_entry_names_text_arity_frontier_and_depth_fail_before_any_native_helper_work() {
    for case in 0..6 {
        let mut tree = parse(
            "fn boot() = choose(when: false, then: cold(value: \"ok\"), otherwise: 0)\nfn base() = 1",
        );
        let Expression::Call { arguments, .. } = &mut tree.function.as_mut().unwrap().body else {
            panic!()
        };
        let Expression::Call {
            callee,
            arguments: cold,
            ..
        } = &mut arguments[1].value
        else {
            panic!()
        };
        let mut limits = LIMITS;
        match case {
            0 => *callee = "private-invalid-name!".into(),
            1 => cold[0].name = "private-invalid-label!".into(),
            2 => {
                let Expression::String { value, .. } = &mut cold[0].value else {
                    panic!()
                };
                *value = "x".repeat(MAX_SCALAR_STRING_BYTES + 1);
            }
            3 => limits.max_entry_arguments = 2,
            4 => limits.max_entry_source_nodes = 3,
            5 => limits.max_entry_source_depth = 1,
            _ => unreachable!(),
        }
        let declarations = declarations(&tree);
        let audit = Rc::new(Audit::default());
        let error = denied(forbid(&declarations, limits, state(&audit)));
        assert!(matches!(error, ProgramSourceError::Source { .. }));
        assert!(!format!("{error:?}: {error}").contains("private-"));
        assert!(error.source().is_none());
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.charges.get(), 0);
    }
}

#[test]
fn current_helper_ceilings_and_dependency_errors_never_reach_the_entry() {
    let tree = parse("fn boot() = 0\nfn base() = 1");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let mut limits = LIMITS;
    limits.helpers.template.max_bindings = 1_025;
    assert!(matches!(
        denied(forbid(&declarations, limits, state(&audit))),
        ProgramSourceError::Helpers(HelperRegistryError::InvalidLimits)
    ));
    limits = LIMITS;
    limits.helpers.dependencies.max_helpers = 0;
    assert!(matches!(
        denied(forbid(&declarations, limits, state(&audit))),
        ProgramSourceError::Helpers(HelperRegistryError::Dependency(
            HelperDependencyError::HelperLimit
        ))
    ));
    assert_eq!(audit.charges.get(), 0);
    assert_eq!(audit.state_drops.get(), 2);
}

#[test]
fn late_cycle_drops_completed_helper_state_without_entry_lowering_or_partial_program() {
    let tree = parse("fn boot() = 0\nfn a() = 1\nfn z() = z()");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let error = denied(assemble_program(
        &declarations,
        LIMITS,
        state(&audit),
        prepare,
        register,
        entry,
        admit,
    ));
    assert!(matches!(
        error,
        ProgramSourceError::Helpers(HelperRegistryError::Dependency(
            HelperDependencyError::Cycle { .. }
        ))
    ));
    assert_eq!(*audit.events.borrow(), ["prepare:a", "register:a"]);
    assert_eq!(audit.charges.get(), 1);
    assert_eq!(audit.state_drops.get(), 1);
    assert_eq!(audit.native_drops.get(), 3);
    assert_eq!(audit.output_drops.get(), 0);
}

#[test]
fn ready_helper_native_errors_keep_original_payload_span_and_precede_later_cycle() {
    let tree = parse("fn boot() = 0\nfn a() = 1\nfn z() = z()");
    let declarations = declarations(&tree);
    for registration in [false, true] {
        let audit = Rc::new(Audit::default());
        let error = denied(assemble_program(
            &declarations,
            LIMITS,
            state(&audit),
            |function, state| {
                if !registration {
                    return Err(PrepareError("private-prepare"));
                }
                prepare(function, state)
            },
            |function, body, state| {
                register(function, body, state)?;
                Err(RegisterError("private-register"))
            },
            entry,
            admit,
        ));
        assert!(!format!("{error:?}: {error}").contains("private-"));
        assert!(error.source().is_none());
        match error {
            ProgramSourceError::Helpers(HelperRegistryError::Lowering {
                span,
                error: PrepareError("private-prepare"),
            })
            | ProgramSourceError::Helpers(HelperRegistryError::Registration {
                span,
                error: RegisterError("private-register"),
            }) => assert_eq!(span, tree.helpers[0].span),
            _ => panic!("native ready error must precede cycle"),
        }
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.output_drops.get(), 0);
    }
}

#[test]
fn entry_error_and_unwind_drop_owned_registry_without_admission_retry_or_counter_refund() {
    let tree = parse("fn boot() = base()\nfn base() = 41");
    let declarations = declarations(&tree);
    for unwind in [false, true] {
        let audit = Rc::new(Audit::default());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            assemble_program(
                &declarations,
                LIMITS,
                state(&audit),
                prepare,
                register,
                |function, state| -> Result<Output, EntryError> {
                    assert!(std::ptr::eq(function, declarations.entry()));
                    assert_eq!(state.rows.len(), 1);
                    state.marker.audit.charges.set(9);
                    if unwind {
                        panic!("entry-unwind");
                    }
                    Err(EntryError("private-entry"))
                },
                |_, _, _| -> Result<(), AdmissionError> { panic!("admission must not run") },
            )
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = denied(outcome.unwrap());
            assert!(!format!("{error:?}: {error}").contains("private-"));
            assert!(error.source().is_none());
            assert!(
                matches!(error, ProgramSourceError::Entry { span, error: EntryError("private-entry") } if span == declarations.entry().span)
            );
        }
        assert_eq!(audit.charges.get(), 9);
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.native_drops.get(), 3);
        assert_eq!(audit.output_drops.get(), 0);
    }
}

#[test]
fn final_admission_error_and_unwind_drop_original_output_and_state_without_partial_handoff() {
    let tree = parse("fn boot() = base()\nfn base() = 41");
    let declarations = declarations(&tree);
    for unwind in [false, true] {
        let audit = Rc::new(Audit::default());
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            assemble_program(
                &declarations,
                LIMITS,
                state(&audit),
                prepare,
                register,
                entry,
                |function, output, state| {
                    assert!(std::ptr::eq(function, declarations.entry()));
                    assert_eq!(output.blob.bytes.len(), 37);
                    assert_eq!(state.rows.len(), 1);
                    state.marker.audit.charges.set(11);
                    if unwind {
                        panic!("admission-unwind");
                    }
                    Err(AdmissionError("private-admission"))
                },
            )
        }));
        if unwind {
            assert!(outcome.is_err());
        } else {
            let error = denied(outcome.unwrap());
            assert!(!format!("{error:?}: {error}").contains("private-"));
            assert!(error.source().is_none());
            assert!(
                matches!(error, ProgramSourceError::Admission { span, error: AdmissionError("private-admission") } if span == declarations.entry().span)
            );
        }
        assert_eq!(audit.charges.get(), 11);
        assert_eq!(audit.state_drops.get(), 1);
        assert_eq!(audit.output_drops.get(), 1);
        assert_eq!(audit.native_drops.get(), 5);
    }
}

#[test]
fn once_entry_and_admission_closures_move_private_buffers_into_and_out_of_original_state() {
    let tree = parse("fn boot() = 0");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let blob = native(&audit);
    let result = native(&audit);
    let blob_ptr = blob.bytes.as_ptr();
    let result_ptr = result.bytes.as_ptr();
    let admission_token = native(&audit);
    let program = assemble_program(
        &declarations,
        LIMITS,
        state(&audit),
        prepare,
        register,
        move |_, _| {
            Ok::<_, EntryError>(Output {
                blob,
                result,
                revision: 1,
            })
        },
        move |_, output, _| {
            assert_eq!(output.blob.bytes.as_ptr(), blob_ptr);
            assert_eq!(output.result.bytes.as_ptr(), result_ptr);
            drop(admission_token);
            Ok::<_, AdmissionError>(())
        },
    )
    .unwrap();
    assert_eq!(format!("{program:?}"), "ProgramSource");
    assert_eq!(audit.native_drops.get(), 1);
    let (function, output, state) = program.into_parts();
    assert!(std::ptr::eq(function, declarations.entry()));
    assert_eq!(output.blob.bytes.as_ptr(), blob_ptr);
    assert_eq!(output.result.bytes.as_ptr(), result_ptr);
    assert_eq!(audit.state_drops.get(), 0);
    drop(state);
    assert_eq!(audit.state_drops.get(), 1);
    assert_eq!(audit.output_drops.get(), 0);
    drop(output);
    assert_eq!(audit.output_drops.get(), 1);
    assert_eq!(audit.native_drops.get(), 4);
}

#[test]
fn empty_helper_zero_policies_still_lower_and_admit_a_parameterless_leaf_entry() {
    let tree = parse("fn boot() = 0");
    let declarations = declarations(&tree);
    let mut limits = LIMITS;
    limits.helpers = HelperRegistryLimits {
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
    limits.max_entry_source_nodes = 1;
    limits.max_entry_source_depth = 0;
    limits.max_entry_arguments = 0;
    let audit = Rc::new(Audit::default());
    let program = assemble_program(
        &declarations,
        limits,
        state(&audit),
        prepare,
        register,
        entry,
        admit,
    )
    .unwrap();
    assert!(program.state().rows.is_empty());
    assert_eq!(*audit.events.borrow(), ["entry:boot", "admit:boot"]);
    drop(program);
    assert_eq!(audit.native_drops.get(), 3);
}

#[test]
fn zero_entry_nodes_reject_before_helpers_even_when_the_helper_policy_is_empty() {
    let tree = parse("fn boot() = 0");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let error = denied(forbid(
        &declarations,
        ProgramSourceLimits {
            max_entry_source_nodes: 0,
            ..LIMITS
        },
        state(&audit),
    ));
    assert!(matches!(
        error,
        ProgramSourceError::Source {
            error: SourceShapeError::Structure(StructureError::NodeLimit),
            ..
        }
    ));
    assert_eq!(audit.state_drops.get(), 1);
}

#[test]
fn unknown_entry_calls_and_stale_native_output_need_explicit_fresh_host_policy() {
    let tree = parse("fn boot() = unknown.call()");
    let declarations = declarations(&tree);
    let audit = Rc::new(Audit::default());
    let error = denied(assemble_program(
        &declarations,
        LIMITS,
        state(&audit),
        prepare,
        register,
        |function, _| -> Result<Output, EntryError> {
            assert!(
                matches!(&function.body, Expression::Call { callee, .. } if callee == "unknown.call")
            );
            Err(EntryError("private-unknown-call"))
        },
        admit,
    ));
    assert!(matches!(
        error,
        ProgramSourceError::Entry {
            error: EntryError("private-unknown-call"),
            ..
        }
    ));
    let error = denied(assemble_program(
        &declarations,
        LIMITS,
        state(&audit),
        prepare,
        register,
        |function, state| {
            let mut output = entry(function, state)?;
            output.revision = 2;
            Ok::<_, EntryError>(output)
        },
        admit,
    ));
    assert!(matches!(
        error,
        ProgramSourceError::Admission {
            error: AdmissionError("private-stale-output"),
            ..
        }
    ));
    assert_eq!(audit.output_drops.get(), 1);
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
) -> Result<HelperBody<Data, ScalarType>, PrepareError> {
    let source = SourceCallLimits {
        max_source_nodes: 128,
        max_source_depth: 16,
        max_lowered_nodes: 128,
        max_lowered_depth: 16,
        max_arguments: 64,
    };
    let mut used = 0;
    let body = lower_helper_body(
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
                    let (value, ty) = finish_helper_instance(
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
                    Ok((value, Some(*ty)))
                },
            )
            .map_err(|_| PrepareError("scalar-source"))?;
            Ok::<_, PrepareError>(HelperBodyObservation {
                expression,
                result_type: ty,
                scalar_result: Some(ty),
            })
        },
        |body, _, result, scalar| {
            if scalar != Some(*result) {
                return Err(PrepareError("scalar-result"));
            }
            ScalarHost
                .admit(body, result)
                .map_err(|_| PrepareError("scalar-type"))
        },
        |_| -> Result<SourceCostExtra, PrepareError> {
            panic!("pure source must not query native cost")
        },
    )
    .map_err(|_| PrepareError("scalar-body"))?;
    state.charges.push((function.name.clone(), used));
    Ok(body)
}

#[test]
fn unrelated_scalar_program_composes_registry_entry_admission_and_value_with_exact_fuel() {
    let tree = parse("fn boot() = plus()\nfn plus() = add(left: base(), right: 1)\nfn base() = 41");
    let declarations = declarations(&tree);
    let program = assemble_program(
        &declarations,
        LIMITS,
        ScalarState {
            rows: vec![],
            charges: vec![],
        },
        scalar_prepare,
        |function, body, state| {
            state.rows.push((function.name.clone(), body));
            Ok::<_, RegisterError>(())
        },
        |function, state| scalar_prepare(function, state).map_err(|_| EntryError("scalar-entry")),
        |_, body, state| {
            assert_eq!(state.rows.len(), 2);
            assert_eq!(body.source_cost().nodes, 3);
            ScalarHost
                .admit(body.template().body(), body.template().result_type())
                .map_err(|_| AdmissionError("scalar-admission"))
        },
    )
    .unwrap();
    assert_eq!(
        program.state().charges,
        [("base".into(), 1), ("plus".into(), 4), ("boot".into(), 4)]
    );
    assert_eq!(program.entry().name, "boot");
    let mut bindings = Vec::new();
    let mut fuel = Fuel::new(100);
    let value = evaluate_pure_in_scope(
        program.output().template().body(),
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
fn reference_program_assembly_keeps_canonical_wire_capabilities_and_helper_before_entry_errors() {
    for source in [
        "fn work() = ui.focus(node_id: \"target\")\nfn main() = work()",
        "fn value() = 41\nfn main() = add(left: value(), right: 1)",
        "fn value() = 41\nfn main() = bind(n: value(), body: ui.focus(node_id: to_string(value: n)))",
        "fn work() = ui.focus(node_id: \"target\")\nfn main() = seq(first: work(), second: ui.activate(node_id: \"other\"))",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let again = leselang_hir::lower(&parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&again).unwrap()
        );
        if source.contains("ui.") {
            assert!(
                leselang_hir::authorize(
                    &program,
                    &leselang_host_contract::CapabilitySet::default()
                )
                .is_err()
            );
        }
    }
    let source = "fn main() = missing_entry\nfn ready() = missing_helper\nfn cycle() = cycle()";
    let errors = leselang_hir::lower(&parse(source)).unwrap_err();
    assert!(errors.iter().any(|row| row.code == "LSH1403"));
    assert!(errors.iter().all(|row| row.code != "LSH1502"));
    assert!(errors.iter().all(|row| {
        row.span
            .is_some_and(|span| span.start == source.find("missing_helper").unwrap())
    }));
}
