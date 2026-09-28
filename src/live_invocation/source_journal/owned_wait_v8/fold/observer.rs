//! Authenticated observer-failure State history only; no owner or cleanup grant.
use super::*;
use crate::resumable_effects::owned_frame::v2;

pub(in crate::live_invocation::source_journal::owned_wait_v8) struct ObserverStateFoldV8 {
    turn: u32,
    attempt: u32,
    started: u32,
    operations: Value,
    target_failed: bool,
    settled: Option<u32>,
    observed: bool,
    stopped: bool,
}
pub(super) fn owned(
    context: &FoldContextV8,
    f: &mut FoldV8,
    b: &Body,
    seq: u32,
) -> Result<bool, SourceJournalError> {
    match b {
        Body::OwnedEffectObserverFailureStateCleanupStarted {
            turn,
            attempt,
            plan,
            settlement,
            recorded,
            decision_cleanup_settled,
            decision_receipt_digest,
            cause,
            selected_effect_failure,
            state_digest,
            operations,
        } => {
            require(
                f.tail == TailV8::EffectCleanupFailed
                    && f.failure_selected
                    && f.reduce.is_none()
                    && f.failed_effect_state.is_none()
                    && f.observer_state.is_none(),
            )?;
            require(
                *turn == 0
                    && f.wait.as_ref().is_some_and(|w| w.attempt == *attempt)
                    && plan == context.checked_binding.binding()
                    && cause == "decision_observation_failed",
            )?;
            let effect = f.effect.as_ref().ok_or(SourceJournalError::Order)?;
            require(
                effect.settlement == Some(*settlement)
                    && effect.recorded == Some(*recorded)
                    && effect.cleanup_started.and_then(|s| s.checked_add(1))
                        == Some(*decision_cleanup_settled)
                    && decision_cleanup_settled.checked_add(1) == Some(seq),
            )?;
            require(
                effect.cleanup_receipt_digest.as_deref() == Some(decision_receipt_digest)
                    && f.state_digest.as_deref() == Some(state_digest),
            )?;
            let target = f
                .ordinary_sequences
                .iter()
                .position(|s| s == settlement)
                .and_then(|i| f.ordinary.get(i))
                .ok_or(SourceJournalError::Order)?;
            match target {
                SourceJournalEntry::EffectObserved {
                    turn: t,
                    attempt: a,
                    ..
                } => require(
                    t == turn
                        && a == attempt
                        && effect.observed
                        && selected_effect_failure.is_none(),
                )?,
                SourceJournalEntry::EffectFailed {
                    turn: t,
                    attempt: a,
                    reason,
                    ..
                } => require(
                    t == turn
                        && a == attempt
                        && !effect.observed
                        && matches!(
                            reason,
                            super::super::super::SourceEffectFailure::HandlerFailed
                                | super::super::super::SourceEffectFailure::ResultLimit
                        )
                        && selected_effect_failure.as_deref() == Some(reason.as_str()),
                )?,
                _ => return order(),
            }
            v2::validate_owned_wait_operations_v8(
                &context.checked_binding.helper().liveness().result_disposal,
                operations,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            f.observer_state = Some(ObserverStateFoldV8 {
                turn: *turn,
                attempt: *attempt,
                started: seq,
                operations: operations.clone(),
                target_failed: selected_effect_failure.is_some(),
                settled: None,
                observed: false,
                stopped: false,
            });
        }
        Body::OwnedEffectObserverFailureStateCleanupSettled {
            turn,
            attempt,
            started,
            receipt,
        } => {
            require(f.tail == TailV8::ObserverFailureState)?;
            let o = f.observer_state.as_mut().ok_or(SourceJournalError::Order)?;
            require(
                (*turn, *attempt) == (o.turn, o.attempt)
                    && *started == o.started
                    && started.checked_add(1) == Some(seq)
                    && o.settled.is_none(),
            )?;
            v2::validate_owned_wait_observed_receipt_v8(&o.operations, receipt)
                .map_err(|_| SourceJournalError::Binding)?;
            o.settled = Some(seq);
            o.observed = receipt["settlement"] == "completed";
        }
        _ => {
            require(f.observer_state.is_none())?;
            return Ok(false);
        }
    }
    f.tail = TailV8::ObserverFailureState;
    Ok(true)
}
pub(super) fn ordinary(
    f: &mut FoldV8,
    row: &SourceJournalEntry,
    seq: u32,
) -> Result<bool, SourceJournalError> {
    let Some(o) = f.observer_state.as_mut() else {
        return Ok(false);
    };
    if o.stopped {
        return Ok(false);
    }
    let SourceJournalEntry::Stop {
        turn,
        attempt,
        status,
        reason,
    } = row
    else {
        return order();
    };
    require(
        f.tail == TailV8::ObserverFailureState
            && o.observed
            && !o.stopped
            && (*turn, *attempt) == (Some(o.turn), Some(o.attempt))
            && o.settled.and_then(|s| s.checked_add(1)) == Some(seq),
    )?;
    use super::super::super::{SourceStopReason as R, SourceStopStatus as S};
    require(
        (*status, *reason)
            == if o.target_failed {
                (S::EffectFailed, R::EffectFailed)
            } else {
                (S::Rejected, R::StageRefused)
            },
    )?;
    o.stopped = true;
    f.tail = TailV8::Stopped;
    f.effect = None;
    f.state_basis = None;
    f.state = None;
    f.state_digest = None;
    f.transfer = None;
    f.decision = None;
    Ok(true)
}

pub(super) fn is_row(row: &EntryV8) -> bool {
    matches!(
        row,
        EntryV8::Owned(
            Body::OwnedEffectObserverFailureStateCleanupStarted { .. }
                | Body::OwnedEffectObserverFailureStateCleanupSettled { .. }
        ) | EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
    )
}
impl ObserverStateFoldV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn coordinates(
        &self,
    ) -> (u32, u32) {
        (self.turn, self.attempt)
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn operations(&self) -> &Value {
        &self.operations
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn settled(&self) -> bool {
        self.settled.is_some()
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn observed(&self) -> bool {
        self.observed
    }
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn stopped(&self) -> bool {
        self.stopped
    }
}

pub(super) fn is_owned_row(row: &EntryV8) -> bool {
    matches!(
        row,
        EntryV8::Owned(
            Body::OwnedEffectObserverFailureStateCleanupStarted { .. }
                | Body::OwnedEffectObserverFailureStateCleanupSettled { .. }
        )
    )
}
pub(super) fn extends_effect(tail: TailV8, row: &EntryV8) -> bool {
    is_row(row)
        && matches!(
            tail,
            TailV8::EffectCleanupFailed | TailV8::ObserverFailureState
        )
}
impl FoldV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn observer_state_fold(
        &self,
    ) -> Option<&ObserverStateFoldV8> {
        self.observer_state.as_ref()
    }
}
