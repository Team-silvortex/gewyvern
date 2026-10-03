use super::*;
use crate::host_call::{HostOperation, invalid_argument};
use crate::result_field::ResultField;
use leselang_syntax::NamedArgument;

pub const MAX_COMPUTATION_NODES: usize = 1_024;
pub const MAX_SCALAR_STRING_BYTES: usize = 4_096;
pub const MAX_LOOP_ITERATIONS: u64 = 1_024;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalarType {
    Integer,
    Boolean,
    String,
    None,
    OptionalString,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OptionalStringValue(pub Option<String>);

impl<'de> Deserialize<'de> for OptionalStringValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = OptionalStringValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an explicit null or bounded string")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(OptionalStringValue(None))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                if value.len() > MAX_SCALAR_STRING_BYTES {
                    return Err(E::custom("optional string exceeds 4096 bytes"));
                }
                self.visit_string(value.to_owned())
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                if value.len() > MAX_SCALAR_STRING_BYTES {
                    return Err(E::custom("optional string exceeds 4096 bytes"));
                }
                Ok(OptionalStringValue(Some(value)))
            }
        }
        // deserialize_any rejects a missing enum payload instead of treating it as null.
        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum ScalarValue {
    Integer(u64),
    Boolean(bool),
    String(String),
    None,
    OptionalString(OptionalStringValue),
}

impl ScalarValue {
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Integer(_) => ScalarType::Integer,
            Self::Boolean(_) => ScalarType::Boolean,
            Self::String(_) => ScalarType::String,
            Self::None => ScalarType::None,
            Self::OptionalString(_) => ScalarType::OptionalString,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            Self::OptionalString(value) => value.0.as_deref(),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BinaryOperator {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
    Concat,
    ValueOr,
    Contains,
    StartsWith,
    EndsWith,
    CharAt,
}

impl BinaryOperator {
    pub fn name(self) -> &'static str {
        match self {
            Self::Add => "add",
            Self::Sub => "sub",
            Self::Mul => "mul",
            Self::Div => "div",
            Self::Rem => "rem",
            Self::Eq => "eq",
            Self::Ne => "ne",
            Self::Lt => "lt",
            Self::Le => "le",
            Self::Gt => "gt",
            Self::Ge => "ge",
            Self::And => "and",
            Self::Or => "or",
            Self::Concat => "concat",
            Self::ValueOr => "value_or",
            Self::Contains => "contains",
            Self::StartsWith => "starts_with",
            Self::EndsWith => "ends_with",
            Self::CharAt => "char_at",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [
            Self::Add,
            Self::Sub,
            Self::Mul,
            Self::Div,
            Self::Rem,
            Self::Eq,
            Self::Ne,
            Self::Lt,
            Self::Le,
            Self::Gt,
            Self::Ge,
            Self::And,
            Self::Or,
            Self::Concat,
            Self::ValueOr,
            Self::Contains,
            Self::StartsWith,
            Self::EndsWith,
            Self::CharAt,
        ]
        .into_iter()
        .find(|op| op.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UnaryOperator {
    Not,
    Len,
    ToString,
    ParseInteger,
    ParseBoolean,
    OptionalString,
    HasValue,
}

impl UnaryOperator {
    pub fn name(self) -> &'static str {
        match self {
            Self::Not => "not",
            Self::Len => "len",
            Self::ToString => "to_string",
            Self::ParseInteger => "parse_integer",
            Self::ParseBoolean => "parse_boolean",
            Self::OptionalString => "optional_string",
            Self::HasValue => "has_value",
        }
    }

    fn parse(name: &str) -> Option<Self> {
        [
            Self::Not,
            Self::Len,
            Self::ToString,
            Self::ParseInteger,
            Self::ParseBoolean,
            Self::OptionalString,
            Self::HasValue,
        ]
        .into_iter()
        .find(|operator| operator.name() == name)
    }

    fn result_type(self, input: Type) -> Option<ScalarType> {
        use ScalarType::{Boolean, Integer, String};
        match (self, input) {
            (Self::OptionalString, Type::Scalar(String | ScalarType::None)) => {
                Some(ScalarType::OptionalString)
            }
            (Self::HasValue, Type::Scalar(ScalarType::OptionalString)) => Some(Boolean),
            (Self::Not, Type::Scalar(Boolean)) | (Self::ParseBoolean, Type::Scalar(String)) => {
                Some(Boolean)
            }
            (Self::Len | Self::ParseInteger, Type::Scalar(String)) => Some(Integer),
            (Self::ToString, Type::Scalar(Integer | Boolean | String)) => Some(String),
            _ => None,
        }
    }

    fn expected_input(self) -> &'static str {
        match self {
            Self::Not => "boolean",
            Self::Len | Self::ParseInteger | Self::ParseBoolean => "string",
            Self::ToString => "integer, boolean or string",
            Self::OptionalString => "string or none",
            Self::HasValue => "optional_string",
        }
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

#[derive(Clone)]
pub(super) struct LocalType {
    ty: Type,
    members: Option<Vec<(String, HostOperation)>>,
}

impl From<Type> for LocalType {
    fn from(ty: Type) -> Self {
        Self { ty, members: None }
    }
}

fn group_members(
    expression: &Computation,
    scope: &[(String, LocalType)],
) -> Option<Vec<(String, HostOperation)>> {
    match expression {
        Computation::Host { effect } => match effect.as_ref() {
            Effect::All { branches } | Effect::Sequence { steps: branches } => branches
                .iter()
                .map(|branch| {
                    Some((
                        branch.name.clone(),
                        HostOperation::for_effect(&branch.effect)?,
                    ))
                })
                .collect(),
            _ => None,
        },
        Computation::Group { branches, .. } => branches
            .iter()
            .map(|branch| {
                let operation = match &branch.value {
                    Computation::Host { effect } => HostOperation::for_effect(effect)?,
                    Computation::Call { operation, .. } => *operation,
                    _ => return None,
                };
                Some((branch.name.clone(), operation))
            })
            .collect(),
        Computation::Local { name } => scope
            .iter()
            .find(|(bound, _)| bound == name)?
            .1
            .members
            .clone(),
        _ => None,
    }
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

    /// One captured atomic suspension followed by a pure body, with pure preparation/selection.
    pub fn is_atomic_capture(&self) -> bool {
        match self {
            Self::Bind { value, body, .. } if value.is_pure() => body.is_atomic_capture(),
            Self::Bind { value, body, .. } => {
                matches!(value.as_ref(), Self::Host { .. } | Self::Call { .. })
                    && value.is_atomic_tail()
                    && body.is_pure()
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
                    matches!(value.as_ref(), Self::Host { .. } | Self::Call { .. })
                        && value.is_atomic_tail()
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
            Self::Bind { value, body, .. } if value.is_atomic_tail() => {
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
                (value.is_pure()
                    || (matches!(value.as_ref(), Self::Host { .. } | Self::Call { .. })
                        && value.is_atomic_tail()))
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
        let (roundtrip, ty) = lower_expression(
            &function.body,
            &mut scope
                .iter()
                .map(|(name, ty)| (name.clone(), LocalType::from(*ty)))
                .chain(groups.iter().map(|group| {
                    (
                        group.name.clone(),
                        LocalType {
                            ty: Type::Structured,
                            members: Some(group.members.clone()),
                        },
                    )
                }))
                .collect(),
            &mut visited,
            scope_len,
        )
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
            Self::Choose {
                when,
                then,
                otherwise,
            } => [Some(when), Some(then), Some(otherwise)],
            Self::Literal { .. }
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
            "bind" | "loop" | "choose" | "recover" | "field" | "member"
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
    let (expression, result_type) =
        lower_expression_with_functions(expression, &mut Vec::new(), &mut 0, 0, functions)?;
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

pub(super) fn lower_expression(
    expression: &Expression,
    scope: &mut Vec<(String, LocalType)>,
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

pub(super) fn lower_expression_with_functions(
    expression: &Expression,
    scope: &mut Vec<(String, LocalType)>,
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
            .iter()
            .find(|(bound, _)| bound == name)
            .map(|(_, ty)| (Computation::Local { name: name.clone() }, ty.ty))
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
        if !valid_local(&binding.name) || scope.iter().any(|(name, _)| name == &binding.name) {
            return Err(invalid(
                "LSH1403",
                "local name must be bounded and cannot shadow an active binding",
                binding.span,
            ));
        }
        let (value, value_type) =
            lower_expression_with_functions(&binding.value, scope, visited, depth + 1, functions)?;
        let captures_result = !value.is_pure();
        let members = group_members(&value, scope);
        let captures_group = captures_result && members.is_some();
        let atomic = match &value {
            Computation::Call { .. } => true,
            Computation::Host { effect } => HostOperation::for_effect(effect).is_some(),
            _ => false,
        };
        if captures_result && !atomic && !captures_group {
            return Err(invalid(
                "LSH1408",
                "result binding requires an atomic host call or a flat named group",
                binding.span,
            ));
        }
        let body = &arguments
            .iter()
            .find(|arg| arg.name == "body")
            .ok_or_else(|| invalid("LSH1401", "missing bind body", span))?
            .value;
        scope.push((
            binding.name.clone(),
            LocalType {
                ty: value_type,
                members,
            },
        ));
        let lowered_body =
            lower_expression_with_functions(body, scope, visited, depth + 1, functions);
        scope.pop();
        let (body, result_type) = lowered_body?;
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
        return Ok((
            Computation::Bind {
                name: binding.name.clone(),
                value: Box::new(value),
                body: Box::new(body),
            },
            result_type,
        ));
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
            .iter()
            .find(|(bound, _)| bound == group)
            .and_then(|(_, ty)| ty.members.as_ref())
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
        let result_type = binary_type(operator, left_type, right_type).ok_or_else(|| {
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
        let output = operator
            .result_type(value_type)
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

fn lower_loop(
    arguments: &[NamedArgument],
    span: Span,
    scope: &mut Vec<(String, LocalType)>,
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
    if !valid_local(&binding.name) || scope.iter().any(|(name, _)| name == &binding.name) {
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
    scope.push((binding.name.clone(), state_type.into()));
    let lowered = (|| {
        let (condition, condition_type) =
            lower_expression_with_functions(args[1], scope, visited, depth + 1, functions)?;
        if scalar(&condition, condition_type, expression_span(args[1]))? != ScalarType::Boolean {
            return Err(invalid(
                "LSH1402",
                "loop while requires a boolean",
                expression_span(args[1]),
            ));
        }
        let (next, next_type) =
            lower_expression_with_functions(args[2], scope, visited, depth + 1, functions)?;
        scalar(&next, next_type, expression_span(args[2]))?;
        if next_type != state_type {
            return Err(invalid(
                "LSH1402",
                "loop next must preserve the initial state's scalar type",
                expression_span(args[2]),
            ));
        }
        Ok((condition, next))
    })();
    scope.pop();
    let (condition, next) = lowered?;
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

fn binary_type(op: BinaryOperator, left: ScalarType, right: ScalarType) -> Option<ScalarType> {
    use BinaryOperator::*;
    use ScalarType::{Boolean, Integer, String};
    if op == ValueOr {
        return (left == ScalarType::OptionalString && right == String).then_some(String);
    }
    if op == CharAt {
        return (left == String && right == Integer).then_some(ScalarType::OptionalString);
    }
    if left != right {
        return None;
    }
    match (op, left) {
        (Eq | Ne, _) => Some(Boolean),
        (And | Or, Boolean) => Some(Boolean),
        (Concat, String) => Some(String),
        (Contains | StartsWith | EndsWith, String) => Some(Boolean),
        (Add | Sub | Mul | Div | Rem, Integer) => Some(Integer),
        (Lt | Le | Gt | Ge, Integer) => Some(Boolean),
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
        "all" | "seq" | "repeat" | "bind" | "loop" | "choose" | "recover" | "field" | "member"
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
        // Folded optional literals still render as a constructor plus its payload.
        let literal_extra = usize::from(matches!(
            expression,
            Computation::Literal {
                value: ScalarValue::OptionalString(_)
            }
        ));
        visited += 1 + literal_extra;
        let invalid_value = match expression {
            Computation::Literal { value } => value
                .text()
                .is_some_and(|value| value.len() > MAX_SCALAR_STRING_BYTES),
            Computation::Local { name } | Computation::Bind { name, .. } => !valid_local(name),
            Computation::Member { group, name, .. } => {
                !valid_local(group) || name.len() > MAX_BRANCH_NAME_BYTES
            }
            Computation::Loop { name, limit, .. } => {
                !valid_local(name)
                    || matches!(name.as_str(), "while" | "next" | "limit")
                    || *limit > MAX_LOOP_ITERATIONS
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
                            || match &branch.value {
                                Computation::Call { operation, .. } => {
                                    branch.result_type != operation.result_type()
                                }
                                Computation::Host { effect } => HostOperation::for_effect(effect)
                                    .is_none_or(|operation| {
                                        branch.result_type != operation.result_type()
                                    }),
                                _ => true,
                            }
                    })
            }
            _ => false,
        };
        if visited > MAX_COMPUTATION_NODES
            || depth.saturating_add(literal_extra) > MAX_EFFECT_NESTING_DEPTH
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
        },
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
