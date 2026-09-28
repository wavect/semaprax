//! Borrowed actual continued Observe metadata, never restoration authority.
use super::*;
pub(crate) struct CheckedContinuedObserveFactsV8 {
    pub(crate) state: serde_json::Value,
    pub(crate) observation: Option<ResumableChannelValue>,
    pub(crate) failure: Option<OwnedFrameFailure>,
    pub(crate) consumed: usize,
}
pub(crate) fn checked_continued_observe_facts_v8(
    outcome: &ContinuedOwnedObserveV2<'_>,
) -> Result<CheckedContinuedObserveFactsV8, SourceJournalError> {
    let (state, observation, failure, consumed, valid) = match outcome {
        ContinuedOwnedObserveV2::Observed(o) => (
            o.observed.live_state_facts_v8(),
            Some(o.observation().clone()),
            None,
            o.consumed(),
            o.validate_store(),
        ),
        ContinuedOwnedObserveV2::Failed(f) => (
            f.failed.live_state_facts_v8(),
            None,
            Some(f.failed.failure().clone()),
            f.consumed(),
            f.validate_store(),
        ),
        ContinuedOwnedObserveV2::Quarantined(_) => return Err(SourceJournalError::Binding),
    };
    if !valid {
        return Err(SourceJournalError::Binding);
    }
    let state = state.map_err(|_| SourceJournalError::Binding)?;
    Ok(CheckedContinuedObserveFactsV8 {
        state,
        observation,
        failure,
        consumed,
    })
}
