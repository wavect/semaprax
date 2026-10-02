//! Closed consuming later-turn failed-Observe bridge.
//! Journal facts cannot recreate this lineage or its failed State owner.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::{
    FailedObserveCacheV8, FailedObserveContextV8, FailedObserveOwnerV8, FailedObserveSourceV8,
};
use crate::resumable_effects::capability::CapabilityPolicy;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LaterFailedObserveContextV8<'j>
{
    lineage: LaterContinueLineageV8<'j>,
}

impl<'j> LaterObserveSettlementV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_failed_state_cleanup(
        self,
        cache: FailedObserveCacheV8<'j>,
    ) -> Result<FailedObserveSourceV8<'j>, (Self, FailedObserveCacheV8<'j>)> {
        if !matches!(self.owner.outcome, ContinuedOwnedObserveV2::Failed(_)) {
            return Err((self, cache));
        }
        let LiveLaterObservedContinueV8 {
            outcome,
            accounting,
            lineage,
            consumed,
        } = self.owner;
        let ContinuedOwnedObserveV2::Failed(failed) = outcome else {
            unreachable!("closed actual variant")
        };
        if failed.consumed() != consumed || lineage.acks.len() != 2 || lineage.turn != cache.turn()
        {
            return Err((
                Self {
                    owner: LiveLaterObservedContinueV8 {
                        outcome: ContinuedOwnedObserveV2::Failed(failed),
                        accounting,
                        lineage,
                        consumed,
                    },
                },
                cache,
            ));
        }
        Ok(FailedObserveSourceV8 {
            owner: FailedObserveOwnerV8::Continued { failed, accounting },
            context: FailedObserveContextV8::Later(LaterFailedObserveContextV8 { lineage }),
            cache,
            acks: Vec::new(),
        })
    }
}

impl<'j> LaterFailedObserveContextV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.lineage.journal
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn guard_at(
        &self,
        seq: usize,
        bytes: usize,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let journal = self.journal();
        let result = (|| {
            if self.lineage.acks.len() != 2 {
                return Err(SourceJournalError::Order);
            }
            self.lineage
                .hold()?
                .validate_failed_observe_cleanup_guard(journal, seq, bytes)?;
            let held = journal.hold()?;
            held.validate_prefix(seq, bytes)?;
            let (runtime, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = plan_owned_effect_v8(
                runtime,
                execution,
                &held.registration().expected_facts().scope,
                &self.lineage.proposal,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            if !self.lineage.policy.allows(plan.operation().effect_id()) {
                return Err(SourceJournalError::Binding);
            }
            if !incurred {
                let ordinary = journal.context().ordinary();
                check_clock_v8(
                    &held,
                    seq,
                    bytes,
                    self.lineage.cancellation,
                    self.lineage.source.continued_model_origin()?.1,
                    ordinary.clock_domain(),
                    ordinary.initial_millis(),
                    ordinary.deadline_millis(),
                )?;
            }
            held.validate_prefix(seq, bytes)?;
            self.lineage
                .hold()?
                .validate_failed_observe_cleanup_guard(journal, seq, bytes)
        })();
        result.inspect_err(|_| journal.quarantine())
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn matches_context(
        &self,
        runtime: &crate::execution_revision::typed::AgentRuntimeV2,
        execution: &crate::execution_revision::typed::CheckedTypedOwnedWaitExecutionV8,
        store: &HeldOwnedWaitStoreV8<'_>,
        policy: &CapabilityPolicy,
    ) -> Result<(), SourceJournalError> {
        let journal = self.journal();
        let (expected_runtime, expected_execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if !std::ptr::eq(runtime, expected_runtime)
            || !std::ptr::eq(execution, expected_execution)
            || !store.belongs_to(journal)
            || !std::ptr::eq(policy, self.lineage.policy)
            || self.lineage.acks.len() != 2
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn append_prefix(
        &self,
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .validate_failed_observe_cleanup_append_prefix(self.journal(), inventory, selected)
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_ack(
        &self,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedFailedObserveStateSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.lineage
            .hold()?
            .advance_failed_observe_cleanup_ack(witness, session)
    }
}
