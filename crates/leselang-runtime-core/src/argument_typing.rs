use std::fmt;

use crate::{
    NamedArgumentError, OperationSchema, ScalarType, ScalarTypeSet, ScalarValue,
    validate_named_arguments,
};

/// Host-owned scalar alternatives and a bounded native literal-domain check.
///
/// The checker calls `accepts_literal` only after purity, scalar type, literal
/// consistency and language bounds pass. This is a trusted developer adapter,
/// not script dispatch: native methods may allocate, mutate interior state or
/// unwind. Hosts bound native work; there is no rollback or callback preemption.
/// Dynamic expressions have no literal and require value/domain revalidation
/// after evaluation, before dispatch. No default accepts an undeclared domain.
pub trait ScalarArgumentDomain {
    fn scalar_types(&self) -> ScalarTypeSet;

    fn accepts_literal(&self, value: &ScalarValue) -> bool;
}

impl ScalarArgumentDomain for ScalarTypeSet {
    fn scalar_types(&self) -> ScalarTypeSet {
        *self
    }

    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        true
    }
}

/// Unchecked facts supplied by a trusted expression checker, not an IR validator.
///
/// `None` scalar type means a host/structured result, not the language `none`
/// value. A missing literal means its value is unknown, not absent/null. A literal
/// must have the declared type; text is never parsed or coerced. Public fields
/// deliberately allow adapters to report rejected/forged facts for preflight.
/// This has no wire codec, authority, lifetime certificate or default. Debug
/// reports facts only, without invoking the literal's payload formatter.
///
/// ```compile_fail
/// use leselang_runtime_core::ScalarArgumentType;
/// let facts = ScalarArgumentType::expression(None, false);
/// let wire = serde_json::to_string(&facts).unwrap();
/// ```
#[derive(Clone, Copy)]
pub struct ScalarArgumentType<'a> {
    pub scalar_type: Option<ScalarType>,
    pub is_pure: bool,
    pub literal: Option<&'a ScalarValue>,
}

impl<'a> ScalarArgumentType<'a> {
    pub const fn expression(scalar_type: Option<ScalarType>, is_pure: bool) -> Self {
        Self {
            scalar_type,
            is_pure,
            literal: None,
        }
    }

    pub fn literal(value: &'a ScalarValue) -> Self {
        Self {
            scalar_type: Some(value.scalar_type()),
            is_pure: true,
            literal: Some(value),
        }
    }
}

impl fmt::Debug for ScalarArgumentType<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ScalarArgumentType")
            .field("scalar_type", &self.scalar_type)
            .field("is_pure", &self.is_pure)
            .field("has_literal", &self.literal.is_some())
            .finish()
    }
}

/// Closed static failures, never private values or recoverable calculation faults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentTypeError {
    Impure,
    NonScalar,
    TypeMismatch,
    InconsistentLiteralType,
    UnboundedLiteral,
    InvalidLiteralDomain,
}

impl fmt::Display for ArgumentTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Impure => "host argument expression must be pure",
            Self::NonScalar => "host argument expression must be scalar",
            Self::TypeMismatch => "host argument scalar type is not accepted",
            Self::InconsistentLiteralType => "host argument literal disagrees with its scalar type",
            Self::UnboundedLiteral => "host argument literal exceeds language bounds",
            Self::InvalidLiteralDomain => "host argument literal is outside its native domain",
        })
    }
}

impl std::error::Error for ArgumentTypeError {}

/// Purity, scalar type, accepted alternatives, literal consistency, bounds, domain.
///
/// Borrows only, with no core allocation, normalization or expression evaluation.
/// For a dynamic expression, success checks type/purity only: it does not validate
/// any eventual value, cold subtree, binding, operation version or capability.
/// Callers infer facts from the original bounded IR, including cold branches,
/// and own diagnostics/fuel. Recheck evaluated/mutated values before effects.
pub fn check_argument_type<Domain: ScalarArgumentDomain + ?Sized>(
    domain: &Domain,
    argument: ScalarArgumentType<'_>,
) -> Result<(), ArgumentTypeError> {
    if !argument.is_pure {
        return Err(ArgumentTypeError::Impure);
    }
    let ty = argument.scalar_type.ok_or(ArgumentTypeError::NonScalar)?;
    let types = domain.scalar_types();
    if !types.contains(ty) {
        return Err(ArgumentTypeError::TypeMismatch);
    }
    if let Some(value) = argument.literal {
        if value.scalar_type() != ty {
            return Err(ArgumentTypeError::InconsistentLiteralType);
        }
        if !value.is_bounded() {
            return Err(ArgumentTypeError::UnboundedLiteral);
        }
        if !domain.accepts_literal(value) {
            return Err(ArgumentTypeError::InvalidLiteralDomain);
        }
    }
    Ok(())
}

/// Positions and closed failures only; no submitted names or native metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperationTypeError {
    ArgumentCountMismatch,
    Names(NamedArgumentError),
    Argument {
        parameter_index: usize,
        argument_index: usize,
        error: ArgumentTypeError,
    },
}

impl fmt::Display for OperationTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ArgumentCountMismatch => {
                formatter.write_str("host argument names and type facts have different lengths")
            }
            Self::Names(error) => error.fmt(formatter),
            Self::Argument { error, .. } => error.fmt(formatter),
        }
    }
}

impl std::error::Error for OperationTypeError {}

impl<Key, Parameter: PartialEq, Domain: ScalarArgumentDomain, Result, Capability>
    OperationSchema<'_, Key, Parameter, Domain, Result, Capability>
{
    /// Validate facts in original input alignment, inspecting declaration order.
    ///
    /// Count alignment precedes named-shape preflight; all names are checked
    /// before any domain method. Required means present, not non-null. Optional
    /// omissions are not filled. Per-argument failures use original input and
    /// declaration positions. No values, metadata or result tags are cloned.
    /// Native key equivalence must remain stable during binding and inspection.
    ///
    /// Work is bounded by caller-bounded schema/input counts and native methods;
    /// the core allocates nothing. Success is an observation, not a checked IR,
    /// registered operation, value/result certificate or execution permission.
    /// Schema lookup/version/capability preflight remains separate.
    ///
    /// ```
    /// use leselang_runtime_core::{NamedParameter, OperationSchema, ScalarArgumentType, ScalarType, ScalarTypeSet};
    /// let parameters = [NamedParameter::required("position", ScalarTypeSet::only(ScalarType::Integer))];
    /// let schema = OperationSchema { key: "device.move", parameters: &parameters,
    ///     result: "position-receipt", required_capability: "motion" };
    /// schema.check_argument_types(&["position"],
    ///     &[ScalarArgumentType::expression(Some(ScalarType::Integer), true)]).unwrap();
    /// assert_eq!(schema.result, "position-receipt");
    /// ```
    pub fn check_argument_types(
        &self,
        names: &[Parameter],
        arguments: &[ScalarArgumentType<'_>],
    ) -> std::result::Result<(), OperationTypeError> {
        if names.len() != arguments.len() {
            return Err(OperationTypeError::ArgumentCountMismatch);
        }
        validate_named_arguments(names, self.parameters).map_err(OperationTypeError::Names)?;
        // Inspect the original declaration index, including omitted optionals.
        for (parameter_index, parameter) in self.parameters.iter().enumerate() {
            if let Some(argument_index) = names.iter().position(|name| name == &parameter.name) {
                check_argument_type(&parameter.domain, arguments[argument_index]).map_err(
                    |error| OperationTypeError::Argument {
                        parameter_index,
                        argument_index,
                        error,
                    },
                )?;
            }
        }
        Ok(())
    }
}
