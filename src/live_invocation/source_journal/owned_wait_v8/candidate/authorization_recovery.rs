//! Exact first-turn TransferCompleted recovery facts. These snapshots remain
//! inert; a separate trusted host grant is required to materialize an owner.
use super::*;
use crate::interpreter::resumable::checkpoint::channel_from_json;
use crate::resumable_effects::owned_frame::v2::{
    bind_recovered_owned_wait_proposal_v8, validate_owned_wait_state_v8, CheckedOwnedWaitProposalV8,
};

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct AuthorizationRecoveryFactsV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) state: Value,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) proposal:
        CheckedOwnedWaitProposalV8,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) wait: String,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) transfer: u32,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) state_digest: String,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) proposal_digest: String,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) sequence: usize,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) bytes: usize,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) authentication: String,
    pub(in crate::live_invocation::source_journal::owned_wait_v8) document_digest: String,
}

impl InventoryV8<'_> {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn first_turn_transfer_completed_authorization_recovery(
        &self,
    ) -> Result<AuthorizationRecoveryFactsV8, SourceJournalError> {
        // Folding the complete authenticated inventory establishes that this
        // is the current State basis and that no later phase consumed it.
        let folded = fold::fold(self.context.fold(), &self.entries)?;
        let completed = self.entries.last().ok_or(SourceJournalError::Order)?;
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted {
            turn: 0,
            attempt: 0,
            wait,
            reservation,
            state,
            state_digest,
            proposal,
            proposal_digest,
            transfer_digest,
        }) = &completed.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if self.entries.len() < 3 || folded.tail != fold::TailV8::PendingAuthorize {
            return Err(SourceJournalError::Order);
        }
        let transfer =
            u32::try_from(self.entries.len() - 1).map_err(|_| SourceJournalError::Capacity)?;
        let reserved = &self.entries[self.entries.len() - 2];
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferReserved {
            turn: 0,
            attempt: 0,
            wait: reserved_wait,
            from,
            to,
            state_digest: reserved_state_digest,
            proposal_digest: reserved_proposal_digest,
            transfer_digest: reserved_transfer_digest,
        }) = &reserved.entry
        else {
            return Err(SourceJournalError::Order);
        };
        if u32::try_from(self.entries.len() - 2).ok() != Some(*reservation)
            || reserved_wait != wait
            || reserved_state_digest != state_digest
            || reserved_proposal_digest != proposal_digest
            || reserved_transfer_digest != transfer_digest
        {
            return Err(SourceJournalError::Binding);
        }
        let admitted = &self.entries[self.entries.len() - 3].entry;
        if !matches!(admitted,
            EntryV8::Ordinary(SourceJournalEntry::ProposalAdmitted {
                turn: 0, attempt: 0, proposal_digest: admitted_digest
            }) if admitted_digest == proposal_digest
        ) {
            return Err(SourceJournalError::Order);
        }

        let context = match self.context {
            ContextV8::Checked(context) => context,
            #[cfg(test)]
            ContextV8::Synthetic(_) => return Err(SourceJournalError::Binding),
        };
        let (_, execution) = context.ready_runtime().ok_or(SourceJournalError::Binding)?;
        let binding = execution.wait();
        validate_owned_wait_state_v8(binding, state).map_err(|_| SourceJournalError::Binding)?;
        if wire::record_argument_digest(state) != *state_digest {
            return Err(SourceJournalError::Binding);
        }
        let scope = &context.registration().expected_facts().scope;
        let carrier = channel_from_json(proposal).map_err(|_| SourceJournalError::Binding)?;
        let checked_proposal =
            bind_recovered_owned_wait_proposal_v8(binding, scope, &carrier, proposal_digest)
                .map_err(|_| SourceJournalError::Binding)?;
        let scope = serde_json::json!({
            "program_root": scope.program_root(),
            "invocation": scope.invocation_id(),
            "policy_epoch": scope.policy_epoch()
        });
        let expected_transfer_digest = wire::recipe_digest(
            wire::RecipeV8::Transfer,
            &serde_json::json!({
                "scope":scope,
                "generation":self.generation,
                "turn":0,
                "attempt":0,
                "wait":wait,
                "from":from,
                "to":to,
                "state_digest":state_digest,
                "proposal_digest":proposal_digest
            }),
        )?;
        if *from != binding.helper().function().id.as_str()
            || *to != binding.authorize().function().id.as_str()
            || *transfer_digest != expected_transfer_digest
        {
            return Err(SourceJournalError::Binding);
        }
        capacity::outstanding(self.context.fold(), &folded)?
            .check(self.document.len(), self.entries.len())?;
        Ok(AuthorizationRecoveryFactsV8 {
            state: state.clone(),
            proposal: checked_proposal,
            wait: wait.clone(),
            transfer,
            state_digest: state_digest.clone(),
            proposal_digest: proposal_digest.clone(),
            sequence: self.sequence(),
            bytes: self.acknowledged_bytes(),
            authentication: self.authentication_tail().to_owned(),
            document_digest: crate::live_invocation::identity::digest(
                b"semaprax.source-agent-owned-wait.recovery-prefix.v1\0",
                &self.document,
            ),
        })
    }
}
