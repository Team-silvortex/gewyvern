//! Frozen pre-boundary field/host preparation pipelines. Shared source helpers,
//! shape budgets and pure-IR checks remain the original kernels, not a second
//! compiler. Native events, faults, spans and exact output are compared.
use crate::host_source::*;
use crate::ir::Computation;
use crate::projection_source::*;
use crate::source_call::{
    SourceCallError, SourceCallLimits, fixed_named_operands, span, valid_limits,
};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, Span, parse};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod frozen_field {
    use super::*;
    use crate::pure_typing::preflight_with_budget;
    type Node<F, O, H, R> = Computation<F, O, H, R>;
    type NativeFieldInput<Node, Input, Error> = Result<(Node, Input), Error>;
    type NativeFieldExport<Field, Error> = Result<Option<(Field, ScalarType)>, Error>;
    type FieldSourceOutput<Node, Error> = Result<(Node, ScalarType), ProjectionSourceError<Error>>;
    pub(crate) struct FieldSource<'source> {
        pub value: &'source Expression,
        name: &'source Expression,
        at: Span,
    }
    impl<'source> FieldSource<'source> {
        pub fn literal_name<Error>(&self) -> Result<&'source str, ProjectionSourceError<Error>> {
            let Expression::String { value, .. } = self.name else {
                return Err(ProjectionSourceError::FieldName {
                    span: span(self.name),
                });
            };
            Ok(value)
        }

        // Calling frontends own cold source/declaration checks and recursive
        // accounting. Keep child errors before literal/closed-name decoding here;
        // decoding is metadata mapping, never a native export or value query.
        pub(crate) fn lower_preflighted<
            Field,
            Operation,
            HostEffect,
            IrResult,
            Input,
            Key,
            Error,
        >(
            self,
            limits: SourceCallLimits,
            lower: impl FnOnce(
                &'source Expression,
            ) -> NativeFieldInput<
                Node<Field, Operation, HostEffect, IrResult>,
                Input,
                Error,
            >,
            decode: impl FnOnce(&'source str) -> Result<Key, Error>,
            export: impl FnOnce(&Input, Key) -> NativeFieldExport<Field, Error>,
        ) -> FieldSourceOutput<Node<Field, Operation, HostEffect, IrResult>, Error> {
            if !valid_limits(limits) {
                return Err(ProjectionSourceError::Source(
                    SourceCallError::InvalidLimits,
                ));
            }
            let mut budget =
                StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
            budget
                .visit(0, 0, 1)
                .map_err(|error| ProjectionSourceError::Generation {
                    span: self.at,
                    error,
                })?;
            budget
                .check_pending(0, 1)
                .map_err(|error| ProjectionSourceError::Generation {
                    span: self.at,
                    error,
                })?;
            let (value, input) =
                lower(self.value).map_err(|error| ProjectionSourceError::Native {
                    span: span(self.value),
                    error,
                })?;
            let at = self.at;
            self.finish_with_budget(
                value,
                input,
                &mut budget,
                |name| {
                    decode(name).map_err(|error| ProjectionSourceError::Native { span: at, error })
                },
                |input, key| {
                    export(input, key)
                        .map_err(|error| ProjectionSourceError::Native { span: at, error })
                },
            )
        }

        fn finish_with_budget<Field, Operation, HostEffect, IrResult, Input, Key, Error>(
            self,
            value: Node<Field, Operation, HostEffect, IrResult>,
            input: Input,
            budget: &mut StructureBudget,
            decode: impl FnOnce(&'source str) -> Result<Key, ProjectionSourceError<Error>>,
            export: impl FnOnce(&Input, Key) -> NativeFieldExport<Field, ProjectionSourceError<Error>>,
        ) -> FieldSourceOutput<Node<Field, Operation, HostEffect, IrResult>, Error> {
            let key = decode(self.literal_name()?)?;
            preflight_with_budget(&value, 1, budget).map_err(|error| {
                ProjectionSourceError::Produced {
                    span: span(self.value),
                    error,
                }
            })?;
            let (field, ty) = export(&input, key)?
                .ok_or(ProjectionSourceError::FieldNotExported { span: self.at })?;
            Ok((self.construct(value, field), ty))
        }

        pub fn construct<Field, Operation, HostEffect, IrResult>(
            &self,
            value: Node<Field, Operation, HostEffect, IrResult>,
            field: Field,
        ) -> Node<Field, Operation, HostEffect, IrResult> {
            Computation::Field {
                value: Box::new(value),
                field,
            }
        }
    }

    pub(crate) fn field_source<Error>(
        arguments: &[NamedArgument],
        at: Span,
    ) -> Result<FieldSource<'_>, ProjectionSourceError<Error>> {
        let [value, name] = fixed_named_operands(arguments, &["value", "name"])
            .ok_or(ProjectionSourceError::Names { span: at })?;
        Ok(FieldSource { value, name, at })
    }
}
mod frozen_host {
    use super::*;
    type Node<F, O, H, R> = Computation<F, O, H, R>;
    pub(crate) fn lower_preflighted_host_source<
        'source,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Type,
        Error,
    >(
        expression: &'source Expression,
        limits: SourceCallLimits,
        prepare: impl FnOnce(&'source Expression) -> NativeHostSource<HostEffect, Type, Error>,
        admit: impl FnOnce(&'source Expression, &HostEffect, &Type) -> Result<(), Error>,
    ) -> HostSourceResult<Node<Field, Operation, HostEffect, IrResult>, Type, Error> {
        if !valid_limits(limits) {
            return Err(HostSourceError::Source(SourceCallError::InvalidLimits));
        }
        let at = span(expression);
        if !matches!(expression, Expression::Call { .. }) {
            return Err(HostSourceError::NotCall { span: at });
        }
        StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth)
            .visit(0, 0, 0)
            .map_err(|error| HostSourceError::Output { span: at, error })?;
        let (effect, ty) = prepare(expression)
            .map_err(|error| HostSourceError::Preparation { span: at, error })?;
        admit(expression, &effect, &ty)
            .map_err(|error| HostSourceError::Admission { span: at, error })?;
        Ok((
            Computation::Host {
                effect: Box::new(effect),
            },
            ty,
        ))
    }
}
type Data = Computation<u8, (), u8, ()>;
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 8,
    max_lowered_nodes: 64,
    max_lowered_depth: 8,
    max_arguments: 4,
};
const BUDGETS: [(usize, usize); 6] = [(0, 0), (1, 0), (2, 1), (3, 1), (8, 4), (16_385, 65)];
fn expression(text: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {text}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn field_observation(
    result: Result<(Data, ScalarType), ProjectionSourceError<&'static str>>,
) -> String {
    match result {
        Ok(value) => format!("ok:{value:?}"),
        Err(ProjectionSourceError::Source(error)) => format!("source:{error:?}"),
        Err(ProjectionSourceError::Native { span, error }) => format!("native:{span:?}:{error}"),
        Err(ProjectionSourceError::Generation { span, error }) => {
            format!("generation:{span:?}:{error:?}")
        }
        Err(ProjectionSourceError::Produced { span, error }) => {
            format!("produced:{span:?}:{error:?}")
        }
        Err(ProjectionSourceError::Names { span }) => format!("names:{span:?}"),
        Err(ProjectionSourceError::FieldName { span }) => format!("name:{span:?}"),
        Err(ProjectionSourceError::FieldNotExported { span }) => format!("export:{span:?}"),
        Err(error) => format!("{error:?}"),
    }
}
fn host_observation(result: HostSourceResult<Data, u8, &'static str>) -> String {
    match result {
        Ok(value) => format!("ok:{value:?}"),
        Err(HostSourceError::Source(error)) => format!("source:{error:?}"),
        Err(HostSourceError::NotCall { span }) => format!("call:{span:?}"),
        Err(HostSourceError::Output { span, error }) => format!("output:{span:?}:{error:?}"),
        Err(HostSourceError::Preparation { span, error }) => format!("prepare:{span:?}:{error}"),
        Err(HostSourceError::Admission { span, error }) => format!("admit:{span:?}:{error}"),
    }
}
#[test]
fn public_field_plan_matches_frozen_child_metadata_purity_and_export_order_for_864_cases() {
    let mut cases = 0;
    for text in [
        "field(value: row, name: \"native\")",
        "field(name: \"native\", value: row)",
        "field(value: row, name: 1)",
        "field(value: row, name: \"unknown\")",
        "field(value: row, other: \"native\")",
        "field(value: row)",
        "field()",
        "field(value: nested(x: 0), name: \"native\")",
    ] {
        let ast = expression(text);
        let Expression::Call {
            arguments,
            span: at,
            ..
        } = &ast
        else {
            panic!()
        };
        for (nodes, depth) in BUDGETS {
            let limits = SourceCallLimits {
                max_lowered_nodes: nodes,
                max_lowered_depth: depth,
                ..LIMITS
            };
            for input in 0..3 {
                for policy in 0..6 {
                    let run = |frozen| {
                        let events = RefCell::new(vec![]);
                        let lower = |original: &Expression| {
                            events
                                .borrow_mut()
                                .push(format!("lower:{:?}", span(original)));
                            if policy == 1 {
                                return Err("child");
                            }
                            assert_ne!(policy, 4, "child unwind");
                            let value = match input {
                                0 => Data::Local { name: "row".into() },
                                1 => Data::Host {
                                    effect: Box::new(7),
                                },
                                _ => Data::Strings {
                                    items: vec![Data::Local { name: "row".into() }; 3],
                                },
                            };
                            Ok((value, 7_u8))
                        };
                        let decode = |name: &str| {
                            events.borrow_mut().push(format!("decode:{name}"));
                            if policy == 2 || name == "unknown" {
                                return Err("decode");
                            }
                            Ok(9_u8)
                        };
                        let export = |input: &u8, key| {
                            events.borrow_mut().push(format!("export:{input}:{key}"));
                            assert_ne!(policy, 5, "export unwind");
                            Ok(if policy == 3 {
                                None
                            } else {
                                Some((key, ScalarType::Integer))
                            })
                        };
                        let result = catch_unwind(AssertUnwindSafe(|| {
                            if frozen {
                                field_observation(
                                    frozen_field::field_source(arguments, *at).and_then(|source| {
                                        source.lower_preflighted(limits, lower, decode, export)
                                    }),
                                )
                            } else {
                                field_observation(
                                    bind_field_source(arguments, *at).and_then(|source| {
                                        source.lower(limits, lower, decode, export)
                                    }),
                                )
                            }
                        }))
                        .unwrap_or_else(|_| "unwind".into());
                        (result, events.into_inner())
                    };
                    assert_eq!(
                        run(false),
                        run(true),
                        "{text}, {nodes}, {depth}, {input}, {policy}"
                    );
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 864);
}
#[test]
fn public_host_plan_matches_frozen_root_prepare_and_admit_order_for_144_cases() {
    let mut cases = 0;
    for text in ["native()", "native(value: nested(x: 0))", "row", "7"] {
        let ast = expression(text);
        for (nodes, depth) in BUDGETS {
            let limits = SourceCallLimits {
                max_lowered_nodes: nodes,
                max_lowered_depth: depth,
                ..LIMITS
            };
            for policy in 0..6 {
                let run = |frozen| {
                    let events = RefCell::new(vec![]);
                    let prepare = |source: &Expression| {
                        assert!(std::ptr::eq(source, &ast));
                        events.borrow_mut().push("prepare");
                        if policy == 1 {
                            return Err("prepare");
                        }
                        assert_ne!(policy, 2, "prepare unwind");
                        Ok((7_u8, 9_u8))
                    };
                    let admit = |source: &Expression, effect: &u8, ty: &u8| {
                        assert!(std::ptr::eq(source, &ast));
                        assert_eq!((*effect, *ty), (7, 9));
                        events.borrow_mut().push("admit");
                        if policy == 3 {
                            return Err("admit");
                        }
                        assert_ne!(policy, 4, "admit unwind");
                        Ok(())
                    };
                    let result = catch_unwind(AssertUnwindSafe(|| {
                        if frozen {
                            host_observation(frozen_host::lower_preflighted_host_source(
                                &ast, limits, prepare, admit,
                            ))
                        } else {
                            host_observation(
                                bind_host_source_root(&ast, limits)
                                    .and_then(|plan| plan.lower(prepare, admit)),
                            )
                        }
                    }))
                    .unwrap_or_else(|_| "unwind".into());
                    (result, events.into_inner())
                };
                assert_eq!(run(false), run(true), "{text}, {nodes}, {depth}, {policy}");
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 144);
}
