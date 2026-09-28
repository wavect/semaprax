//! Borrow-only facts from an actual completely Decision-released failed observer.
//! Partial release and successful observer receipts cannot enter this profile.
use super::*;
impl OwnedEffectDecisionReleaseV8 {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn observer_failed_state_facts_v8(
        &self,
    ) -> Option<serde_json::Value> {
        let staged = &self.holder.ready.staged;
        let state = &staged.state;
        let root = state.root.as_ref()?;
        if self.observations_succeeded
            || !self.holder.release_started
            || staged.decision.is_some()
            || state.creator != std::process::id()
            || !root_valid(&state.plan, root)
            || !state.allocations.validate(&[root])
        {
            return None;
        }
        super::super::live_run::root_facts(&state.plan, root)
    }
}
