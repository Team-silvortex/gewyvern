//! Product dispatch recovery policy over original, caller-owned storage reads.
//!
//! This inspection neither issues leases nor authenticates supplied observations.
//! The owner retains schema/state decoding, same-snapshot reads, complete graph
//! validation and atomic checkpoint publication. Success is not replay authority.

use leselang_runtime_core::{
    DispatchRecoveryError, DispatchRecoveryHost, DispatchRecoveryLimits, Fault,
    StoredDispatchMetadata, inspect_dispatch_recovery,
};

use crate::bounded_json::{self, BoundedJsonError};
use crate::reference_dispatch::MAX_DISPATCH_ATTEMPTS;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::reference_request::validate_effect_request;
use crate::retry_policy::{EffectRetryError, MAX_SEMANTIC_RETRIES, validate_effect_error};
use crate::{EffectError, EffectRequest, MAX_CONTINUATION_BYTES};

/// Read from the same original snapshot as the already-read token. Hooks must
/// bound storage work, preserve native faults and never repair or commit data.
/// Metadata must retain signed counters and the original optional retry bytes;
/// state decoding and schema integrity remain the storage owner's responsibility.
/// Borrowed bytes and local, unsized sources need no Clone/Send implementation.
pub trait ReferenceDispatchSource {
    type Bytes: AsRef<[u8]>;

    fn read_request(&mut self) -> Result<Self::Bytes, Fault>;
    fn read_image(&mut self) -> Result<Self::Bytes, Fault>;
    fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Self::Bytes>, Fault>;
}

/// Inspect only. A caller-supplied token/source cannot override a live lease or
/// pending frame. Keep original storage authority until the full checkpoint commits.
pub fn inspect_dispatch_record<Source: ReferenceDispatchSource + ?Sized>(
    source: &mut Source,
    token: &str,
) -> Result<(), Fault> {
    inspect_dispatch_recovery(
        &mut DispatchPolicy { source, token },
        DispatchRecoveryLimits {
            max_attempts: MAX_DISPATCH_ATTEMPTS,
            max_retries: MAX_SEMANTIC_RETRIES,
        },
    )
    .map_err(|error| match error {
        DispatchRecoveryError::Request(error)
        | DispatchRecoveryError::Identity(error)
        | DispatchRecoveryError::Image(error)
        | DispatchRecoveryError::Metadata(error)
        | DispatchRecoveryError::RetryError(error) => error,
        DispatchRecoveryError::Attempt => fault("LSV4007", "dispatch attempt is invalid"),
        DispatchRecoveryError::RetryState => fault("LSV4007", "dispatch retry state is invalid"),
        DispatchRecoveryError::RetryHistory => {
            fault("LSV4007", "dispatch retry history is inconsistent")
        }
        DispatchRecoveryError::CompletionState => fault(
            "LSV4007",
            "dispatch and continuation states are inconsistent",
        ),
    })
}

struct DispatchPolicy<'a, Source: ReferenceDispatchSource + ?Sized> {
    source: &'a mut Source,
    token: &'a str,
}

impl<Source: ReferenceDispatchSource + ?Sized> DispatchRecoveryHost for DispatchPolicy<'_, Source> {
    type Request = EffectRequest;
    type Retry = Source::Bytes;
    type Error = Fault;

    fn admit_request(&mut self) -> Result<EffectRequest, Fault> {
        let bytes = self.source.read_request()?;
        let request = decode_journal_value(bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        Ok(request)
    }
    fn validate_identity(&mut self, request: &EffectRequest) -> Result<(), Fault> {
        if request.continuation.token.as_str() != self.token {
            return Err(fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        Ok(())
    }
    fn validate_image(&mut self, request: &EffectRequest) -> Result<(), Fault> {
        let effect_image = self.source.read_image()?;
        // Do not use the stricter restoration codec: recovery compares original
        // compact wire bytes after request admission, without new restore gates.
        let request_image = bounded_json::encode(&request.continuation, MAX_CONTINUATION_BYTES)
            .map_err(|error| match error {
                BoundedJsonError::Limit { limit, .. } => {
                    fault("LSV3002", format!("continuation exceeds {limit} bytes"))
                }
                BoundedJsonError::Serialize(error) => {
                    fault("LSV3001", format!("failed to encode continuation: {error}"))
                }
            })?;
        if request_image != effect_image.as_ref() {
            return Err(fault(
                "LSV4007",
                "dispatch request conflicts with continuation state",
            ));
        }
        Ok(())
    }
    fn read_metadata(&mut self) -> Result<StoredDispatchMetadata<Self::Retry>, Fault> {
        self.source.read_metadata()
    }
    fn validate_retry_error(&mut self, bytes: Self::Retry) -> Result<(), Fault> {
        let error: EffectError = decode_journal_value(bytes.as_ref(), 2_048)?;
        validate_effect_error(&error).map_err(|error| {
            fault(
                match error {
                    EffectRetryError::Metadata => "LSV2202",
                    EffectRetryError::Policy => "LSV2201",
                    EffectRetryError::Clock => "LSV2203",
                },
                error.to_string(),
            )
        })
    }
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
