//! Future closure forecasting from compiler maxima and the bound turn ceiling.
//! Templates grant no admission or physical owner authority.
use super::*;

pub(super) fn future(
    context: &FoldContextV8,
    folded: &FoldV8,
) -> Result<RoomV8, SourceJournalError> {
    let Some(remaining) = fold::cumulative::remaining_turns(context, folded)? else {
        return Ok(RoomV8::default());
    };
    let max = templates::maxima(context)?;
    let one = fresh_turn_room(context, &max)?;
    let turns = RoomV8 {
        bytes: one
            .bytes
            .checked_mul(remaining as usize)
            .ok_or(SourceJournalError::Capacity)?,
        rows: one
            .rows
            .checked_mul(remaining as usize)
            .ok_or(SourceJournalError::Capacity)?,
    };
    // At the authenticated ceiling, Continue still owns a mapped State. Reserve
    // its complete cleanup and terminal closure without implying execution.
    let terminal = RoomV8 {
        bytes: super::super::super::execution::TERMINAL_ROOM_BYTES,
        rows: 2,
    };
    cleanup(&max, OwnerV8::State, &max.state_operations)?
        .add(terminal)?
        .add(turns)
}

// Only immutable compiler-derived future room is retained. Current fold, byte
// counts, physical pins, policy, cancellation and append witnesses are not.
#[derive(Default)]
pub(in crate::live_invocation::source_journal::owned_wait_v8) struct FutureTemplateCacheV8 {
    entry: std::cell::RefCell<Option<FutureTemplateV8>>,
}
struct FutureTemplateV8 {
    key: Value,
    binding:
        std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8>,
    reduce: std::sync::Arc<crate::resumable_effects::owned_frame::v2::CheckedOwnedReduceV2>,
    room: Result<RoomV8, SourceJournalError>,
}
impl FutureTemplateCacheV8 {
    pub(in crate::live_invocation::source_journal::owned_wait_v8) fn reset(&mut self) {
        *self.entry.get_mut() = None;
    }
}
fn fresh_turn_room(
    context: &FoldContextV8,
    max: &templates::Maxima,
) -> Result<RoomV8, SourceJournalError> {
    // Retain exact proof objects as well as scalar inputs: a matching digest or
    // an independently reconstructed DTO never substitutes for checked proof.
    let reduce = context.checked_reduce()?;
    let key = json!({
        "created": serde_json::to_value(&context.created).map_err(|_| SourceJournalError::Malformed)?,
        "stage_fuel": context.ordinary.max_steps_per_stage(),
        "attempts": context.ordinary.max_attempts(),
        "iterations": context.ordinary.max_iterations(),
        "response_limit": context.ordinary.response_limit(),
        "cumulative": context.cumulative_initialization,
        "initialized_task": context.initialized_task,
        "plan_digest": context.plan_digest,
        "cleanup_plan_digest": context.cleanup_plan_digest,
        "signature": context.signature,
        "helper": context.helper,
        "authorize": context.authorize,
        "refused_cleanup_empty": context.refused_cleanup_empty,
    });
    if let Some(retained) = context.future_templates.entry.borrow().as_ref() {
        if retained.key == key
            && std::sync::Arc::ptr_eq(&retained.binding, &context.checked_binding)
            && std::sync::Arc::ptr_eq(&retained.reduce, reduce)
        {
            return retained.room;
        }
    }
    let room = fresh_turn_room_uncached(context, max);
    *context.future_templates.entry.borrow_mut() = Some(FutureTemplateV8 {
        key,
        binding: std::sync::Arc::clone(&context.checked_binding),
        reduce: std::sync::Arc::clone(reduce),
        room,
    });
    room
}
fn fresh_turn_room_uncached(
    context: &FoldContextV8,
    max: &templates::Maxima,
) -> Result<RoomV8, SourceJournalError> {
    let committed = row(json!({"kind":"owned_state_committed","turn":u32::MAX,
        "state":max.state,"argument_digest":hash(),"cleanup_plan_digest":hash()}))?;
    committed.add(outstanding_current(
        context,
        &FoldV8::capacity_fresh_turn(u32::MAX),
    )?)
}

#[cfg(test)]
mod tests;
