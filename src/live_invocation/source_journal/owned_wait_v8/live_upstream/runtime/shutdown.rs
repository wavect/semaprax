//! Explicit cleanup of the same completed first-model State. No evaluator,
//! dispatch, reservation, transfer or owner reconstruction enters this path.
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::model::{OwnedBodyV8, OwnerV8};
use crate::live_invocation::source_journal::{SourceStopReason, SourceStopStatus};
use crate::resumable_effects::owned_frame::v2::{
    self, CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitProposalV8,
};
use serde_json::{json, Value};

pub(crate) struct LiveCompletedStateShutdownPermitV8<'p, 'j> {
    held: &'p HeldOwnedWaitStoreV8<'j>,
    session: &'p AppendSessionV8<'j>,
    binding: &'p CheckedOwnedAgentWaitBindingV8,
    state: &'p Value,
    proposal: &'p CheckedOwnedWaitProposalV8,
}
impl LiveCompletedStateShutdownPermitV8<'_, '_> {
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.binding
    }
    pub(crate) fn proposal(&self) -> &CheckedOwnedWaitProposalV8 {
        self.proposal
    }
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.held
            .validate_prefix(self.session.sequence(), self.session.acknowledged_bytes())
    }
    pub(crate) fn validate_actual(&self, state: &Value) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        if state != self.state {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
pub(super) struct ShutdownStoppedV8<'j> {
    _owner: Box<CompletedLiveOwnedRunV8<'j>>,
    _acks: Vec<AppendSessionV8<'j>>,
}
#[inline(never)]
fn stop_completed<'j>(
    mut owner: Box<CompletedLiveOwnedRunV8<'j>>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<Box<ShutdownStoppedV8<'j>>, RunQuarantineV8<'j>> {
    let journal = owner.journal;
    let mut acks: Vec<AppendSessionV8<'j>> = Vec::with_capacity(4);
    macro_rules! checked {
        ($expr:expr) => {
            match $expr {
                Ok(x) => x,
                Err(error) => {
                    return Err(continue_run::quarantine(
                        journal,
                        "completed-state-shutdown",
                        (owner, acks, error),
                    ))
                }
            }
        };
    }
    macro_rules! ack {
        ($row:expr) => {{
            let row = $row;
            let current = acks.last().unwrap_or(&owner.session);
            checked!(owner
                .held
                .validate_prefix(current.sequence(), current.acknowledged_bytes()));
            let next = checked!(journal.begin_session());
            let next = checked!(next.append(row));
            acks.push(next);
        }};
    }
    let (_, execution) = checked!(journal
        .context()
        .ready_runtime()
        .ok_or(SourceJournalError::Binding));
    let binding = execution.wait();
    let state = checked!(owner
        .owner
        .checked_facts(binding)
        .ok_or(SourceJournalError::Binding));
    checked!(owner.session.inventory.completed_shutdown_basis(
        owner.completed,
        &owner.wait,
        &state,
        &owner.proposal
    ));
    let operations = checked!(v2::owned_wait_operations_v8(
        &binding.helper().liveness().result_disposal
    )
    .map_err(|_| SourceJournalError::Binding));
    let terminal = json!({"failure":"host_abandoned","language_status":null});
    ack!(EntryV8::Owned(OwnedBodyV8::OwnedWaitFailed {
        turn: 0,
        attempt: 0,
        wait: owner.wait.clone(),
        reservation: None,
        status: terminal.clone(),
        consumed: 0
    }));
    let basis = owner.completed;
    let operations_digest = checked!(wire::recipe_digest(
        wire::RecipeV8::Operations,
        &json!({"owner":"state","basis":basis,"terminal":terminal,"operations":operations})
    ));
    let started = checked!(u32::try_from(acks.last().expect("failure ACK").sequence())
        .map_err(|_| SourceJournalError::Capacity));
    ack!(EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
        turn: 0,
        attempt: Some(0),
        wait: Some(owner.wait.clone()),
        owner: OwnerV8::State,
        basis,
        terminal,
        operations: operations.clone(),
        operations_digest
    }));
    let permit = LiveCompletedStateShutdownPermitV8 {
        held: &owner.held,
        session: acks.last().expect("Started ACK"),
        binding,
        state: &state,
        proposal: &owner.proposal,
    };
    let receipt = checked!(owner.owner.release_completed_state_v8(&permit, observe));
    checked!(
        v2::validate_owned_wait_observed_receipt_v8(&operations, &receipt)
            .map_err(|_| SourceJournalError::Binding)
    );
    let completed = receipt["settlement"].as_str() == Some("completed");
    let receipt_digest = checked!(wire::recipe_digest(wire::RecipeV8::Receipt, &receipt));
    ack!(EntryV8::Owned(OwnedBodyV8::OwnedCleanupSettled {
        turn: 0,
        attempt: Some(0),
        wait: Some(owner.wait.clone()),
        owner: OwnerV8::State,
        started,
        receipt,
        receipt_digest
    }));
    if !completed {
        return Err(continue_run::quarantine(
            journal,
            "completed-state-shutdown",
            (owner, acks),
        ));
    }
    ack!(EntryV8::Ordinary(SourceJournalEntry::Stop {
        turn: Some(0),
        attempt: Some(0),
        status: SourceStopStatus::Cancelled,
        reason: SourceStopReason::Cancelled
    }));
    Ok(Box::new(ShutdownStoppedV8 {
        _owner: owner,
        _acks: acks,
    }))
}
impl OwnedLifecycleRuntimeV8<'_> {
    pub(super) fn shutdown_completed(
        &mut self,
        observe: impl FnMut(&FinalizeAction),
    ) -> OwnedLifecycleStatusV8 {
        if !matches!(self.custody, CustodyV8::ModelCompleted(_)) {
            return self.status();
        }
        let CustodyV8::ModelCompleted(owner) =
            std::mem::replace(&mut self.custody, CustodyV8::InFlight)
        else {
            unreachable!()
        };
        self.custody = match stop_completed(owner, observe) {
            Ok(stopped) => CustodyV8::ShutdownStopped(stopped),
            Err(failure) => CustodyV8::Run(Box::new(failure)),
        };
        self.status()
    }
}
