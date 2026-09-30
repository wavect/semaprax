//! Borrowed descriptive facts from the same authenticated current inventory.
//! This does not construct a runtime ledger, owner, token or SDK permit.
use super::*;
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        let turn = folded
            .continued_model_turn()
            .ok_or(SourceJournalError::Order)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let actual = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedWaitPrepared {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedWaitReserved {
                    turn,
                    attempt: 0,
                    phase: model::PhaseV8::Resume,
                    replay_of: None,
                    ..
                }
                | model::OwnedBodyV8::OwnedWaitCompleted {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(
                SourceJournalEntry::AttemptIntent {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptSettled {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptFailed {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptUsage {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(SourceJournalEntry::PricedAttemptIntent(intent))
                if intent.attempt == 0 =>
            {
                intent.turn
            }
            _ => return Err(SourceJournalError::Order),
        };
        if actual != turn {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(self.context.fold(), &folded)?
            .check(self.document.len(), self.entries.len())?;
        Ok((folded.reserved_total, folded.stages, turn, selected))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_accounting(
        &self,
    ) -> Result<
        crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        SourceJournalError,
    > {
        let ContextV8::Checked(context) = self.context else {
            return Err(SourceJournalError::Binding);
        };
        let proof = self
            .accounting
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        if !proof.matches(context, self.document.len(), self.entries.len(), &self.mac) {
            return Err(SourceJournalError::Binding);
        }
        Ok(proof
            .previous()
            .ok_or(SourceJournalError::Binding)?
            .total()
            .clone())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_request_basis(
        &self,
    ) -> Result<
        (
            u32,
            Option<Vec<u8>>,
            crate::agent_lifecycle::authorization::target_protocol::TargetAccounting,
        ),
        SourceJournalError,
    > {
        let (_, _, turn, selected) = self.continued_model_facts()?;
        if !matches!(
            selected,
            EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        let total = self.continued_model_accounting()?;
        let proof = self
            .accounting
            .as_ref()
            .ok_or(SourceJournalError::Binding)?;
        let sequence = proof
            .previous_settlement_sequence()
            .ok_or(SourceJournalError::Binding)?;
        let Some(EntryV8::Ordinary(SourceJournalEntry::EffectObserved {
            turn: previous,
            observation,
            ..
        })) = self.entries.get(sequence).map(|e| &e.entry)
        else {
            return Err(SourceJournalError::Binding);
        };
        if previous.checked_add(1) != Some(turn) {
            return Err(SourceJournalError::Binding);
        }
        let ordinal = u32::try_from(
            self.entries
                .iter()
                .filter(|e| {
                    matches!(
                        e.entry,
                        EntryV8::Ordinary(
                            SourceJournalEntry::AttemptIntent { .. }
                                | SourceJournalEntry::PricedAttemptIntent(..)
                        )
                    )
                })
                .count(),
        )
        .map_err(|_| SourceJournalError::Capacity)?;
        Ok((ordinal, Some(observation.clone()), total))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_model_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, previous) = self.continued_model_facts()?;
        let next_turn = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedWaitReserved {
                    turn,
                    attempt: 0,
                    phase: model::PhaseV8::Resume,
                    replay_of: None,
                    ..
                }
                | model::OwnedBodyV8::OwnedWaitCompleted {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(
                SourceJournalEntry::AttemptIntent {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptSettled {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptFailed {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::AttemptUsage {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(SourceJournalEntry::PricedAttemptIntent(intent))
                if intent.attempt == 0 =>
            {
                intent.turn
            }
            _ => return Err(SourceJournalError::Binding),
        };
        let allowed = match (previous, selected) {
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitPrepared { .. }),
                EntryV8::Ordinary(
                    SourceJournalEntry::AttemptIntent { .. }
                    | SourceJournalEntry::PricedAttemptIntent(..),
                ),
            ) => true,
            (
                EntryV8::Ordinary(
                    SourceJournalEntry::AttemptIntent { .. }
                    | SourceJournalEntry::PricedAttemptIntent(..),
                ),
                EntryV8::Ordinary(
                    SourceJournalEntry::AttemptSettled { .. }
                    | SourceJournalEntry::AttemptFailed { .. },
                ),
            ) => true,
            (
                EntryV8::Ordinary(
                    SourceJournalEntry::AttemptSettled { .. }
                    | SourceJournalEntry::AttemptFailed { .. },
                ),
                EntryV8::Ordinary(SourceJournalEntry::AttemptUsage { .. }),
            ) => true,
            (
                EntryV8::Ordinary(SourceJournalEntry::AttemptUsage { .. }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved { .. }),
            ) => true,
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitReserved {
                    phase: model::PhaseV8::Resume,
                    ..
                }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCompleted { .. }),
            ) => true,
            _ => false,
        };
        if !allowed || next_turn != turn {
            return Err(SourceJournalError::Binding);
        }
        self.continued_model_accounting()?;
        Ok(())
    }
}

impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_model(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedModelAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedModel(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedModel(journal, permit),
            )
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_model_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedModelAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
#[cfg(test)]
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn test_model_accounting_matches(
        &self,
        context: &CheckedOwnedWaitJournalContextV8,
        bytes: usize,
        rows: usize,
        mac: &str,
    ) -> bool {
        self.accounting
            .as_ref()
            .is_some_and(|p| p.matches(context, bytes, rows, mac))
    }
}
