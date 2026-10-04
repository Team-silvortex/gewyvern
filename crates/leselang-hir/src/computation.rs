use super::*;
use crate::host_call::{HostOperation, invalid_argument};
use crate::result_field::ResultField;
use leselang_runtime_core::ScopeFrame;
use leselang_syntax::NamedArgument;

pub const MAX_COMPUTATION_NODES: usize = 1_024;

pub use leselang_runtime_core::{
    BinaryOperator, MAX_LOOP_ITERATIONS, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    OptionalStringValue, ScalarType, ScalarValue, StringListValue, UnaryOperator,
};

// Source expansion cost belongs to HIR, not the host-neutral data contract.
pub(crate) fn source_shape_extra(value: &ScalarValue) -> (usize, usize) {
    match value {
        ScalarValue::OptionalString(_) => (1, 1),
        ScalarValue::StringList(value) => (value.0.len(), usize::from(!value.0.is_empty())),
        _ => (0, 0),
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    Sequence,
    Parallel,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedBranch {
    pub name: String,
    pub value: Computation,
    pub result_type: Type,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Computation {
    Literal {
        value: ScalarValue,
    },
    Strings {
        items: Vec<Self>,
    },
    Local {
        name: String,
    },
    Field {
        value: Box<Self>,
        field: ResultField,
    },
    Member {
        group: String,
        name: String,
        operation: HostOperation,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Self>,
        right: Box<Self>,
    },
    Unary {
        operator: UnaryOperator,
        value: Box<Self>,
    },
    Bind {
        name: String,
        value: Box<Self>,
        body: Box<Self>,
    },
    Loop {
        name: String,
        initial: Box<Self>,
        condition: Box<Self>,
        next: Box<Self>,
        limit: u64,
    },
    Fold {
        name: String,
        item: String,
        items: Box<Self>,
        initial: Box<Self>,
        next: Box<Self>,
        limit: u64,
    },
    Choose {
        when: Box<Self>,
        then: Box<Self>,
        otherwise: Box<Self>,
    },
    Recover {
        value: Box<Self>,
        fallback: Box<Self>,
    },
    Host {
        effect: Box<Effect>,
    },
    Call {
        operation: HostOperation,
        arguments: Vec<ComputedArgument>,
    },
    Group {
        group_kind: GroupKind,
        branches: Vec<ComputedBranch>,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComputedArgument {
    pub name: String,
    pub value: Computation,
}

/// A bounded named group signature supplied by a validated embedding environment.
#[derive(Clone)]
pub struct GroupLocalType {
    pub name: String,
    pub members: Vec<(String, HostOperation)>,
}

pub(super) struct LocalType {
    ty: Type,
    members: Option<Vec<(String, HostOperation)>>,
}

pub(super) type TypeScope<'scope, 'names> = ScopeFrame<'scope, 'names, LocalType>;

impl From<Type> for LocalType {
    fn from(ty: Type) -> Self {
        Self { ty, members: None }
    }
}

fn group_members(
    expression: &Computation,
    scope: &TypeScope<'_, '_>,
) -> Option<Vec<(String, HostOperation)>> {
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => group_members(body, scope),
        Computation::Local { name } => scope.get(name)?.members.clone(),
        Computation::Host { .. } | Computation::Group { .. } | Computation::Choose { .. } => {
            group_signature(expression).map(|(_, members)| members)
        }
        _ => None,
    }
}

// Conditional exports are a closed signature, never a union of branch members.
fn group_signature(expression: &Computation) -> Option<(GroupKind, Vec<(String, HostOperation)>)> {
    expression.validate_structure().ok()?;
    let mut pending = vec![expression];
    let mut signature = None;
    while let Some(expression) = pending.pop() {
        let current = match expression {
            Computation::Bind { value, body, .. } if value.is_pure() => {
                pending.push(body);
                continue;
            }
            Computation::Choose {
                when,
                then,
                otherwise,
            } if when.is_pure() => {
                pending.extend([then.as_ref(), otherwise.as_ref()]);
                continue;
            }
            Computation::Host { effect } => {
                let (kind, branches) = match effect.as_ref() {
                    Effect::Sequence { steps } => (GroupKind::Sequence, steps),
                    Effect::All { branches } => (GroupKind::Parallel, branches),
                    _ => return None,
                };
                let members = branches
                    .iter()
                    .map(|branch| {
                        Some((
                            branch.name.clone(),
                            HostOperation::for_effect(&branch.effect)?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                (kind, members)
            }
            Computation::Group {
                group_kind,
                branches,
            } => {
                let members = branches
                    .iter()
                    .map(|branch| {
                        Some((
                            branch.name.clone(),
                            branch.value.prepared_atomic_operation()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                (*group_kind, members)
            }
            _ => return None,
        };
        if signature
            .as_ref()
            .is_some_and(|signature| *signature != current)
        {
            return None;
        }
        signature = Some(current);
    }
    signature
}

impl Computation {
    /// Checks bounded structure only; lexical types and authority need full HIR validation.
    pub fn validate_structure(&self) -> Result<(), CanonicalSourceError> {
        validate_shape(self)
    }

    pub fn is_pure(&self) -> bool {
        let mut pending = vec![self];
        while let Some(expression) = pending.pop() {
            if matches!(
                expression,
                Self::Host { .. } | Self::Call { .. } | Self::Group { .. }
            ) {
                return false;
            }
            pending.extend(expression.children());
        }
        true
    }

    /// Recognizes one atomic tail effect, with only pure preparation and selection.
    /// Callers must validate structure and lexical types before using this classification.
    pub fn is_atomic_tail(&self) -> bool {
        match self {
            Self::Host { effect } => HostOperation::for_effect(effect).is_some(),
            Self::Call { arguments, .. } => {
                arguments.iter().all(|argument| argument.value.is_pure())
            }
            Self::Bind { value, body, .. } => value.is_pure() && body.is_atomic_tail(),
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_atomic_tail() && otherwise.is_atomic_tail(),
            _ => false,
        }
    }

    /// One statically typed operation after pure preparation/selection, never a capture.
    /// Validate bounded structure and lexical types before relying on this signature.
    pub fn prepared_atomic_operation(&self) -> Option<HostOperation> {
        let mut pending = vec![(self, 0usize, true)];
        let mut visited = 0usize;
        let mut signature = None;
        while let Some((value, depth, atomic)) = pending.pop() {
            let (extra_nodes, extra_depth) = match value {
                Self::Literal { value } => source_shape_extra(value),
                _ => (0, 0),
            };
            visited += 1 + extra_nodes;
            if visited > MAX_COMPUTATION_NODES || depth + extra_depth > MAX_EFFECT_NESTING_DEPTH {
                return None;
            }
            if !atomic {
                if matches!(
                    value,
                    Self::Host { .. } | Self::Call { .. } | Self::Group { .. }
                ) {
                    return None;
                }
                for child in value.children() {
                    if visited + pending.len() >= MAX_COMPUTATION_NODES {
                        return None;
                    }
                    pending.push((child, depth + 1, false));
                }
                continue;
            }
            let operation = match value {
                Self::Host { effect } => {
                    visited += 1;
                    if visited > MAX_COMPUTATION_NODES || depth >= MAX_EFFECT_NESTING_DEPTH {
                        return None;
                    }
                    HostOperation::for_effect(effect)?
                }
                Self::Call {
                    operation,
                    arguments,
                } => {
                    for argument in arguments {
                        if visited + pending.len() >= MAX_COMPUTATION_NODES {
                            return None;
                        }
                        pending.push((&argument.value, depth + 1, false));
                    }
                    *operation
                }
                Self::Bind { value, body, .. } => {
                    if visited + pending.len() + 2 > MAX_COMPUTATION_NODES {
                        return None;
                    }
                    pending.push((value, depth + 1, false));
                    pending.push((body, depth + 1, true));
                    continue;
                }
                Self::Choose {
                    when,
                    then,
                    otherwise,
                } => {
                    if visited + pending.len() + 3 > MAX_COMPUTATION_NODES {
                        return None;
                    }
                    pending.push((when, depth + 1, false));
                    pending.push((then, depth + 1, true));
                    pending.push((otherwise, depth + 1, true));
                    continue;
                }
                _ => return None,
            };
            if signature.is_some_and(|expected| expected != operation) {
                return None;
            }
            signature = Some(operation);
        }
        signature
    }

    /// One captured atomic suspension followed by a pure body, with pure preparation/selection.
    pub fn is_atomic_capture(&self) -> bool {
        match self {
            Self::Bind { value, body, .. } if value.is_pure() => body.is_atomic_capture(),
            Self::Bind { value, body, .. } => {
                value.prepared_atomic_operation().is_some() && body.is_pure()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_atomic_capture() && otherwise.is_atomic_capture(),
            _ => false,
        }
    }

    /// A bounded atomic chain, excluding effectful operands and dynamic groups.
    /// Every non-pure branch must reach an atomic suspension before returning.
    pub fn is_result_chain(&self) -> bool {
        match self {
            Self::Host { .. } | Self::Call { .. } => self.is_atomic_tail(),
            Self::Bind { value, body, .. } => {
                if value.is_pure() {
                    body.is_result_chain()
                } else {
                    value.prepared_atomic_operation().is_some()
                        && (body.is_pure() || body.is_result_chain())
                }
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_result_chain() && otherwise.is_result_chain(),
            _ => false,
        }
    }

    /// Maximum atomic suspensions on any path of a statically bounded result chain.
    /// Pure preparation does not consume a graph slot; cold branches still reserve slots.
    pub fn atomic_chain_bound(&self) -> Option<usize> {
        if !self.is_pure() && !self.is_result_chain() {
            return None;
        }
        self.atomic_flow_bound()
    }

    /// Includes scalar early exits while reserving the longest possible cold path.
    pub fn atomic_flow_bound(&self) -> Option<usize> {
        if !self.is_result_flow() {
            return None;
        }
        if self.is_pure() {
            return Some(0);
        }
        match self {
            Self::Host { .. } | Self::Call { .. } if self.is_atomic_tail() => Some(1),
            Self::Bind { value, body, .. } if value.is_pure() => body.atomic_flow_bound(),
            Self::Bind { value, body, .. } if value.prepared_atomic_operation().is_some() => {
                body.atomic_flow_bound()?.checked_add(1)
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } if when.is_pure() => Some(
                then.atomic_flow_bound()?
                    .max(otherwise.atomic_flow_bound()?),
            ),
            _ => None,
        }
    }

    /// Allows pure exits between atomic suspensions; callers must require a scalar final type.
    pub fn is_result_flow(&self) -> bool {
        if self.is_pure() {
            return true;
        }
        match self {
            Self::Host { .. } | Self::Call { .. } => self.is_atomic_tail(),
            Self::Bind { value, body, .. } => {
                (value.is_pure() || value.prepared_atomic_operation().is_some())
                    && body.is_result_flow()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => when.is_pure() && then.is_result_flow() && otherwise.is_result_flow(),
            _ => false,
        }
    }

    /// Whether any path returns before another effect, without evaluating its guards.
    pub fn can_return_without_suspending(&self) -> bool {
        if self.is_pure() {
            return true;
        }
        match self {
            Self::Bind { value, body, .. } => {
                value.is_pure() && body.can_return_without_suspending()
            }
            Self::Choose {
                when,
                then,
                otherwise,
            } => {
                when.is_pure()
                    && (then.can_return_without_suspending()
                        || otherwise.can_return_without_suspending())
            }
            _ => false,
        }
    }

    /// Revalidates a residual computation against a bounded, typed lexical environment.
    pub fn validate_in_scope(
        &self,
        scope: &[(String, Type)],
    ) -> Result<Type, CanonicalSourceError> {
        self.validate_in_group_scope(scope, &[])
    }

    /// Revalidates a residual body using closed, statically named group-member signatures.
    pub fn validate_in_group_scope(
        &self,
        scope: &[(String, Type)],
        groups: &[GroupLocalType],
    ) -> Result<Type, CanonicalSourceError> {
        validate_shape(self)?;
        let mut names = HashSet::new();
        let scope_len = scope.len().saturating_add(groups.len());
        if scope_len > MAX_EFFECT_NESTING_DEPTH
            || scope
                .iter()
                .any(|(name, _)| !valid_local(name) || !names.insert(name))
            || groups.iter().any(|group| {
                let mut members = HashSet::new();
                !valid_local(&group.name)
                    || !names.insert(&group.name)
                    || group.members.is_empty()
                    || group.members.len() >= MAX_SEQUENCE_STEPS
                    || group.members.iter().any(|(name, _)| {
                        name.is_empty()
                            || name.len() > MAX_BRANCH_NAME_BYTES
                            || !name.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                            })
                            || !members.insert(name)
                    })
            })
        {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        let source = format!("fn main() = {}", source(self));
        let formatted = format_syntax(&parse(&source)).map_err(CanonicalSourceError::Syntax)?;
        let tree = parse(&formatted);
        let function = tree
            .function
            .ok_or(CanonicalSourceError::RoundTripMismatch)?;
        let mut visited = scope_len;
        let mut bindings = scope
            .iter()
            .map(|(name, ty)| (name.as_str(), LocalType::from(*ty)))
            .chain(groups.iter().map(|group| {
                (
                    group.name.as_str(),
                    LocalType {
                        ty: Type::Structured,
                        members: Some(group.members.clone()),
                    },
                )
            }))
            .collect();
        let mut lexical = ScopeFrame::new(&mut bindings);
        let (roundtrip, ty) =
            lower_expression(&function.body, &mut lexical, &mut visited, scope_len)
                .map_err(CanonicalSourceError::InvalidEffect)?;
        if roundtrip != *self {
            return Err(CanonicalSourceError::RoundTripMismatch);
        }
        Ok(ty)
    }

    pub(crate) fn children(&self) -> impl Iterator<Item = &Self> {
        let children: [Option<&Self>; 3] = match self {
            Self::Binary { left, right, .. } => [Some(left), Some(right), None],
            Self::Unary { value, .. } | Self::Field { value, .. } => [Some(value), None, None],
            Self::Bind { value, body, .. } => [Some(value), Some(body), None],
            Self::Recover { value, fallback } => [Some(value), Some(fallback), None],
            Self::Loop {
                initial,
                condition,
                next,
                ..
            } => [Some(initial), Some(condition), Some(next)],
            Self::Fold {
                items,
                initial,
                next,
                ..
            } => [Some(items), Some(initial), Some(next)],
            Self::Choose {
                when,
                then,
                otherwise,
            } => [Some(when), Some(then), Some(otherwise)],
            Self::Literal { .. }
            | Self::Strings { .. }
            | Self::Local { .. }
            | Self::Member { .. }
            | Self::Host { .. }
            | Self::Call { .. }
            | Self::Group { .. } => [None, None, None],
        };
        let arguments: &[ComputedArgument] = match self {
            Self::Call { arguments, .. } => arguments,
            _ => &[],
        };
        let branches: &[ComputedBranch] = match self {
            Self::Group { branches, .. } => branches,
            _ => &[],
        };
        children
            .into_iter()
            .flatten()
            .chain(arguments.iter().map(|argument| &argument.value))
            .chain(branches.iter().map(|branch| &branch.value))
            .chain(
                match self {
                    Self::Strings { items } => items.as_slice(),
                    _ => &[],
                }
                .iter(),
            )
    }

    /// Includes cold branches. Validate bounded structure before inspecting authority.
    pub fn required_capabilities(&self) -> BTreeSet<&'static str> {
        let mut required = BTreeSet::new();
        let mut pending = vec![self];
        while let Some(expression) = pending.pop() {
            match expression {
                Self::Host { effect } => required.extend(required_capabilities_for_effect(effect)),
                Self::Call { operation, .. } => {
                    required.insert(operation.required_capability());
                }
                _ => {}
            }
            pending.extend(expression.children());
        }
        required
    }
}

fn has_computed_arguments(arguments: &[NamedArgument]) -> bool {
    arguments.iter().any(|argument| {
        matches!(
            argument.value,
            Expression::Call { .. } | Expression::Reference { .. }
        )
    })
}

pub(crate) fn is_computation(expression: &Expression) -> bool {
    let mut pending = vec![(expression, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        if depth > MAX_EFFECT_NESTING_DEPTH {
            return true;
        }
        let Expression::Call {
            callee, arguments, ..
        } = expression
        else {
            return true;
        };
        if matches!(
            callee.as_str(),
            "bind" | "loop" | "fold" | "strings" | "choose" | "recover" | "field" | "member"
        ) || BinaryOperator::parse(callee).is_some()
            || UnaryOperator::parse(callee).is_some()
            || (HostOperation::parse(callee).is_some() && has_computed_arguments(arguments))
        {
            return true;
        }
        match callee.as_str() {
            "seq" | "all" => pending.extend(
                arguments
                    .iter()
                    .map(|argument| (&argument.value, depth + 1)),
            ),
            "repeat" => pending.extend(
                arguments
                    .iter()
                    .filter(|argument| argument.name == "body")
                    .map(|argument| (&argument.value, depth + 1)),
            ),
            _ => {}
        }
    }
    false
}

fn invalid(code: &str, message: impl Into<String>, span: Span) -> Vec<Diagnostic> {
    vec![Diagnostic {
        code: code.to_string(),
        message: message.into(),
        span: Some(span),
    }]
}

pub(super) fn lower_computation(expression: &Expression) -> Result<LoweredEffect, Vec<Diagnostic>> {
    lower_computation_with_functions(expression, &mut functions::FunctionTemplates::default())
}

pub(super) fn lower_computation_with_functions(
    expression: &Expression,
    functions: &mut functions::FunctionTemplates,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let (expression, result_type) =
        lower_expression_with_functions(expression, &mut scope, &mut 0, 0, functions)?;
    validate_shape(&expression).map_err(|error| match error {
        CanonicalSourceError::InvalidEffect(errors) => errors,
        _ => invalid(
            "LSH1405",
            "invalid computation shape",
            Span { start: 0, end: 0 },
        ),
    })?;
    let effect = Effect::Compute {
        expression: Box::new(expression),
    };
    let required_capabilities = required_capabilities_for_effect(&effect)
        .into_iter()
        .map(str::to_string)
        .collect();
    Ok(LoweredEffect {
        effect,
        result_type,
        required_capabilities,
    })
}

fn named<'a>(
    arguments: &'a [NamedArgument],
    names: &[&str],
    span: Span,
) -> Result<Vec<&'a Expression>, Vec<Diagnostic>> {
    if arguments.len() != names.len()
        || names
            .iter()
            .any(|name| arguments.iter().filter(|arg| arg.name == *name).count() != 1)
    {
        return Err(invalid(
            "LSH1401",
            format!("expected exactly the named arguments: {}", names.join(", ")),
            span,
        ));
    }
    Ok(names
        .iter()
        .filter_map(|name| {
            arguments
                .iter()
                .find(|arg| arg.name == *name)
                .map(|arg| &arg.value)
        })
        .collect())
}

fn scalar(
    value: &Computation,
    result_type: Type,
    span: Span,
) -> Result<ScalarType, Vec<Diagnostic>> {
    match result_type {
        Type::Scalar(scalar) if value.is_pure() => Ok(scalar),
        _ => Err(invalid(
            "LSH1402",
            "expected a pure scalar expression, not a host operation",
            span,
        )),
    }
}

pub(super) fn lower_expression<'a>(
    expression: &'a Expression,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    lower_expression_with_functions(
        expression,
        scope,
        visited,
        depth,
        &mut functions::FunctionTemplates::default(),
    )
}

pub(super) fn lower_expression_with_functions<'a>(
    expression: &'a Expression,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    *visited += 1;
    let span = expression_span(expression);
    if *visited > MAX_COMPUTATION_NODES || depth > MAX_EFFECT_NESTING_DEPTH {
        return Err(invalid(
            "LSH1405",
            "computation exceeds its node or nesting limit",
            span,
        ));
    }
    let literal = match expression {
        Expression::Integer { value, .. } => Some(ScalarValue::Integer(*value)),
        Expression::Boolean { value, .. } => Some(ScalarValue::Boolean(*value)),
        Expression::String { value, .. } => {
            if value.len() > MAX_SCALAR_STRING_BYTES {
                return Err(invalid("LSH1405", "scalar string exceeds 4096 bytes", span));
            }
            Some(ScalarValue::String(value.clone()))
        }
        Expression::None { .. } => Some(ScalarValue::None),
        _ => None,
    };
    if let Some(value) = literal {
        let result_type = Type::Scalar(value.scalar_type());
        return Ok((Computation::Literal { value }, result_type));
    }
    if let Expression::Reference { name, .. } = expression {
        return scope
            .get(name)
            .map(|ty| (Computation::Local { name: name.clone() }, ty.ty))
            .ok_or_else(|| invalid("LSH1403", format!("undefined local '{name}'"), span));
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(invalid("LSH1401", "invalid computation", span));
    };
    if matches!(callee.as_str(), "seq" | "repeat" | "all") {
        return computed_group::lower(callee, arguments, span, scope, visited, depth, functions);
    }
    if callee == "loop" {
        return lower_loop(arguments, span, scope, visited, depth, functions);
    }
    if callee == "fold" {
        return lower_fold(arguments, span, scope, visited, depth, functions);
    }
    if callee == "strings" {
        if arguments.len() > MAX_STRING_LIST_ITEMS {
            return Err(invalid("LSH1405", "string list exceeds 64 entries", span));
        }
        let mut names = HashSet::new();
        let mut items = Vec::with_capacity(arguments.len());
        for argument in arguments {
            if !valid_local(&argument.name) || !names.insert(&argument.name) {
                return Err(invalid(
                    "LSH1403",
                    "string list labels must be bounded and unique",
                    argument.span,
                ));
            }
            let (value, ty) = lower_expression_with_functions(
                &argument.value,
                scope,
                visited,
                depth + 1,
                functions,
            )?;
            if scalar(&value, ty, argument.span)? != ScalarType::String {
                return Err(invalid(
                    "LSH1402",
                    "string list entries require strings",
                    argument.span,
                ));
            }
            items.push(value);
        }
        let literal = items
            .iter()
            .map(|item| match item {
                Computation::Literal {
                    value: ScalarValue::String(value),
                } => Some(value.clone()),
                _ => None,
            })
            .collect::<Option<Vec<_>>>();
        let expression = if let Some(items) = literal {
            let value = StringListValue(items);
            if !value.is_bounded() {
                return Err(invalid("LSH1405", "string list exceeds 4096 bytes", span));
            }
            Computation::Literal {
                value: ScalarValue::StringList(value),
            }
        } else {
            Computation::Strings { items }
        };
        return Ok((expression, Type::Scalar(ScalarType::StringList)));
    }
    if callee == "bind" {
        if arguments.len() != 2 || arguments.iter().filter(|arg| arg.name == "body").count() != 1 {
            return Err(invalid(
                "LSH1401",
                "bind requires one named value and 'body'",
                span,
            ));
        }
        let binding = arguments
            .iter()
            .find(|arg| arg.name != "body")
            .ok_or_else(|| invalid("LSH1401", "bind requires a local name", span))?;
        if !valid_local(&binding.name) || scope.get(&binding.name).is_some() {
            return Err(invalid(
                "LSH1403",
                "local name must be bounded and cannot shadow an active binding",
                binding.span,
            ));
        }
        let (value, value_type) =
            lower_expression_with_functions(&binding.value, scope, visited, depth + 1, functions)?;
        let captures_result = !value.is_pure();
        let direct_function_result = captures_result
            && matches!(&binding.value,
            Expression::Call { callee, .. } if functions.contains(callee));
        let members = group_members(&value, scope);
        let captures_group = captures_result && members.is_some();
        let atomic = value
            .prepared_atomic_operation()
            .is_some_and(|operation| operation.result_type() == value_type);
        let function_result = direct_function_result
            || (captures_result
                && !atomic
                && !captures_group
                && matches!(value_type, Type::Scalar(_))
                && crate::function_flow::is_data_call_selection(&binding.value, &value, functions));
        if captures_result && !atomic && !captures_group && !function_result {
            return Err(invalid(
                "LSH1408",
                "result binding requires one prepared atomic operation or a flat named group",
                binding.span,
            ));
        }
        let body = &arguments
            .iter()
            .find(|arg| arg.name == "body")
            .ok_or_else(|| invalid("LSH1401", "missing bind body", span))?
            .value;
        let (body, result_type) = {
            let mut local = scope.nested();
            local
                .push(
                    &binding.name,
                    LocalType {
                        ty: value_type,
                        members,
                    },
                )
                .map_err(|_| {
                    invalid(
                        "LSH1403",
                        "local name must be bounded and cannot shadow an active binding",
                        binding.span,
                    )
                })?;
            lower_expression_with_functions(body, &mut local, visited, depth + 1, functions)?
        };
        if captures_group
            && !((body.is_pure() && matches!(result_type, Type::Scalar(_)))
                || body.is_atomic_tail()
                || (body.is_result_flow() && matches!(result_type, Type::Scalar(_))))
        {
            return Err(invalid(
                "LSH1412",
                "a captured group requires a pure scalar body, one atomic tail, or a bounded scalar result flow",
                span,
            ));
        }
        if captures_group
            && !body.is_pure()
            && group_members(&value, scope).is_none_or(|members| {
                body.atomic_flow_bound()
                    .is_none_or(|bound| members.len().saturating_add(bound) > MAX_SEQUENCE_STEPS)
            })
        {
            return Err(invalid(
                "LSH1412",
                "a result-driven group and its longest atomic chain must fit in 64 steps",
                span,
            ));
        }
        if captures_result
            && !((body.is_result_flow() && matches!(result_type, Type::Scalar(_)))
                || body.is_result_chain())
        {
            return Err(invalid(
                "LSH1408",
                "a captured host result requires a pure scalar body or a bounded atomic result chain",
                span,
            ));
        }
        let expression = if function_result {
            crate::function_flow::bind_result(binding.name.clone(), value, body, span)?
        } else {
            Computation::Bind {
                name: binding.name.clone(),
                value: Box::new(value),
                body: Box::new(body),
            }
        };
        return Ok((expression, result_type));
    }
    if callee == "member" {
        let args = named(arguments, &["value", "name"], span)?;
        let (Expression::Reference { name: group, .. }, Expression::String { value: name, .. }) =
            (args[0], args[1])
        else {
            return Err(invalid(
                "LSH1412",
                "member requires a bound group reference and a literal step name",
                span,
            ));
        };
        let operation = scope
            .get(group)
            .and_then(|ty| ty.members.as_ref())
            .and_then(|members| members.iter().find(|(member, _)| member == name))
            .map(|(_, operation)| *operation)
            .ok_or_else(|| {
                invalid(
                    "LSH1412",
                    "member is not exported by this bound group",
                    span,
                )
            })?;
        return Ok((
            Computation::Member {
                group: group.clone(),
                name: name.clone(),
                operation,
            },
            operation.result_type(),
        ));
    }
    if callee == "field" {
        let args = named(arguments, &["value", "name"], span)?;
        let (value, input_type) =
            lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
        let Expression::String { value: name, .. } = args[1] else {
            return Err(invalid(
                "LSH1409",
                "field name must be a string literal",
                expression_span(args[1]),
            ));
        };
        let field = ResultField::parse(name)
            .ok_or_else(|| invalid("LSH1409", "unknown result field", span))?;
        let result_type = field
            .result_type(input_type)
            .filter(|_| value.is_pure())
            .ok_or_else(|| {
                invalid(
                    "LSH1409",
                    "field is not exported by this bound result type",
                    span,
                )
            })?;
        return Ok((
            Computation::Field {
                value: Box::new(value),
                field,
            },
            Type::Scalar(result_type),
        ));
    }
    if callee == "choose" {
        let args = named(arguments, &["when", "then", "otherwise"], span)?;
        let (when, when_type) =
            lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
        if when_type != Type::Scalar(ScalarType::Boolean) || !when.is_pure() {
            return Err(invalid(
                "LSH1402",
                "choose when requires a boolean",
                expression_span(args[0]),
            ));
        }
        let (then, then_type) =
            lower_expression_with_functions(args[1], scope, visited, depth + 1, functions)?;
        let (otherwise, otherwise_type) =
            lower_expression_with_functions(args[2], scope, visited, depth + 1, functions)?;
        if then_type != otherwise_type {
            return Err(invalid(
                "LSH1404",
                "choose branches must have the same result type",
                span,
            ));
        }
        return Ok((
            Computation::Choose {
                when: Box::new(when),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            then_type,
        ));
    }
    if callee == "recover" {
        let args = named(arguments, &["value", "fallback"], span)?;
        let (value, value_type) =
            lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
        let (fallback, fallback_type) =
            lower_expression_with_functions(args[1], scope, visited, depth + 1, functions)?;
        if !matches!(value_type, Type::Scalar(_))
            || value_type != fallback_type
            || !value.is_pure()
            || !fallback.is_pure()
        {
            return Err(invalid(
                "LSH1411",
                "recover value and fallback must be pure expressions of the same scalar type",
                span,
            ));
        }
        return Ok((
            Computation::Recover {
                value: Box::new(value),
                fallback: Box::new(fallback),
            },
            value_type,
        ));
    }
    if let Some(operator) = BinaryOperator::parse(callee) {
        let args = named(arguments, &["left", "right"], span)?;
        let (left, left_type) =
            lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
        let (right, right_type) =
            lower_expression_with_functions(args[1], scope, visited, depth + 1, functions)?;
        let left_type = scalar(&left, left_type, expression_span(args[0]))?;
        let right_type = scalar(&right, right_type, expression_span(args[1]))?;
        let result_type = operator.result_type(left_type, right_type).ok_or_else(|| {
            invalid(
                "LSH1402",
                format!(
                    "{} does not accept {left_type:?} and {right_type:?}",
                    operator.name()
                ),
                span,
            )
        })?;
        return Ok((
            Computation::Binary {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            },
            Type::Scalar(result_type),
        ));
    }
    if let Some(operator) = UnaryOperator::parse(callee) {
        let args = named(arguments, &["value"], span)?;
        let (value, value_type) =
            lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
        let output = unary_type(operator, value_type)
            .filter(|_| value.is_pure())
            .ok_or_else(|| {
                invalid(
                    "LSH1402",
                    format!("{callee} requires a pure {}", operator.expected_input()),
                    span,
                )
            })?;
        if operator == UnaryOperator::OptionalString
            && let Computation::Literal { value } = &value
        {
            let text = match value {
                ScalarValue::String(text) => Some(text.clone()),
                ScalarValue::None => None,
                _ => {
                    return Err(invalid(
                        "LSH1402",
                        "optional_string requires string or none",
                        span,
                    ));
                }
            };
            return Ok((
                Computation::Literal {
                    value: ScalarValue::OptionalString(OptionalStringValue(text)),
                },
                Type::Scalar(output),
            ));
        }
        return Ok((
            Computation::Unary {
                operator,
                value: Box::new(value),
            },
            Type::Scalar(output),
        ));
    }
    if functions.contains(callee) {
        return functions::lower_call(callee, arguments, span, scope, visited, depth, functions);
    }
    if let Some(operation) = HostOperation::parse(callee)
        && has_computed_arguments(arguments)
    {
        if arguments.len() > operation.parameters().len() {
            return Err(invalid_argument("too many host arguments", Some(span)));
        }
        operation.validate_names(
            &arguments
                .iter()
                .map(|arg| arg.name.as_str())
                .collect::<Vec<_>>(),
            Some(span),
        )?;
        let mut lowered = Vec::with_capacity(arguments.len());
        for parameter in operation.parameters() {
            let Some(argument) = arguments.iter().find(|arg| arg.name == parameter.name) else {
                continue;
            };
            let (value, ty) = lower_expression_with_functions(
                &argument.value,
                scope,
                visited,
                depth + 1,
                functions,
            )?;
            if !parameter.domain.accepts(scalar(&value, ty, argument.span)?)
                || matches!(&value, Computation::Literal { value } if !parameter.domain.validate(value))
            {
                return Err(invalid_argument(
                    format!("invalid {} argument '{}'", operation.name(), parameter.name),
                    Some(argument.span),
                ));
            }
            lowered.push(ComputedArgument {
                name: argument.name.clone(),
                value,
            });
        }
        return Ok((
            Computation::Call {
                operation,
                arguments: lowered,
            },
            operation.result_type(),
        ));
    }
    let lowered = lower_effect(expression)?;
    if contains_computation(&lowered.effect) {
        return Err(invalid(
            "LSH1406",
            "computation must wrap a host group, not occur inside its branches",
            span,
        ));
    }
    Ok((
        Computation::Host {
            effect: Box::new(lowered.effect),
        },
        lowered.result_type,
    ))
}

fn lower_loop<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let reserved = ["while", "next", "limit"];
    let binding = arguments
        .iter()
        .find(|argument| !reserved.contains(&argument.name.as_str()))
        .ok_or_else(|| invalid("LSH1410", "loop requires a named initial state", span))?;
    let args = named(
        arguments,
        &[binding.name.as_str(), "while", "next", "limit"],
        span,
    )?;
    if !valid_local(&binding.name) || scope.get(&binding.name).is_some() {
        return Err(invalid(
            "LSH1403",
            "loop state name must be bounded and cannot shadow an active binding",
            binding.span,
        ));
    }
    let Expression::Integer { value: limit, .. } = args[3] else {
        return Err(invalid(
            "LSH1410",
            "loop limit must be an integer literal from 0 through 1024",
            expression_span(args[3]),
        ));
    };
    if *limit > MAX_LOOP_ITERATIONS {
        return Err(invalid(
            "LSH1410",
            "loop limit exceeds 1024 iterations",
            expression_span(args[3]),
        ));
    }
    let (initial, state_type) =
        lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
    scalar(&initial, state_type, expression_span(args[0]))?;
    let (condition, next) = {
        let mut local = scope.nested();
        local.push(&binding.name, state_type.into()).map_err(|_| {
            invalid(
                "LSH1403",
                "loop state name must be bounded and cannot shadow an active binding",
                binding.span,
            )
        })?;
        let (condition, condition_type) =
            lower_expression_with_functions(args[1], &mut local, visited, depth + 1, functions)?;
        if scalar(&condition, condition_type, expression_span(args[1]))? != ScalarType::Boolean {
            return Err(invalid(
                "LSH1402",
                "loop while requires a boolean",
                expression_span(args[1]),
            ));
        }
        let (next, next_type) =
            lower_expression_with_functions(args[2], &mut local, visited, depth + 1, functions)?;
        scalar(&next, next_type, expression_span(args[2]))?;
        if next_type != state_type {
            return Err(invalid(
                "LSH1402",
                "loop next must preserve the initial state's scalar type",
                expression_span(args[2]),
            ));
        }
        (condition, next)
    };
    Ok((
        Computation::Loop {
            name: binding.name.clone(),
            initial: Box::new(initial),
            condition: Box::new(condition),
            next: Box::new(next),
            limit: *limit,
        },
        state_type,
    ))
}

fn lower_fold<'a>(
    arguments: &'a [NamedArgument],
    span: Span,
    scope: &mut TypeScope<'_, 'a>,
    visited: &mut usize,
    depth: usize,
    functions: &mut functions::FunctionTemplates,
) -> Result<(Computation, Type), Vec<Diagnostic>> {
    let reserved = ["items", "item", "next", "limit"];
    let binding = arguments
        .iter()
        .find(|argument| !reserved.contains(&argument.name.as_str()))
        .ok_or_else(|| invalid("LSH1413", "fold requires a named initial state", span))?;
    let args = named(
        arguments,
        &[binding.name.as_str(), "items", "item", "next", "limit"],
        span,
    )?;
    let Expression::String { value: item, .. } = args[2] else {
        return Err(invalid(
            "LSH1413",
            "fold item requires a literal local name",
            expression_span(args[2]),
        ));
    };
    if !valid_local(&binding.name)
        || !valid_local(item)
        || item == &binding.name
        || scope.get(&binding.name).is_some()
        || scope.get(item).is_some()
    {
        return Err(invalid(
            "LSH1403",
            "fold locals must be bounded, distinct and cannot shadow active bindings",
            span,
        ));
    }
    let Expression::Integer { value: limit, .. } = args[4] else {
        return Err(invalid(
            "LSH1413",
            "fold limit requires an integer literal from 0 through 64",
            expression_span(args[4]),
        ));
    };
    if *limit > MAX_STRING_LIST_ITEMS as u64 {
        return Err(invalid(
            "LSH1413",
            "fold limit exceeds 64 entries",
            expression_span(args[4]),
        ));
    }
    // The collection is prepared before the accumulator, independent of argument spelling order.
    let (items, items_type) =
        lower_expression_with_functions(args[1], scope, visited, depth + 1, functions)?;
    if scalar(&items, items_type, expression_span(args[1]))? != ScalarType::StringList {
        return Err(invalid(
            "LSH1402",
            "fold items requires a string_list",
            expression_span(args[1]),
        ));
    }
    let (initial, state_type) =
        lower_expression_with_functions(args[0], scope, visited, depth + 1, functions)?;
    scalar(&initial, state_type, expression_span(args[0]))?;
    let (next, next_type) = {
        let mut local = scope.nested();
        for (name, ty) in [
            (binding.name.as_str(), state_type),
            (item.as_str(), Type::Scalar(ScalarType::String)),
        ] {
            local.push(name, ty.into()).map_err(|_| {
                invalid(
                    "LSH1403",
                    "fold locals must be bounded, distinct and cannot shadow active bindings",
                    span,
                )
            })?;
        }
        lower_expression_with_functions(args[3], &mut local, visited, depth + 1, functions)?
    };
    scalar(&next, next_type, expression_span(args[3]))?;
    if next_type != state_type {
        return Err(invalid(
            "LSH1402",
            "fold next must preserve its initial state's type",
            expression_span(args[3]),
        ));
    }
    Ok((
        Computation::Fold {
            name: binding.name.clone(),
            item: item.clone(),
            items: Box::new(items),
            initial: Box::new(initial),
            next: Box::new(next),
            limit: *limit,
        },
        state_type,
    ))
}

fn unary_type(operator: UnaryOperator, input: Type) -> Option<ScalarType> {
    match input {
        Type::Scalar(input) => operator.result_type(input),
        _ => None,
    }
}

pub(super) fn valid_local(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_BRANCH_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(name, "body" | "fn" | "true" | "false" | "none")
}

pub(super) fn is_builtin(name: &str) -> bool {
    matches!(
        name,
        "all"
            | "seq"
            | "repeat"
            | "bind"
            | "loop"
            | "fold"
            | "strings"
            | "choose"
            | "recover"
            | "field"
            | "member"
    ) || BinaryOperator::parse(name).is_some()
        || UnaryOperator::parse(name).is_some()
}

pub(crate) fn contains_computation(effect: &Effect) -> bool {
    let mut pending = vec![effect];
    while let Some(effect) = pending.pop() {
        match effect {
            Effect::Compute { .. } => return true,
            Effect::All { branches } | Effect::Sequence { steps: branches } => {
                pending.extend(branches.iter().map(|branch| &branch.effect));
            }
            _ => {}
        }
    }
    false
}

pub(super) fn validate_shape(expression: &Computation) -> Result<(), CanonicalSourceError> {
    let mut pending = vec![(expression, 0usize)];
    let mut visited = 0;
    while let Some((expression, depth)) = pending.pop() {
        // Literal constructors retain the source node/depth budget after constant folding.
        let (literal_nodes, literal_depth) = match expression {
            Computation::Literal { value } => source_shape_extra(value),
            _ => (0, 0),
        };
        visited += 1 + literal_nodes;
        let invalid_value =
            match expression {
                Computation::Literal { value } => !value.is_bounded(),
                Computation::Strings { items } => items.len() > MAX_STRING_LIST_ITEMS,
                Computation::Local { name } | Computation::Bind { name, .. } => !valid_local(name),
                Computation::Member { group, name, .. } => {
                    !valid_local(group) || name.len() > MAX_BRANCH_NAME_BYTES
                }
                Computation::Loop { name, limit, .. } => {
                    !valid_local(name)
                        || matches!(name.as_str(), "while" | "next" | "limit")
                        || *limit > MAX_LOOP_ITERATIONS
                }
                Computation::Fold {
                    name, item, limit, ..
                } => {
                    !valid_local(name)
                        || !valid_local(item)
                        || name == item
                        || matches!(name.as_str(), "items" | "item" | "next" | "limit")
                        || *limit > MAX_STRING_LIST_ITEMS as u64
                }
                Computation::Call {
                    operation,
                    arguments,
                } => {
                    arguments.len() > operation.parameters().len()
                        || arguments
                            .iter()
                            .any(|argument| argument.name.len() > MAX_BRANCH_NAME_BYTES)
                }
                Computation::Group {
                    group_kind,
                    branches,
                } => {
                    let minimum = if *group_kind == GroupKind::Sequence {
                        1
                    } else {
                        2
                    };
                    !(minimum..=MAX_ALL_BRANCHES).contains(&branches.len())
                        || branches.iter().any(|branch| {
                            branch.name.len() > MAX_BRANCH_NAME_BYTES
                                || branch.value.prepared_atomic_operation().is_none_or(
                                    |operation| branch.result_type != operation.result_type(),
                                )
                        })
                }
                _ => false,
            };
        if visited > MAX_COMPUTATION_NODES
            || depth.saturating_add(literal_depth) > MAX_EFFECT_NESTING_DEPTH
            || invalid_value
        {
            return Err(CanonicalSourceError::InvalidEffect(invalid(
                "LSH1405",
                "invalid or oversized computation",
                Span { start: 0, end: 0 },
            )));
        }
        if let Computation::Host { effect } = expression {
            let mut effects = vec![(effect.as_ref(), depth)];
            while let Some((effect, effect_depth)) = effects.pop() {
                visited += 1;
                if visited > MAX_COMPUTATION_NODES
                    || effect_depth >= MAX_EFFECT_NESTING_DEPTH
                    || matches!(effect, Effect::Compute { .. })
                {
                    return Err(CanonicalSourceError::InvalidEffect(invalid(
                        "LSH1405",
                        "invalid or oversized computation host graph",
                        Span { start: 0, end: 0 },
                    )));
                }
                if let Effect::All { branches } | Effect::Sequence { steps: branches } = effect {
                    if branches.len() > MAX_ALL_BRANCHES {
                        return Err(CanonicalSourceError::InvalidEffect(invalid(
                            "LSH1405",
                            "oversized computation host group",
                            Span { start: 0, end: 0 },
                        )));
                    }
                    effects.extend(
                        branches
                            .iter()
                            .map(|branch| (&branch.effect, effect_depth + 1)),
                    );
                }
            }
        }
        pending.extend(expression.children().map(|child| (child, depth + 1)));
    }
    Ok(())
}

pub(super) fn source(expression: &Computation) -> String {
    match expression {
        Computation::Literal { value } => match value {
            ScalarValue::Integer(value) => value.to_string(),
            ScalarValue::Boolean(value) => value.to_string(),
            ScalarValue::String(value) => quote(value),
            ScalarValue::None => "none".to_string(),
            ScalarValue::OptionalString(value) => format!(
                "optional_string(value: {})",
                value.0.as_deref().map_or_else(|| "none".into(), quote)
            ),
            ScalarValue::StringList(value) => format!(
                "strings({})",
                value
                    .0
                    .iter()
                    .enumerate()
                    .map(|(index, value)| format!("item{index}: {}", quote(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        },
        Computation::Strings { items } => format!(
            "strings({})",
            items
                .iter()
                .enumerate()
                .map(|(index, value)| format!("item{index}: {}", source(value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Computation::Local { name } => name.clone(),
        Computation::Member { group, name, .. } => {
            format!("member(value: {group}, name: {})", quote(name))
        }
        Computation::Field { value, field } => format!(
            "field(value: {}, name: {})",
            source(value),
            quote(field.name())
        ),
        Computation::Binary {
            operator,
            left,
            right,
        } => format!(
            "{}(left: {}, right: {})",
            operator.name(),
            source(left),
            source(right)
        ),
        Computation::Unary { operator, value } => {
            format!("{}(value: {})", operator.name(), source(value))
        }
        Computation::Recover { value, fallback } => format!(
            "recover(value: {}, fallback: {})",
            source(value),
            source(fallback)
        ),
        Computation::Bind { name, value, body } => {
            format!("bind({name}: {}, body: {})", source(value), source(body))
        }
        Computation::Loop {
            name,
            initial,
            condition,
            next,
            limit,
        } => format!(
            "loop({name}: {}, while: {}, next: {}, limit: {limit})",
            source(initial),
            source(condition),
            source(next)
        ),
        Computation::Fold {
            name,
            item,
            items,
            initial,
            next,
            limit,
        } => format!(
            "fold({name}: {}, items: {}, item: {}, next: {}, limit: {limit})",
            source(initial),
            source(items),
            quote(item),
            source(next)
        ),
        Computation::Choose {
            when,
            then,
            otherwise,
        } => format!(
            "choose(when: {}, then: {}, otherwise: {})",
            source(when),
            source(then),
            source(otherwise)
        ),
        Computation::Host { effect } => canonical_effect_source(effect, 0),
        Computation::Call {
            operation,
            arguments,
        } => format!(
            "{}({})",
            operation.name(),
            arguments
                .iter()
                .map(|argument| format!("{}: {}", argument.name, source(&argument.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Computation::Group {
            group_kind,
            branches,
        } => format!(
            "{}({})",
            if *group_kind == GroupKind::Sequence {
                "seq"
            } else {
                "all"
            },
            branches
                .iter()
                .map(|branch| format!("{}: {}", branch.name, source(&branch.value)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn failed_lowering_cleans_locals_without_copying_parent_names_or_refunding_nodes() {
        for body in [
            "bind(tmp: outer, body: bind(inner: tmp, body: missing))",
            r#"loop(tmp: outer, while: true, next: "bad", limit: 1)"#,
            "loop(tmp: outer, while: 1, next: tmp, limit: 0)",
            r#"fold(tmp: outer, items: strings(a: "x"), item: "entry", next: missing, limit: 1)"#,
            r#"fold(tmp: outer, items: strings(a: "x"), item: "entry", next: entry, limit: 1)"#,
        ] {
            let tree = parse(&format!("fn main() = {body}"));
            let good = parse("fn main() = bind(tmp: outer, body: tmp)");
            let name = String::from("outer");
            let mut bindings = vec![(name.as_str(), Type::Scalar(ScalarType::Integer).into())];
            let mut scope = ScopeFrame::new(&mut bindings);
            let mut visited = 1;
            assert!(
                lower_expression(
                    &tree.function.as_ref().unwrap().body,
                    &mut scope,
                    &mut visited,
                    1
                )
                .is_err(),
                "{body}"
            );
            let spent = visited;
            assert!(spent > 1);
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.local_len(), 0);
            assert_eq!(scope.bindings()[0].0.as_ptr(), name.as_ptr());
            assert_eq!(
                scope.get("outer").unwrap().ty,
                Type::Scalar(ScalarType::Integer)
            );
            let (_, ty) = lower_expression(
                &good.function.as_ref().unwrap().body,
                &mut scope,
                &mut visited,
                1,
            )
            .unwrap();
            assert_eq!(ty, Type::Scalar(ScalarType::Integer));
            assert_eq!(visited, spent + 3);
            assert_eq!(scope.len(), 1);
            assert_eq!(scope.local_len(), 0);
        }
    }
}
