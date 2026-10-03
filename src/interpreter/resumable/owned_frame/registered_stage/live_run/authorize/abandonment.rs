//! The live holder retains an interrupted Refused cleanup in place.
use super::*;
impl LiveStagedAuthorizationV8 {
    pub(crate) fn release_refused_state_v8(
        &mut self,
        permit: &crate::live_invocation::source_journal::LiveRefusedStateCleanupPermitV8<'_, '_>,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<serde_json::Value, crate::live_invocation::source_journal::SourceJournalError> {
        self.staged.release_refused_state_v8(permit, observe)
    }
}
