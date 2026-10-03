//! Materialize one State with a consuming permit; failed guards retain custody.
use super::*;
use crate::live_invocation::source_journal::{
    LiveRecoveredStateTransferPermitV8, SourceJournalError,
};

pub(crate) struct LiveTransferredRestoreFailureV8 {
    pub(crate) error: SourceJournalError,
    owner: Option<OwnedAgentStateArgument>,
}
impl Drop for LiveTransferredRestoreFailureV8 {
    fn drop(&mut self) {
        // Forced host destruction cannot invoke an unacknowledged semantic disposer.
        if let Some(owner) = self.owner.as_mut() {
            drop(owner.root.take());
        }
    }
}
pub(crate) fn restore_transferred_state_v8(
    permit: LiveRecoveredStateTransferPermitV8<'_, '_>,
    input: OwnedFrameInput,
) -> Result<LiveTransferredStateV8, LiveTransferredRestoreFailureV8> {
    permit
        .validate_guard()
        .map_err(|error| LiveTransferredRestoreFailureV8 { error, owner: None })?;
    let binding = permit.binding();
    let mut admitted = admit_owned_agent_state_input(binding.helper(), input).map_err(|_| {
        LiveTransferredRestoreFailureV8 {
            error: SourceJournalError::Binding,
            owner: None,
        }
    })?;
    let valid = admitted.root.as_ref().is_some_and(|root| {
        admitted.creator == std::process::id()
            && admitted
                .allocations
                .as_ref()
                .is_some_and(|a| a.validate(&[root]))
            && root_facts(&admitted.plan, root).as_ref() == Some(permit.state())
    });
    if !valid {
        return Err(LiveTransferredRestoreFailureV8 {
            error: SourceJournalError::Binding,
            owner: Some(admitted),
        });
    }
    if let Err(error) = permit.validate_guard() {
        return Err(LiveTransferredRestoreFailureV8 {
            error,
            owner: Some(admitted),
        });
    }
    let Some(allocations) = admitted.allocations.take() else {
        return Err(LiveTransferredRestoreFailureV8 {
            error: SourceJournalError::Binding,
            owner: Some(admitted),
        });
    };
    let state = CompletedOwnedAgentStateV2 {
        allocations,
        plan: admitted.plan.clone(),
        root: admitted.root.take(),
        proposal: permit.proposal().carrier().clone(),
        creator: admitted.creator,
    };
    Ok(LiveTransferredStateV8 { state })
}
