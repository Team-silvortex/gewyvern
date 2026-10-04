use serde::{Deserialize, Serialize};

use crate::{
    MAX_SCALAR_STRING_BYTES, OptionalStringValue, ScalarType, ScalarValue, StringListBuilder,
};

/// Closed scalar operations; tags and names retain the original HIR wire spelling.
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
    Split,
    Join,
    Append,
    ItemAt,
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
            Self::Split => "split",
            Self::Join => "join",
            Self::Append => "append",
            Self::ItemAt => "item_at",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
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
            Self::Split,
            Self::Join,
            Self::Append,
            Self::ItemAt,
        ]
        .into_iter()
        .find(|operator| operator.name() == name)
    }

    /// Pure signature lookup, without evaluating or converting either operand.
    pub fn result_type(self, left: ScalarType, right: ScalarType) -> Option<ScalarType> {
        use BinaryOperator::*;
        use ScalarType::{Boolean, Integer, String};
        match self {
            ValueOr => {
                return (left == ScalarType::OptionalString && right == String).then_some(String);
            }
            CharAt => {
                return (left == String && right == Integer).then_some(ScalarType::OptionalString);
            }
            ItemAt => {
                return (left == ScalarType::StringList && right == Integer)
                    .then_some(ScalarType::OptionalString);
            }
            Append => {
                return (left == ScalarType::StringList && right == String)
                    .then_some(ScalarType::StringList);
            }
            Join => return (left == ScalarType::StringList && right == String).then_some(String),
            _ => {}
        }
        if left != right {
            return None;
        }
        match (self, left) {
            (Eq | Ne, _) => Some(Boolean),
            (And | Or, Boolean) => Some(Boolean),
            (Concat, String) => Some(String),
            (Split, String) => Some(ScalarType::StringList),
            (Contains | StartsWith | EndsWith, String) => Some(Boolean),
            (Add | Sub | Mul | Div | Rem, Integer) => Some(Integer),
            (Lt | Le | Gt | Ge, Integer) => Some(Boolean),
            _ => None,
        }
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

    pub fn parse(name: &str) -> Option<Self> {
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

    pub fn result_type(self, input: ScalarType) -> Option<ScalarType> {
        use ScalarType::{Boolean, Integer, String};
        match (self, input) {
            (Self::OptionalString, String | ScalarType::None) => Some(ScalarType::OptionalString),
            (Self::HasValue, ScalarType::OptionalString) => Some(Boolean),
            (Self::Not, Boolean) | (Self::ParseBoolean, String) => Some(Boolean),
            (Self::Len | Self::ParseInteger, String) | (Self::Len, ScalarType::StringList) => {
                Some(Integer)
            }
            (Self::ToString, Integer | Boolean | String) => Some(String),
            _ => None,
        }
    }

    pub fn expected_input(self) -> &'static str {
        match self {
            Self::Not => "boolean",
            Self::Len => "string or string_list",
            Self::ParseInteger | Self::ParseBoolean => "string",
            Self::ToString => "integer, boolean or string",
            Self::OptionalString => "string or none",
            Self::HasValue => "optional_string",
        }
    }
}

/// Payload-free scalar failure; adapters own diagnostic codes and recovery policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarError {
    TypeMismatch,
    UnboundedOperand,
    IntegerArithmetic,
    InvalidIntegerText,
    InvalidBooleanText,
    StringLimit,
    StringListLimit,
}

impl std::fmt::Display for ScalarError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::TypeMismatch => "scalar operands do not match the operation signature",
            Self::UnboundedOperand => "scalar operand exceeds its text or list bounds",
            Self::IntegerArithmetic => "integer overflow, underflow, or division by zero",
            Self::InvalidIntegerText => "invalid integer text: expected ASCII decimal within u64",
            Self::InvalidBooleanText => "invalid boolean text: expected true or false",
            Self::StringLimit => "computed string exceeds 4096 bytes",
            Self::StringListLimit => "computed string list exceeds 64 entries or 4096 bytes",
        })
    }
}

impl std::error::Error for ScalarError {}

/// Apply one operation to an already evaluated, owned value.
///
/// This validates the operand and bounds output, but does not charge fuel or
/// evaluate an expression. Callers own aggregate work limits and recovery.
///
/// ```
/// use leselang_runtime_core::{ScalarValue, UnaryOperator, apply_unary};
/// assert_eq!(apply_unary(UnaryOperator::ParseInteger, ScalarValue::String("42".into())),
///            Ok(ScalarValue::Integer(42)));
/// ```
pub fn apply_unary(
    operator: UnaryOperator,
    value: ScalarValue,
) -> Result<ScalarValue, ScalarError> {
    if operator.result_type(value.scalar_type()).is_none() {
        return Err(ScalarError::TypeMismatch);
    }
    if !value.is_bounded() {
        return Err(ScalarError::UnboundedOperand);
    }
    match (operator, value) {
        (UnaryOperator::OptionalString, ScalarValue::String(value)) => Ok(
            ScalarValue::OptionalString(OptionalStringValue(Some(value))),
        ),
        (UnaryOperator::OptionalString, ScalarValue::None) => {
            Ok(ScalarValue::OptionalString(OptionalStringValue(None)))
        }
        (UnaryOperator::HasValue, ScalarValue::OptionalString(value)) => {
            Ok(ScalarValue::Boolean(value.0.is_some()))
        }
        (UnaryOperator::Not, ScalarValue::Boolean(value)) => Ok(ScalarValue::Boolean(!value)),
        (UnaryOperator::Len, ScalarValue::String(value)) => {
            Ok(ScalarValue::Integer(value.chars().count() as u64))
        }
        (UnaryOperator::Len, ScalarValue::StringList(value)) => {
            Ok(ScalarValue::Integer(value.0.len() as u64))
        }
        (UnaryOperator::ToString, value) => Ok(ScalarValue::String(match value {
            ScalarValue::Integer(value) => value.to_string(),
            ScalarValue::Boolean(value) => value.to_string(),
            ScalarValue::String(value) => value,
            _ => return Err(ScalarError::TypeMismatch),
        })),
        (UnaryOperator::ParseInteger, ScalarValue::String(value)) => {
            // Rust accepts a leading '+', but the language accepts only ASCII digits.
            if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(ScalarError::InvalidIntegerText);
            }
            value
                .parse::<u64>()
                .map(ScalarValue::Integer)
                .map_err(|_| ScalarError::InvalidIntegerText)
        }
        (UnaryOperator::ParseBoolean, ScalarValue::String(value)) => match value.as_str() {
            "true" => Ok(ScalarValue::Boolean(true)),
            "false" => Ok(ScalarValue::Boolean(false)),
            _ => Err(ScalarError::InvalidBooleanText),
        },
        _ => Err(ScalarError::TypeMismatch),
    }
}

/// Apply one operation to already evaluated operands, not an expression or callback.
///
/// Both values must match the closed signature and be bounded, even when a value
/// would determine a lazy expression's result. An evaluator must implement
/// `and`/`or`/`value_or` short-circuiting **before** evaluating the right expression.
/// This function has no scheduling, fuel, host calls, authority or mutable globals.
///
/// ```
/// use leselang_runtime_core::{BinaryOperator, ScalarValue, apply_binary};
/// assert_eq!(apply_binary(BinaryOperator::Add, ScalarValue::Integer(2), ScalarValue::Integer(3)),
///            Ok(ScalarValue::Integer(5)));
/// ```
pub fn apply_binary(
    operator: BinaryOperator,
    left: ScalarValue,
    right: ScalarValue,
) -> Result<ScalarValue, ScalarError> {
    use BinaryOperator::*;
    use ScalarValue::*;
    if operator
        .result_type(left.scalar_type(), right.scalar_type())
        .is_none()
    {
        return Err(ScalarError::TypeMismatch);
    }
    if !left.is_bounded() || !right.is_bounded() {
        return Err(ScalarError::UnboundedOperand);
    }
    if matches!(operator, Eq | Ne) {
        return Ok(Boolean(if operator == Eq {
            left == right
        } else {
            left != right
        }));
    }
    match (operator, left, right) {
        (Split, String(left), String(right)) => {
            let mut items = StringListBuilder::new();
            for item in left.split(&right) {
                items.try_push_text(item)?;
            }
            Ok(StringList(items.finish()))
        }
        (Append, StringList(left), String(right)) => {
            let mut items = StringListBuilder::try_from(left)?;
            items.try_push(right)?;
            Ok(StringList(items.finish()))
        }
        (Join, StringList(left), String(right)) => {
            let bytes = left
                .0
                .iter()
                .map(std::string::String::len)
                .sum::<usize>()
                .saturating_add(right.len().saturating_mul(left.0.len().saturating_sub(1)));
            if bytes > MAX_SCALAR_STRING_BYTES {
                return Err(ScalarError::StringLimit);
            }
            Ok(String(left.0.join(&right)))
        }
        (ItemAt, StringList(left), Integer(index)) => {
            let value = usize::try_from(index)
                .ok()
                .and_then(|index| left.0.get(index))
                .cloned();
            Ok(OptionalString(OptionalStringValue(value)))
        }
        (ValueOr, OptionalString(value), String(fallback)) => {
            Ok(String(value.0.unwrap_or(fallback)))
        }
        (And, Boolean(left), Boolean(right)) => Ok(Boolean(left && right)),
        (Or, Boolean(left), Boolean(right)) => Ok(Boolean(left || right)),
        (Contains, String(left), String(right)) => Ok(Boolean(left.contains(&right))),
        (StartsWith, String(left), String(right)) => Ok(Boolean(left.starts_with(&right))),
        (EndsWith, String(left), String(right)) => Ok(Boolean(left.ends_with(&right))),
        (CharAt, String(left), Integer(index)) => {
            // Indices address Unicode scalars, not UTF-8 bytes or grapheme clusters.
            let value = usize::try_from(index)
                .ok()
                .and_then(|index| left.chars().nth(index))
                .map(|value| value.to_string());
            Ok(OptionalString(OptionalStringValue(value)))
        }
        (Concat, String(mut left), String(right)) => {
            if left.len().saturating_add(right.len()) > MAX_SCALAR_STRING_BYTES {
                return Err(ScalarError::StringLimit);
            }
            left.push_str(&right);
            Ok(String(left))
        }
        (operator, Integer(left), Integer(right)) => {
            let value = match operator {
                Lt => return Ok(Boolean(left < right)),
                Le => return Ok(Boolean(left <= right)),
                Gt => return Ok(Boolean(left > right)),
                Ge => return Ok(Boolean(left >= right)),
                Add => left.checked_add(right),
                Sub => left.checked_sub(right),
                Mul => left.checked_mul(right),
                Div => left.checked_div(right),
                Rem => left.checked_rem(right),
                _ => return Err(ScalarError::TypeMismatch),
            };
            value.map(Integer).ok_or(ScalarError::IntegerArithmetic)
        }
        _ => Err(ScalarError::TypeMismatch),
    }
}
