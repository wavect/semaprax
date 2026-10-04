//! Usage evidence and explicit protocol outcomes. Missing usage is `unknown`,
//! never zero; mismatches are typed outcomes, never a silent downgrade.

use super::probe::{HttpReply, SseEvent};
use super::types::Protocol;
use serde_json::{json, Value};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageEvidence {
    ProviderReported {
        input: Option<u64>,
        output: Option<u64>,
        total: Option<u64>,
        cached: Option<u64>,
    },
    Unknown,
}

impl UsageEvidence {
    /// Absent figures render as the string "unknown", never 0.
    pub fn to_json(&self) -> Value {
        let f = |o: &Option<u64>| o.map_or(json!("unknown"), |n| json!(n));
        match self {
            Self::Unknown => {
                json!({"source": "unknown", "input_tokens": "unknown", "output_tokens": "unknown", "total_tokens": "unknown", "cached_input_tokens": "unknown"})
            }
            Self::ProviderReported {
                input,
                output,
                total,
                cached,
            } => json!({
                "source": "provider_reported", "input_tokens": f(input), "output_tokens": f(output),
                "total_tokens": f(total), "cached_input_tokens": f(cached),
            }),
        }
    }
}

/// Parse a `usage` object of either OpenAI shape (Responses `input_tokens`,
/// Chat `prompt_tokens`). Non-integer or missing figures stay unknown.
pub fn parse_usage(usage: Option<&Value>) -> UsageEvidence {
    let Some(u) = usage.filter(|u| u.is_object()) else {
        return UsageEvidence::Unknown;
    };
    let n = |keys: &[&str]| keys.iter().find_map(|k| u.get(*k).and_then(Value::as_u64));
    let input = n(&["input_tokens", "prompt_tokens"]);
    let output = n(&["output_tokens", "completion_tokens"]);
    let total = n(&["total_tokens"]);
    let cached = u
        .get("input_tokens_details")
        .or_else(|| u.get("prompt_tokens_details"))
        .and_then(|d| d.get("cached_tokens"))
        .and_then(Value::as_u64);
    if input.is_none() && output.is_none() && total.is_none() {
        return UsageEvidence::Unknown;
    }
    UsageEvidence::ProviderReported {
        input,
        output,
        total,
        cached,
    }
}

/// Explicit, typed result of one call against an expected protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// HPL020: the reply is shaped for another protocol.
    ProtocolMismatch {
        expected: Protocol,
        observed: Protocol,
    },
    /// HPL021: the endpoint does not serve the protocol at all.
    ProtocolUnsupported { protocol: Protocol, status: u16 },
    /// HPL022: the endpoint refused the tool schema.
    UnsupportedToolSchema { detail: String },
    /// HPL023: usage absent; recorded as unknown, not zero.
    MissingUsage,
    /// HPL024: the returned model label differs from the bound identity.
    IdentityChanged { bound: String, returned: String },
    /// HPL025: the reply names no model; identity stays uncertain.
    IdentityUnreported,
    /// HPL026: any other non-2xx or malformed reply.
    Failed { status: u16 },
}

impl Outcome {
    pub fn code(&self) -> &'static str {
        match self {
            Self::ProtocolMismatch { .. } => "SPX-HPL020",
            Self::ProtocolUnsupported { .. } => "SPX-HPL021",
            Self::UnsupportedToolSchema { .. } => "SPX-HPL022",
            Self::MissingUsage => "SPX-HPL023",
            Self::IdentityChanged { .. } => "SPX-HPL024",
            Self::IdentityUnreported => "SPX-HPL025",
            Self::Failed { .. } => "SPX-HPL026",
        }
    }
    /// Outcomes that stop the call from being accepted as-is.
    pub fn is_refusal(&self) -> bool {
        !matches!(self, Self::MissingUsage | Self::IdentityUnreported)
    }
    pub fn to_json(&self) -> Value {
        json!({"code": self.code(), "outcome": match self {
            Self::ProtocolMismatch { expected, observed } => format!("protocol_mismatch expected={} observed={}", expected.as_str(), observed.as_str()),
            Self::ProtocolUnsupported { protocol, status } => format!("protocol_unsupported {} status={status}", protocol.as_str()),
            Self::UnsupportedToolSchema { detail } => format!("unsupported_tool_schema {detail}"),
            Self::MissingUsage => "missing_usage".to_string(),
            Self::IdentityChanged { bound, returned } => format!("identity_changed bound={bound} returned={returned}"),
            Self::IdentityUnreported => "identity_unreported".to_string(),
            Self::Failed { status } => format!("failed status={status}"),
        }})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallAssessment {
    pub usage: UsageEvidence,
    pub returned_model: Option<String>,
    pub outcomes: Vec<Outcome>,
}

impl CallAssessment {
    pub fn accepted(&self) -> bool {
        !self.outcomes.iter().any(Outcome::is_refusal)
    }
}

/// Shape of a reply body, independent of the path that was called.
pub fn detect_shape(v: &Value) -> Option<Protocol> {
    if v.get("choices").is_some_and(Value::is_array) {
        Some(Protocol::ChatCompletions)
    } else if v.get("object").and_then(Value::as_str) == Some("response")
        || v.get("output").is_some_and(Value::is_array)
    {
        Some(Protocol::Responses)
    } else if v.get("type").and_then(Value::as_str) == Some("message") && v.get("content").is_some()
    {
        Some(Protocol::AnthropicMessages)
    } else {
        None
    }
}

fn error_text(v: Option<&Value>, raw: &str) -> String {
    let m = v
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("error"))
                .or_else(|| v.get("message"))
        })
        .and_then(Value::as_str);
    m.unwrap_or(raw).to_string()
}

/// Assess a buffered reply. `bound_returned_model` is the model label observed
/// at adoption; a different label is `IdentityChanged`.
pub fn assess_reply(
    expected: Protocol,
    reply: &HttpReply,
    uses_tools: bool,
    bound_returned_model: Option<&str>,
) -> CallAssessment {
    let body = reply.json();
    let mut out = CallAssessment {
        usage: UsageEvidence::Unknown,
        returned_model: None,
        outcomes: Vec::new(),
    };
    if !(200..300).contains(&reply.status) {
        let text = error_text(body.as_ref(), &reply.excerpt()).to_ascii_lowercase();
        let outcome = if uses_tools
            && reply.status == 400
            && (text.contains("tool") || text.contains("function"))
        {
            Outcome::UnsupportedToolSchema {
                detail: text.chars().take(120).collect(),
            }
        } else if matches!(reply.status, 404 | 405 | 501) && !text.contains("model") {
            Outcome::ProtocolUnsupported {
                protocol: expected,
                status: reply.status,
            }
        } else {
            Outcome::Failed {
                status: reply.status,
            }
        };
        out.outcomes.push(outcome);
        return out;
    }
    let Some(v) = body else {
        out.outcomes.push(Outcome::Failed {
            status: reply.status,
        });
        return out;
    };
    match detect_shape(&v) {
        Some(p) if p == expected => {}
        Some(observed) => out
            .outcomes
            .push(Outcome::ProtocolMismatch { expected, observed }),
        None => out.outcomes.push(Outcome::Failed {
            status: reply.status,
        }),
    }
    out.usage = parse_usage(v.get("usage"));
    out.returned_model = v.get("model").and_then(Value::as_str).map(str::to_string);
    finish(&mut out, bound_returned_model);
    out
}

/// Assess collected SSE events (final usage and model may be in any event).
pub fn assess_stream(
    expected: Protocol,
    events: &[SseEvent],
    bound_returned_model: Option<&str>,
) -> CallAssessment {
    let mut out = CallAssessment {
        usage: UsageEvidence::Unknown,
        returned_model: None,
        outcomes: Vec::new(),
    };
    let mut merged = json!({});
    for e in events {
        let Ok(v) = serde_json::from_str::<Value>(&e.data) else {
            continue;
        };
        let inner = v.get("response").unwrap_or(&v);
        let shape_ok = match expected {
            Protocol::Responses => {
                e.event
                    .as_deref()
                    .is_some_and(|n| n.starts_with("response."))
                    || v.get("type")
                        .and_then(Value::as_str)
                        .is_some_and(|t| t.starts_with("response."))
            }
            Protocol::ChatCompletions => v.get("choices").is_some(),
            Protocol::AnthropicMessages => v.get("type").is_some(),
        };
        if !shape_ok && out.outcomes.is_empty() {
            if v.get("choices").is_some() {
                out.outcomes.push(Outcome::ProtocolMismatch {
                    expected,
                    observed: Protocol::ChatCompletions,
                });
            } else if v
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t.starts_with("response."))
            {
                out.outcomes.push(Outcome::ProtocolMismatch {
                    expected,
                    observed: Protocol::Responses,
                });
            }
        }
        if let Some(m) = inner.get("model").and_then(Value::as_str) {
            out.returned_model = Some(m.to_string());
        }
        // Usage updates are cumulative snapshots (and Anthropic nests the first one
        // in `message`): merge them key by key so a repeated snapshot is not
        // double counted and an output-only update keeps the earlier input count.
        if let Some(u) =
            crate::receipt::event_usage(&v).or_else(|| crate::receipt::event_usage(inner))
        {
            crate::receipt::merge_native(&mut merged, u);
        }
    }
    out.usage = parse_usage(Some(&merged));
    if events.is_empty() {
        out.outcomes.push(Outcome::Failed { status: 0 });
    }
    finish(&mut out, bound_returned_model);
    out
}

fn finish(out: &mut CallAssessment, bound: Option<&str>) {
    if out.usage == UsageEvidence::Unknown {
        out.outcomes.push(Outcome::MissingUsage);
    }
    match (&out.returned_model, bound) {
        (None, _) => out.outcomes.push(Outcome::IdentityUnreported),
        (Some(r), Some(b)) if r != b => out.outcomes.push(Outcome::IdentityChanged {
            bound: b.to_string(),
            returned: r.clone(),
        }),
        _ => {}
    }
}
