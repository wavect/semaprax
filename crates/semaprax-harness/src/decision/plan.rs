//! Frozen fallback plan per attempt lineage and the attempt ledger that
//! meters initial-route, reasoning-escalation and transport-retry events.

use super::policy::LineageBudgets;
use super::route::{bad, shape, text, Destination, ModelPlan};
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanSlot {
    pub model_id: String,
    pub destination: Destination,
    pub authorized: bool,
}

/// Ordered admitted plans for one lineage. Fields are data; the type offers no
/// mutator, and any later change means a new lineage, never a reshuffle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrozenRoutePlan {
    pub lineage_id: String,
    pub ordered: Vec<PlanSlot>,
    pub decision_digest: String,
    pub policy_digest: String,
}

impl FrozenRoutePlan {
    /// Chosen plan first, then an escalation ladder: stronger plans ascending
    /// by strength, then weaker ones descending; ties by cost then id. Only
    /// admissible plans appear, so no unauthorized destination is ever listed.
    pub fn freeze(
        lineage_id: &str,
        chosen: &str,
        admissible: &[ModelPlan],
        decision_digest: String,
        policy_digest: String,
    ) -> HarnessResult<Self> {
        let first = admissible
            .iter()
            .find(|p| p.id == chosen)
            .ok_or_else(|| bad("SPX-HPJ011", format!("choice `{chosen}` is not admissible")))?;
        let mut rest: Vec<&ModelPlan> = admissible.iter().filter(|p| p.id != chosen).collect();
        let s = first.strength_rank;
        rest.sort_by(|a, b| {
            let key = |p: &ModelPlan| {
                (
                    p.strength_rank < s,
                    if p.strength_rank >= s {
                        p.strength_rank
                    } else {
                        u32::MAX - p.strength_rank
                    },
                    p.est_cost_micros,
                    p.id.clone(),
                )
            };
            key(a).cmp(&key(b))
        });
        let ordered = std::iter::once(first)
            .chain(rest)
            .map(|p| PlanSlot {
                model_id: p.id.clone(),
                destination: p.destination.clone(),
                authorized: true,
            })
            .collect();
        Ok(Self {
            lineage_id: lineage_id.to_string(),
            ordered,
            decision_digest,
            policy_digest,
        })
    }

    /// `(id, authorized)` in exact order; maps 1:1 onto
    /// `ProviderPolicy::new(Vec<ProviderSlot>)` (slot 0 is the primary).
    pub fn to_provider_slots(&self) -> Vec<(String, bool)> {
        self.ordered
            .iter()
            .map(|s| (s.model_id.clone(), s.authorized))
            .collect()
    }

    pub fn to_json(&self) -> Value {
        let slots: Vec<Value> = self
            .ordered
            .iter()
            .map(|s| {
                let d = match &s.destination {
                    Destination::Local => json!({"kind": "local"}),
                    Destination::Remote { origin } => json!({"kind": "remote", "origin": origin}),
                };
                json!({"model_id": s.model_id, "destination": d, "authorized": s.authorized})
            })
            .collect();
        json!({"lineage_id": self.lineage_id, "ordered": slots, "decision_digest": self.decision_digest, "policy_digest": self.policy_digest})
    }

    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        const C: &str = "SPX-HPJ008";
        let m = shape(
            v,
            "plan",
            &["lineage_id", "ordered", "decision_digest", "policy_digest"],
            &[],
            C,
        )?;
        let mut ordered = Vec::new();
        for s in m["ordered"]
            .as_array()
            .ok_or_else(|| bad(C, "`ordered` must be an array"))?
        {
            let sm = shape(
                s,
                "plan slot",
                &["model_id", "destination", "authorized"],
                &[],
                C,
            )?;
            let d = shape(&sm["destination"], "destination", &["kind"], &["origin"], C)?;
            let destination = match d["kind"].as_str() {
                Some("local") => Destination::Local,
                Some("remote") => Destination::Remote {
                    origin: text(d, "origin", C)?,
                },
                _ => return Err(bad(C, "bad destination kind")),
            };
            ordered.push(PlanSlot {
                model_id: text(sm, "model_id", C)?,
                destination,
                authorized: sm["authorized"]
                    .as_bool()
                    .ok_or_else(|| bad(C, "`authorized` must be boolean"))?,
            });
        }
        Ok(Self {
            lineage_id: text(m, "lineage_id", C)?,
            ordered,
            decision_digest: text(m, "decision_digest", C)?,
            policy_digest: text(m, "policy_digest", C)?,
        })
    }

    pub fn digest(&self) -> String {
        json::digest("semaprax.decision.plan.v1", &self.to_json())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AttemptKind {
    InitialRoute,
    ReasoningEscalation,
    TransportRetry,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptGrant {
    pub slot_index: usize,
    pub model_id: String,
    pub destination: Destination,
    /// True when `attempt_id` was already recorded: no budget was consumed.
    pub duplicate: bool,
}

/// Meters attempts against a frozen plan. Escalation moves forward through the
/// plan (never skipping or reordering); a transport retry repeats the current
/// slot; each kind has its own budget and duplicate attempt ids are idempotent.
#[derive(Clone, Debug)]
pub struct AttemptLedger {
    plan: FrozenRoutePlan,
    budgets: LineageBudgets,
    used: BTreeMap<AttemptKind, u32>,
    cursor: usize,
    started: bool,
    seen: BTreeMap<String, (AttemptKind, AttemptGrant)>,
    duplicates: u32,
}

impl AttemptLedger {
    pub fn new(plan: FrozenRoutePlan, budgets: LineageBudgets) -> Self {
        Self {
            plan,
            budgets,
            used: BTreeMap::new(),
            cursor: 0,
            started: false,
            seen: BTreeMap::new(),
            duplicates: 0,
        }
    }

    pub fn plan(&self) -> &FrozenRoutePlan {
        &self.plan
    }

    pub fn used(&self, kind: AttemptKind) -> u32 {
        self.used.get(&kind).copied().unwrap_or(0)
    }

    pub fn duplicates(&self) -> u32 {
        self.duplicates
    }

    pub fn begin(&mut self, kind: AttemptKind, attempt_id: &str) -> HarnessResult<AttemptGrant> {
        if let Some((k, g)) = self.seen.get(attempt_id) {
            if *k != kind {
                return Err(bad(
                    "SPX-HPJ010",
                    format!("attempt `{attempt_id}` was already recorded as another kind"),
                ));
            }
            self.duplicates += 1;
            return Ok(AttemptGrant {
                duplicate: true,
                ..g.clone()
            });
        }
        let limit = match kind {
            AttemptKind::InitialRoute => self.budgets.initial_route,
            AttemptKind::ReasoningEscalation => self.budgets.reasoning_escalation,
            AttemptKind::TransportRetry => self.budgets.transport_retry,
        };
        if self.used(kind) >= limit {
            return Err(bad("SPX-HPJ009", format!("{kind:?} budget exhausted")));
        }
        let index = match kind {
            AttemptKind::InitialRoute => {
                if self.started {
                    return Err(bad(
                        "SPX-HPJ009",
                        "initial route already taken for this lineage",
                    ));
                }
                0
            }
            _ if !self.started => {
                return Err(bad("SPX-HPJ009", "no initial route taken for this lineage"))
            }
            AttemptKind::TransportRetry => self.cursor,
            AttemptKind::ReasoningEscalation => self.cursor + 1,
        };
        let slot = self
            .plan
            .ordered
            .get(index)
            .ok_or_else(|| bad("SPX-HPJ009", "frozen plan has no further alternative"))?;
        if !slot.authorized {
            return Err(bad(
                "SPX-HPJ009",
                format!("plan slot `{}` is not authorized", slot.model_id),
            ));
        }
        let grant = AttemptGrant {
            slot_index: index,
            model_id: slot.model_id.clone(),
            destination: slot.destination.clone(),
            duplicate: false,
        };
        *self.used.entry(kind).or_insert(0) += 1;
        self.started = true;
        self.cursor = index;
        self.seen
            .insert(attempt_id.to_string(), (kind, grant.clone()));
        Ok(grant)
    }
}
