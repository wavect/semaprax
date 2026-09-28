//! Settlement data derived only from the actual dispatched owner. This sealed
//! inert projection creates no ACK, cleanup permit, Outcome or reducer input.
use super::*;
use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
    checked_owned_effect_settlement_v8, OwnedEffectSettlementInputsV8,
};
use crate::live_invocation::source_journal::{source_effect_digest, SourceJournalEntry};

pub(crate) struct CheckedLiveOwnedEffectSettlementV8 {
    ordinary: SourceJournalEntry,
    evidence: Vec<u8>,
    evidence_digest: String,
    result: Option<Vec<u8>>,
    intent: u32,
}
impl CheckedLiveOwnedEffectSettlementV8 {
    pub(crate) fn ordinary(&self) -> &SourceJournalEntry {
        &self.ordinary
    }
    pub(crate) fn evidence(&self) -> &[u8] {
        &self.evidence
    }
    pub(crate) fn evidence_digest(&self) -> &str {
        &self.evidence_digest
    }
    pub(crate) fn result(&self) -> Option<&[u8]> {
        self.result.as_deref()
    }
    pub(crate) fn intent(&self) -> u32 {
        self.intent
    }
}
impl StagedOwnedEffectV8<'_> {
    /// The real owner and retained ledger select the row. Decoded evidence or
    /// caller-shaped rows cannot enter this projection. Source guards surround
    /// this pure validation before any physical append starts.
    pub(crate) fn checked_live_settlement_v8(
        &self,
        accounting: &TargetAccounting,
    ) -> Result<CheckedLiveOwnedEffectSettlementV8, SourceJournalError> {
        if self.cleanup_started
            || self.authority_lost
            || self.prepared.creator != std::process::id()
            || self.prepared.inputs.cancellation.is_cancelled()
        {
            return Err(SourceJournalError::Binding);
        }
        let (basis, budget) = checked_basis(&self.prepared.inputs, &self.prepared.owner)
            .ok_or(SourceJournalError::Binding)?;
        if basis != self.prepared.basis || budget != self.prepared.budget {
            return Err(SourceJournalError::Binding);
        }
        let dispatch = self.dispatch.as_ref().ok_or(SourceJournalError::Binding)?;
        if dispatch.evidence().accounting() != *accounting {
            return Err(SourceJournalError::Binding);
        }
        let ordinary = match (self.observation(), self.reason()) {
            (Some(observation), None) => SourceJournalEntry::EffectObserved {
                turn: basis.turn,
                attempt: basis.attempt,
                operation: self.prepared.plan.operation().operation_id().into(),
                observation: observation.to_vec(),
                observation_digest: source_effect_digest(observation),
            },
            (
                None,
                Some(
                    reason
                    @ (SourceEffectFailure::HandlerFailed | SourceEffectFailure::ResultLimit),
                ),
            ) => SourceJournalEntry::EffectFailed {
                turn: basis.turn,
                attempt: basis.attempt,
                operation: self.prepared.plan.operation().operation_id().into(),
                reason,
            },
            _ => return Err(SourceJournalError::Binding),
        };
        let evidence = dispatch.evidence().canonical_wire();
        let result = self.target_result_wire();
        let inputs = &self.prepared.inputs;
        let facts = checked_owned_effect_settlement_v8(
            OwnedEffectSettlementInputsV8 {
                runtime: inputs.runtime,
                execution: inputs.execution,
                scope: &inputs.store.registration().expected_facts().scope,
                turn: inputs.turn,
                attempt: inputs.attempt,
                state: &basis.state,
                decision: &basis.decision,
                proposal: &inputs.proposal,
            },
            &ordinary,
            &evidence,
            result.as_deref(),
        )?;
        if facts.accepted_payload() != self.observation()
            || facts.evidence().accounting() != *accounting
        {
            return Err(SourceJournalError::Binding);
        }
        Ok(CheckedLiveOwnedEffectSettlementV8 {
            ordinary,
            evidence_digest: facts.evidence().digest().to_owned(),
            evidence,
            result,
            intent: self.intent,
        })
    }
}

pub(in crate::interpreter::resumable::owned_frame::registered_stage::effect) mod cleanup;
