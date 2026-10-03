//! Consuming successful physical Outcome to the original Reduce reservation.
//! Selection is inert until the fixed same-hold adapter acknowledges this row.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::TargetEvidence;
use crate::live_invocation::source_journal::SourceStageRole;
use crate::resumable_effects::owned_frame::v2::{compile_owned_reduce_v2, CheckedOwnedReduceV2};

/// The actual Executed owner is first; its ledger and SAME held store remain
/// inside it. No facts/JSON admission or recovered-prefix constructor exists.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveOwnedReduceReservationAppendV8<
    'j,
> {
    owner: LiveExecutedOwnedEffectV8<'j>,
    plan: CheckedOwnedReduceV2,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveReduceReservationRejectionV8<
    'j,
> {
    _owner: LiveExecutedOwnedEffectV8<'j>,
    error: SourceJournalError,
}
impl<'j> LiveExecutedOwnedEffectV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_reduce(
        self,
    ) -> Result<LiveOwnedReduceReservationAppendV8<'j>, LiveReduceReservationRejectionV8<'j>> {
        let selected = (|| {
            self.validate_live()?;
            let origin = &self.lineage.recorded.intent;
            let journal = origin.journal;
            let (_, execution) = journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let plan = compile_owned_reduce_v2(execution.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            let fuel = execution.evaluation_fuel();
            if Some(fuel) != journal.context().ordinary().max_steps_per_stage()
                || plan.binding() != execution.wait().binding()
                || !plan.helper().same_helper(execution.wait().helper())
            {
                return Err(SourceJournalError::Binding);
            }
            // The actual ledger was moved through dispatch and every ACK. The
            // authenticated target evidence is comparison data, not its source.
            let evidence = TargetEvidence::decode(self.lineage.recorded.facts.evidence())
                .map_err(|_| SourceJournalError::Binding)?;
            if evidence.accounting() != self.accounting
                || !matches!(
                    self.lineage.recorded.facts.ordinary(),
                    SourceJournalEntry::EffectObserved {
                        turn: 0,
                        attempt: 0,
                        ..
                    }
                )
                || self.lineage.settled.is_none()
            {
                return Err(SourceJournalError::Binding);
            }
            let selected = EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: 0,
                attempt: Some(0),
                role: SourceStageRole::Reduce,
                fuel,
            });
            self.validate_live()?;
            Ok((plan, selected))
        })();
        match selected {
            Ok((plan, selected)) => Ok(LiveOwnedReduceReservationAppendV8 {
                owner: self,
                plan,
                selected,
            }),
            Err(error) => {
                self.lineage.recorded.intent.journal.quarantine();
                Err(LiveReduceReservationRejectionV8 {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
impl<'j> LiveOwnedReduceReservationAppendV8<'j> {
    fn journal(&self) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.lineage.recorded.intent.journal
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn belongs_to(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> bool {
        std::ptr::eq(self.journal(), journal)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn sequence(&self) -> usize {
        self.owner.lineage.current().session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.lineage.current().session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.owner.validate_live()?;
            let (_, execution) = self
                .journal()
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            if self.plan.binding() != execution.wait().binding()
                || !self.plan.helper().same_helper(execution.wait().helper())
                || self.selected
                    != EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                        turn: 0,
                        attempt: Some(0),
                        role: SourceStageRole::Reduce,
                        fuel: execution.evaluation_fuel(),
                    })
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner.validate_live()
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
}

#[cfg(all(test, unix))]
mod tests;

use crate::interpreter::resumable::owned_frame::registered_stage::reduce::{
    evaluate_live_executed_owned_reduce_v2, CheckedLiveOwnedReduceStageFactsV8,
    LiveReduceEvaluationFailureV8, StagedExecutedOwnedReduceV2,
};
use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedReduceReservationSuccessorV8;
use crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8;

/// Borrowed only from the actual owner-containing reservation obligation.
/// Full guards run outside the append marker; prefix checks contain no callbacks.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedReduceReservationAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveOwnedReduceReservationAppendV8<'j>,
}
impl<'j> LiveOwnedReduceReservationAppendV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedReduceReservationAppendPermitV8<'_, 'j>, SourceJournalError> {
        self.validate_live()?;
        Ok(FixedOwnedReduceReservationAppendPermitV8 { owner: self })
    }
}
impl FixedOwnedReduceReservationAppendPermitV8<'_, '_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected_row(
        &self,
    ) -> &EntryV8 {
        self.owner.selected_row()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_preflight(
        &self,
        journal: &SourceOwnedWaitJournalV8,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        self.owner.validate_live()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_selected_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        inventory: &InventoryV8<'_>,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner
            .owner
            .lineage
            .recorded
            .intent
            .hold
            .validate_reduce_append_prefix(journal, inventory, self.owner.selected_row())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &VerifiedOwnedReduceReservationSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .owner
            .lineage
            .recorded
            .intent
            .hold
            .advance_reduce_ack(witness, session)
    }
}
/// Old successful cleanup is inert lineage after the original Reduce ACK.
/// The SAME hold stays here and its new spent phase is the only fresh guard.
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ReduceLineageV8<'j> {
    cleanup: CleanupLineageV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedReduceReservationSuccessorV8<'j>,
    selected: EntryV8,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
}
impl ReduceLineageV8<'_> {
    fn validate_current(&self) -> Result<(), SourceJournalError> {
        reduce_current(
            &self.cleanup.recorded.intent,
            &self.session,
            &self.witness,
            self.predecessor_sequence,
            self.predecessor_bytes,
            &self.selected,
        )
    }
}
fn reduce_current(
    origin: &super::super::super::super::activation::IntentLineageV8<'_>,
    session: &AppendSessionV8<'_>,
    witness: &VerifiedOwnedReduceReservationSuccessorV8<'_>,
    predecessor_sequence: usize,
    predecessor_bytes: usize,
    selected: &EntryV8,
) -> Result<(), SourceJournalError> {
    let result = (|| {
        let journal = origin.journal;
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_predecessor(journal, predecessor_sequence, predecessor_bytes, selected)?;
        witness.validate_current_session(session)?;
        origin.hold.validate_spent_reduce_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let held = journal.hold()?;
        let (runtime, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
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
        witness.validate_current_session(session)?;
        origin.hold.validate_spent_reduce_guard(
            journal,
            session.sequence(),
            session.acknowledged_bytes(),
        )
    })();
    result.inspect_err(|_| origin.journal.quarantine())
}
impl LiveOwnedReduceReservationAppendV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_reduce_successor(
        &self,
        witness: &VerifiedOwnedReduceReservationSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        reduce_current(
            &self.owner.lineage.recorded.intent,
            session,
            witness,
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )
    }
}
/// Constructor is confined to the consuming actual ACK envelope below.
/// No decoded history, caller fuel/closure or raw owner can construct this.
pub(crate) struct LiveReduceEvaluationPermitV8<'p, 'j> {
    lineage: &'p ReduceLineageV8<'j>,
}
impl LiveReduceEvaluationPermitV8<'_, '_> {
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.lineage.validate_current()
    }
    pub(crate) fn fuel(&self) -> Result<usize, SourceJournalError> {
        let journal = self.lineage.cleanup.recorded.intent.journal;
        let (_, e) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let EntryV8::Ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: Some(0),
            role: SourceStageRole::Reduce,
            fuel,
        }) = &self.lineage.selected
        else {
            return Err(SourceJournalError::Binding);
        };
        if *fuel != e.evaluation_fuel() {
            return Err(SourceJournalError::Binding);
        }
        Ok(*fuel)
    }
    pub(crate) fn validate_plan(
        &self,
        plan: &CheckedOwnedReduceV2,
    ) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        let (_, e) = self
            .lineage
            .cleanup
            .recorded
            .intent
            .journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if plan.binding() != e.wait().binding() || !plan.helper().same_helper(e.wait().helper()) {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
impl crate::interpreter::resumable::owned_frame::registered_stage::reduce::LiveReduceEvaluationGuardV8
    for LiveReduceEvaluationPermitV8<'_, '_>
{
    fn validate_current(&self) -> Result<(), SourceJournalError> {
        LiveReduceEvaluationPermitV8::validate_current(self)
    }
    fn fuel(&self) -> Result<usize, SourceJournalError> {
        LiveReduceEvaluationPermitV8::fuel(self)
    }
    fn validate_plan(&self, plan: &CheckedOwnedReduceV2) -> Result<(), SourceJournalError> {
        LiveReduceEvaluationPermitV8::validate_plan(self, plan)
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveEvaluatedOwnedReduceV8<'j>
{
    staged: StagedExecutedOwnedReduceV2<'j>,
    accounting: TargetAccounting,
    lineage: ReduceLineageV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveReduceAdvanceFailureV8<'j> {
    Before {
        _owner: LiveOwnedReduceReservationAppendV8<'j>,
        _session: AppendSessionV8<'j>,
        _witness: VerifiedOwnedReduceReservationSuccessorV8<'j>,
        error: SourceJournalError,
    },
    Evaluation {
        _owner: LiveReduceEvaluationFailureV8<'j>,
        _accounting: TargetAccounting,
        _lineage: ReduceLineageV8<'j>,
    },
    After {
        _owner: LiveEvaluatedOwnedReduceV8<'j>,
        error: SourceJournalError,
    },
}
impl LiveEvaluatedOwnedReduceV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        &self.accounting
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.lineage.validate_current()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn stage_facts(
        &self,
    ) -> Result<CheckedLiveOwnedReduceStageFactsV8, SourceJournalError> {
        let result = (|| {
            self.validate_live()?;
            let (_, e) = self
                .lineage
                .cleanup
                .recorded
                .intent
                .journal
                .context()
                .ready_runtime()
                .ok_or(SourceJournalError::Binding)?;
            let facts = self
                .staged
                .live_stage_facts(e.wait())
                .map_err(|_| SourceJournalError::Binding)?;
            self.validate_live()?;
            Ok(facts)
        })();
        result.inspect_err(|_| self.lineage.cleanup.recorded.intent.journal.quarantine())
    }
}
/// Only the fixed adapter consumes its private unchanged obligation + real ACK
/// here. The owner is never recreated from the selected row or Step facts.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_reduce_v8<'j>(
    owner: LiveOwnedReduceReservationAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedReduceReservationSuccessorV8<'j>,
) -> Result<LiveEvaluatedOwnedReduceV8<'j>, LiveReduceAdvanceFailureV8<'j>> {
    let predecessor_sequence = owner.sequence();
    let predecessor_bytes = owner.acknowledged_bytes();
    let journal = owner.journal();
    let valid = (|| {
        if !session.belongs_to(journal) {
            return Err(SourceJournalError::Binding);
        }
        witness.validate_predecessor(
            journal,
            predecessor_sequence,
            predecessor_bytes,
            &owner.selected,
        )?;
        witness.validate_current_session(&session)?;
        owner
            .owner
            .lineage
            .recorded
            .intent
            .hold
            .validate_spent_reduce_guard(journal, session.sequence(), session.acknowledged_bytes())
    })();
    if let Err(error) = valid {
        journal.quarantine();
        return Err(LiveReduceAdvanceFailureV8::Before {
            _owner: owner,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let LiveOwnedReduceReservationAppendV8 {
        owner,
        plan,
        selected,
    } = owner;
    let LiveExecutedOwnedEffectV8 {
        executed,
        accounting,
        lineage: cleanup,
    } = owner;
    let lineage = ReduceLineageV8 {
        cleanup,
        session,
        witness,
        selected,
        predecessor_sequence,
        predecessor_bytes,
    };
    let permit = LiveReduceEvaluationPermitV8 { lineage: &lineage };
    let staged = match evaluate_live_executed_owned_reduce_v2(executed, &plan, &permit) {
        Ok(staged) => staged,
        Err(error) => {
            journal.quarantine();
            return Err(LiveReduceAdvanceFailureV8::Evaluation {
                _owner: error,
                _accounting: accounting,
                _lineage: lineage,
            });
        }
    };
    let actual = LiveEvaluatedOwnedReduceV8 {
        staged,
        accounting,
        lineage,
    };
    if let Err(error) = actual.validate_live() {
        return Err(LiveReduceAdvanceFailureV8::After {
            _owner: actual,
            error,
        });
    }
    Ok(actual)
}

#[cfg(test)]
impl LiveExecutedOwnedEffectV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_reduce_arguments_v8(
        &self,
    ) -> Vec<crate::interpreter::retained_call::RetainedValue> {
        self.executed.test_reduce_arguments_v8()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod step;
