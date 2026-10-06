//! Recorded decisions and deterministic replay: no router call, current
//! inputs must still match every recorded digest.

use super::call::CallMetadata;
use super::diag::DecisionResult;
use super::plan::FrozenRoutePlan;
use super::registry;
use super::render::V2Digests;
use super::route::{bad, screen, shape, text};
use super::router::{choice_digest, DecisionSource, Digests, RouteDecision, RouteInputs};
use serde_json::{json, Value};

pub const RECORD_SCHEMA: &str = "semaprax.decision-record.v1";

#[derive(Clone, Debug, PartialEq)]
pub struct DecisionRecord {
    pub provider_id: String,
    pub checkpoint: String,
    pub source: String,
    pub choice: String,
    pub digests: Digests,
    pub plan: FrozenRoutePlan,
    /// MR-03: answering identity/usage of the router call that produced the
    /// choice, journaled so resume never re-infers it. Absent for rules.
    pub identity: Option<CallMetadata>,
}

impl DecisionRecord {
    pub fn from_decision(d: &RouteDecision) -> Self {
        let source = match d.source {
            DecisionSource::Fallback(r) => format!("fallback:{r:?}"),
            s => format!("{s:?}").to_lowercase(),
        };
        Self {
            provider_id: d.provider_id.clone(),
            checkpoint: d.checkpoint.clone(),
            source,
            choice: d.choice.clone(),
            digests: d.digests.clone(),
            plan: d.plan.clone(),
            identity: d.wire.call.clone(),
        }
    }

    pub fn to_json(&self) -> Value {
        let mut v = json!({
            "schema": RECORD_SCHEMA, "task": self.digests.task(),
            "provider_id": self.provider_id, "checkpoint": self.checkpoint, "source": self.source,
            "choice": self.choice, "digests": self.digests.to_json(), "plan": self.plan.to_json(),
        });
        if let Some(c) = &self.identity {
            v["identity"] = c.to_json();
        }
        v
    }

    pub fn from_json(v: &Value) -> DecisionResult<Self> {
        const C: &str = "SPX-HPJ008";
        let m = shape(
            v,
            "decision record",
            &[
                "schema",
                "task",
                "provider_id",
                "checkpoint",
                "source",
                "choice",
                "digests",
                "plan",
            ],
            &["identity"],
            C,
        )?;
        if m["schema"] != RECORD_SCHEMA {
            return Err(bad(C, "unknown record schema"));
        }
        registry::resolve_route(m["task"].as_str().unwrap_or(""))?;
        let dm = shape(
            &m["digests"],
            "digests",
            &["features", "catalog", "policy", "candidates"],
            &["v2"],
            C,
        )?;
        let v2 = match dm.get("v2") {
            None => None,
            Some(x) => {
                let x = shape(
                    x,
                    "v2 digests",
                    &["features", "candidates", "renderer", "disclosure"],
                    &[],
                    C,
                )?;
                Some(V2Digests {
                    features: text(x, "features", C)?,
                    candidates: text(x, "candidates", C)?,
                    renderer: text(x, "renderer", C)?,
                    disclosure: text(x, "disclosure", C)?,
                })
            }
        };
        let task = m["task"].as_str().unwrap_or("");
        if (task == "model-route/v2") != v2.is_some() {
            return Err(bad(
                C,
                "record task and digests disagree on the routing version",
            ));
        }
        let identity = match m.get("identity") {
            None => None,
            Some(c) => Some(CallMetadata::from_json(c).map_err(|e| bad(C, e.message))?),
        };
        Ok(Self {
            provider_id: text(m, "provider_id", C)?,
            checkpoint: text(m, "checkpoint", C)?,
            source: text(m, "source", C)?,
            choice: text(m, "choice", C)?,
            digests: Digests {
                features: text(dm, "features", C)?,
                catalog: text(dm, "catalog", C)?,
                policy: text(dm, "policy", C)?,
                candidates: text(dm, "candidates", C)?,
                v2,
            },
            plan: FrozenRoutePlan::from_json(&m["plan"])?,
            identity,
        })
    }
}

/// Rebuild the frozen plan from a record against the current inputs. Any
/// changed feature, catalog, policy or candidate set is a refusal (`SPX-HPJ007`).
pub fn replay(record: &DecisionRecord, inputs: &RouteInputs) -> DecisionResult<FrozenRoutePlan> {
    let scr = screen(&inputs.request, &inputs.policy);
    let mut now = inputs.digests(&scr);
    if record.digests.v2.is_some() {
        // Recompute the v2 projection deterministically: no router call.
        now.v2 = inputs.prepare_v2(&scr).ok().map(|p| p.digests());
        if now.v2 != record.digests.v2 {
            return Err(bad(
                "SPX-HPJ007",
                "replay refused: v2 feature/candidate/renderer/disclosure digest changed since the decision was recorded",
            ));
        }
    }
    for (name, a, b) in [
        ("features", &now.features, &record.digests.features),
        ("catalog", &now.catalog, &record.digests.catalog),
        ("policy", &now.policy, &record.digests.policy),
        ("candidates", &now.candidates, &record.digests.candidates),
    ] {
        if a != b {
            return Err(bad(
                "SPX-HPJ007",
                format!("replay refused: {name} digest changed since the decision was recorded"),
            ));
        }
    }
    let digest = choice_digest(
        &now,
        &record.provider_id,
        &record.checkpoint,
        &record.choice,
    );
    let plan = FrozenRoutePlan::freeze(
        &record.plan.lineage_id,
        &record.choice,
        &scr.admissible,
        digest,
        inputs.policy_digest(),
    )?;
    if plan != record.plan {
        return Err(bad(
            "SPX-HPJ007",
            "replay refused: recorded plan does not match the rebuilt plan",
        ));
    }
    Ok(plan)
}
