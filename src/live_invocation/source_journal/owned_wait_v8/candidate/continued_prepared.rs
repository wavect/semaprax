use super::*;

impl InventoryV8<'_> {
    pub(super) fn continued_prepared_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let result = self.continued_start_facts()?;
        if !matches!(
            result.3,
            EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        Ok(result)
    }
    pub(super) fn validate_continued_prepared_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, previous) = self.continued_start_facts()?;
        let allowed = matches!((previous,selected),
            (EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved{turn:t,attempt:0,wait,phase:model::PhaseV8::Start,replay_of:None,fuel,..}),
             EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared{turn:next,attempt:0,wait:next_wait,reservation,consumed,..}))
            if *t==turn&&t==next&&wait==next_wait&&usize::try_from(*reservation).ok()==self.sequence().checked_sub(1)&&consumed<=fuel);
        if allowed {
            Ok(())
        } else {
            Err(SourceJournalError::Binding)
        }
    }
}

impl InventoryV8<'_> {
    pub(super) fn continued_start_checkpoint_basis(
        &self,
        observation: &crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8,
    ) -> Result<(u64, u64, String), SourceJournalError> {
        let (_, _, current_turn, _) = self.continued_start_facts()?;
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        if folded.tail != fold::TailV8::StartReserved {
            return Err(SourceJournalError::Order);
        }
        let [.., created, reserved] = self.entries.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
            turn: actual_turn,
            attempt: 0,
            wait,
            phase: model::PhaseV8::Start,
            replay_of: None,
            ..
        }) = &reserved.entry
        else {
            return Err(SourceJournalError::Order);
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCreated {
            turn: created_turn,
            attempt: 0,
            wait: created_wait,
            argument_digest,
            copy_arguments,
            ..
        }) = &created.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if *actual_turn != current_turn
            || actual_turn != created_turn
            || wait != created_wait
            || copy_arguments != observation.copy_arguments()
        {
            return Err(SourceJournalError::Binding);
        }
        Ok((
            folded.reserved_total,
            folded.consumed_recorded,
            argument_digest.clone(),
        ))
    }
}
