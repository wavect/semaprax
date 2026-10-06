//! Evidence registry (HN-16): measured task outcomes keyed by what a learned
//! routing profile actually is. A record never carries authority; only the
//! predeclared gate in `qualify` turns one into an `EnablementGate`.

use super::provider::ProviderProfile;
use super::registry::DecisionTask;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Feature normalization identity: the closed `model-route/v1` feature set.
pub const NORMALIZATION_ID: &str = "model-route/v1/closed-features.v1";
/// `model-route/v2` normalization: v2 features through the host renderer.
pub const NORMALIZATION_ID_V2: &str = "model-route/v2/semaprax.route-render.v2";

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPJ017", msg)
}

/// Everything a qualification applies to. Any change is a different key, so
/// evidence for one profile never transfers to another.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EvidenceKey {
    pub task: String,
    pub provider_id: String,
    /// Weights/checkpoint identity (`ProviderProfile::checkpoint`).
    pub weights_digest: String,
    /// Digest of the approved candidate catalog the profile chooses from.
    pub catalog_digest: String,
    pub normalization: String,
    /// Declared task distribution (families and context ceiling).
    pub distribution: String,
}

impl EvidenceKey {
    /// The key of the profile as it would run now.
    pub fn live(profile: &ProviderProfile, catalog_digest: &str) -> Self {
        Self::live_versioned(profile, catalog_digest, 1)
    }

    /// The key at a negotiated wire version. v1 and v2 keys never coincide
    /// (task and normalization differ), so qualification never transfers; the
    /// MR-15 adapter/profile/instance scope is bound into the distribution
    /// when the profile declares one (legacy v1 keys keep their bytes).
    pub fn live_versioned(profile: &ProviderProfile, catalog_digest: &str, version: u32) -> Self {
        let fam: Vec<&str> = profile
            .supported_families
            .as_ref()
            .map(|s| s.iter().map(|f| f.as_str()).collect())
            .unwrap_or_default();
        let mut dist = json!({"families": fam, "max_context_tokens": profile.max_context_tokens});
        let scope = profile.scope_json();
        if !scope.is_null() {
            dist["scope"] = scope;
        }
        let distribution = json::digest("semaprax.decision.distribution.v1", &dist);
        let (task, normalization) = if version == 2 {
            (DecisionTask::ModelRouteV2.id(), NORMALIZATION_ID_V2)
        } else {
            (DecisionTask::ModelRoute.id(), NORMALIZATION_ID)
        };
        Self {
            task: task.into(),
            provider_id: profile.provider_id.clone(),
            weights_digest: profile.checkpoint.clone(),
            catalog_digest: catalog_digest.into(),
            normalization: normalization.into(),
            distribution,
        }
    }

    /// MR-13: the same key bound to one execution domain and the exact
    /// candidate-set and renderer revisions the evidence was collected under.
    /// Development and application evidence never share a key, and a changed
    /// candidate or renderer revision is a different key.
    pub fn bound(
        &self,
        domain: super::route_v2::ExecutionDomain,
        candidate_revision: &str,
        renderer_revision: &str,
    ) -> Self {
        let mut k = self.clone();
        k.distribution = json::digest(
            "semaprax.decision.distribution.mr13.v1",
            &json!({"base": self.distribution, "execution_domain": domain.as_str(),
                    "candidate_revision": candidate_revision,
                    "renderer_revision": renderer_revision}),
        );
        k
    }

    /// The key of a `choice-select/v1` profile over one admitted option set
    /// (MR-11). Task and normalization are the choice task's own, so no
    /// model-route evidence, gate or calibration ever matches it.
    pub fn choice(profile: &ProviderProfile, option_set_digest: &str) -> Self {
        let scope = profile.scope_json();
        let distribution = json::digest(
            "semaprax.decision.distribution.v1",
            &json!({"task": DecisionTask::ChoiceSelect.id(), "scope": scope}),
        );
        Self {
            task: DecisionTask::ChoiceSelect.id().into(),
            provider_id: profile.provider_id.clone(),
            weights_digest: profile.checkpoint.clone(),
            catalog_digest: option_set_digest.into(),
            normalization: semaprax_decision_core::CHOICE_NORMALIZATION.into(),
            distribution,
        }
    }

    pub fn to_json(&self) -> Value {
        json!({"task": self.task, "provider": self.provider_id, "weights": self.weights_digest,
               "catalog": self.catalog_digest, "normalization": self.normalization,
               "distribution": self.distribution})
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.evidence-key.v1", &self.to_json())
    }
}

/// Where an observation came from. Only `Real` can support a gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    Real,
    Fixture,
    /// The model or provider could not run; never a success.
    Unavailable,
}

impl Origin {
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::Real => "real",
            Origin::Fixture => "fixture",
            Origin::Unavailable => "unavailable",
        }
    }
}

/// Who owned transport retries for the run (never counted twice).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryOwner {
    Host,
    Gateway,
}

/// One arm's verified result on one evaluation item.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub item: String,
    /// `rules` or the learned provider id.
    pub arm: String,
    pub model: String,
    pub origin: Origin,
    /// Independent verifier (law gates, tests, scripted grader), never the
    /// model or router.
    pub verified_by: String,
    pub completed: bool,
    pub regressions: u32,
    pub attempts: u32,
    /// `None` is unknown, never zero.
    pub cost_micros: Option<u64>,
    pub latency_ms: Option<u64>,
    /// Router, context and skill spend charged to this task.
    pub router_cost_micros: u64,
    pub context_cost_micros: u64,
    pub retry_owner: RetryOwner,
}

impl Outcome {
    fn validate(&self) -> HarnessResult<()> {
        if self.item.is_empty() || self.arm.is_empty() {
            return Err(bad("outcome needs an item and an arm"));
        }
        if self.completed {
            if self.origin == Origin::Unavailable {
                return Err(bad(format!(
                    "`{}`: an unavailable cell cannot be a success",
                    self.item
                )));
            }
            if self.cost_micros.is_none() {
                return Err(bad(format!(
                    "`{}`: unknown cost cannot be recorded as a zero-cost success",
                    self.item
                )));
            }
            if self.verified_by.is_empty() {
                return Err(bad(format!(
                    "`{}`: success without an independent verifier",
                    self.item
                )));
            }
        }
        Ok(())
    }

    /// Total spend including router/context; unknown cost is charged the
    /// matched ceiling so a failure never looks cheap.
    pub fn total_cost(&self, ceiling: u64) -> u64 {
        self.cost_micros.unwrap_or(ceiling) + self.router_cost_micros + self.context_cost_micros
    }

    fn to_json(&self) -> Value {
        json!({"item": self.item, "arm": self.arm, "model": self.model, "origin": self.origin.as_str(),
               "verified_by": self.verified_by, "completed": self.completed,
               "regressions": self.regressions, "attempts": self.attempts,
               "cost_micros": self.cost_micros, "latency_ms": self.latency_ms,
               "router_cost_micros": self.router_cost_micros,
               "context_cost_micros": self.context_cost_micros,
               "retry_owner": match self.retry_owner { RetryOwner::Host => "host", RetryOwner::Gateway => "gateway" }})
    }
}

/// The budget every compared arm was held to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchedBudget {
    pub max_cost_micros: u64,
    pub max_attempts: u32,
}

/// An outcome-calibration of a provider's scores (MR-02), bound to exactly one
/// evidence key (provider, checkpoint, task profile distribution, renderer and
/// candidate-set regime). Raw scores without one are uncalibrated.
#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    pub calibration_id: String,
    /// The score kind that was calibrated; never `none`.
    pub score_kind: super::call::ScoreKind,
    /// Digest of the evidence key the calibration was fit on.
    pub key_digest: String,
    /// Calibrated estimate of task success at the operating threshold.
    pub success_estimate: f64,
}

impl Calibration {
    fn to_json(&self) -> Value {
        json!({"calibration_id": self.calibration_id, "score_kind": self.score_kind.as_str(),
               "key_digest": self.key_digest, "success_estimate": self.success_estimate})
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct EvidenceRecord {
    pub key: EvidenceKey,
    pub budget: MatchedBudget,
    /// Sealed held-out items.
    pub eval_items: BTreeSet<String>,
    /// Items the provider was trained or calibrated on.
    pub trained_on: BTreeSet<String>,
    pub outcomes: Vec<Outcome>,
    /// Outcome calibration of the provider's scores; `None` is uncalibrated.
    pub calibration: Option<Calibration>,
}

impl EvidenceRecord {
    pub fn digest(&self) -> String {
        let mut o: Vec<Value> = self.outcomes.iter().map(Outcome::to_json).collect();
        o.sort_by_key(json::canonical);
        let mut v = json!({"key": self.key.to_json(),
                    "budget": {"max_cost_micros": self.budget.max_cost_micros, "max_attempts": self.budget.max_attempts},
                    "eval_items": self.eval_items, "trained_on": self.trained_on, "outcomes": o});
        if let Some(c) = &self.calibration {
            v["calibration"] = c.to_json();
        }
        json::digest("semaprax.decision.evidence-record.v1", &v)
    }

    pub fn validate(&self) -> HarnessResult<()> {
        self.outcomes.iter().try_for_each(Outcome::validate)
    }
}

/// Records by key digest. Registration validates; it never enables anything.
#[derive(Default, Debug)]
pub struct EvidenceRegistry {
    records: BTreeMap<String, EvidenceRecord>,
}

impl EvidenceRegistry {
    pub fn register(&mut self, record: EvidenceRecord) -> HarnessResult<String> {
        record.validate()?;
        let k = record.key.digest();
        self.records.insert(k.clone(), record);
        Ok(k)
    }

    pub fn get(&self, key: &EvidenceKey) -> Option<&EvidenceRecord> {
        self.records.get(&key.digest())
    }

    /// The keys of every record (read-only; MR-14 staleness review).
    pub fn keys(&self) -> impl Iterator<Item = &EvidenceKey> {
        self.records.values().map(|r| &r.key)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
