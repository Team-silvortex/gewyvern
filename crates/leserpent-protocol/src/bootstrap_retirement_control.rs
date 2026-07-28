use std::fmt;

use leserpent_domain::bootstrap_retirement::{
    DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION, DaemonRetirementCheckpoint, DaemonRetirementError,
    DaemonRetirementIntent, DaemonRetirementPhase, DaemonRetirementSnapshot,
};
use leserpent_domain::{CapabilitySet, Principal};
use serde::{Deserialize, Serialize};

pub const DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementRequestEnvelope {
    pub schema_version: u32,
    pub request: DaemonRetirementRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub intent: DaemonRetirementIntent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementEffectEnvelope {
    pub schema_version: u32,
    pub checkpoint: DaemonRetirementCheckpoint,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementResponseEnvelope {
    pub schema_version: u32,
    pub response: DaemonRetirementResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum DaemonRetirementResponse {
    State(DaemonRetirementSnapshot),
    Error(DaemonRetirementProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonRetirementProtocolError {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retirement_id: Option<leserpent_domain::retirement::RetirementId>,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DaemonRetirementCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidRequest(DaemonRetirementError),
    InvalidEffect,
    InvalidResponse,
}

impl fmt::Display for DaemonRetirementCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => write!(
                formatter,
                "daemon retirement message size {size} exceeds {limit}"
            ),
            Self::InvalidJson(error) => {
                write!(formatter, "invalid daemon retirement JSON: {error}")
            }
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported daemon retirement protocol schema {actual}, expected {expected}"
            ),
            Self::InvalidRequest(error) => {
                write!(formatter, "invalid daemon retirement request: {error}")
            }
            Self::InvalidEffect => formatter.write_str("invalid daemon retirement effect"),
            Self::InvalidResponse => formatter.write_str("invalid daemon retirement response"),
        }
    }
}

impl std::error::Error for DaemonRetirementCodecError {}

pub fn decode_daemon_retirement_request(
    bytes: &[u8],
) -> Result<DaemonRetirementRequestEnvelope, DaemonRetirementCodecError> {
    require_bound(bytes)?;
    let envelope = serde_json::from_slice::<DaemonRetirementRequestEnvelope>(bytes)
        .map_err(|error| DaemonRetirementCodecError::InvalidJson(error.to_string()))?;
    validate_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    Ok(envelope)
}

pub fn encode_daemon_retirement_request(
    envelope: &DaemonRetirementRequestEnvelope,
) -> Result<Vec<u8>, DaemonRetirementCodecError> {
    validate_schema(envelope.schema_version)?;
    validate_request(&envelope.request)?;
    encode_bounded(envelope)
}

pub fn decode_daemon_retirement_effect(
    bytes: &[u8],
) -> Result<DaemonRetirementEffectEnvelope, DaemonRetirementCodecError> {
    require_bound(bytes)?;
    let envelope = serde_json::from_slice::<DaemonRetirementEffectEnvelope>(bytes)
        .map_err(|error| DaemonRetirementCodecError::InvalidJson(error.to_string()))?;
    validate_schema(envelope.schema_version)?;
    validate_effect(&envelope)?;
    Ok(envelope)
}

pub fn encode_daemon_retirement_effect(
    envelope: &DaemonRetirementEffectEnvelope,
) -> Result<Vec<u8>, DaemonRetirementCodecError> {
    validate_schema(envelope.schema_version)?;
    validate_effect(envelope)?;
    encode_bounded(envelope)
}

pub fn decode_daemon_retirement_response(
    bytes: &[u8],
) -> Result<DaemonRetirementResponseEnvelope, DaemonRetirementCodecError> {
    require_bound(bytes)?;
    let envelope = serde_json::from_slice::<DaemonRetirementResponseEnvelope>(bytes)
        .map_err(|error| DaemonRetirementCodecError::InvalidJson(error.to_string()))?;
    validate_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    Ok(envelope)
}

pub fn encode_daemon_retirement_response(
    envelope: &DaemonRetirementResponseEnvelope,
) -> Result<Vec<u8>, DaemonRetirementCodecError> {
    validate_schema(envelope.schema_version)?;
    validate_response(&envelope.response)?;
    encode_bounded(envelope)
}

fn validate_request(request: &DaemonRetirementRequest) -> Result<(), DaemonRetirementCodecError> {
    if request.intent.schema_version != DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION {
        return Err(DaemonRetirementCodecError::InvalidRequest(
            DaemonRetirementError::InvalidSchemaVersion {
                actual: request.intent.schema_version,
                expected: DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION,
            },
        ));
    }
    request
        .intent
        .validate()
        .map_err(DaemonRetirementCodecError::InvalidRequest)?;
    if request.principal.id != request.intent.requested_by {
        return Err(DaemonRetirementCodecError::InvalidRequest(
            DaemonRetirementError::PrincipalMismatch,
        ));
    }
    if !request
        .capabilities
        .contains(leserpent_domain::bootstrap_retirement::CAPABILITY_HOST_RETIRE)
    {
        return Err(DaemonRetirementCodecError::InvalidRequest(
            DaemonRetirementError::Unauthorized,
        ));
    }
    Ok(())
}

fn validate_effect(
    envelope: &DaemonRetirementEffectEnvelope,
) -> Result<(), DaemonRetirementCodecError> {
    envelope
        .checkpoint
        .validate()
        .map_err(|_| DaemonRetirementCodecError::InvalidEffect)?;
    if envelope.checkpoint.revision != 1
        || envelope.checkpoint.state.phase != DaemonRetirementPhase::Planned
    {
        return Err(DaemonRetirementCodecError::InvalidEffect);
    }
    Ok(())
}

fn validate_response(
    response: &DaemonRetirementResponse,
) -> Result<(), DaemonRetirementCodecError> {
    match response {
        DaemonRetirementResponse::State(state) => state
            .validate()
            .map_err(|_| DaemonRetirementCodecError::InvalidResponse),
        DaemonRetirementResponse::Error(error) => {
            let valid = valid_code(&error.code)
                && !error.message.is_empty()
                && error.message.len() <= 512
                && error.message == error.message.trim()
                && !error.message.chars().any(char::is_control);
            valid
                .then_some(())
                .ok_or(DaemonRetirementCodecError::InvalidResponse)
        }
    }
}

fn validate_schema(actual: u32) -> Result<(), DaemonRetirementCodecError> {
    if actual != DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION {
        return Err(DaemonRetirementCodecError::InvalidSchemaVersion {
            actual,
            expected: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), DaemonRetirementCodecError> {
    if bytes.len() > MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES {
        return Err(DaemonRetirementCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES,
        });
    }
    Ok(())
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, DaemonRetirementCodecError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|error| DaemonRetirementCodecError::InvalidJson(error.to_string()))?;
    require_bound(&bytes)?;
    Ok(bytes)
}

fn valid_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use leserpent_domain::bootstrap::{
        BootstrapId, BootstrapTarget, BootstrapTransport, CredentialHandle, DaemonId,
    };
    use leserpent_domain::bootstrap_retirement::{
        CAPABILITY_HOST_RETIRE, DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
    };
    use leserpent_domain::retirement::RetirementId;
    use serde_json::json;

    use super::*;

    fn request() -> DaemonRetirementRequestEnvelope {
        DaemonRetirementRequestEnvelope {
            schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            request: DaemonRetirementRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
                intent: DaemonRetirementIntent {
                    schema_version: DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION,
                    retirement_id: RetirementId::new("retire-daemon-1").unwrap(),
                    bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                    retirement_credential_handle: CredentialHandle::new("vault:ssh:host").unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        }
    }

    fn checkpoint() -> DaemonRetirementCheckpoint {
        DaemonRetirementCheckpoint {
            schema_version: DAEMON_RETIREMENT_CHECKPOINT_SCHEMA_VERSION,
            revision: 1,
            state: DaemonRetirementSnapshot {
                retirement_id: RetirementId::new("retire-daemon-1").unwrap(),
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                daemon_id: DaemonId::new("daemon-host").unwrap(),
                phase: DaemonRetirementPhase::Planned,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                generation: "a".repeat(64),
                install_profile: "system".into(),
                retirement_credential_present: true,
                service_retired: false,
                fault_code: None,
            },
            retirement_credential_handle: Some(CredentialHandle::new("vault:ssh:host").unwrap()),
        }
    }

    #[test]
    fn public_request_contains_no_target_authority_fields() {
        let request = request();
        let encoded = encode_daemon_retirement_request(&request).unwrap();
        let text = String::from_utf8(encoded.clone()).unwrap();
        assert!(!text.contains("generation"));
        assert!(!text.contains("install_profile"));
        assert!(!text.contains("daemon_id"));
        assert!(!text.contains("\"target\""));
        assert_eq!(decode_daemon_retirement_request(&encoded).unwrap(), request);

        let mut forged = serde_json::to_value(&request).unwrap();
        forged["request"]["intent"]["generation"] = json!("a".repeat(64));
        assert!(matches!(
            decode_daemon_retirement_request(&serde_json::to_vec(&forged).unwrap()),
            Err(DaemonRetirementCodecError::InvalidJson(_))
        ));
    }

    #[test]
    fn internal_effect_is_bounded_strict_and_authority_complete() {
        let effect = DaemonRetirementEffectEnvelope {
            schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            checkpoint: checkpoint(),
        };
        let encoded = encode_daemon_retirement_effect(&effect).unwrap();
        assert_eq!(decode_daemon_retirement_effect(&encoded).unwrap(), effect);

        let mut invalid = effect;
        invalid.checkpoint.state.install_profile = "portable".into();
        assert_eq!(
            encode_daemon_retirement_effect(&invalid),
            Err(DaemonRetirementCodecError::InvalidEffect)
        );
        assert!(matches!(
            decode_daemon_retirement_effect(&vec![b'x'; MAX_DAEMON_RETIREMENT_PROTOCOL_BYTES + 1]),
            Err(DaemonRetirementCodecError::Oversized { .. })
        ));
    }

    #[test]
    fn terminal_response_is_strict_and_validated() {
        let mut state = checkpoint().state;
        state.phase = DaemonRetirementPhase::ServiceRetired;
        state.retirement_credential_present = false;
        state.service_retired = true;
        let response = DaemonRetirementResponseEnvelope {
            schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            response: DaemonRetirementResponse::State(state),
        };
        let encoded = encode_daemon_retirement_response(&response).unwrap();
        assert_eq!(
            decode_daemon_retirement_response(&encoded).unwrap(),
            response
        );
    }
}
