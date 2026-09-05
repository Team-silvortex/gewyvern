use std::fmt;

use leserpent_domain::bootstrap::{
    BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BootstrapError, BootstrapId, BootstrapIntent,
    DeploymentBootstrap, DeploymentBootstrapSnapshot,
};
use leserpent_domain::{CapabilitySet, Principal};
use serde::{Deserialize, Serialize};

use crate::{BoundedJsonEncodeError, encode_json_bounded};

pub const BOOTSTRAP_PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOOTSTRAP_PROTOCOL_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRequestEnvelope {
    pub schema_version: u32,
    pub request: BootstrapRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub intent: BootstrapIntent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapResponseEnvelope {
    pub schema_version: u32,
    pub response: BootstrapResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum BootstrapResponse {
    State(DeploymentBootstrapSnapshot),
    Error(BootstrapProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapProtocolError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_id: Option<BootstrapId>,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidDomainSchemaVersion { actual: u32, expected: u32 },
    InvalidRequest(BootstrapError),
    InvalidResponse,
}

impl fmt::Display for BootstrapCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(formatter, "bootstrap message size {size} exceeds {limit}")
            }
            Self::InvalidJson(error) => write!(formatter, "invalid bootstrap JSON: {error}"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported bootstrap protocol schema {actual}, expected {expected}"
            ),
            Self::InvalidDomainSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported bootstrap domain schema {actual}, expected {expected}"
            ),
            Self::InvalidRequest(error) => write!(formatter, "invalid bootstrap request: {error}"),
            Self::InvalidResponse => write!(formatter, "invalid bootstrap response"),
        }
    }
}

impl std::error::Error for BootstrapCodecError {}

pub fn decode_bootstrap_request(
    bytes: &[u8],
) -> Result<BootstrapRequestEnvelope, BootstrapCodecError> {
    require_bound(bytes)?;
    let envelope: BootstrapRequestEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| BootstrapCodecError::InvalidJson(error.to_string()))?;
    validate_protocol_schema(envelope.schema_version)?;
    if envelope.request.intent.schema_version != BOOTSTRAP_DOMAIN_SCHEMA_VERSION {
        return Err(BootstrapCodecError::InvalidDomainSchemaVersion {
            actual: envelope.request.intent.schema_version,
            expected: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
        });
    }
    validate_request(&envelope.request)?;
    Ok(envelope)
}

pub fn encode_bootstrap_request(
    envelope: &BootstrapRequestEnvelope,
) -> Result<Vec<u8>, BootstrapCodecError> {
    validate_protocol_schema(envelope.schema_version)?;
    if envelope.request.intent.schema_version != BOOTSTRAP_DOMAIN_SCHEMA_VERSION {
        return Err(BootstrapCodecError::InvalidDomainSchemaVersion {
            actual: envelope.request.intent.schema_version,
            expected: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
        });
    }
    validate_request(&envelope.request)?;
    encode_bounded(envelope)
}

pub fn decode_bootstrap_response(
    bytes: &[u8],
) -> Result<BootstrapResponseEnvelope, BootstrapCodecError> {
    require_bound(bytes)?;
    let envelope: BootstrapResponseEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| BootstrapCodecError::InvalidJson(error.to_string()))?;
    validate_protocol_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    Ok(envelope)
}

pub fn encode_bootstrap_response(
    envelope: &BootstrapResponseEnvelope,
) -> Result<Vec<u8>, BootstrapCodecError> {
    validate_protocol_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    encode_bounded(envelope)
}

fn validate_request(request: &BootstrapRequest) -> Result<(), BootstrapCodecError> {
    DeploymentBootstrap::plan(
        &request.principal,
        &request.capabilities,
        request.intent.clone(),
    )
    .map(|_| ())
    .map_err(BootstrapCodecError::InvalidRequest)
}

fn validate_response(response: &BootstrapResponse) -> Result<(), BootstrapCodecError> {
    match response {
        BootstrapResponse::State(snapshot) => snapshot
            .validate()
            .map_err(|_| BootstrapCodecError::InvalidResponse),
        BootstrapResponse::Error(error) => {
            let valid = valid_error_code(&error.code)
                && !error.message.is_empty()
                && error.message.len() <= 512
                && !error.message.chars().any(char::is_control);
            valid
                .then_some(())
                .ok_or(BootstrapCodecError::InvalidResponse)
        }
    }
}

fn validate_protocol_schema(actual: u32) -> Result<(), BootstrapCodecError> {
    if actual != BOOTSTRAP_PROTOCOL_SCHEMA_VERSION {
        return Err(BootstrapCodecError::InvalidSchemaVersion {
            actual,
            expected: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), BootstrapCodecError> {
    if bytes.len() > MAX_BOOTSTRAP_PROTOCOL_BYTES {
        return Err(BootstrapCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_BOOTSTRAP_PROTOCOL_BYTES,
        });
    }
    Ok(())
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, BootstrapCodecError> {
    encode_json_bounded(value, MAX_BOOTSTRAP_PROTOCOL_BYTES).map_err(|error| match error {
        BoundedJsonEncodeError::Oversized { size, limit } => {
            BootstrapCodecError::Oversized { size, limit }
        }
        BoundedJsonEncodeError::InvalidJson(error) => {
            BootstrapCodecError::InvalidJson(error.to_string())
        }
    })
}

fn valid_error_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use leserpent_domain::bootstrap::{
        BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BootstrapPhase, BootstrapTarget, BootstrapTransport,
        CAPABILITY_HOST_BOOTSTRAP, CredentialHandle,
    };

    use super::*;

    fn request() -> BootstrapRequestEnvelope {
        BootstrapRequestEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            request: BootstrapRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                intent: BootstrapIntent {
                    schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
                    bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                    target: BootstrapTarget {
                        transport: BootstrapTransport::Ssh,
                        host: "host.example".into(),
                        port: 22,
                    },
                    credential_handle: CredentialHandle::new("vault:ssh:host-example").unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    #[test]
    fn bootstrap_request_round_trips_without_raw_credentials() {
        let request = request();
        let bytes = encode_bootstrap_request(&request).unwrap();
        assert_eq!(decode_bootstrap_request(&bytes).unwrap(), request);
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("credential_handle"));
        assert!(!text.contains("password"));
        assert!(!text.contains("private_key"));
        assert!(!text.contains("session_token"));
    }

    #[test]
    fn canonical_bootstrap_fixtures_decode_and_match_the_typed_contract() {
        let decoded = decode_bootstrap_request(include_bytes!(
            "../tests/fixtures/bootstrap-request-v1.json"
        ))
        .unwrap();
        assert_eq!(decoded, request());

        let response = decode_bootstrap_response(include_bytes!(
            "../tests/fixtures/bootstrap-planned-response-v1.json"
        ))
        .unwrap();
        let BootstrapResponse::State(snapshot) = response.response else {
            panic!("bootstrap fixture must contain state");
        };
        assert_eq!(snapshot.phase, BootstrapPhase::Planned);
        assert!(!snapshot.mutation_authorized);
        assert!(snapshot.bootstrap_credential_present);
    }

    #[test]
    fn unknown_secret_fields_and_raw_secret_handles_are_rejected() {
        let mut value = serde_json::to_value(request()).unwrap();
        value["request"]["intent"]["password"] =
            serde_json::Value::String("test-only-raw-secret".into());
        assert!(matches!(
            decode_bootstrap_request(&serde_json::to_vec(&value).unwrap()),
            Err(BootstrapCodecError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let mut value = serde_json::to_value(request()).unwrap();
        value["request"]["intent"]["credential_handle"] =
            serde_json::Value::String("test-only-raw-secret".into());
        assert!(matches!(
            decode_bootstrap_request(&serde_json::to_vec(&value).unwrap()),
            Err(BootstrapCodecError::InvalidJson(message))
                if message.contains("invalid credential handle")
        ));
    }

    #[test]
    fn bootstrap_request_requires_capability_confirmation_and_principal_binding() {
        let mut unauthorized = request();
        unauthorized.request.capabilities = CapabilitySet::default();
        assert!(matches!(
            encode_bootstrap_request(&unauthorized),
            Err(BootstrapCodecError::InvalidRequest(
                BootstrapError::Unauthorized
            ))
        ));

        let mut unconfirmed = request();
        unconfirmed.request.intent.confirmed = false;
        assert!(matches!(
            encode_bootstrap_request(&unconfirmed),
            Err(BootstrapCodecError::InvalidRequest(
                BootstrapError::ConfirmationRequired
            ))
        ));

        let mut mismatched = request();
        mismatched.request.principal.id = "operator-b".into();
        assert!(matches!(
            encode_bootstrap_request(&mismatched),
            Err(BootstrapCodecError::InvalidRequest(
                BootstrapError::PrincipalMismatch
            ))
        ));
    }

    #[test]
    fn bootstrap_state_response_is_strict_and_bounded() {
        let bootstrap = DeploymentBootstrap::plan(
            &request().request.principal,
            &request().request.capabilities,
            request().request.intent,
        )
        .unwrap();
        let response = BootstrapResponseEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            response: BootstrapResponse::State(bootstrap.snapshot()),
        };
        let bytes = encode_bootstrap_response(&response).unwrap();
        assert_eq!(decode_bootstrap_response(&bytes).unwrap(), response);

        let mut value = serde_json::to_value(response).unwrap();
        value["response"]["payload"]["mutation_authorized"] = serde_json::Value::Bool(true);
        assert_eq!(
            decode_bootstrap_response(&serde_json::to_vec(&value).unwrap()),
            Err(BootstrapCodecError::InvalidResponse)
        );

        assert!(matches!(
            decode_bootstrap_response(&vec![b' '; MAX_BOOTSTRAP_PROTOCOL_BYTES + 1]),
            Err(BootstrapCodecError::Oversized { .. })
        ));
        assert_eq!(bootstrap.snapshot().phase, BootstrapPhase::Planned);
    }

    #[test]
    fn bootstrap_error_response_rejects_control_text() {
        let response = BootstrapResponseEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            response: BootstrapResponse::Error(BootstrapProtocolError {
                bootstrap_id: Some(BootstrapId::new("bootstrap-1").unwrap()),
                code: "transport_failure".into(),
                message: "failed\ntoken=secret".into(),
            }),
        };
        assert_eq!(
            encode_bootstrap_response(&response),
            Err(BootstrapCodecError::InvalidResponse)
        );
    }
}
