//! The builtin rules decision provider: project policy, no model call.

use super::policy::RoutePolicy;
use super::route::{ModelPlan, TaskFeatures};

pub const RULES_PROVIDER_ID: &str = "semaprax/rules-decision";
pub const RULES_CHECKPOINT: &str = "builtin";

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
            (b.strength_rank, a.est_cost_micros, &a.id).cmp(&(
                a.strength_rank,
                b.est_cost_micros,
                &b.id,
            ))
        })
    } else {
        admissible
            .iter()
            .min_by(|a, b| (a.est_cost_micros, &a.id).cmp(&(b.est_cost_micros, &b.id)))
    }
}
