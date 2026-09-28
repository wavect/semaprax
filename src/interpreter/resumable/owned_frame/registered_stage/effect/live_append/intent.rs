//! Actual Intent ACK activation, with no target entry or accounting operation.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_owned_effect_request_v8, OwnedEffectSettlementInputsV8,
};
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
        let Some((basis, budget)) = checked_basis(&self.inputs, &self.owner) else {
            return Err(SourceJournalError::Binding);
        };
        if basis != self.basis || budget != self.budget {
            return Err(SourceJournalError::Binding);
        }
        let checked = checked_owned_effect_request_v8(&OwnedEffectSettlementInputsV8 {
            runtime: self.inputs.runtime,
            execution: self.inputs.execution,
            scope: &self.inputs.store.registration().expected_facts().scope,
            turn: self.inputs.turn,
            attempt: self.inputs.attempt,
            state: &basis.state,
            decision: &basis.decision,
            proposal: &self.inputs.proposal,
        })?;
        let request_digest =
            target_protocol::owned_wait_v8::physical::request_digest(&self.request);
        if checked.request_digest() != request_digest
            || checked.operation().operation_id() != self.plan.operation().operation_id()
        {
            return Err(SourceJournalError::Binding);
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
