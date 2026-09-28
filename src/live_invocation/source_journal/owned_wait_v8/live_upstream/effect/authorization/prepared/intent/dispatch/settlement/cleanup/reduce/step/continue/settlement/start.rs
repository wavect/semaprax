//! The physical continued State enters its existing helper only under the real
//! original Start ACK; its old observation/target ledger are never re-admitted.
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    begin_live_continued_wait_v8, LiveContinuedWaitStartOutcomeV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    prepare_continued_copy_wait_v8, ContinuedWaitPreparationFailureV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveWaitStartPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedStartedWaitV8<'j> {
    outcome: LiveContinuedWaitStartOutcomeV8<'j>,
    accounting: TargetAccounting,
    lineage: ContinueLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum ContinuedStartEntryFailureV8<'j>
{
    Before {
        owner: ContinuedObserveSettlementV8<'j>,
        error: SourceJournalError,
    },
    Preparation {
        owner: ContinuedWaitPreparationFailureV8<'j>,
        accounting: TargetAccounting,
        lineage: ContinueLineageV8<'j>,
        error: SourceJournalError,
    },
    After {
        owner: ContinuedStartedWaitV8<'j>,
        error: SourceJournalError,
    },
}
fn guard_start(
    lineage: &ContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal();
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        let origin = lineage.step.origin();
        origin.hold.validate_continued_start_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let held = journal.hold()?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        runtime
            .owned_wait_effects_v8(execution)
            .map_err(|_| SourceJournalError::Binding)?;
        let plan = plan_owned_effect_v8(
            runtime,
            execution,
            &held.registration().expected_facts().scope,
            &origin.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !origin.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let ordinary = journal.context().ordinary();
        check_clock_v8(
            &held,
            session.sequence(),
            session.acknowledged_bytes(),
            origin.cancellation,
            origin.clock,
            ordinary.clock_domain(),
            ordinary.initial_millis(),
            ordinary.deadline_millis(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}
impl<'j> ContinuedObserveSettlementV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn enter_actual_wait(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'j>,
    ) -> Result<ContinuedStartedWaitV8<'j>, ContinuedStartEntryFailureV8<'j>> {
        let checked = (|| {
            guard_start(&self.owner.lineage, session, witness)?;
            self.data()?;
            let execution = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            if !matches!(witness.selected_row(),EntryV8::Owned(crate::live_invocation::source_journal::owned_wait_v8::model::OwnedBodyV8::OwnedWaitReserved{turn,attempt:0,phase:crate::live_invocation::source_journal::owned_wait_v8::model::PhaseV8::Start,replay_of:None,fuel,..})if *turn==self.turn()&&*fuel==execution.evaluation_fuel() as u64)
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(execution.evaluation_fuel())
        })();
        let fuel = match checked {
            Ok(f) => f,
            Err(error) => return Err(ContinuedStartEntryFailureV8::Before { owner: self, error }),
        };
        let LiveObservedContinueV8 {
            outcome,
            accounting,
            lineage,
            ..
        } = self.owner;
        let prepared = match prepare_continued_copy_wait_v8(outcome) {
            Ok(owner) => owner,
            Err(owner) => {
                return Err(ContinuedStartEntryFailureV8::Preparation {
                    owner,
                    accounting,
                    lineage,
                    error: SourceJournalError::Binding,
                })
            }
        };
        if let Err(error) = guard_start(&lineage, session, witness) {
            return Err(ContinuedStartEntryFailureV8::Preparation {
                owner: ContinuedWaitPreparationFailureV8::After {
                    owner: prepared,
                    error,
                },
                accounting,
                lineage,
                error,
            });
        }
        let cancellation = lineage.step.origin().cancellation;
        let held = match lineage.journal().hold() {
            Ok(x) => x,
            Err(error) => {
                return Err(ContinuedStartEntryFailureV8::Preparation {
                    owner: ContinuedWaitPreparationFailureV8::After {
                        owner: prepared,
                        error,
                    },
                    accounting,
                    lineage,
                    error,
                })
            }
        };
        let permit = LiveWaitStartPermitV8 {
            held,
            fuel,
            cancellation,
        };
        let outcome = begin_live_continued_wait_v8(permit, prepared);
        let actual = ContinuedStartedWaitV8 {
            outcome,
            accounting,
            lineage,
        };
        if let Err(error) = actual.validate_live(session, witness) {
            return Err(ContinuedStartEntryFailureV8::After {
                owner: actual,
                error,
            });
        }
        Ok(actual)
    }
}
impl ContinuedStartedWaitV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            guard_start(&self.lineage, session, witness)?;
            let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
                return Err(SourceJournalError::Binding);
            };
            let execution = self
                .lineage
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            owner
                .checked_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.lineage.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn consumed(
        &self,
    ) -> Option<u64> {
        match &self.outcome {
            LiveContinuedWaitStartOutcomeV8::Parked(o) => Some(o.consumed()),
            LiveContinuedWaitStartOutcomeV8::GuardLost { owner, .. } => Some(owner.consumed()),
            LiveContinuedWaitStartOutcomeV8::Terminal(owner) => Some(owner.consumed()),
            LiveContinuedWaitStartOutcomeV8::Refused(_) => None,
        }
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
}
