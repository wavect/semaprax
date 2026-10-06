//! Decision-adapter identities (MR-15): adapter implementation, model profile
//! and configured instance are three separate things.
//!
//! - adapter implementation: descriptor `provider.id` + `adapter.version`;
//! - model profile: which model/checkpoint and what it can do (capabilities
//!   checked before inference), supplied as host configuration;
//! - configured instance: endpoint or local worker plus a secret *reference*.
//!
//! Credentials are never part of a profile. No routing code branches on a
//! vendor or model name; everything below is declared data.

use super::call::{IdentityKind, ScoreKind};
use super::diag::DecisionResult;
use super::json;
use super::render::RENDERER_V2;
use super::route::{bad, enum_of, flag, shape, uint};
use super::route_v2::{modalities, Modality};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// Host environment variable carrying a model profile as JSON.
pub const MODEL_PROFILE_VAR: &str = "SEMAPRAX_HARNESS_MODEL_PROFILE";
/// Adapter config field carrying the same JSON (forwarded as
/// `SEMAPRAX_HARNESS_CFG_MODEL_PROFILE`).
pub const MODEL_PROFILE_FIELD: &str = "model_profile";
/// Renderers this host implements; a profile naming any other is refused.
pub const RENDERERS: [&str; 1] = [RENDERER_V2];

const C: &str = "SPX-HPJ020";

fn ident(v: &Value, k: &str, max: usize) -> DecisionResult<String> {
    match v.get(k).and_then(Value::as_str) {
        Some(s) if super::call::ident_ok(s, max) => Ok(s.to_string()),
        _ => Err(bad(
            C,
            format!("model profile `{k}` must be an identifier of at most {max} bytes"),
        )),
    }
}

/// Adapter implementation identity.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterIdentity {
    pub provider_id: String,
    pub adapter_version: String,
}

impl AdapterIdentity {
    pub fn label(&self) -> String {
        format!("{}@{}", self.provider_id, self.adapter_version)
    }
}

/// A capability-driven model profile for one decision adapter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelProfile {
    pub profile_id: String,
    pub model: String,
    /// Attestable immutable checkpoint, when the backend has one.
    pub checkpoint: Option<String>,
    pub identity_kind: IdentityKind,
    /// Declared score semantics of a scored answer.
    pub score_kind: ScoreKind,
    /// The adapter may answer without scores (`score_kind: none`).
    pub scoreless: bool,
    pub max_options: u32,
    pub max_state_bytes: u32,
    pub modalities: BTreeSet<Modality>,
    pub renderer: String,
}

impl ModelProfile {
    /// Strict parse; unknown members, an unknown renderer (contract version),
    /// inconsistent identity or score declarations are refused (`SPX-HPJ020`).
    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        let m = shape(
            v,
            "model profile",
            &["profile_id", "model"],
            &[
                "checkpoint",
                "identity_kind",
                "score_kind",
                "scoreless",
                "max_options",
                "max_state_bytes",
                "modalities",
                "renderer",
            ],
            C,
        )?;
        let checkpoint = match m.get("checkpoint") {
            None | Some(Value::Null) => None,
            Some(_) => Some(ident(v, "checkpoint", 128)?),
        };
        let identity_kind = match m.get("identity_kind") {
            None => IdentityKind::Unknown,
            Some(_) => enum_of(m, "identity_kind", IdentityKind::parse, C)?,
        };
        let score_kind = match m.get("score_kind") {
            None => ScoreKind::OptionDistribution,
            Some(_) => enum_of(m, "score_kind", ScoreKind::parse, C)?,
        };
        let scoreless = match m.get("scoreless") {
            None => score_kind == ScoreKind::None,
            Some(_) => flag(m, "scoreless", C)?,
        };
        let renderer = match m.get("renderer") {
            None => RENDERER_V2.to_string(),
            Some(r) => r.as_str().unwrap_or("").to_string(),
        };
        if !RENDERERS.contains(&renderer.as_str()) {
            return Err(bad(
                C,
                format!("model profile renderer `{renderer}` is not a contract version this host implements"),
            ));
        }
        let p = Self {
            profile_id: ident(v, "profile_id", 64)?,
            model: ident(v, "model", 128)?,
            checkpoint,
            identity_kind,
            score_kind,
            scoreless,
            max_options: match m.get("max_options") {
                None => 16,
                Some(_) => uint(m, "max_options", 16, C)? as u32,
            },
            max_state_bytes: match m.get("max_state_bytes") {
                None => 4096,
                Some(_) => uint(m, "max_state_bytes", 4096, C)? as u32,
            },
            modalities: match m.get("modalities") {
                None => [Modality::Text].into(),
                Some(_) => modalities(m, C)?,
            },
            renderer,
        };
        if p.max_options < 2 || p.max_state_bytes < 256 {
            return Err(bad(
                C,
                "model profile needs max_options >= 2 and max_state_bytes >= 256",
            ));
        }
        if p.identity_kind == IdentityKind::ImmutableCheckpoint && p.checkpoint.is_none() {
            return Err(bad(
                C,
                "an immutable_checkpoint profile must name its checkpoint",
            ));
        }
        if p.score_kind == ScoreKind::None && !p.scoreless {
            return Err(bad(C, "score_kind `none` requires `scoreless: true`"));
        }
        Ok(p)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "profile_id": self.profile_id, "model": self.model, "checkpoint": self.checkpoint,
            "identity_kind": self.identity_kind.as_str(), "score_kind": self.score_kind.as_str(),
            "scoreless": self.scoreless, "max_options": self.max_options,
            "max_state_bytes": self.max_state_bytes,
            "modalities": self.modalities.iter().map(|m| m.as_str()).collect::<Vec<_>>(),
            "renderer": self.renderer,
        })
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.model-profile.v1", &self.to_json())
    }

    /// The checkpoint label used in cache/evidence keys. A mutable service
    /// carries an explicit mutable label, never an invented weights digest.
    pub fn checkpoint_label(&self) -> String {
        match (&self.checkpoint, self.identity_kind) {
            (Some(c), _) => c.clone(),
            (None, k) => format!("{}:{}", k.as_str(), self.model),
        }
    }

    /// A scored answer of `kind` is consistent with the declaration.
    pub fn admits_score_kind(&self, kind: ScoreKind) -> bool {
        match kind {
            ScoreKind::None => self.scoreless,
            k => self.score_kind == ScoreKind::None || k == self.score_kind,
        }
    }
}

/// A configured instance of an adapter: where it runs and which secret it is
/// allowed to name. The secret value is never held here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceConfig {
    pub instance_id: String,
    /// Endpoint origin or local worker id; `None` is the adapter default.
    pub endpoint: Option<String>,
    /// Names of the granted secrets (references, not values).
    pub secret_refs: Vec<String>,
}

impl InstanceConfig {
    pub fn to_json(&self) -> Value {
        json!({"instance_id": self.instance_id, "endpoint": self.endpoint, "secret_refs": self.secret_refs})
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.instance.v1", &self.to_json())
    }
}
