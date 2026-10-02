//! The sole restartable Prepared tail rejoins runtime custody before Model.
use super::super::wait::{
    continue_recovered_first_turn_prepared_v8, recover_first_turn_prepared_owner_v8,
    FirstTurnPreparedContinuationHostGrantV8, FirstTurnPreparedRecoveryHostGrantV8,
};
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::wait::{
    recover_first_turn_transferred_state_v8, TransferredStateRecoveryHostGrantV8,
};

impl<'j> OwnedLifecycleRuntimeV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8::live_upstream) fn restore_first_transferred_state(
        journal: &'j SourceOwnedWaitJournalV8,
        recovery: TransferredStateRecoveryHostGrantV8,
        cancellation: &'j AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
    ) -> Result<
        super::super::authorize::StagedLiveOwnedRunV8<'j>,
        super::super::wait::TransferredStateRecoveryFailureV8<'j>,
    > {
        recover_first_turn_transferred_state_v8(journal, recovery, cancellation, clock)
    }

    /// Consume two independent one-use host grants into the same two-turn
    /// runtime as fresh execution. An error precedes continuation; once a
    /// physical owner enters Model, success and failure both stay in this slot.
    /// There is no caller-supplied State, replay budget, or replacement journal.
    pub(in crate::live_invocation::source_journal::owned_wait_v8::live_upstream) fn restart_first_prepared(
        journal: &'j SourceOwnedWaitJournalV8,
        recovery: FirstTurnPreparedRecoveryHostGrantV8,
        continuation: FirstTurnPreparedContinuationHostGrantV8,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        clock: &'j dyn SourceInvocationClock,
        cancellation: &'j AgentCancellation,
    ) -> Result<Self, SourceJournalError> {
        let context = journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        if !context.fold().cumulative_initialization || execution.ordinary().max_iterations() != 2 {
            return Err(SourceJournalError::Binding);
        }
        // The existing restoration seam admits only a read-only recovered
        // lease and its exact first Prepared tail. It refuses fresh, uncertain,
        // answered and terminal histories before any new model invocation.
        let owner = recover_first_turn_prepared_owner_v8(journal, recovery, cancellation)?;
        #[cfg(test)]
        let backings = owner.test_fresh_backings();
        let mut runtime = Self {
            custody: CustodyV8::InFlight,
            journal,
            cancellation,
            #[cfg(test)]
            backings,
        };
        runtime.custody =
            match continue_recovered_first_turn_prepared_v8(owner, continuation, adapter, clock) {
                Ok(owner) => CustodyV8::ModelCompleted(Box::new(owner)),
                Err(failure) => {
                    journal.quarantine();
                    CustodyV8::Restart(Box::new(failure))
                }
            };
        Ok(runtime)
    }
}
