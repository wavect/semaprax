//! Typed `decision.evaluate` v2 result and call metadata (MR-02, MR-03).
//!
//! Billing and identity facts come only from the result's `call` member,
//! never from human diagnostic text. Every string is a bounded identifier, so
//! raw task text or a secret cannot ride along in identity or usage fields.

use super::route::str_enum;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

str_enum!(
    /// What a result's scores mean. Raw option mass is never a probability of
    /// task success.
    ScoreKind { OptionDistribution = "option_distribution", CandidateRelative = "candidate_relative", None = "none" }
);
str_enum!(
    /// How far an answering model identity can be attested.
    IdentityKind { ImmutableCheckpoint = "immutable_checkpoint", MutableService = "mutable_service", LocalDeclared = "local_declared", Unknown = "unknown" }
);
str_enum!(AbstentionReason { None = "none", Native = "native", HostThreshold = "host_threshold", UnsupportedInput = "unsupported_input" });
str_enum!(UsageBasis { ProviderReported = "provider_reported", LocalMeasured = "local_measured", Unknown = "unknown" });
str_enum!(Billing { Api = "api", Local = "local", Unknown = "unknown" });

fn bad(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Identifier charset of call metadata: no whitespace, so no prose.
pub(crate) fn ident_ok(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:@/+-".contains(&b))
        && !crate::profile::config::looks_like_secret(s)
}

fn opt_ident(m: &Map<String, Value>, k: &str, max: usize) -> HarnessResult<Option<String>> {
    match m.get(k) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(s)) if ident_ok(s, max) => Ok(Some(s.clone())),
        _ => Err(bad(
            "SPX-HPA047",
            format!("call `{k}` must be null or an identifier of at most {max} bytes"),
        )),
    }
}

fn opt_u64(m: &Map<String, Value>, k: &str) -> HarnessResult<Option<u64>> {
    match m.get(k) {
        Some(Value::Null) => Ok(None),
        Some(v) => v.as_u64().map(Some).ok_or_else(|| {
            bad(
                "SPX-HPA047",
                format!("usage `{k}` must be an integer or null"),
            )
        }),
        None => Err(bad("SPX-HPA047", format!("usage is missing `{k}`"))),
    }
}

fn closed<'a>(v: &'a Value, what: &str, keys: &[&str]) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| bad("SPX-HPA047", format!("{what} must be an object")))?;
    for k in m.keys() {
        if !keys.contains(&k.as_str()) {
            return Err(bad(
                "SPX-HPA047",
                format!("{what} has unexpected member `{k}`"),
            ));
        }
    }
    for k in keys {
        if !m.contains_key(*k) {
            return Err(bad("SPX-HPA047", format!("{what} is missing `{k}`")));
        }
    }
    Ok(m)
}

fn closed_enum<T>(
    m: &Map<String, Value>,
    k: &str,
    parse: fn(&str) -> Option<T>,
) -> HarnessResult<T> {
    m.get(k)
        .and_then(Value::as_str)
        .and_then(parse)
        .ok_or_else(|| {
            bad(
                "SPX-HPA047",
                format!("call `{k}` is not a member of its closed set"),
            )
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub basis: UsageBasis,
}

impl Usage {
    /// Provider-reported input tokens: the only usage that may settle spend.
    pub fn authoritative_input(&self) -> Option<u64> {
        (self.basis == UsageBasis::ProviderReported)
            .then_some(self.input_tokens)
            .flatten()
    }
}

/// What one router call reported about itself (MR-03).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallMetadata {
    pub adapter: String,
    pub requested_model: Option<String>,
    pub answering_model: Option<String>,
    pub checkpoint: Option<String>,
    pub identity_kind: IdentityKind,
    pub rendered_digest: String,
    pub wire_bytes: u64,
    pub usage: Usage,
    pub billing: Billing,
}

impl CallMetadata {
    pub const MEMBERS: [&'static str; 9] = [
        "adapter",
        "requested_model",
        "answering_model",
        "checkpoint",
        "identity_kind",
        "rendered_digest",
        "wire_bytes",
        "usage",
        "billing",
    ];

    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let m = closed(v, "call", &Self::MEMBERS)?;
        let adapter = match m["adapter"].as_str() {
            Some(s) if ident_ok(s, 128) => s.to_string(),
            _ => return Err(bad("SPX-HPA047", "call `adapter` must be an identifier")),
        };
        let rendered_digest = m["rendered_digest"]
            .as_str()
            .filter(|s| crate::contract::payload::is_digest(s))
            .ok_or_else(|| {
                bad(
                    "SPX-HPA047",
                    "call `rendered_digest` must be `sha256:<64 hex>`",
                )
            })?
            .to_string();
        let wire_bytes = m["wire_bytes"]
            .as_u64()
            .ok_or_else(|| bad("SPX-HPA047", "call `wire_bytes` must be an integer"))?;
        let u = closed(
            &m["usage"],
            "usage",
            &["input_tokens", "output_tokens", "basis"],
        )?;
        let usage = Usage {
            input_tokens: opt_u64(u, "input_tokens")?,
            output_tokens: opt_u64(u, "output_tokens")?,
            basis: closed_enum(u, "basis", UsageBasis::parse)?,
        };
        Ok(Self {
            adapter,
            requested_model: opt_ident(m, "requested_model", 128)?,
            answering_model: opt_ident(m, "answering_model", 128)?,
            checkpoint: opt_ident(m, "checkpoint", 128)?,
            identity_kind: closed_enum(m, "identity_kind", IdentityKind::parse)?,
            rendered_digest,
            wire_bytes,
            usage,
            billing: closed_enum(m, "billing", Billing::parse)?,
        })
    }

    pub fn to_json(&self) -> Value {
        json!({
            "adapter": self.adapter, "requested_model": self.requested_model,
            "answering_model": self.answering_model, "checkpoint": self.checkpoint,
            "identity_kind": self.identity_kind.as_str(), "rendered_digest": self.rendered_digest,
            "wire_bytes": self.wire_bytes,
            "usage": {"input_tokens": self.usage.input_tokens, "output_tokens": self.usage.output_tokens,
                      "basis": self.usage.basis.as_str()},
            "billing": self.billing.as_str(),
        })
    }
}

/// A validated `model-route/v2` result. Selection ids are still the wire ids
/// (`m0..`); the router maps them back to opaque model ids.
#[derive(Clone, Debug, PartialEq)]
pub struct ResultV2 {
    pub choice: Option<String>,
    pub abstention_reason: AbstentionReason,
    pub scores: Option<BTreeMap<String, f64>>,
    pub score_kind: ScoreKind,
    pub native_confidence: Option<f64>,
    pub native_confidence_kind: Option<String>,
    pub calibration_id: Option<String>,
    pub call: CallMetadata,
}

pub const RESULT_V2_MEMBERS: [&str; 9] = [
    "choice",
    "abstain",
    "abstention_reason",
    "scores",
    "score_kind",
    "native_confidence",
    "native_confidence_kind",
    "calibration_id",
    "call",
];

fn num01(x: &Value) -> Option<f64> {
    x.as_f64()
        .filter(|f| f.is_finite() && (0.0..=1.0).contains(f))
}

impl ResultV2 {
    /// Structural validation of a v2 result on its own (`SPX-HPA040/044/047`).
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let m = v
            .as_object()
            .ok_or_else(|| bad("SPX-HPA040", "decision result must be an object"))?;
        for k in m.keys() {
            if !RESULT_V2_MEMBERS.contains(&k.as_str()) {
                return Err(bad(
                    "SPX-HPA040",
                    format!("decision result has unexpected member `{k}`"),
                ));
            }
        }
        for k in RESULT_V2_MEMBERS {
            if !m.contains_key(k) {
                return Err(bad(
                    "SPX-HPA040",
                    format!("decision result is missing `{k}`"),
                ));
            }
        }
        let abstain = m["abstain"]
            .as_bool()
            .ok_or_else(|| bad("SPX-HPA040", "`abstain` must be a boolean"))?;
        let choice = match &m["choice"] {
            Value::Null if abstain => None,
            Value::String(s) if !abstain && !s.is_empty() && s.len() <= 128 => Some(s.clone()),
            _ => {
                return Err(bad(
                    "SPX-HPA040",
                    "`choice` must be null exactly when `abstain` is true",
                ))
            }
        };
        let reason = m["abstention_reason"]
            .as_str()
            .and_then(AbstentionReason::parse)
            .ok_or_else(|| {
                bad(
                    "SPX-HPA040",
                    "`abstention_reason` is not a member of its closed set",
                )
            })?;
        if abstain == (reason == AbstentionReason::None) {
            return Err(bad(
                "SPX-HPA040",
                "`abstention_reason` must be `none` exactly when `abstain` is false",
            ));
        }
        let score_kind = m["score_kind"]
            .as_str()
            .and_then(ScoreKind::parse)
            .ok_or_else(|| {
                bad(
                    "SPX-HPA040",
                    "`score_kind` is not a member of its closed set",
                )
            })?;
        let scores = match (&m["scores"], score_kind) {
            (Value::Null, ScoreKind::None) => None,
            (Value::Object(o), k) if k != ScoreKind::None => {
                let mut out = BTreeMap::new();
                for (id, x) in o {
                    let f = num01(x).ok_or_else(|| {
                        bad(
                            "SPX-HPA044",
                            format!("score for `{id}` must be finite in [0,1]"),
                        )
                    })?;
                    out.insert(id.clone(), f);
                }
                Some(out)
            }
            _ => {
                return Err(bad(
                    "SPX-HPA044",
                    "`scores` must be null exactly when `score_kind` is `none`",
                ))
            }
        };
        let native_confidence = match &m["native_confidence"] {
            Value::Null => None,
            x => Some(num01(x).ok_or_else(|| {
                bad(
                    "SPX-HPA044",
                    "`native_confidence` must be null or finite in [0,1]",
                )
            })?),
        };
        let native_confidence_kind = match &m["native_confidence_kind"] {
            Value::Null => None,
            Value::String(s) if ident_ok(s, 64) => Some(s.clone()),
            _ => {
                return Err(bad(
                    "SPX-HPA044",
                    "`native_confidence_kind` must be null or an identifier",
                ))
            }
        };
        if native_confidence.is_some() != native_confidence_kind.is_some() {
            return Err(bad(
                "SPX-HPA044",
                "`native_confidence_kind` must be null exactly when `native_confidence` is null",
            ));
        }
        let calibration_id = match &m["calibration_id"] {
            Value::Null => None,
            Value::String(s) if ident_ok(s, 128) => Some(s.clone()),
            _ => {
                return Err(bad(
                    "SPX-HPA044",
                    "`calibration_id` must be null or an identifier",
                ))
            }
        };
        Ok(Self {
            choice,
            abstention_reason: reason,
            scores,
            score_kind,
            native_confidence,
            native_confidence_kind,
            calibration_id,
            call: CallMetadata::from_json(&m["call"])?,
        })
    }

    /// Cross-check against the request's options, rendered digest and wire
    /// bound: exact score coverage, argmax choice, normalized distributions.
    pub fn check_against(
        &self,
        options: &[&str],
        rendered_digest: &str,
        max_wire_bytes: u64,
    ) -> HarnessResult<()> {
        if let Some(c) = &self.choice {
            if !options.contains(&c.as_str()) {
                return Err(bad(
                    "SPX-HPA043",
                    format!("choice `{c}` is not one of the request options"),
                ));
            }
        }
        if let Some(s) = &self.scores {
            let keys: Vec<&str> = s.keys().map(String::as_str).collect();
            let mut want: Vec<&str> = options.to_vec();
            want.sort_unstable();
            if keys != want {
                return Err(bad(
                    "SPX-HPA043",
                    "scores must cover exactly the request options",
                ));
            }
            let max = s.values().cloned().fold(f64::MIN, f64::max);
            if let Some(c) = &self.choice {
                if s[c] + 1e-3 < max {
                    return Err(bad("SPX-HPA043", "choice is not an argmax of its scores"));
                }
            }
            if self.score_kind == ScoreKind::OptionDistribution
                && (s.values().sum::<f64>() - 1.0).abs() > 0.05
            {
                return Err(bad(
                    "SPX-HPA044",
                    "an option_distribution must sum to 1 within 0.05",
                ));
            }
        }
        if self.call.rendered_digest != rendered_digest {
            return Err(bad(
                "SPX-HPA047",
                "call `rendered_digest` is not the digest of the prepared request",
            ));
        }
        if self.call.wire_bytes > max_wire_bytes {
            return Err(bad(
                "SPX-HPA047",
                format!(
                    "call `wire_bytes` {} exceeds the request bound {max_wire_bytes}",
                    self.call.wire_bytes
                ),
            ));
        }
        Ok(())
    }
}
