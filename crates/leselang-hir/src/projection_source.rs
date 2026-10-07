//! Native projection source construction, not result acceptance or effect authority.

use std::fmt;

use leselang_runtime_core::{ScalarType, StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{
    PureType, PureTypeError, preflight_with_budget, valid_local_name, valid_member_name,
};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type NativeFieldInput<Node, Input, Error> = Result<(Node, Input), Error>;
type NativeFieldExport<Field, Error> = Result<Option<(Field, ScalarType)>, Error>;
type FieldSourceOutput<Node, Error> = Result<(Node, ScalarType), ProjectionSourceError<Error>>;

pub type ProjectionSourceResult<Node, Result, Error> =
    std::result::Result<(Node, PureType<Result>), ProjectionSourceError<Error>>;

pub type NativeProjectionInputResult<Node, Result, Error> =
    std::result::Result<(Node, PureType<Result>), Error>;

/// Trusted native construction/type observations, not actual result values.
///
/// Callbacks receive original borrowed AST/name metadata. Lowering must bound
/// hidden expansion/work and use the exact caller lexical prefix. Field/member
/// queries must reject exports absent from that exact result/group, not a union
/// of unrelated schemas. They must not dispatch effects or create receipts.
/// No native slot, result observation or error needs Clone/Debug/serde/Send.
/// Complete cold IR type inference is still mandatory before execution.
pub trait ProjectionSourceEnvironment<'source, Field, Operation, HostEffect, IrResult> {
    type Result;
    type Error;

    fn lower_value(
        &mut self,
        expression: &'source Expression,
    ) -> NativeProjectionInputResult<
        Node<Field, Operation, HostEffect, IrResult>,
        Self::Result,
        Self::Error,
    >;

    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error>;

    fn member(
        &mut self,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error>;
}

pub enum ProjectionSourceError<Error> {
    Source(SourceCallError<Error>),
    NotProjection { span: Span },
    Names { span: Span },
    FieldName { span: Span },
    MemberShape { span: Span },
    MemberNames { span: Span },
    NonResult { span: Span },
    FieldNotExported { span: Span },
    MemberNotExported { span: Span },
    Native { span: Span, error: Error },
    Generation { span: Span, error: StructureError },
    Produced { span: Span, error: PureTypeError },
}

impl<Error> fmt::Display for ProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "projection source shape is invalid",
            Self::NotProjection { .. } => "projection source requires field or member",
            Self::Names { .. } => "projection requires exactly value and name operands",
            Self::FieldName { .. } => "field name must be a string literal",
            Self::MemberShape { .. } => "member requires a group reference and literal name",
            Self::MemberNames { .. } => "member source names are invalid",
            Self::NonResult { .. } => "field input must observe a native result type",
            Self::FieldNotExported { .. } => "field is not exported by this native result",
            Self::MemberNotExported { .. } => "member is not exported by this bound group",
            Self::Native { .. } => "native projection source preparation failed",
            Self::Generation { .. } => "projection source generation exceeds its limits",
            Self::Produced { .. } => "native projection input IR is invalid",
        })
    }
}
impl<Error> fmt::Debug for ProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ProjectionSourceError<Error> {}

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

    // The reference frontend owns cold source/declaration checks and recursive
    // accounting. Keep child errors before literal/closed-name decoding here;
    // decoding is metadata mapping, never a native export or value query.
    pub(crate) fn lower_preflighted<Field, Operation, HostEffect, IrResult, Input, Key, Error>(
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
        let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
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
        let (value, input) = lower(self.value).map_err(|error| ProjectionSourceError::Native {
            span: span(self.value),
            error,
        })?;
        let at = self.at;
        self.finish_with_budget(
            value,
            input,
            &mut budget,
            |name| decode(name).map_err(|error| ProjectionSourceError::Native { span: at, error }),
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
    if arguments.len() != 2
        || ["value", "name"].iter().any(|name| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(ProjectionSourceError::Names { span: at });
    }
    let get = |name: &str| {
        arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value)
            .ok_or(ProjectionSourceError::Names { span: at })
    };
    Ok(FieldSource {
        value: get("value")?,
        name: get("name")?,
        at,
    })
}

pub(crate) struct MemberSource<'source> {
    pub group: &'source str,
    pub name: &'source str,
}
impl MemberSource<'_> {
    fn check_names<Error>(&self, at: Span) -> Result<(), ProjectionSourceError<Error>> {
        if !valid_local_name(self.group) || !valid_member_name(self.name) {
            return Err(ProjectionSourceError::MemberNames { span: at });
        }
        Ok(())
    }

    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        operation: Operation,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Member {
            group: self.group.to_owned(),
            name: self.name.to_owned(),
            operation,
        }
    }
}

pub(crate) fn member_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<MemberSource<'_>, ProjectionSourceError<Error>> {
    let source = field_source(arguments, at)?;
    let (Expression::Reference { name: group, .. }, Expression::String { value: name, .. }) =
        (source.value, source.name)
    else {
        return Err(ProjectionSourceError::MemberShape { span: at });
    };
    Ok(MemberSource { group, name })
}

pub(crate) fn preflight_names<Error>(
    expression: &Expression,
) -> Result<(), ProjectionSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = expression
        {
            if callee == "field" {
                field_source(arguments, span(expression))?.literal_name()?;
            } else if callee == "member" {
                member_source(arguments, span(expression))?.check_names(span(expression))?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

/// Construct one native field/member projection on the original generic IR.
///
/// Physical source and minimum generated capacity precede native work. All
/// cold projection signatures/literal metadata inside the input source precede
/// every callback. Field value lowering receives the original AST, then its
/// entire pure generated IR is checked before the native field export query.
/// Member metadata is a bound group reference plus a literal step name: it does
/// not evaluate either operand, synthesize a Local node or grant a group export.
/// Name literals are source metadata, not generated IR nodes. Field roots and
/// all native input nodes share one generated budget; no folding refund occurs.
///
/// Native observations are not type certificates. Complete cold inference,
/// actual result acceptance, live authority and value-domain validation remain
/// adapter-owned. Hidden native expansion/allocation/Drop cannot be preempted.
/// Errors/unwind drop partial native IR without rollback, retries or dispatch.
pub fn lower_projection_source<'source, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    environment: &mut Environment,
) -> ProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: ProjectionSourceEnvironment<'source, Field, Operation, HostEffect, IrResult>,
{
    if !valid_limits(limits) {
        return Err(ProjectionSourceError::Source(
            SourceCallError::InvalidLimits,
        ));
    }
    preflight(expression, limits).map_err(ProjectionSourceError::Source)?;
    preflight_names(expression)?;
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ProjectionSourceError::NotProjection { span: at });
    };
    if !matches!(callee.as_str(), "field" | "member") {
        return Err(ProjectionSourceError::NotProjection { span: at });
    }
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    budget
        .visit(0, 0, usize::from(callee == "field"))
        .map_err(|error| ProjectionSourceError::Generation { span: at, error })?;
    if callee == "member" {
        let source = member_source(arguments, at)?;
        let (operation, result) = environment
            .member(source.group, source.name)
            .map_err(|error| ProjectionSourceError::Native { span: at, error })?
            .ok_or(ProjectionSourceError::MemberNotExported { span: at })?;
        return Ok((source.construct(operation), PureType::Result(result)));
    }
    budget
        .check_pending(0, 1)
        .map_err(|error| ProjectionSourceError::Generation { span: at, error })?;
    let source = field_source(arguments, at)?;
    let input_at = span(source.value);
    let (value, input_type) =
        environment
            .lower_value(source.value)
            .map_err(|error| ProjectionSourceError::Native {
                span: input_at,
                error,
            })?;
    let (value, ty) =
        source.finish_with_budget(value, input_type, &mut budget, Ok, |input_type, name| {
            let PureType::Result(result) = input_type else {
                return Err(ProjectionSourceError::NonResult { span: input_at });
            };
            environment
                .field(result, name)
                .map_err(|error| ProjectionSourceError::Native { span: at, error })
        })?;
    Ok((value, PureType::Scalar(ty)))
}

#[cfg(test)]
mod field_finish_tests {
    use std::cell::{Cell, RefCell};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::rc::Rc;

    use leselang_runtime_core::ScalarValue;
    use leselang_syntax::parse;

    use super::*;

    // Deliberately move-only, non-Debug and non-Send native metadata.
    struct Native(Rc<()>, Rc<Cell<usize>>);
    impl Drop for Native {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }
    struct PrivateError(&'static str);
    type TestNode = Computation<Native, (), Native, ()>;

    fn limits() -> SourceCallLimits {
        SourceCallLimits {
            max_source_nodes: 3,
            max_source_depth: 1,
            max_lowered_nodes: 3,
            max_lowered_depth: 2,
            max_arguments: 2,
        }
    }

    fn source(expression: &Expression) -> FieldSource<'_> {
        let Expression::Call { arguments, .. } = expression else {
            panic!()
        };
        field_source::<PrivateError>(arguments, span(expression)).unwrap()
    }

    #[test]
    fn exact_original_ast_name_input_buffer_and_move_only_slots_pass_once() {
        let ast = parse("fn main() = field(value: row, name: \"native\")");
        let source = source(&ast.function.as_ref().unwrap().body);
        let original = source.value;
        let name = source.literal_name::<PrivateError>().unwrap();
        let owner = Rc::new(());
        let drops = Rc::new(Cell::new(0));
        let input = Native(owner.clone(), drops.clone());
        let field = Native(owner.clone(), drops.clone());
        let local = String::from("row");
        let buffer = local.as_ptr();
        let events = RefCell::new(Vec::new());
        let (node, ty) = source
            .lower_preflighted(
                limits(),
                |expression| {
                    events.borrow_mut().push("lower");
                    assert!(std::ptr::eq(expression, original));
                    Ok((TestNode::Local { name: local }, input))
                },
                |selected| {
                    events.borrow_mut().push("decode");
                    assert!(std::ptr::eq(selected, name));
                    Ok::<_, PrivateError>(field)
                },
                |input, field| {
                    events.borrow_mut().push("export");
                    assert!(Rc::ptr_eq(&input.0, &owner));
                    assert!(Rc::ptr_eq(&field.0, &owner));
                    Ok(Some((field, ScalarType::Integer)))
                },
            )
            .unwrap();
        assert_eq!(*events.borrow(), ["lower", "decode", "export"]);
        assert_eq!(ty, ScalarType::Integer);
        assert_eq!(drops.get(), 1);
        let TestNode::Field { value, field } = &node else {
            panic!()
        };
        let TestNode::Local { name } = value.as_ref() else {
            panic!()
        };
        assert_eq!(name.as_ptr(), buffer);
        assert!(Rc::ptr_eq(&field.0, &owner));
        drop(node);
        assert_eq!(drops.get(), 2);
    }

    #[test]
    fn minimum_root_and_child_capacity_precede_native_lowering() {
        let ast = parse("fn main() = field(value: row, name: \"native\")");
        for (nodes, depth) in [(0, 2), (1, 2), (3, 0)] {
            let limits = SourceCallLimits {
                max_lowered_nodes: nodes,
                max_lowered_depth: depth,
                ..limits()
            };
            let outcome = source(&ast.function.as_ref().unwrap().body).lower_preflighted(
                limits,
                |_| -> Result<(TestNode, ()), PrivateError> { panic!("no capacity") },
                |_| -> Result<(), PrivateError> { panic!("no decode") },
                |_, _| -> NativeFieldExport<Native, PrivateError> { panic!("no export") },
            );
            assert!(matches!(
                outcome,
                Err(ProjectionSourceError::Generation { .. })
            ));
        }
    }

    #[test]
    fn native_child_failure_precedes_nonliteral_metadata_and_is_redacted() {
        let ast = parse("fn main() = field(value: missing, name: 1)");
        let input_at = span(source(&ast.function.as_ref().unwrap().body).value);
        let error = source(&ast.function.as_ref().unwrap().body)
            .lower_preflighted(
                limits(),
                |_| -> Result<(TestNode, ()), PrivateError> { Err(PrivateError("secret child")) },
                |_| -> Result<(), PrivateError> { panic!("child failed") },
                |_, _| -> NativeFieldExport<Native, PrivateError> { panic!("child failed") },
            )
            .err()
            .unwrap();
        assert!(!format!("{error:?}").contains("secret"));
        assert!(std::error::Error::source(&error).is_none());
        let ProjectionSourceError::Native { span, error } = error else {
            panic!()
        };
        assert_eq!(span, input_at);
        assert_eq!(error.0, "secret child");
    }

    #[test]
    fn closed_name_failure_precedes_impure_input_without_an_export_query() {
        let ast = parse("fn main() = field(value: row, name: \"unknown\")");
        let drops = Rc::new(Cell::new(0));
        let outcome = source(&ast.function.as_ref().unwrap().body).lower_preflighted(
            limits(),
            |_| {
                Ok((
                    TestNode::Host {
                        effect: Box::new(Native(Rc::new(()), drops.clone())),
                    },
                    (),
                ))
            },
            |_| -> Result<(), PrivateError> { Err(PrivateError("unknown name")) },
            |_, _| -> NativeFieldExport<Native, PrivateError> { panic!("unknown name") },
        );
        let Err(ProjectionSourceError::Native { error, .. }) = outcome else {
            panic!()
        };
        assert_eq!(error.0, "unknown name");
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn entire_input_purity_and_one_root_budget_precede_native_export() {
        let ast = parse("fn main() = field(value: row, name: \"native\")");
        for impure in [false, true] {
            let drops = Rc::new(Cell::new(0));
            let owner = Rc::new(());
            let node = if impure {
                TestNode::Host {
                    effect: Box::new(Native(owner.clone(), drops.clone())),
                }
            } else {
                TestNode::Strings {
                    items: vec![TestNode::Literal {
                        value: ScalarValue::String("one".into()),
                    }],
                }
            };
            let error = source(&ast.function.as_ref().unwrap().body)
                .lower_preflighted(
                    SourceCallLimits {
                        max_lowered_nodes: 2,
                        ..limits()
                    },
                    |_| Ok((node, Native(owner.clone(), drops.clone()))),
                    |_| Ok(Native(owner.clone(), drops.clone())),
                    |_, _| -> NativeFieldExport<Native, PrivateError> {
                        panic!("invalid complete input")
                    },
                )
                .err()
                .unwrap();
            assert!(matches!(error, ProjectionSourceError::Produced { .. }));
            assert_eq!(drops.get(), if impure { 3 } else { 2 });
        }
    }

    #[test]
    fn export_error_or_unwind_drops_original_input_and_key_without_retry() {
        let ast = parse("fn main() = field(value: row, name: \"native\")");
        for unwind in [false, true] {
            let drops = Rc::new(Cell::new(0));
            let queries = Cell::new(0);
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                source(&ast.function.as_ref().unwrap().body).lower_preflighted(
                    limits(),
                    |_| {
                        Ok((
                            TestNode::Local { name: "row".into() },
                            Native(Rc::new(()), drops.clone()),
                        ))
                    },
                    |_| Ok(Native(Rc::new(()), drops.clone())),
                    |_, _| -> NativeFieldExport<Native, PrivateError> {
                        queries.set(queries.get() + 1);
                        if unwind {
                            panic!("native query unwind")
                        }
                        Err(PrivateError("private query error"))
                    },
                )
            }));
            assert_eq!(outcome.is_err(), unwind);
            if let Ok(error) = outcome {
                assert!(matches!(error, Err(ProjectionSourceError::Native { .. })));
            }
            assert_eq!(queries.get(), 1);
            assert_eq!(drops.get(), 2);
        }
    }
}
