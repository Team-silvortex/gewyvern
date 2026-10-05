//! Shared scalar/control source construction with explicit native extension boundaries.

use std::{collections::HashSet, fmt};

use leselang_runtime_core::{
    BinaryOperator, MAX_LOOP_ITERATIONS, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarType,
    ScalarValue, ScopeFrame, StringListBuilder, StructureBudget, StructureError, UnaryOperator,
};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, PureTypeError, preflight_with_budget, valid_local_name,
};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

mod fold;
pub(crate) use fold::{fold_items, fold_next, fold_source};

pub enum ScalarSourceError<Error> {
    Source(SourceCallError<Error>),
    Names {
        span: Span,
        expected: &'static [&'static str],
    },
    LiteralLimit {
        span: Span,
    },
    NonScalar {
        span: Span,
    },
    Binary {
        span: Span,
        operator: BinaryOperator,
        left: ScalarType,
        right: ScalarType,
    },
    Unary {
        span: Span,
        operator: UnaryOperator,
    },
    InconsistentLiteral {
        span: Span,
    },
    Condition {
        span: Span,
    },
    BranchTypes {
        span: Span,
    },
    RecoveryTypes {
        span: Span,
    },
    StringCount {
        span: Span,
    },
    StringLabels {
        span: Span,
    },
    StringItem {
        span: Span,
    },
    StringBytes {
        span: Span,
    },
    BindingShape {
        span: Span,
    },
    Bindings {
        span: Span,
        error: PureTypeError,
    },
    LoopShape {
        span: Span,
        missing_state: bool,
    },
    LoopLimit {
        span: Span,
        exceeds_bound: bool,
    },
    LoopCondition {
        span: Span,
    },
    LoopState {
        span: Span,
    },
    FoldShape {
        span: Span,
        missing_state: bool,
    },
    FoldItem {
        span: Span,
    },
    FoldBindings {
        span: Span,
        error: PureTypeError,
    },
    FoldLimit {
        span: Span,
        exceeds_bound: bool,
    },
    FoldItems {
        span: Span,
    },
    FoldState {
        span: Span,
    },
    Native {
        span: Span,
        error: Error,
    },
    Generation {
        span: Span,
        error: StructureError,
    },
    Produced {
        span: Span,
        error: PureTypeError,
    },
}

impl<Error> fmt::Display for ScalarSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "scalar source shape is invalid",
            Self::Names { .. } => "scalar source named operands are invalid",
            Self::LiteralLimit { .. } => "scalar source literal exceeds language bounds",
            Self::NonScalar { .. } => "scalar source operand must be pure and scalar",
            Self::Binary { .. } => "binary source operand types are incompatible",
            Self::Unary { .. } => "unary source operand type is incompatible",
            Self::InconsistentLiteral { .. } => "native source facts disagree with a literal",
            Self::Condition { .. } => "source condition must be pure and boolean",
            Self::BranchTypes { .. } => "source branches must have the same result type",
            Self::RecoveryTypes { .. } => {
                "source recovery operands must be pure and have the same scalar type"
            }
            Self::StringCount { .. } => "source string list has too many entries",
            Self::StringLabels { .. } => "source string list labels must be bounded and unique",
            Self::StringItem { .. } => "source string list entries must have string type",
            Self::StringBytes { .. } => "source string list exceeds aggregate text bounds",
            Self::BindingShape { .. } => "source binding requires one named value and body",
            Self::Bindings { .. } => "source lexical binding policy is invalid",
            Self::LoopShape { .. } => "source loop named operands are invalid",
            Self::LoopLimit { .. } => "source loop requires a bounded integer literal limit",
            Self::LoopCondition { .. } => "source loop condition must be boolean",
            Self::LoopState { .. } => "source loop next must preserve its scalar state type",
            Self::FoldShape { .. } => "source fold named operands are invalid",
            Self::FoldItem { .. } => "source fold item requires a literal local name",
            Self::FoldBindings { .. } => "source fold lexical binding policy is invalid",
            Self::FoldLimit { .. } => "source fold requires a bounded integer literal limit",
            Self::FoldItems { .. } => "source fold items must have string list type",
            Self::FoldState { .. } => "source fold next must preserve its scalar state type",
            Self::Native { .. } => "native source extension lowering failed",
            Self::Generation { .. } => "scalar source generation exceeds its limits",
            Self::Produced { .. } => "native source extension IR is invalid",
        })
    }
}
impl<Error> fmt::Debug for ScalarSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ScalarSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ScalarSourceResult<Node, Error> = Result<(Node, ScalarType), ScalarSourceError<Error>>;
type FormResult<Node, Type, Error> = Result<(Node, Type), ScalarSourceError<Error>>;

/// Unchecked native type/purity observations for the reference adapter only.
/// The public bounded entry establishes structural purity itself. No native
/// Clone/Debug/serde/Send requirement is imposed on IR slots or errors.
pub(crate) struct Operand<Node> {
    pub value: Node,
    pub scalar_type: Option<ScalarType>,
    pub pure: bool,
}

type OperandResult<Node, Error> = Result<Operand<Node>, ScalarSourceError<Error>>;

/// Explicit inclusive lexical policy. Zero bindings forbids prefix/local bindings;
/// the fixed ceiling is shared with cold IR inference. No default grants locals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScalarSourceLimits {
    pub source: SourceCallLimits,
    pub max_bindings: usize,
}

/// Read-only borrowed type observations, not runtime values or mutable authority.
/// Debug exposes counts only. The callback cannot insert, remove or retain a frame.
///
/// ```compile_fail
/// use leselang_hir::scalar_source::ScalarSourceScope;
/// use leselang_runtime_core::ScalarType;
/// fn mutate(mut scope: ScalarSourceScope<'_, '_>) {
///     scope.push("injected", ScalarType::Integer);
/// }
/// ```
#[derive(Clone, Copy)]
pub struct ScalarSourceScope<'scope, 'names> {
    bindings: &'scope [(&'names str, ScalarType)],
}

impl ScalarSourceScope<'_, '_> {
    pub fn get(&self, name: &str) -> Option<ScalarType> {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == name)
            .map(|(_, ty)| *ty)
    }

    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

impl fmt::Debug for ScalarSourceScope<'_, '_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScalarSourceScope")
            .field("bindings", &self.len())
            .finish()
    }
}

pub(crate) struct BindingSource<'source> {
    pub binding: &'source NamedArgument,
    pub body: &'source Expression,
}

impl BindingSource<'_> {
    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        value: Node<Field, Operation, HostEffect, IrResult>,
        body: Node<Field, Operation, HostEffect, IrResult>,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Bind {
            name: self.binding.name.clone(),
            value: Box::new(value),
            body: Box::new(body),
        }
    }
}

pub(crate) fn binding_source<Error>(
    expression: &Expression,
) -> Result<BindingSource<'_>, ScalarSourceError<Error>> {
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ScalarSourceError::BindingShape { span: at });
    };
    if callee != "bind"
        || arguments.len() != 2
        || arguments
            .iter()
            .filter(|argument| argument.name == "body")
            .count()
            != 1
    {
        return Err(ScalarSourceError::BindingShape { span: at });
    }
    let binding = arguments
        .iter()
        .find(|argument| argument.name != "body")
        .ok_or(ScalarSourceError::BindingShape { span: at })?;
    if !valid_local_name(&binding.name) {
        return Err(ScalarSourceError::Bindings {
            span: binding.span,
            error: PureTypeError::InvalidName,
        });
    }
    let body = arguments
        .iter()
        .find(|argument| argument.name == "body")
        .ok_or(ScalarSourceError::BindingShape { span: at })?;
    Ok(BindingSource {
        binding,
        body: &body.value,
    })
}

fn check_binding<Error>(
    source: &BindingSource<'_>,
    scope: &ScopeFrame<'_, '_, ScalarType>,
    max_bindings: usize,
) -> Result<(), ScalarSourceError<Error>> {
    check_local_binding(
        &source.binding.name,
        source.binding.span,
        scope,
        max_bindings,
    )
}

fn check_local_binding<Error>(
    name: &str,
    at: Span,
    scope: &ScopeFrame<'_, '_, ScalarType>,
    max_bindings: usize,
) -> Result<(), ScalarSourceError<Error>> {
    let error = if scope.get(name).is_some() {
        Some(PureTypeError::ShadowedBinding)
    } else if scope.len() >= max_bindings {
        Some(PureTypeError::BindingLimit)
    } else {
        None
    };
    match error {
        Some(error) => Err(ScalarSourceError::Bindings { span: at, error }),
        None => Ok(()),
    }
}

pub(crate) struct LoopSource<'source> {
    pub binding: &'source NamedArgument,
    pub condition: &'source Expression,
    pub next: &'source Expression,
    limit: &'source Expression,
}

pub(crate) struct LoopBody<Node> {
    condition: Node,
    next: Node,
}

impl LoopSource<'_> {
    pub fn limit<Error>(&self) -> Result<u64, ScalarSourceError<Error>> {
        let Expression::Integer { value, .. } = self.limit else {
            return Err(ScalarSourceError::LoopLimit {
                span: span(self.limit),
                exceeds_bound: false,
            });
        };
        if *value > MAX_LOOP_ITERATIONS {
            return Err(ScalarSourceError::LoopLimit {
                span: span(self.limit),
                exceeds_bound: true,
            });
        }
        Ok(*value)
    }

    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        initial: Node<Field, Operation, HostEffect, IrResult>,
        body: LoopBody<Node<Field, Operation, HostEffect, IrResult>>,
        limit: u64,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Loop {
            name: self.binding.name.clone(),
            initial: Box::new(initial),
            condition: Box::new(body.condition),
            next: Box::new(body.next),
            limit,
        }
    }
}

pub(crate) fn loop_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<LoopSource<'_>, ScalarSourceError<Error>> {
    let reserved = ["while", "next", "limit"];
    let binding = arguments
        .iter()
        .find(|argument| !reserved.contains(&argument.name.as_str()))
        .ok_or(ScalarSourceError::LoopShape {
            span: at,
            missing_state: true,
        })?;
    if arguments.len() != 4
        || [binding.name.as_str(), "while", "next", "limit"]
            .iter()
            .any(|name| {
                arguments
                    .iter()
                    .filter(|argument| argument.name == *name)
                    .count()
                    != 1
            })
    {
        return Err(ScalarSourceError::LoopShape {
            span: at,
            missing_state: false,
        });
    }
    if !valid_local_name(&binding.name) {
        return Err(ScalarSourceError::Bindings {
            span: binding.span,
            error: PureTypeError::InvalidName,
        });
    }
    let get = |name: &str| {
        arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value)
            .ok_or(ScalarSourceError::LoopShape {
                span: at,
                missing_state: false,
            })
    };
    Ok(LoopSource {
        binding,
        condition: get("while")?,
        next: get("next")?,
        limit: get("limit")?,
    })
}

pub(crate) fn scalar_operand<Node, Error>(
    operand: Operand<Node>,
    at: Span,
) -> ScalarSourceResult<Node, Error> {
    let ty = operand
        .scalar_type
        .filter(|_| operand.pure)
        .ok_or(ScalarSourceError::NonScalar { span: at })?;
    Ok((operand.value, ty))
}

// Scope entry stays with the adapter, but both callers use the same sequential
// pure condition/state gates. Even zero/cold iterations compile both children.
pub(crate) fn lower_loop_body<'source, Node, Error>(
    source: &LoopSource<'source>,
    state_type: ScalarType,
    mut child: impl FnMut(&'source Expression) -> OperandResult<Node, Error>,
) -> Result<LoopBody<Node>, ScalarSourceError<Error>> {
    let (condition, ty) = scalar_operand(child(source.condition)?, span(source.condition))?;
    if ty != ScalarType::Boolean {
        return Err(ScalarSourceError::LoopCondition {
            span: span(source.condition),
        });
    }
    let (next, ty) = scalar_operand(child(source.next)?, span(source.next))?;
    if ty != state_type {
        return Err(ScalarSourceError::LoopState {
            span: span(source.next),
        });
    }
    Ok(LoopBody { condition, next })
}

fn preflight_bindings<'source, Error>(
    expression: &'source Expression,
    scope: &mut ScopeFrame<'_, 'source, ScalarType>,
    max_bindings: usize,
) -> Result<(), ScalarSourceError<Error>> {
    if let Expression::Call {
        callee, arguments, ..
    } = expression
    {
        if callee == "bind" {
            let source = binding_source(expression)?;
            check_binding(&source, scope, max_bindings)?;
            preflight_bindings(&source.binding.value, scope, max_bindings)?;
            let mut local = scope.nested();
            local
                .push(&source.binding.name, ScalarType::None)
                .map_err(|_| ScalarSourceError::Bindings {
                    span: source.binding.span,
                    error: PureTypeError::ShadowedBinding,
                })?;
            preflight_bindings(source.body, &mut local, max_bindings)?;
        } else if callee == "loop" {
            let source = loop_source(arguments, span(expression))?;
            check_local_binding(
                &source.binding.name,
                source.binding.span,
                scope,
                max_bindings,
            )?;
            source.limit()?;
            preflight_bindings(&source.binding.value, scope, max_bindings)?;
            let mut local = scope.nested();
            local
                .push(&source.binding.name, ScalarType::None)
                .map_err(|_| ScalarSourceError::Bindings {
                    span: source.binding.span,
                    error: PureTypeError::ShadowedBinding,
                })?;
            preflight_bindings(source.condition, &mut local, max_bindings)?;
            preflight_bindings(source.next, &mut local, max_bindings)?;
        } else if callee == "fold" {
            let source = fold_source(arguments, span(expression))?;
            source.check_scope(scope, Some(max_bindings))?;
            source.limit()?;
            preflight_bindings(source.items, scope, max_bindings)?;
            preflight_bindings(&source.binding.value, scope, max_bindings)?;
            let mut local = scope.nested();
            for (name, ty) in [
                (source.binding.name.as_str(), ScalarType::None),
                (source.item, ScalarType::String),
            ] {
                local
                    .push(name, ty)
                    .map_err(|_| source.binding_error(PureTypeError::ShadowedBinding))?;
            }
            preflight_bindings(source.next, &mut local, max_bindings)?;
        } else {
            for argument in arguments {
                preflight_bindings(&argument.value, scope, max_bindings)?;
            }
        }
    }
    Ok(())
}

fn check_named<Error>(
    arguments: &[NamedArgument],
    expected: &'static [&'static str],
    at: Span,
) -> Result<(), ScalarSourceError<Error>> {
    if arguments.len() != expected.len()
        || expected.iter().any(|name| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(ScalarSourceError::Names { span: at, expected });
    }
    Ok(())
}

fn named<'source, Error>(
    arguments: &'source [NamedArgument],
    expected: &'static [&'static str],
    at: Span,
) -> Result<Vec<&'source Expression>, ScalarSourceError<Error>> {
    check_named(arguments, expected, at)?;
    Ok(expected
        .iter()
        .filter_map(|name| {
            arguments
                .iter()
                .find(|argument| argument.name == *name)
                .map(|argument| &argument.value)
        })
        .collect())
}

pub(crate) fn primitive(expression: &Expression) -> bool {
    match expression {
        Expression::Call { callee, .. } => {
            matches!(callee.as_str(), "strings" | "recover")
                || BinaryOperator::parse(callee).is_some()
                || UnaryOperator::parse(callee).is_some()
        }
        Expression::Reference { .. } => false,
        _ => true,
    }
}

// Shared single-form construction. Legacy callers retain source accounting,
// lexical/type gates and exact diagnostics; neither child is evaluated here.
pub(crate) fn lower_form<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    mut child: impl FnMut(
        &'source Expression,
    ) -> Result<
        Operand<Node<Field, Operation, HostEffect, IrResult>>,
        ScalarSourceError<Error>,
    >,
) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    let at = span(expression);
    let literal = match expression {
        Expression::Integer { value, .. } => Some(ScalarValue::Integer(*value)),
        Expression::Boolean { value, .. } => Some(ScalarValue::Boolean(*value)),
        Expression::None { .. } => Some(ScalarValue::None),
        Expression::String { value, .. } => {
            if value.len() > leselang_runtime_core::MAX_SCALAR_STRING_BYTES {
                return Err(ScalarSourceError::LiteralLimit { span: at });
            }
            Some(ScalarValue::String(value.clone()))
        }
        _ => None,
    };
    if let Some(value) = literal {
        let ty = value.scalar_type();
        return Ok((Computation::Literal { value }, ty));
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ScalarSourceError::NonScalar { span: at });
    };
    if callee == "strings" {
        check_string_count(arguments, at)?;
        let mut names = HashSet::new();
        let mut items = Vec::with_capacity(arguments.len());
        for argument in arguments {
            check_string_label(argument, &mut names)?;
            let item = child(&argument.value)?;
            let ty =
                item.scalar_type
                    .filter(|_| item.pure)
                    .ok_or(ScalarSourceError::NonScalar {
                        span: argument.span,
                    })?;
            if ty != ScalarType::String {
                return Err(ScalarSourceError::StringItem {
                    span: argument.span,
                });
            }
            items.push(item.value);
        }
        let all_literal = items.iter().all(|item| {
            matches!(
                item,
                Computation::Literal {
                    value: ScalarValue::String(_)
                }
            )
        });
        let value = if all_literal {
            let mut literal = StringListBuilder::with_capacity(items.len())
                .map_err(|_| ScalarSourceError::StringCount { span: at })?;
            for item in items {
                let Computation::Literal {
                    value: ScalarValue::String(text),
                } = item
                else {
                    return Err(ScalarSourceError::InconsistentLiteral { span: at });
                };
                literal
                    .try_push(text)
                    .map_err(|_| ScalarSourceError::StringBytes { span: at })?;
            }
            Computation::Literal {
                value: ScalarValue::StringList(literal.finish()),
            }
        } else {
            Computation::Strings { items }
        };
        return Ok((value, ScalarType::StringList));
    }
    if callee == "recover" {
        let arguments = named(arguments, &["value", "fallback"], at)?;
        let value = child(arguments[0])?;
        let fallback = child(arguments[1])?;
        let ty = value
            .scalar_type
            .filter(|ty| value.pure && fallback.pure && fallback.scalar_type == Some(*ty))
            .ok_or(ScalarSourceError::RecoveryTypes { span: at })?;
        return Ok((
            Computation::Recover {
                value: Box::new(value.value),
                fallback: Box::new(fallback.value),
            },
            ty,
        ));
    }
    if let Some(operator) = BinaryOperator::parse(callee) {
        let arguments = named(arguments, &["left", "right"], at)?;
        let left = child(arguments[0])?;
        let right = child(arguments[1])?;
        let scalar = |operand: &Operand<_>, expression| {
            operand
                .scalar_type
                .filter(|_| operand.pure)
                .ok_or(ScalarSourceError::NonScalar {
                    span: span(expression),
                })
        };
        let left_type = scalar(&left, arguments[0])?;
        let right_type = scalar(&right, arguments[1])?;
        let ty = operator
            .result_type(left_type, right_type)
            .ok_or(ScalarSourceError::Binary {
                span: at,
                operator,
                left: left_type,
                right: right_type,
            })?;
        return Ok((
            Computation::Binary {
                operator,
                left: Box::new(left.value),
                right: Box::new(right.value),
            },
            ty,
        ));
    }
    let operator = UnaryOperator::parse(callee).ok_or(ScalarSourceError::NonScalar { span: at })?;
    let arguments = named(arguments, &["value"], at)?;
    let input = child(arguments[0])?;
    let ty = input
        .scalar_type
        .filter(|_| input.pure)
        .and_then(|ty| operator.result_type(ty))
        .ok_or(ScalarSourceError::Unary { span: at, operator })?;
    if operator == UnaryOperator::OptionalString
        && let Computation::Literal { value } = input.value
    {
        let text = match value {
            ScalarValue::String(text) => Some(text),
            ScalarValue::None => None,
            _ => return Err(ScalarSourceError::Unary { span: at, operator }),
        };
        return Ok((
            Computation::Literal {
                value: ScalarValue::OptionalString(OptionalStringValue(text)),
            },
            ty,
        ));
    }
    Ok((
        Computation::Unary {
            operator,
            value: Box::new(input.value),
        },
        ty,
    ))
}

fn check_string_count<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<(), ScalarSourceError<Error>> {
    if arguments.len() > MAX_STRING_LIST_ITEMS {
        return Err(ScalarSourceError::StringCount { span: at });
    }
    Ok(())
}

fn check_string_label<'source, Error>(
    argument: &'source NamedArgument,
    names: &mut HashSet<&'source str>,
) -> Result<(), ScalarSourceError<Error>> {
    if !valid_local_name(&argument.name) || !names.insert(&argument.name) {
        return Err(ScalarSourceError::StringLabels {
            span: argument.span,
        });
    }
    Ok(())
}

// Generic branch identity preserves the reference adapter's non-scalar and
// effect branches. The public scalar entry separately rejects impure extensions.
pub(crate) fn lower_choose_form<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Type: PartialEq,
    Error,
>(
    expression: &'source Expression,
    mut child: impl FnMut(
        &'source Expression,
    ) -> FormResult<
        Operand<Node<Field, Operation, HostEffect, IrResult>>,
        Type,
        Error,
    >,
    is_boolean: impl Fn(&Type) -> bool,
) -> FormResult<Node<Field, Operation, HostEffect, IrResult>, Type, Error> {
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ScalarSourceError::NonScalar { span: at });
    };
    if callee != "choose" {
        return Err(ScalarSourceError::NonScalar { span: at });
    }
    let arguments = named(arguments, &["when", "then", "otherwise"], at)?;
    let (when, when_type) = child(arguments[0])?;
    if !is_boolean(&when_type) || !when.pure {
        return Err(ScalarSourceError::Condition {
            span: span(arguments[0]),
        });
    }
    let (then, then_type) = child(arguments[1])?;
    let (otherwise, otherwise_type) = child(arguments[2])?;
    if then_type != otherwise_type {
        return Err(ScalarSourceError::BranchTypes { span: at });
    }
    Ok((
        Computation::Choose {
            when: Box::new(when.value),
            then: Box::new(then.value),
            otherwise: Box::new(otherwise.value),
        },
        then_type,
    ))
}

fn preflight_names<Error>(expression: &Expression) -> Result<(), ScalarSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            if callee == "choose" {
                check_named::<Error>(arguments, &["when", "then", "otherwise"], span(node))?;
            } else if callee == "recover" {
                check_named::<Error>(arguments, &["value", "fallback"], span(node))?;
            } else if callee == "strings" {
                check_string_count::<Error>(arguments, span(node))?;
                let mut names = HashSet::new();
                for argument in arguments {
                    check_string_label::<Error>(argument, &mut names)?;
                }
            } else if BinaryOperator::parse(callee).is_some() {
                check_named::<Error>(arguments, &["left", "right"], span(node))?;
            } else if UnaryOperator::parse(callee).is_some() {
                check_named::<Error>(arguments, &["value"], span(node))?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

struct Builder<'scope, 'source, Extension> {
    max_depth: usize,
    max_bindings: Option<usize>,
    budget: &'scope mut StructureBudget,
    scope: ScopeFrame<'scope, 'source, ScalarType>,
    extension: &'scope mut Extension,
}

impl<'source, Extension> Builder<'_, 'source, Extension> {
    fn build<Field, Operation, HostEffect, IrResult, Error>(
        &mut self,
        expression: &'source Expression,
        depth: usize,
    ) -> OperandResult<Node<Field, Operation, HostEffect, IrResult>, Error>
    where
        Extension: FnMut(
            &'source Expression,
            ScalarSourceScope<'_, 'source>,
        ) -> Result<
            (
                Node<Field, Operation, HostEffect, IrResult>,
                Option<ScalarType>,
            ),
            Error,
        >,
    {
        let at = span(expression);
        if depth > self.max_depth {
            return Err(ScalarSourceError::Generation {
                span: at,
                error: StructureError::DepthLimit,
            });
        }
        if let Some(max_bindings) = self.max_bindings {
            if let Expression::Call {
                callee, arguments, ..
            } = expression
                && callee == "fold"
            {
                let source = fold_source(arguments, at)?;
                source.check_scope(&self.scope, Some(max_bindings))?;
                let limit = source.limit()?;
                self.budget
                    .visit(depth, 0, 0)
                    .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
                let items = self.build(source.items, depth + 1)?;
                let items = fold_items(items, span(source.items))?;
                let initial = self.build(&source.binding.value, depth + 1)?;
                let (initial, state_type) = scalar_operand(initial, span(&source.binding.value))?;
                let next = {
                    let mut scope = self.scope.nested();
                    for (name, ty) in [
                        (source.binding.name.as_str(), state_type),
                        (source.item, ScalarType::String),
                    ] {
                        scope
                            .push(name, ty)
                            .map_err(|_| source.binding_error(PureTypeError::ShadowedBinding))?;
                    }
                    let mut nested = Builder {
                        max_depth: self.max_depth,
                        max_bindings: self.max_bindings,
                        budget: self.budget,
                        scope,
                        extension: self.extension,
                    };
                    let next = nested.build(source.next, depth + 1)?;
                    fold_next(next, state_type, span(source.next))?
                };
                return Ok(Operand {
                    value: source.construct(items, initial, next, limit),
                    scalar_type: Some(state_type),
                    pure: true,
                });
            }
            if let Expression::Call {
                callee, arguments, ..
            } = expression
                && callee == "loop"
            {
                let source = loop_source(arguments, at)?;
                check_local_binding(
                    &source.binding.name,
                    source.binding.span,
                    &self.scope,
                    max_bindings,
                )?;
                let limit = source.limit()?;
                self.budget
                    .visit(depth, 0, 0)
                    .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
                let initial = self.build(&source.binding.value, depth + 1)?;
                let (initial, state_type) = scalar_operand(initial, span(&source.binding.value))?;
                let body = {
                    let mut scope = self.scope.nested();
                    scope.push(&source.binding.name, state_type).map_err(|_| {
                        ScalarSourceError::Bindings {
                            span: source.binding.span,
                            error: PureTypeError::ShadowedBinding,
                        }
                    })?;
                    let mut nested = Builder {
                        max_depth: self.max_depth,
                        max_bindings: self.max_bindings,
                        budget: self.budget,
                        scope,
                        extension: self.extension,
                    };
                    lower_loop_body(&source, state_type, |child| nested.build(child, depth + 1))?
                };
                return Ok(Operand {
                    value: source.construct(initial, body, limit),
                    scalar_type: Some(state_type),
                    pure: true,
                });
            }
            if matches!(expression, Expression::Call { callee, .. } if callee == "bind") {
                let source = binding_source(expression)?;
                check_binding(&source, &self.scope, max_bindings)?;
                self.budget
                    .visit(depth, 0, 0)
                    .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
                let value = self.build(&source.binding.value, depth + 1)?;
                let ty = value.scalar_type.ok_or(ScalarSourceError::NonScalar {
                    span: span(&source.binding.value),
                })?;
                let body = {
                    let mut scope = self.scope.nested();
                    scope.push(&source.binding.name, ty).map_err(|_| {
                        ScalarSourceError::Bindings {
                            span: source.binding.span,
                            error: PureTypeError::ShadowedBinding,
                        }
                    })?;
                    let mut nested = Builder {
                        max_depth: self.max_depth,
                        max_bindings: self.max_bindings,
                        budget: self.budget,
                        scope,
                        extension: self.extension,
                    };
                    nested.build(source.body, depth + 1)?
                };
                let ty = body.scalar_type.ok_or(ScalarSourceError::NonScalar {
                    span: span(source.body),
                })?;
                return Ok(Operand {
                    value: source.construct(value.value, body.value),
                    scalar_type: Some(ty),
                    pure: true,
                });
            }
            if let Expression::Reference { name, .. } = expression
                && let Some(ty) = self.scope.get(name)
            {
                self.budget
                    .visit(depth, 0, 0)
                    .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
                return Ok(Operand {
                    value: Computation::Local { name: name.clone() },
                    scalar_type: Some(*ty),
                    pure: true,
                });
            }
        }
        if matches!(expression, Expression::Call { callee, .. } if callee == "choose") {
            self.budget
                .visit(depth, 0, 0)
                .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
            let (value, scalar_type) = lower_choose_form(
                expression,
                |child| {
                    let child = self.build(child, depth + 1)?;
                    let ty = child.scalar_type;
                    Ok((child, ty))
                },
                |ty| *ty == Some(ScalarType::Boolean),
            )?;
            return Ok(Operand {
                value,
                scalar_type,
                pure: true,
            });
        }
        if primitive(expression) {
            self.budget
                .visit(depth, 0, 0)
                .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
            let (value, ty) = lower_form(expression, |child| self.build(child, depth + 1))?;
            return Ok(Operand {
                value,
                scalar_type: Some(ty),
                pure: true,
            });
        }
        self.budget
            .check_pending(0, 1)
            .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
        let (value, scalar_type) = (self.extension)(
            expression,
            ScalarSourceScope {
                bindings: self.scope.bindings(),
            },
        )
        .map_err(|error| ScalarSourceError::Native { span: at, error })?;
        preflight_with_budget(&value, depth, self.budget)
            .map_err(|error| ScalarSourceError::Produced { span: at, error })?;
        if let Computation::Literal { value } = &value
            && scalar_type != Some(value.scalar_type())
        {
            return Err(ScalarSourceError::InconsistentLiteral { span: at });
        }
        Ok(Operand {
            value,
            scalar_type,
            pure: true,
        })
    }
}

/// Lower literals, every closed scalar operator, strings, choose and recover into
/// the original generic IR. Whole cold AST bounds and reserved named signatures/
/// labels precede any extension. Other forms, including locals/fields/bind/loops/
/// helpers, require an explicit
/// trusted callback returning bounded pure IR and inferred type observations.
/// Extensions are not automatically parsed, typed, dispatched or authorized.
///
/// Generation shares one aggregate budget, counting constructors before folding;
/// optional/list literal text moves unchanged, with no refund or second buffer copy.
/// Both short-circuit operands are lowered/typed; no arithmetic or parsing executes.
/// Extension errors/unwind release partial IR without retries or rollback. Native
/// callbacks/metadata/drop cannot be preempted. Hosts own ingress, source/span
/// authenticity, lexical/complete cold typing, native expansion, work and redaction.
/// Output is an observed scalar expression, not a program, receipt, authority,
/// continuation, complete helper/control lowerer or security sandbox. All native IR slots
/// and errors can be non-Clone, non-Debug, non-serde and GUI-local.
pub fn lower_scalar_source<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    mut extension: impl FnMut(
        &'source Expression,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits) {
        return Err(ScalarSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits).map_err(ScalarSourceError::Source)?;
    preflight_names(expression)?;
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    let mut bindings = Vec::new();
    let output = Builder {
        max_depth: limits.max_lowered_depth,
        max_bindings: None,
        budget: &mut budget,
        scope: ScopeFrame::new(&mut bindings),
        extension: &mut |expression, _scope: ScalarSourceScope<'_, 'source>| extension(expression),
    }
    .build(expression, 0)?;
    let ty = output.scalar_type.ok_or(ScalarSourceError::NonScalar {
        span: span(expression),
    })?;
    Ok((output.value, ty))
}

/// Bounded scalar construction with shared lexical bind/local, loop and fold support.
///
/// The entire physical source, reserved signatures, prefix and every cold lexical
/// binding are checked before any native callback. Prefix names must be bounded
/// and unique; active names cannot be shadowed. Initializers see the parent scope,
/// bodies see their binding, and sibling branches cannot leak bindings. The active
/// binding limit includes the prefix, not cumulative sibling declarations.
/// Loops require a literal limit from 0 through 1024; limits are source metadata,
/// not generated literal nodes. Initial state is pure/scalar, while is boolean and
/// next preserves the state type. Both bodies compile even when limit is zero;
/// construction never executes/unrolls iterations or grants evaluator fuel.
/// Folds prepare items before initial state, then bind scalar state/string item
/// only for next. Both locals count against the active binding policy; literal
/// item labels and limits 0-64 are metadata, not generated IR nodes. Empty/zero
/// folds still compile next, but actual list length/exhaustion stays with execution.
///
/// Bound references always produce Local IR without invoking the extension.
/// Unbound references remain explicit native aliases; rejecting unknown names is
/// the adapter's responsibility. The callback sees borrowed read-only scalar type
/// observations, not values. Other native forms still require complete cold IR
/// typing against the exact prefix and bounded expansion/work. Output references
/// need corresponding runtime bindings; source facts confer no dispatch authority.
///
/// Scope guards restore the frame on errors and unwind; partial native IR drops
/// once without rollback/retry. This does not implement host-result capture,
/// helpers, suspension or arbitrary native lexical semantics. The legacy entry
/// leaves bind/local handling to its callback for compatibility.
pub fn lower_scalar_source_with_scope<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    limits: ScalarSourceLimits,
    prefix: &[(&'source str, ScalarType)],
    mut extension: impl FnMut(
        &'source Expression,
        ScalarSourceScope<'_, 'source>,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits.source) || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS {
        return Err(ScalarSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits.source).map_err(ScalarSourceError::Source)?;
    preflight_names(expression)?;
    let mut names = HashSet::new();
    if prefix.len() > limits.max_bindings
        || prefix
            .iter()
            .any(|(name, _)| !valid_local_name(name) || !names.insert(*name))
    {
        return Err(ScalarSourceError::Bindings {
            span: span(expression),
            error: PureTypeError::InvalidScope,
        });
    }
    let mut bindings = prefix.to_vec();
    let mut scope = ScopeFrame::new(&mut bindings);
    preflight_bindings(expression, &mut scope, limits.max_bindings)?;
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    let output = Builder {
        max_depth: limits.source.max_lowered_depth,
        max_bindings: Some(limits.max_bindings),
        budget: &mut budget,
        scope,
        extension: &mut extension,
    }
    .build(expression, 0)?;
    let ty = output.scalar_type.ok_or(ScalarSourceError::NonScalar {
        span: span(expression),
    })?;
    Ok((output.value, ty))
}
