//! Turn-one Reduce reservation after actual continued cleanup settlement.
//! This stops at the durable reservation ACK; it neither evaluates nor Steps.
use super::*;
use crate::live_invocation::source_journal::SourceStageRole;
use crate::resumable_effects::owned_frame::v2::{compile_owned_reduce_v2, CheckedOwnedReduceV2};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedReduceReservationAppendV8<
    'j,
> {
    owner: SettledContinuedDecisionCleanupV8<'j>,
    plan: CheckedOwnedReduceV2,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedReduceReservationRejectionV8<
    'j,
> {
    _owner: SettledContinuedDecisionCleanupV8<'j>,
    error: SourceJournalError,
}
impl<'j> SettledContinuedDecisionCleanupV8<'j> {
    fn continued_reduce_row(&self) -> Result<(CheckedOwnedReduceV2, EntryV8), SourceJournalError> {
        self.validate_live()?;
        if !self.outcome_minted()? {
            return Err(SourceJournalError::Binding);
        }
        let journal = self.owner.owner.owner.journal();
        let (_, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let plan =
            compile_owned_reduce_v2(execution.wait()).map_err(|_| SourceJournalError::Binding)?;
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            turn, attempt, ..
        }) = self.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        if *turn == 0
            || *attempt != 0
            || Some(execution.evaluation_fuel())
                != journal.context().ordinary().max_steps_per_stage()
            || plan.binding() != execution.wait().binding()
            || !plan.helper().same_helper(execution.wait().helper())
        {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            plan,
            EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                turn: *turn,
                attempt: Some(*attempt),
                role: SourceStageRole::Reduce,
                fuel: execution.evaluation_fuel(),
            }),
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_continued_reduce(
        self,
    ) -> Result<LiveContinuedReduceReservationAppendV8<'j>, ContinuedReduceReservationRejectionV8<'j>>
    {
        match self.continued_reduce_row() {
            Ok((plan, selected)) => Ok(LiveContinuedReduceReservationAppendV8 {
                owner: self,
                plan,
                selected,
            }),
            Err(error) => {
                self.owner.owner.owner.journal().quarantine();
                Err(ContinuedReduceReservationRejectionV8 {
                    _owner: self,
                    error,
                })
            }
        }
    }
}
impl<'j> LiveContinuedReduceReservationAppendV8<'j> {
    fn journal(&self) -> &SourceOwnedWaitJournalV8 {
        self.owner.owner.owner.owner.journal()
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
        self.owner.session.sequence()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledged_bytes(
        &self,
    ) -> usize {
        self.owner.session.acknowledged_bytes()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            self.owner.validate_live()?;
            let (plan, selected) = self.owner.continued_reduce_row()?;
            if plan.binding() != self.plan.binding()
                || !plan.helper().same_helper(self.plan.helper())
                || selected != self.selected
            {
                return Err(SourceJournalError::Binding);
            }
            self.owner.validate_live()
        })();
        result.inspect_err(|_| self.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixed_append_permit(
        &self,
    ) -> Result<FixedOwnedContinuedReduceReservationAppendPermitV8<'_, 'j>, SourceJournalError>
    {
        self.validate_live()?;
        Ok(FixedOwnedContinuedReduceReservationAppendPermitV8 { owner: self })
    }
    fn hold(&self) -> Result<&ProspectiveOwnedReduceHoldV8<'_>, SourceJournalError> {
        self.owner.owner.owner.owner.hold()
    }
    fn accounting(&self) -> &TargetAccounting {
        self.owner.accounting()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_reduce_successor(
        &self,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.sequence(),
            self.acknowledged_bytes(),
            &self.selected,
        )?;
        witness.validate_current_session(session)?;
        self.hold()?.validate_continued_spent_reduce_guard(
            self.journal(),
            session.sequence(),
            session.acknowledged_bytes(),
        )?;
        let (_, execution) = self
            .journal()
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        if self.plan.binding() != execution.wait().binding()
            || !self.plan.helper().same_helper(execution.wait().helper())
            || !matches!(
                &self.selected,
                EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    turn,
                    attempt: Some(0),
                    role: SourceStageRole::Reduce,
                    fuel,
                }) if *turn > 0 && *fuel == execution.evaluation_fuel()
            )
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FixedOwnedContinuedReduceReservationAppendPermitV8<
    'p,
    'j,
> {
    owner: &'p LiveContinuedReduceReservationAppendV8<'j>,
}
impl FixedOwnedContinuedReduceReservationAppendPermitV8<'_, '_> {
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
        inventory: &crate::live_invocation::source_journal::owned_wait_v8::candidate::InventoryV8<
            '_,
        >,
    ) -> Result<(), SourceJournalError> {
        if !self.owner.belongs_to(journal)
            || inventory.sequence() != self.owner.sequence()
            || inventory.acknowledged_bytes() != self.owner.acknowledged_bytes()
        {
            return Err(SourceJournalError::Binding);
        }
        self.owner.hold()?.validate_continued_reduce_append_prefix(
            journal,
            inventory,
            self.owner.selected_row(),
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_registry(
        &self,
        witness: &crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'_>,
        session: &AppendSessionV8<'_>,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .hold()?
            .advance_continued_reduce_ack(witness, session)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct LiveContinuedReduceReservedV8<'j> {
    owner: LiveContinuedReduceReservationAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) enum LiveContinuedReduceAdvanceFailureV8<
    'j,
> {
    Before { _owner: LiveContinuedReduceReservationAppendV8<'j>, _session: AppendSessionV8<'j>, _witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>, error: SourceJournalError },
    After { _owner: LiveContinuedReduceReservedV8<'j>, error: SourceJournalError },
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn advance_verified_continued_reduce_v8<
    'j,
>(
    owner: LiveContinuedReduceReservationAppendV8<'j>,
    session: AppendSessionV8<'j>,
    witness: crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::VerifiedOwnedContinuedReduceSuccessorV8<'j>,
) -> Result<LiveContinuedReduceReservedV8<'j>, LiveContinuedReduceAdvanceFailureV8<'j>> {
    if let Err(error) = owner.validate_reduce_successor(&witness, &session) {
        return Err(LiveContinuedReduceAdvanceFailureV8::Before {
            _owner: owner,
            _session: session,
            _witness: witness,
            error,
        });
    }
    let reserved = LiveContinuedReduceReservedV8 {
        owner,
        session,
        witness,
    };
    if let Err(error) = reserved.validate_live() {
        return Err(LiveContinuedReduceAdvanceFailureV8::After {
            _owner: reserved,
            error,
        });
    }
    Ok(reserved)
}
impl LiveContinuedReduceReservedV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .validate_reduce_successor(&self.witness, &self.session)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.accounting()
    }
}
