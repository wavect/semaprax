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
    let committed = row(json!({"kind":"owned_state_committed","turn":u32::MAX,
        "state":max.state,"argument_digest":hash(),"cleanup_plan_digest":hash()}))?;
    let one = committed.add(outstanding_current(
        context,
        &FoldV8::capacity_fresh_turn(u32::MAX),
    )?)?;
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
