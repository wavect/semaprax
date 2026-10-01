//! Sole later-turn source entry from two exact Start ACKs and the moved State.
pub(in crate::live_invocation::source_journal::owned_wait_v8) mod prepared;
use super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::live_run::{
    begin_live_continued_wait_v8, LiveContinuedWaitStartOutcomeV8,
};
use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    prepare_continued_copy_wait_v8, ContinuedWaitPreparationFailureV8,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::VerifiedOwnedContinuedStartSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::LiveWaitStartPermitV8;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LaterStartedWaitV8<'j> {
    outcome: LiveContinuedWaitStartOutcomeV8<'j>,
    accounting: TargetAccounting,
    lineage: LaterContinueLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LaterStartEntryFailureV8<'j> {
    Before {
        owner: LaterObserveSettlementV8<'j>,
        error: SourceJournalError,
    },
    Preparation {
        owner: ContinuedWaitPreparationFailureV8<'j>,
        accounting: TargetAccounting,
        lineage: LaterContinueLineageV8<'j>,
        error: SourceJournalError,
    },
    After {
        owner: LaterStartedWaitV8<'j>,
        error: SourceJournalError,
    },
}

fn guard_start(
    lineage: &LaterContinueLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedContinuedStartSuccessorV8<'_>,
) -> Result<(), SourceJournalError> {
    let journal = lineage.journal;
    let result = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_current_session(session)?;
        lineage.hold()?.validate_continued_start_guard(
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
            &lineage.proposal,
        )
        .map_err(|_| SourceJournalError::Binding)?;
        if !lineage.policy.allows(plan.operation().effect_id()) {
            return Err(SourceJournalError::Binding);
        }
        let ordinary = journal.context().ordinary();
        check_clock_v8(
            &held,
            session.sequence(),
            session.acknowledged_bytes(),
            lineage.cancellation,
            lineage.source.continued_model_origin()?.1,
            ordinary.clock_domain(),
            ordinary.initial_millis(),
            ordinary.deadline_millis(),
        )?;
        witness.validate_current_session(session)
    })();
    result.inspect_err(|_| journal.quarantine())
}

impl<'j> LaterObserveSettlementV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn enter_actual_wait(
        self,
        session: &AppendSessionV8<'j>,
        witness: &VerifiedOwnedContinuedStartSuccessorV8<'j>,
    ) -> Result<LaterStartedWaitV8<'j>, LaterStartEntryFailureV8<'j>> {
        let checked = (|| {
            guard_start(&self.owner.lineage, session, witness)?;
            self.data()?;
            let execution = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?
                .1;
            if !matches!(witness.selected_row(), EntryV8::Owned(
                journal_model::OwnedBodyV8::OwnedWaitReserved {
                    turn, attempt: 0, phase: journal_model::PhaseV8::Start,
                    replay_of: None, fuel, ..
                }) if *turn == self.turn() && *fuel == execution.evaluation_fuel() as u64)
            {
                return Err(SourceJournalError::Binding);
            }
            Ok(execution.evaluation_fuel())
        })();
        let fuel = match checked {
            Ok(fuel) => fuel,
            Err(error) => return Err(LaterStartEntryFailureV8::Before { owner: self, error }),
        };
        let LiveLaterObservedContinueV8 {
            outcome,
            accounting,
            lineage,
            ..
        } = self.owner;
        let prepared = match prepare_continued_copy_wait_v8(outcome) {
            Ok(owner) => owner,
            Err(owner) => {
                return Err(LaterStartEntryFailureV8::Preparation {
                    owner,
                    accounting,
                    lineage,
                    error: SourceJournalError::Binding,
                })
            }
        };
        if let Err(error) = guard_start(&lineage, session, witness) {
            return Err(LaterStartEntryFailureV8::Preparation {
                owner: ContinuedWaitPreparationFailureV8::After {
                    owner: prepared,
                    error,
                },
                accounting,
                lineage,
                error,
            });
        }
        let held = match lineage.journal.hold() {
            Ok(held) => held,
            Err(error) => {
                return Err(LaterStartEntryFailureV8::Preparation {
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
            cancellation: lineage.cancellation,
        };
        let outcome = begin_live_continued_wait_v8(permit, prepared);
        let actual = LaterStartedWaitV8 {
            outcome,
            accounting,
            lineage,
        };
        if let Err(error) = actual.validate_live(session, witness) {
            return Err(LaterStartEntryFailureV8::After {
                owner: actual,
                error,
            });
        }
        Ok(actual)
    }
}
impl LaterStartedWaitV8<'_> {
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
            let (_, execution) = self
                .lineage
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            owner
                .checked_facts(execution.wait())
                .ok_or(SourceJournalError::Binding)?;
            witness.validate_current_session(session)
        })();
        result.inspect_err(|_| self.lineage.journal.quarantine())
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
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_ordinary_start(
        &self,
    ) -> (
        serde_json::Value,
        crate::interpreter::resumable::ResumableChannelValue,
        usize,
    ) {
        let LiveContinuedWaitStartOutcomeV8::Parked(owner) = &self.outcome else {
            panic!("actual parked source")
        };
        owner.test_ordinary_start()
    }
    #[cfg(test)]
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }
}
