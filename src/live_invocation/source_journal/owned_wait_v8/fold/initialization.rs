//! Actual Initialize completion under the sealed expected initialized profile.
use super::*;
pub(super) fn commit(
    context: &FoldContextV8,
    f: &mut FoldV8,
    body: &Body,
) -> Result<(), SourceJournalError> {
    let Body::OwnedInitializationCommitted {
        reservation,
        task,
        task_digest,
        state,
        state_digest,
        consumed,
    } = body
    else {
        return order();
    };
    require(
        context.initialized_task.as_ref() == Some(task) && f.tail == TailV8::InitializeReserved,
    )?;
    let (original, role, fuel) = f.stage_current.ok_or(SourceJournalError::Order)?;
    require(original == *reservation && role == SourceStageRole::Initialize && *consumed <= fuel)?;
    require(
        wire::record_argument_digest(task) == *task_digest
            && wire::record_argument_digest(state) == *state_digest,
    )?;
    f.consume(*consumed, fuel)?;
    f.state = Some(state.clone());
    f.state_digest = Some(state_digest.clone());
    f.tail = TailV8::Initialized;
    Ok(())
}
