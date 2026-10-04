use std::{fmt, iter::FusedIterator};

use crate::{ScalarType, ScalarValue};

/// Explicit accepted scalar alternatives, not coercion or implicit nullability.
///
/// Copyable native metadata, not authority, a versioned wire schema, an operation
/// registration or a lasting value certificate. There is no implicit accept-all
/// default, raw-bit constructor or serde codec. Hosts own schema versioning.
///
/// ```
/// use leselang_runtime_core::{NamedParameter, ScalarType, ScalarTypeSet};
/// const TEXT: ScalarTypeSet = ScalarTypeSet::only(ScalarType::String)
///     .with(ScalarType::None).with(ScalarType::OptionalString);
/// let parameter = NamedParameter::required("caption", TEXT);
/// assert!(parameter.domain.contains(ScalarType::None));
/// assert!(!parameter.domain.contains(ScalarType::Boolean));
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScalarTypeSet;
/// let forged = ScalarTypeSet(255);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScalarTypeSet;
/// let wire = serde_json::to_string(&ScalarTypeSet::empty()).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::ScalarTypeSet;
/// let restored: ScalarTypeSet = serde_json::from_str("0").unwrap();
/// ```
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ScalarTypeSet(u8);

impl ScalarTypeSet {
    /// An explicit deny-all contract, useful while building alternatives.
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn only(ty: ScalarType) -> Self {
        Self(type_bit(ty))
    }

    /// Return a new set; duplicates are idempotent and do not change order.
    #[must_use]
    pub const fn with(self, ty: ScalarType) -> Self {
        Self(self.0 | type_bit(ty))
    }

    pub const fn contains(self, ty: ScalarType) -> bool {
        self.0 & type_bit(ty) != 0
    }

    /// At most six tags in canonical order, independent of insertion order.
    /// No allocation, host callback or caller-provided iterator is involved.
    pub fn iter(self) -> impl DoubleEndedIterator<Item = ScalarType> + FusedIterator {
        [
            ScalarType::Integer,
            ScalarType::Boolean,
            ScalarType::String,
            ScalarType::None,
            ScalarType::OptionalString,
            ScalarType::StringList,
        ]
        .into_iter()
        .filter(move |ty| self.contains(*ty))
    }

    /// Check type first, then existing per-value language bounds, borrowing only.
    ///
    /// No normalization, parsing, cloning, truncation, fuel, native domain checks
    /// or effects. String-list validation examines at most 64 entries after its
    /// count check; text uses UTF-8 bytes, not characters. This neither bounds
    /// prior allocation/decoding nor aggregate host memory/CPU. Recheck mutated
    /// values; success is not permission to dispatch or a persistent certificate.
    ///
    /// ```
    /// use leselang_runtime_core::{ScalarContractError, ScalarType, ScalarTypeSet, ScalarValue};
    /// let contract = ScalarTypeSet::only(ScalarType::String);
    /// let mut value = ScalarValue::String("ready".into());
    /// contract.validate(&value).unwrap();
    /// value = ScalarValue::String("x".repeat(4097));
    /// assert_eq!(contract.validate(&value), Err(ScalarContractError::UnboundedValue));
    /// assert_eq!(contract.validate(&ScalarValue::None), Err(ScalarContractError::TypeMismatch));
    /// ```
    pub fn validate(self, value: &ScalarValue) -> Result<(), ScalarContractError> {
        if !self.contains(value.scalar_type()) {
            return Err(ScalarContractError::TypeMismatch);
        }
        if !value.is_bounded() {
            return Err(ScalarContractError::UnboundedValue);
        }
        Ok(())
    }
}

impl fmt::Debug for ScalarTypeSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ScalarTypeSet ")?;
        formatter.debug_set().entries(self.iter()).finish()
    }
}

const fn type_bit(ty: ScalarType) -> u8 {
    match ty {
        ScalarType::Integer => 1,
        ScalarType::Boolean => 2,
        ScalarType::String => 4,
        ScalarType::None => 8,
        ScalarType::OptionalString => 16,
        ScalarType::StringList => 32,
    }
}

/// Closed, payload-free preflight failures, not recoverable calculation faults.
/// Hosts retain their own diagnostic mapping and domain/authority checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScalarContractError {
    TypeMismatch,
    UnboundedValue,
}

impl fmt::Display for ScalarContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TypeMismatch => "scalar type is not accepted by contract",
            Self::UnboundedValue => "scalar exceeds language bounds",
        })
    }
}

impl std::error::Error for ScalarContractError {}
