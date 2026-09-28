//! Inert projection of the original actual State after complete failed observation.
//! This does not release State, manufacture Outcome, or grant a terminal ACK.
use super::*;
impl PendingOwnedEffectReceiptV8<'_> {
    pub(crate) fn live_observer_failed_state_facts_v8(
        &self,
    ) -> Result<serde_json::Value, SourceJournalError> {
        if self.failure.is_none()
            || self.receipt["kind"] != "observed"
            || self.receipt["settlement"] != "failed"
        {
            return Err(SourceJournalError::Binding);
        }
        self.released
            .observer_failed_state_facts_v8()
            .ok_or(SourceJournalError::Binding)
    }
}
