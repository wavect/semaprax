//! Outstanding rows shrink after each ACK; emitted payloads are never reserved twice.
use super::*;

pub(super) fn check(
    journal: &SourceJournal,
    document_bytes: usize,
) -> Result<(), SourceJournalError> {
    let (bytes, entries) = outstanding(journal)?;
    limits(document_bytes, journal.combined_len(), bytes, entries)
}

pub(super) fn outstanding(journal: &SourceJournal) -> Result<(usize, usize), SourceJournalError> {
    let folded = fold(journal)?;
    if matches!(
        journal.entries.last(),
        Some(SourceJournalEntry::TerminalSnapshot { .. })
    ) {
        return Ok((0, 0));
    }
    if matches!(
        journal.entries.last(),
        Some(SourceJournalEntry::Stop { .. })
    ) {
        return Ok((execution::TERMINAL_ROOM_BYTES, 1));
    }
    let mut future = Vec::new();
    let latest = folded
        .states
        .values()
        .filter(|s| !s.closed)
        .max_by_key(|s| s.public.start_reservation);
    if let Some(state) = latest {
        let p = &state.public;
        if p.prepared.is_none() {
            future.push(prepared(p));
        }
        if !state.intent {
            future.push(intent(journal, p));
        }
        if !state.settled && !state.closed {
            future.push(settled(journal, p));
            future.push(usage(p));
        } else if p.resume_reservation.is_none()
            && matches!(
                journal.entries.last(),
                Some(SourceJournalEntry::AttemptSettled { .. })
            )
        {
            future.push(usage(p));
        }
        if p.resume_reservation.is_none() {
            future.push(reservation(journal, p));
        }
        if p.completed.is_none() {
            future.push(completed(p));
        }
        future.push(admitted(p));
    }
    // Only the latest newly charged reconstruction needs closure room. Older
    // interrupted reservations remain charged and confer no evaluation credit.
    let pending = folded
        .states
        .values()
        .flat_map(|s| s.public.reservations.iter().map(move |row| (s, row)))
        .max_by_key(|(_, (seq, _))| *seq)
        .filter(|(s, (seq, row))| {
            matches!(
                row,
                SourceModelWaitEntryV7::EvaluationReserved {
                    replay_of: Some(_),
                    ..
                }
            ) && !s.checks.contains(seq)
        });
    if let Some((state, (seq, row))) = pending {
        let SourceModelWaitEntryV7::EvaluationReserved { phase, .. } = row else {
            unreachable!()
        };
        let closure = match phase {
            SourceModelWaitPhaseV7::Start => &state.public.prepared,
            SourceModelWaitPhaseV7::Resume => &state.public.completed,
        };
        if closure.is_none() && latest.is_none_or(|latest| latest.public.wait != state.public.wait)
        {
            future.push(match phase {
                SourceModelWaitPhaseV7::Start => prepared(&state.public),
                SourceModelWaitPhaseV7::Resume => completed(&state.public),
            });
        }
        future.push(wire::encode(
            &SourceModelWaitEntryV7::ReplayChecked {
                turn: state.public.turn,
                attempt: state.public.attempt,
                wait: state.public.wait.clone(),
                reservation: *seq,
                original: u32::MAX,
                result_digest: hash(),
            },
            u32::MAX,
        ));
    }
    let bytes = future
        .iter()
        .try_fold(execution::TERMINAL_ROOM_BYTES, |total, row| {
            total
                .checked_add(row.len() + 1)
                .ok_or(SourceJournalError::Capacity)
        })?;
    let entries = future
        .len()
        .checked_add(2)
        .ok_or(SourceJournalError::Capacity)?;
    // The opt-in wait inventory also retains the ordinary adapter's physical
    // effect settlement guarantee. Its boundary is independent of model waits.
    if matches!(
        journal.entries.last(),
        Some(SourceJournalEntry::EffectIntent { .. })
    ) {
        return Ok((
            bytes
                .checked_add(2 * MAX_SOURCE_EFFECT_BYTES + 4096)
                .ok_or(SourceJournalError::Capacity)?,
            entries.checked_add(3).ok_or(SourceJournalError::Capacity)?,
        ));
    }
    Ok((bytes, entries))
}
fn limits(
    bytes: usize,
    entries: usize,
    future_bytes: usize,
    future_entries: usize,
) -> Result<(), SourceJournalError> {
    if bytes
        .checked_add(future_bytes)
        .is_none_or(|n| n > MAX_SOURCE_DOCUMENT_BYTES)
        || entries
            .checked_add(future_entries)
            .is_none_or(|n| n > MAX_SOURCE_ENTRIES)
    {
        Err(SourceJournalError::Capacity)
    } else {
        Ok(())
    }
}
fn hash() -> String {
    format!("sha256:{}", "f".repeat(64))
}
fn wait(row: SourceModelWaitEntryV7) -> String {
    wire::encode(&row, u32::MAX)
}
fn ordinary(row: SourceJournalEntry) -> String {
    super::super::wire::encode_entry(&row, u32::MAX as usize)
}
fn prepared(p: &SourceModelWaitStateV7) -> String {
    wait(SourceModelWaitEntryV7::Prepared {
        turn: p.turn,
        attempt: p.attempt,
        wait: p.wait.clone(),
        reservation: u32::MAX,
        observation_digest: hash(),
        checkpoint_digest: hash(),
        checkpoint: vec![255; SOURCE_MODEL_WAIT_CHECKPOINT_LIMIT],
    })
}
fn intent(journal: &SourceJournal, p: &SourceModelWaitStateV7) -> String {
    ordinary(SourceJournalEntry::AttemptIntent {
        turn: p.turn,
        attempt: p.attempt,
        attempt_digest: hash(),
        request_digest: hash(),
        prompt_digest: hash(),
        request_bytes: MAX_SOURCE_REQUEST_BYTES,
        reserved_units: i64::MAX,
        response_limit: journal.binding.response_limit,
    })
}
fn settled(journal: &SourceJournal, p: &SourceModelWaitStateV7) -> String {
    ordinary(SourceJournalEntry::AttemptSettled {
        turn: p.turn,
        attempt: p.attempt,
        response: vec![255; journal.binding.response_limit],
        response_digest: hash(),
    })
}
fn usage(p: &SourceModelWaitStateV7) -> String {
    ordinary(SourceJournalEntry::AttemptUsage {
        turn: p.turn,
        attempt: p.attempt,
        reported: Some(SourceReportedUsage {
            total: Some(u64::MAX),
            input: Some(u64::MAX),
            output: Some(u64::MAX),
            reasoning: Some(u64::MAX),
            cache_read: Some(u64::MAX),
            cache_write: Some(u64::MAX),
        }),
    })
}
fn reservation(journal: &SourceJournal, p: &SourceModelWaitStateV7) -> String {
    wait(SourceModelWaitEntryV7::EvaluationReserved {
        turn: p.turn,
        attempt: p.attempt,
        wait: p.wait.clone(),
        phase: SourceModelWaitPhaseV7::Resume,
        replay_of: None,
        fuel: journal
            .binding
            .wait
            .as_ref()
            .expect("wait profile")
            .evaluation_fuel,
    })
}
fn completed(p: &SourceModelWaitStateV7) -> String {
    wait(SourceModelWaitEntryV7::Completed {
        turn: p.turn,
        attempt: p.attempt,
        wait: p.wait.clone(),
        reservation: u32::MAX,
        proposal_digest: hash(),
    })
}
fn admitted(p: &SourceModelWaitStateV7) -> String {
    ordinary(SourceJournalEntry::ProposalAdmitted {
        turn: p.turn,
        attempt: p.attempt,
        proposal_digest: hash(),
    })
}
