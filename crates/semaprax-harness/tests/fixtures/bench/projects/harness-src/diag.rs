//! Harness diagnostics: stable `SPX-HP<letter><3 digits>` codes (see the
//! specification's Diagnostics section for the per-work-item letter).

use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessDiagnostic {
    pub code: &'static str,
    pub message: String,
}

impl HarnessDiagnostic {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    /// One JSON object `{"code":..,"message":..}`.
    pub fn json(&self) -> serde_json::Value {
        serde_json::json!({"code": self.code, "message": self.message})
    }
}

impl fmt::Display for HarnessDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code, self.message)
    }
}

impl std::error::Error for HarnessDiagnostic {}

pub type HarnessResult<T> = Result<T, HarnessDiagnostic>;
