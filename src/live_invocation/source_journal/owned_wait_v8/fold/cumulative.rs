//! Versioned cumulative-profile admission, inert until actual owner successors.
//! A MAC-bound row and the independently selected checked Context are both needed.
use super::*;

pub(in crate::live_invocation::source_journal::owned_wait_v8) const PROFILE_V1: &str =
    "semaprax.source-agent-owned-wait.cumulative.v1";

pub(in crate::live_invocation::source_journal::owned_wait_v8) fn profile_row(
    context: &FoldContextV8,
) -> Body {
    Body::OwnedContinuationProfileSelected {
        profile: PROFILE_V1.into(),
        max_iterations: context.ordinary.max_iterations(),
    }
}

pub(super) fn select_profile(
    context: &FoldContextV8,
    folded: &mut FoldV8,
    body: &Body,
    sequence: u32,
) -> Result<(), SourceJournalError> {
    require(
        context.cumulative_initialization
            && context.initialized_task.is_some()
            && folded.tail == TailV8::Created
            && !folded.continuation_profile_selected
            && sequence == 1
            && body == &profile_row(context),
    )?;
    folded.continuation_profile_selected = true;
    Ok(())
}

pub(super) fn is_next_state_commit(
    context: &FoldContextV8,
    folded: &FoldV8,
    entry: &EntryV8,
) -> bool {
    context.cumulative_initialization
        && folded.continuation_profile_selected
        && folded.tail == TailV8::Reduce
        && folded.reduce.as_ref().is_some_and(|reduce| {
            reduce.fold().tail() == super::super::reduce_fold::ReduceTailV8::Continued
        })
        && matches!(entry, EntryV8::Owned(Body::OwnedStateCommitted { .. }))
}

/// Conservative legal continuation count, selected only by the checked profile.
/// Unknown Step branches can continue; known terminal/failure branches cannot.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn remaining_turns(
    context: &FoldContextV8,
    folded: &FoldV8,
) -> Result<Option<u32>, SourceJournalError> {
    if !context.cumulative_initialization
        || folded.failure_selected
        || folded.failed_effect_state.is_some()
        || matches!(
            folded.tail,
            TailV8::Stopped
                | TailV8::StopInDoubt
                | TailV8::Terminal
                | TailV8::TerminalInDoubt
                | TailV8::MetadataOnly
        )
    {
        return Ok(None);
    }
    if let Some(reduced) = &folded.reduce {
        let facts = reduced.fold().closure_facts();
        if facts.failure || facts.tail == super::super::reduce_fold::ReduceTailV8::Quarantined {
            return Ok(None);
        }
        if let Some(case) = facts.case {
            let mapping = reduced
                .plan()
                .mappings()
                .iter()
                .find(|mapping| mapping.case.as_str() == case)
                .ok_or(SourceJournalError::Binding)?;
            if mapping.role != "Continue" {
                return Ok(None);
            }
        }
    }
    context
        .ordinary
        .max_iterations()
        .checked_sub(folded.current_turn)
        .and_then(|n| n.checked_sub(1))
        .map(Some)
        .ok_or(SourceJournalError::Capacity)
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) fn effect_prefix_matches(
    context: &FoldContextV8,
    folded: &FoldV8,
    turn: u32,
    attempt: u32,
    state: &Value,
) -> bool {
    context.cumulative_initialization
        && context.initialized_task.is_some()
        && folded.continuation_profile_selected
        && folded.current_turn == turn
        && turn < context.ordinary.max_iterations()
        && folded
            .wait
            .as_ref()
            .is_some_and(|wait| wait.attempt == attempt)
        && folded.state.as_ref() == Some(state)
        && matches!(
            folded.tail,
            TailV8::ReadyPair | TailV8::EffectSettlementUncommitted
        )
}

/// Extend descriptive history only after the exact checked Continue transition.
/// A physical actor must separately move the actual mapped State and use its
/// fixed append permit. Generic producer appends cannot invoke that authority.
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn commit_next_state(
    context: &FoldContextV8,
    folded: &mut FoldV8,
    body: &Body,
    sequence: u32,
) -> Result<bool, SourceJournalError> {
    let Body::OwnedStateCommitted {
        turn,
        state,
        argument_digest,
        cleanup_plan_digest,
    } = body
    else {
        return Ok(false);
    };
    if folded.reduce.is_none() {
        return Ok(false);
    }
    require(
        context.cumulative_initialization
            && context.initialized_task.is_some()
            && folded.continuation_profile_selected
            && folded.tail == TailV8::Reduce
            && folded.failed_effect_state.is_none()
            && !folded.failure_selected
            && folded.current_turn.checked_add(1) == Some(*turn)
            && *turn < context.ordinary.max_iterations()
            && cleanup_plan_digest == &context.cleanup_plan_digest,
    )?;
    let reduced = folded.reduce.as_ref().ok_or(SourceJournalError::Order)?;
    require(
        reduced.fold().continued_state(sequence)? == state
            && wire::record_argument_digest(state) == *argument_digest,
    )?;

    // Totals, recorded consumption, wait fuel, and the entire ordinary history
    // stay cumulative. Only retired per-turn causal bases are cleared.
    folded.current_turn = *turn;
    folded.state = Some(state.clone());
    folded.state_digest = Some(argument_digest.clone());
    folded.state_basis = Some(sequence);
    folded.observation = None;
    folded.observe_settlement = None;
    folded.wait = None;
    folded.transfer = None;
    folded.decision = None;
    folded.cleanup = None;
    folded.effect = None;
    folded.reduce = None;
    folded.stage_originals.clear();
    folded.stage_current = None;
    folded.model_usage_pending = false;
    folded.model_failed = false;
    folded.cleanup_terminal = None;
    folded.tail = TailV8::CommittedState;
    Ok(true)
}

#[cfg(test)]
mod tests;
