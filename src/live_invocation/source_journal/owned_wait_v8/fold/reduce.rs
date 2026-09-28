//! Authenticated-parent glue for the inert first-turn Reduce inventory.
//! No evaluator, physical ACK, restoration, next-turn or terminal delivery API.
use super::super::reduce_fold::{FailedEffectStateFoldV8, ReduceFoldV8};
use super::*;
use crate::resumable_effects::owned_frame::v2;

pub(super) struct ReduceJournalV8 {
    plan: v2::CheckedOwnedReduceV2,
    fold: ReduceFoldV8,
}
impl ReduceJournalV8 {
    pub(super) fn plan(&self) -> &v2::CheckedOwnedReduceV2 {
        &self.plan
    }
    pub(super) fn fold(&self) -> &ReduceFoldV8 {
        &self.fold
    }
}
fn scope(context: &FoldContextV8) -> Result<&Value, SourceJournalError> {
    match &context.created {
        Body::OwnedRunCreated { scope, binding, .. }
            if binding == context.checked_binding.binding() =>
        {
            Ok(scope)
        }
        _ => Err(SourceJournalError::Binding),
    }
}
fn coordinates(f: &FoldV8, turn: u32, attempt: u32) -> Result<(), SourceJournalError> {
    require(turn == 0 && f.wait.as_ref().is_some_and(|w| w.attempt == attempt))
}
/// These references have already been joined by effect_fold to the actual
/// authenticated Recorded row and a complete observed Decision receipt. Never
/// accept a supplied settlement or cleanup reference as an independent seed.
fn effect_refs(f: &FoldV8, observed: bool) -> Result<(u32, u32, u32), SourceJournalError> {
    let e = f.effect.as_ref().ok_or(SourceJournalError::Order)?;
    require(e.observed == observed)?;
    let settled = e
        .cleanup_started
        .and_then(|s| s.checked_add(1))
        .ok_or(SourceJournalError::Order)?;
    Ok((
        e.settlement.ok_or(SourceJournalError::Order)?,
        e.recorded.ok_or(SourceJournalError::Order)?,
        settled,
    ))
}
fn active(f: &FoldV8) -> bool {
    f.reduce.is_some() || f.failed_effect_state.is_some()
}
fn reduce_mut(f: &mut FoldV8) -> Result<&mut ReduceJournalV8, SourceJournalError> {
    require(f.tail == TailV8::Reduce && f.failed_effect_state.is_none())?;
    f.reduce.as_mut().ok_or(SourceJournalError::Order)
}
fn recorded(f: &mut FoldV8, delta: u64) -> Result<(), SourceJournalError> {
    add(&mut f.consumed_recorded, delta)
}
pub(super) fn owned(
    context: &FoldContextV8,
    f: &mut FoldV8,
    b: &Body,
    seq: u32,
) -> Result<bool, SourceJournalError> {
    match b {
        Body::OwnedReduceStaged {
            turn,
            attempt,
            plan,
            stage_reservation,
            effect_cleanup_settled,
            step,
            step_digest,
            consumed,
        } => {
            let r = reduce_mut(f)?;
            let delta = r.fold.staged(
                &r.plan,
                plan,
                scope(context)?,
                seq,
                *turn,
                *attempt,
                *stage_reservation,
                *effect_cleanup_settled,
                step,
                step_digest,
                *consumed,
            )?;
            recorded(f, delta)?;
        }
        Body::OwnedReduceCleanupStarted {
            turn,
            attempt,
            plan,
            stage_reservation,
            effect_cleanup_settled,
            basis,
            basis_digest,
            consumed,
            operations,
        } => {
            let r = reduce_mut(f)?;
            let delta = r.fold.cleanup_started(
                &r.plan,
                plan,
                scope(context)?,
                seq,
                *turn,
                *attempt,
                *stage_reservation,
                *effect_cleanup_settled,
                basis,
                basis_digest,
                *consumed,
                operations,
            )?;
            recorded(f, delta)?;
        }
        Body::OwnedReduceCleanupSettled {
            turn,
            attempt,
            started,
            receipt,
        } => {
            reduce_mut(f)?
                .fold
                .cleanup_settled(seq, *turn, *attempt, *started, receipt)?;
        }
        Body::OwnedStepTransferReserved {
            turn,
            attempt,
            plan,
            stage_reservation,
            staged,
            cleanup,
            case,
        } => {
            let r = reduce_mut(f)?;
            r.fold.transfer_reserved(
                &r.plan,
                plan,
                seq,
                *turn,
                *attempt,
                *stage_reservation,
                *staged,
                cleanup,
                case,
            )?;
        }
        Body::OwnedStepTransferCompleted {
            turn,
            attempt,
            reserved,
            target,
            transfer_digest,
        } => {
            let target = serde_json::to_value(target).map_err(|_| SourceJournalError::Malformed)?;
            let r = reduce_mut(f)?;
            r.fold.transfer_completed(
                &r.plan,
                scope(context)?,
                seq,
                *turn,
                *attempt,
                *reserved,
                &target,
                transfer_digest,
            )?;
        }
        Body::OwnedEffectFailureStateCleanupStarted {
            turn,
            attempt,
            plan,
            settlement,
            recorded,
            decision_cleanup_settled,
            effect_failure,
            state_digest,
            operations,
        } => {
            require(f.tail == TailV8::EffectFailedState && !active(f) && f.failure_selected)?;
            coordinates(f, *turn, *attempt)?;
            let (expected_settlement, expected_recorded, expected_cleanup) = effect_refs(f, false)?;
            require(expected_cleanup.checked_add(1) == Some(seq))?;
            let failure = f
                .ordinary_sequences
                .iter()
                .position(|s| *s == expected_settlement)
                .and_then(|i| f.ordinary.get(i))
                .and_then(|e| match e {
                    SourceJournalEntry::EffectFailed {
                        turn: t,
                        attempt: a,
                        reason,
                        ..
                    } if t == turn && a == attempt => Some(*reason),
                    _ => None,
                })
                .ok_or(SourceJournalError::Order)?;
            let digest = f.state_digest.as_deref().ok_or(SourceJournalError::Order)?;
            let mut failed = FailedEffectStateFoldV8::after_checked_effect_failure(
                &context.checked_binding,
                scope(context)?,
                *turn,
                *attempt,
                expected_settlement,
                expected_recorded,
                expected_cleanup,
                failure,
                digest,
            )?;
            failed.cleanup_started(
                plan,
                scope(context)?,
                seq,
                *turn,
                *attempt,
                *settlement,
                *recorded,
                *decision_cleanup_settled,
                effect_failure
                    .as_str()
                    .ok_or(SourceJournalError::Malformed)?,
                state_digest,
                operations,
            )?;
            f.failed_effect_state = Some(failed);
        }
        Body::OwnedEffectFailureStateCleanupSettled {
            turn,
            attempt,
            started,
            receipt,
        } => {
            require(f.tail == TailV8::Reduce && f.reduce.is_none())?;
            f.failed_effect_state
                .as_mut()
                .ok_or(SourceJournalError::Order)?
                .cleanup_settled(seq, *turn, *attempt, *started, receipt)?;
        }
        _ => {
            require(!active(f))?;
            return Ok(false);
        }
    }
    if f.reduce.as_ref().is_some_and(|r| {
        r.fold.failure().is_some()
            || r.fold.tail() == super::super::reduce_fold::ReduceTailV8::Quarantined
    }) {
        f.failure_selected = true;
    }
    f.tail = TailV8::Reduce;
    Ok(true)
}
/// Parent retains every handled ordinary row/true seq in its common epilogue.
/// Reservation charging uses the existing aggregate machinery exactly once.
pub(super) fn ordinary(
    context: &FoldContextV8,
    f: &mut FoldV8,
    e: &SourceJournalEntry,
    seq: u32,
) -> Result<bool, SourceJournalError> {
    match e {
        SourceJournalEntry::StageReservation {
            turn,
            attempt,
            role: SourceStageRole::Reduce,
            fuel,
        } => {
            require(f.tail == TailV8::EffectDecisionReleased && !active(f) && !f.failure_selected)?;
            let attempt = attempt.ok_or(SourceJournalError::Order)?;
            coordinates(f, *turn, attempt)?;
            let (_, _, cleanup) = effect_refs(f, true)?;
            require(cleanup.checked_add(1) == Some(seq))?;
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding)
                .map_err(|_| SourceJournalError::Binding)?;
            let allowance = u64::try_from(*fuel).map_err(|_| SourceJournalError::Capacity)?;
            let fold = ReduceFoldV8::after_checked_reservation(
                &plan,
                scope(context)?,
                *turn,
                attempt,
                seq,
                allowance,
                cleanup,
            )?;
            f.reserve(context, allowance, true, false)?;
            f.stage_originals
                .push((seq, SourceStageRole::Reduce, allowance));
            f.stage_current = Some((seq, SourceStageRole::Reduce, allowance));
            f.reduce = Some(ReduceJournalV8 { plan, fold });
            f.tail = TailV8::Reduce;
        }
        SourceJournalEntry::Transition {
            turn,
            attempt,
            case,
            carrier_digest,
        } if active(f) => {
            let r = reduce_mut(f)?;
            r.fold
                .transition(&r.plan, seq, *turn, *attempt, *case, carrier_digest)?;
        }
        SourceJournalEntry::Stop {
            turn,
            attempt,
            status,
            reason,
        } if active(f) => {
            require(f.tail == TailV8::Reduce)?;
            let turn = turn.ok_or(SourceJournalError::Order)?;
            let attempt = attempt.ok_or(SourceJournalError::Order)?;
            if let Some(r) = f.reduce.as_mut() {
                r.fold.stop(seq, turn, attempt, *status, *reason)?;
            } else {
                f.failed_effect_state
                    .as_mut()
                    .ok_or(SourceJournalError::Order)?
                    .stop(seq, turn, attempt, *status, *reason)?;
            }
            // Successfully observed failed-root cleanup retires all earlier bases.
            f.state_basis = None;
            f.state = None;
            f.state_digest = None;
            f.transfer = None;
            f.decision = None;
        }
        _ => {
            require(!active(f))?;
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "reduce/tests.rs"]
mod tests;
