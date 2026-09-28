//! Private phase-specific closure room; acknowledged payloads are not reserved twice.
mod templates;
use super::*;
use fold::{FoldV8, TailV8};
use model::{OwnerV8, PhaseV8};
use serde_json::json;

pub(super) struct ClosureFactsV8<'a> {
    pub intent: bool,
    pub model_failed: bool,
    pub response_closed: bool,
    pub usage_closed: bool,
    pub pending_historical: bool,
    pub cleanup_owner: Option<OwnerV8>,
    pub cleanup_operations: Option<&'a Value>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RoomV8 {
    bytes: usize,
    rows: usize,
}
impl RoomV8 {
    fn add(self, other: Self) -> Result<Self, SourceJournalError> {
        Ok(Self {
            bytes: self
                .bytes
                .checked_add(other.bytes)
                .ok_or(SourceJournalError::Capacity)?,
            rows: self
                .rows
                .checked_add(other.rows)
                .ok_or(SourceJournalError::Capacity)?,
        })
    }
    fn either(self, other: Self) -> Self {
        Self {
            bytes: self.bytes.max(other.bytes),
            rows: self.rows.max(other.rows),
        }
    }
    pub(super) fn check(self, bytes: usize, rows: usize) -> Result<(), SourceJournalError> {
        if bytes
            .checked_add(self.bytes)
            .is_none_or(|n| n > super::super::MAX_SOURCE_DOCUMENT_BYTES)
            || rows
                .checked_add(self.rows)
                .is_none_or(|n| n > super::super::MAX_SOURCE_ENTRIES)
        {
            Err(SourceJournalError::Capacity)
        } else {
            Ok(())
        }
    }
}
fn row(mut value: Value) -> Result<RoomV8, SourceJournalError> {
    let object = value.as_object_mut().ok_or(SourceJournalError::Binding)?;
    for (key, value) in [
        ("schema", json!(SCHEMA)),
        ("invocation", json!(hash())),
        ("generation", json!(hash())),
        ("seq", json!(u32::MAX)),
        ("prev_mac", json!("f".repeat(64))),
        ("authentication", json!("f".repeat(64))),
    ] {
        object.insert(key.into(), value);
    }
    Ok(RoomV8 {
        bytes: wire::canonical(&value)
            .len()
            .checked_add(1)
            .ok_or(SourceJournalError::Capacity)?,
        rows: 1,
    })
}
fn ordinary(entry: SourceJournalEntry) -> Result<RoomV8, SourceJournalError> {
    row(wire::parse(
        super::super::wire::encode_entry(&entry, u32::MAX as usize).as_bytes(),
    )?)
}
fn hash() -> String {
    format!("sha256:{}", "f".repeat(64))
}
fn sum(items: &[RoomV8]) -> Result<RoomV8, SourceJournalError> {
    items.iter().try_fold(RoomV8::default(), |a, b| a.add(*b))
}
fn cleanup(
    max: &templates::Maxima,
    owner: OwnerV8,
    operations: &Value,
) -> Result<RoomV8, SourceJournalError> {
    sum(&[
        row(
            json!({"kind":"owned_cleanup_started","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"owner":owner,"basis":u32::MAX,"terminal":max.terminal,"operations":operations,"operations_digest":hash()}),
        )?,
        cleanup_receipt(owner, operations)?,
    ])
}
fn cleanup_receipt(owner: OwnerV8, operations: &Value) -> Result<RoomV8, SourceJournalError> {
    row(
        json!({"kind":"owned_cleanup_settled","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"owner":owner,"started":u32::MAX,"receipt":templates::receipt(operations)?,"receipt_digest":hash()}),
    )
}
/// Derive only from actual checked B and causal inventory. There is no raw
/// future-byte allowance or callback accepting host-shaped size estimates.
pub(super) fn outstanding(
    context: &FoldContextV8,
    folded: &FoldV8,
) -> Result<RoomV8, SourceJournalError> {
    use TailV8::*;
    let facts = folded.capacity_facts();
    if matches!(folded.tail, ReadyPair | Terminal | TerminalInDoubt) {
        return Ok(RoomV8::default());
    }
    let terminal = RoomV8 {
        bytes: super::super::execution::TERMINAL_ROOM_BYTES,
        rows: 2,
    };
    if matches!(folded.tail, Stopped | StopInDoubt) {
        return Ok(RoomV8 {
            rows: 1,
            ..terminal
        });
    }
    if folded.tail == MetadataOnly {
        return Ok(terminal);
    }
    let max = templates::maxima(context)?;
    let state_cleanup = cleanup(&max, OwnerV8::State, &max.state_operations)?.add(terminal)?;
    let decision_cleanup =
        cleanup(&max, OwnerV8::Decision, &max.decision_operations)?.add(state_cleanup)?;
    let partial_cleanup =
        cleanup(&max, OwnerV8::Decision, &max.partial_operations)?.add(state_cleanup)?;
    if folded.tail == CleanupInDoubt {
        return cleanup_receipt(
            facts.cleanup_owner.ok_or(SourceJournalError::Order)?,
            facts.cleanup_operations.ok_or(SourceJournalError::Order)?,
        )?
        .add(if facts.cleanup_owner == Some(OwnerV8::Decision) {
            state_cleanup
        } else {
            terminal
        });
    }
    if matches!(folded.tail, FailedState | PendingStateCleanup) {
        return Ok(state_cleanup);
    }
    if folded.tail == FailedDecisionThenState {
        return Ok(decision_cleanup.either(partial_cleanup));
    }
    if facts.model_failed {
        let pending_usage = if facts.usage_closed {
            RoomV8::default()
        } else {
            ordinary(SourceJournalEntry::AttemptUsage {
                turn: u32::MAX,
                attempt: u32::MAX,
                reported: Some(super::super::SourceReportedUsage {
                    total: Some(u64::MAX),
                    input: Some(u64::MAX),
                    output: Some(u64::MAX),
                    reasoning: Some(u64::MAX),
                    cache_read: Some(u64::MAX),
                    cache_write: Some(u64::MAX),
                }),
            })?
        };
        return pending_usage.add(state_cleanup);
    }
    let consumed = ordinary(SourceJournalEntry::AuthorizationConsumed {
        turn: u32::MAX,
        attempt: u32::MAX,
        grant_digest: hash(),
    })?;
    let ready = row(
        json!({"kind":"owned_authorization_ready","turn":u32::MAX,"attempt":u32::MAX,"staged":u32::MAX,"state_digest":hash(),"decision_digest":hash(),"grant_digest":hash()}),
    )?;
    if folded.tail == ResultDeliveryInDoubt {
        return Ok(consumed);
    }
    if folded.tail == PendingReady {
        return ready.add(consumed);
    }
    let refused = ordinary(SourceJournalEntry::AuthorizationRefused {
        turn: u32::MAX,
        attempt: u32::MAX,
        reason: super::super::SourceAuthorizationRefusal::DeadlineExceeded,
    })?;
    if folded.tail == PendingRefusal {
        return refused.add(if context.refused_cleanup_empty {
            state_cleanup
        } else {
            decision_cleanup
        });
    }
    let staged = row(
        json!({"kind":"owned_authorization_staged","turn":u32::MAX,"attempt":u32::MAX,"stage_reservation":u32::MAX,"transfer":u32::MAX,"state_digest":hash(),"proposal_digest":hash(),"decision":max.decision,"decision_digest":hash(),"consumed":u64::MAX}),
    )?;
    let authorize = staged
        .add(ready)?
        .add(consumed)?
        .either(partial_cleanup)
        .either(staged.add(refused)?.add(if context.refused_cleanup_empty {
            state_cleanup
        } else {
            decision_cleanup
        })?);
    if matches!(folded.tail, PendingAuthorize | ChargedAuthorizeReplay) {
        return Ok(authorize);
    }
    let stage = ordinary(SourceJournalEntry::StageReservation {
        turn: u32::MAX,
        attempt: Some(u32::MAX),
        role: super::super::SourceStageRole::Authorize,
        fuel: context
            .ordinary
            .max_steps_per_stage()
            .ok_or(SourceJournalError::Binding)?,
    })?;
    let transfer_completed = row(
        json!({"kind":"owned_state_transfer_completed","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"reservation":u32::MAX,"state":max.state,"state_digest":hash(),"proposal":max.proposal,"proposal_digest":hash(),"transfer_digest":hash()}),
    )?;
    let transfer_reserved = row(
        json!({"kind":"owned_state_transfer_reserved","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"from":context.helper,"to":context.authorize,"state_digest":hash(),"proposal_digest":hash(),"transfer_digest":hash()}),
    )?;
    let transfer = transfer_reserved
        .add(transfer_completed)?
        .add(stage)?
        .add(authorize)?;
    if folded.tail == TransferReserved {
        return transfer_completed.add(stage)?.add(authorize);
    }
    let admitted = ordinary(SourceJournalEntry::ProposalAdmitted {
        turn: u32::MAX,
        attempt: u32::MAX,
        proposal_digest: hash(),
    })?;
    let retired = row(
        json!({"kind":"owned_wait_retired","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"prepared":u32::MAX,"state_digest":hash(),"observation_digest":hash()}),
    )?;
    let rearmed = row(
        json!({"kind":"owned_state_rearmed","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"retired":u32::MAX,"state":max.state,"state_digest":hash(),"observation":max.observation,"observation_digest":hash()}),
    )?;
    if folded.tail == TransferInDoubt {
        return Ok(rearmed.either(state_cleanup));
    }
    if folded.tail == ProposalRefused {
        return Ok(retired.add(rearmed)?.either(state_cleanup));
    }
    let completed = row(
        json!({"kind":"owned_wait_completed","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"reservation":u32::MAX,"proposal":max.proposal,"proposal_digest":hash(),"result_digest":hash(),"consumed":u64::MAX}),
    )?;
    let replay_checked = row(
        json!({"kind":"owned_wait_replay_checked","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"reservation":u32::MAX,"original":u32::MAX,"result_digest":hash(),"consumed":u64::MAX}),
    )?;
    let pending_replay = if facts.pending_historical {
        replay_checked
    } else {
        RoomV8::default()
    };
    if folded.tail == Admitted {
        return transfer.add(pending_replay);
    }
    // Evaluator failure selects its sticky status before State cleanup. This
    // durable selection row is part of the legal closure, even if another
    // successful branch happens to reserve more bytes.
    let wait_failure = row(json!({"kind":"owned_wait_failed","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"reservation":u32::MAX,"status":max.terminal,"consumed":u64::MAX}))?.add(state_cleanup)?;
    let after_completed = admitted.add(transfer)?.either(wait_failure);
    if folded.tail == Completed {
        return after_completed.add(pending_replay);
    }
    if folded.tail == ResumeReserved {
        return completed.add(after_completed)?.either(wait_failure);
    }
    let resume = row(
        json!({"kind":"owned_wait_reserved","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"phase":PhaseV8::Resume,"replay_of":u32::MAX,"fuel":u64::MAX}),
    )?;
    let usage = ordinary(SourceJournalEntry::AttemptUsage {
        turn: u32::MAX,
        attempt: u32::MAX,
        reported: Some(super::super::SourceReportedUsage {
            total: Some(u64::MAX),
            input: Some(u64::MAX),
            output: Some(u64::MAX),
            reasoning: Some(u64::MAX),
            cache_read: Some(u64::MAX),
            cache_write: Some(u64::MAX),
        }),
    })?;
    let mut future = resume.add(completed)?.add(after_completed)?;
    if !facts.usage_closed {
        future = usage.add(future)?;
    }
    if !facts.response_closed {
        future = ordinary(SourceJournalEntry::AttemptSettled {
            turn: u32::MAX,
            attempt: u32::MAX,
            response: vec![255; context.ordinary.response_limit()],
            response_digest: hash(),
        })?
        .add(future)?;
    }
    if !facts.intent {
        future = ordinary(SourceJournalEntry::AttemptIntent {
            turn: u32::MAX,
            attempt: u32::MAX,
            attempt_digest: hash(),
            request_digest: hash(),
            prompt_digest: hash(),
            request_bytes: super::super::MAX_SOURCE_REQUEST_BYTES,
            reserved_units: i64::MAX,
            response_limit: context.ordinary.response_limit(),
        })?
        .add(future)?;
    }
    if matches!(folded.tail, Settled | ModelDispatchInDoubt | Prepared) {
        return future.add(pending_replay)?.either(wait_failure);
    }
    let prepared = row(
        json!({"kind":"owned_wait_prepared","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"reservation":u32::MAX,"observation_digest":hash(),"checkpoint_digest":hash(),"checkpoint":"ff".repeat(65536),"consumed":u64::MAX}),
    )?;
    future = prepared.add(future)?.either(wait_failure);
    if folded.tail == StartReserved {
        return Ok(future);
    }
    let start = row(
        json!({"kind":"owned_wait_reserved","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"phase":PhaseV8::Start,"replay_of":u32::MAX,"fuel":u64::MAX}),
    )?;
    future = start.add(future)?;
    if folded.tail == WaitCreated {
        return Ok(future);
    }
    let wait_created = row(
        json!({"kind":"owned_wait_created","turn":u32::MAX,"attempt":u32::MAX,"wait":hash(),"plan_digest":context.plan_digest,"cleanup_plan_digest":context.cleanup_plan_digest,"signature":context.signature,"argument_digest":hash(),"copy_arguments":[{"parameter":context.checked_binding.helper().function().params[1].id.as_str(),"value":max.observation}],"copy_arguments_digest":hash()}),
    )?;
    future = wait_created.add(future)?;
    if matches!(folded.tail, Observed | RearmedState) {
        return Ok(future);
    }
    let observed = ordinary(SourceJournalEntry::TurnObserved {
        turn: u32::MAX,
        state: hash(),
        observation: hash(),
        feedback: hash(),
    })?;
    future = observed.add(future)?.either(state_cleanup);
    if folded.tail == ObserveReserved {
        return Ok(future);
    }
    future = ordinary(SourceJournalEntry::StageReservation {
        turn: u32::MAX,
        attempt: Some(u32::MAX),
        role: super::super::SourceStageRole::Observe,
        fuel: context
            .ordinary
            .max_steps_per_stage()
            .ok_or(SourceJournalError::Binding)?,
    })?
    .add(future)?;
    if folded.tail == CommittedState {
        return Ok(future);
    }
    future = row(json!({"kind":"owned_state_committed","turn":u32::MAX,"state":max.state,"argument_digest":hash(),"cleanup_plan_digest":context.cleanup_plan_digest}))?.add(future)?;
    if folded.tail == Opened {
        return Ok(future);
    }
    future = ordinary(SourceJournalEntry::RunOpened)?.add(future)?;
    if folded.tail == Created {
        return Ok(future);
    }
    if folded.tail == Empty {
        return row(
            serde_json::to_value(&context.created).map_err(|_| SourceJournalError::Binding)?
        )
        .and_then(|created| created.add(future));
    }
    Err(SourceJournalError::Order)
}

#[cfg(test)]
impl RoomV8 {
    pub(super) fn bytes_for_inert_test(self) -> usize {
        self.bytes
    }
    pub(super) fn rows_for_inert_test(self) -> usize {
        self.rows
    }
}
