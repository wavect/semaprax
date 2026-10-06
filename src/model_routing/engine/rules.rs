//! The builtin rules decision provider: project policy, no model call.

use super::policy::RoutePolicy;
use super::route::{ModelPlan, TaskFeatures};

pub const RULES_PROVIDER_ID: &str = "semaprax/rules-decision";
pub const RULES_CHECKPOINT: &str = "builtin";

/// Unknown cost (MR-01 descriptor) sorts last, never as a zero-cost winner.
fn cost(p: &ModelPlan) -> u64 {
    p.known_cost().unwrap_or(u64::MAX)
}

/// Hard families pick the strongest admissible plan (ties: cheaper, then id);
/// everything else picks the cheapest (ties: id). `None` iff nothing is
/// admissible.
pub fn rules_choice<'a>(
    features: &TaskFeatures,
    admissible: &'a [ModelPlan],
    policy: &RoutePolicy,
) -> Option<&'a ModelPlan> {
    if policy.hard_families.contains(&features.task_family) {
        admissible.iter().min_by(|a, b| {
            (b.strength_rank, cost(a), &a.id).cmp(&(a.strength_rank, cost(b), &b.id))
        })
    } else {
        admissible
            .iter()
            .min_by(|a, b| (cost(a), &a.id).cmp(&(cost(b), &b.id)))
    }
}
