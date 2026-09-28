//! Borrow-only facts from an actual completely Decision-released failed observer.
//! Partial release and successful observer receipts cannot enter this profile.
use super::*;
use crate::live_invocation::source_journal::{
    LiveObserverFailedStateCleanupPermitV8, SourceJournalError,
};
use serde_json::{json, Value as Json};
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

/// Actual Decision-released holder remains first, including interrupted State.
pub(in crate::interpreter::resumable::owned_frame::registered_stage) struct ObserverFailedStateReleaseV8
{
    _owner: OwnedEffectDecisionReleaseV8,
    receipt: Json,
}
pub(in crate::interpreter::resumable::owned_frame::registered_stage) struct ObserverFailedStateReleaseRejectionV8
{
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) owner:
        OwnedEffectDecisionReleaseV8,
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) error: SourceJournalError,
}
impl ObserverFailedStateReleaseV8 {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn receipt(
        &self,
    ) -> &Json {
        &self.receipt
    }
}

impl OwnedEffectDecisionReleaseV8 {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage) fn release_observer_failed_state_v8(
        mut self,
        permit: &LiveObserverFailedStateCleanupPermitV8<'_, '_>,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> Result<ObserverFailedStateReleaseV8, ObserverFailedStateReleaseRejectionV8> {
        let prepared = (|| {
            let facts = self
                .observer_failed_state_facts_v8()
                .ok_or(SourceJournalError::Binding)?;
            let state = &self.holder.ready.staged.state;
            permit.validate_actual_state(&state.plan, &facts)?;
            permit.validate_cleanup_current()?;
            let actions = state.plan.liveness().result_disposal.clone();
            let Value::Record(root) = state.root.as_ref().ok_or(SourceJournalError::Binding)?
            else {
                return Err(SourceJournalError::Binding);
            };
            let leaves: Vec<_> = root
                .fields
                .iter()
                .filter_map(|(id, v)| matches!(v, Value::Bytes(_)).then_some(id))
                .collect();
            if actions.len() != leaves.len()
                || actions.iter().any(|a| {
                    a.active_case.is_some()
                        || a.source.projections.len() != 1
                        || !leaves.contains(&&a.source.projections[0])
                })
            {
                return Err(SourceJournalError::Binding);
            }
            let operations =
                crate::resumable_effects::owned_frame::v2::owned_wait_operations_v8(&actions)
                    .map_err(|_| SourceJournalError::Binding)?;
            Ok((actions, operations))
        })();
        let (actions, operations) = match prepared {
            Ok(a) => a,
            Err(error) => return Err(ObserverFailedStateReleaseRejectionV8 { owner: self, error }),
        };
        // Storage is bounded/allocated before any physical leaf removal. Every
        // compiler State action is active; no skipped operation is invented.
        let mut outcomes = Vec::with_capacity(actions.len());
        for action in &actions {
            if let Err(error) = permit.validate_cleanup_current() {
                return Err(ObserverFailedStateReleaseRejectionV8 { owner: self, error });
            }
            let state = &mut self.holder.ready.staged.state;
            let Some(Value::Record(record)) = state.root.as_mut() else {
                unreachable!("checked actual State")
            };
            let value = Arc::get_mut(record)
                .expect("validated exclusive State")
                .fields
                .remove(&action.source.projections[0])
                .expect("validated actual State leaf");
            drop(value);
            // Authority checks remain outside the observer unwind boundary.
            let completed =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_ok();
            outcomes.push(completed);
            if let Err(error) = permit.validate_cleanup_current() {
                return Err(ObserverFailedStateReleaseRejectionV8 { owner: self, error });
            }
        }
        drop(self.holder.ready.staged.state.root.take());
        let receipt = json!({"kind":"observed",
            "operations":operations.as_array().expect("checked vector").iter().zip(&outcomes)
                .map(|(operation,completed)|json!({"operation":operation,"outcome":if *completed{"completed"}else{"failed"}})).collect::<Vec<_>>(),
            "settlement":if outcomes.iter().all(|v|*v){"completed"}else{"failed"}});
        Ok(ObserverFailedStateReleaseV8 {
            _owner: self,
            receipt,
        })
    }
}
