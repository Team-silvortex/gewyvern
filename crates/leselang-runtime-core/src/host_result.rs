use std::fmt;

use crate::{OperationSchema, ScalarContractError, ScalarTypeSet, ScalarValue};

/// Developer-owned validation of a received value, not declared type metadata.
///
/// `matches_type` checks the original schema's result kind without coercion;
/// `validate_value` checks bounds, domains and any host-specific correlation.
/// Neither has an accept-all default. Both are trusted synchronous native code:
/// hosts bound ingress and callback work, preserve stable metadata and own
/// authority, raw receipt identity, deadlines and replay. Interior mutation and
/// unwinding are not rolled back or preempted. No effect is invoked by the core.
pub trait HostResultDomain<Reply: ?Sized> {
    type Error;

    fn matches_type(&self, reply: &Reply) -> bool;

    fn validate_value(&self, reply: &Reply) -> Result<(), Self::Error>;
}

/// Explicit result-type failure or the unmodified native validation failure.
///
/// Debug/Display never format native errors. There is no wire codec, cloning,
/// implicit retry classification or calculation-recovery promotion. Matching
/// `InvalidValue` exposes the original error to its owning adapter; that error
/// may contain private payloads and must be redacted before logging.
///
/// ```compile_fail
/// use leselang_runtime_core::HostResultError;
/// let wire = serde_json::to_string(&HostResultError::InvalidValue(7)).unwrap();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::HostResultError;
/// let restored: HostResultError<u8> = serde_json::from_str("7").unwrap();
/// ```
#[derive(Eq, PartialEq)]
pub enum HostResultError<External> {
    TypeMismatch,
    InvalidValue(External),
}

impl<External> fmt::Debug for HostResultError<External> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TypeMismatch => "HostResultError::TypeMismatch",
            Self::InvalidValue(_) => "HostResultError::InvalidValue",
        })
    }
}

impl<External> fmt::Display for HostResultError<External> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TypeMismatch => "host result type does not match its declaration",
            Self::InvalidValue(_) => "host result failed native value validation",
        })
    }
}

impl<External> std::error::Error for HostResultError<External> {}

/// Borrowed type-before-value validation of an actual host result.
///
/// Wrong kinds never enter the native value validator. The core does not copy,
/// format, normalize or mutate the reply or declaration, spend fuel, execute an
/// expression, dispatch effects or install a callback retry. Native failures
/// retain their explicit provenance, including errors resembling scalar faults.
/// Success is an observation of these data now, not an authentic receipt,
/// lasting bounds certificate, accepted continuation or execution authority.
///
/// ```
/// use leselang_runtime_core::{HostResultError, ScalarContractError, ScalarType,
///     ScalarTypeSet, ScalarValue, validate_host_result};
/// let domain = ScalarTypeSet::only(ScalarType::String);
/// let mut reply = ScalarValue::String("ready".into());
/// validate_host_result(&domain, &reply).unwrap();
/// reply = ScalarValue::String("x".repeat(4097));
/// assert_eq!(validate_host_result(&domain, &reply),
///     Err(HostResultError::InvalidValue(ScalarContractError::UnboundedValue)));
/// assert_eq!(validate_host_result(&domain, &ScalarValue::Boolean(true)),
///     Err(HostResultError::TypeMismatch));
/// ```
pub fn validate_host_result<Reply: ?Sized, Domain: HostResultDomain<Reply> + ?Sized>(
    domain: &Domain,
    reply: &Reply,
) -> Result<(), HostResultError<Domain::Error>> {
    if !domain.matches_type(reply) {
        return Err(HostResultError::TypeMismatch);
    }
    domain
        .validate_value(reply)
        .map_err(HostResultError::InvalidValue)
}

impl HostResultDomain<ScalarValue> for ScalarTypeSet {
    type Error = ScalarContractError;

    fn matches_type(&self, reply: &ScalarValue) -> bool {
        self.contains(reply.scalar_type())
    }

    fn validate_value(&self, reply: &ScalarValue) -> Result<(), Self::Error> {
        // Keep direct trait calls fail-closed too, not only the combined entry.
        self.validate(reply)
    }
}

impl<Key, Parameter, Domain, Result, Capability>
    OperationSchema<'_, Key, Parameter, Domain, Result, Capability>
{
    /// Reuse the original result declaration selected during static inference.
    ///
    /// Catalog version/capability, evaluated arguments and pending-effect identity
    /// must be checked separately. Public schemas are unchecked native metadata;
    /// this does not authorize a reply simply because its type matches.
    pub fn check_result<Reply: ?Sized>(
        &self,
        reply: &Reply,
    ) -> std::result::Result<(), HostResultError<Result::Error>>
    where
        Result: HostResultDomain<Reply>,
    {
        validate_host_result(&self.result, reply)
    }
}
