//! Workflow routing wiring (HN-16): `[routing]` of `semaprax.harness.toml`
//! plus the machine-local evidence registry become the `RoutingConfig`,
//! `EvidenceRegistry` and `SessionLock` that `governed_decide` runs under. The
//! registry never carries authority: only the predeclared gate turns a real,
//! matched record for the live key into a qualified profile.

use crate::decision::evidence::{EvidenceRecord, MatchedBudget, Origin, Outcome, RetryOwner};
use crate::decision::{
    EvidenceKey, EvidenceRegistry, GateSpec, RoutingConfig, RoutingMode, SessionLock,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::profile::config::{LadderConfig, RoutingSection};
use serde_json::Value;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;

pub const EVIDENCE_SCHEMA: &str = "semaprax.harness-routing-evidence.v1";
/// Machine-local evidence file under the harness home.
pub const EVIDENCE_FILE: &str = "routing/evidence.json";

/// Everything the route step needs beyond the task: policy knobs, the
/// registry and the qualified profile the run locked.
pub struct RoutingWiring {
    pub cfg: RoutingConfig,
    /// `[routing] mode` was written explicitly (otherwise the provider's own
    /// mode decides, as before HN-16).
    pub explicit_mode: bool,
    /// `[routing] allow_remote = true`: approve the remote origins of the
    /// task's own (policy-checked) catalog; otherwise remote stays unapproved.
    pub approve_remote: bool,
    pub registry: Option<EvidenceRegistry>,
    pub spec: GateSpec,
    /// `[routing] cost_aware` (TC-10, opt-in) and the approved per-family ladders.
    pub cost_aware: bool,
    pub ladders: BTreeMap<String, LadderConfig>,
    /// Locked at the first route of the run; later routes of the same run
    /// compare against it, so a changed profile cannot slip in mid-session.
    pub session_lock: RefCell<Option<SessionLock>>,
}

impl Default for RoutingWiring {
    fn default() -> Self {
        Self {
            cfg: RoutingConfig::default(),
            explicit_mode: false,
            approve_remote: false,
            registry: None,
            spec: GateSpec::default(),
            cost_aware: false,
            ladders: BTreeMap::new(),
            session_lock: RefCell::new(None),
        }
    }
}

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPJ017", msg)
}

impl RoutingWiring {
    /// Build from the project's `[routing]` and the machine-local evidence file.
    pub fn from_config(sec: &RoutingSection, home: Option<&Path>) -> HarnessResult<Self> {
        let mode = match sec.mode.as_str() {
            "pin" => RoutingMode::Pin(sec.pin.clone().unwrap_or_default()),
            "experimental" => RoutingMode::Experimental,
            "auto" => RoutingMode::QualifiedAuto,
            _ => RoutingMode::Rules,
        };
        let cfg = RoutingConfig {
            mode,
            project_pin: sec.pin.clone(),
            user_allow_remote: sec.allow_remote != Some(false),
            ..RoutingConfig::default()
        };
        let registry = match (sec.mode.as_str(), home) {
            (m, Some(h)) if m == "auto" || sec.cost_aware => load_registry(h)?,
            _ => None,
        };
        Ok(Self {
            cfg,
            explicit_mode: sec.explicit,
            approve_remote: sec.allow_remote == Some(true),
            registry,
            cost_aware: sec.cost_aware,
            ladders: sec.ladders.clone(),
            ..Self::default()
        })
    }

    /// The session lock for `key`: the first call fixes it (to the registry's
    /// record for the key, when there is one); later calls return that lock.
    pub fn lock_for(&self, key: &EvidenceKey) -> Option<SessionLock> {
        let mut slot = self.session_lock.borrow_mut();
        if slot.is_none() {
            *slot = self
                .registry
                .as_ref()
                .and_then(|r| r.get(key))
                .map(|rec| SessionLock {
                    key_digest: key.digest(),
                    record_digest: rec.digest(),
                });
        }
        slot.clone()
    }
}

fn s(v: &Value, k: &str) -> HarnessResult<String> {
    v[k].as_str()
        .map(str::to_string)
        .ok_or_else(|| bad(format!("evidence field `{k}` must be a string")))
}

fn set(v: &Value) -> BTreeSet<String> {
    v.as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn outcome(v: &Value) -> HarnessResult<Outcome> {
    let origin = match v["origin"].as_str() {
        Some("real") => Origin::Real,
        Some("fixture") => Origin::Fixture,
        Some("unavailable") => Origin::Unavailable,
        _ => return Err(bad("outcome `origin` must be real, fixture or unavailable")),
    };
    Ok(Outcome {
        item: s(v, "item")?,
        arm: s(v, "arm")?,
        model: s(v, "model")?,
        origin,
        verified_by: v["verified_by"].as_str().unwrap_or("").to_string(),
        completed: v["completed"].as_bool().unwrap_or(false),
        regressions: v["regressions"].as_u64().unwrap_or(0) as u32,
        attempts: v["attempts"].as_u64().unwrap_or(1) as u32,
        cost_micros: v["cost_micros"].as_u64(),
        latency_ms: v["latency_ms"].as_u64(),
        router_cost_micros: v["router_cost_micros"].as_u64().unwrap_or(0),
        context_cost_micros: v["context_cost_micros"].as_u64().unwrap_or(0),
        retry_owner: match v["retry_owner"].as_str() {
            Some("gateway") => RetryOwner::Gateway,
            _ => RetryOwner::Host,
        },
    })
}

/// Optional MR-02 outcome calibration of a record; absent is uncalibrated.
fn calibration(v: &Value) -> HarnessResult<Option<crate::decision::Calibration>> {
    if v.is_null() {
        return Ok(None);
    }
    let kind = v["score_kind"]
        .as_str()
        .and_then(crate::decision::ScoreKind::parse)
        .filter(|k| *k != crate::decision::ScoreKind::None)
        .ok_or_else(|| bad("calibration `score_kind` must be a scored kind"))?;
    Ok(Some(crate::decision::Calibration {
        calibration_id: s(v, "calibration_id")?,
        score_kind: kind,
        key_digest: s(v, "key_digest")?,
        success_estimate: v["success_estimate"]
            .as_f64()
            .ok_or_else(|| bad("calibration `success_estimate` must be a number"))?,
    }))
}

/// Parse an evidence document (`semaprax.harness-routing-evidence.v1`).
pub fn parse_registry(bytes: &[u8]) -> HarnessResult<EvidenceRegistry> {
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|e| bad(format!("evidence is not JSON: {e}")))?;
    if doc["schema"] != EVIDENCE_SCHEMA {
        return Err(bad(format!("evidence schema must be `{EVIDENCE_SCHEMA}`")));
    }
    let mut reg = EvidenceRegistry::default();
    for r in doc["records"].as_array().into_iter().flatten() {
        let k = &r["key"];
        let key = EvidenceKey {
            task: s(k, "task")?,
            provider_id: s(k, "provider")?,
            weights_digest: s(k, "weights")?,
            catalog_digest: s(k, "catalog")?,
            normalization: s(k, "normalization")?,
            distribution: s(k, "distribution")?,
        };
        let outcomes = r["outcomes"]
            .as_array()
            .into_iter()
            .flatten()
            .map(outcome)
            .collect::<HarnessResult<Vec<_>>>()?;
        reg.register(EvidenceRecord {
            key,
            budget: MatchedBudget {
                max_cost_micros: r["budget"]["max_cost_micros"].as_u64().unwrap_or(0),
                max_attempts: r["budget"]["max_attempts"].as_u64().unwrap_or(1) as u32,
            },
            eval_items: set(&r["eval_items"]),
            trained_on: set(&r["trained_on"]),
            outcomes,
            calibration: calibration(&r["calibration"])?,
        })?;
    }
    Ok(reg)
}

/// Load `<home>/routing/evidence.json`; absent is no evidence (never an error).
pub fn load_registry(home: &Path) -> HarnessResult<Option<EvidenceRegistry>> {
    match std::fs::read(home.join(EVIDENCE_FILE)) {
        Ok(b) => parse_registry(&b).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(bad(format!("cannot read {EVIDENCE_FILE}: {e}"))),
    }
}
