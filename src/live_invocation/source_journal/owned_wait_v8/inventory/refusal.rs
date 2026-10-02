//! Inert replay of the bounded first-turn Refused State cleanup.
use super::super::model::OwnerV8;
use super::*;
use crate::live_invocation::source_journal::SourceAuthorizationRefusal;

pub(super) fn validate(
    context: &super::super::FoldContextV8,
    rows: &[ValidatedEntryV8],
    entry: &EntryV8,
    staged: Option<&(u32, u32, usize, Value)>,
) -> Result<(), Error> {
    require(context.cumulative_initialization && context.refused_cleanup_empty)?;
    let Some((0, 0, staged_seq, decision)) = staged else {
        return Err(Error::Binding);
    };
    require(
        decision["case"].as_str() == Some(context.checked_binding.authorize().refused().as_str()),
    )?;
    let Some(ValidatedEntryV8 {
        entry:
            EntryV8::Owned(Body::OwnedAuthorizationStaged {
                turn: 0,
                attempt: 0,
                transfer,
                ..
            }),
        ..
    }) = rows.get(*staged_seq)
    else {
        return Err(Error::Binding);
    };
    let Some(ValidatedEntryV8 {
        entry:
            EntryV8::Owned(Body::OwnedStateTransferCompleted {
                turn: 0,
                attempt: 0,
                wait,
                ..
            }),
        ..
    }) = rows.get(*transfer as usize)
    else {
        return Err(Error::Binding);
    };
    let terminal = serde_json::json!({"authorization_refused":decision});
    let operations =
        v2::owned_wait_operations_v8(&context.checked_binding.helper().liveness().result_disposal)
            .map_err(|_| Error::Binding)?;
    match entry {
        EntryV8::Owned(Body::OwnedCleanupStarted {
            turn: 0,
            attempt: Some(0),
            wait: Some(actual_wait),
            owner: OwnerV8::State,
            basis,
            terminal: actual_terminal,
            operations: actual_operations,
            ..
        }) => {
            require(
                actual_wait == wait
                    && *basis == *transfer
                    && actual_terminal == &terminal
                    && actual_operations == &operations,
            )?;
            require(matches!(
                rows.last().map(|r| &r.entry),
                Some(EntryV8::Ordinary(Ordinary::AuthorizationRefused {
                    turn: 0,
                    attempt: 0,
                    reason: SourceAuthorizationRefusal::GateDenied
                }))
            ))?;
        }
        EntryV8::Owned(Body::OwnedCleanupSettled {
            turn: 0,
            attempt: Some(0),
            wait: Some(actual_wait),
            owner: OwnerV8::State,
            started,
            receipt,
            ..
        }) => {
            require(actual_wait == wait && *started as usize + 1 == rows.len())?;
            require(
                matches!(rows.last().map(|r| &r.entry), Some(EntryV8::Owned(Body::OwnedCleanupStarted {
                turn:0, attempt:Some(0), wait:Some(w), owner:OwnerV8::State,
                basis, terminal:t, operations:o, ..
            })) if w == wait && basis == transfer && t == &terminal && o == &operations),
            )?;
            v2::validate_owned_wait_observed_receipt_v8(&operations, receipt)
                .map_err(|_| Error::Binding)?;
        }
        _ => return Err(Error::Binding),
    }
    Ok(())
}

pub(super) fn validate_stop(rows: &[ValidatedEntryV8], entry: &EntryV8) -> Result<(), Error> {
    require(matches!(
        entry,
        EntryV8::Ordinary(Ordinary::Stop {
            turn: Some(0),
            attempt: Some(0),
            status: crate::live_invocation::source_journal::SourceStopStatus::Rejected,
            reason: crate::live_invocation::source_journal::SourceStopReason::StageRefused,
        })
    ))?;
    require(
        matches!(rows.last().map(|r| &r.entry), Some(EntryV8::Owned(Body::OwnedCleanupSettled {
        turn:0, attempt:Some(0), owner:OwnerV8::State, receipt, ..
    })) if receipt["kind"].as_str() == Some("observed") && receipt["settlement"].as_str() == Some("completed")),
    )
}
