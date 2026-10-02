//! Actual continued Decision cleanup through the shared fixed ACK adapter.
//! The retained Settled holder grants no Outcome, Reduce or recovery authority.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::wire;
use serde_json::{json, Value};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct PreparedContinuedDecisionCleanupV8<
    'j,
> {
    owner: LiveRecordedContinuedEffectV8<'j>,
    selected: EntryV8,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedDecisionCleanupRejectionV8<
    'j,
> {
    owner: LiveRecordedContinuedEffectV8<'j>,
    error: SourceJournalError,
}
fn sequence(session: &AppendSessionV8<'_>) -> Result<u32, SourceJournalError> {
    session
        .sequence()
        .checked_sub(1)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or(SourceJournalError::Capacity)
}
impl<'j> LiveRecordedContinuedEffectV8<'j> {
    fn cleanup_values(&self) -> Result<(Value, Value), SourceJournalError> {
        let owner = &self.phase.owner.phase.owner.owner;
        let prior = owner.acks.last().ok_or(SourceJournalError::Order)?;
        let [settlement, recorded] = self.phase.acks.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        owner
            .authorization
            .actual()?
            .owner
            .continued_cleanup_values(
                &prior.session,
                &prior.witness,
                &self.phase.owner.phase.ack.session,
                &self.phase.owner.phase.ack.witness,
                owner.authorization.proposal()?,
                &owner.commitments,
                owner.preparation_references()?,
                Some((&settlement.session, &settlement.witness)),
                Some((&recorded.session, &recorded.witness)),
            )
    }
    fn cleanup_row(&self) -> Result<EntryV8, SourceJournalError> {
        self.validate_live()?;
        let (decision, operations) = self.cleanup_values()?;
        let effect = &self.phase.owner.phase.owner.owner;
        let (staged, ready, consumed) = effect.preparation_references()?;
        let turn = match self.phase.facts.ordinary() {
            SourceJournalEntry::EffectObserved {
                turn, attempt: 0, ..
            }
            | SourceJournalEntry::EffectFailed {
                turn, attempt: 0, ..
            } if *turn > 0 => *turn,
            _ => return Err(SourceJournalError::Binding),
        };
        let journal = self.phase.owner.journal();
        let held = journal.hold()?;
        let scope = &held.registration().expected_facts().scope;
        let (_, execution) = journal
            .context()
            .ready_runtime()
            .ok_or(SourceJournalError::Binding)?;
        let decision_digest = wire::recipe_digest(
            wire::RecipeV8::Decision,
            &json!({
                "scope":{"program_root":scope.program_root(),"invocation":scope.invocation_id(),"policy_epoch":scope.policy_epoch()},
                "turn":turn,"attempt":0,"authorize":execution.wait().authorize().function().id.as_str(),"decision":decision
            }),
        )?;
        let [settlement, recorded] = self.phase.acks.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        let settlement = sequence(&settlement.session)?;
        let recorded = sequence(&recorded.session)?;
        let intent = self.phase.facts.intent();
        let operations_digest = wire::recipe_digest(
            wire::RecipeV8::EffectDecisionOperations,
            &json!({
                "turn":turn,"attempt":0,"staged":staged,"ready":ready,"consumed":consumed,
                "intent":intent,"settlement":settlement,"recorded":recorded,"decision_digest":decision_digest,"operations":operations
            }),
        )?;
        self.validate_live()?;
        Ok(EntryV8::Owned(
            OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
                turn,
                attempt: 0,
                staged,
                ready,
                consumed,
                intent,
                settlement,
                recorded,
                decision_digest,
                operations,
                operations_digest,
            },
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_decision_cleanup(
        self,
    ) -> Result<PreparedContinuedDecisionCleanupV8<'j>, ContinuedDecisionCleanupRejectionV8<'j>>
    {
        match self.cleanup_row() {
            Ok(selected) => Ok(PreparedContinuedDecisionCleanupV8 {
                owner: self,
                selected,
            }),
            Err(error) => {
                self.phase.owner.journal().quarantine();
                Err(ContinuedDecisionCleanupRejectionV8 { owner: self, error })
            }
        }
    }
}
impl PreparedContinuedDecisionCleanupV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner
            .cleanup_row()
            .and_then(|actual| {
                if actual == self.selected {
                    Ok(())
                } else {
                    Err(SourceJournalError::Binding)
                }
            })
            .inspect_err(|_| self.owner.phase.owner.journal().quarantine())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.accounting()
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) mod continued_reduce;

#[cfg(all(test, unix))]
mod tests;

use crate::live_invocation::source_journal::owned_wait_v8::append::owned_effect::{ProspectiveOwnedReduceHoldV8, VerifiedOwnedEffectCleanupSuccessorV8};
use crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::cleanup::LiveOwnedEffectCleanupAppendV8;
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct StartedContinuedDecisionCleanupV8<
    'j,
> {
    owner: PreparedContinuedDecisionCleanupV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
}
impl<'j> PreparedContinuedDecisionCleanupV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.phase.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn session(
        &self,
    ) -> &AppendSessionV8<'j> {
        self.owner.phase.current()
    }
    fn actual(&self) -> Result<&crate::live_invocation::source_journal::owned_wait_v8::live_upstream::ContinuedResumedWaitV8<'j>, SourceJournalError>{
        Ok(&self
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .actual()?
            .owner)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn hold(
        &self,
    ) -> Result<&ProspectiveOwnedReduceHoldV8<'j>, SourceJournalError> {
        Ok(self.actual()?.cleanup_hold())
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(&self) -> &EntryV8 {
        &self.selected
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn into_append(
        self,
    ) -> LiveOwnedEffectCleanupAppendV8<'j> {
        LiveOwnedEffectCleanupAppendV8::from_continued(self)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        witness.validate_predecessor(
            self.journal(),
            self.session().sequence(),
            self.session().acknowledged_bytes(),
            &self.selected,
        )?;
        self.actual()?.validate_continued_cleanup_guard(
            session,
            witness,
            self.owner
                .phase
                .owner
                .phase
                .owner
                .owner
                .authorization
                .proposal()?,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledge(
        self,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
    ) -> StartedContinuedDecisionCleanupV8<'j> {
        // The shared adapter has just checked this exact actual owner and ACK.
        StartedContinuedDecisionCleanupV8 {
            owner: self,
            session,
            witness,
        }
    }
}
impl StartedContinuedDecisionCleanupV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.validate_successor(&self.session, &self.witness)
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ReleasedContinuedDecisionCleanupV8<
    'j,
> {
    owner: StartedContinuedDecisionCleanupV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct SettledContinuedDecisionCleanupV8<
    'j,
> {
    owner: ReleasedContinuedDecisionCleanupV8<'j>,
    session: AppendSessionV8<'j>,
    witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedDecisionReleaseFailureV8<
    'j,
> {
    owner: StartedContinuedDecisionCleanupV8<'j>,
    error: SourceJournalError,
}
impl<'j> StartedContinuedDecisionCleanupV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn release_decision(
        mut self,
        observe: impl FnMut(&crate::cleanup_plan::FinalizeAction),
    ) -> Result<ReleasedContinuedDecisionCleanupV8<'j>, ContinuedDecisionReleaseFailureV8<'j>> {
        let result = (|| {
            self.validate_live()?;
            let completed = &mut self
                .owner
                .owner
                .phase
                .owner
                .phase
                .owner
                .owner
                .authorization
                .completed;
            let ModelOwnerV8::Resumed(resumed) = &mut completed.owner else {
                return Err(SourceJournalError::Order);
            };
            resumed.owner.release_continued_decision(
                &self.session,
                &self.witness,
                completed
                    .proposal
                    .as_ref()
                    .ok_or(SourceJournalError::Binding)?,
                &self.owner.owner.phase.facts,
                observe,
            )?;
            self.validate_live()
        })();
        match result {
            Ok(()) => Ok(ReleasedContinuedDecisionCleanupV8 { owner: self }),
            Err(error) => {
                self.owner.journal().quarantine();
                Err(ContinuedDecisionReleaseFailureV8 { owner: self, error })
            }
        }
    }
}
impl<'j> ReleasedContinuedDecisionCleanupV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn journal(
        &self,
    ) -> &'j SourceOwnedWaitJournalV8 {
        self.owner.owner.journal()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn session(
        &self,
    ) -> &AppendSessionV8<'j> {
        &self.owner.session
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn hold(
        &self,
    ) -> Result<&ProspectiveOwnedReduceHoldV8<'j>, SourceJournalError> {
        self.owner.owner.hold()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn receipt(
        &self,
    ) -> Result<&Value, SourceJournalError> {
        self.owner.owner.actual()?.continued_decision_receipt()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn selected(
        &self,
    ) -> Result<EntryV8, SourceJournalError> {
        self.owner.validate_live()?;
        let receipt = self.receipt()?.clone();
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
            turn, attempt, ..
        }) = self.owner.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        Ok(EntryV8::Owned(
            OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
                turn: *turn,
                attempt: *attempt,
                started: sequence(&self.owner.session)?,
                receipt_digest: wire::recipe_digest(wire::RecipeV8::Receipt, &receipt)?,
                receipt,
            },
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn prepare_settled(
        self,
    ) -> Result<LiveOwnedEffectCleanupAppendV8<'j>, Self> {
        match self.selected() {
            Ok(selected) => Ok(LiveOwnedEffectCleanupAppendV8::from_continued_released(
                self, selected,
            )),
            Err(_) => Err(self),
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_successor(
        &self,
        session: &AppendSessionV8<'_>,
        witness: &VerifiedOwnedEffectCleanupSuccessorV8<'_>,
    ) -> Result<(), SourceJournalError> {
        // After ACK the old Started guard is historical; compare exact receipt
        // without re-entering its obsolete physical cursor.
        let receipt = self.receipt()?;
        let EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupStarted {
            turn, attempt, ..
        }) = self.owner.witness.selected_row()
        else {
            return Err(SourceJournalError::Binding);
        };
        let selected = EntryV8::Owned(OwnedBodyV8::OwnedEffectDecisionCleanupSettled {
            turn: *turn,
            attempt: *attempt,
            started: sequence(&self.owner.session)?,
            receipt: receipt.clone(),
            receipt_digest: wire::recipe_digest(wire::RecipeV8::Receipt, receipt)?,
        });
        witness.validate_predecessor(
            self.journal(),
            self.session().sequence(),
            self.session().acknowledged_bytes(),
            &selected,
        )?;
        self.owner.owner.actual()?.validate_continued_cleanup_guard(
            session,
            witness,
            self.owner
                .owner
                .owner
                .phase
                .owner
                .phase
                .owner
                .owner
                .authorization
                .proposal()?,
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn acknowledge(
        self,
        session: AppendSessionV8<'j>,
        witness: VerifiedOwnedEffectCleanupSuccessorV8<'j>,
    ) -> SettledContinuedDecisionCleanupV8<'j> {
        SettledContinuedDecisionCleanupV8 {
            owner: self,
            session,
            witness,
        }
    }
}
impl SettledContinuedDecisionCleanupV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn validate_live(
        &self,
    ) -> Result<(), SourceJournalError> {
        self.owner.validate_successor(&self.session, &self.witness)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn accounting(
        &self,
    ) -> &TargetAccounting {
        self.owner.owner.owner.accounting()
    }
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ContinuedOutcomeFailureV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) owner:
        SettledContinuedDecisionCleanupV8<'j>,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) error: SourceJournalError,
}
impl<'j> SettledContinuedDecisionCleanupV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn mint_outcome(
        mut self,
    ) -> Result<Self, ContinuedOutcomeFailureV8<'j>> {
        let result = (|| {
            self.validate_live()?;
            let completed = &mut self
                .owner
                .owner
                .owner
                .owner
                .phase
                .owner
                .phase
                .owner
                .owner
                .authorization
                .completed;
            let ModelOwnerV8::Resumed(resumed) = &mut completed.owner else {
                return Err(SourceJournalError::Order);
            };
            resumed.owner.mint_continued_outcome(
                &self.session,
                &self.witness,
                completed
                    .proposal
                    .as_ref()
                    .ok_or(SourceJournalError::Binding)?,
            )?;
            self.validate_live()
        })();
        match result {
            Ok(()) => Ok(self),
            Err(error) => {
                self.owner.journal().quarantine();
                Err(ContinuedOutcomeFailureV8 { owner: self, error })
            }
        }
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn outcome_minted(
        &self,
    ) -> Result<bool, SourceJournalError> {
        Ok(self.owner.owner.owner.actual()?.continued_outcome_minted())
    }
    fn take_reduce_outcome(
        &mut self,
    ) -> Result<crate::interpreter::resumable::owned_frame::registered_stage::effect::ExecutedOwnedAgentTurnV2<'j>, SourceJournalError>{
        let completed = &mut self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &mut completed.owner else {
            return Err(SourceJournalError::Order);
        };
        resumed.owner.take_continued_reduce_outcome()
    }
    fn validate_spent_reduce_context(
        &self,
        sequence: usize,
        bytes: usize,
        step: bool,
        incurred: bool,
    ) -> Result<(), SourceJournalError> {
        let completed = &self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &completed.owner else {
            return Err(SourceJournalError::Order);
        };
        resumed.owner.validate_spent_reduce_context(
            sequence,
            bytes,
            completed
                .proposal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
            step,
            incurred,
        )
    }

    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn continued_model_origin(
        &self,
    ) -> Result<
        (
            &'j SourceOwnedWaitJournalV8,
            &dyn crate::live_invocation::SourceInvocationClock,
            TargetAccounting,
        ),
        SourceJournalError,
    > {
        let completed = &self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &completed.owner else {
            return Err(SourceJournalError::Order);
        };
        Ok((
            resumed.owner.model_journal(),
            resumed.owner.model_clock(),
            *resumed.owner.model_accounting(),
        ))
    }
}

impl<'j> SettledContinuedDecisionCleanupV8<'j> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_target(&self) -> bool {
        matches!(
            self.owner.owner.owner.owner.phase.facts.ordinary(),
            SourceJournalEntry::EffectFailed {
                reason: crate::live_invocation::source_journal::SourceEffectFailure::HandlerFailed
                    | crate::live_invocation::source_journal::SourceEffectFailure::ResultLimit,
                ..
            }
        )
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failure_session(
        &self,
    ) -> &AppendSessionV8<'j> {
        &self.session
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failure_origin(&self) -> Result<crate::live_invocation::source_journal::owned_wait_v8::live_upstream::effect::authorization::failed_state::continued::OriginV8<'_, 'j>, SourceJournalError>{
        let completed = &self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &completed.owner else {
            return Err(SourceJournalError::Order);
        };
        Ok(resumed.owner.failed_state_origin(
            completed
                .proposal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn failed_state_facts(
        &self,
    ) -> Result<
        (
            crate::live_invocation::source_journal::SourceEffectFailure,
            Value,
            u32,
            u32,
        ),
        SourceJournalError,
    > {
        let completed = &self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &completed.owner else {
            return Err(SourceJournalError::Order);
        };
        let (reason, state) = resumed.owner.continued_failed_state_facts(
            &self.session,
            &self.witness,
            completed
                .proposal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
        )?;
        let [settled, recorded] = self.owner.owner.owner.owner.phase.acks.as_slice() else {
            return Err(SourceJournalError::Order);
        };
        Ok((
            reason,
            state,
            sequence(&settled.session)?,
            sequence(&recorded.session)?,
        ))
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn take_failed_state(&mut self) -> Result<crate::interpreter::resumable::owned_frame::registered_stage::effect::PendingOwnedEffectReceiptV8<'j>, SourceJournalError>{
        let completed = &mut self
            .owner
            .owner
            .owner
            .owner
            .phase
            .owner
            .phase
            .owner
            .owner
            .authorization
            .completed;
        let ModelOwnerV8::Resumed(resumed) = &mut completed.owner else {
            return Err(SourceJournalError::Order);
        };
        resumed.owner.take_continued_failed_state(
            &self.session,
            &self.witness,
            completed
                .proposal
                .as_ref()
                .ok_or(SourceJournalError::Binding)?,
        )
    }
}
