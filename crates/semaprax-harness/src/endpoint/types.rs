//! Endpoint catalog vocabulary: kinds, protocols, verdicts, identities.

use crate::decision::Destination;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};

pub fn err(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndpointKind {
    Ollama,
    LiteLlm,
    OpenAiCompatible,
}

impl EndpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::LiteLlm => "litellm",
            Self::OpenAiCompatible => "openai-compatible",
        }
    }
    pub fn parse(s: &str) -> HarnessResult<Self> {
        match s {
            "ollama" => Ok(Self::Ollama),
            "litellm" => Ok(Self::LiteLlm),
            "openai-compatible" => Ok(Self::OpenAiCompatible),
            o => Err(err("SPX-HPL001", format!("unknown endpoint kind `{o}`"))),
        }
    }
    /// A gateway may retry, fall back or balance internally; a direct model
    /// server (Ollama) has no such layer.
    pub fn is_gateway(self) -> bool {
        !matches!(self, Self::Ollama)
    }
}

/// Wire protocol a logical model is bound to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Protocol {
    Responses,
    ChatCompletions,
    AnthropicMessages,
}

impl Protocol {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
            Self::AnthropicMessages => "anthropic_messages",
        }
    }
    pub fn parse(s: &str) -> HarnessResult<Self> {
        match s {
            "responses" => Ok(Self::Responses),
            "chat" | "chat_completions" => Ok(Self::ChatCompletions),
            "anthropic" | "anthropic_messages" => Ok(Self::AnthropicMessages),
            o => Err(err("SPX-HPL001", format!("unknown protocol `{o}`"))),
        }
    }
    /// The catalog probe key proving this protocol.
    pub fn probe_key(self) -> &'static str {
        self.as_str()
    }
    pub fn path(self) -> &'static str {
        match self {
            Self::Responses => "/v1/responses",
            Self::ChatCompletions => "/v1/chat/completions",
            Self::AnthropicMessages => "/v1/messages",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Supported,
    Unsupported,
    Unverified,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Unverified => "unverified",
        }
    }
}

/// Verdict plus the observation that justified it (no secrets, no timings).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolVerdict {
    pub verdict: Verdict,
    pub evidence: String,
}

impl ProtocolVerdict {
    pub fn new(verdict: Verdict, evidence: impl Into<String>) -> Self {
        Self {
            verdict,
            evidence: evidence.into(),
        }
    }
    pub fn to_json(&self) -> Value {
        json!({"verdict": self.verdict.as_str(), "evidence": self.evidence})
    }
    pub fn from_json(v: &Value) -> Option<Self> {
        let verdict = match v.get("verdict")?.as_str()? {
            "supported" => Verdict::Supported,
            "unsupported" => Verdict::Unsupported,
            "unverified" => Verdict::Unverified,
            _ => return None,
        };
        Some(Self {
            verdict,
            evidence: v.get("evidence")?.as_str()?.to_string(),
        })
    }
}

/// What is known about the model behind a name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelIdentity {
    /// Content digest from the server's catalog (e.g. Ollama `/api/tags`).
    Digest(String),
    /// Server-reported label with no content binding.
    Reported(String),
    Unknown,
}

impl ModelIdentity {
    pub fn to_json(&self) -> Value {
        match self {
            Self::Digest(d) => json!({"kind": "digest", "value": d}),
            Self::Reported(r) => json!({"kind": "reported", "value": r}),
            Self::Unknown => json!({"kind": "unknown"}),
        }
    }
    pub fn from_json(v: &Value) -> Option<Self> {
        let val = || v.get("value")?.as_str().map(str::to_string);
        match v.get("kind")?.as_str()? {
            "digest" => Some(Self::Digest(val()?)),
            "reported" => Some(Self::Reported(val()?)),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

pub fn destination_to_json(d: &Destination) -> Value {
    match d {
        Destination::Local => json!({"kind": "local"}),
        Destination::Remote { origin } => json!({"kind": "remote", "origin": origin}),
    }
}

pub fn destination_from_json(v: &Value) -> Option<Destination> {
    match v.get("kind")?.as_str()? {
        "local" => Some(Destination::Local),
        "remote" => Some(Destination::Remote {
            origin: v.get("origin")?.as_str()?.to_string(),
        }),
        _ => None,
    }
}

/// Destination of an endpoint whose upstream is not disclosed.
pub fn remote_unknown() -> Destination {
    Destination::Remote {
        origin: "unknown".to_string(),
    }
}

/// Credential variables are referenced by name only.
pub fn validate_credential_name(name: &str) -> HarnessResult<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
        && !name.starts_with(|c: char| c.is_ascii_digit());
    if ok {
        Ok(())
    } else {
        Err(err(
            "SPX-HPL007",
            "credential must be an environment variable NAME ([A-Z_][A-Z0-9_]*), never a value",
        ))
    }
}
