//! Bounded source-call binding, not a parser, complete lowerer or execution authority.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    ArgumentTypeError, MAX_SCALAR_STRING_BYTES, NamedArgumentError, OperationCatalog,
    OperationCatalogError, OperationSchema, ScalarArgumentDomain, ScalarArgumentType, ScalarType,
    StructureBudget, StructureError, check_argument_type,
};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::{Computation, ComputedArgument};
use crate::pure_typing::{
    MAX_LOCAL_NAME_BYTES, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureTypeError,
    preflight_with_budget,
};

pub const MAX_SOURCE_CALL_ARGUMENTS: usize = 64;
pub const MAX_SOURCE_CALL_NAME_BYTES: usize = 128;

/// Separate inclusive budgets for the physical AST and produced IR. No source
/// expansion, lexical type inference, helper expansion or evaluation is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCallLimits {
    pub max_source_nodes: usize,
    pub max_source_depth: usize,
    pub max_lowered_nodes: usize,
    pub max_lowered_depth: usize,
    pub max_arguments: usize,
}

/// Current physical IR policy, not the earlier source budget or an authority
/// certificate. One budget covers the call root and the complete operand forest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCallFinishLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_arguments: usize,
}

/// Native mapping/admission failures are recoverable by matching, never exposed
/// through formatting or error source chains. No native trait bounds are needed.
pub enum SourceCallFinishError<Mapping, Admission> {
    InvalidLimits,
    ArgumentLimit,
    ParameterLimit,
    Root(StructureError),
    Output {
        argument_index: usize,
        error: PureTypeError,
    },
    Mapping(Mapping),
    Admission(Admission),
}

impl<Mapping, Admission> fmt::Display for SourceCallFinishError<Mapping, Admission> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "source call completion limits exceed safety ceilings",
            Self::ArgumentLimit => "source call completion has too many arguments",
            Self::ParameterLimit => "source call completion schema has too many parameters",
            Self::Root(_) => "source call completion root exceeds its limits",
            Self::Output { .. } => "source call completion operand shape is invalid",
            Self::Mapping(_) => "native source call operation mapping failed",
            Self::Admission(_) => "native source call admission failed",
        })
    }
}
impl<Mapping, Admission> fmt::Debug for SourceCallFinishError<Mapping, Admission> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Mapping, Admission> std::error::Error for SourceCallFinishError<Mapping, Admission> {}

pub type SourceCallFinishResult<
    Field,
    Operation,
    HostEffect,
    IrResult,
    Output,
    Mapping,
    Admission,
> = Result<
    (Computation<Field, Operation, HostEffect, IrResult>, Output),
    SourceCallFinishError<Mapping, Admission>,
>;

pub type SourceSchema<'schema, Key, Domain, Result, Capability> =
    OperationSchema<'schema, Key, &'schema str, Domain, Result, Capability>;

/// Trusted version/grants, not values accepted from source. Native key Borrow,
/// equality and domain methods may run code or unwind; hosts bound that work.
pub struct SourceCallHost<'host, 'schema, Key, Domain, Result, Capability> {
    pub catalog: &'host OperationCatalog<'schema, Key, &'schema str, Domain, Result, Capability>,
    pub version: u32,
    pub granted: &'host [Capability],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceShapeError {
    Structure(StructureError),
    ArgumentLimit,
    InvalidName,
    UnboundedText,
}

/// Native errors remain available by matching, never by formatting/source chains.
pub enum SourceCallError<Error> {
    NotCall,
    InvalidLimits,
    ArgumentLimit,
    ParameterLimit,
    Shape {
        span: Span,
        error: SourceShapeError,
    },
    Catalog(OperationCatalogError),
    Names(NamedArgumentError),
    OutputRoot(StructureError),
    Lowering {
        argument_index: usize,
        error: Error,
    },
    Output {
        argument_index: usize,
        error: PureTypeError,
    },
    Argument {
        parameter_index: usize,
        argument_index: usize,
        error: ArgumentTypeError,
    },
}

impl<Error> fmt::Display for SourceCallError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotCall => "source call lowering requires a call",
            Self::InvalidLimits => "source call limits exceed safety ceilings",
            Self::ArgumentLimit => "source call has too many arguments",
            Self::ParameterLimit => "source schema has too many parameters",
            Self::Shape { .. } => "source call shape is invalid",
            Self::Catalog(_) => "source call schema selection failed",
            Self::Names(_) => "source call named signature is invalid",
            Self::OutputRoot(_) => "lowered source call root exceeds its limits",
            Self::Lowering { .. } => "native source argument lowering failed",
            Self::Output { .. } => "lowered source argument shape is invalid",
            Self::Argument { .. } => "lowered source argument type or domain is invalid",
        })
    }
}
impl<Error> fmt::Debug for SourceCallError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for SourceCallError<Error> {}

/// Declaration order plus original submitted positions. Names borrow the AST;
/// values move without a second IR tree or native Clone/serde/Debug requirements.
pub struct LoweredSourceArgument<'source, Node> {
    pub name: &'source str,
    pub parameter_index: usize,
    pub argument_index: usize,
    pub value: Node,
}

/// Ephemeral source preparation, not a typed program or authentic receipt.
/// The original schema is borrowed. Native opcode mapping, complete cold typing,
/// canonicalization, live authority and actual value checks remain host-owned.
#[must_use = "consume the lowered arguments or deliberately discard them"]
pub struct LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node> {
    schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
    arguments: Vec<LoweredSourceArgument<'source, Node>>,
}
impl<'source, 'schema, Key, Domain, Result, Capability, Node>
    LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node>
{
    pub fn schema(&self) -> &'schema SourceSchema<'schema, Key, Domain, Result, Capability> {
        self.schema
    }

    pub fn arguments(&self) -> &[LoweredSourceArgument<'source, Node>] {
        &self.arguments
    }

    /// Low-level handoff, not final admission. Materialize bounded language names
    /// once, moving the exact native nodes.
    pub fn into_arguments(self) -> Vec<ComputedArgument<Node>> {
        self.arguments
            .into_iter()
            .map(|argument| ComputedArgument {
                name: argument.name.to_owned(),
                value: argument.value,
            })
            .collect()
    }
}

impl<'source, 'schema, Key, Domain, Result, Capability, Field, Operation, HostEffect, IrResult>
    LoweredSourceCall<
        'source,
        'schema,
        Key,
        Domain,
        Result,
        Capability,
        Computation<Field, Operation, HostEffect, IrResult>,
    >
{
    /// Consume the original prepared operands into one call. Current cold shape
    /// checks precede the once-only mapper; the core constructs the exact call
    /// before mandatory once-only native admission. Mapping cannot replace the
    /// operands. Admission must establish native row/result identity, full lexical
    /// typing, canonical form and current host policy, not trust cached facts.
    ///
    /// This does not invoke, charge/refund fuel, retry, roll back native work or
    /// issue an execution receipt. Native interior mutation/work/Drop are trusted;
    /// failures and unwinds release owned output without returning partial IR.
    pub fn finish_call<Output, Mapping, Admission>(
        self,
        limits: SourceCallFinishLimits,
        map: impl FnOnce(
            &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
        ) -> std::result::Result<(Operation, Output), Mapping>,
        admit: impl FnOnce(
            &Computation<Field, Operation, HostEffect, IrResult>,
            &Output,
            &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
        ) -> std::result::Result<(), Admission>,
    ) -> SourceCallFinishResult<Field, Operation, HostEffect, IrResult, Output, Mapping, Admission>
    {
        if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
            || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
            || limits.max_arguments > MAX_SOURCE_CALL_ARGUMENTS
        {
            return Err(SourceCallFinishError::InvalidLimits);
        }
        if self.schema.parameters.len() > limits.max_arguments {
            return Err(SourceCallFinishError::ParameterLimit);
        }
        if self.arguments.len() > limits.max_arguments {
            return Err(SourceCallFinishError::ArgumentLimit);
        }
        let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
        budget.visit(0, 0, 0).map_err(SourceCallFinishError::Root)?;
        budget
            .check_pending(0, self.arguments.len())
            .map_err(SourceCallFinishError::Root)?;
        for argument in &self.arguments {
            preflight_with_budget(&argument.value, 1, &mut budget).map_err(|error| {
                SourceCallFinishError::Output {
                    argument_index: argument.argument_index,
                    error,
                }
            })?;
        }
        let schema = self.schema;
        let (operation, output) = map(schema).map_err(SourceCallFinishError::Mapping)?;
        let call = Computation::Call {
            operation,
            arguments: self.into_arguments(),
        };
        admit(&call, &output, schema).map_err(SourceCallFinishError::Admission)?;
        Ok((call, output))
    }
}
impl<Key, Domain, Result, Capability, Node> fmt::Debug
    for LoweredSourceCall<'_, '_, Key, Domain, Result, Capability, Node>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoweredSourceCall")
            .field("arguments", &self.arguments.len())
            .finish()
    }
}

pub type SourceCallResult<'source, 'schema, Key, Domain, Result, Capability, Node, Error> =
    std::result::Result<
        LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node>,
        SourceCallError<Error>,
    >;

fn valid_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_LOCAL_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
pub(crate) fn span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::String { span, .. }
        | Expression::Integer { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::Reference { span, .. }
        | Expression::None { span } => *span,
    }
}
// Nonempty fixed signatures only. Borrow original operands in protocol order;
// malformed arguments or a duplicate signature never produce a partial view.
pub(crate) fn fixed_named_operands<'source, const N: usize>(
    arguments: &'source [NamedArgument],
    expected: &[&str; N],
) -> Option<[&'source Expression; N]> {
    if arguments.len() != N {
        return None;
    }
    let mut ordered = [&arguments.first()?.value; N];
    for (index, (slot, name)) in ordered.iter_mut().zip(expected).enumerate() {
        if expected.iter().take(index).any(|previous| previous == name) {
            return None;
        }
        let mut matches = arguments.iter().filter(|argument| argument.name == *name);
        *slot = &matches.next()?.value;
        if matches.next().is_some() {
            return None;
        }
    }
    Some(ordered)
}

pub(crate) fn valid_limits(limits: SourceCallLimits) -> bool {
    limits.max_source_nodes <= MAX_TYPE_INFERENCE_NODES
        && limits.max_lowered_nodes <= MAX_TYPE_INFERENCE_NODES
        && limits.max_source_depth <= MAX_TYPE_INFERENCE_DEPTH
        && limits.max_lowered_depth <= MAX_TYPE_INFERENCE_DEPTH
        && limits.max_arguments <= MAX_SOURCE_CALL_ARGUMENTS
}

// Whole physical source, including cold operands, before native metadata queries.
pub(crate) fn preflight<Error>(
    expression: &Expression,
    limits: SourceCallLimits,
) -> Result<(), SourceCallError<Error>> {
    let mut budget = StructureBudget::new(limits.max_source_nodes, limits.max_source_depth);
    let mut pending = vec![(expression, 0)];
    while let Some((node, depth)) = pending.pop() {
        let fail = |error| SourceCallError::Shape {
            span: span(node),
            error,
        };
        budget
            .visit(depth, 0, 0)
            .map_err(|error| fail(SourceShapeError::Structure(error)))?;
        match node {
            Expression::Call {
                callee, arguments, ..
            } => {
                if callee.len() > MAX_SOURCE_CALL_NAME_BYTES
                    || !callee.split('.').all(valid_identifier)
                {
                    return Err(fail(SourceShapeError::InvalidName));
                }
                if arguments.len() > limits.max_arguments {
                    return Err(fail(SourceShapeError::ArgumentLimit));
                }
                for argument in arguments {
                    if !valid_identifier(&argument.name) {
                        return Err(SourceCallError::Shape {
                            span: argument.span,
                            error: SourceShapeError::InvalidName,
                        });
                    }
                }
                budget
                    .check_pending(pending.len(), arguments.len())
                    .map_err(|error| fail(SourceShapeError::Structure(error)))?;
                pending.extend(
                    arguments
                        .iter()
                        .rev()
                        .map(|argument| (&argument.value, depth + 1)),
                );
            }
            Expression::Reference { name, .. } if !valid_identifier(name) => {
                return Err(fail(SourceShapeError::InvalidName));
            }
            Expression::String { value, .. } if value.len() > MAX_SCALAR_STRING_BYTES => {
                return Err(fail(SourceShapeError::UnboundedText));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Bind an atomic source call against the exact original native catalog row.
/// All physical source checks precede native lookup; exact version and grants,
/// selected signature and all names precede operand lowering/domain callbacks.
/// Physical AST bounds do not prove source/span authenticity; retain the parser
/// and validated SyntaxTree decoding boundary. Native generated-IR preflight
/// establishes physical purity once, not lexical binding or complete type safety.
/// The callback receives the original borrowed AST argument, in declaration order,
/// and returns a native IR node plus trusted inferred scalar type (not inference
/// performed here). Produced pure IR shares one physical budget across arguments.
/// No evaluation, helper expansion, dispatch, fuel, persistence or recovery occurs.
/// Native callback/equality/drop unwind propagates, releasing partial output; native
/// side effects and caller-owned type-scope/accounting are not rolled back.
pub fn lower_source_call<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Error,
>(
    expression: &'source Expression,
    host: &SourceCallHost<'_, 'schema, Key, Domain, Result, Capability>,
    limits: SourceCallLimits,
    lower: impl FnMut(
        &'source NamedArgument,
    ) -> std::result::Result<
        (
            Computation<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> SourceCallResult<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Computation<Field, Operation, HostEffect, IrResult>,
    Error,
>
where
    Key: Borrow<str>,
    Capability: PartialEq,
    Domain: ScalarArgumentDomain,
{
    if !valid_limits(limits) {
        return Err(SourceCallError::InvalidLimits);
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(SourceCallError::NotCall);
    };
    preflight(expression, limits)?;
    let schema = host
        .catalog
        .authorize(callee.as_str(), host.version, host.granted)
        .map_err(SourceCallError::Catalog)?;
    bind_source_arguments(arguments, schema, limits)?.lower(lower)
}

/// Immutable named alignment against one original host-selected schema.
/// This does not certify physical source, lexical typing, versions or grants.
/// Descendant source/native work is trusted adapter policy. No native metadata
/// trait bounds, payload formatting, cloning, serde codec or mutable view exist.
///
/// ```compile_fail
/// use leselang_hir::source_call::BoundSourceArguments;
/// let bound: BoundSourceArguments<'_, '_, (), (), (), ()> =
///     serde_json::from_str("{}").unwrap();
/// ```
#[must_use = "lower the bound operands or deliberately discard the alignment"]
pub struct BoundSourceArguments<'source, 'schema, Key, Domain, Result, Capability> {
    arguments: &'source [NamedArgument],
    schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
    bindings: Vec<(usize, usize)>,
    limits: SourceCallLimits,
}
impl<Key, Domain, Result, Capability> fmt::Debug
    for BoundSourceArguments<'_, '_, Key, Domain, Result, Capability>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BoundSourceArguments")
    }
}

/// Bound top-level argument/signature counts and bind all submitted names before
/// lowering. Optional omission creates no default value. Only immutable names
/// and indices are inspected; no domain callback or native lookup runs. The host
/// selects and authorizes the exact schema separately. For complete eager cold
/// source scanning and catalog selection, use `lower_source_call` instead.
/// Source node/depth fields validate ceilings here, not observed descendants.
pub fn bind_source_arguments<'source, 'schema, Key, Domain, Result, Capability, Error>(
    arguments: &'source [NamedArgument],
    schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
    limits: SourceCallLimits,
) -> std::result::Result<
    BoundSourceArguments<'source, 'schema, Key, Domain, Result, Capability>,
    SourceCallError<Error>,
> {
    if !valid_limits(limits) {
        return Err(SourceCallError::InvalidLimits);
    }
    if schema.parameters.len() > limits.max_arguments {
        return Err(SourceCallError::ParameterLimit);
    }
    if arguments.len() > limits.max_arguments || arguments.len() > schema.parameters.len() {
        return Err(SourceCallError::ArgumentLimit);
    }
    let mut names = [""; MAX_SOURCE_CALL_ARGUMENTS];
    for (name, argument) in names.iter_mut().zip(arguments) {
        *name = argument.name.as_str();
    }
    let bound = schema
        .bind_arguments(&names[..arguments.len()])
        .map_err(SourceCallError::Names)?;
    let bindings = bound
        .iter()
        .map(|(parameter, argument_index)| {
            schema
                .parameters
                .iter()
                .position(|candidate| std::ptr::eq(candidate, parameter))
                .map(|parameter_index| (parameter_index, argument_index))
                .ok_or(SourceCallError::Names(
                    NamedArgumentError::UnknownArgument {
                        index: argument_index,
                    },
                ))
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(BoundSourceArguments {
        arguments,
        schema,
        bindings,
        limits,
    })
}

impl<'source, 'schema, Key, Domain, Result, Capability>
    BoundSourceArguments<'source, 'schema, Key, Domain, Result, Capability>
where
    Domain: ScalarArgumentDomain,
{
    /// Consume the alignment once, keeping declaration order and original borrows.
    /// The output call root and operand forest share one meter; physical purity
    /// precedes current native scalar-domain queries. Native lowerers must bound
    /// all descendant source/work and corroborate lexical types before use.
    /// Failure/unwind releases partial output without retry or native rollback.
    /// Binding is not source admission or live execution authority.
    pub fn lower<Field, Operation, HostEffect, IrResult, Error>(
        self,
        mut lower: impl FnMut(
            &'source NamedArgument,
        ) -> std::result::Result<
            (
                Computation<Field, Operation, HostEffect, IrResult>,
                Option<ScalarType>,
            ),
            Error,
        >,
    ) -> SourceCallResult<
        'source,
        'schema,
        Key,
        Domain,
        Result,
        Capability,
        Computation<Field, Operation, HostEffect, IrResult>,
        Error,
    > {
        let Self {
            arguments,
            schema,
            bindings,
            limits,
        } = self;
        let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
        budget.visit(0, 0, 0).map_err(SourceCallError::OutputRoot)?;
        let mut lowered = Vec::with_capacity(arguments.len());
        for (parameter_index, argument_index) in bindings {
            let argument = &arguments[argument_index];
            let parameter = &schema.parameters[parameter_index];
            budget
                .check_pending(0, 1)
                .map_err(|error| SourceCallError::Output {
                    argument_index,
                    error: PureTypeError::Structure(error),
                })?;
            let (value, scalar_type) =
                lower(argument).map_err(|error| SourceCallError::Lowering {
                    argument_index,
                    error,
                })?;
            preflight_with_budget(&value, 1, &mut budget).map_err(|error| match error {
                PureTypeError::Impure => SourceCallError::Argument {
                    parameter_index,
                    argument_index,
                    error: ArgumentTypeError::Impure,
                },
                _ => SourceCallError::Output {
                    argument_index,
                    error,
                },
            })?;
            // Generated-IR preflight already proved physical purity; do not walk it again.
            let facts = ScalarArgumentType {
                scalar_type,
                is_pure: true,
                literal: match &value {
                    Computation::Literal { value } => Some(value),
                    _ => None,
                },
            };
            check_argument_type(&parameter.domain, facts).map_err(|error| {
                SourceCallError::Argument {
                    parameter_index,
                    argument_index,
                    error,
                }
            })?;
            lowered.push(LoweredSourceArgument {
                name: argument.name.as_str(),
                parameter_index,
                argument_index,
                value,
            });
        }
        Ok(LoweredSourceCall {
            schema,
            arguments: lowered,
        })
    }
}

#[cfg(test)]
mod fixed_named_tests {
    use super::fixed_named_operands;
    use leselang_syntax::{Expression, NamedArgument, parse};

    fn arguments(text: &str) -> Vec<NamedArgument> {
        let tree = parse(&format!("fn main() = {text}"));
        assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
        let Expression::Call { arguments, .. } = tree.function.unwrap().body else {
            panic!("expected a source call");
        };
        arguments
    }

    #[test]
    fn fixed_signature_views_keep_original_operands_in_all_submission_orders() {
        for order in [
            ["when", "then", "otherwise"],
            ["when", "otherwise", "then"],
            ["then", "when", "otherwise"],
            ["then", "otherwise", "when"],
            ["otherwise", "when", "then"],
            ["otherwise", "then", "when"],
        ] {
            let arguments = arguments(&format!(
                "choose({}: first, {}: second, {}: third)",
                order[0], order[1], order[2]
            ));
            let buffer = arguments.as_ptr();
            let expected = ["when", "then", "otherwise"];
            let operands = fixed_named_operands(&arguments, &expected).unwrap();
            for (operand, name) in operands.into_iter().zip(expected) {
                let original = &arguments.iter().find(|arg| arg.name == name).unwrap().value;
                assert!(std::ptr::eq(operand, original));
            }
            assert_eq!(arguments.as_ptr(), buffer);
            assert_eq!(
                arguments
                    .iter()
                    .map(|arg| arg.name.as_str())
                    .collect::<Vec<_>>(),
                order
            );
        }
        let unary = arguments("not(value: original)");
        assert!(std::ptr::eq(
            fixed_named_operands(&unary, &["value"]).unwrap()[0],
            &unary[0].value
        ));
        let binary = arguments("add(right: second, left: first)");
        let [left, right] = fixed_named_operands(&binary, &["left", "right"]).unwrap();
        assert!(std::ptr::eq(left, &binary[1].value));
        assert!(std::ptr::eq(right, &binary[0].value));
    }

    #[test]
    fn fixed_signature_views_reject_missing_extra_duplicate_and_unknown_names() {
        for text in [
            "choose()",
            "choose(when: true, then: first)",
            "choose(when: true, then: first, otherwise: second, extra: third)",
            "choose(when: true, then: first, then: second)",
            "choose(when: true, otherwise: first, otherwise: second)",
            "choose(when: true, then: first, unknown: second)",
            "choose(When: true, then: first, otherwise: second)",
        ] {
            assert!(
                fixed_named_operands(&arguments(text), &["when", "then", "otherwise"]).is_none()
            );
        }
        for text in [
            "field(value: first, value: second)",
            "field(value: first, other: second)",
        ] {
            assert!(fixed_named_operands(&arguments(text), &["value", "name"]).is_none());
        }
        assert!(fixed_named_operands(&arguments("not(other: first)"), &["value"]).is_none());
    }

    #[test]
    fn fixed_signature_views_fail_closed_for_empty_or_duplicate_protocols() {
        assert!(fixed_named_operands(&[], &[]).is_none());
        let arguments = arguments("call(first: 1, second: 2)");
        assert!(fixed_named_operands(&arguments, &["first", "first"]).is_none());
        assert!(fixed_named_operands(&arguments, &["absent", "absent"]).is_none());
    }
}
