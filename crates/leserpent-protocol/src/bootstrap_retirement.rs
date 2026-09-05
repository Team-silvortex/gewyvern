use std::fmt;

use leserpent_domain::bootstrap::{BootstrapId, DaemonId};
use serde::{Deserialize, Serialize};

use crate::{BoundedJsonEncodeError, encode_json_bounded};

pub const BOOTSTRAP_RETIREMENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOOTSTRAP_RETIREMENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRetirementRequest {
    pub schema_version: u32,
    pub retirement_id: String,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub generation: String,
    pub install_profile: String,
}

impl BootstrapRetirementRequest {
    pub fn new(
        retirement_id: impl Into<String>,
        bootstrap_id: BootstrapId,
        daemon_id: DaemonId,
        generation: impl Into<String>,
        install_profile: impl Into<String>,
    ) -> Result<Self, BootstrapRetirementCodecError> {
        let request = Self {
            schema_version: BOOTSTRAP_RETIREMENT_SCHEMA_VERSION,
            retirement_id: retirement_id.into(),
            bootstrap_id,
            daemon_id,
            generation: generation.into(),
            install_profile: install_profile.into(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn validate(&self) -> Result<(), BootstrapRetirementCodecError> {
        if self.schema_version != BOOTSTRAP_RETIREMENT_SCHEMA_VERSION {
            return Err(BootstrapRetirementCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: BOOTSTRAP_RETIREMENT_SCHEMA_VERSION,
            });
        }
        if !valid_id(&self.retirement_id) {
            return Err(BootstrapRetirementCodecError::InvalidRetirementId);
        }
        if !valid_generation(&self.generation) {
            return Err(BootstrapRetirementCodecError::InvalidGeneration);
        }
        if !matches!(self.install_profile.as_str(), "system" | "user" | "test") {
            return Err(BootstrapRetirementCodecError::InvalidInstallProfile);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapRetirementResponse {
    pub schema_version: u32,
    pub retirement_id: String,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub generation: String,
    pub service_retired: bool,
    pub replayed: bool,
}

impl BootstrapRetirementResponse {
    pub fn validate(&self) -> Result<(), BootstrapRetirementCodecError> {
        if self.schema_version != BOOTSTRAP_RETIREMENT_SCHEMA_VERSION {
            return Err(BootstrapRetirementCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: BOOTSTRAP_RETIREMENT_SCHEMA_VERSION,
            });
        }
        if !valid_id(&self.retirement_id) {
            return Err(BootstrapRetirementCodecError::InvalidRetirementId);
        }
        if !valid_generation(&self.generation) {
            return Err(BootstrapRetirementCodecError::InvalidGeneration);
        }
        if !self.service_retired {
            return Err(BootstrapRetirementCodecError::InvalidResponse);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapRetirementCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson,
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidRetirementId,
    InvalidGeneration,
    InvalidInstallProfile,
    InvalidResponse,
}

impl fmt::Display for BootstrapRetirementCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(
                    formatter,
                    "bootstrap retirement message size {size} exceeds {limit}"
                )
            }
            Self::InvalidJson => formatter.write_str("invalid bootstrap retirement JSON"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported bootstrap retirement schema {actual}, expected {expected}"
            ),
            Self::InvalidRetirementId => {
                formatter.write_str("invalid bootstrap retirement identity")
            }
            Self::InvalidGeneration => {
                formatter.write_str("invalid bootstrap retirement generation")
            }
            Self::InvalidInstallProfile => {
                formatter.write_str("invalid bootstrap retirement install profile")
            }
            Self::InvalidResponse => formatter.write_str("invalid bootstrap retirement response"),
        }
    }
}

impl std::error::Error for BootstrapRetirementCodecError {}

pub fn decode_bootstrap_retirement_request(
    bytes: &[u8],
) -> Result<BootstrapRetirementRequest, BootstrapRetirementCodecError> {
    require_bound(bytes)?;
    let request = serde_json::from_slice::<BootstrapRetirementRequest>(bytes)
        .map_err(|_| BootstrapRetirementCodecError::InvalidJson)?;
    request.validate()?;
    Ok(request)
}

pub fn encode_bootstrap_retirement_request(
    request: &BootstrapRetirementRequest,
) -> Result<Vec<u8>, BootstrapRetirementCodecError> {
    request.validate()?;
    encode_bounded(request)
}

pub fn decode_bootstrap_retirement_response(
    bytes: &[u8],
) -> Result<BootstrapRetirementResponse, BootstrapRetirementCodecError> {
    require_bound(bytes)?;
    let response = serde_json::from_slice::<BootstrapRetirementResponse>(bytes)
        .map_err(|_| BootstrapRetirementCodecError::InvalidJson)?;
    response.validate()?;
    Ok(response)
}

pub fn encode_bootstrap_retirement_response(
    response: &BootstrapRetirementResponse,
) -> Result<Vec<u8>, BootstrapRetirementCodecError> {
    response.validate()?;
    encode_bounded(response)
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, BootstrapRetirementCodecError> {
    encode_json_bounded(value, MAX_BOOTSTRAP_RETIREMENT_BYTES).map_err(|error| match error {
        BoundedJsonEncodeError::Oversized { size, limit } => {
            BootstrapRetirementCodecError::Oversized { size, limit }
        }
        BoundedJsonEncodeError::InvalidJson(_) => BootstrapRetirementCodecError::InvalidJson,
    })
}

pub fn validate_bootstrap_retirement_response_binding(
    request: &BootstrapRetirementRequest,
    response: &BootstrapRetirementResponse,
) -> Result<(), BootstrapRetirementCodecError> {
    request.validate()?;
    response.validate()?;
    if response.retirement_id != request.retirement_id
        || response.bootstrap_id != request.bootstrap_id
        || response.daemon_id != request.daemon_id
        || response.generation != request.generation
    {
        return Err(BootstrapRetirementCodecError::InvalidResponse);
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), BootstrapRetirementCodecError> {
    if bytes.len() > MAX_BOOTSTRAP_RETIREMENT_BYTES {
        return Err(BootstrapRetirementCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_BOOTSTRAP_RETIREMENT_BYTES,
        });
    }
    Ok(())
}

fn valid_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
}

fn valid_generation(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BootstrapRetirementRequest {
        BootstrapRetirementRequest::new(
            "retire-bootstrap-1",
            BootstrapId::new("bootstrap-1").unwrap(),
            DaemonId::new("daemon-1").unwrap(),
            "a".repeat(64),
            "user",
        )
        .unwrap()
    }

    #[test]
    fn retirement_wire_is_strict_bounded_and_identity_bound() {
        let request = request();
        let encoded = encode_bootstrap_retirement_request(&request).unwrap();
        assert_eq!(
            decode_bootstrap_retirement_request(&encoded).unwrap(),
            request
        );

        let response = BootstrapRetirementResponse {
            schema_version: BOOTSTRAP_RETIREMENT_SCHEMA_VERSION,
            retirement_id: request.retirement_id.clone(),
            bootstrap_id: request.bootstrap_id.clone(),
            daemon_id: request.daemon_id.clone(),
            generation: request.generation.clone(),
            service_retired: true,
            replayed: false,
        };
        let encoded = encode_bootstrap_retirement_response(&response).unwrap();
        assert_eq!(
            decode_bootstrap_retirement_response(&encoded).unwrap(),
            response
        );
        validate_bootstrap_retirement_response_binding(&request, &response).unwrap();

        let mut forged = response;
        forged.generation = "b".repeat(64);
        assert!(validate_bootstrap_retirement_response_binding(&request, &forged).is_err());
    }

    #[test]
    fn retirement_wire_rejects_unknown_fields_invalid_state_and_oversize() {
        let mut value = serde_json::to_value(request()).unwrap();
        value["command"] = serde_json::json!("rm -rf");
        assert!(matches!(
            decode_bootstrap_retirement_request(&serde_json::to_vec(&value).unwrap()),
            Err(BootstrapRetirementCodecError::InvalidJson)
        ));

        let mut invalid = request();
        invalid.install_profile = "portable".into();
        assert!(encode_bootstrap_retirement_request(&invalid).is_err());
        assert!(matches!(
            decode_bootstrap_retirement_request(&vec![b'x'; MAX_BOOTSTRAP_RETIREMENT_BYTES + 1]),
            Err(BootstrapRetirementCodecError::Oversized { .. })
        ));

        let response = BootstrapRetirementResponse {
            schema_version: BOOTSTRAP_RETIREMENT_SCHEMA_VERSION,
            retirement_id: "retire-bootstrap-1".into(),
            bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
            daemon_id: DaemonId::new("daemon-1").unwrap(),
            generation: "a".repeat(64),
            service_retired: false,
            replayed: false,
        };
        assert!(encode_bootstrap_retirement_response(&response).is_err());
    }
}
