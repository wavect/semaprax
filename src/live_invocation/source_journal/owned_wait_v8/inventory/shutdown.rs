//! Inert replay of first Completed -> abandonment -> exact State cleanup -> Stop.
use super::super::model::OwnerV8;
use super::*;
use crate::live_invocation::source_journal::{SourceStopReason, SourceStopStatus};
fn terminal() -> Value {
    serde_json::json!({"failure":"host_abandoned","language_status":null})
}
fn completed<'a>(
    context: &super::super::FoldContextV8,
    rows: &'a [ValidatedEntryV8],
    index: usize,
) -> Result<&'a str, Error> {
    require(context.cumulative_initialization)?;
    let Some(ValidatedEntryV8 {
        entry:
            EntryV8::Owned(Body::OwnedWaitCompleted {
                turn: 0,
                attempt: 0,
                wait,
                ..
            }),
        ..
    }) = rows.get(index)
    else {
        return Err(Error::Binding);
    };
    require(fold::fold(context, &rows[..=index])?.tail == fold::TailV8::Completed)?;
    Ok(wait)
}
fn failure(row: &EntryV8, wait: &str) -> bool {
    matches!(row, EntryV8::Owned(Body::OwnedWaitFailed {turn:0,attempt:0,wait:w,reservation:None,status,consumed:0}) if w == wait && status == &terminal())
}
pub(super) fn validate_failure(
    context: &super::super::FoldContextV8,
    rows: &[ValidatedEntryV8],
    entry: &EntryV8,
) -> Result<(), Error> {
    let index = rows.len().checked_sub(1).ok_or(Error::Binding)?;
    let wait = completed(context, rows, index)?;
    require(failure(entry, wait))
}
pub(super) fn selected(rows: &[ValidatedEntryV8]) -> bool {
    matches!(rows.last().map(|r|&r.entry), Some(EntryV8::Owned(Body::OwnedWaitFailed {reservation:None,status,..})) if status == &terminal())
        || matches!(rows.last().map(|r|&r.entry), Some(EntryV8::Owned(Body::OwnedCleanupStarted {owner:OwnerV8::State,terminal:t, ..})) if t == &terminal())
}
pub(super) fn validate(
    context: &super::super::FoldContextV8,
    rows: &[ValidatedEntryV8],
    entry: &EntryV8,
) -> Result<(), Error> {
    let is_started = matches!(entry, EntryV8::Owned(Body::OwnedCleanupStarted { .. }));
    let index = rows
        .len()
        .checked_sub(if is_started { 2 } else { 3 })
        .ok_or(Error::Binding)?;
    let wait = completed(context, rows, index)?;
    require(failure(&rows[index + 1].entry, wait))?;
    let operations =
        v2::owned_wait_operations_v8(&context.checked_binding.helper().liveness().result_disposal)
            .map_err(|_| Error::Binding)?;
    let started = if is_started {
        entry
    } else {
        &rows[index + 2].entry
    };
    require(matches!(started, EntryV8::Owned(Body::OwnedCleanupStarted {
        turn:0,attempt:Some(0),wait:Some(w),owner:OwnerV8::State,basis,terminal:t,operations:o,..
    }) if w == wait && *basis as usize == index && t == &terminal() && o == &operations))?;
    if !is_started {
        let EntryV8::Owned(Body::OwnedCleanupSettled {
            turn: 0,
            attempt: Some(0),
            wait: Some(w),
            owner: OwnerV8::State,
            started,
            receipt,
            ..
        }) = entry
        else {
            return Err(Error::Binding);
        };
        require(w == wait && *started as usize == index + 2)?;
        v2::validate_owned_wait_observed_receipt_v8(&operations, receipt)
            .map_err(|_| Error::Binding)?;
    }
    Ok(())
}
pub(super) fn stop_selected(rows: &[ValidatedEntryV8]) -> bool {
    rows.len().checked_sub(2).is_some_and(|i| matches!(&rows[i].entry,EntryV8::Owned(Body::OwnedCleanupStarted {owner:OwnerV8::State,terminal:t,..}) if t == &terminal()))
}
pub(super) fn validate_stop(
    context: &super::super::FoldContextV8,
    rows: &[ValidatedEntryV8],
    entry: &EntryV8,
) -> Result<(), Error> {
    require(matches!(
        entry,
        EntryV8::Ordinary(Ordinary::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: SourceStopStatus::Cancelled,
            reason: SourceStopReason::Cancelled
        })
    ))?;
    let (last, prefix) = rows.split_last().ok_or(Error::Binding)?;
    validate(context, prefix, &last.entry)?;
    require(
        matches!(&last.entry,EntryV8::Owned(Body::OwnedCleanupSettled {receipt,..}) if receipt["kind"].as_str()==Some("observed") && receipt["settlement"].as_str()==Some("completed")),
    )
}
