//! Versioned additive observation event. Metadata only: identities, digests
//! and counts. No field can carry payload text.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};

pub const OBSERVATION_SCHEMA: &str = "semaprax.harness-observation.v1";
/// Longest accepted identity/digest string.
pub const MAX_FIELD_BYTES: usize = 256;

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPO001", msg)
}

macro_rules! str_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $name { $($var),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $(Self::$var => $s),+ } }
            pub fn parse(s: &str) -> HarnessResult<Self> {
                match s { $($s => Ok(Self::$var),)+ _ => Err(bad(format!("unknown {} `{}`", stringify!($name), s))) }
            }
        }
    };
}

str_enum!(Stage {
    ContextSelect = "context_select", Compression = "compression", Dedup = "dedup",
    Decision = "decision", Generation = "generation", CommandView = "command_view",
    SkillCatalog = "skill_catalog", RetrievalWrapper = "retrieval_wrapper",
    IndexBuild = "index_build", ModelLoad = "model_load",
});
str_enum!(
    /// What the event contributes: a step of a payload's transformation
    /// lineage, an extra request that actually occurred, or local-only work.
    Role { Transform = "transform", Incurred = "incurred", Local = "local" });
str_enum!(CacheState { Miss = "miss", Hit = "hit", Bypass = "bypass" });
str_enum!(Availability { Available = "available", Fallback = "fallback", Unavailable = "unavailable" });
str_enum!(Outcome { Ok = "ok", Failed = "failed" });
str_enum!(Warmth { Cold = "cold", Warm = "warm", NotApplicable = "n/a" });

/// Measurement identity. Byte counts are never model tokens.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TokenizerId {
    Named { name: String, fingerprint: String },
    ByteOnly,
}

impl TokenizerId {
    pub fn key(&self) -> String {
        match self {
            Self::Named { name, fingerprint } => format!("named:{name}:{fingerprint}"),
            Self::ByteOnly => "byte_only".into(),
        }
    }
    pub fn to_json(&self) -> Value {
        match self {
            Self::Named { name, fingerprint } => {
                json!({"kind":"named","name":name,"fingerprint":fingerprint})
            }
            Self::ByteOnly => json!({"kind":"byte_only"}),
        }
    }
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        match v.get("kind").and_then(Value::as_str) {
            Some("byte_only") => Ok(Self::ByteOnly),
            Some("named") => Ok(Self::Named {
                name: field_str(v, "name")?,
                fingerprint: field_str(v, "fingerprint")?,
            }),
            _ => Err(bad("tokenizer kind must be `named` or `byte_only`")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenCount {
    pub tokenizer: TokenizerId,
    pub value: u64,
}

impl TokenCount {
    pub fn named(name: &str, fingerprint: &str, value: u64) -> Self {
        Self {
            tokenizer: TokenizerId::Named {
                name: name.into(),
                fingerprint: fingerprint.into(),
            },
            value,
        }
    }
    pub fn bytes(value: u64) -> Self {
        Self {
            tokenizer: TokenizerId::ByteOnly,
            value,
        }
    }
    /// Signed difference; refuses to mix byte estimates with named counts or
    /// two different tokenizers (`SPX-HPO004`).
    pub fn checked_sub(&self, other: &TokenCount) -> HarnessResult<i64> {
        if self.tokenizer != other.tokenizer {
            return Err(HarnessDiagnostic::new(
                "SPX-HPO004",
                "refusing to combine counts from different tokenizer kinds",
            ));
        }
        Ok(self.value as i64 - other.value as i64)
    }
    fn to_json(&self) -> Value {
        json!({"tokenizer": self.tokenizer.to_json(), "value": self.value})
    }
    fn from_json(v: &Value) -> HarnessResult<Self> {
        let value = v
            .get("value")
            .and_then(Value::as_u64)
            .ok_or_else(|| bad("count value must be an unsigned integer"))?;
        Ok(Self {
            tokenizer: TokenizerId::from_json(
                v.get("tokenizer")
                    .ok_or_else(|| bad("count needs tokenizer"))?,
            )?,
            value,
        })
    }
}

/// Resource and cost provenance. `None` is explicit "unknown".
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Cost {
    /// Provider-billed micro-units, or unknown.
    pub provider_billed: Option<u64>,
    pub local_compute_ms: u64,
    /// Extra attempts a gateway made on the caller's behalf, or unknown.
    pub hidden_attempts: Option<u64>,
}

fn opt_or_unknown(v: Option<u64>) -> Value {
    v.map_or_else(|| json!("unknown"), |n| json!(n))
}

fn read_known(v: &Value, key: &str) -> HarnessResult<Option<u64>> {
    match v.get(key) {
        Some(Value::String(s)) if s == "unknown" => Ok(None),
        Some(n) if n.is_u64() => Ok(n.as_u64()),
        _ => Err(bad(format!(
            "`{key}` must be an unsigned integer or \"unknown\""
        ))),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Observation {
    /// Assigned by the observer.
    pub seq: u64,
    pub provider: String,
    pub capability: String,
    pub stage: Stage,
    pub role: Role,
    pub invocation_id: String,
    pub parent_invocation: Option<String>,
    /// Lineage key: all `Transform` events of one payload share it.
    pub payload_id: Option<String>,
    pub source_revision: String,
    pub config_revision: String,
    /// `None` is explicit "unknown".
    pub upstream_model: Option<String>,
    pub cache: CacheState,
    pub availability: Availability,
    pub outcome: Outcome,
    pub warmth: Warmth,
    pub latency_ms: u64,
    pub cost: Cost,
    /// Payload size entering / leaving a `Transform` stage (`None` = unmeasured).
    pub before: Option<TokenCount>,
    pub after: Option<TokenCount>,
    /// The `after` of this step is the exact final model-visible envelope.
    pub model_visible: bool,
    /// Size of an `Incurred` request (`None` = unmeasured).
    pub incurred: Option<TokenCount>,
    pub before_digest: Option<String>,
    pub after_digest: Option<String>,
    /// Provider-reported usage of an `Incurred` request (`None`: no receipt).
    pub usage: Option<crate::receipt::Usage>,
    /// Locally estimated cost in micro-units (`None`: unknown); never the provider's charge.
    pub estimated_cost: Option<u64>,
}

impl Observation {
    pub fn new(
        provider: &str,
        capability: &str,
        stage: Stage,
        role: Role,
        invocation_id: &str,
    ) -> Self {
        Self {
            seq: 0,
            provider: provider.into(),
            capability: capability.into(),
            stage,
            role,
            invocation_id: invocation_id.into(),
            parent_invocation: None,
            payload_id: None,
            source_revision: "unknown".into(),
            config_revision: "unknown".into(),
            upstream_model: None,
            cache: CacheState::Bypass,
            availability: Availability::Available,
            outcome: Outcome::Ok,
            warmth: Warmth::NotApplicable,
            latency_ms: 0,
            cost: Cost::default(),
            before: None,
            after: None,
            model_visible: false,
            incurred: None,
            before_digest: None,
            after_digest: None,
            usage: None,
            estimated_cost: None,
        }
    }

    /// Bounds check: every string field is short; nothing here can hold text.
    pub fn validate(&self) -> HarnessResult<()> {
        let mut strings: Vec<&str> = vec![
            &self.provider,
            &self.capability,
            &self.invocation_id,
            &self.source_revision,
            &self.config_revision,
        ];
        strings.extend(self.parent_invocation.as_deref());
        strings.extend(self.payload_id.as_deref());
        strings.extend(self.upstream_model.as_deref());
        strings.extend(self.before_digest.as_deref());
        strings.extend(self.after_digest.as_deref());
        for c in [&self.before, &self.after, &self.incurred]
            .into_iter()
            .flatten()
        {
            if let TokenizerId::Named { name, fingerprint } = &c.tokenizer {
                strings.push(name);
                strings.push(fingerprint);
            }
        }
        if strings.iter().any(|s| s.len() > MAX_FIELD_BYTES) {
            return Err(bad(format!(
                "observation field exceeds {MAX_FIELD_BYTES} bytes"
            )));
        }
        if self.provider.is_empty() || self.invocation_id.is_empty() {
            return Err(bad("provider and invocation id are required"));
        }
        if self.role == Role::Transform && self.payload_id.is_none() {
            return Err(bad("transform events need a payload id"));
        }
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        let c = |x: &Option<TokenCount>| x.as_ref().map_or(Value::Null, TokenCount::to_json);
        let s = |x: &Option<String>| x.as_ref().map_or(Value::Null, |v| json!(v));
        let mut v = json!({
            "schema": OBSERVATION_SCHEMA, "seq": self.seq,
            "provider": self.provider, "capability": self.capability,
            "stage": self.stage.as_str(), "role": self.role.as_str(),
            "invocation": {"id": self.invocation_id, "parent": s(&self.parent_invocation)},
            "payload_id": s(&self.payload_id),
            "source_revision": self.source_revision, "config_revision": self.config_revision,
            "upstream_model": self.upstream_model.clone().unwrap_or_else(|| "unknown".into()),
            "cache": self.cache.as_str(), "availability": self.availability.as_str(),
            "outcome": self.outcome.as_str(), "warmth": self.warmth.as_str(),
            "latency_ms": self.latency_ms,
            "cost": {"provider_billed": opt_or_unknown(self.cost.provider_billed),
                     "local_compute_ms": self.cost.local_compute_ms,
                     "hidden_attempts": opt_or_unknown(self.cost.hidden_attempts)},
            "before": c(&self.before), "after": c(&self.after), "model_visible": self.model_visible,
            "incurred": c(&self.incurred),
            "before_digest": s(&self.before_digest), "after_digest": s(&self.after_digest),
        });
        // Additive members appear only when present, so older readers and goldens are unaffected.
        if let Some(u) = &self.usage {
            v["usage"] = u.to_json();
        }
        if let Some(c) = self.estimated_cost {
            v["estimated_cost"] = json!(c);
        }
        v
    }

    /// Strict decode; unknown additive members are ignored, required ones checked.
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        if v.get("schema").and_then(Value::as_str) != Some(OBSERVATION_SCHEMA) {
            return Err(bad("unsupported observation schema"));
        }
        let opt_s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let count = |k: &str| -> HarnessResult<Option<TokenCount>> {
            match v.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(x) => TokenCount::from_json(x).map(Some),
            }
        };
        let inv = v
            .get("invocation")
            .ok_or_else(|| bad("missing invocation"))?;
        let cost = v.get("cost").ok_or_else(|| bad("missing cost"))?;
        let model = field_str(v, "upstream_model")?;
        let o = Self {
            seq: v.get("seq").and_then(Value::as_u64).unwrap_or(0),
            provider: field_str(v, "provider")?,
            capability: field_str(v, "capability")?,
            stage: Stage::parse(&field_str(v, "stage")?)?,
            role: Role::parse(&field_str(v, "role")?)?,
            invocation_id: field_str(inv, "id")?,
            parent_invocation: inv
                .get("parent")
                .and_then(Value::as_str)
                .map(str::to_string),
            payload_id: opt_s("payload_id"),
            source_revision: field_str(v, "source_revision")?,
            config_revision: field_str(v, "config_revision")?,
            upstream_model: (model != "unknown").then_some(model),
            cache: CacheState::parse(&field_str(v, "cache")?)?,
            availability: Availability::parse(&field_str(v, "availability")?)?,
            outcome: Outcome::parse(&field_str(v, "outcome")?)?,
            warmth: Warmth::parse(&field_str(v, "warmth")?)?,
            latency_ms: v
                .get("latency_ms")
                .and_then(Value::as_u64)
                .ok_or_else(|| bad("latency_ms"))?,
            cost: Cost {
                provider_billed: read_known(cost, "provider_billed")?,
                local_compute_ms: cost
                    .get("local_compute_ms")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| bad("local_compute_ms"))?,
                hidden_attempts: read_known(cost, "hidden_attempts")?,
            },
            before: count("before")?,
            after: count("after")?,
            model_visible: v
                .get("model_visible")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            incurred: count("incurred")?,
            before_digest: opt_s("before_digest"),
            after_digest: opt_s("after_digest"),
            usage: v
                .get("usage")
                .filter(|u| u.is_object())
                .map(crate::receipt::Usage::from_json),
            estimated_cost: v.get("estimated_cost").and_then(Value::as_u64),
        };
        o.validate()?;
        Ok(o)
    }
}

pub(crate) fn field_str(v: &Value, key: &str) -> HarnessResult<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| bad(format!("missing string `{key}`")))
}
