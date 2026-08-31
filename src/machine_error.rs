use serde::Serialize;

pub const MACHINE_ERROR_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Input,
    Configuration,
    Environment,
    Io,
    Runtime,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct MachineError {
    pub code: &'static str,
    pub category: ErrorCategory,
    pub message: String,
    pub retryable: bool,
    pub exit_code: u8,
}

#[derive(Serialize)]
struct MachineErrorEnvelope<'a> {
    schema_version: u32,
    ok: bool,
    error: &'a MachineError,
}

impl MachineError {
    pub fn new(
        code: &'static str,
        category: ErrorCategory,
        message: impl Into<String>,
        retryable: bool,
        exit_code: u8,
    ) -> Self {
        Self {
            code,
            category,
            message: message.into(),
            retryable,
            exit_code,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(&MachineErrorEnvelope {
            schema_version: MACHINE_ERROR_SCHEMA_VERSION,
            ok: false,
            error: self,
        })
    }
}

impl std::fmt::Display for MachineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for MachineError {}

#[cfg(test)]
mod tests {
    use super::{ErrorCategory, MACHINE_ERROR_SCHEMA_VERSION, MachineError};

    #[test]
    fn machine_error_envelope_is_stable_and_non_lossy() {
        let error = MachineError::new(
            "runtime_config_load_failed",
            ErrorCategory::Configuration,
            "config is invalid",
            false,
            2,
        );
        let payload: serde_json::Value = serde_json::from_str(&error.to_json().unwrap()).unwrap();

        assert_eq!(payload["schema_version"], MACHINE_ERROR_SCHEMA_VERSION);
        assert_eq!(payload["ok"], false);
        assert_eq!(payload["error"]["code"], "runtime_config_load_failed");
        assert_eq!(payload["error"]["category"], "configuration");
        assert_eq!(payload["error"]["message"], "config is invalid");
        assert_eq!(payload["error"]["retryable"], false);
        assert_eq!(payload["error"]["exit_code"], 2);
    }
}
