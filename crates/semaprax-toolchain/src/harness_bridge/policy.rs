//! Frozen route plan -> the compiler's ordered deployment failover policy.

use semaprax::model_budget_policy::{ProviderPolicy, ProviderSlot};
use semaprax_harness::decision::FrozenRoutePlan;

/// Exact order and authorization flags of the frozen plan; slot 0 is primary.
/// `ProviderPolicy::admit_failover` then owns forward-only, authorized-only
/// failover admission; nothing is sorted or repaired here.
#[must_use]
pub fn provider_policy(plan: &FrozenRoutePlan) -> ProviderPolicy {
    ProviderPolicy::new(
        plan.to_provider_slots()
            .into_iter()
            .map(|(id, authorized)| ProviderSlot { id, authorized })
            .collect(),
    )
}
