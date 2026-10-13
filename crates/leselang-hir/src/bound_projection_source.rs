//! Projection of explicit bound native type observations, never name-only exports.

use std::fmt;

use leselang_runtime_core::{ScalarType, StructureBudget};
use leselang_syntax::{Expression, Span};

use crate::ir::Computation;
use crate::projection_source::{
    ProjectionSourceError, field_source, member_source, preflight_names,
};
use crate::pure_typing::{MAX_TYPE_INFERENCE_BINDINGS, PureType, PureTypeError, valid_local_name};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundProjectionSourceLimits {
    pub source: SourceCallLimits,
    pub max_bindings: usize,
}

/// Explicit export queries on the exact borrowed native binding observation.
/// Result metadata is not a received value, schema grant or group certificate.
/// Queries must corroborate original native schema/owner/version and exact closed
/// field/member exports; member must reject non-groups and foreign/look-alike
/// observations. Native returned slots/observations move, with no Clone/PartialEq/
/// Debug/serde/Send bounds. No lowering, result copying or dispatch occurs here.
pub trait BoundProjectionSourceEnvironment<'source, Field, Operation> {
    type Result;
    type Error;

    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error>;
    fn member(
        &mut self,
        result: &Self::Result,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error>;
}

pub enum BoundProjectionSourceError<Error> {
    Projection(ProjectionSourceError<Error>),
    Scope { span: Span, error: PureTypeError },
    BoundReference { span: Span },
    Unbound { span: Span },
}
impl<Error> fmt::Display for BoundProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => fmt::Display::fmt(error, formatter),
            Self::Scope { .. } => formatter.write_str("bound projection type scope is invalid"),
            Self::BoundReference { .. } => {
                formatter.write_str("bound projection requires a direct bound reference")
            }
            Self::Unbound { .. } => {
                formatter.write_str("bound projection input is absent from the type scope")
            }
        }
    }
}
impl<Error> fmt::Debug for BoundProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for BoundProjectionSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type BoundProjectionSourceResult<Node, Type, Error> =
    Result<(Node, PureType<Type>), BoundProjectionSourceError<Error>>;

/// Construct field/member IR directly from an explicit read-only native type
/// prefix, including `ControlSourceScope::bindings()`. The prefix is borrowed,
/// bounded and unique: no type clone, mutable host frame or lower-value callback.
/// `field` accepts a direct result reference and builds one Local plus Field;
/// `member` accepts a bound group reference and builds only Member, not a fake
/// Local or implicit export union. Other pure inputs use the existing projection
/// entry with an explicit child compiler. Bound scalar inputs never query native
/// exports. Missing names remain missing rather than consulting a global registry.
///
/// Whole cold source/projection metadata, prefix names/counts and minimum output
/// depth/nodes precede native queries. Fixed ceilings: 16,384 nodes, depth 64,
/// 64 operands, 1,024 bindings. Name literals are source metadata, not IR nodes.
/// Original native observations/names are borrowed, returned fields/operations
/// and member observations move unchanged. Queries run once; errors/unwind stop
/// without partial output, retry, dispatch or native side-effect rollback.
///
/// This is type-directed construction, not full lexical/type inference, native
/// value projection, actual result acceptance or an execution grant. Whole cold
/// native IR admission, source/span authenticity, hidden expansion/work, ingress,
/// live version/grants, mutation and resource policy remain adapter-owned. Native
/// work/allocation/Drop cannot be preempted; a second cleanup panic can abort.
pub fn lower_bound_projection_source<'source, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: BoundProjectionSourceLimits,
    environment: &mut Environment,
) -> BoundProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: BoundProjectionSourceEnvironment<'source, Field, Operation>,
{
    lower_projection(expression, prefix, limits, environment, false)
}

/// Catalog composition also accepts one field over a direct bound member.
/// This is not recursive expression lowering or an expansion of the public
/// direct-reference entry. Both forms produce exactly two nodes for a field.
pub(crate) fn lower_catalog_projection_source<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Environment,
>(
    expression: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: BoundProjectionSourceLimits,
    environment: &mut Environment,
) -> BoundProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: BoundProjectionSourceEnvironment<'source, Field, Operation>,
{
    lower_projection(expression, prefix, limits, environment, true)
}

fn lower_projection<'source, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: BoundProjectionSourceLimits,
    environment: &mut Environment,
    allow_member_field: bool,
) -> BoundProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: BoundProjectionSourceEnvironment<'source, Field, Operation>,
{
    let projection_error = BoundProjectionSourceError::Projection;
    if !valid_limits(limits.source) || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS {
        return Err(projection_error(ProjectionSourceError::Source(
            SourceCallError::InvalidLimits,
        )));
    }
    preflight(expression, limits.source)
        .map_err(ProjectionSourceError::Source)
        .map_err(projection_error)?;
    preflight_names(expression).map_err(projection_error)?;
    let at = span(expression);
    if prefix.len() > limits.max_bindings
        || prefix.iter().enumerate().any(|(index, (name, _))| {
            !valid_local_name(name) || prefix[..index].iter().any(|(old, _)| old == name)
        })
    {
        return Err(BoundProjectionSourceError::Scope {
            span: at,
            error: PureTypeError::InvalidScope,
        });
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(projection_error(ProjectionSourceError::NotProjection {
            span: at,
        }));
    };
    if !matches!(callee.as_str(), "field" | "member") {
        return Err(projection_error(ProjectionSourceError::NotProjection {
            span: at,
        }));
    }
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    budget
        .visit(0, 0, usize::from(callee == "field"))
        .map_err(|error| projection_error(ProjectionSourceError::Generation { span: at, error }))?;
    let form = field_source(arguments, at).map_err(projection_error)?;
    let name = form.literal_name().map_err(projection_error)?;
    let input_at = span(form.value);
    let member = match form.value {
        Expression::Call {
            callee: child,
            arguments,
            ..
        } if allow_member_field && callee == "field" && child == "member" => {
            Some(member_source(arguments, input_at).map_err(projection_error)?)
        }
        _ => None,
    };
    let bound = match (form.value, &member) {
        (Expression::Reference { name, .. }, _) => name.as_str(),
        (_, Some(member)) => member.group,
        _ => return Err(BoundProjectionSourceError::BoundReference { span: input_at }),
    };
    let observed = prefix
        .iter()
        .find(|(old, _)| *old == bound)
        .map(|(_, ty)| *ty)
        .ok_or(BoundProjectionSourceError::Unbound { span: input_at })?;
    let PureType::Result(result) = observed else {
        return Err(projection_error(ProjectionSourceError::NonResult {
            span: input_at,
        }));
    };
    let native_error = |error| projection_error(ProjectionSourceError::Native { span: at, error });
    if callee == "member" {
        let member = member_source(arguments, at).map_err(projection_error)?;
        let (operation, result) = environment
            .member(result, member.group, member.name)
            .map_err(native_error)?
            .ok_or(projection_error(ProjectionSourceError::MemberNotExported {
                span: at,
            }))?;
        return Ok((member.construct(operation), PureType::Result(result)));
    }
    budget
        .visit(1, 0, 0)
        .map_err(|error| projection_error(ProjectionSourceError::Generation { span: at, error }))?;
    if let Some(member) = member {
        let (operation, observed) = environment
            .member(result, member.group, member.name)
            .map_err(|error| {
                projection_error(ProjectionSourceError::Native {
                    span: input_at,
                    error,
                })
            })?
            .ok_or(projection_error(ProjectionSourceError::MemberNotExported {
                span: input_at,
            }))?;
        let (field, ty) = environment
            .field(&observed, name)
            .map_err(native_error)?
            .ok_or(projection_error(ProjectionSourceError::FieldNotExported {
                span: at,
            }))?;
        return Ok((
            form.construct(member.construct(operation), field),
            PureType::Scalar(ty),
        ));
    }
    let (field, ty) = environment
        .field(result, name)
        .map_err(native_error)?
        .ok_or(projection_error(ProjectionSourceError::FieldNotExported {
            span: at,
        }))?;
    Ok((
        form.construct(
            Node::Local {
                name: bound.to_owned(),
            },
            field,
        ),
        PureType::Scalar(ty),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    struct Observation {
        owner: Rc<()>,
        group: bool,
    }
    struct Slot(Box<str>);
    struct PrivateError(Box<str>);
    struct Host {
        owner: Rc<()>,
        visits: Vec<&'static str>,
        fail: Option<&'static str>,
        unwind: Option<&'static str>,
        error: Option<PrivateError>,
    }
    impl Host {
        fn step(&mut self, stage: &'static str) -> Result<(), PrivateError> {
            self.visits.push(stage);
            assert_ne!(self.unwind, Some(stage), "native projection unwind");
            if self.fail == Some(stage) {
                Err(self.error.take().unwrap())
            } else {
                Ok(())
            }
        }
    }
    impl<'s> BoundProjectionSourceEnvironment<'s, Slot, Slot> for Host {
        type Result = Observation;
        type Error = PrivateError;
        fn member(
            &mut self,
            result: &Observation,
            group: &'s str,
            name: &'s str,
        ) -> Result<Option<(Slot, Observation)>, PrivateError> {
            self.step("member")?;
            Ok((Rc::ptr_eq(&result.owner, &self.owner)
                && result.group
                && group == "batch"
                && name == "right")
                .then(|| {
                    (
                        Slot("selected-operation".into()),
                        Observation {
                            owner: Rc::clone(&self.owner),
                            group: false,
                        },
                    )
                }))
        }
        fn field(
            &mut self,
            result: &Observation,
            name: &'s str,
        ) -> Result<Option<(Slot, ScalarType)>, PrivateError> {
            self.step("field")?;
            Ok(
                (Rc::ptr_eq(&result.owner, &self.owner) && !result.group && name == "text")
                    .then(|| (Slot("selected-field".into()), ScalarType::String)),
            )
        }
    }
    type TestNode = Node<Slot, Slot, (), ()>;
    const LIMITS: BoundProjectionSourceLimits = BoundProjectionSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 64,
            max_source_depth: 8,
            max_lowered_nodes: 2,
            max_lowered_depth: 1,
            max_arguments: 8,
        },
        max_bindings: 1,
    };
    fn source(text: &str) -> Expression {
        let parsed = leselang_syntax::parse(&format!("fn main() = {text}"));
        assert!(parsed.diagnostics.is_empty());
        parsed.function.unwrap().body
    }
    fn fixture() -> (Host, PureType<Observation>) {
        let owner = Rc::new(());
        let observed = PureType::Result(Observation {
            owner: Rc::clone(&owner),
            group: true,
        });
        (
            Host {
                owner,
                visits: vec![],
                fail: None,
                unwind: None,
                error: None,
            },
            observed,
        )
    }
    const SOURCE: &str = r#"field(value: member(value: batch, name: "right"), name: "text")"#;

    #[test]
    fn catalog_member_field_composition_keeps_direct_entry_closed_and_moves_native_slots() {
        let input = source(SOURCE);
        let (mut host, observed) = fixture();
        let prefix = [("batch", &observed)];
        let direct: BoundProjectionSourceResult<TestNode, _, _> =
            lower_bound_projection_source(&input, &prefix, LIMITS, &mut host);
        assert!(matches!(
            direct,
            Err(BoundProjectionSourceError::BoundReference { .. })
        ));
        assert!(host.visits.is_empty());
        let (node, ty): (TestNode, _) =
            lower_catalog_projection_source(&input, &prefix, LIMITS, &mut host).unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::String)));
        let Node::Field { value, field } = node else {
            panic!()
        };
        let Node::Member {
            group,
            name,
            operation,
        } = *value
        else {
            panic!()
        };
        assert_eq!((group.as_str(), name.as_str()), ("batch", "right"));
        assert_eq!(&*operation.0, "selected-operation");
        assert_eq!(&*field.0, "selected-field");
        assert_eq!(host.visits, ["member", "field"]);
        assert_eq!(
            Rc::strong_count(&host.owner),
            2,
            "temporary member observation must be released"
        );
    }

    #[test]
    fn catalog_member_field_bounds_and_cold_metadata_precede_native_export_queries() {
        let (mut host, observed) = fixture();
        for text in [
            r#"field(value: member(value: batch, name: false), name: "text")"#,
            r#"field(value: member(value: batch, name: "right"), name: false)"#,
            r#"field(value: member(value: missing, name: "right"), name: "text")"#,
            r#"field(value: foreign.call(), name: "text")"#,
            r#"field(value: choose(when: true, then: batch, otherwise: batch), name: "text")"#,
        ] {
            let input = source(text);
            let result: BoundProjectionSourceResult<TestNode, _, _> =
                lower_catalog_projection_source(&input, &[("batch", &observed)], LIMITS, &mut host);
            assert!(result.is_err());
            assert!(host.visits.is_empty());
        }
        let input = source(SOURCE);
        for limits in [
            BoundProjectionSourceLimits {
                source: SourceCallLimits {
                    max_lowered_nodes: 1,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            BoundProjectionSourceLimits {
                source: SourceCallLimits {
                    max_lowered_depth: 0,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            BoundProjectionSourceLimits {
                source: SourceCallLimits {
                    max_source_nodes: 1,
                    ..LIMITS.source
                },
                ..LIMITS
            },
            BoundProjectionSourceLimits {
                max_bindings: 0,
                ..LIMITS
            },
        ] {
            let result: BoundProjectionSourceResult<TestNode, _, _> =
                lower_catalog_projection_source(&input, &[("batch", &observed)], limits, &mut host);
            assert!(result.is_err());
            assert!(host.visits.is_empty());
        }
    }

    #[test]
    fn catalog_member_field_refusal_and_unwind_stop_without_copying_native_errors() {
        for stage in ["member", "field"] {
            for unwind in [false, true] {
                let input = source(SOURCE);
                let (mut host, observed) = fixture();
                let private: Box<str> = "private native refusal".into();
                let pointer = private.as_ptr();
                host.error = Some(PrivateError(private));
                if unwind {
                    host.unwind = Some(stage);
                } else {
                    host.fail = Some(stage);
                }
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    let result: BoundProjectionSourceResult<TestNode, _, _> =
                        lower_catalog_projection_source(
                            &input,
                            &[("batch", &observed)],
                            LIMITS,
                            &mut host,
                        );
                    result
                }));
                if unwind {
                    assert!(outcome.is_err());
                } else {
                    let Err(error) = outcome.unwrap() else {
                        panic!()
                    };
                    assert!(!error.to_string().contains("private"));
                    let BoundProjectionSourceError::Projection(ProjectionSourceError::Native {
                        error,
                        ..
                    }) = error
                    else {
                        panic!()
                    };
                    assert_eq!(error.0.as_ptr(), pointer);
                }
                assert_eq!(
                    host.visits,
                    if stage == "member" {
                        vec!["member"]
                    } else {
                        vec!["member", "field"]
                    }
                );
                assert_eq!(Rc::strong_count(&host.owner), 2);
            }
        }
        for (from, to, expected) in [
            ("right", "missing", vec!["member"]),
            ("text", "missing", vec!["member", "field"]),
        ] {
            let input = source(&SOURCE.replace(from, to));
            let (mut host, observed) = fixture();
            let result: BoundProjectionSourceResult<TestNode, _, _> =
                lower_catalog_projection_source(&input, &[("batch", &observed)], LIMITS, &mut host);
            assert!(result.is_err());
            assert_eq!(host.visits, expected);
        }
    }
}
