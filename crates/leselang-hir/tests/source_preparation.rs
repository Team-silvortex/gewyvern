use std::cell::{Cell, RefCell};
use std::convert::Infallible;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::host_source::*;
use leselang_hir::ir::Computation;
use leselang_hir::projection_source::*;
use leselang_hir::source_call::{SourceCallError, SourceCallLimits, SourceShapeError};
use leselang_runtime_core::ScalarType;
use leselang_syntax::{Expression, NamedArgument, Span, parse};

const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 8,
    max_lowered_nodes: 64,
    max_lowered_depth: 8,
    max_arguments: 4,
};
// Borrowed, move-only GUI-local metadata deliberately lacks Clone/Debug/serde/Send.
struct Native<'a> {
    bytes: Vec<u8>,
    drops: &'a Cell<usize>,
}
impl Drop for Native<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError<'a>(&'a str);
type Data<'a> = Computation<Native<'a>, (), Native<'a>, ()>;
fn native(drops: &Cell<usize>) -> Native<'_> {
    Native {
        bytes: vec![7; 31],
        drops,
    }
}
fn expression(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn parts(source: &Expression) -> (&[NamedArgument], Span) {
    let Expression::Call {
        arguments, span, ..
    } = source
    else {
        panic!()
    };
    (arguments, *span)
}

#[test]
fn external_field_plan_retains_original_operands_buffers_and_once_native_order() {
    let source = expression("field(name: \"native\", value: row)");
    let (args, at) = parts(&source);
    let plan = bind_field_source::<Infallible>(args, at).unwrap();
    assert_eq!(format!("{plan:?}"), "BoundFieldSource");
    let drops = Cell::new(0);
    let local = String::from("original");
    let local_buffer = local.as_ptr();
    let selected = native(&drops);
    let field_buffer = selected.bytes.as_ptr();
    let events = RefCell::new(vec![]);
    let (node, ty): (Data<'_>, _) = plan
        .lower(
            LIMITS,
            |original| {
                events.borrow_mut().push("lower");
                assert!(std::ptr::eq(original, &args[1].value));
                Ok::<_, PrivateError<'_>>((Data::Local { name: local }, native(&drops)))
            },
            |name| {
                events.borrow_mut().push("decode");
                let Expression::String { value, .. } = &args[0].value else {
                    panic!()
                };
                assert_eq!(name.as_ptr(), value.as_ptr());
                Ok(selected)
            },
            |input, selected| {
                events.borrow_mut().push("export");
                assert_eq!(input.bytes.len(), 31);
                Ok(Some((selected, ScalarType::Integer)))
            },
        )
        .unwrap();
    assert_eq!(*events.borrow(), ["lower", "decode", "export"]);
    assert_eq!(ty, ScalarType::Integer);
    assert_eq!(drops.get(), 1);
    let Data::Field { value, field } = &node else {
        panic!()
    };
    let Data::Local { name } = value.as_ref() else {
        panic!()
    };
    assert_eq!(name.as_ptr(), local_buffer);
    assert_eq!(field.bytes.as_ptr(), field_buffer);
    drop(node);
    assert_eq!(drops.get(), 2);
}

#[test]
fn external_host_plan_preserves_source_effect_and_result_identity_without_copies() {
    let source = expression("native(value: 7)");
    let plan = bind_host_source_root::<Infallible>(&source, LIMITS).unwrap();
    assert_eq!(format!("{plan:?}"), "BoundHostSource");
    let drops = Cell::new(0);
    let effect = native(&drops);
    let effect_buffer = effect.bytes.as_ptr();
    let result = Box::new(native(&drops));
    let result_pointer = result.as_ref() as *const _;
    let calls = Cell::new(0);
    let (node, result): (Data<'_>, _) = plan
        .lower(
            |original| {
                assert!(std::ptr::eq(original, &source));
                calls.set(calls.get() + 1);
                Ok::<_, PrivateError<'_>>((effect, result))
            },
            |original, effect, result| {
                assert!(std::ptr::eq(original, &source));
                assert_eq!(effect.bytes.as_ptr(), effect_buffer);
                assert_eq!(result.as_ref() as *const _, result_pointer);
                calls.set(calls.get() + 1);
                Ok(())
            },
        )
        .unwrap();
    let Data::Host { effect } = &node else {
        panic!()
    };
    assert_eq!(effect.bytes.as_ptr(), effect_buffer);
    assert_eq!(result.as_ref() as *const _, result_pointer);
    assert_eq!((calls.get(), drops.get()), (2, 0));
    drop((node, result));
    assert_eq!(drops.get(), 2);
}

struct StrictEnvironment;
impl<'a> ProjectionSourceEnvironment<'a, (), (), (), ()> for StrictEnvironment {
    type Result = ();
    type Error = Infallible;
    fn lower_value(
        &mut self,
        _: &'a Expression,
    ) -> NativeProjectionInputResult<Computation<(), (), (), ()>, (), Infallible> {
        panic!("strict scan must reject first")
    }
    fn field(&mut self, _: &(), _: &'a str) -> Result<Option<((), ScalarType)>, Infallible> {
        panic!()
    }
    fn member(&mut self, _: &'a str, _: &'a str) -> Result<Option<((), ())>, Infallible> {
        panic!()
    }
}

#[test]
fn native_child_diagnostic_priority_does_not_weaken_the_strict_projection_entry() {
    let source = expression("field(value: missing, name: 1)");
    let (args, at) = parts(&source);
    let message = String::from("native child refusal");
    let result = bind_field_source::<Infallible>(args, at).unwrap().lower(
        LIMITS,
        |_| Err::<(Data<'_>, ()), _>(PrivateError(&message)),
        |_| -> Result<(), PrivateError<'_>> { panic!() },
        |_, _| -> Result<Option<(Native<'_>, ScalarType)>, PrivateError<'_>> { panic!() },
    );
    let Err(ProjectionSourceError::Native { error, .. }) = result else {
        panic!()
    };
    assert_eq!(error.0.as_ptr(), message.as_ptr());
    assert!(matches!(
        lower_projection_source(&source, LIMITS, &mut StrictEnvironment),
        Err(ProjectionSourceError::FieldName { .. })
    ));
}

#[test]
fn root_plan_is_not_a_source_receipt_and_strict_host_entry_still_scans_source() {
    let mut source = expression("native(value: 7)");
    let Expression::Call { callee, .. } = &mut source else {
        panic!()
    };
    *callee = "invalid name".into();
    let limits = SourceCallLimits {
        max_source_nodes: 0,
        ..LIMITS
    };
    let result: HostSourceResult<Data<'_>, (), PrivateError<'_>> =
        bind_host_source_root::<Infallible>(&source, limits)
            .unwrap()
            .lower(
                |_| Err(PrivateError("native policy owns cold source")),
                |_, _, _| panic!(),
            );
    assert!(matches!(result, Err(HostSourceError::Preparation { .. })));
    let strict: HostSourceResult<Data<'_>, (), Infallible> =
        lower_host_source(&source, LIMITS, |_| panic!(), |_, _, _| panic!());
    assert!(matches!(
        strict,
        Err(HostSourceError::Source(SourceCallError::Shape {
            error: SourceShapeError::InvalidName,
            ..
        }))
    ));
}

#[test]
fn native_limits_precede_all_callbacks_without_treating_names_as_generated_values() {
    let source = expression("field(value: row, name: \"native\")");
    let (args, at) = parts(&source);
    for (nodes, depth) in [(0, 1), (1, 1), (2, 0)] {
        let limits = SourceCallLimits {
            max_lowered_nodes: nodes,
            max_lowered_depth: depth,
            ..LIMITS
        };
        let result = bind_field_source::<Infallible>(args, at).unwrap().lower(
            limits,
            |_| -> Result<(Data<'_>, ()), Infallible> { panic!() },
            |_| -> Result<(), Infallible> { panic!() },
            |_, _| -> Result<Option<(Native<'_>, ScalarType)>, Infallible> { panic!() },
        );
        assert!(matches!(
            result,
            Err(ProjectionSourceError::Generation { .. })
        ));
    }
    assert!(matches!(
        bind_host_source_root::<Infallible>(
            &source,
            SourceCallLimits {
                max_lowered_nodes: 0,
                ..LIMITS
            }
        ),
        Err(HostSourceError::Output { .. })
    ));
    let leaf = expression("7");
    assert!(matches!(
        bind_host_source_root::<Infallible>(&leaf, LIMITS),
        Err(HostSourceError::NotCall { .. })
    ));
    assert!(matches!(
        bind_host_source_root::<Infallible>(
            &leaf,
            SourceCallLimits {
                max_arguments: 65,
                ..LIMITS
            }
        ),
        Err(HostSourceError::Source(SourceCallError::InvalidLimits))
    ));
}

#[test]
fn host_admission_refusal_or_unwind_releases_original_parts_without_retry_or_refund() {
    let source = expression("native()");
    for unwind in [false, true] {
        let drops = Cell::new(0);
        let spent = Cell::new(0);
        let admitted = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let result: HostSourceResult<Data<'_>, Native<'_>, PrivateError<'_>> =
                bind_host_source_root::<Infallible>(&source, LIMITS)
                    .unwrap()
                    .lower(
                        |_| {
                            spent.set(spent.get() + 1);
                            Ok((native(&drops), native(&drops)))
                        },
                        |_, _, _| {
                            admitted.set(admitted.get() + 1);
                            assert!(!unwind, "admission unwind");
                            Err(PrivateError("refused"))
                        },
                    );
            result
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(matches!(result, Ok(Err(HostSourceError::Admission { .. }))));
        }
        assert_eq!((spent.get(), admitted.get(), drops.get()), (1, 1, 2));
    }
}

#[test]
fn field_decode_purity_and_export_failures_release_original_input_and_native_key() {
    let source = expression("field(value: row, name: \"native\")");
    let (args, at) = parts(&source);
    for policy in 0..4 {
        let drops = Cell::new(0);
        let exports = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            bind_field_source::<Infallible>(args, at).unwrap().lower(
                LIMITS,
                |_| {
                    Ok::<_, PrivateError<'_>>((
                        if policy == 1 {
                            Data::Host {
                                effect: Box::new(native(&drops)),
                            }
                        } else {
                            Data::Local { name: "row".into() }
                        },
                        native(&drops),
                    ))
                },
                |_| {
                    if policy == 0 {
                        Err(PrivateError("decode"))
                    } else {
                        Ok(native(&drops))
                    }
                },
                |_, _: Native<'_>| -> Result<Option<(Native<'_>, ScalarType)>, PrivateError<'_>> {
                    exports.set(exports.get() + 1);
                    assert_ne!(policy, 3, "export unwind");
                    Ok(None)
                },
            )
        }));
        match policy {
            0 => assert!(matches!(
                result,
                Ok(Err(ProjectionSourceError::Native { .. }))
            )),
            1 => assert!(matches!(
                result,
                Ok(Err(ProjectionSourceError::Produced { .. }))
            )),
            2 => assert!(matches!(
                result,
                Ok(Err(ProjectionSourceError::FieldNotExported { .. }))
            )),
            _ => assert!(result.is_err()),
        }
        assert_eq!(exports.get(), usize::from(policy >= 2));
        assert_eq!(drops.get(), [1, 3, 2, 2][policy]);
    }
}

#[test]
fn delayed_field_and_host_plans_query_current_native_policy_before_handoff() {
    let source = expression("field(value: row, name: \"native\")");
    let (args, at) = parts(&source);
    let field = bind_field_source::<Infallible>(args, at).unwrap();
    let host = bind_host_source_root::<Infallible>(&source, LIMITS).unwrap();
    let allowed = Cell::new(true);
    let calls = Cell::new(0);
    allowed.set(false);
    let result = field.lower(
        LIMITS,
        |_| {
            Ok::<_, Infallible>((
                Computation::<(), (), (), ()>::Local { name: "row".into() },
                (),
            ))
        },
        |_| Ok(()),
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(allowed.get().then_some(((), ScalarType::Integer)))
        },
    );
    assert!(matches!(
        result,
        Err(ProjectionSourceError::FieldNotExported { .. })
    ));
    let result: HostSourceResult<Computation<(), (), (), ()>, (), PrivateError<'_>> = host.lower(
        |_| Ok(((), ())),
        |_, _, _| {
            calls.set(calls.get() + 1);
            if allowed.get() {
                Ok(())
            } else {
                Err(PrivateError("revoked"))
            }
        },
    );
    assert!(matches!(result, Err(HostSourceError::Admission { .. })));
    assert_eq!(calls.get(), 2);
}
