//! Actual Intent ACK activation, with no target entry or accounting operation.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_owned_effect_request_v8, OwnedEffectSettlementInputsV8,
};
use crate::agent_lifecycle::authorization::CheckedOwnedWaitReadyCommitmentsV8;
use crate::live_invocation::source_journal::SourceJournalEntry;

pub(crate) struct ActivatedOwnedEffectV8<'j> {
    pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) staged:
        StagedOwnedEffectV8<'j>,
}

/// Existing ACK checks and conversion boundary, shared with the old dispatcher.
/// It cannot enter a host, emit settlement evidence or release an owned leaf.
pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) fn activate_ack_owned_effect_v8<
    'j,
>(
    prepared: PreparedOwnedEffectV8<'j>,
    ack: OwnedEffectIntentAckV8,
) -> Result<ActivatedOwnedEffectV8<'j>, OwnedEffectDispatchRejectionV8<'j>> {
    let request_digest =
        target_protocol::owned_wait_v8::physical::request_digest(&prepared.request);
    if ack.basis != prepared.basis
        || ack.authorization != prepared.authorization_tail
        || ack.intent <= ack.authorization
        || ack.request != request_digest
        || ack.operation != prepared.plan.operation().operation_id()
    {
        return Err(OwnedEffectDispatchRejectionV8 {
            prepared,
            diagnostic: rejected("effect intent ACK differs"),
        });
    }
    // A valid acknowledged intent is consumed even if the subsequent guard
    // refuses. Returning Staged rather than Prepared prevents redispatch.
    let staged = StagedOwnedEffectV8 {
        prepared,
        intent: ack.intent,
        dispatch: None,
        accepted: None,
        failure: None,
        cleanup_started: false,
        authority_lost: false,
    };
    Ok(ActivatedOwnedEffectV8 { staged })
}

impl PreparedOwnedEffectV8<'_> {
    /// Pure request facts borrowed from the actual still-retained engine owner.
    pub(crate) fn live_intent_row(&self) -> Result<SourceJournalEntry, SourceJournalError> {
        self.live_intent_row_checked(None)
    }
    pub(crate) fn live_continued_intent_row(
        &self,
        permit: &crate::live_invocation::source_journal::LiveContinuedIntentPermitV8<'_, '_>,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        permit.validate_guard(&self.inputs)?;
        let row = self.live_intent_row_checked(Some(permit))?;
        permit.validate_guard(&self.inputs)?;
        Ok(row)
    }
    fn live_intent_row_checked(
        &self,
        continued: Option<
            &crate::live_invocation::source_journal::LiveContinuedIntentPermitV8<'_, '_>,
        >,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        let Some((basis, budget)) = checked_basis(&self.inputs, &self.owner) else {
            return Err(SourceJournalError::Binding);
        };
        if basis != self.basis || budget != self.budget {
            return Err(SourceJournalError::Binding);
        }
        let inputs = OwnedEffectSettlementInputsV8 {
            runtime: self.inputs.runtime,
            execution: self.inputs.execution,
            scope: &self.inputs.store.registration().expected_facts().scope,
            turn: self.inputs.turn,
            attempt: self.inputs.attempt,
            state: &basis.state,
            decision: &basis.decision,
            proposal: &self.inputs.proposal,
        };
        let request_digest =
            target_protocol::owned_wait_v8::physical::request_digest(&self.request);
        if let Some(permit) = continued {
            permit.check_request(
                &inputs,
                &request_digest,
                self.plan.operation().operation_id(),
            )?;
        } else {
            let checked = checked_owned_effect_request_v8(&inputs)?;
            if checked.request_digest() != request_digest
                || checked.operation().operation_id() != self.plan.operation().operation_id()
            {
                return Err(SourceJournalError::Binding);
            }
        }
        Ok(SourceJournalEntry::EffectIntent {
            turn: self.inputs.turn,
            attempt: self.inputs.attempt,
            operation: self.plan.operation().operation_id().into(),
            request_digest,
        })
    }
}

#[cfg(test)]
mod tests;

/// Rejections retain the actual consumed engine owner, never rewind an Intent.
pub(crate) enum LiveEffectActivationRejectionV8<'j> {
    Before {
        prepared: PreparedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
    After {
        activated: ActivatedOwnedEffectV8<'j>,
        error: SourceJournalError,
    },
}
impl LiveEffectActivationRejectionV8<'_> {
    pub(crate) fn error(&self) -> SourceJournalError {
        match self {
            Self::Before { error, .. } | Self::After { error, .. } => *error,
        }
    }
}
use crate::live_invocation::source_journal::LiveEffectIntentPermitV8;
pub(crate) enum LiveIntentGuardV8<'g, 'p, 'j> {
    Initial(&'g LiveEffectIntentPermitV8<'p, 'j>),
    Continued(&'g crate::live_invocation::source_journal::LiveContinuedIntentPermitV8<'p, 'j>),
}
impl LiveIntentGuardV8<'_, '_, '_> {
    fn validate_guard(&self, inputs: &OwnedEffectInputsV8<'_>) -> Result<(), SourceJournalError> {
        match self {
            Self::Initial(p) => p.validate_guard(inputs),
            Self::Continued(p) => p.validate_guard(inputs),
        }
    }
    fn references(&self) -> Result<(u32, u32, u32, u32), SourceJournalError> {
        match self {
            Self::Initial(p) => p.references(),
            Self::Continued(p) => p.references(),
        }
    }
    fn matches_commitments(&self, c: &CheckedOwnedWaitReadyCommitmentsV8) -> bool {
        match self {
            Self::Initial(p) => p.matches_commitments(c),
            Self::Continued(p) => p.matches_commitments(c),
        }
    }
    fn matches_intent_row(&self, row: &SourceJournalEntry) -> bool {
        match self {
            Self::Initial(p) => p.matches_intent_row(row),
            Self::Continued(p) => p.matches_intent_row(row),
        }
    }
    fn row(
        &self,
        prepared: &PreparedOwnedEffectV8<'_>,
    ) -> Result<SourceJournalEntry, SourceJournalError> {
        match self {
            Self::Initial(_) => prepared.live_intent_row(),
            Self::Continued(p) => prepared.live_continued_intent_row(p),
        }
    }
}
pub(crate) fn activate_live_owned_effect_v8<'j>(
    prepared: PreparedOwnedEffectV8<'j>,
    permit: &LiveEffectIntentPermitV8<'_, 'j>,
) -> Result<ActivatedOwnedEffectV8<'j>, LiveEffectActivationRejectionV8<'j>> {
    activate_with_guard_v8(prepared, LiveIntentGuardV8::Initial(permit))
}
pub(crate) fn activate_with_guard_v8<'j>(
    prepared: PreparedOwnedEffectV8<'j>,
    permit: LiveIntentGuardV8<'_, '_, 'j>,
) -> Result<ActivatedOwnedEffectV8<'j>, LiveEffectActivationRejectionV8<'j>> {
    if let Err(error) = prepared.validate_intent_guard(&permit) {
        prepared.quarantine_live_authorization();
        return Err(LiveEffectActivationRejectionV8::Before { prepared, error });
    }
    let (_, _, consumed, intent) = match permit.references() {
        Ok(r) => r,
        Err(error) => {
            prepared.quarantine_live_authorization();
            return Err(LiveEffectActivationRejectionV8::Before { prepared, error });
        }
    };
    let ack = OwnedEffectIntentAckV8 {
        basis: prepared.basis.clone(),
        authorization: consumed,
        intent,
        request: target_protocol::owned_wait_v8::physical::request_digest(&prepared.request),
        operation: prepared.plan.operation().operation_id().into(),
    };
    let activated = match activate_ack_owned_effect_v8(prepared, ack) {
        Ok(owner) => owner,
        Err(failed) => {
            failed.prepared.quarantine_live_authorization();
            return Err(LiveEffectActivationRejectionV8::Before {
                prepared: failed.prepared,
                error: SourceJournalError::Binding,
            });
        }
    };
    #[cfg(test)]
    if matches!(&permit, LiveIntentGuardV8::Continued(_)) {
        crate::interpreter::resumable::owned_frame::registered_stage::live_run::test_note_continued_activation();
    }
    if let Err(error) = activated.validate_intent_guard(&permit) {
        activated.staged.prepared.quarantine_live_authorization();
        return Err(LiveEffectActivationRejectionV8::After { activated, error });
    }
    Ok(activated)
}
impl PreparedOwnedEffectV8<'_> {
    pub(crate) fn validate_live_intent(
        &self,
        permit: &LiveEffectIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_intent_guard(&LiveIntentGuardV8::Initial(permit))
    }
    pub(crate) fn validate_intent_guard(
        &self,
        permit: &LiveIntentGuardV8<'_, '_, '_>,
    ) -> Result<(), SourceJournalError> {
        let result = (|| {
            permit.validate_guard(&self.inputs)?;
            let Some((basis, budget)) = checked_basis(&self.inputs, &self.owner) else {
                return Err(SourceJournalError::Binding);
            };
            let (staged, ready, consumed, intent) = permit.references()?;
            if basis != self.basis
                || budget != self.budget
                || (staged, ready, consumed) != (self.staged, self.ready, self.authorization_tail)
                || consumed.checked_add(1) != Some(intent)
            {
                return Err(SourceJournalError::Binding);
            }
            let commitments = checked_owned_wait_ready_commitments_v8(
                self.inputs.runtime,
                self.inputs.execution,
                &self.inputs.store.registration().expected_facts().scope,
                self.inputs.turn,
                self.inputs.attempt,
                &basis.state,
                &basis.decision,
                &self.inputs.proposal,
            )?;
            if !permit.matches_commitments(&commitments)
                || !permit.matches_intent_row(&permit.row(self)?)
            {
                return Err(SourceJournalError::Binding);
            }
            permit.validate_guard(&self.inputs)
        })();
        if result.is_err() {
            self.quarantine_live_authorization();
        }
        result
    }
}
impl ActivatedOwnedEffectV8<'_> {
    pub(crate) fn quarantine_live_intent(&self) {
        self.staged.prepared.quarantine_live_authorization();
    }

    pub(crate) fn validate_live_intent(
        &self,
        permit: &LiveEffectIntentPermitV8<'_, '_>,
    ) -> Result<(), SourceJournalError> {
        self.validate_intent_guard(&LiveIntentGuardV8::Initial(permit))
    }
    pub(crate) fn validate_intent_guard(
        &self,
        permit: &LiveIntentGuardV8<'_, '_, '_>,
    ) -> Result<(), SourceJournalError> {
        let (_, _, _, intent) = permit.references()?;
        if self.staged.intent != intent
            || self.staged.dispatch.is_some()
            || self.staged.cleanup_started
            || self.staged.authority_lost
        {
            self.staged.prepared.quarantine_live_authorization();
            return Err(SourceJournalError::Binding);
        }
        self.staged.prepared.validate_intent_guard(permit)
    }
}

pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) mod dispatch;
