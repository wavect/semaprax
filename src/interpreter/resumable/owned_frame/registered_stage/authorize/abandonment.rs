//! Disposal of the actual scalar-only Refused Decision and retained State.
use super::*;
use crate::live_invocation::source_journal::{LiveRefusedStateCleanupPermitV8, SourceJournalError};
use serde_json::{json, Value as Json};

impl StagedOwnedAuthorizeV2 {
    pub(crate) fn release_refused_state_v8(
        &mut self,
        permit: &LiveRefusedStateCleanupPermitV8<'_, '_>,
        mut observe: impl FnMut(&FinalizeAction),
    ) -> Result<Json, SourceJournalError> {
        let (state, decision) = self
            .live_staged_facts(permit.binding())
            .ok_or(SourceJournalError::Binding)?;
        permit.validate_actual(&state, &decision)?;
        // The admitted Refused case has one scalar code and no owning leaves.
        // Check the actual root and compiler disposal together before skipping it.
        let Some(Value::Variant(value)) = self.decision.as_ref() else {
            return Err(SourceJournalError::Binding);
        };
        if value.case != *self.plan.refused()
            || value.fields.values().any(|v| !matches!(v, Value::Int(_)))
            || self
                .plan
                .disposal()
                .iter()
                .any(|a| a.active_case.as_ref().is_none_or(|c| c.case == value.case))
        {
            return Err(SourceJournalError::Binding);
        }
        let actions = self.state.plan.liveness().result_disposal.clone();
        let Some(Value::Record(root)) = self.state.root.as_ref() else {
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
        let mut outcomes = Vec::with_capacity(actions.len());
        permit.validate_current()?;
        self.settlement_started = true;
        drop(self.decision.take());
        for action in &actions {
            permit.validate_current()?;
            let Some(Value::Record(root)) = self.state.root.as_mut() else {
                unreachable!()
            };
            drop(
                Arc::get_mut(root)
                    .expect("exclusive checked State")
                    .fields
                    .remove(&action.source.projections[0])
                    .expect("checked State leaf"),
            );
            outcomes.push(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(action))).is_ok(),
            );
            permit.validate_current()?;
        }
        drop(self.state.root.take());
        Ok(
            json!({"kind":"observed", "operations": operations.as_array().expect("canonical vector").iter().zip(&outcomes)
            .map(|(operation, ok)| json!({"operation":operation,"outcome":if *ok {"completed"} else {"failed"}})).collect::<Vec<_>>(),
            "settlement":if outcomes.iter().all(|ok|*ok) {"completed"} else {"failed"}}),
        )
    }
}
