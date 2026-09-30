//! Pure effect causal metadata; no live obligation or dispatch authority.
use super::*;
use crate::resumable_effects::owned_frame::v2;
use serde_json::json;

#[derive(Clone, Debug)]
pub(super) struct EffectV8 {
    pub consumed: u32,
    pub intent: Option<u32>,
    pub operation: Option<String>,
    pub settlement: Option<u32>,
    pub observed: bool,
    pub recorded: Option<u32>,
    pub cleanup_started: Option<u32>,
    pub operations: Option<Value>,
    pub cleanup_receipt_digest: Option<String>,
}
impl EffectV8 {
    pub(super) fn consumed(seq: u32) -> Self {
        Self {
            consumed: seq,
            intent: None,
            operation: None,
            settlement: None,
            observed: false,
            recorded: None,
            cleanup_started: None,
            operations: None,
            cleanup_receipt_digest: None,
        }
    }
}
fn coordinates(f: &FoldV8, turn: u32, attempt: u32) -> Result<(), SourceJournalError> {
    require(turn == f.current_turn && f.wait.as_ref().is_some_and(|w| w.attempt == attempt))
}
pub(super) fn ordinary(
    f: &mut FoldV8,
    row: &SourceJournalEntry,
    seq: u32,
) -> Result<(), SourceJournalError> {
    use SourceJournalEntry as E;
    match row {
        E::EffectIntent {
            turn,
            attempt,
            operation,
            ..
        } => {
            coordinates(f, *turn, *attempt)?;
            require(f.tail == TailV8::ReadyPair && !f.failure_selected)?;
            let effect = f.effect.as_mut().ok_or(SourceJournalError::Order)?;
            require(effect.intent.is_none())?;
            effect.intent = Some(seq);
            effect.operation = Some(operation.clone());
            f.tail = TailV8::EffectInDoubt;
        }
        E::EffectObserved {
            turn,
            attempt,
            operation,
            ..
        }
        | E::EffectFailed {
            turn,
            attempt,
            operation,
            ..
        } => {
            coordinates(f, *turn, *attempt)?;
            require(f.tail == TailV8::EffectInDoubt)?;
            let effect = f.effect.as_mut().ok_or(SourceJournalError::Order)?;
            require(
                effect.intent.is_some_and(|i| i.checked_add(1) == Some(seq))
                    && effect.settlement.is_none()
                    && effect.operation.as_ref() == Some(operation),
            )?;
            effect.settlement = Some(seq);
            effect.observed = matches!(row, E::EffectObserved { .. });
            f.failure_selected = !effect.observed;
            f.tail = TailV8::EffectSettlementUncommitted;
        }
        _ => return order(),
    }
    Ok(())
}
pub(super) fn owned(
    context: &FoldContextV8,
    f: &mut FoldV8,
    row: &Body,
    seq: u32,
) -> Result<(), SourceJournalError> {
    match row {
        Body::OwnedEffectSettlementRecorded {
            turn,
            attempt,
            intent,
            settlement,
            ..
        } => {
            coordinates(f, *turn, *attempt)?;
            require(f.tail == TailV8::EffectSettlementUncommitted)?;
            let effect = f.effect.as_mut().ok_or(SourceJournalError::Order)?;
            require(
                effect.intent == Some(*intent)
                    && effect.settlement == Some(*settlement)
                    && settlement.checked_add(1) == Some(seq)
                    && effect.recorded.is_none(),
            )?;
            effect.recorded = Some(seq);
            f.tail = TailV8::EffectSettled;
        }
        Body::OwnedEffectDecisionCleanupStarted {
            turn,
            attempt,
            staged,
            ready,
            consumed,
            intent,
            settlement,
            recorded,
            decision_digest,
            operations,
            operations_digest,
        } => {
            coordinates(f, *turn, *attempt)?;
            require(f.tail == TailV8::EffectSettled)?;
            let d = f.decision.as_ref().ok_or(SourceJournalError::Order)?;
            require(
                d.granted
                    && d.staged == *staged
                    && d.ready == Some(*ready)
                    && d.digest == *decision_digest,
            )?;
            let effect = f.effect.as_mut().ok_or(SourceJournalError::Order)?;
            require(
                effect.consumed == *consumed
                    && effect.intent == Some(*intent)
                    && effect.settlement == Some(*settlement)
                    && effect.recorded == Some(*recorded)
                    && recorded.checked_add(1) == Some(seq)
                    && effect.cleanup_started.is_none(),
            )?;
            let authorize = context.checked_binding.authorize();
            let actions: Vec<_> = authorize
                .disposal()
                .iter()
                .filter(|a| {
                    a.active_case
                        .as_ref()
                        .is_some_and(|c| c.case == *authorize.granted())
                })
                .cloned()
                .collect();
            v2::validate_owned_wait_operations_v8(&actions, operations)
                .map_err(|_| SourceJournalError::Binding)?;
            let commitment = json!({"turn":turn,"attempt":attempt,"staged":staged,
                "ready":ready,"consumed":consumed,"intent":intent,"settlement":settlement,
                "recorded":recorded,"decision_digest":decision_digest,"operations":operations});
            require(
                wire::recipe_digest(wire::RecipeV8::EffectDecisionOperations, &commitment)?
                    == *operations_digest,
            )?;
            effect.cleanup_started = Some(seq);
            effect.operations = Some(operations.clone());
            f.tail = TailV8::EffectCleanupInDoubt;
        }
        Body::OwnedEffectDecisionCleanupSettled {
            turn,
            attempt,
            started,
            receipt,
            receipt_digest,
        } => {
            coordinates(f, *turn, *attempt)?;
            require(f.tail == TailV8::EffectCleanupInDoubt && started.checked_add(1) == Some(seq))?;
            let effect = f.effect.as_ref().ok_or(SourceJournalError::Order)?;
            require(effect.cleanup_started == Some(*started))?;
            v2::validate_owned_wait_observed_receipt_v8(
                effect
                    .operations
                    .as_ref()
                    .ok_or(SourceJournalError::Order)?,
                receipt,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            require(wire::recipe_digest(wire::RecipeV8::Receipt, receipt)? == *receipt_digest)?;
            f.tail = if receipt["settlement"] != "completed" {
                f.failure_selected = true;
                TailV8::EffectCleanupFailed
            } else if effect.observed {
                TailV8::EffectDecisionReleased
            } else {
                TailV8::EffectFailedState
            };
            f.effect
                .as_mut()
                .ok_or(SourceJournalError::Order)?
                .cleanup_receipt_digest = Some(receipt_digest.clone());
            f.decision = None;
        }
        _ => return order(),
    }
    Ok(())
}
/// Until a separately reviewed live one-use obligation overload exists, the
/// generic raw candidate path cannot produce any of these effect continuations.
pub(super) fn is_effect_row(row: &EntryV8) -> bool {
    matches!(
        row,
        EntryV8::Ordinary(
            SourceJournalEntry::EffectIntent { .. }
                | SourceJournalEntry::EffectObserved { .. }
                | SourceJournalEntry::EffectFailed { .. }
        ) | EntryV8::Owned(
            Body::OwnedEffectSettlementRecorded { .. }
                | Body::OwnedEffectDecisionCleanupStarted { .. }
                | Body::OwnedEffectDecisionCleanupSettled { .. }
        )
    )
}
