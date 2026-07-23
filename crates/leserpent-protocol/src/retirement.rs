use std::fmt;

use leserpent_domain::retirement::{
    RetirementError, RetirementId, RuntimeRetirement, RuntimeRetirementIntent,
    RuntimeRetirementSnapshot,
};
use leserpent_domain::{CapabilitySet, Principal};
use serde::{Deserialize, Serialize};

pub const RETIREMENT_PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const MAX_RETIREMENT_PROTOCOL_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementRequestEnvelope {
    pub schema_version: u32,
    pub request: RetirementRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub intent: RuntimeRetirementIntent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementResponseEnvelope {
    pub schema_version: u32,
    pub response: RetirementResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum RetirementResponse {
    State(RuntimeRetirementSnapshot),
    Error(RetirementProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RetirementProtocolError {
    pub retirement_id: Option<RetirementId>,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RetirementCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidRequest(RetirementError),
    InvalidResponse,
}

impl fmt::Display for RetirementCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(formatter, "retirement message size {size} exceeds {limit}")
            }
            Self::InvalidJson(error) => write!(formatter, "invalid retirement JSON: {error}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported retirement protocol schema {actual}, expected {expected}"
            ),
            Self::InvalidRequest(error) => write!(formatter, "invalid retirement request: {error}"),
            Self::InvalidResponse => formatter.write_str("invalid retirement response"),
        }
    }
}

impl std::error::Error for RetirementCodecError {}

pub fn decode_retirement_request(
    bytes: &[u8],
) -> Result<RetirementRequestEnvelope, RetirementCodecError> {
    require_bound(bytes)?;
    let envelope: RetirementRequestEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| RetirementCodecError::InvalidJson(error.to_string()))?;
    validate_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    Ok(envelope)
}

pub fn encode_retirement_request(
    envelope: &RetirementRequestEnvelope,
) -> Result<Vec<u8>, RetirementCodecError> {
    validate_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    encode_bounded(envelope)
}

pub fn decode_retirement_response(
    bytes: &[u8],
) -> Result<RetirementResponseEnvelope, RetirementCodecError> {
    require_bound(bytes)?;
    let envelope: RetirementResponseEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| RetirementCodecError::InvalidJson(error.to_string()))?;
    validate_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    Ok(envelope)
}

pub fn encode_retirement_response(
    envelope: &RetirementResponseEnvelope,
) -> Result<Vec<u8>, RetirementCodecError> {
    validate_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    encode_bounded(envelope)
}

fn validate_request(request: &RetirementRequest) -> Result<(), RetirementCodecError> {
    RuntimeRetirement::plan(
        &request.principal,
        &request.capabilities,
        request.intent.clone(),
    )
    .map(|_| ())
    .map_err(RetirementCodecError::InvalidRequest)
}

fn validate_response(response: &RetirementResponse) -> Result<(), RetirementCodecError> {
    match response {
        RetirementResponse::State(state) => state
            .validate()
            .map_err(|_| RetirementCodecError::InvalidResponse),
        RetirementResponse::Error(error) => {
            let code_valid = !error.code.is_empty()
                && error.code.len() <= 64
                && error
                    .code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
            let message_valid = !error.message.is_empty()
                && error.message.len() <= 512
                && error.message == error.message.trim()
                && !error.message.chars().any(char::is_control);
            (code_valid && message_valid)
                .then_some(())
                .ok_or(RetirementCodecError::InvalidResponse)
        }
    }
}

fn validate_schema(actual: u32) -> Result<(), RetirementCodecError> {
    if actual != RETIREMENT_PROTOCOL_SCHEMA_VERSION {
        return Err(RetirementCodecError::InvalidSchemaVersion {
            actual,
            expected: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), RetirementCodecError> {
    if bytes.len() > MAX_RETIREMENT_PROTOCOL_BYTES {
        return Err(RetirementCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_RETIREMENT_PROTOCOL_BYTES,
        });
    }
    Ok(())
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, RetirementCodecError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| RetirementCodecError::InvalidJson(error.to_string()))?;
    require_bound(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use leserpent_domain::RuntimeId;
    use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
    use leserpent_domain::provisioning::ProvisioningId;
    use leserpent_domain::retirement::{
        CAPABILITY_RUNTIME_RETIRE, RETIREMENT_DOMAIN_SCHEMA_VERSION, RetirementPhase,
    };
    use serde_json::json;

    use super::*;

    fn request() -> RetirementRequestEnvelope {
        RetirementRequestEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            request: RetirementRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]),
                intent: RuntimeRetirementIntent {
                    schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
                    retirement_id: RetirementId::new("retire-1").unwrap(),
                    provisioning_id: ProvisioningId::new("provision-1").unwrap(),
                    runtime_id: RuntimeId::new("runtime-1").unwrap(),
                    target: BootstrapTarget {
                        transport: BootstrapTransport::Ssh,
                        host: "runtime.example".into(),
                        port: 22,
                    },
                    retirement_credential_handle: CredentialHandle::new("vault:ssh:runtime")
                        .unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    fn planned_state() -> RuntimeRetirementSnapshot {
        RuntimeRetirement::plan(
            &request().request.principal,
            &request().request.capabilities,
            request().request.intent,
        )
        .unwrap()
        .snapshot()
    }

    #[test]
    fn request_round_trip_is_strict_and_secret_free() {
        let request = request();
        let bytes = encode_retirement_request(&request).unwrap();
        assert_eq!(decode_retirement_request(&bytes).unwrap(), request);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("vault:ssh:runtime"));
        assert!(!text.contains("password"));

        let mut value = serde_json::to_value(&request).unwrap();
        value["request"]["intent"]["password"] = json!("secret");
        assert!(matches!(
            decode_retirement_request(&serde_json::to_vec(&value).unwrap()),
            Err(RetirementCodecError::InvalidJson(_))
        ));
    }

    #[test]
    fn request_rejects_unconfirmed_or_wrong_capability() {
        let mut unconfirmed = request();
        unconfirmed.request.intent.confirmed = false;
        assert_eq!(
            encode_retirement_request(&unconfirmed),
            Err(RetirementCodecError::InvalidRequest(
                RetirementError::ConfirmationRequired
            ))
        );
        let mut unauthorized = request();
        unauthorized.request.capabilities = CapabilitySet::default();
        assert_eq!(
            encode_retirement_request(&unauthorized),
            Err(RetirementCodecError::InvalidRequest(
                RetirementError::Unauthorized
            ))
        );
    }

    #[test]
    fn response_round_trip_validates_safe_ordering() {
        let response = RetirementResponseEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            response: RetirementResponse::State(planned_state()),
        };
        let bytes = encode_retirement_response(&response).unwrap();
        assert_eq!(decode_retirement_response(&bytes).unwrap(), response);

        let mut invalid = planned_state();
        invalid.phase = RetirementPhase::RuntimeUnregistered;
        assert_eq!(
            encode_retirement_response(&RetirementResponseEnvelope {
                schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
                response: RetirementResponse::State(invalid),
            }),
            Err(RetirementCodecError::InvalidResponse)
        );
    }

    #[test]
    fn codec_bounds_messages_and_error_fields() {
        assert!(matches!(
            decode_retirement_request(&vec![b' '; MAX_RETIREMENT_PROTOCOL_BYTES + 1]),
            Err(RetirementCodecError::Oversized { .. })
        ));
        let invalid = RetirementResponseEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            response: RetirementResponse::Error(RetirementProtocolError {
                retirement_id: None,
                code: "Bad-Code".into(),
                message: "bad".into(),
            }),
        };
        assert_eq!(
            encode_retirement_response(&invalid),
            Err(RetirementCodecError::InvalidResponse)
        );
    }
}
