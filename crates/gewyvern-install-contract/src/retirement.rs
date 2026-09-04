//! Retirement request, response, and identity-binding validation.

use std::fmt;

use leserpent_domain::RuntimeId;
use leserpent_domain::provisioning::ProvisioningId;
use leserpent_domain::retirement::RetirementId;
use serde::{Deserialize, Serialize};

pub const GEWYVERN_RETIREMENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_GEWYVERN_RETIREMENT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernRetirementRequest {
    pub schema_version: u32,
    pub retirement_id: RetirementId,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub install_profile: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernRetirementResponse {
    pub schema_version: u32,
    pub retirement_id: RetirementId,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub service_retired: bool,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GewyvernRetirementCodecError {
    Oversized,
    InvalidJson,
    InvalidSchema,
    InvalidProfile,
    InvalidResponse,
}

impl fmt::Display for GewyvernRetirementCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid Gewyvern retirement message: {self:?}")
    }
}

impl std::error::Error for GewyvernRetirementCodecError {}

impl GewyvernRetirementRequest {
    pub fn validate(&self) -> Result<(), GewyvernRetirementCodecError> {
        validate_schema(self.schema_version)?;
        if !matches!(self.install_profile.as_str(), "system" | "user") {
            return Err(GewyvernRetirementCodecError::InvalidProfile);
        }
        Ok(())
    }
}

impl GewyvernRetirementResponse {
    pub fn validate(&self) -> Result<(), GewyvernRetirementCodecError> {
        validate_schema(self.schema_version)?;
        if !self.service_retired {
            return Err(GewyvernRetirementCodecError::InvalidResponse);
        }
        Ok(())
    }
}

pub fn encode_gewyvern_retirement_request(
    request: &GewyvernRetirementRequest,
) -> Result<Vec<u8>, GewyvernRetirementCodecError> {
    request.validate()?;
    encode_bounded(request)
}

pub fn decode_gewyvern_retirement_request(
    bytes: &[u8],
) -> Result<GewyvernRetirementRequest, GewyvernRetirementCodecError> {
    require_bound(bytes)?;
    let request = serde_json::from_slice::<GewyvernRetirementRequest>(bytes)
        .map_err(|_| GewyvernRetirementCodecError::InvalidJson)?;
    request.validate()?;
    Ok(request)
}

pub fn encode_gewyvern_retirement_response(
    response: &GewyvernRetirementResponse,
) -> Result<Vec<u8>, GewyvernRetirementCodecError> {
    response.validate()?;
    encode_bounded(response)
}

pub fn decode_gewyvern_retirement_response(
    bytes: &[u8],
) -> Result<GewyvernRetirementResponse, GewyvernRetirementCodecError> {
    require_bound(bytes)?;
    let response = serde_json::from_slice::<GewyvernRetirementResponse>(bytes)
        .map_err(|_| GewyvernRetirementCodecError::InvalidJson)?;
    response.validate()?;
    Ok(response)
}

pub fn validate_gewyvern_retirement_response_binding(
    request: &GewyvernRetirementRequest,
    response: &GewyvernRetirementResponse,
) -> Result<(), GewyvernRetirementCodecError> {
    request.validate()?;
    response.validate()?;
    if response.retirement_id != request.retirement_id
        || response.provisioning_id != request.provisioning_id
        || response.runtime_id != request.runtime_id
    {
        return Err(GewyvernRetirementCodecError::InvalidResponse);
    }
    Ok(())
}

fn validate_schema(actual: u32) -> Result<(), GewyvernRetirementCodecError> {
    if actual != GEWYVERN_RETIREMENT_SCHEMA_VERSION {
        return Err(GewyvernRetirementCodecError::InvalidSchema);
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), GewyvernRetirementCodecError> {
    if bytes.len() > MAX_GEWYVERN_RETIREMENT_BYTES {
        return Err(GewyvernRetirementCodecError::Oversized);
    }
    Ok(())
}

fn encode_bounded(value: &impl Serialize) -> Result<Vec<u8>, GewyvernRetirementCodecError> {
    let bytes = serde_json::to_vec(value).map_err(|_| GewyvernRetirementCodecError::InvalidJson)?;
    require_bound(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request() -> GewyvernRetirementRequest {
        GewyvernRetirementRequest {
            schema_version: GEWYVERN_RETIREMENT_SCHEMA_VERSION,
            retirement_id: RetirementId::new("retire-a").unwrap(),
            provisioning_id: ProvisioningId::new("provision-a").unwrap(),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            install_profile: "user".into(),
        }
    }

    #[test]
    fn strict_request_and_response_bind_all_identities() {
        let request = request();
        let bytes = encode_gewyvern_retirement_request(&request).unwrap();
        assert_eq!(decode_gewyvern_retirement_request(&bytes).unwrap(), request);
        let response = GewyvernRetirementResponse {
            schema_version: GEWYVERN_RETIREMENT_SCHEMA_VERSION,
            retirement_id: request.retirement_id.clone(),
            provisioning_id: request.provisioning_id.clone(),
            runtime_id: request.runtime_id.clone(),
            service_retired: true,
            replayed: false,
        };
        validate_gewyvern_retirement_response_binding(&request, &response).unwrap();

        let mut forged = response.clone();
        forged.runtime_id = RuntimeId::new("runtime-b").unwrap();
        assert!(validate_gewyvern_retirement_response_binding(&request, &forged).is_err());
        let mut unknown = serde_json::to_value(&request).unwrap();
        unknown["password"] = json!("forbidden");
        assert!(
            decode_gewyvern_retirement_request(&serde_json::to_vec(&unknown).unwrap()).is_err()
        );
    }

    #[test]
    fn codec_rejects_invalid_profile_false_receipt_and_oversized_frames() {
        let mut request = request();
        request.install_profile = "test".into();
        assert!(encode_gewyvern_retirement_request(&request).is_err());

        let response = GewyvernRetirementResponse {
            schema_version: GEWYVERN_RETIREMENT_SCHEMA_VERSION,
            retirement_id: RetirementId::new("retire-a").unwrap(),
            provisioning_id: ProvisioningId::new("provision-a").unwrap(),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            service_retired: false,
            replayed: false,
        };
        assert!(encode_gewyvern_retirement_response(&response).is_err());
        assert!(matches!(
            decode_gewyvern_retirement_request(&vec![b' '; MAX_GEWYVERN_RETIREMENT_BYTES + 1]),
            Err(GewyvernRetirementCodecError::Oversized)
        ));
    }
}
