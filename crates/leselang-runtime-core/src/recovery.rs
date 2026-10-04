use crate::ScalarError;

/// Typed pure-calculation failure with an opaque adapter-owned external error.
///
/// Only explicit language scalar arithmetic/parse failures are recoverable.
/// External errors stay external regardless of their type, code or message.
/// `From<External>` never promotes an error into the scalar channel. Hosts must
/// preserve that provenance; a trusted adapter can construct enum variants.
///
/// This contains error data, not a callback, saved frame, retry/authority grant or
/// evaluator. Querying it neither runs a fallback nor refunds fuel/changes state.
/// Move-only ownership imposes no Clone/serde/Debug/Send requirement on external
/// errors; thread ownership is conditional on their type. Debug shows tags only,
/// never the external formatter. Pattern matching exposes the original error;
/// adapters still own diagnostics, redaction and external resource lifetimes.
///
/// ```
/// use leselang_runtime_core::{CalculationFailure, ScalarError};
/// let scalar = CalculationFailure::<()>::Scalar(ScalarError::InvalidIntegerText);
/// assert!(scalar.is_recoverable());
/// let external: CalculationFailure<ScalarError> = ScalarError::IntegerArithmetic.into();
/// assert!(!external.is_recoverable());
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{CalculationFailure, ScalarError};
/// let failure = CalculationFailure::<()>::Scalar(ScalarError::IntegerArithmetic);
/// let duplicate = failure.clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::{CalculationFailure, ScalarError};
/// fn wire<T: serde::Serialize>(_: &T) {}
/// wire(&CalculationFailure::<()>::Scalar(ScalarError::IntegerArithmetic));
/// ```
pub enum CalculationFailure<External> {
    Scalar(ScalarError),
    External(External),
}

impl<External> CalculationFailure<External> {
    /// Observe the closed scalar recovery class, without calling adapter code.
    pub const fn is_recoverable(&self) -> bool {
        match self {
            Self::Scalar(failure) => failure.is_recoverable(),
            Self::External(_) => false,
        }
    }
}

impl<External> From<External> for CalculationFailure<External> {
    fn from(failure: External) -> Self {
        Self::External(failure)
    }
}

impl<External> std::fmt::Debug for CalculationFailure<External> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scalar(failure) => formatter.debug_tuple("Scalar").field(failure).finish(),
            Self::External(_) => formatter.write_str("External"),
        }
    }
}
