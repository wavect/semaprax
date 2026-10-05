//! Stable diagnostics: `SPX-HP<letter><3 digits>` codes plus a message. The
//! harness re-exports this type as `HarnessDiagnostic`, so a rejection carries
//! the same code and text through the harness and the runtime boundary.

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub code: &'static str,
    pub message: String,
}

impl Diagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    /// One JSON object `{"code":..,"message":..}`.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({"code": self.code, "message": self.message})
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for Diagnostic {}

pub type DecisionResult<T> = Result<T, Diagnostic>;
