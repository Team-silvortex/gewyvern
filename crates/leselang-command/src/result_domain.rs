//! Borrowed result-domain composition with explicit native host policy.

use std::fmt;

use leselang_runtime_core::HostResultDomain;
use leserpent_hir::Type;

use crate::{Value, ValueBoundsError, ValueValidationLimits, validate_value_items};

/// Trusted synchronous hooks, with no implicit name grammar or encoding policy.
///
/// The host bounds callback work and ingress bytes. Interior mutation and unwind
/// are not rolled back. Encoded-size policy must be checked here, not inferred
/// from the leaf/item count. Authentication and durable receipt binding are
/// separate gates; this interface supplies neither authority nor a codec.
/// Scoped pending acceptance checks liveness around the whole domain callback,
/// not between individual field hooks. Hosts must bound or stop their own work.
pub trait ResultValueHost {
    type Error;

    fn valid_field_name(&self, name: &str) -> bool;

    fn validate_encoded_value(&self, value: &Value) -> Result<(), Self::Error>;
}

/// Original pending declaration and explicit policy, not an admission certificate.
///
/// Use through `validate_host_result` or `PendingReply` for type-before-value
/// ordering. The trait's value-only method does not repeat the type gate. After
/// type checking, traversal starts at zero, followed by the per-execution item
/// limit (zero rejects even an empty query), then the native encoding hook.
/// The host obtains the type/limits from admitted execution state, never a reply.
/// No copies, fuel charges, dispatch, implicit retry or authority checks occur.
pub struct ResultValueDomain<'host, Host: ?Sized> {
    pub result_type: Type,
    pub value_limits: ValueValidationLimits,
    pub max_output_items: usize,
    pub host: &'host Host,
}

impl<Host: ?Sized> fmt::Debug for ResultValueDomain<'_, Host> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResultValueDomain")
    }
}

/// Native errors retain ownership without imposing Clone, Debug or codec bounds.
/// Debug/Display omit payloads, including native errors that may contain secrets.
/// Matching `Encoding` exposes the original error; hosts must redact it themselves.
#[derive(Eq, PartialEq)]
pub enum ResultValueError<Native> {
    Bounds(ValueBoundsError),
    OutputLimit { count: usize, limit: usize },
    Encoding(Native),
}

impl<Native> fmt::Debug for ResultValueError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bounds(_) => "ResultValueError::Bounds",
            Self::OutputLimit { .. } => "ResultValueError::OutputLimit",
            Self::Encoding(_) => "ResultValueError::Encoding",
        })
    }
}

impl<Native> fmt::Display for ResultValueError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Bounds(_) => "result value exceeds its domain bounds",
            Self::OutputLimit { .. } => "result value exceeds its output item limit",
            Self::Encoding(_) => "result value failed native encoding validation",
        })
    }
}

impl<Native> std::error::Error for ResultValueError<Native> {}

impl<Host: ResultValueHost + ?Sized> HostResultDomain<Value> for ResultValueDomain<'_, Host> {
    type Error = ResultValueError<Host::Error>;

    fn matches_type(&self, reply: &Value) -> bool {
        reply.result_type() == self.result_type
    }

    fn validate_value(&self, reply: &Value) -> Result<(), Self::Error> {
        let count = validate_value_items(reply, 0, self.value_limits, |name| {
            self.host.valid_field_name(name)
        })
        .map_err(ResultValueError::Bounds)?;
        if count > self.max_output_items || self.max_output_items == 0 {
            return Err(ResultValueError::OutputLimit {
                count,
                limit: self.max_output_items,
            });
        }
        self.host
            .validate_encoded_value(reply)
            .map_err(ResultValueError::Encoding)
    }
}
