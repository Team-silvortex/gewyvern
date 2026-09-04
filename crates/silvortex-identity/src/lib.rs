//! Product-neutral validated identities shared across native protocol boundaries.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Maximum UTF-8 byte length accepted by the shared ASCII identifier grammar.
pub const MAX_IDENTIFIER_BYTES: usize = 128;
/// Prefix required by every validated credential handle.
pub const CREDENTIAL_HANDLE_PREFIX: &str = "vault:";

/// A product-neutral identity construction or decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    /// The value does not satisfy the shared bounded ASCII identifier grammar.
    InvalidIdentifier { field: &'static str },
    /// The value is not a non-empty `vault:<provider>:<key>` handle.
    InvalidCredentialHandle,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidCredentialHandle => formatter.write_str("invalid credential handle"),
        }
    }
}

impl std::error::Error for IdentityError {}

macro_rules! validated_identifier {
    ($name:ident, $field:literal, $documentation:literal) => {
        #[doc = $documentation]
        #[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            /// Constructs an identity after enforcing the shared bounded ASCII grammar.
            pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
                validate_identifier($field, value.into()).map(Self)
            }

            /// Returns the validated scalar value.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_identifier!(
    RuntimeId,
    "runtime_id",
    "A validated runtime instance identity."
);
validated_identifier!(
    ProvisioningId,
    "provisioning_id",
    "A validated provisioning operation identity."
);
validated_identifier!(
    RetirementId,
    "retirement_id",
    "A validated retirement operation identity."
);

/// A validated reference to a credential held outside protocol messages.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CredentialHandle(String);

impl CredentialHandle {
    /// Constructs a handle using the strict `vault:<provider>:<key>` grammar.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = validate_identifier("credential_handle", value.into())?;
        let Some((provider, key)) = credential_parts(&value) else {
            return Err(IdentityError::InvalidCredentialHandle);
        };
        if provider.is_empty() || key.is_empty() {
            return Err(IdentityError::InvalidCredentialHandle);
        }
        Ok(Self(value))
    }

    /// Returns the complete validated handle.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the validated provider and key portions of the handle.
    pub fn parts(&self) -> (&str, &str) {
        credential_parts(&self.0).unwrap_or_default()
    }
}

impl<'de> Deserialize<'de> for CredentialHandle {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validates and returns an owned scalar using the shared identifier grammar.
pub fn validate_identifier(field: &'static str, value: String) -> Result<String, IdentityError> {
    let valid = !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(value)
        .ok_or(IdentityError::InvalidIdentifier { field })
}

fn credential_parts(value: &str) -> Option<(&str, &str)> {
    value
        .strip_prefix(CREDENTIAL_HANDLE_PREFIX)
        .and_then(|suffix| suffix.split_once(':'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_preserve_the_shared_grammar_and_wire_shape() {
        let runtime = RuntimeId::new("runtime:a-1.test").unwrap();
        let provisioning = ProvisioningId::new("provision:a-1.test").unwrap();
        let retirement = RetirementId::new("retire:a-1.test").unwrap();

        assert_eq!(runtime.as_str(), "runtime:a-1.test");
        assert_eq!(
            serde_json::to_string(&runtime).unwrap(),
            "\"runtime:a-1.test\""
        );
        assert_eq!(
            serde_json::from_str::<RuntimeId>("\"runtime:a-1.test\"").unwrap(),
            runtime
        );
        assert_eq!(
            serde_json::to_string(&provisioning).unwrap(),
            "\"provision:a-1.test\""
        );
        assert_eq!(
            serde_json::to_string(&retirement).unwrap(),
            "\"retire:a-1.test\""
        );
    }

    #[test]
    fn identifiers_reject_empty_oversized_non_ascii_and_unsafe_values() {
        for value in [
            String::new(),
            "a".repeat(MAX_IDENTIFIER_BYTES + 1),
            "runtime/path".into(),
            "runtime name".into(),
            "runtime-龙".into(),
        ] {
            assert!(RuntimeId::new(value).is_err());
        }
        assert!(RuntimeId::new("a".repeat(MAX_IDENTIFIER_BYTES)).is_ok());
        assert!(serde_json::from_str::<RuntimeId>("\"runtime/path\"").is_err());
    }

    #[test]
    fn credential_handles_are_validated_and_split_without_panics() {
        let handle = CredentialHandle::new("vault:ssh:host-example").unwrap();
        assert_eq!(handle.as_str(), "vault:ssh:host-example");
        assert_eq!(handle.parts(), ("ssh", "host-example"));
        assert_eq!(
            serde_json::to_string(&handle).unwrap(),
            "\"vault:ssh:host-example\""
        );

        for value in ["ssh:host-example", "vault::host-example", "vault:ssh:"] {
            assert_eq!(
                CredentialHandle::new(value),
                Err(IdentityError::InvalidCredentialHandle)
            );
        }
        assert_eq!(
            CredentialHandle::new("vault:ssh:host/example"),
            Err(IdentityError::InvalidIdentifier {
                field: "credential_handle"
            })
        );
    }

    #[test]
    fn identity_errors_keep_the_existing_operator_messages() {
        assert_eq!(
            RuntimeId::new("runtime/path").unwrap_err().to_string(),
            "invalid runtime_id"
        );
        assert_eq!(
            CredentialHandle::new("raw-secret").unwrap_err().to_string(),
            "invalid credential handle"
        );
    }
}
