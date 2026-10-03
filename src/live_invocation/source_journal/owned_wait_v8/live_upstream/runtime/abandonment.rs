//! First-turn source Refused cleanup. An ACKed State Started is the only
//! physical disposal authority; failed appends retain the same actual owner.
use super::super::authorize::StagedLiveOwnedRunV8;
use super::*;
use crate::live_invocation::source_journal::owned_wait_v8::model::{OwnedBodyV8, OwnerV8};
use crate::live_invocation::source_journal::{
    SourceAuthorizationRefusal, SourceStopReason, SourceStopStatus,
};
use crate::resumable_effects::owned_frame::v2::{self, CheckedOwnedAgentWaitBindingV8};
use serde_json::{json, Value};

pub(crate) struct LiveRefusedStateCleanupPermitV8<'p, 'j> {
    held: &'p HeldOwnedWaitStoreV8<'j>,
    session: &'p AppendSessionV8<'j>,
    binding: &'p CheckedOwnedAgentWaitBindingV8,
    state: &'p Value,
    decision: &'p Value,
}
impl LiveRefusedStateCleanupPermitV8<'_, '_> {
    pub(crate) fn binding(&self) -> &CheckedOwnedAgentWaitBindingV8 {
        self.binding
    }
    pub(crate) fn validate_current(&self) -> Result<(), SourceJournalError> {
        self.held
            .validate_prefix(self.session.sequence(), self.session.acknowledged_bytes())
    }
    pub(crate) fn validate_actual(
        &self,
        state: &Value,
        decision: &Value,
    ) -> Result<(), SourceJournalError> {
        self.validate_current()?;
        if state != self.state || decision != self.decision {
            return Err(SourceJournalError::Binding);
        }
        Ok(())
    }
}
pub(super) struct RefusedStoppedV8<'j> {
    _owner: Box<StagedLiveOwnedRunV8<'j>>,
    _acks: Vec<AppendSessionV8<'j>>,
}
fn fresh(
    owner: &StagedLiveOwnedRunV8<'_>,
    session: &AppendSessionV8<'_>,
) -> Result<(), SourceJournalError> {
    let c = owner.journal.context().ordinary();
    super::super::model::check_clock_v8(
        &owner.held,
        session.sequence(),
        session.acknowledged_bytes(),
        owner.cancellation,
        owner.clock,
        c.clock_domain(),
        c.initial_millis(),
        c.deadline_millis(),
    )
}
pub(super) fn is_refused(owner: &StagedLiveOwnedRunV8<'_>) -> bool {
    owner
        .journal
        .context()
        .ready_runtime()
        .and_then(|(_, e)| {
            owner
                .owner
                .checked_facts(e.wait())
                .map(|(_, d)| d["case"].as_str() == Some(e.wait().authorize().refused().as_str()))
        })
        .unwrap_or(false)
}
#[inline(never)]
pub(super) fn stop_refused<'j>(
    mut owner: Box<StagedLiveOwnedRunV8<'j>>,
    observe: impl FnMut(&FinalizeAction),
) -> Result<Box<RefusedStoppedV8<'j>>, RunQuarantineV8<'j>> {
    let journal = owner.journal;
    let mut acks: Vec<AppendSessionV8<'j>> = Vec::with_capacity(4);
    macro_rules! checked {
        ($expr:expr) => {
            match $expr {
                Ok(x) => x,
                Err(error) => {
                    return Err(continue_run::quarantine(
                        journal,
                        "refused-state-cleanup",
                        (owner, acks, error),
                    ))
                }
            }
        };
    }
    macro_rules! ack {
        ($row:expr, $incurred:expr) => {{
            let row = $row;
            let current = acks.last().unwrap_or(&owner.session);
            if $incurred {
                checked!(owner
                    .held
                    .validate_prefix(current.sequence(), current.acknowledged_bytes()));
            } else {
                checked!(fresh(&owner, current));
            }
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
    let (state, decision) = checked!(owner
        .owner
        .checked_facts(binding)
        .ok_or(SourceJournalError::Binding));
    if decision["case"].as_str() != Some(binding.authorize().refused().as_str())
        || binding.authorize().disposal().iter().any(|a| {
            a.active_case
                .as_ref()
                .is_none_or(|c| c.case == *binding.authorize().refused())
        })
    {
        return Err(continue_run::quarantine(
            journal,
            "refused-state-cleanup",
            (owner, acks),
        ));
    }
    let wait = checked!(owner
        .session
        .inventory
        .refused_wait(owner.transfer, owner.staged));
    let operations = checked!(v2::owned_wait_operations_v8(
        &binding.helper().liveness().result_disposal
    )
    .map_err(|_| SourceJournalError::Binding));
    // Retain the actual source Refused payload as terminal evidence; neither
    // the payload nor a serialized Started row can construct this permit.
    let terminal = json!({"authorization_refused":decision});
    ack!(
        EntryV8::Ordinary(SourceJournalEntry::AuthorizationRefused {
            turn: 0,
            attempt: 0,
            reason: SourceAuthorizationRefusal::GateDenied
        }),
        false
    );
    let basis = owner.transfer;
    let operations_digest = checked!(wire::recipe_digest(
        wire::RecipeV8::Operations,
        &json!({"owner":"state","basis":basis,"terminal":terminal,"operations":operations})
    ));
    let started = u32::try_from(acks.last().expect("refusal ACK").sequence())
        .expect("bounded journal sequence");
    ack!(
        EntryV8::Owned(OwnedBodyV8::OwnedCleanupStarted {
            turn: 0,
            attempt: Some(0),
            wait: Some(wait.clone()),
            owner: OwnerV8::State,
            basis,
            terminal,
            operations: operations.clone(),
            operations_digest
        }),
        false
    );
    let permit = LiveRefusedStateCleanupPermitV8 {
        held: &owner.held,
        session: acks.last().expect("Started ACK"),
        binding,
        state: &state,
        decision: &decision,
    };
    let receipt = checked!(owner.owner.release_refused_state_v8(&permit, observe));
    checked!(
        v2::validate_owned_wait_observed_receipt_v8(&operations, &receipt)
            .map_err(|_| SourceJournalError::Binding)
    );
    let completed = receipt["settlement"].as_str() == Some("completed");
    let receipt_digest = checked!(wire::recipe_digest(wire::RecipeV8::Receipt, &receipt));
    ack!(
        EntryV8::Owned(OwnedBodyV8::OwnedCleanupSettled {
            turn: 0,
            attempt: Some(0),
            wait: Some(wait),
            owner: OwnerV8::State,
            started,
            receipt,
            receipt_digest
        }),
        true
    );
    if !completed {
        return Err(continue_run::quarantine(
            journal,
            "refused-state-cleanup",
            (owner, acks),
        ));
    }
    ack!(
        EntryV8::Ordinary(SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: SourceStopStatus::Rejected,
            reason: SourceStopReason::StageRefused
        }),
        false
    );
    Ok(Box::new(RefusedStoppedV8 {
        _owner: owner,
        _acks: acks,
    }))
}
