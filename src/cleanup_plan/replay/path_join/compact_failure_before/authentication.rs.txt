//! Warm authentication publishes only into the caller's already paid slot.
use super::*;
impl ReplayBudget {
    pub(in crate::cleanup_plan::replay) fn authenticate_zero_into(
        &mut self,
        function: &ResolvedFunction,
        phase: &str,
        refusal: &mut Option<Diagnostic>,
    ) {
        if refusal.is_some() {
            return;
        }
        match self.payer {
            AllocationPayer::LegacyUnobserved => return,
            AllocationPayer::ActiveBuilder => {
                if self.refusal_spent || crate::bounded_output::active_refused() {
                    *refusal =
                        Some(self.budget_refusal(function, "ordinary builder already failed", ""));
                    return;
                }
                if crate::bounded_output::reserve_active_required(0) {
                    return;
                }
            }
            AllocationPayer::SelectedReservation => {
                if self
                    .physical
                    .as_deref_mut()
                    .expect("selected reservation payer")
                    .authenticate_zero()
                {
                    return;
                }
            }
            #[cfg(test)]
            AllocationPayer::Oracle(lease) => {
                if self.refusal_spent || lease.refused_zero() {
                    *refusal =
                        Some(self.budget_refusal(function, "ordinary builder already failed", ""));
                    return;
                }
                if lease.authenticate_zero() {
                    return;
                }
            }
        }
        *refusal = Some(self.budget_refusal(
            function,
            "cleanup replay physical reservation exhausted during ",
            phase,
        ));
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod source;
#[cfg(test)]
pub(in crate::cleanup_plan::replay) use source::{previous, previous_include};
