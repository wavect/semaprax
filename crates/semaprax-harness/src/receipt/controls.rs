//! Generation controls carried from fitting into the `model.generate` request,
//! and what the provider reported it applied. The output-token cap is a
//! token control; `max_output_bytes` stays a separate transport safety bound.

use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Effort {
    Minimal,
    Low,
    Medium,
    High,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "minimal" => Self::Minimal,
            "low" => Self::Low,
            "medium" => Self::Medium,
            "high" => Self::High,
            _ => return None,
        })
    }
}

/// What one request asks of the provider.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GenerationControls {
    /// Reserved output tokens (the accepted reservation), when the host sets one.
    pub max_output_tokens: Option<u64>,
    pub reasoning: Option<Effort>,
    /// Refuse before dispatch when the provider cannot honor the cap.
    pub strict: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    Supported,
    Unsupported,
    /// Not declared (legacy provider): enforceable only when assumed.
    Unknown,
}

/// Host-declared support of the selected provider for each control.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GenerationSupport {
    pub output_cap: Support,
    pub reasoning: Support,
}

impl Default for GenerationSupport {
    fn default() -> Self {
        Self {
            output_cap: Support::Unknown,
            reasoning: Support::Unknown,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlStatus {
    NotRequested,
    Applied,
    Unsupported,
    /// Requested but the provider reported nothing either way.
    Unreported,
}

impl ControlStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Applied => "applied",
            Self::Unsupported => "unsupported",
            Self::Unreported => "unreported",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlReport {
    pub status: ControlStatus,
    /// Effective cap the provider enforced, when reported.
    pub effective: Option<u64>,
}

impl ControlReport {
    pub const NOT_REQUESTED: Self = Self {
        status: ControlStatus::NotRequested,
        effective: None,
    };
    pub fn to_json(self) -> Value {
        json!({"status": self.status.as_str(), "effective": self.effective})
    }
    /// Read `{status, effective}`; a request that was made but not answered is `unreported`.
    pub fn from_reply(requested: bool, v: Option<&Value>) -> Self {
        if !requested {
            return Self::NOT_REQUESTED;
        }
        let status = match v.and_then(|v| v.get("status")).and_then(Value::as_str) {
            Some("applied") => ControlStatus::Applied,
            Some("unsupported") => ControlStatus::Unsupported,
            _ => ControlStatus::Unreported,
        };
        Self {
            status,
            effective: v.and_then(|v| v.get("effective")).and_then(Value::as_u64),
        }
    }
}
