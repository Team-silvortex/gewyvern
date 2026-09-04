#![forbid(unsafe_code)]

//! Product-independent values shared by Leselang and its host adapters.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};
pub use silvortex_identity::RuntimeId;

pub const CAPABILITY_RUNTIME_READ: &str = "runtime.read";
pub const CAPABILITY_RUNTIME_REFRESH: &str = "runtime.refresh";
pub const CAPABILITY_RUNTIME_DEPLOY: &str = "runtime.deploy";
pub const CAPABILITY_DEBUGGER_CONTROL: &str = "debugger.control";
pub const CAPABILITY_UI_PRESENTATION: &str = "ui.presentation";

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Principal {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
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

impl CapabilitySet {
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(values.into_iter().map(Into::into).collect())
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
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
