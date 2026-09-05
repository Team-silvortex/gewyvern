#![forbid(unsafe_code)]

//! Product-independent values shared by Leselang and its host adapters.

use std::collections::BTreeSet;
use std::fmt;

use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};
pub use silvortex_identity::RuntimeId;

pub const CAPABILITY_RUNTIME_READ: &str = "runtime.read";
pub const CAPABILITY_RUNTIME_REFRESH: &str = "runtime.refresh";
pub const CAPABILITY_RUNTIME_DEPLOY: &str = "runtime.deploy";
pub const CAPABILITY_DEBUGGER_CONTROL: &str = "debugger.control";
pub const CAPABILITY_UI_PRESENTATION: &str = "ui.presentation";
pub const MAX_CAPABILITY_COUNT: usize = 64;
pub const MAX_CAPABILITY_BYTES: usize = 128;
pub const MAX_RUNTIME_FILTER_BYTES: usize = 128;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Principal {
    pub id: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CapabilitySet(BTreeSet<String>);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOrigin {
    Gui,
    Cli,
    Leselang,
    Model,
    CompatibilityAdapter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    NotRequired,
    Confirmed,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct RuntimeListFilter {
    pub environment: Option<String>,
    pub cluster: Option<String>,
    pub role: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostContractError {
    InvalidIdentifier { field: &'static str },
}

impl fmt::Display for HostContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
        }
    }
}

impl std::error::Error for HostContractError {}

impl Principal {
    pub fn new(id: impl Into<String>) -> Result<Self, HostContractError> {
        silvortex_identity::validate_identifier("principal.id", id.into())
            .map(|id| Self { id })
            .map_err(|_| HostContractError::InvalidIdentifier {
                field: "principal.id",
            })
    }

    pub fn is_valid(&self) -> bool {
        Self::new(self.id.clone()).is_ok()
    }
}

impl<'de> Deserialize<'de> for Principal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct PrincipalWire {
            id: String,
        }

        let wire = PrincipalWire::deserialize(deserializer)?;
        Self::new(wire.id).map_err(de::Error::custom)
    }
}

impl CapabilitySet {
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(values.into_iter().map(Into::into).collect())
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }
}

impl<'de> Deserialize<'de> for CapabilitySet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CapabilitySetVisitor;

        impl<'de> Visitor<'de> for CapabilitySetVisitor {
            type Value = CapabilitySet;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(
                    formatter,
                    "an array of at most {MAX_CAPABILITY_COUNT} bounded capability identifiers"
                )
            }

            fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut values = BTreeSet::new();
                let mut count = 0usize;
                while let Some(value) = sequence.next_element::<String>()? {
                    count = count.saturating_add(1);
                    if count > MAX_CAPABILITY_COUNT {
                        return Err(de::Error::custom(format_args!(
                            "capability set exceeds {MAX_CAPABILITY_COUNT} entries"
                        )));
                    }
                    if !valid_capability(&value) {
                        return Err(de::Error::custom("invalid capability identifier"));
                    }
                    values.insert(value);
                }
                Ok(CapabilitySet(values))
            }
        }

        deserializer.deserialize_seq(CapabilitySetVisitor)
    }
}

impl<'de> Deserialize<'de> for RuntimeListFilter {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RuntimeListFilterWire {
            environment: Option<String>,
            cluster: Option<String>,
            role: Option<String>,
        }

        let wire = RuntimeListFilterWire::deserialize(deserializer)?;
        if [
            wire.environment.as_deref(),
            wire.cluster.as_deref(),
            wire.role.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.len() > MAX_RUNTIME_FILTER_BYTES || value.chars().any(char::is_control))
        {
            return Err(de::Error::custom("invalid runtime list filter"));
        }
        Ok(Self {
            environment: wire.environment,
            cluster: wire.cluster,
            role: wire.role,
        })
    }
}

impl RuntimeListFilter {
    pub fn normalized(self) -> Self {
        Self {
            environment: normalize_filter_value(self.environment),
            cluster: normalize_filter_value(self.cluster),
            role: normalize_filter_value(self.role),
        }
    }
}

/// Validates the debugger session identity used across language-host boundaries.
pub fn validate_debugger_session_id(session_id: &str) -> Result<(), HostContractError> {
    silvortex_identity::validate_identifier("session_id", session_id.to_string())
        .map(|_| ())
        .map_err(|_| HostContractError::InvalidIdentifier {
            field: "session_id",
        })
}

/// Validates deployment fields before a product adapter turns them into a command.
pub fn validate_deployment_intent(
    pipeline_kind: &str,
    target: Option<&str>,
) -> Result<(), HostContractError> {
    let pipeline_valid = !pipeline_kind.is_empty()
        && pipeline_kind.len() <= 128
        && pipeline_kind == pipeline_kind.trim()
        && pipeline_kind
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'_' | b'-'));
    let target_valid = target.is_none_or(|target| {
        !target.is_empty()
            && target.len() <= 256
            && target == target.trim()
            && !target.chars().any(char::is_control)
    });
    if !pipeline_valid {
        return Err(HostContractError::InvalidIdentifier {
            field: "pipeline_kind",
        });
    }
    if !target_valid {
        return Err(HostContractError::InvalidIdentifier { field: "target" });
    }
    Ok(())
}

fn normalize_filter_value(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn valid_capability(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_CAPABILITY_BYTES
        && value == value.trim()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b':' | b'_' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_values_preserve_the_existing_wire_shape() {
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_READ]);
        assert_eq!(
            serde_json::to_string(&capabilities).unwrap(),
            r#"["runtime.read"]"#
        );
        assert_eq!(serde_json::to_string(&Revision(7)).unwrap(), "7");
        assert_eq!(
            serde_json::to_string(&CommandOrigin::Leselang).unwrap(),
            r#""leselang""#
        );
    }

    #[test]
    fn object_values_reject_unknown_fields() {
        assert!(serde_json::from_str::<Principal>(r#"{"id":"operator","admin":true}"#).is_err());
        assert!(
            serde_json::from_str::<RuntimeListFilter>(
                r#"{"environment":null,"cluster":null,"role":null,"all":true}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn decoded_principals_capabilities_and_filters_are_bounded() {
        assert!(serde_json::from_str::<Principal>(r#"{"id":"operator-a"}"#).is_ok());
        for id in [
            String::new(),
            "x".repeat(silvortex_identity::MAX_IDENTIFIER_BYTES + 1),
            "operator/path".into(),
        ] {
            let source = serde_json::json!({ "id": id });
            assert!(serde_json::from_value::<Principal>(source).is_err());
        }

        assert!(
            serde_json::from_value::<CapabilitySet>(serde_json::json!([
                CAPABILITY_RUNTIME_READ,
                CAPABILITY_UI_PRESENTATION
            ]))
            .is_ok()
        );
        assert!(
            serde_json::from_value::<CapabilitySet>(serde_json::json!([
                "x".repeat(MAX_CAPABILITY_BYTES + 1)
            ]))
            .is_err()
        );
        assert!(
            serde_json::from_value::<CapabilitySet>(serde_json::Value::Array(
                (0..=MAX_CAPABILITY_COUNT)
                    .map(|index| serde_json::Value::String(format!("capability.{index}")))
                    .collect(),
            ))
            .is_err()
        );

        let oversized_filter = serde_json::json!({
            "environment": "x".repeat(MAX_RUNTIME_FILTER_BYTES + 1),
            "cluster": null,
            "role": null,
        });
        assert!(serde_json::from_value::<RuntimeListFilter>(oversized_filter).is_err());
    }

    #[test]
    fn filters_normalize_without_product_policy() {
        assert_eq!(
            RuntimeListFilter {
                environment: Some(" production ".into()),
                cluster: Some(" ".into()),
                role: None,
            }
            .normalized(),
            RuntimeListFilter {
                environment: Some("production".into()),
                cluster: None,
                role: None,
            }
        );
    }

    #[test]
    fn shared_effect_inputs_are_bounded() {
        assert!(validate_debugger_session_id("session-a").is_ok());
        assert!(validate_debugger_session_id("session/a").is_err());
        assert!(validate_deployment_intent("capture/http", Some("eth0")).is_ok());
        assert_eq!(
            validate_deployment_intent("capture http", None),
            Err(HostContractError::InvalidIdentifier {
                field: "pipeline_kind"
            })
        );
    }
}
