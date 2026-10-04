//! Recorded decisions and deterministic replay: no router call, current
//! inputs must still match every recorded digest.

use super::plan::FrozenRoutePlan;
use super::registry::{self, DecisionTask};
use super::route::{bad, screen, shape, text};
use super::router::{choice_digest, DecisionSource, Digests, RouteDecision, RouteInputs};
use crate::diag::HarnessResult;
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
        }
    }

    pub fn to_json(&self) -> Value {
        json!({
            "schema": RECORD_SCHEMA, "task": DecisionTask::ModelRoute.id(),
            "provider_id": self.provider_id, "checkpoint": self.checkpoint, "source": self.source,
            "choice": self.choice, "digests": self.digests.to_json(), "plan": self.plan.to_json(),
        })
    }

    pub fn from_json(v: &Value) -> HarnessResult<Self> {
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
            &[],
            C,
        )?;
        if m["schema"] != RECORD_SCHEMA {
            return Err(bad(C, "unknown record schema"));
        }
        registry::resolve(m["task"].as_str().unwrap_or(""))?;
        let dm = shape(
            &m["digests"],
            "digests",
            &["features", "catalog", "policy", "candidates"],
            &[],
            C,
        )?;
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
            },
            plan: FrozenRoutePlan::from_json(&m["plan"])?,
        })
    }
}

/// Rebuild the frozen plan from a record against the current inputs. Any
/// changed feature, catalog, policy or candidate set is a refusal (`SPX-HPJ007`).
pub fn replay(record: &DecisionRecord, inputs: &RouteInputs) -> HarnessResult<FrozenRoutePlan> {
    let scr = screen(&inputs.request, &inputs.policy);
    let now = inputs.digests(&scr);
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
