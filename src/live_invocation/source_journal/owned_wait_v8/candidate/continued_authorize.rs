//! Current authenticated A-prefix facts are descriptive, never an owner factory.
use super::*;
use crate::live_invocation::source_journal::SourceStageRole;
impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_authorize_facts(
        &self,
    ) -> Result<(u64, u32, u32, &EntryV8), SourceJournalError> {
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        let turn = folded
            .continued_authorize_turn()
            .ok_or(SourceJournalError::Order)?;
        let selected = &self.entries.last().ok_or(SourceJournalError::Order)?.entry;
        let actual = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedWaitCompleted {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedStateTransferReserved {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedStateTransferCompleted {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedAuthorizationStaged {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(
                SourceJournalEntry::ProposalAdmitted {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::StageReservation {
                    turn,
                    attempt: Some(0),
                    role: SourceStageRole::Authorize,
                    ..
                },
            ) => *turn,
            _ => return Err(SourceJournalError::Order),
        };
        if actual != turn {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(self.context.fold(), &folded)?
            .check(self.document.len(), self.entries.len())?;
        self.continued_model_accounting()?;
        Ok((folded.reserved_total, folded.stages, turn, selected))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_continued_authorize_prefix(
        &self,
        selected: &EntryV8,
    ) -> Result<(), SourceJournalError> {
        let (_, _, turn, previous) = self.continued_authorize_facts()?;
        let actual = match selected {
            EntryV8::Owned(
                model::OwnedBodyV8::OwnedStateTransferReserved {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedStateTransferCompleted {
                    turn, attempt: 0, ..
                }
                | model::OwnedBodyV8::OwnedAuthorizationStaged {
                    turn, attempt: 0, ..
                },
            ) => *turn,
            EntryV8::Ordinary(
                SourceJournalEntry::ProposalAdmitted {
                    turn, attempt: 0, ..
                }
                | SourceJournalEntry::StageReservation {
                    turn,
                    attempt: Some(0),
                    role: SourceStageRole::Authorize,
                    ..
                },
            ) => *turn,
            _ => return Err(SourceJournalError::Binding),
        };
        let allowed = matches!(
            (previous, selected),
            (
                EntryV8::Owned(model::OwnedBodyV8::OwnedWaitCompleted { .. }),
                EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted { .. })
            ) | (
                EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted { .. }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferReserved { .. })
            ) | (
                EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferReserved { .. }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { .. })
            ) | (
                EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { .. }),
                EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    role: SourceStageRole::Authorize,
                    ..
                })
            ) | (
                EntryV8::Ordinary(SourceJournalEntry::StageReservation {
                    role: SourceStageRole::Authorize,
                    ..
                }),
                EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged { .. })
            )
        );
        if !allowed || actual != turn {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
impl<'a> InventoryV8<'a> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_fixed_continued_authorize(
        self,
        lease: &SourceOwnedWaitLeaseV8,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedAuthorizeAppendPermitV8<'_, '_>,
    ) -> Result<CandidateV8<'a>, CandidateRejectionV8<'a>> {
        let row = permit.selected_row().clone();
        #[cfg(test)]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedAuthorize(journal, permit),
                None,
            )
        }
        #[cfg(not(test))]
        {
            self.prepare_inner(
                row,
                Some(lease),
                ProducerV8::ContinuedAuthorize(journal, permit),
            )
        }
    }
}
impl PendingV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_fixed_continued_authorize_prefix(
        &self,
        journal: &SourceOwnedWaitJournalV8,
        permit: &super::super::live_upstream::FixedOwnedContinuedAuthorizeAppendPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        if self.0.row.entry != *permit.selected_row() {
            return Err(SourceJournalError::Binding);
        }
        permit.validate_selected_prefix(journal, &self.0.inventory)
    }
}
