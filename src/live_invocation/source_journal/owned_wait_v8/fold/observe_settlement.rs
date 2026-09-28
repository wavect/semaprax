//! Profile-only descriptive Observe consumption; no owner or ACK authority.
use super::*;
use crate::resumable_effects::owned_frame::v2;
use model::ObserveSettlementV8;

pub(super) struct ObserveSettlementFactsV8 {
    sequence: u32,
    ordinary_state: String,
    observation: Option<String>,
    failure: Option<Value>,
}

pub(super) fn settle(
    context: &FoldContextV8,
    folded: &mut FoldV8,
    body: &Body,
    sequence: u32,
) -> Result<bool, SourceJournalError> {
    let Body::OwnedObserveSettled {
        turn,
        reservation,
        state_digest,
        consumed,
        settlement,
    } = body
    else {
        return Ok(false);
    };
    require(
        context.cumulative_initialization
            && folded.continuation_profile_selected
            && context.initialized_task.is_some()
            && folded.current_turn == *turn
            && folded.tail == TailV8::ObserveReserved
            && !folded.failure_selected
            && folded.observe_settlement.is_none()
            && folded.state_digest.as_ref() == Some(state_digest),
    )?;
    let (original, role, fuel) = folded.stage_current.ok_or(SourceJournalError::Order)?;
    require(
        original == *reservation
            && role == SourceStageRole::Observe
            && *consumed <= fuel
            && original.checked_add(1) == Some(sequence),
    )?;
    let state = folded.state.as_ref().ok_or(SourceJournalError::Order)?;
    let ordinary_state = v2::owned_wait_ordinary_state_digest_v8(&context.checked_binding, state)
        .map_err(|_| SourceJournalError::Binding)?;
    let (observation, failure) = match settlement {
        ObserveSettlementV8::Observed {
            observation,
            observation_digest,
        } => {
            require(
                wire::canonical(observation).len() <= super::super::super::MAX_SOURCE_CARRIER_BYTES,
            )?;
            let channel = crate::interpreter::resumable::checkpoint::channel_from_json(observation)
                .map_err(|_| SourceJournalError::Binding)?;
            let Body::OwnedRunCreated { scope, .. } = &context.created else {
                return order();
            };
            let scope = crate::resumable_effects::source_checkpoint::SourceCheckpointScope::new(
                scope["program_root"]
                    .as_str()
                    .ok_or(SourceJournalError::Binding)?,
                scope["invocation"]
                    .as_str()
                    .ok_or(SourceJournalError::Binding)?,
                scope["policy_epoch"]
                    .as_u64()
                    .ok_or(SourceJournalError::Binding)?,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            let checked =
                v2::bind_owned_wait_observation_v8(&context.checked_binding, &scope, &channel)
                    .map_err(|_| SourceJournalError::Binding)?;
            require(checked.ordinary_digest() == observation_digest)?;
            (Some(observation_digest.clone()), None)
        }
        ObserveSettlementV8::Failed { status } => {
            require(
                wire::canonical(status).len() <= super::super::super::MAX_SOURCE_CARRIER_BYTES,
            )?;
            v2::validate_owned_wait_failure_v8(
                &context.checked_binding,
                &context.checked_binding.observe().function().id,
                status,
            )
            .map_err(|_| SourceJournalError::Binding)?;
            require(matches!(
                status["failure"].as_str(),
                Some(
                    "language_failure"
                        | "fuel_exhausted"
                        | "call_depth_exceeded"
                        | "host_abandoned"
                        | "evaluation_rejected"
                )
            ))?;
            (None, Some(status.clone()))
        }
    };
    folded.consume(*consumed, fuel)?;
    if let Some(status) = &failure {
        folded.failure_selected = true;
        folded.cleanup_terminal = Some(status.clone());
        folded.tail = TailV8::FailedState;
    } else {
        folded.tail = TailV8::ObserveSettled;
    }
    folded.observe_settlement = Some(ObserveSettlementFactsV8 {
        sequence,
        ordinary_state,
        observation,
        failure,
    });
    Ok(true)
}

pub(super) fn validate_cleanup(
    context: &FoldContextV8,
    folded: &FoldV8,
    body: &Body,
) -> Result<(), SourceJournalError> {
    if !context.cumulative_initialization {
        return Ok(());
    }
    if matches!(body, Body::OwnedCleanupStarted { .. }) && folded.tail == TailV8::ObserveReserved {
        return order();
    }
    let Some(settled) = &folded.observe_settlement else {
        return Ok(());
    };
    let Some(failure) = &settled.failure else {
        return Ok(());
    };
    let actual = &context
        .checked_binding
        .observe()
        .helper()
        .liveness()
        .failure_cleanup;
    match body {
        Body::OwnedCleanupStarted {
            owner,
            terminal,
            operations,
            ..
        } => {
            require(*owner == OwnerV8::State && terminal == failure)?;
            v2::validate_owned_wait_operations_v8(actual, operations)
                .map_err(|_| SourceJournalError::Binding)?;
        }
        Body::OwnedCleanupSettled { owner, receipt, .. } => {
            require(*owner == OwnerV8::State)?;
            let operations =
                v2::owned_wait_operations_v8(actual).map_err(|_| SourceJournalError::Binding)?;
            v2::validate_owned_wait_observed_receipt_v8(&operations, receipt)
                .map_err(|_| SourceJournalError::Binding)?;
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn validate_observed(
    context: &FoldContextV8,
    folded: &FoldV8,
    sequence: u32,
    state: &str,
    observation: &str,
) -> Result<(), SourceJournalError> {
    if !context.cumulative_initialization {
        return require(
            folded.tail == TailV8::ObserveReserved && folded.observe_settlement.is_none(),
        );
    }
    let settled = folded
        .observe_settlement
        .as_ref()
        .ok_or(SourceJournalError::Order)?;
    require(
        folded.tail == TailV8::ObserveSettled
            && settled.sequence.checked_add(1) == Some(sequence)
            && settled.failure.is_none()
            && settled.ordinary_state == state
            && settled.observation.as_deref() == Some(observation),
    )
}

pub(super) fn validate_stop(
    folded: &FoldV8,
    status: super::super::super::SourceStopStatus,
    reason: super::super::super::SourceStopReason,
) -> Result<(), SourceJournalError> {
    let Some(failure) = folded
        .observe_settlement
        .as_ref()
        .and_then(|settled| settled.failure.as_ref())
    else {
        return Ok(());
    };
    require(
        folded.tail == TailV8::MetadataOnly
            && folded.state_basis.is_none()
            && folded.cleanup.as_ref().is_some_and(|cleanup| {
                cleanup.settled && !cleanup.host_confirmed && cleanup.completed
            }),
    )?;
    let budget = matches!(
        failure["failure"].as_str(),
        Some("fuel_exhausted" | "call_depth_exceeded")
    );
    require(if budget {
        status == super::super::super::SourceStopStatus::BudgetExhausted
            && reason == super::super::super::SourceStopReason::BudgetExhausted
    } else {
        status == super::super::super::SourceStopStatus::Rejected
            && reason == super::super::super::SourceStopReason::StageRefused
    })
}

#[cfg(test)]
mod tests;
