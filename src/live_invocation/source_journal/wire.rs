//! Exact canonical source-journal wire. All decode allocation is preceded by
//! the whole-document byte cap; response hex is capped before conversion.

use serde_json::{Map, Value};

use super::*;
use crate::live_invocation::identity::unhex;

const CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v1\0";
const EXECUTION_CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v2\0";
const MIGRATED_CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v3\0";
const PRICED_CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v4\0";
const PRICED_MIGRATED_CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v4-migrated\0";
const POLICY_CHAIN_DOMAIN: &[u8] = b"semaprax.live-invocation.source-chain.v6\0";

fn kind(entry: &SourceJournalEntry) -> &'static str {
    match entry {
        SourceJournalEntry::MigrationOpened { .. } => "migration_opened",
        SourceJournalEntry::MigrationEvaluationIntent { .. } => "migration_evaluation_intent",
        SourceJournalEntry::MigrationEvaluationSettled { .. } => "migration_evaluation_settled",
        SourceJournalEntry::MigrationEvaluationFailed { .. } => "migration_evaluation_failed",
        SourceJournalEntry::RunOpened => "run_opened",
        SourceJournalEntry::StageReservation { .. } => "stage_reservation",
        SourceJournalEntry::ReplayStageReservation { .. } => "replay_stage_reservation",
        SourceJournalEntry::TurnObserved { .. } => "turn_observed",
        SourceJournalEntry::AttemptIntent { .. } => "attempt_intent",
        SourceJournalEntry::AttemptSettled { .. } => "attempt_settled",
        SourceJournalEntry::AttemptFailed { .. } => "attempt_failed",
        SourceJournalEntry::AttemptUsage { .. } => "attempt_usage",
        SourceJournalEntry::PricedAttemptIntent(_) => "priced_attempt_intent",
        SourceJournalEntry::PricedAttemptUsage(_) => "priced_attempt_usage",
        SourceJournalEntry::PolicyAttemptIntent(_) => "policy_attempt_intent",
        SourceJournalEntry::PolicyAttemptUsage { .. } => "policy_attempt_usage",
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
        SourceJournalEntry::TerminalSnapshot { .. } => "terminal_snapshot",
    }
}

fn turn_attempt(entry: &SourceJournalEntry) -> (Option<u32>, Option<u32>) {
    match entry {
        SourceJournalEntry::MigrationOpened { .. } => (None, None),
        SourceJournalEntry::MigrationEvaluationIntent { attempt, .. }
        | SourceJournalEntry::MigrationEvaluationSettled { attempt, .. }
        | SourceJournalEntry::MigrationEvaluationFailed { attempt, .. } => (None, Some(*attempt)),
        SourceJournalEntry::RunOpened => (None, None),
        SourceJournalEntry::StageReservation { turn, attempt, .. } => (Some(*turn), *attempt),
        SourceJournalEntry::ReplayStageReservation { .. } => (None, None),
        SourceJournalEntry::TurnObserved { turn, .. } => (Some(*turn), None),
        SourceJournalEntry::Stop { turn, attempt, .. } => (*turn, *attempt),
        SourceJournalEntry::TerminalOutcome { turn, .. } => (*turn, None),
        SourceJournalEntry::TerminalSnapshot { turn, .. } => (*turn, None),
        SourceJournalEntry::AttemptIntent { turn, attempt, .. }
        | SourceJournalEntry::AttemptSettled { turn, attempt, .. }
        | SourceJournalEntry::AttemptFailed { turn, attempt, .. }
        | SourceJournalEntry::AttemptUsage { turn, attempt, .. }
        | SourceJournalEntry::ProposalRefused { turn, attempt, .. }
        | SourceJournalEntry::ProposalAdmitted { turn, attempt, .. }
        | SourceJournalEntry::AuthorizationConsumed { turn, attempt, .. }
        | SourceJournalEntry::AuthorizationRefused { turn, attempt, .. }
        | SourceJournalEntry::EffectIntent { turn, attempt, .. }
        | SourceJournalEntry::EffectObserved { turn, attempt, .. }
        | SourceJournalEntry::EffectFailed { turn, attempt, .. }
        | SourceJournalEntry::Transition { turn, attempt, .. } => (Some(*turn), Some(*attempt)),
        SourceJournalEntry::PricedAttemptIntent(intent) => {
            (Some(intent.turn), Some(intent.attempt))
        }
        SourceJournalEntry::PricedAttemptUsage(usage) => (Some(usage.turn), Some(usage.attempt)),
        SourceJournalEntry::PolicyAttemptIntent(intent) => {
            (Some(intent.turn), Some(intent.attempt))
        }
        SourceJournalEntry::PolicyAttemptUsage { turn, attempt, .. } => {
            (Some(*turn), Some(*attempt))
        }
    }
}

fn encode_optional_number(value: Option<u64>) -> String {
    value.map_or_else(|| "null".to_owned(), |number| number.to_string())
}
fn encode_usage(usage: &Option<SourceReportedUsage>) -> String {
    match usage {
        None => "null".to_owned(),
        Some(usage) => format!(
            "{{\"total\":{},\"input\":{},\"output\":{},\"reasoning\":{},\"cache_read\":{},\"cache_write\":{}}}",
            encode_optional_number(usage.total), encode_optional_number(usage.input),
            encode_optional_number(usage.output), encode_optional_number(usage.reasoning),
            encode_optional_number(usage.cache_read), encode_optional_number(usage.cache_write),
        ),
    }
}

pub(super) fn encode_entry(entry: &SourceJournalEntry, seq: usize) -> String {
    match entry {
        SourceJournalEntry::PricedAttemptIntent(intent) => return intent.render(seq),
        SourceJournalEntry::PricedAttemptUsage(usage) => return usage.render(seq),
        _ => {}
    }
    let (turn, attempt) = turn_attempt(entry);
    let mut output = format!("{{\"seq\":{},\"kind\":{}", seq, quote_json(kind(entry)));
    if let Some(turn) = turn {
        output.push_str(&format!(",\"turn\":{turn}"));
    }
    if let Some(attempt) = attempt {
        output.push_str(&format!(",\"attempt\":{attempt}"));
    }
    let fields = match entry {
        // Returned above so the typed V4 renderer remains the sole canonical
        // encoding authority for its closed payloads.
        SourceJournalEntry::PricedAttemptIntent(_) | SourceJournalEntry::PricedAttemptUsage(_) => {
            unreachable!("priced entries returned from encode_entry before field rendering")
        }
        SourceJournalEntry::PolicyAttemptIntent(intent) => format!(
            ",\"attempt_digest\":{},\"request_digest\":{},\"prompt_digest\":{},\"request_bytes\":{},\"reserved_units\":{},\"response_limit\":{},\"policy_ordinal\":{},\"policy_kind\":\"fresh\",\"provider\":{},\"reserved_context_tokens\":{},\"reserved_output_tokens\":{},\"reserved_cost_micros\":{}",
            quote_json(&intent.attempt_digest), quote_json(&intent.request_digest), quote_json(&intent.prompt_digest),
            intent.request_bytes, intent.reserved_units, intent.response_limit, intent.reservation.ordinal,
            quote_json(&intent.reservation.provider_id), intent.reservation.reserved_context_tokens,
            intent.reservation.reserved_output_tokens, intent.reservation.reserved_cost_micros),
        SourceJournalEntry::PolicyAttemptUsage { ordinal, usage, .. } => match usage {
            PolicyAttemptUsageV6::Unknown => format!(",\"policy_ordinal\":{},\"usage\":\"unknown\"", ordinal),
            PolicyAttemptUsageV6::Observed { context_tokens, output_tokens, cost_micros } => format!(
                ",\"policy_ordinal\":{},\"usage\":\"observed\",\"context_tokens\":{},\"output_tokens\":{},\"cost_micros\":{}",
                ordinal, context_tokens, output_tokens, cost_micros),
        },
        SourceJournalEntry::MigrationOpened { handoff_digest } =>
            format!(",\"handoff_digest\":{}", quote_json(handoff_digest)),
        SourceJournalEntry::MigrationEvaluationIntent { fuel, .. } =>
            format!(",\"fuel\":{fuel}"),
        SourceJournalEntry::MigrationEvaluationSettled { state, state_digest, .. } => format!(
            ",\"state\":{},\"state_digest\":{}",
            quote_json(&hex(state)), quote_json(state_digest)),
        SourceJournalEntry::MigrationEvaluationFailed { reason, .. } =>
            format!(",\"reason\":{}", quote_json(reason.as_str())),
        SourceJournalEntry::RunOpened => String::new(),
        SourceJournalEntry::StageReservation { role, fuel, .. } => format!(
            ",\"role\":{},\"fuel\":{}", quote_json(role.as_str()), fuel),
        SourceJournalEntry::ReplayStageReservation { replay, causal_seq, role, fuel } => format!(
            ",\"replay\":{},\"causal_seq\":{},\"role\":{},\"fuel\":{}",
            replay, causal_seq, quote_json(role.as_str()), fuel),
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
        SourceJournalEntry::AttemptUsage { reported, .. } =>
            format!(",\"reported\":{}", encode_usage(reported)),
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
        SourceJournalEntry::TerminalSnapshot {
            status, carrier_digest, carrier, evidence, evidence_digest,
            committed_model_units, committed_stage_fuel, stages, effects, attempts, ..
        } => format!(
            ",\"status\":{},\"carrier_digest\":{},\"carrier\":{},\"evidence\":{},\"evidence_digest\":{},\"committed_model_units\":{},\"committed_stage_fuel\":{},\"stages\":{},\"effects\":{},\"attempts\":{}",
            quote_json(status.as_str()),
            carrier_digest.as_deref().map(quote_json).unwrap_or_else(|| "null".to_owned()),
            carrier.as_ref().map(|bytes| quote_json(&hex(bytes))).unwrap_or_else(|| "null".to_owned()),
            quote_json(&hex(evidence)), quote_json(evidence_digest),
            committed_model_units, committed_stage_fuel, stages, effects, attempts,
        ),
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
        quote_json(journal.binding.schema()), quote_json(journal.binding.invocation()), generation,
        quote_json(journal.binding.clock_domain()), journal.last_checked_millis(), entries)
}

pub(super) fn encode_envelope(
    journal: &SourceJournal,
    generation: u64,
) -> Result<String, SourceJournalError> {
    if generation == 0 || generation != journal.combined_len() as u64 {
        return Err(SourceJournalError::Generation);
    }
    let entries = if journal.binding.wait.is_some() {
        let mut rows = Vec::new();
        for (seq, row) in journal.execution_entries_v7() {
            rows.push(match row {
                SourceExecutionEntryV7::Ordinary(entry) => encode_entry(&entry, seq as usize),
                SourceExecutionEntryV7::Wait(entry) => wait_v7::wire::encode(&entry, seq),
            });
        }
        format!("[{}]", rows.join(","))
    } else {
        encode_entries(journal.entries())?
    };
    let link = digest(
        if journal.binding.wait.is_some() {
            b"semaprax.live-invocation.source-chain.v7\0"
        } else if journal.binding.policy_binding().is_some() {
            POLICY_CHAIN_DOMAIN
        } else if journal.binding.io_limits().is_some() {
            b"semaprax.live-invocation.source-chain.v5\0"
        } else if journal.binding.is_priced_migrated_profile() {
            PRICED_MIGRATED_CHAIN_DOMAIN
        } else if journal.binding.is_priced_profile() {
            PRICED_CHAIN_DOMAIN
        } else if journal.binding.migration().is_some() {
            MIGRATED_CHAIN_DOMAIN
        } else if journal.binding.is_execution_profile() {
            EXECUTION_CHAIN_DOMAIN
        } else {
            CHAIN_DOMAIN
        },
        chain_input(journal, generation, &entries).as_bytes(),
    );
    let document = format!(
        "{{\"schema\":{},\"invocation\":{},\"generation\":{},\"clock_domain\":{},\"last_checked_millis\":{},\"chain\":{},\"entries\":{}}}\n",
        quote_json(journal.binding.schema()), quote_json(journal.binding.invocation()), generation,
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
fn optional_bytes(
    map: &Map<String, Value>,
    key: &str,
    cap: usize,
) -> Result<Option<Vec<u8>>, SourceJournalError> {
    match map.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(_)) => bytes_field(map, key, cap).map(Some),
        _ => Err(SourceJournalError::Malformed),
    }
}
fn optional_number(map: &Map<String, Value>, key: &str) -> Result<Option<u64>, SourceJournalError> {
    match map.get(key) {
        Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_u64()
            .map(Some)
            .ok_or(SourceJournalError::Malformed),
        None => Err(SourceJournalError::Malformed),
    }
}
fn decode_usage(
    map: &Map<String, Value>,
) -> Result<Option<SourceReportedUsage>, SourceJournalError> {
    match map.get("reported") {
        Some(Value::Null) => Ok(None),
        Some(value) => {
            let usage = object(value)?;
            Ok(Some(SourceReportedUsage {
                total: optional_number(usage, "total")?,
                input: optional_number(usage, "input")?,
                output: optional_number(usage, "output")?,
                reasoning: optional_number(usage, "reasoning")?,
                cache_read: optional_number(usage, "cache_read")?,
                cache_write: optional_number(usage, "cache_write")?,
            }))
        }
        None => Err(SourceJournalError::Malformed),
    }
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

pub(super) fn decode_entry(
    value: &Value,
    seq: usize,
    expected: &SourceInvocationBinding,
) -> Result<SourceJournalEntry, SourceJournalError> {
    let map = object(value)?;
    if usize_field(map, "seq")? != seq {
        return Err(SourceJournalError::Order);
    }
    let turn = || u32_field(map, "turn");
    let attempt = || u32_field(map, "attempt");
    let entry = match string(map, "kind")?.as_str() {
        "migration_opened" => SourceJournalEntry::MigrationOpened {
            handoff_digest: string(map, "handoff_digest")?,
        },
        "migration_evaluation_intent" => SourceJournalEntry::MigrationEvaluationIntent {
            attempt: attempt()?,
            fuel: usize_field(map, "fuel")?,
        },
        "migration_evaluation_settled" => SourceJournalEntry::MigrationEvaluationSettled {
            attempt: attempt()?,
            state: bytes_field(map, "state", MAX_SOURCE_CARRIER_BYTES)?,
            state_digest: string(map, "state_digest")?,
        },
        "migration_evaluation_failed" => SourceJournalEntry::MigrationEvaluationFailed {
            attempt: attempt()?,
            reason: tag(map, "reason", SourceMigrationFailure::parse)?,
        },
        "run_opened" => SourceJournalEntry::RunOpened,
        "stage_reservation" => SourceJournalEntry::StageReservation {
            turn: turn()?,
            attempt: optional_u32(map, "attempt")?,
            role: tag(map, "role", SourceStageRole::parse)?,
            fuel: usize_field(map, "fuel")?,
        },
        "replay_stage_reservation" => SourceJournalEntry::ReplayStageReservation {
            replay: u32_field(map, "replay")?,
            causal_seq: u32_field(map, "causal_seq")?,
            role: tag(map, "role", SourceStageRole::parse)?,
            fuel: usize_field(map, "fuel")?,
        },
        "turn_observed" => SourceJournalEntry::TurnObserved {
            turn: turn()?,
            state: string(map, "state")?,
            observation: string(map, "observation")?,
            feedback: string(map, "feedback")?,
        },
        "attempt_intent"
            if !expected.is_priced_profile() && expected.policy_binding().is_none() =>
        {
            SourceJournalEntry::AttemptIntent {
                turn: turn()?,
                attempt: attempt()?,
                attempt_digest: string(map, "attempt_digest")?,
                request_digest: string(map, "request_digest")?,
                prompt_digest: string(map, "prompt_digest")?,
                request_bytes: usize_field(map, "request_bytes")?,
                reserved_units: i64_field(map, "reserved_units")?,
                response_limit: usize_field(map, "response_limit")?,
            }
        }
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
        "attempt_usage" if !expected.is_priced_profile() && expected.policy_binding().is_none() => {
            SourceJournalEntry::AttemptUsage {
                turn: turn()?,
                attempt: attempt()?,
                reported: decode_usage(map)?,
            }
        }
        "priced_attempt_intent" if expected.is_priced_profile() => {
            SourceJournalEntry::PricedAttemptIntent(
                super::priced_v4::PricedAttemptIntentV4::decode(
                    value,
                    seq,
                    expected
                        .priced_binding()
                        .ok_or(SourceJournalError::Binding)?,
                )?,
            )
        }
        "priced_attempt_usage" if expected.is_priced_profile() => {
            SourceJournalEntry::PricedAttemptUsage(super::priced_v4::PricedAttemptUsageV4::decode(
                value,
                seq,
                expected
                    .priced_binding()
                    .ok_or(SourceJournalError::Binding)?,
            )?)
        }
        "policy_attempt_intent" if expected.policy_binding().is_some() => {
            let kind = string(map, "policy_kind")?;
            if kind != "fresh" {
                return Err(SourceJournalError::Malformed);
            }
            SourceJournalEntry::PolicyAttemptIntent(PolicyAttemptIntentV6 {
                turn: turn()?,
                attempt: attempt()?,
                attempt_digest: string(map, "attempt_digest")?,
                request_digest: string(map, "request_digest")?,
                prompt_digest: string(map, "prompt_digest")?,
                request_bytes: usize_field(map, "request_bytes")?,
                reserved_units: i64_field(map, "reserved_units")?,
                response_limit: usize_field(map, "response_limit")?,
                reservation: PolicyAttemptReservationV6 {
                    ordinal: u64_field(map, "policy_ordinal")?,
                    kind: crate::model_budget_policy::AttemptKind::Fresh,
                    provider_id: string(map, "provider")?,
                    reserved_context_tokens: u64_field(map, "reserved_context_tokens")?,
                    reserved_output_tokens: u64_field(map, "reserved_output_tokens")?,
                    reserved_cost_micros: i64_field(map, "reserved_cost_micros")?,
                },
            })
        }
        "policy_attempt_usage" if expected.policy_binding().is_some() => {
            let usage = match string(map, "usage")?.as_str() {
                "unknown" => PolicyAttemptUsageV6::Unknown,
                "observed" => PolicyAttemptUsageV6::Observed {
                    context_tokens: u64_field(map, "context_tokens")?,
                    output_tokens: u64_field(map, "output_tokens")?,
                    cost_micros: i64_field(map, "cost_micros")?,
                },
                _ => return Err(SourceJournalError::Malformed),
            };
            SourceJournalEntry::PolicyAttemptUsage {
                turn: turn()?,
                attempt: attempt()?,
                ordinal: u64_field(map, "policy_ordinal")?,
                usage,
            }
        }
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
        "terminal_snapshot" => SourceJournalEntry::TerminalSnapshot {
            turn: optional_u32(map, "turn")?,
            status: tag(map, "status", SourceTerminalStatus::parse)?,
            carrier_digest: optional_digest(map, "carrier_digest")?,
            carrier: optional_bytes(map, "carrier", MAX_SOURCE_CARRIER_BYTES)?,
            evidence: bytes_field(map, "evidence", MAX_SOURCE_TERMINAL_EVIDENCE_BYTES)?,
            evidence_digest: string(map, "evidence_digest")?,
            committed_model_units: i64_field(map, "committed_model_units")?,
            committed_stage_fuel: u64_field(map, "committed_stage_fuel")?,
            stages: u32_field(map, "stages")?,
            effects: u32_field(map, "effects")?,
            attempts: u32_field(map, "attempts")?,
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
    if string(map, "schema")? != expected.schema() {
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
    let mut wait_entries = Vec::new();
    for (seq, item) in raw_entries.iter().enumerate() {
        if expected.wait.is_some()
            && item["kind"]
                .as_str()
                .is_some_and(|kind| kind.starts_with("wait_"))
        {
            wait_entries.push((seq as u32, wait_v7::wire::decode(item, seq as u32)?));
        } else {
            entries.push(decode_entry(item, seq, expected)?);
        }
    }
    let journal = SourceJournal {
        binding: expected.clone(),
        entries,
        wait_entries,
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
