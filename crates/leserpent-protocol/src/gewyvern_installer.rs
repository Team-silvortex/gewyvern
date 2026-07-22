use std::fmt;

use leserpent_domain::RuntimeId;
use leserpent_domain::bootstrap::CredentialHandle;
use leserpent_domain::provisioning::ProvisioningId;
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, Zeroizing};

pub const GEWYVERN_INSTALLER_SCHEMA_VERSION: u32 = 1;
pub const MAX_GEWYVERN_INSTALLER_BYTES: usize = 64 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernInstallerRequest {
    pub schema_version: u32,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub endpoint: String,
    pub install_profile: String,
    pub artifact_sha256: String,
    pub api_credential_handle: CredentialHandle,
    pub trust_credential_handle: CredentialHandle,
    api_token: String,
}

impl GewyvernInstallerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provisioning_id: ProvisioningId,
        runtime_id: RuntimeId,
        endpoint: impl Into<String>,
        install_profile: impl Into<String>,
        artifact_sha256: impl Into<String>,
        api_credential_handle: CredentialHandle,
        trust_credential_handle: CredentialHandle,
        api_token: impl Into<String>,
    ) -> Result<Self, GewyvernInstallerCodecError> {
        let request = Self {
            schema_version: GEWYVERN_INSTALLER_SCHEMA_VERSION,
            provisioning_id,
            runtime_id,
            endpoint: endpoint.into(),
            install_profile: install_profile.into(),
            artifact_sha256: artifact_sha256.into(),
            api_credential_handle,
            trust_credential_handle,
            api_token: api_token.into(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn api_token(&self) -> &str {
        &self.api_token
    }

    pub fn validate(&self) -> Result<(), GewyvernInstallerCodecError> {
        if self.schema_version != GEWYVERN_INSTALLER_SCHEMA_VERSION {
            return Err(GewyvernInstallerCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: GEWYVERN_INSTALLER_SCHEMA_VERSION,
            });
        }
        if !valid_https_origin(&self.endpoint) {
            return Err(GewyvernInstallerCodecError::InvalidEndpoint);
        }
        if !matches!(self.install_profile.as_str(), "system" | "user" | "test") {
            return Err(GewyvernInstallerCodecError::InvalidInstallProfile);
        }
        if !valid_sha256(&self.artifact_sha256) {
            return Err(GewyvernInstallerCodecError::InvalidArtifactDigest);
        }
        if !(32..=256).contains(&self.api_token.len())
            || self.api_token.chars().any(char::is_whitespace)
            || self.api_token.chars().any(char::is_control)
        {
            return Err(GewyvernInstallerCodecError::InvalidApiToken);
        }
        let (api_provider, _) = self.api_credential_handle.parts();
        let (trust_provider, _) = self.trust_credential_handle.parts();
        if api_provider != "gewyvern" || trust_provider != "gewyvern-ca" {
            return Err(GewyvernInstallerCodecError::InvalidCredentialBinding);
        }
        Ok(())
    }
}

impl Drop for GewyvernInstallerRequest {
    fn drop(&mut self) {
        self.api_token.zeroize();
    }
}

impl fmt::Debug for GewyvernInstallerRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GewyvernInstallerRequest")
            .field("schema_version", &self.schema_version)
            .field("provisioning_id", &self.provisioning_id)
            .field("runtime_id", &self.runtime_id)
            .field("endpoint", &self.endpoint)
            .field("install_profile", &self.install_profile)
            .field("artifact_sha256", &self.artifact_sha256)
            .field("api_credential_handle", &self.api_credential_handle)
            .field("trust_credential_handle", &self.trust_credential_handle)
            .field("api_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GewyvernInstallerServiceState {
    Installed,
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernInstallerResponse {
    pub schema_version: u32,
    pub provisioning_id: ProvisioningId,
    pub runtime_id: RuntimeId,
    pub endpoint: String,
    pub service_state: GewyvernInstallerServiceState,
    pub generation: String,
    pub replayed: bool,
    pub api_credential_handle: CredentialHandle,
    pub trust_credential_handle: CredentialHandle,
    pub tls_ca_pem: String,
    pub tls_ca_sha256: String,
}

impl GewyvernInstallerResponse {
    pub fn validate(&self) -> Result<(), GewyvernInstallerCodecError> {
        if self.schema_version != GEWYVERN_INSTALLER_SCHEMA_VERSION {
            return Err(GewyvernInstallerCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: GEWYVERN_INSTALLER_SCHEMA_VERSION,
            });
        }
        if !valid_https_origin(&self.endpoint) {
            return Err(GewyvernInstallerCodecError::InvalidEndpoint);
        }
        if !valid_sha256(&self.generation) {
            return Err(GewyvernInstallerCodecError::InvalidArtifactDigest);
        }
        let (api_provider, _) = self.api_credential_handle.parts();
        let (trust_provider, _) = self.trust_credential_handle.parts();
        if api_provider != "gewyvern" || trust_provider != "gewyvern-ca" {
            return Err(GewyvernInstallerCodecError::InvalidCredentialBinding);
        }
        if !valid_tls_ca_pem(&self.tls_ca_pem)
            || !valid_sha256(&self.tls_ca_sha256)
            || self.tls_ca_sha256 != hex(digest(&SHA256, self.tls_ca_pem.as_bytes()).as_ref())
        {
            return Err(GewyvernInstallerCodecError::InvalidTlsIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GewyvernInstallerCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson,
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidEndpoint,
    InvalidInstallProfile,
    InvalidArtifactDigest,
    InvalidApiToken,
    InvalidCredentialBinding,
    InvalidTlsIdentity,
    InvalidResponseBinding,
}

impl fmt::Display for GewyvernInstallerCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(
                    formatter,
                    "Gewyvern installer message size {size} exceeds {limit}"
                )
            }
            Self::InvalidJson => formatter.write_str("invalid Gewyvern installer JSON"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported Gewyvern installer schema {actual}, expected {expected}"
            ),
            Self::InvalidEndpoint => formatter.write_str("invalid Gewyvern service endpoint"),
            Self::InvalidInstallProfile => formatter.write_str("invalid Gewyvern install profile"),
            Self::InvalidArtifactDigest => formatter.write_str("invalid Gewyvern artifact digest"),
            Self::InvalidApiToken => formatter.write_str("invalid Gewyvern API token"),
            Self::InvalidCredentialBinding => {
                formatter.write_str("invalid Gewyvern credential binding")
            }
            Self::InvalidTlsIdentity => formatter.write_str("invalid Gewyvern TLS identity"),
            Self::InvalidResponseBinding => {
                formatter.write_str("Gewyvern installer response does not match its request")
            }
        }
    }
}

impl std::error::Error for GewyvernInstallerCodecError {}

pub fn decode_gewyvern_installer_request(
    bytes: &[u8],
) -> Result<GewyvernInstallerRequest, GewyvernInstallerCodecError> {
    require_bound(bytes)?;
    let request = serde_json::from_slice::<GewyvernInstallerRequest>(bytes)
        .map_err(|_| GewyvernInstallerCodecError::InvalidJson)?;
    request.validate()?;
    Ok(request)
}

pub fn encode_gewyvern_installer_request(
    request: &GewyvernInstallerRequest,
) -> Result<Zeroizing<Vec<u8>>, GewyvernInstallerCodecError> {
    request.validate()?;
    let bytes = Zeroizing::new(
        serde_json::to_vec(request).map_err(|_| GewyvernInstallerCodecError::InvalidJson)?,
    );
    require_bound(&bytes)?;
    Ok(bytes)
}

pub fn decode_gewyvern_installer_response(
    bytes: &[u8],
) -> Result<GewyvernInstallerResponse, GewyvernInstallerCodecError> {
    require_bound(bytes)?;
    let response = serde_json::from_slice::<GewyvernInstallerResponse>(bytes)
        .map_err(|_| GewyvernInstallerCodecError::InvalidJson)?;
    response.validate()?;
    Ok(response)
}

pub fn encode_gewyvern_installer_response(
    response: &GewyvernInstallerResponse,
) -> Result<Vec<u8>, GewyvernInstallerCodecError> {
    response.validate()?;
    let bytes =
        serde_json::to_vec(response).map_err(|_| GewyvernInstallerCodecError::InvalidJson)?;
    require_bound(&bytes)?;
    Ok(bytes)
}

pub fn validate_gewyvern_installer_readiness(
    request: &GewyvernInstallerRequest,
    response: &GewyvernInstallerResponse,
) -> Result<(), GewyvernInstallerCodecError> {
    request.validate()?;
    response.validate()?;
    if response.provisioning_id != request.provisioning_id
        || response.runtime_id != request.runtime_id
        || response.endpoint != request.endpoint
        || response.service_state != GewyvernInstallerServiceState::Ready
        || response.generation != request.artifact_sha256
        || response.api_credential_handle != request.api_credential_handle
        || response.trust_credential_handle != request.trust_credential_handle
    {
        return Err(GewyvernInstallerCodecError::InvalidResponseBinding);
    }
    Ok(())
}

fn require_bound(bytes: &[u8]) -> Result<(), GewyvernInstallerCodecError> {
    if bytes.len() > MAX_GEWYVERN_INSTALLER_BYTES {
        return Err(GewyvernInstallerCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_GEWYVERN_INSTALLER_BYTES,
        });
    }
    Ok(())
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn valid_https_origin(value: &str) -> bool {
    let Some(authority) = value.strip_prefix("https://") else {
        return false;
    };
    !authority.is_empty()
        && authority.len() <= 320
        && !authority.contains(['/', '?', '#', '@'])
        && !authority.chars().any(char::is_whitespace)
}

fn valid_tls_ca_pem(value: &str) -> bool {
    value.len() <= 32 * 1024
        && value.starts_with("-----BEGIN CERTIFICATE-----\n")
        && value.ends_with("-----END CERTIFICATE-----\n")
        && value
            .chars()
            .all(|character| character == '\n' || !character.is_control())
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";
    const CA_PEM: &str =
        "-----BEGIN CERTIFICATE-----\nZ2V3eXZlcm4tdGVzdC1jYQ==\n-----END CERTIFICATE-----\n";

    fn request() -> GewyvernInstallerRequest {
        GewyvernInstallerRequest::new(
            ProvisioningId::new("provision-1").unwrap(),
            RuntimeId::new("runtime-1").unwrap(),
            "https://runtime.example:9443",
            "test",
            "a".repeat(64),
            CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
            CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
            TOKEN,
        )
        .unwrap()
    }

    fn ready_response() -> GewyvernInstallerResponse {
        GewyvernInstallerResponse {
            schema_version: GEWYVERN_INSTALLER_SCHEMA_VERSION,
            provisioning_id: ProvisioningId::new("provision-1").unwrap(),
            runtime_id: RuntimeId::new("runtime-1").unwrap(),
            endpoint: "https://runtime.example:9443".into(),
            service_state: GewyvernInstallerServiceState::Ready,
            generation: "a".repeat(64),
            replayed: false,
            api_credential_handle: CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:gewyvern-ca:runtime-ca").unwrap(),
            tls_ca_pem: CA_PEM.into(),
            tls_ca_sha256: hex(digest(&SHA256, CA_PEM.as_bytes()).as_ref()),
        }
    }

    #[test]
    fn request_is_strict_bounded_and_debug_redacted() {
        let request = request();
        let encoded = encode_gewyvern_installer_request(&request).unwrap();
        assert_eq!(
            decode_gewyvern_installer_request(&encoded)
                .unwrap()
                .api_token(),
            TOKEN
        );
        let debug = format!("{request:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains(TOKEN));

        let mut unknown = serde_json::to_value(&request).unwrap();
        unknown["private_key"] = json!("forbidden");
        assert!(matches!(
            decode_gewyvern_installer_request(&serde_json::to_vec(&unknown).unwrap()),
            Err(GewyvernInstallerCodecError::InvalidJson)
        ));
        assert!(matches!(
            decode_gewyvern_installer_request(&vec![b' '; MAX_GEWYVERN_INSTALLER_BYTES + 1]),
            Err(GewyvernInstallerCodecError::Oversized { .. })
        ));
    }

    #[test]
    fn response_binds_identity_handles_readiness_and_ca_digest() {
        let response = ready_response();
        let encoded = encode_gewyvern_installer_response(&response).unwrap();
        assert_eq!(
            decode_gewyvern_installer_response(&encoded).unwrap(),
            response
        );
        assert!(!String::from_utf8(encoded).unwrap().contains(TOKEN));

        let mut wrong_digest = ready_response();
        wrong_digest.tls_ca_sha256 = "b".repeat(64);
        assert_eq!(
            encode_gewyvern_installer_response(&wrong_digest),
            Err(GewyvernInstallerCodecError::InvalidTlsIdentity)
        );
        let mut wrong_handle = ready_response();
        wrong_handle.api_credential_handle =
            CredentialHandle::new("vault:leserpentd:runtime-api").unwrap();
        assert_eq!(
            encode_gewyvern_installer_response(&wrong_handle),
            Err(GewyvernInstallerCodecError::InvalidCredentialBinding)
        );
    }

    #[test]
    fn readiness_validation_rejects_installed_or_identity_drift() {
        let request = request();
        let response = ready_response();
        validate_gewyvern_installer_readiness(&request, &response).unwrap();

        let mut installed = ready_response();
        installed.service_state = GewyvernInstallerServiceState::Installed;
        assert_eq!(
            validate_gewyvern_installer_readiness(&request, &installed),
            Err(GewyvernInstallerCodecError::InvalidResponseBinding)
        );
        let mut confused = ready_response();
        confused.runtime_id = RuntimeId::new("runtime-other").unwrap();
        assert_eq!(
            validate_gewyvern_installer_readiness(&request, &confused),
            Err(GewyvernInstallerCodecError::InvalidResponseBinding)
        );
    }

    #[test]
    fn canonical_v1_fixtures_decode_without_shape_guessing() {
        let request = decode_gewyvern_installer_request(include_bytes!(
            "../tests/fixtures/gewyvern-installer-request-v1.json"
        ))
        .unwrap();
        assert_eq!(request.provisioning_id.as_str(), "provision-fixture-1");
        assert_eq!(request.api_token(), TOKEN);

        let response = decode_gewyvern_installer_response(include_bytes!(
            "../tests/fixtures/gewyvern-installer-ready-response-v1.json"
        ))
        .unwrap();
        assert_eq!(response.service_state, GewyvernInstallerServiceState::Ready);
        assert_eq!(response.runtime_id.as_str(), "runtime-fixture-1");
    }
}
