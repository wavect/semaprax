//! Closed V7 wait rows. Re-encoding checks exact fields and canonical types.
use super::*;
use serde_json::Value;

pub(in crate::live_invocation::source_journal) fn encode(
    entry: &SourceModelWaitEntryV7,
    seq: u32,
) -> String {
    let (turn, attempt, wait) = entry.identity();
    let suffix=match entry {
        SourceModelWaitEntryV7::EvaluationReserved{phase,replay_of,fuel,..}=>format!(",\"phase\":{},\"replay_of\":{},\"fuel\":{}",quote_json(phase.as_str()),replay_of.map_or_else(||"null".into(),|n|n.to_string()),fuel),
        SourceModelWaitEntryV7::Prepared{reservation,observation_digest,checkpoint_digest,checkpoint,..}=>format!(",\"reservation\":{},\"observation_digest\":{},\"checkpoint_digest\":{},\"checkpoint\":{}",reservation,quote_json(observation_digest),quote_json(checkpoint_digest),quote_json(&hex(checkpoint))),
        SourceModelWaitEntryV7::Completed{reservation,proposal_digest,..}=>format!(",\"reservation\":{},\"proposal_digest\":{}",reservation,quote_json(proposal_digest)),
        SourceModelWaitEntryV7::ReplayChecked{reservation,original,result_digest,..}=>format!(",\"reservation\":{},\"original\":{},\"result_digest\":{}",reservation,original,quote_json(result_digest)),
    };
    let kind = match entry {
        SourceModelWaitEntryV7::EvaluationReserved { .. } => "wait_evaluation_reserved",
        SourceModelWaitEntryV7::Prepared { .. } => "wait_prepared",
        SourceModelWaitEntryV7::Completed { .. } => "wait_completed",
        SourceModelWaitEntryV7::ReplayChecked { .. } => "wait_replay_checked",
    };
    format!(
        "{{\"seq\":{},\"kind\":{},\"turn\":{},\"attempt\":{},\"wait\":{}{}}}",
        seq,
        quote_json(kind),
        turn,
        attempt,
        quote_json(wait),
        suffix
    )
}
pub(in crate::live_invocation::source_journal) fn decode(
    value: &Value,
    seq: u32,
) -> Result<SourceModelWaitEntryV7, SourceJournalError> {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or(SourceJournalError::Malformed)
    };
    let number = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
            .ok_or(SourceJournalError::Malformed)
    };
    if number("seq")? != seq {
        return Err(SourceJournalError::Generation);
    }
    let (turn, attempt, wait) = (number("turn")?, number("attempt")?, text("wait")?);
    let row = match text("kind")?.as_str() {
        "wait_evaluation_reserved" => SourceModelWaitEntryV7::EvaluationReserved {
            turn,
            attempt,
            wait,
            phase: match text("phase")?.as_str() {
                "start" => SourceModelWaitPhaseV7::Start,
                "resume" => SourceModelWaitPhaseV7::Resume,
                _ => return Err(SourceJournalError::Malformed),
            },
            replay_of: if value["replay_of"].is_null() {
                None
            } else {
                Some(number("replay_of")?)
            },
            fuel: value["fuel"]
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or(SourceJournalError::Malformed)?,
        },
        "wait_prepared" => {
            let encoded = text("checkpoint")?;
            if encoded.len() > 2 * SOURCE_MODEL_WAIT_CHECKPOINT_LIMIT {
                return Err(SourceJournalError::Capacity);
            }
            SourceModelWaitEntryV7::Prepared {
                turn,
                attempt,
                wait,
                reservation: number("reservation")?,
                observation_digest: text("observation_digest")?,
                checkpoint_digest: text("checkpoint_digest")?,
                checkpoint: crate::live_invocation::identity::unhex(&encoded)
                    .ok_or(SourceJournalError::Malformed)?,
            }
        }
        "wait_completed" => SourceModelWaitEntryV7::Completed {
            turn,
            attempt,
            wait,
            reservation: number("reservation")?,
            proposal_digest: text("proposal_digest")?,
        },
        "wait_replay_checked" => SourceModelWaitEntryV7::ReplayChecked {
            turn,
            attempt,
            wait,
            reservation: number("reservation")?,
            original: number("original")?,
            result_digest: text("result_digest")?,
        },
        _ => return Err(SourceJournalError::Malformed),
    };
    let canonical: Value =
        serde_json::from_str(&encode(&row, seq)).map_err(|_| SourceJournalError::Malformed)?;
    if canonical != *value {
        return Err(SourceJournalError::Malformed);
    }
    Ok(row)
}
