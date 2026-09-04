use serde_json::json;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ConsoleWriteStatus {
    BadRequest,
    NotFound,
    Conflict,
    ServiceUnavailable,
    InternalServerError,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConsoleWriteError {
    pub(crate) status: ConsoleWriteStatus,
    pub(crate) code: &'static str,
    pub(crate) reason: &'static str,
}

impl ConsoleWriteError {
    pub(crate) const fn new(
        status: ConsoleWriteStatus,
        code: &'static str,
        reason: &'static str,
    ) -> Self {
        Self {
            status,
            code,
            reason,
        }
    }

    pub(crate) const fn projection_failed() -> Self {
        Self::new(
            ConsoleWriteStatus::InternalServerError,
            "web_projection_failed",
            "Rust Web compatibility projection failed",
        )
    }

    pub(crate) fn body(&self) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "error": self.code,
            "reason": self.reason,
        }))
        .unwrap_or_else(|_| {
            br#"{"error":"web_projection_failed","reason":"Rust Web error response serialization failed"}"#
                .to_vec()
        })
    }
}

impl From<String> for ConsoleWriteError {
    fn from(_: String) -> Self {
        Self::projection_failed()
    }
}

impl From<&str> for ConsoleWriteError {
    fn from(_: &str) -> Self {
        Self::projection_failed()
    }
}
