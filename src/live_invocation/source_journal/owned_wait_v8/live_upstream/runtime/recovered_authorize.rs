//! Rejoin the ordinary public run only after exact transferred-State restore.
use super::wait::TransferredStateRecoveryHostGrantV8;
use super::*;

impl<'j> OwnedLifecycleRuntimeV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8::live_upstream) fn restart_first_transferred_state(
        journal: &'j SourceOwnedWaitJournalV8,
        recovery: TransferredStateRecoveryHostGrantV8,
        policy: &'j crate::resumable_effects::CapabilityPolicy,
        cancellation: &'j AgentCancellation,
        clock: &'j dyn SourceInvocationClock,
        adapter: &mut StreamingSourceProposalAdapter<'_>,
        handler: &mut dyn crate::agent_lifecycle::authorization::target_protocol::TargetHostHandler,
        observe: impl FnMut(&FinalizeAction),
    ) -> Result<Self, SourceJournalError> {
        let context = journal.context();
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        if !context.fold().cumulative_initialization || execution.ordinary().max_iterations() != 2 {
            return Err(SourceJournalError::Binding);
        }
        let mut runtime = Self {
            custody: CustodyV8::InFlight,
            journal,
            cancellation,
            #[cfg(test)]
            backings: Vec::new(),
        };
        let staged =
            match Self::restore_first_transferred_state(journal, recovery, cancellation, clock) {
                Ok(staged) => staged,
                Err(super::super::wait::TransferredStateRecoveryFailureV8::BeforeRestore(
                    error,
                )) => {
                    runtime.custody = CustodyV8::Ready;
                    return Err(error);
                }
                Err(super::super::wait::TransferredStateRecoveryFailureV8::Retained(failure)) => {
                    #[cfg(test)]
                    {
                        runtime.backings = failure.test_backings();
                    }
                    runtime.custody = CustodyV8::Run(Box::new(failure));
                    return Ok(runtime);
                }
            };
        #[cfg(test)]
        {
            runtime.backings = staged.owner.test_weak();
        }
        runtime.custody = match continue_run::finish_staged_run(
            journal, staged, policy, adapter, handler, observe,
        ) {
            Ok(continue_run::RunOutcomeV8::AuthorizationRefusedStopped(owner)) => {
                CustodyV8::AuthorizationRefusedStopped(owner)
            }
            Ok(continue_run::RunOutcomeV8::ContinuedFailedEffectStopped(owner)) => {
                CustodyV8::ContinuedFailedEffectStopped(owner)
            }
            Ok(continue_run::RunOutcomeV8::Complete(projection)) => CustodyV8::Complete(projection),
            Ok(continue_run::RunOutcomeV8::FailedObserve(owner)) => {
                CustodyV8::ObserveCleanupPending(Box::new(owner))
            }
            Ok(continue_run::RunOutcomeV8::FailedEffect(owner)) => {
                if owner.observer_cleanup_pending() {
                    CustodyV8::ObserverFailureCleanupPending(Box::new(owner))
                } else {
                    CustodyV8::FailedEffectCleanupPending(Box::new(owner))
                }
            }
            Err(failure) => CustodyV8::Run(Box::new(failure)),
        };
        Ok(runtime)
    }
}
