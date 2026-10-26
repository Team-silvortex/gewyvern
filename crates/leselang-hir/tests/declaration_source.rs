use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::helper_body::{HelperBody, HelperBodyLimits};
use leselang_hir::helper_body_source::{
    HelperBodyObservation, HelperBodySourceError, HelperBodySourceLimits,
    lower_declared_helper_body, lower_helper_body,
};
use leselang_hir::helper_declarations::{
    HelperDeclarationError, HelperDeclarationLimits, HelperDeclarations, accept_helper_declarations,
};
use leselang_hir::helper_dependencies::HelperDependencyLimits;
use leselang_hir::helper_registry::{HelperRegistryError, HelperRegistryLimits};
use leselang_hir::helper_templates::HelperTemplateLimits;
use leselang_hir::ir::Computation;
use leselang_hir::program_source::{
    ProgramSourceError, ProgramSourceLimits, assemble_declared_program, assemble_program,
};
use leselang_hir::source_call::SourceShapeError;
use leselang_hir::source_cost::{SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::{ScalarType, ScalarValue};
use leselang_syntax::{Expression, Function, parse};

const DECLARATIONS: HelperDeclarationLimits = HelperDeclarationLimits {
    max_functions: 32,
    max_parameters: 8,
    max_source_nodes: 128,
    max_source_depth: 16,
};
const BODY: HelperBodySourceLimits = HelperBodySourceLimits {
    max_source_nodes: 128,
    max_source_depth: 16,
    max_arguments: 64,
    body: HelperBodyLimits {
        template: HelperTemplateLimits {
            max_nodes: 128,
            max_depth: 16,
            max_bindings: 16,
            max_parameters: 8,
        },
        source_cost: SourceCostLimits {
            max_nodes: 128,
            max_depth: 16,
        },
    },
};
const PROGRAM: ProgramSourceLimits = ProgramSourceLimits {
    helpers: HelperRegistryLimits {
        dependencies: HelperDependencyLimits {
            max_helpers: 31,
            max_source_nodes: 128,
            max_source_depth: 16,
        },
        template: BODY.body.template,
        source_cost: BODY.body.source_cost,
    },
    max_entry_source_nodes: 128,
    max_entry_source_depth: 16,
    max_entry_arguments: 64,
};
type Data = Computation<(), (), (), ()>;
type Prepared = HelperBody<Data, ScalarType>;
// Deliberately no Clone/Debug/serde/Send bounds on native data or errors.
struct PrivateError<'a>(&'a str);
struct Owned<'a>(&'a Cell<usize>);
impl Drop for Owned<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
fn function(source: &str) -> Function {
    let parsed = parse(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap()
}
fn declarations<'a>(functions: &[&'a Function]) -> HelperDeclarations<'a> {
    accept_helper_declarations(functions, "boot", DECLARATIONS, |_| {
        Ok::<_, Infallible>(false)
    })
    .unwrap()
}
fn observed() -> HelperBodyObservation<Data, ScalarType> {
    HelperBodyObservation {
        expression: Data::Literal {
            value: ScalarValue::Integer(7),
        },
        result_type: ScalarType::Integer,
        scalar_result: Some(ScalarType::Integer),
    }
}

#[test]
fn receipts_borrow_only_original_declarations_without_exposing_source_in_debug() {
    let boot = function("fn boot() = 0");
    let helper = function("fn private_name(value: integer) = value");
    let duplicate = helper.clone();
    let accepted = declarations(&[&helper, &boot]);
    assert!(accepted.source_for(&duplicate).is_none());
    assert!(std::ptr::eq(accepted.entry_source().function(), &boot));
    let receipt = accepted.source_for(&helper).unwrap();
    assert!(std::ptr::eq(receipt.function(), &helper));
    assert_eq!(format!("{receipt:?}"), "DeclarationBody");
    drop(accepted);
    assert!(std::ptr::eq(receipt.function(), &helper));
}

#[test]
fn tightened_receipts_match_fresh_physical_admission_for_every_node_and_depth_cutoff() {
    let mut cases = 0;
    for source in [
        "0",
        "a(x: 0)",
        "a(x: b(y: 0))",
        "a(x: 0, y: b(z: 1))",
        "a(x: b(y: 0), z: c(k: d(v: 1)))",
    ] {
        let boot = function(&format!("fn boot() = {source}"));
        let accepted = declarations(&[&boot]);
        for max_nodes in 0..=8 {
            for max_depth in 0..=4 {
                let expected = accept_helper_declarations(
                    &[&boot],
                    "boot",
                    HelperDeclarationLimits {
                        max_source_nodes: max_nodes,
                        max_source_depth: max_depth,
                        ..DECLARATIONS
                    },
                    |_| Ok::<_, Infallible>(false),
                );
                let calls = Cell::new(0);
                let mut used = 0;
                let result = lower_declared_helper_body(
                    accepted.entry_source(),
                    HelperBodySourceLimits {
                        max_source_nodes: max_nodes,
                        max_source_depth: max_depth,
                        ..BODY
                    },
                    &mut used,
                    |_, _, _| {
                        calls.set(calls.get() + 1);
                        Err::<HelperBodyObservation<Data, ScalarType>, _>(PrivateError("native"))
                    },
                    |_, _, _, _| -> Result<(), Infallible> { panic!("no output") },
                    |_| -> Result<SourceCostExtra, Infallible> { panic!("no output") },
                );
                match (expected, result) {
                    (Ok(_), Err(HelperBodySourceError::Lowering(error))) => {
                        assert_eq!(error.0, "native");
                        assert_eq!(calls.get(), 1);
                    }
                    (
                        Err(HelperDeclarationError::Source { span, error, .. }),
                        Err(HelperBodySourceError::Source {
                            span: actual,
                            error: SourceShapeError::Structure(found),
                        }),
                    ) => {
                        assert_eq!((span, error), (actual, found));
                        assert_eq!(calls.get(), 0);
                    }
                    _ => {
                        panic!("physical admission diverged at {source}, {max_nodes}, {max_depth}")
                    }
                }
                assert_eq!(used, 0);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 225);
}

#[test]
fn semantic_diagnostics_remain_native_while_strict_ingress_still_scans_cold_source() {
    let mut boot = function("fn boot() = 0");
    boot.body = Expression::Reference {
        name: "private invalid name".into(),
        span: boot.span,
    };
    let accepted = declarations(&[&boot]);
    let mut used = 3;
    let message = String::from("native diagnostic precedes lexical detail");
    let result = lower_declared_helper_body(
        accepted.entry_source(),
        BODY,
        &mut used,
        |original, parameters, counter| {
            assert!(std::ptr::eq(original, &boot));
            assert!(parameters.is_empty());
            *counter += 1;
            Err::<HelperBodyObservation<Data, ScalarType>, _>(PrivateError(&message))
        },
        |_, _, _, _| -> Result<(), Infallible> { panic!() },
        |_| -> Result<SourceCostExtra, Infallible> { panic!() },
    );
    let Err(HelperBodySourceError::Lowering(error)) = result else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), message.as_ptr());
    assert_eq!(used, 4);
    let strict = lower_helper_body(
        &boot,
        BODY,
        &mut used,
        |_, _, _| -> Result<HelperBodyObservation<Data, ScalarType>, Infallible> { panic!() },
        |_, _, _, _| -> Result<(), Infallible> { panic!() },
        |_| -> Result<SourceCostExtra, Infallible> { panic!() },
    );
    assert!(matches!(
        strict,
        Err(HelperBodySourceError::Source {
            error: SourceShapeError::InvalidName,
            ..
        })
    ));
    assert_eq!(used, 4);
}

#[test]
fn native_buffers_signature_counter_admission_and_cost_are_retained_once() {
    let boot = function("fn boot() = 0");
    let helper = function("fn helper(z: integer, a: boolean) = z");
    let accepted = declarations(&[&helper, &boot]);
    let drops = Cell::new(0);
    let events = RefCell::new(vec![]);
    let mut used = 2;
    let counter_address = &mut used as *mut usize;
    let mut effect_address = std::ptr::null();
    let mut result_address = std::ptr::null();
    let output = lower_declared_helper_body(
        accepted.source_for(&helper).unwrap(),
        BODY,
        &mut used,
        |original, parameters, counter| {
            events.borrow_mut().push("lower");
            assert!(std::ptr::eq(original, &helper));
            assert_eq!(
                parameters.iter().map(|p| p.name).collect::<Vec<_>>(),
                ["z", "a"]
            );
            assert_eq!(counter as *mut _, counter_address);
            *counter += 1;
            let effect = Box::new(Owned(&drops));
            effect_address = effect.as_ref() as *const _;
            let result = Box::new(Owned(&drops));
            result_address = result.as_ref() as *const _;
            Ok::<_, PrivateError<'_>>(HelperBodyObservation {
                expression: Computation::<(), (), _, ()>::Host { effect },
                result_type: result,
                scalar_result: None,
            })
        },
        |_, parameters, _, scalar| {
            events.borrow_mut().push("admit");
            assert_eq!(
                parameters,
                &[
                    ("z".into(), ScalarType::Integer),
                    ("a".into(), ScalarType::Boolean)
                ]
            );
            assert!(scalar.is_none());
            Ok::<_, PrivateError<'_>>(())
        },
        |_| {
            events.borrow_mut().push("cost");
            Ok::<_, PrivateError<'_>>(SourceCostExtra { nodes: 0, depth: 0 })
        },
    )
    .unwrap();
    let Computation::Host { effect } = output.template().body() else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const _, effect_address);
    assert_eq!(
        output.template().result_type().as_ref() as *const _,
        result_address
    );
    assert_eq!(&*events.borrow(), &["lower", "admit", "cost"]);
    assert_eq!(used, 3);
    assert_eq!(drops.get(), 0);
    drop(output);
    assert_eq!(drops.get(), 2);
}

#[test]
fn callback_overcharge_and_unwind_never_refund_or_publish_a_partial_body() {
    let boot = function("fn boot() = 0");
    let accepted = declarations(&[&boot]);
    let drops = Cell::new(0);
    let mut used = 0;
    let result = lower_declared_helper_body(
        accepted.entry_source(),
        BODY,
        &mut used,
        |_, _, counter| {
            *counter = 129;
            Ok::<_, Infallible>(HelperBodyObservation {
                expression: Computation::<(), (), _, ()>::Host {
                    effect: Box::new(Owned(&drops)),
                },
                result_type: (),
                scalar_result: None,
            })
        },
        |_, _, _, _| -> Result<(), Infallible> { panic!("overcharge must precede admission") },
        |_| -> Result<SourceCostExtra, Infallible> { panic!() },
    );
    assert!(matches!(
        result,
        Err(HelperBodySourceError::SourceCounterLimit)
    ));
    assert_eq!((used, drops.get()), (129, 1));
    used = 0;
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = lower_declared_helper_body(
                accepted.entry_source(),
                BODY,
                &mut used,
                |_, _, counter| {
                    *counter += 1;
                    Ok::<_, Infallible>(observed())
                },
                |_, _, _, _| -> Result<(), Infallible> { panic!("native admission unwind") },
                |_| -> Result<SourceCostExtra, Infallible> { panic!() },
            );
        }))
        .is_err()
    );
    assert_eq!(used, 1);
}

#[test]
fn declared_program_preserves_helper_error_priority_over_cold_entry_semantics() {
    let mut boot = function("fn boot() = 0");
    boot.body = Expression::Reference {
        name: "invalid entry".into(),
        span: boot.span,
    };
    let helper = function("fn helper() = 0");
    let accepted = declarations(&[&boot, &helper]);
    let drops = Cell::new(0);
    let result = assemble_declared_program(
        &accepted,
        PROGRAM,
        Owned(&drops),
        |original, _| {
            assert!(std::ptr::eq(original, &helper));
            Err::<Prepared, _>(PrivateError("helper-first"))
        },
        |_, _, _| -> Result<(), Infallible> { panic!() },
        |_, _| -> Result<(), Infallible> { panic!() },
        |_, _, _| -> Result<(), Infallible> { panic!() },
    );
    let Err(ProgramSourceError::Helpers(HelperRegistryError::Lowering { error, .. })) = result
    else {
        panic!()
    };
    assert_eq!(error.0, "helper-first");
    assert_eq!(drops.get(), 1);
    let strict = assemble_program(
        &accepted,
        PROGRAM,
        Owned(&drops),
        |_, _| -> Result<Prepared, Infallible> { panic!() },
        |_, _, _| -> Result<(), Infallible> { panic!() },
        |_, _| -> Result<(), Infallible> { panic!() },
        |_, _, _| -> Result<(), Infallible> { panic!() },
    );
    assert!(matches!(
        strict,
        Err(ProgramSourceError::Source {
            error: SourceShapeError::InvalidName,
            ..
        })
    ));
    assert_eq!(drops.get(), 2);
}

#[test]
fn program_current_physical_limits_and_final_native_admission_cannot_be_skipped() {
    let boot = function("fn boot() = a(x: b(y: 0))");
    let accepted = declarations(&[&boot]);
    let drops = Cell::new(0);
    for (nodes, depth) in [(0, 16), (2, 16), (128, 1)] {
        let result = assemble_declared_program(
            &accepted,
            ProgramSourceLimits {
                max_entry_source_nodes: nodes,
                max_entry_source_depth: depth,
                ..PROGRAM
            },
            Owned(&drops),
            |_, _| -> Result<Prepared, Infallible> { panic!() },
            |_, _, _| -> Result<(), Infallible> { panic!() },
            |_, _| -> Result<(), Infallible> { panic!() },
            |_, _, _| -> Result<(), Infallible> { panic!() },
        );
        assert!(matches!(
            result,
            Err(ProgramSourceError::Source {
                error: SourceShapeError::Structure(_),
                ..
            })
        ));
    }
    assert_eq!(drops.get(), 3);
    let order = RefCell::new(vec![]);
    let result = assemble_declared_program(
        &accepted,
        PROGRAM,
        Owned(&drops),
        |_, _| -> Result<Prepared, Infallible> { panic!() },
        |_, _, _| -> Result<(), Infallible> { panic!() },
        |original, _| {
            assert!(std::ptr::eq(original, &boot));
            order.borrow_mut().push("entry");
            Ok::<_, Infallible>(Owned(&drops))
        },
        |original, _, _| {
            assert!(std::ptr::eq(original, &boot));
            order.borrow_mut().push("admit");
            Err(PrivateError("final"))
        },
    );
    let Err(ProgramSourceError::Admission { error, .. }) = result else {
        panic!()
    };
    assert_eq!(error.0, "final");
    assert_eq!(&*order.borrow(), &["entry", "admit"]);
    assert_eq!(drops.get(), 5);
}

#[test]
fn foreign_program_composes_receipts_helpers_registration_and_complete_state_admission() {
    let boot = function("fn boot() = helper()");
    let helper = function("fn helper() = 7");
    let accepted = declarations(&[&boot, &helper]);
    let events = RefCell::new(vec![]);
    let mut used = 0;
    let prepared = assemble_declared_program(
        &accepted,
        PROGRAM,
        Vec::<Prepared>::new(),
        |function, _| {
            events.borrow_mut().push("prepare");
            lower_declared_helper_body(
                accepted.source_for(function).unwrap(),
                BODY,
                &mut used,
                |original, _, counter| {
                    assert!(std::ptr::eq(original, &helper));
                    *counter += 1;
                    Ok::<_, Infallible>(observed())
                },
                |_, _, _, _| Ok::<_, Infallible>(()),
                |_| -> Result<SourceCostExtra, Infallible> { panic!() },
            )
        },
        |original, body, state| {
            assert!(std::ptr::eq(original, &helper));
            events.borrow_mut().push("register");
            state.push(body);
            Ok::<_, Infallible>(())
        },
        |original, state| {
            assert!(std::ptr::eq(original, &boot));
            assert_eq!(state.len(), 1);
            events.borrow_mut().push("entry");
            Ok::<_, Infallible>(Box::new(7_u64))
        },
        |original, output, state| {
            assert!(std::ptr::eq(original, &boot));
            assert_eq!(state.len(), 1);
            assert_eq!(**output, 7);
            events.borrow_mut().push("admit");
            Ok::<_, Infallible>(())
        },
    )
    .unwrap();
    assert_eq!(
        &*events.borrow(),
        &["prepare", "register", "entry", "admit"]
    );
    assert_eq!(used, 1);
    let output_address = prepared.output().as_ref() as *const _;
    let (entry, output, state) = prepared.into_parts();
    assert!(std::ptr::eq(entry, &boot));
    assert_eq!(output.as_ref() as *const _, output_address);
    assert_eq!(state.len(), 1);
}

#[test]
fn program_admission_unwind_drops_original_state_and_output_without_retry() {
    let boot = function("fn boot() = 0");
    let accepted = declarations(&[&boot]);
    let drops = Cell::new(0);
    let calls = Cell::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = assemble_declared_program(
                &accepted,
                PROGRAM,
                Owned(&drops),
                |_, _| -> Result<Prepared, Infallible> { panic!() },
                |_, _, _| -> Result<(), Infallible> { panic!() },
                |_, _| Ok::<_, Infallible>(Owned(&drops)),
                |_, _, _| -> Result<(), Infallible> {
                    calls.set(calls.get() + 1);
                    panic!("native admission unwind")
                },
            );
        }))
        .is_err()
    );
    assert_eq!(drops.get(), 2);
    assert_eq!(calls.get(), 1);
}
