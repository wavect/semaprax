//! The versioned, retained routing record (`semaprax.runtime-route-record.v1`).
//! It binds the approved profile set, the routing policy, the request feature
//! digest, the selected concrete deployment and the decision identity. It is
//! evidence for replay, never authority: resume still requires the current
//! approved set to hold the recorded deployment.

use serde_json::{json, Map, Value};

use super::error::RuntimeRoutingError;
use crate::model_routing::engine::json;
use crate::model_routing::engine::{DecisionSource, FallbackReason};

pub const ROUTE_RECORD_SCHEMA: &str = "semaprax.runtime-route-record.v1";
const RECORD_DIGEST_DOMAIN: &str = "semaprax.runtime-route-record.digest.v1";
const MAX_RECORD_BYTES: usize = 8192;

/// How the profile was chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteSource {
    /// An admissible operator pin (zero router calls).
    Pin,
    /// One admissible profile or a rules-only family (zero router calls).
    Trivial,
    /// The rules path (no decision provider attached or enabled).
    Rules,
    /// The attached decision provider's validated choice.
    Provider,
    Cache,
    /// An explained, permitted rules fallback after the provider failed.
    Fallback,
}

impl RouteSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pin => "pin",
            Self::Trivial => "trivial",
            Self::Rules => "rules",
            Self::Provider => "provider",
            Self::Cache => "cache",
            Self::Fallback => "fallback",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        [
            Self::Pin,
            Self::Trivial,
            Self::Rules,
            Self::Provider,
            Self::Cache,
            Self::Fallback,
        ]
        .into_iter()
        .find(|x| x.as_str() == s)
    }
    pub(crate) fn from_decision(source: DecisionSource) -> (Self, Option<FallbackReason>) {
        match source {
            DecisionSource::Rules => (Self::Rules, None),
            DecisionSource::Trivial => (Self::Trivial, None),
            DecisionSource::Cache => (Self::Cache, None),
            DecisionSource::Provider => (Self::Provider, None),
            DecisionSource::Fallback(r) => (Self::Fallback, Some(r)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RouteRecord {
    pub(crate) profile_set: String,
    pub(crate) route_policy: String,
    pub(crate) features: String,
    pub(crate) profile: String,
    pub(crate) deployment: String,
    pub(crate) definition: String,
    pub(crate) source: RouteSource,
    pub(crate) decision_provider: String,
    pub(crate) decision_checkpoint: String,
    pub(crate) decision: String,
    pub(crate) router_calls: u32,
    pub(crate) lineage: String,
    pub(crate) explanation: Option<String>,
}

const KEYS: [&str; 14] = [
    "schema",
    "profile_set",
    "route_policy",
    "features",
    "profile",
    "deployment",
    "definition",
    "source",
    "decision_provider",
    "decision_checkpoint",
    "decision",
    "router_calls",
    "lineage",
    "explanation",
];

impl RouteRecord {
    pub fn profile_set(&self) -> &str {
        &self.profile_set
    }
    pub fn route_policy(&self) -> &str {
        &self.route_policy
    }
    pub fn features(&self) -> &str {
        &self.features
    }
    pub fn profile(&self) -> &str {
        &self.profile
    }
    /// The selected concrete bound-deployment digest.
    pub fn deployment(&self) -> &str {
        &self.deployment
    }
    pub fn definition(&self) -> &str {
        &self.definition
    }
    pub fn source(&self) -> RouteSource {
        self.source
    }
    /// The decision identity: the core's choice digest, or the pin digest.
    pub fn decision(&self) -> &str {
        &self.decision
    }
    pub fn decision_provider(&self) -> &str {
        &self.decision_provider
    }
    pub fn router_calls(&self) -> u32 {
        self.router_calls
    }
    pub fn lineage(&self) -> &str {
        &self.lineage
    }
    /// Why a fallback was taken, when one was.
    pub fn explanation(&self) -> Option<&str> {
        self.explanation.as_deref()
    }

    pub fn to_json(&self) -> Value {
        json!({
            "schema": ROUTE_RECORD_SCHEMA,
            "profile_set": self.profile_set, "route_policy": self.route_policy,
            "features": self.features, "profile": self.profile,
            "deployment": self.deployment, "definition": self.definition,
            "source": self.source.as_str(), "decision_provider": self.decision_provider,
            "decision_checkpoint": self.decision_checkpoint, "decision": self.decision,
            "router_calls": self.router_calls, "lineage": self.lineage,
            "explanation": self.explanation,
        })
    }

    /// Canonical bytes (sorted keys, compact).
    pub fn canonical_json(&self) -> String {
        json::canonical(&self.to_json())
    }

    pub fn digest(&self) -> String {
        json::digest(RECORD_DIGEST_DOMAIN, &self.to_json())
    }

    /// Strict parse of canonical record bytes; any other shape is refused.
    pub fn parse(text: &str) -> Result<Self, RuntimeRoutingError> {
        let bad = |why: &str| RuntimeRoutingError::RecordMismatch(why.to_owned());
        if text.len() > MAX_RECORD_BYTES {
            return Err(bad("route record exceeds its byte bound"));
        }
        let value: Value = serde_json::from_str(text).map_err(|_| bad("route record JSON"))?;
        Self::from_value(&value).and_then(|record| {
            if record.canonical_json() == text {
                Ok(record)
            } else {
                Err(bad("route record is not canonical"))
            }
        })
    }

    pub(crate) fn from_value(value: &Value) -> Result<Self, RuntimeRoutingError> {
        let bad = |why: &str| RuntimeRoutingError::RecordMismatch(why.to_owned());
        let m: &Map<String, Value> = value.as_object().ok_or_else(|| bad("record object"))?;
        if m.len() != KEYS.len() || KEYS.iter().any(|k| !m.contains_key(*k)) {
            return Err(bad("route record members"));
        }
        if m["schema"] != ROUTE_RECORD_SCHEMA {
            return Err(bad("route record schema"));
        }
        let text = |k: &str| {
            m[k].as_str()
                .filter(|s| !s.is_empty() && s.len() <= 512)
                .map(str::to_owned)
                .ok_or_else(|| bad(k))
        };
        Ok(Self {
            profile_set: text("profile_set")?,
            route_policy: text("route_policy")?,
            features: text("features")?,
            profile: text("profile")?,
            deployment: text("deployment")?,
            definition: text("definition")?,
            source: RouteSource::parse(&text("source")?).ok_or_else(|| bad("source"))?,
            decision_provider: text("decision_provider")?,
            decision_checkpoint: text("decision_checkpoint")?,
            decision: text("decision")?,
            router_calls: m["router_calls"]
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| bad("router_calls"))?,
            lineage: text("lineage")?,
            explanation: match &m["explanation"] {
                Value::Null => None,
                Value::String(s) if s.len() <= 1024 => Some(s.clone()),
                _ => return Err(bad("explanation")),
            },
        })
    }
}
