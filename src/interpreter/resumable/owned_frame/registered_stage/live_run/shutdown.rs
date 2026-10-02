//! Release only the original completed State behind its live Started ACK.
use super::*;
use crate::live_invocation::source_journal::{
    LiveCompletedStateShutdownPermitV8, SourceJournalError,
};
use serde_json::{json, Value as Json};
impl LiveResumedStateV8 {
    pub(crate) fn release_completed_state_v8(
        &mut self,
        permit: &LiveCompletedStateShutdownPermitV8<'_, '_>,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> Result<Json, SourceJournalError> {
        let state = self
            .checked_facts(permit.binding())
            .ok_or(SourceJournalError::Binding)?;
        if !self.transfer_ready(permit.binding(), permit.proposal()) {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_actual(&state)?;
        let actions = self.terminal.plan.liveness().result_disposal.clone();
        let Some(Value::Record(root)) = self.terminal.root.as_ref() else {
            return Err(SourceJournalError::Binding);
        };
        let leaves: Vec<_> = root
            .fields
            .iter()
            .filter_map(|(id, value)| matches!(value, Value::Bytes(_)).then_some(id))
            .collect();
        if Arc::strong_count(root) != 1
            || actions.len() != leaves.len()
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
        let mut outcomes = Vec::with_capacity(actions.len());
        permit.validate_current()?;
        self.terminal.provisional = false;
        drop(self.terminal.proposal.take());
        for action in &actions {
            permit.validate_current()?;
            let Some(Value::Record(root)) = self.terminal.root.as_mut() else {
                unreachable!()
            };
            drop(
                Arc::get_mut(root)
                    .expect("exclusive completed State")
                    .fields
                    .remove(&action.source.projections[0])
                    .expect("checked State leaf"),
            );
            outcomes.push(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_ok(),
            );
            permit.validate_current()?;
        }
        drop(self.terminal.root.take());
        Ok(
            json!({"kind":"observed","operations":operations.as_array().expect("canonical vector").iter().zip(&outcomes).map(|(operation,ok)|json!({"operation":operation,"outcome":if *ok {"completed"} else {"failed"}})).collect::<Vec<_>>(),"settlement":if outcomes.iter().all(|ok|*ok){"completed"}else{"failed"}}),
        )
    }
}
