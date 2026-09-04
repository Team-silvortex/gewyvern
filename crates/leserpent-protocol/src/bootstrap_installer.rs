use std::fmt;

use leserpent_domain::bootstrap::{BootstrapId, DaemonId};
use ring::digest::{SHA256, digest};
use serde::{Deserialize, Serialize};
use silvortex_bounded_io::parse_https_origin;
use zeroize::{Zeroize, Zeroizing};

pub const BOOTSTRAP_INSTALLER_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOOTSTRAP_INSTALLER_BYTES: usize = 64 * 1024;
pub const MAX_BOOTSTRAP_ARTIFACT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapInstallerRequest {
    pub schema_version: u32,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub endpoint: String,
    pub install_profile: String,
    pub artifact_sha256: String,
    session_token: String,
}

impl BootstrapInstallerRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bootstrap_id: BootstrapId,
        daemon_id: DaemonId,
        endpoint: impl Into<String>,
        install_profile: impl Into<String>,
        artifact_sha256: impl Into<String>,
        session_token: impl Into<String>,
    ) -> Result<Self, BootstrapInstallerCodecError> {
        let request = Self {
            schema_version: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
            bootstrap_id,
            daemon_id,
            endpoint: endpoint.into(),
            install_profile: install_profile.into(),
            artifact_sha256: artifact_sha256.into(),
            session_token: session_token.into(),
        };
        request.validate()?;
        Ok(request)
    }

    pub fn session_token(&self) -> &str {
        &self.session_token
    }

    pub fn validate(&self) -> Result<(), BootstrapInstallerCodecError> {
        if self.schema_version != BOOTSTRAP_INSTALLER_SCHEMA_VERSION {
            return Err(BootstrapInstallerCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
            });
        }
        if !valid_https_origin(&self.endpoint) {
            return Err(BootstrapInstallerCodecError::InvalidEndpoint);
        }
        if !matches!(self.install_profile.as_str(), "system" | "user" | "test") {
            return Err(BootstrapInstallerCodecError::InvalidInstallProfile);
        }
        if self.artifact_sha256.len() != 64
            || !self
                .artifact_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(BootstrapInstallerCodecError::InvalidArtifactDigest);
        }
        if !(32..=256).contains(&self.session_token.len())
            || self.session_token.chars().any(char::is_whitespace)
            || self.session_token.chars().any(char::is_control)
        {
            return Err(BootstrapInstallerCodecError::InvalidSessionToken);
        }
        Ok(())
    }
}

impl Drop for BootstrapInstallerRequest {
    fn drop(&mut self) {
        self.session_token.zeroize();
    }
}

impl fmt::Debug for BootstrapInstallerRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BootstrapInstallerRequest")
            .field("schema_version", &self.schema_version)
            .field("bootstrap_id", &self.bootstrap_id)
            .field("daemon_id", &self.daemon_id)
            .field("endpoint", &self.endpoint)
            .field("install_profile", &self.install_profile)
            .field("artifact_sha256", &self.artifact_sha256)
            .field("session_token", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BootstrapInstallerServiceState {
    Installed,
    Ready,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapInstallerResponse {
    pub schema_version: u32,
    pub bootstrap_id: BootstrapId,
    pub daemon_id: DaemonId,
    pub endpoint: String,
    pub service_state: BootstrapInstallerServiceState,
    pub generation: String,
    pub replayed: bool,
    pub tls_ca_pem: String,
    pub tls_ca_sha256: String,
}

impl BootstrapInstallerResponse {
    pub fn validate(&self) -> Result<(), BootstrapInstallerCodecError> {
        if self.schema_version != BOOTSTRAP_INSTALLER_SCHEMA_VERSION {
            return Err(BootstrapInstallerCodecError::InvalidSchemaVersion {
                actual: self.schema_version,
                expected: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
            });
        }
        if !valid_https_origin(&self.endpoint) {
            return Err(BootstrapInstallerCodecError::InvalidEndpoint);
        }
        if self.generation.len() != 64
            || !self
                .generation
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(BootstrapInstallerCodecError::InvalidArtifactDigest);
        }
        if !valid_tls_ca_pem(&self.tls_ca_pem)
            || self.tls_ca_sha256.len() != 64
            || !self
                .tls_ca_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(BootstrapInstallerCodecError::InvalidTlsIdentity);
        }
        if self.tls_ca_sha256 != hex(digest(&SHA256, self.tls_ca_pem.as_bytes()).as_ref()) {
            return Err(BootstrapInstallerCodecError::InvalidTlsIdentity);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BootstrapInstallerCodecError {
    Oversized { size: usize, limit: usize },
    InvalidJson,
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidEndpoint,
    InvalidInstallProfile,
    InvalidArtifactDigest,
    InvalidSessionToken,
    InvalidTlsIdentity,
    InvalidResponse,
}

impl fmt::Display for BootstrapInstallerCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Oversized { size, limit } => {
                write!(
                    formatter,
                    "bootstrap installer message size {size} exceeds {limit}"
                )
            }
            Self::InvalidJson => formatter.write_str("invalid bootstrap installer JSON"),
            Self::InvalidSchemaVersion { actual, expected } => write!(
                formatter,
                "unsupported bootstrap installer schema {actual}, expected {expected}"
            ),
            Self::InvalidEndpoint => formatter.write_str("invalid bootstrap daemon endpoint"),
            Self::InvalidInstallProfile => formatter.write_str("invalid bootstrap install profile"),
            Self::InvalidArtifactDigest => formatter.write_str("invalid bootstrap artifact digest"),
            Self::InvalidSessionToken => formatter.write_str("invalid daemon session token"),
            Self::InvalidTlsIdentity => formatter.write_str("invalid daemon TLS identity"),
            Self::InvalidResponse => formatter.write_str("invalid bootstrap installer response"),
        }
    }
}

impl std::error::Error for BootstrapInstallerCodecError {}

pub fn decode_bootstrap_installer_request(
    bytes: &[u8],
) -> Result<BootstrapInstallerRequest, BootstrapInstallerCodecError> {
    require_bound(bytes)?;
    let request: BootstrapInstallerRequest =
        serde_json::from_slice(bytes).map_err(|_| BootstrapInstallerCodecError::InvalidJson)?;
    request.validate()?;
    Ok(request)
}

pub fn encode_bootstrap_installer_request(
    request: &BootstrapInstallerRequest,
) -> Result<Zeroizing<Vec<u8>>, BootstrapInstallerCodecError> {
    request.validate()?;
    let bytes = Zeroizing::new(
        serde_json::to_vec(request).map_err(|_| BootstrapInstallerCodecError::InvalidJson)?,
    );
    require_bound(&bytes)?;
    Ok(bytes)
}

pub fn decode_bootstrap_installer_response(
    bytes: &[u8],
) -> Result<BootstrapInstallerResponse, BootstrapInstallerCodecError> {
    require_bound(bytes)?;
    let response: BootstrapInstallerResponse =
        serde_json::from_slice(bytes).map_err(|_| BootstrapInstallerCodecError::InvalidJson)?;
    response.validate()?;
    Ok(response)
}

pub fn encode_bootstrap_installer_response(
    response: &BootstrapInstallerResponse,
) -> Result<Vec<u8>, BootstrapInstallerCodecError> {
    response.validate()?;
    let bytes =
        serde_json::to_vec(response).map_err(|_| BootstrapInstallerCodecError::InvalidJson)?;
    require_bound(&bytes)?;
    Ok(bytes)
}

fn require_bound(bytes: &[u8]) -> Result<(), BootstrapInstallerCodecError> {
    if bytes.len() > MAX_BOOTSTRAP_INSTALLER_BYTES {
        return Err(BootstrapInstallerCodecError::Oversized {
            size: bytes.len(),
            limit: MAX_BOOTSTRAP_INSTALLER_BYTES,
        });
    }
    Ok(())
}

fn valid_https_origin(value: &str) -> bool {
    parse_https_origin(value).is_some()
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
    use super::*;

    fn request() -> BootstrapInstallerRequest {
        BootstrapInstallerRequest::new(
            BootstrapId::new("bootstrap-1").unwrap(),
            DaemonId::new("daemon-1").unwrap(),
            "https://host.example:7443",
            "test",
            "a".repeat(64),
            "0123456789abcdef0123456789abcdef",
        )
        .unwrap()
    }

    #[test]
    fn installer_request_is_strict_bounded_and_redacted() {
        let request = request();
        let encoded = encode_bootstrap_installer_request(&request).unwrap();
        assert_eq!(
            decode_bootstrap_installer_request(&encoded)
                .unwrap()
                .session_token(),
            "0123456789abcdef0123456789abcdef"
        );
        assert!(!format!("{request:?}").contains(request.session_token()));

        let mut value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        value["command"] = serde_json::json!("sh");
        assert!(matches!(
            decode_bootstrap_installer_request(&serde_json::to_vec(&value).unwrap()),
            Err(BootstrapInstallerCodecError::InvalidJson)
        ));
        assert!(matches!(
            decode_bootstrap_installer_request(&vec![b'x'; MAX_BOOTSTRAP_INSTALLER_BYTES + 1]),
            Err(BootstrapInstallerCodecError::Oversized { .. })
        ));
    }

    #[test]
    fn installer_response_distinguishes_installation_from_readiness() {
        for service_state in [
            BootstrapInstallerServiceState::Installed,
            BootstrapInstallerServiceState::Ready,
        ] {
            let tls_ca_pem = "-----BEGIN CERTIFICATE-----\nY2VydA==\n-----END CERTIFICATE-----\n";
            let response = BootstrapInstallerResponse {
                schema_version: BOOTSTRAP_INSTALLER_SCHEMA_VERSION,
                bootstrap_id: BootstrapId::new("bootstrap-1").unwrap(),
                daemon_id: DaemonId::new("daemon-1").unwrap(),
                endpoint: "https://host.example:7443".into(),
                service_state,
                generation: "a".repeat(64),
                replayed: false,
                tls_ca_pem: tls_ca_pem.into(),
                tls_ca_sha256: hex(digest(&SHA256, tls_ca_pem.as_bytes()).as_ref()),
            };
            let encoded = encode_bootstrap_installer_response(&response).unwrap();
            assert_eq!(
                decode_bootstrap_installer_response(&encoded).unwrap(),
                response
            );
        }
    }

    #[test]
    fn installer_credentials_and_digest_are_validated() {
        for endpoint in [
            "http://host.example:7443",
            "https://host.example:+443",
            "https://host.example:0443",
            "https://host.example\\ignored",
            "https://127.1:7443",
            "https://host_example:7443",
        ] {
            assert!(
                BootstrapInstallerRequest::new(
                    BootstrapId::new("bootstrap-1").unwrap(),
                    DaemonId::new("daemon-1").unwrap(),
                    endpoint,
                    "test",
                    "a".repeat(64),
                    "0123456789abcdef0123456789abcdef",
                )
                .is_err(),
                "unsafe endpoint was accepted: {endpoint:?}"
            );
        }

        assert!(
            BootstrapInstallerRequest::new(
                BootstrapId::new("bootstrap-1").unwrap(),
                DaemonId::new("daemon-1").unwrap(),
                "https://host.example:7443",
                "test",
                "A".repeat(64),
                "short",
            )
            .is_err()
        );
    }
}
