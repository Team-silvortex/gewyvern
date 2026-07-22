use std::fmt;

use leserpent_domain::provisioning::{
    ProvisioningError, ProvisioningId, RuntimeProvisioning, RuntimeProvisioningIntent,
    RuntimeProvisioningSnapshot,
};
use leserpent_domain::{CapabilitySet, Principal};
use serde::{Deserialize, Serialize};

pub const PROVISIONING_PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROVISIONING_PROTOCOL_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningRequestEnvelope {
    pub schema_version: u32,
    pub request: ProvisioningRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub intent: RuntimeProvisioningIntent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningResponseEnvelope {
    pub schema_version: u32,
    pub response: ProvisioningResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProvisioningResponse {
    State(RuntimeProvisioningSnapshot),
    Error(ProvisioningProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvisioningProtocolError {
    pub provisioning_id: Option<ProvisioningId>,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProvisioningCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidRequest(ProvisioningError),
    InvalidResponse,
}

impl fmt::Display for ProvisioningCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => write!(
                formatter,
                "provisioning message size {size} exceeds {limit}"
            ),
            Self::InvalidJson(error) => write!(formatter, "invalid provisioning JSON: {error}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported provisioning protocol schema {actual}, expected {expected}"
            ),
            Self::InvalidRequest(error) => {
                write!(formatter, "invalid provisioning request: {error}")
            }
            Self::InvalidResponse => formatter.write_str("invalid provisioning response"),
        }
    }
}

impl std::error::Error for ProvisioningCodecError {}

pub fn decode_provisioning_request(
    bytes: &[u8],
) -> Result<ProvisioningRequestEnvelope, ProvisioningCodecError> {
    require_bound(bytes)?;
    let envelope: ProvisioningRequestEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| ProvisioningCodecError::InvalidJson(error.to_string()))?;
    validate_protocol_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    Ok(envelope)
}

pub fn encode_provisioning_request(
    envelope: &ProvisioningRequestEnvelope,
) -> Result<Vec<u8>, ProvisioningCodecError> {
    validate_protocol_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    encode_bounded(envelope)
}

pub fn decode_provisioning_response(
    bytes: &[u8],
) -> Result<ProvisioningResponseEnvelope, ProvisioningCodecError> {
    require_bound(bytes)?;
    let envelope: ProvisioningResponseEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| ProvisioningCodecError::InvalidJson(error.to_string()))?;
    validate_protocol_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    Ok(envelope)
}

pub fn encode_provisioning_response(
    envelope: &ProvisioningResponseEnvelope,
) -> Result<Vec<u8>, ProvisioningCodecError> {
    validate_protocol_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    encode_bounded(envelope)
}

fn validate_request(request: &ProvisioningRequest) -> Result<(), ProvisioningCodecError> {
    RuntimeProvisioning::plan(
        &request.principal,
        &request.capabilities,
        request.intent.clone(),
    )
    .map(|_| ())
    .map_err(ProvisioningCodecError::InvalidRequest)
}

fn validate_response(response: &ProvisioningResponse) -> Result<(), ProvisioningCodecError> {
    match response {
        ProvisioningResponse::State(state) => state
            .validate()
            .map_err(|_| ProvisioningCodecError::InvalidResponse),
        ProvisioningResponse::Error(error) => {
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
                .ok_or(ProvisioningCodecError::InvalidResponse)
        }
    }
}

fn validate_protocol_schema(actual: u32) -> Result<(), ProvisioningCodecError> {
    if actual != PROVISIONING_PROTOCOL_SCHEMA_VERSION {
        return Err(ProvisioningCodecError::InvalidSchemaVersion {
            actual,
            expected: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), ProvisioningCodecError> {
    if bytes.len() > MAX_PROVISIONING_PROTOCOL_BYTES {
        return Err(ProvisioningCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_PROVISIONING_PROTOCOL_BYTES,
        });
    }
    Ok(())
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, ProvisioningCodecError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| ProvisioningCodecError::InvalidJson(error.to_string()))?;
    require_bound(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use leserpent_domain::RuntimeId;
    use leserpent_domain::bootstrap::{BootstrapTarget, BootstrapTransport, CredentialHandle};
    use leserpent_domain::provisioning::{
        CAPABILITY_RUNTIME_PROVISION, PROVISIONING_DOMAIN_SCHEMA_VERSION, ProvisioningPhase,
    };
    use serde_json::json;

    use super::*;

    fn request() -> ProvisioningRequestEnvelope {
        ProvisioningRequestEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            request: ProvisioningRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]),
                intent: RuntimeProvisioningIntent {
                    schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
                    provisioning_id: ProvisioningId::new("provision-1").unwrap(),
                    runtime_id: RuntimeId::new("runtime-a").unwrap(),
                    target: BootstrapTarget {
                        transport: BootstrapTransport::Ssh,
                        host: "host.example".into(),
                        port: 22,
                    },
                    install_credential_handle: CredentialHandle::new("vault:ssh:host-example")
                        .unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    fn planned_state() -> RuntimeProvisioningSnapshot {
        RuntimeProvisioning::plan(
            &request().request.principal,
            &request().request.capabilities,
            request().request.intent,
        )
        .unwrap()
        .snapshot()
    }

    #[test]
    fn request_round_trips_with_only_opaque_credential_authority() {
        let request = request();
        let bytes = encode_provisioning_request(&request).unwrap();
        assert_eq!(decode_provisioning_request(&bytes).unwrap(), request);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("vault:ssh:host-example"));
        assert!(!text.contains("password"));
        assert!(!text.contains("private_key"));
        assert!(!text.contains("token"));
    }

    #[test]
    fn request_rejects_unknown_and_raw_credential_fields() {
        let mut value = serde_json::to_value(request()).unwrap();
        value["request"]["intent"]["password"] = json!("secret");
        assert!(matches!(
            decode_provisioning_request(&serde_json::to_vec(&value).unwrap()),
            Err(ProvisioningCodecError::InvalidJson(message))
                if message.contains("unknown field")
        ));

        let mut unconfirmed = request();
        unconfirmed.request.intent.confirmed = false;
        assert_eq!(
            encode_provisioning_request(&unconfirmed),
            Err(ProvisioningCodecError::InvalidRequest(
                ProvisioningError::ConfirmationRequired
            ))
        );
    }

    #[test]
    fn response_round_trips_only_valid_public_state() {
        let response = ProvisioningResponseEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            response: ProvisioningResponse::State(planned_state()),
        };
        let bytes = encode_provisioning_response(&response).unwrap();
        assert_eq!(decode_provisioning_response(&bytes).unwrap(), response);

        let mut invalid = planned_state();
        invalid.phase = ProvisioningPhase::RuntimeRegistered;
        assert_eq!(
            encode_provisioning_response(&ProvisioningResponseEnvelope {
                schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
                response: ProvisioningResponse::State(invalid),
            }),
            Err(ProvisioningCodecError::InvalidResponse)
        );
    }

    #[test]
    fn protocol_is_bounded_and_error_messages_are_sanitized() {
        assert!(matches!(
            decode_provisioning_request(&vec![b' '; MAX_PROVISIONING_PROTOCOL_BYTES + 1]),
            Err(ProvisioningCodecError::Oversized { .. })
        ));
        let invalid = ProvisioningResponseEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            response: ProvisioningResponse::Error(ProvisioningProtocolError {
                provisioning_id: None,
                code: "transport_failure".into(),
                message: "unsafe\nmessage".into(),
            }),
        };
        assert_eq!(
            encode_provisioning_response(&invalid),
            Err(ProvisioningCodecError::InvalidResponse)
        );
    }
}
