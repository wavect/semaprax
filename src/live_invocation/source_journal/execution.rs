//! V2 execution profile over the same append-only source checkpoint.
//! Causal entries use the v1 phase grammar after projecting out fuel and
//! observational usage; this pass owns their additional execution ordering.

use serde_json::{Map, Value};

use super::*;

const EVIDENCE_SCHEMA: &str = "semaprax.agent-source-terminal-evidence.v2";
const EVIDENCE_DOMAIN: &[u8] = b"semaprax.agent-source-terminal-evidence.v2\0";
const CARRIER_DOMAIN: &[u8] = b"semaprax.agent-step.value.v2\0";
pub(super) const TERMINAL_ROOM_BYTES: usize =
    2 * MAX_SOURCE_TERMINAL_EVIDENCE_BYTES + 2 * MAX_SOURCE_CARRIER_BYTES + 4_096;

#[derive(Default)]
pub(super) struct ExecutionFold {
    pub model_units: i64,
    pub stage_fuel: u64,
    pub stages: u32,
    pub effects: u32,
    pub attempts: u32,
    roles: Vec<SourceStageRole>,
}

#[derive(Clone, Copy)]
struct RequiredStage {
    role: SourceStageRole,
    turn: u32,
    attempt: Option<u32>,
}
#[derive(Clone, Copy)]
struct OriginalStage {
    seq: u32,
    role: SourceStageRole,
    fuel: usize,
}
struct ReplayPass {
    number: u32,
    limit: usize,
    next: usize,
}

fn stage_fields_valid(
    binding: &SourceInvocationBinding,
    input: &SourceTerminalEvidenceInput,
    fold: &ExecutionFold,
) -> bool {
    let Some(max_steps) = binding.max_steps_per_stage() else {
        return false;
    };
    if input.completed_stages > fold.stages
        || input
            .stage_rows
            .len()
            .checked_add(input.omitted_stage_rows as usize)
            != Some(input.completed_stages as usize)
        || input.stage_rows.len() > fold.roles.len()
        || input.checked_run_evidence.as_ref().is_some_and(|bytes| {
            bytes.len() > MAX_SOURCE_TERMINAL_EVIDENCE_BYTES || std::str::from_utf8(bytes).is_err()
        })
    {
        return false;
    }
    input.stage_rows.iter().enumerate().all(|(index, row)| {
        row.role == fold.roles[index]
            && row.steps_used <= max_steps
            && !row.function_id.is_empty()
            && row.function_id.len() <= 240
            && !row.function_id.chars().any(char::is_control)
    })
}

fn optional_u32(value: Option<u32>) -> String {
    value.map_or_else(|| "null".to_owned(), |number| number.to_string())
}
fn optional_digest(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_owned(), quote_json)
}
fn render_evidence(
    binding: &SourceInvocationBinding,
    fold: &ExecutionFold,
    turn: Option<u32>,
    status: SourceTerminalStatus,
    carrier_digest: Option<&str>,
    input: &SourceTerminalEvidenceInput,
) -> Result<Vec<u8>, SourceJournalError> {
    if !stage_fields_valid(binding, input, fold) {
        return Err(SourceJournalError::Malformed);
    }
    let mut rows = String::from("[");
    for (index, row) in input.stage_rows.iter().enumerate() {
        if index != 0 {
            rows.push(',');
        }
        rows.push_str(&format!(
            "[{}, {},{},{}]",
            quote_json(row.role.as_str()),
            quote_json(&row.function_id),
            quote_json(row.outcome.as_str()),
            row.steps_used,
        ));
    }
    rows.push(']');
    let checked = input.checked_run_evidence.as_ref().map_or_else(
        || "null".to_owned(),
        |bytes| quote_json(std::str::from_utf8(bytes).expect("checked above")),
    );
    let rendered = format!(
        "{{\"schema\":{},\"invocation\":{},\"status\":{},\"turn\":{},\"carrier_digest\":{},\"committed_model_units\":{},\"committed_stage_fuel\":{},\"stages\":{},\"effects\":{},\"attempts\":{},\"completed_stages\":{},\"omitted_stage_rows\":{},\"stage_rows\":{},\"checked_run_evidence\":{}}}\n",
        quote_json(EVIDENCE_SCHEMA), quote_json(binding.invocation()),
        quote_json(status.as_str()), optional_u32(turn), optional_digest(carrier_digest),
        fold.model_units, fold.stage_fuel, fold.stages, fold.effects, fold.attempts,
        input.completed_stages, input.omitted_stage_rows, rows, checked,
    );
    if rendered.len() > MAX_SOURCE_TERMINAL_EVIDENCE_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    Ok(rendered.into_bytes())
}

fn evidence_object(value: &Value) -> Result<&Map<String, Value>, SourceJournalError> {
    value.as_object().ok_or(SourceJournalError::Malformed)
}
fn evidence_number(map: &Map<String, Value>, key: &str) -> Result<u32, SourceJournalError> {
    map.get(key)
        .and_then(Value::as_u64)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or(SourceJournalError::Malformed)
}
fn decode_evidence_input(bytes: &[u8]) -> Result<SourceTerminalEvidenceInput, SourceJournalError> {
    if bytes.len() > MAX_SOURCE_TERMINAL_EVIDENCE_BYTES {
        return Err(SourceJournalError::Capacity);
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| SourceJournalError::Malformed)?;
    let map = evidence_object(&value)?;
    let keys = [
        "schema",
        "invocation",
        "status",
        "turn",
        "carrier_digest",
        "committed_model_units",
        "committed_stage_fuel",
        "stages",
        "effects",
        "attempts",
        "completed_stages",
        "omitted_stage_rows",
        "stage_rows",
        "checked_run_evidence",
    ];
    if map.len() != keys.len() || !keys.iter().all(|key| map.contains_key(*key)) {
        return Err(SourceJournalError::Malformed);
    }
    if map.get("schema").and_then(Value::as_str) != Some(EVIDENCE_SCHEMA) {
        return Err(SourceJournalError::Malformed);
    }
    let rows = map
        .get("stage_rows")
        .and_then(Value::as_array)
        .ok_or(SourceJournalError::Malformed)?;
    if rows.len() > 12_289 {
        return Err(SourceJournalError::Capacity);
    }
    let mut stage_rows = Vec::with_capacity(rows.len());
    for row in rows {
        let fields = row.as_array().ok_or(SourceJournalError::Malformed)?;
        if fields.len() != 4 {
            return Err(SourceJournalError::Malformed);
        }
        let role = fields[0]
            .as_str()
            .and_then(SourceStageRole::parse)
            .ok_or(SourceJournalError::Malformed)?;
        let function_id = fields[1]
            .as_str()
            .ok_or(SourceJournalError::Malformed)?
            .to_owned();
        let outcome = fields[2]
            .as_str()
            .and_then(SourceStageOutcome::parse)
            .ok_or(SourceJournalError::Malformed)?;
        let steps_used = fields[3]
            .as_u64()
            .and_then(|number| usize::try_from(number).ok())
            .ok_or(SourceJournalError::Malformed)?;
        stage_rows.push(SourceStageSummary {
            role,
            function_id,
            outcome,
            steps_used,
        });
    }
    let checked_run_evidence = match map.get("checked_run_evidence") {
        Some(Value::Null) => None,
        Some(Value::String(value)) => Some(value.as_bytes().to_vec()),
        _ => return Err(SourceJournalError::Malformed),
    };
    Ok(SourceTerminalEvidenceInput {
        completed_stages: evidence_number(map, "completed_stages")?,
        omitted_stage_rows: evidence_number(map, "omitted_stage_rows")?,
        stage_rows,
        checked_run_evidence,
    })
}

pub(super) fn terminal_entry(
    binding: &SourceInvocationBinding,
    fold: &ExecutionFold,
    turn: Option<u32>,
    status: SourceTerminalStatus,
    carrier: Option<Vec<u8>>,
    input: SourceTerminalEvidenceInput,
) -> Result<SourceJournalEntry, SourceJournalError> {
    if carrier
        .as_ref()
        .is_some_and(|bytes| bytes.len() > MAX_SOURCE_CARRIER_BYTES)
    {
        return Err(SourceJournalError::Capacity);
    }
    if carrier
        .as_ref()
        .is_some_and(|bytes| serde_json::from_slice::<Value>(bytes).is_err())
    {
        return Err(SourceJournalError::Malformed);
    }
    let carrier_digest = carrier
        .as_deref()
        .map(|bytes| digest(CARRIER_DOMAIN, bytes));
    let evidence = render_evidence(
        binding,
        fold,
        turn,
        status,
        carrier_digest.as_deref(),
        &input,
    )?;
    let evidence_digest = digest(EVIDENCE_DOMAIN, &evidence);
    Ok(SourceJournalEntry::TerminalSnapshot {
        turn,
        status,
        carrier_digest,
        carrier,
        evidence,
        evidence_digest,
        committed_model_units: fold.model_units,
        committed_stage_fuel: fold.stage_fuel,
        stages: fold.stages,
        effects: fold.effects,
        attempts: fold.attempts,
    })
}

fn check_terminal(
    binding: &SourceInvocationBinding,
    fold: &ExecutionFold,
    entry: &SourceJournalEntry,
) -> Result<(), SourceJournalError> {
    let SourceJournalEntry::TerminalSnapshot {
        turn,
        status,
        carrier_digest,
        carrier,
        evidence,
        evidence_digest,
        committed_model_units,
        committed_stage_fuel,
        stages,
        effects,
        attempts,
    } = entry
    else {
        return Err(SourceJournalError::Order);
    };
    if *committed_model_units != fold.model_units
        || *committed_stage_fuel != fold.stage_fuel
        || *stages != fold.stages
        || *effects != fold.effects
        || *attempts != fold.attempts
        || evidence.len() > MAX_SOURCE_TERMINAL_EVIDENCE_BYTES
        || carrier
            .as_ref()
            .is_some_and(|bytes| bytes.len() > MAX_SOURCE_CARRIER_BYTES)
        || carrier.is_some() != carrier_digest.is_some()
        || carrier
            .as_deref()
            .map(|bytes| digest(CARRIER_DOMAIN, bytes))
            != *carrier_digest
        || digest(EVIDENCE_DOMAIN, evidence) != *evidence_digest
    {
        return Err(SourceJournalError::Malformed);
    }
    let input = decode_evidence_input(evidence)?;
    if render_evidence(
        binding,
        fold,
        *turn,
        *status,
        carrier_digest.as_deref(),
        &input,
    )? != *evidence
    {
        return Err(SourceJournalError::Malformed);
    }
    if carrier
        .as_ref()
        .is_some_and(|bytes| serde_json::from_slice::<Value>(bytes).is_err())
    {
        return Err(SourceJournalError::Malformed);
    }
    Ok(())
}

/// Validates one v2 prefix and returns its one checked model/fuel fold.
pub(super) fn validate(
    binding: &SourceInvocationBinding,
    entries: &[SourceJournalEntry],
) -> Result<ExecutionFold, SourceJournalError> {
    let Some(stage_allowance) = binding.max_steps_per_stage() else {
        return Err(SourceJournalError::Binding);
    };
    let Some(max_fuel) = binding.max_total_steps() else {
        return Err(SourceJournalError::Binding);
    };
    if entries.len() > MAX_SOURCE_ENTRIES {
        return Err(SourceJournalError::Capacity);
    }
    let mut fold = ExecutionFold::default();
    let mut causal = Vec::with_capacity(entries.len());
    let mut originals: Vec<OriginalStage> = Vec::new();
    let mut required: Option<RequiredStage> = None;
    let mut last_stage: Option<RequiredStage> = None;
    let mut replay: Option<ReplayPass> = None;
    let mut last_causal: Option<&SourceJournalEntry> = None;

    for (index, entry) in entries.iter().enumerate() {
        match entry {
            SourceJournalEntry::ReplayStageReservation {
                replay: number,
                causal_seq,
                role,
                fuel,
            } => {
                if matches!(
                    last_causal,
                    None | Some(
                        SourceJournalEntry::AttemptIntent { .. }
                            | SourceJournalEntry::EffectIntent { .. }
                            | SourceJournalEntry::Stop { .. }
                            | SourceJournalEntry::TerminalSnapshot { .. }
                    )
                ) || originals.is_empty()
                {
                    return Err(SourceJournalError::Order);
                }
                let pass = match replay.take() {
                    None if *number == 0 => ReplayPass {
                        number: 0,
                        limit: originals.len(),
                        next: 0,
                    },
                    Some(pass) if *number == pass.number => pass,
                    Some(pass) if pass.number.checked_add(1) == Some(*number) => ReplayPass {
                        number: *number,
                        limit: originals.len(),
                        next: 0,
                    },
                    _ => return Err(SourceJournalError::Order),
                };
                let original = originals
                    .get(pass.next)
                    .filter(|_| pass.next < pass.limit)
                    .ok_or(SourceJournalError::Order)?;
                if pass.next == 0 && original.role != SourceStageRole::Initialize {
                    return Err(SourceJournalError::Order);
                }
                if original.seq != *causal_seq || original.role != *role || original.fuel != *fuel {
                    return Err(SourceJournalError::Order);
                }
                fold.stage_fuel = fold
                    .stage_fuel
                    .checked_add(*fuel as u64)
                    .ok_or(SourceJournalError::Capacity)?;
                if fold.stage_fuel > max_fuel as u64 {
                    return Err(SourceJournalError::Capacity);
                }
                replay = Some(ReplayPass {
                    next: pass.next + 1,
                    ..pass
                });
                continue;
            }
            SourceJournalEntry::AttemptUsage { turn, attempt, .. } => {
                if !matches!(entries.get(index.wrapping_sub(1)),
                    Some(SourceJournalEntry::AttemptSettled { turn: prior_turn, attempt: prior_attempt, .. }
                        | SourceJournalEntry::AttemptFailed { turn: prior_turn, attempt: prior_attempt, .. })
                        if prior_turn == turn && prior_attempt == attempt)
                {
                    return Err(SourceJournalError::Order);
                }
                continue;
            }
            _ => {}
        }
        if replay.as_ref().is_some_and(|pass| pass.next < pass.limit)
            && !matches!(entry, SourceJournalEntry::Stop { .. })
            && !(matches!(entry, SourceJournalEntry::TerminalSnapshot { .. })
                && matches!(last_causal, Some(SourceJournalEntry::Stop { .. })))
        {
            return Err(SourceJournalError::Order);
        }
        if matches!(
            last_causal,
            Some(SourceJournalEntry::TerminalSnapshot { .. })
        ) {
            return Err(SourceJournalError::Order);
        }
        match entry {
            SourceJournalEntry::RunOpened => {
                required = Some(RequiredStage {
                    role: SourceStageRole::Initialize,
                    turn: 0,
                    attempt: None,
                });
            }
            SourceJournalEntry::StageReservation {
                turn,
                attempt,
                role,
                fuel,
            } => {
                if matches!(last_causal, Some(SourceJournalEntry::Stop { .. })) {
                    return Err(SourceJournalError::Order);
                }
                let expected = required.take().ok_or(SourceJournalError::Order)?;
                if *turn != expected.turn
                    || *attempt != expected.attempt
                    || *role != expected.role
                    || *fuel != stage_allowance
                {
                    return Err(SourceJournalError::Order);
                }
                let seq = u32::try_from(index).map_err(|_| SourceJournalError::Capacity)?;
                originals.push(OriginalStage {
                    seq,
                    role: *role,
                    fuel: *fuel,
                });
                fold.roles.push(*role);
                fold.stages = fold
                    .stages
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?;
                fold.stage_fuel = fold
                    .stage_fuel
                    .checked_add(*fuel as u64)
                    .ok_or(SourceJournalError::Capacity)?;
                if fold.stages > binding.max_stages || fold.stage_fuel > max_fuel as u64 {
                    return Err(SourceJournalError::Capacity);
                }
                last_stage = Some(expected);
                if *role == SourceStageRole::Initialize {
                    required = Some(RequiredStage {
                        role: SourceStageRole::Observe,
                        turn: 0,
                        attempt: None,
                    });
                }
                last_causal = Some(entry);
                continue;
            }
            SourceJournalEntry::TurnObserved { turn, .. } => {
                if !matches!(last_stage, Some(RequiredStage {
                    role: SourceStageRole::Observe, turn: prior, attempt: None }) if prior == *turn)
                {
                    return Err(SourceJournalError::Order);
                }
                last_stage = None;
            }
            SourceJournalEntry::ProposalAdmitted { turn, attempt, .. } => {
                required = Some(RequiredStage {
                    role: SourceStageRole::Authorize,
                    turn: *turn,
                    attempt: Some(*attempt),
                });
            }
            SourceJournalEntry::AuthorizationConsumed { turn, attempt, .. }
            | SourceJournalEntry::AuthorizationRefused { turn, attempt, .. } => {
                if !matches!(last_stage, Some(RequiredStage {
                    role: SourceStageRole::Authorize, turn: prior_turn,
                    attempt: Some(prior_attempt) }) if prior_turn == *turn && prior_attempt == *attempt)
                {
                    return Err(SourceJournalError::Order);
                }
                last_stage = None;
            }
            SourceJournalEntry::EffectIntent { .. } => {
                if fold
                    .stages
                    .checked_add(1)
                    .is_none_or(|value| value > binding.max_stages)
                    || fold
                        .stage_fuel
                        .checked_add(stage_allowance as u64)
                        .is_none_or(|value| value > max_fuel as u64)
                {
                    return Err(SourceJournalError::Capacity);
                }
                fold.effects = fold
                    .effects
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?;
            }
            SourceJournalEntry::EffectObserved { turn, attempt, .. } => {
                required = Some(RequiredStage {
                    role: SourceStageRole::Reduce,
                    turn: *turn,
                    attempt: Some(*attempt),
                });
            }
            SourceJournalEntry::Transition {
                turn,
                attempt,
                case,
                ..
            } => {
                if !matches!(last_stage, Some(RequiredStage {
                    role: SourceStageRole::Reduce, turn: prior_turn,
                    attempt: Some(prior_attempt) }) if prior_turn == *turn && prior_attempt == *attempt)
                {
                    return Err(SourceJournalError::Order);
                }
                last_stage = None;
                if *case == SourceTransitionCase::Continue {
                    required = Some(RequiredStage {
                        role: SourceStageRole::Observe,
                        turn: turn.checked_add(1).ok_or(SourceJournalError::Capacity)?,
                        attempt: None,
                    });
                }
            }
            SourceJournalEntry::AttemptIntent { reserved_units, .. } => {
                fold.attempts = fold
                    .attempts
                    .checked_add(1)
                    .ok_or(SourceJournalError::Capacity)?;
                fold.model_units = fold
                    .model_units
                    .checked_add(*reserved_units)
                    .ok_or(SourceJournalError::Capacity)?;
                if fold.model_units > binding.ceiling {
                    return Err(SourceJournalError::Capacity);
                }
            }
            SourceJournalEntry::TerminalSnapshot { .. } => check_terminal(binding, &fold, entry)?,
            SourceJournalEntry::TerminalOutcome { .. } => return Err(SourceJournalError::Order),
            _ => {}
        }
        let projected = match entry {
            SourceJournalEntry::TerminalSnapshot {
                turn,
                status,
                carrier_digest,
                ..
            } => SourceJournalEntry::TerminalOutcome {
                turn: *turn,
                status: *status,
                carrier_digest: carrier_digest.clone(),
            },
            _ => entry.clone(),
        };
        causal.push(projected);
        last_causal = Some(entry);
    }
    if validate::validate(binding, &causal)? != fold.model_units {
        return Err(SourceJournalError::Malformed);
    }
    Ok(fold)
}
