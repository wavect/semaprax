//! Exact canonical source-journal wire. All decode allocation is preceded by
//! the whole-document byte cap; response hex is capped before conversion.

use serde_json::{Map, Value};

use super::*;
use crate::live_invocation::identity::unhex;

const CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v1\0";

fn kind(entry: &SourceJournalEntry) -> &'static str {
    match entry {
        SourceJournalEntry::RunOpened => "run_opened",
        SourceJournalEntry::TurnObserved { .. } => "turn_observed",
        SourceJournalEntry::AttemptIntent { .. } => "attempt_intent",
        SourceJournalEntry::AttemptSettled { .. } => "attempt_settled",
        SourceJournalEntry::AttemptFailed { .. } => "attempt_failed",
        SourceJournalEntry::ProposalRefused { .. } => "proposal_refused",
        SourceJournalEntry::ProposalAdmitted { .. } => "proposal_admitted",
        SourceJournalEntry::AuthorizationConsumed { .. } => "authorization_consumed",
        SourceJournalEntry::AuthorizationRefused { .. } => "authorization_refused",
        SourceJournalEntry::EffectIntent { .. } => "effect_intent",
        SourceJournalEntry::EffectObserved { .. } => "effect_observed",
        SourceJournalEntry::EffectFailed { .. } => "effect_failed",
        SourceJournalEntry::Transition { .. } => "transition",
        SourceJournalEntry::Stop { .. } => "stop",
        SourceJournalEntry::TerminalOutcome { .. } => "terminal_outcome",
    }
}

fn turn_attempt(entry: &SourceJournalEntry) -> (Option<u32>, Option<u32>) {
    match entry {
        SourceJournalEntry::RunOpened => (None, None),
        SourceJournalEntry::TurnObserved { turn, .. } => (Some(*turn), None),
        SourceJournalEntry::Stop { turn, attempt, .. } => (*turn, *attempt),
        SourceJournalEntry::TerminalOutcome { turn, .. } => (*turn, None),
        SourceJournalEntry::AttemptIntent { turn, attempt, .. }
        | SourceJournalEntry::AttemptSettled { turn, attempt, .. }
        | SourceJournalEntry::AttemptFailed { turn, attempt, .. }
        | SourceJournalEntry::ProposalRefused { turn, attempt, .. }
        | SourceJournalEntry::ProposalAdmitted { turn, attempt, .. }
        | SourceJournalEntry::AuthorizationConsumed { turn, attempt, .. }
        | SourceJournalEntry::AuthorizationRefused { turn, attempt, .. }
        | SourceJournalEntry::EffectIntent { turn, attempt, .. }
        | SourceJournalEntry::EffectObserved { turn, attempt, .. }
        | SourceJournalEntry::EffectFailed { turn, attempt, .. }
        | SourceJournalEntry::Transition { turn, attempt, .. } => (Some(*turn), Some(*attempt)),
    }
}

fn encode_entry(entry: &SourceJournalEntry, seq: usize) -> String {
    let (turn, attempt) = turn_attempt(entry);
    let mut output = format!("{{\"seq\":{},\"kind\":{}", seq, quote_json(kind(entry)));
    if let Some(turn) = turn {
        output.push_str(&format!(",\"turn\":{turn}"));
    }
    if let Some(attempt) = attempt {
        output.push_str(&format!(",\"attempt\":{attempt}"));
    }
    let fields = match entry {
        SourceJournalEntry::RunOpened => String::new(),
        SourceJournalEntry::TurnObserved { state, observation, feedback, .. } => format!(
            ",\"state\":{},\"observation\":{},\"feedback\":{}",
            quote_json(state), quote_json(observation), quote_json(feedback)),
        SourceJournalEntry::AttemptIntent {
            attempt_digest, request_digest, prompt_digest, request_bytes,
            reserved_units, response_limit, ..
        } => format!(
            ",\"attempt_digest\":{},\"request_digest\":{},\"prompt_digest\":{},\"request_bytes\":{},\"reserved_units\":{},\"response_limit\":{}",
            quote_json(attempt_digest), quote_json(request_digest), quote_json(prompt_digest),
            request_bytes, reserved_units, response_limit),
        SourceJournalEntry::AttemptSettled { response, response_digest, .. } => format!(
            ",\"response\":{},\"response_digest\":{}",
            quote_json(&hex(response)), quote_json(response_digest)),
        SourceJournalEntry::AttemptFailed { reason, attempted_bytes, .. } => format!(
            ",\"reason\":{},\"attempted_bytes\":{}",
            quote_json(reason.as_str()), attempted_bytes),
        SourceJournalEntry::ProposalRefused { reason, .. } =>
            format!(",\"reason\":{}", quote_json(reason.as_str())),
        SourceJournalEntry::ProposalAdmitted { proposal_digest, .. } =>
            format!(",\"proposal_digest\":{}", quote_json(proposal_digest)),
        SourceJournalEntry::AuthorizationConsumed { grant_digest, .. } =>
            format!(",\"grant_digest\":{}", quote_json(grant_digest)),
        SourceJournalEntry::AuthorizationRefused { reason, .. } =>
            format!(",\"reason\":{}", quote_json(reason.as_str())),
        SourceJournalEntry::EffectIntent { operation, request_digest, .. } => format!(
            ",\"operation\":{},\"request_digest\":{}",
            quote_json(operation), quote_json(request_digest)),
        SourceJournalEntry::EffectObserved {
            operation, observation, observation_digest, ..
        } => format!(
            ",\"operation\":{},\"observation\":{},\"observation_digest\":{}",
            quote_json(operation), quote_json(&hex(observation)), quote_json(observation_digest)),
        SourceJournalEntry::EffectFailed { operation, reason, .. } => format!(
            ",\"operation\":{},\"reason\":{}",
            quote_json(operation), quote_json(reason.as_str())),
        SourceJournalEntry::Transition { case, carrier_digest, .. } => format!(
            ",\"case\":{},\"carrier_digest\":{}",
            quote_json(case.as_str()), quote_json(carrier_digest)),
        SourceJournalEntry::Stop { status, reason, .. } => format!(
            ",\"status\":{},\"reason\":{}",
            quote_json(status.as_str()), quote_json(reason.as_str())),
        SourceJournalEntry::TerminalOutcome { status, carrier_digest, .. } => format!(
            ",\"status\":{},\"carrier_digest\":{}",
            quote_json(status.as_str()), carrier_digest.as_deref().map(quote_json)
                .unwrap_or_else(|| "null".to_owned())),
    };
    output.push_str(&fields);
    output.push('}');
    output
}

fn encode_entries(entries: &[SourceJournalEntry]) -> Result<String, SourceJournalError> {
    if entries.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceJournalError::Capacity);
    }
    let mut output = String::from("[");
    for (seq, entry) in entries.iter().enumerate() {
        if seq != 0 {
            output.push(',');
        }
        output.push_str(&encode_entry(entry, seq));
        if output.len() > MAX_SOURCE_DOCUMENT_BYTES {
            return Err(SourceJournalError::Capacity);
        }
    }
    output.push(']');
    Ok(output)
}

fn chain_input(journal: &SourceJournal, generation: u64, entries: &str) -> String {
    format!(
        "{{\"schema\":{},\"invocation\":{},\"generation\":{},\"clock_domain\":{},\"last_checked_millis\":{},\"entries\":{}}}",
        quote_json(SOURCE_JOURNAL_SCHEMA), quote_json(journal.binding.invocation()), generation,
        quote_json(journal.binding.clock_domain()), journal.last_checked_millis(), entries)
}

pub(super) fn encode_envelope(
    journal: &SourceJournal,
    generation: u64,
) -> Result<String, SourceJournalError> {
    if generation == 0 || generation != journal.entries.len() as u64 {
        return Err(SourceJournalError::Generation);
    }
    let entries = encode_entries(journal.entries())?;
    let link = digest(
        CHAIN_DOMAIN,
        chain_input(journal, generation, &entries).as_bytes(),
    );
    let document = format!(
        "{{\"schema\":{},\"invocation\":{},\"generation\":{},\"clock_domain\":{},\"last_checked_millis\":{},\"chain\":{},\"entries\":{}}}\n",
        quote_json(SOURCE_JOURNAL_SCHEMA), quote_json(journal.binding.invocation()), generation,
        quote_json(journal.binding.clock_domain()), journal.last_checked_millis(),
        quote_json(&link), entries,
    );
    if document.len() > MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    Ok(document)
}

fn object(value: &Value) -> Result<&Map<String, Value>, SourceJournalError> {
    value.as_object().ok_or(SourceJournalError::Malformed)
}
fn string(map: &Map<String, Value>, key: &str) -> Result<String, SourceJournalError> {
    map.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(SourceJournalError::Malformed)
}
fn u64_field(map: &Map<String, Value>, key: &str) -> Result<u64, SourceJournalError> {
    map.get(key)
        .and_then(Value::as_u64)
        .ok_or(SourceJournalError::Malformed)
}
fn u32_field(map: &Map<String, Value>, key: &str) -> Result<u32, SourceJournalError> {
    u32::try_from(u64_field(map, key)?).map_err(|_| SourceJournalError::Malformed)
}
fn usize_field(map: &Map<String, Value>, key: &str) -> Result<usize, SourceJournalError> {
    usize::try_from(u64_field(map, key)?).map_err(|_| SourceJournalError::Malformed)
}
fn i64_field(map: &Map<String, Value>, key: &str) -> Result<i64, SourceJournalError> {
    map.get(key)
        .and_then(Value::as_i64)
        .ok_or(SourceJournalError::Malformed)
}
fn optional_u32(map: &Map<String, Value>, key: &str) -> Result<Option<u32>, SourceJournalError> {
    if map.contains_key(key) {
        Ok(Some(u32_field(map, key)?))
    } else {
        Ok(None)
    }
}
fn optional_digest(
    map: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, SourceJournalError> {
    match map.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(SourceJournalError::Malformed),
    }
}
fn bytes_field(
    map: &Map<String, Value>,
    key: &str,
    cap: usize,
) -> Result<Vec<u8>, SourceJournalError> {
    let encoded = map
        .get(key)
        .and_then(Value::as_str)
        .ok_or(SourceJournalError::Malformed)?;
    if encoded.len() > cap.saturating_mul(2) {
        return Err(SourceJournalError::Capacity);
    }
    unhex(encoded).ok_or(SourceJournalError::Malformed)
}
fn tag<T>(
    map: &Map<String, Value>,
    key: &str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, SourceJournalError> {
    parse(
        map.get(key)
            .and_then(Value::as_str)
            .ok_or(SourceJournalError::Malformed)?,
    )
    .ok_or(SourceJournalError::Malformed)
}

fn decode_entry(value: &Value, seq: usize) -> Result<SourceJournalEntry, SourceJournalError> {
    let map = object(value)?;
    if usize_field(map, "seq")? != seq {
        return Err(SourceJournalError::Order);
    }
    let turn = || u32_field(map, "turn");
    let attempt = || u32_field(map, "attempt");
    let entry = match string(map, "kind")?.as_str() {
        "run_opened" => SourceJournalEntry::RunOpened,
        "turn_observed" => SourceJournalEntry::TurnObserved {
            turn: turn()?,
            state: string(map, "state")?,
            observation: string(map, "observation")?,
            feedback: string(map, "feedback")?,
        },
        "attempt_intent" => SourceJournalEntry::AttemptIntent {
            turn: turn()?,
            attempt: attempt()?,
            attempt_digest: string(map, "attempt_digest")?,
            request_digest: string(map, "request_digest")?,
            prompt_digest: string(map, "prompt_digest")?,
            request_bytes: usize_field(map, "request_bytes")?,
            reserved_units: i64_field(map, "reserved_units")?,
            response_limit: usize_field(map, "response_limit")?,
        },
        "attempt_settled" => SourceJournalEntry::AttemptSettled {
            turn: turn()?,
            attempt: attempt()?,
            response: bytes_field(map, "response", MAX_SOURCE_RESPONSE_BYTES)?,
            response_digest: string(map, "response_digest")?,
        },
        "attempt_failed" => SourceJournalEntry::AttemptFailed {
            turn: turn()?,
            attempt: attempt()?,
            reason: tag(map, "reason", SourceAttemptFailure::parse)?,
            attempted_bytes: usize_field(map, "attempted_bytes")?,
        },
        "proposal_refused" => SourceJournalEntry::ProposalRefused {
            turn: turn()?,
            attempt: attempt()?,
            reason: tag(map, "reason", SourceProposalRefusal::parse)?,
        },
        "proposal_admitted" => SourceJournalEntry::ProposalAdmitted {
            turn: turn()?,
            attempt: attempt()?,
            proposal_digest: string(map, "proposal_digest")?,
        },
        "authorization_consumed" => SourceJournalEntry::AuthorizationConsumed {
            turn: turn()?,
            attempt: attempt()?,
            grant_digest: string(map, "grant_digest")?,
        },
        "authorization_refused" => SourceJournalEntry::AuthorizationRefused {
            turn: turn()?,
            attempt: attempt()?,
            reason: tag(map, "reason", SourceAuthorizationRefusal::parse)?,
        },
        "effect_intent" => SourceJournalEntry::EffectIntent {
            turn: turn()?,
            attempt: attempt()?,
            operation: string(map, "operation")?,
            request_digest: string(map, "request_digest")?,
        },
        "effect_observed" => SourceJournalEntry::EffectObserved {
            turn: turn()?,
            attempt: attempt()?,
            operation: string(map, "operation")?,
            observation: bytes_field(map, "observation", MAX_SOURCE_EFFECT_BYTES)?,
            observation_digest: string(map, "observation_digest")?,
        },
        "effect_failed" => SourceJournalEntry::EffectFailed {
            turn: turn()?,
            attempt: attempt()?,
            operation: string(map, "operation")?,
            reason: tag(map, "reason", SourceEffectFailure::parse)?,
        },
        "transition" => SourceJournalEntry::Transition {
            turn: turn()?,
            attempt: attempt()?,
            case: tag(map, "case", SourceTransitionCase::parse)?,
            carrier_digest: string(map, "carrier_digest")?,
        },
        "stop" => SourceJournalEntry::Stop {
            turn: optional_u32(map, "turn")?,
            attempt: optional_u32(map, "attempt")?,
            status: tag(map, "status", SourceStopStatus::parse)?,
            reason: tag(map, "reason", SourceStopReason::parse)?,
        },
        "terminal_outcome" => SourceJournalEntry::TerminalOutcome {
            turn: optional_u32(map, "turn")?,
            status: tag(map, "status", SourceTerminalStatus::parse)?,
            carrier_digest: optional_digest(map, "carrier_digest")?,
        },
        _ => return Err(SourceJournalError::Malformed),
    };
    // A typed re-encoding has exactly the admitted keys and value types.
    // This catches unknown keys without maintaining a second field registry.
    let canonical: Value = serde_json::from_str(&encode_entry(&entry, seq))
        .map_err(|_| SourceJournalError::Malformed)?;
    if canonical != *value {
        return Err(SourceJournalError::Malformed);
    }
    Ok(entry)
}

pub(super) fn decode_envelope(
    document: &str,
    expected: &SourceInvocationBinding,
) -> Result<(SourceJournal, u64, String), SourceJournalError> {
    if document.len() > MAX_SOURCE_DOCUMENT_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    let value: Value = serde_json::from_str(document).map_err(|_| SourceJournalError::Malformed)?;
    let map = object(&value)?;
    let keys = [
        "schema",
        "invocation",
        "generation",
        "clock_domain",
        "last_checked_millis",
        "chain",
        "entries",
    ];
    if map.len() != keys.len() || !keys.iter().all(|key| map.contains_key(*key)) {
        return Err(SourceJournalError::Malformed);
    }
    if string(map, "schema")? != SOURCE_JOURNAL_SCHEMA {
        return Err(SourceJournalError::Malformed);
    }
    if string(map, "invocation")? != expected.invocation()
        || string(map, "clock_domain")? != expected.clock_domain()
    {
        return Err(SourceJournalError::Binding);
    }
    let generation = u64_field(map, "generation")?;
    let last_checked_millis = i64_field(map, "last_checked_millis")?;
    if last_checked_millis < expected.initial_millis() {
        return Err(SourceJournalError::Time);
    }
    let raw_entries = map
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(SourceJournalError::Malformed)?;
    if raw_entries.is_empty() || raw_entries.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceJournalError::Capacity);
    }
    if generation != raw_entries.len() as u64 {
        return Err(SourceJournalError::Generation);
    }
    let mut entries = Vec::with_capacity(raw_entries.len());
    for (seq, item) in raw_entries.iter().enumerate() {
        entries.push(decode_entry(item, seq)?);
    }
    let journal = SourceJournal {
        binding: expected.clone(),
        entries,
        last_checked_millis,
    };
    let canonical = encode_envelope(&journal, generation)?;
    let recorded_chain = string(map, "chain")?;
    if !looks_like_digest(&recorded_chain) {
        return Err(SourceJournalError::Malformed);
    }
    let parsed: Value =
        serde_json::from_str(&canonical).map_err(|_| SourceJournalError::Malformed)?;
    let expected_chain = string(object(&parsed)?, "chain")?;
    if recorded_chain != expected_chain {
        return Err(SourceJournalError::Chain);
    }
    if document != canonical {
        return Err(SourceJournalError::Malformed);
    }
    Ok((journal, generation, recorded_chain))
}
