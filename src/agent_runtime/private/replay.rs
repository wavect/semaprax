//! Independent replay of rendered trace and evidence documents against the
//! replayed state machine.

use super::admission::parse_profile;
use super::evidence::{render_budget, render_event, render_production_limits, render_usage};
use super::*;

pub(in crate::agent_runtime) fn replay_trace(source: &str) -> Result<(), Diagnostic> {
    let value = canonical_document(source, "trace", TRACE_SCHEMA, MAX_TRACE_BYTES as usize)?;
    let top = object(&value, "trace", TRACE_SCHEMA)?;
    if !exact_keys(
        top,
        &[
            "schema",
            "run_id",
            "profile_digest",
            "task_digest",
            "events",
            "usage",
            "termination",
            "nonclaims",
        ],
    ) {
        return Err(g204("trace", TRACE_SCHEMA));
    }
    let events = top["events"]
        .as_array()
        .ok_or_else(|| g204("trace", TRACE_SCHEMA))?;
    if events.is_empty() || events.len() > MAX_TRACE_EVENTS as usize {
        return Err(g209());
    }
    let run_id_value = top["run_id"].as_str().ok_or_else(g209)?;
    let profile_digest = top["profile_digest"].as_str().ok_or_else(g209)?;
    let task_digest = top["task_digest"].as_str().ok_or_else(g209)?;
    if !canonical_sha256(run_id_value)
        || !canonical_sha256(profile_digest)
        || !canonical_sha256(task_digest)
    {
        return Err(g209());
    }
    let mut seen_finished = false;
    let mut usage_sum = UsageDelta::default();
    let mut maximum_turn = 0;
    let mut provider_attempts = 0;
    let mut tool_calls = 0;
    for (index, event) in events.iter().enumerate() {
        let event = event.as_object().ok_or_else(g209)?;
        if !exact_keys(
            event,
            &[
                "index",
                "turn",
                "kind",
                "provider_id",
                "model_id",
                "tool_id",
                "input_digest",
                "output_digest",
                "status",
                "usage",
            ],
        ) || event["index"].as_u64() != Some(index as u64)
        {
            return Err(g209());
        }
        let kind = event["kind"].as_str().ok_or_else(g209)?;
        let status = event["status"].as_str().ok_or_else(g209)?;
        let turn = event["turn"].as_u64().ok_or_else(g209)?;
        if kind != "run_finished" {
            maximum_turn = maximum_turn.max(turn);
        }
        let delta = parse_usage_delta(&event["usage"])?;
        add_usage_delta(&mut usage_sum, delta)?;
        provider_attempts += u64::from(kind == "provider_attempt_started");
        tool_calls += u64::from(kind == "tool_authorized");
        let allowed = match kind {
            "run_started" => status == "started" && index == 0,
            "route_selected" => status == "selected",
            "provider_attempt_started" => status == "started",
            "provider_attempt_finished" => matches!(
                status,
                "succeeded"
                    | "definitely_not_started"
                    | "failed_uncertain"
                    | "cancelled"
                    | "deadline_exceeded"
                    | "policy_rejected"
            ),
            "action_accepted" => matches!(status, "final" | "tool"),
            "tool_authorized" => status == "authorized",
            "tool_finished" => matches!(
                status,
                "succeeded" | "failed" | "cancelled" | "deadline_exceeded" | "policy_rejected"
            ),
            "run_finished" => {
                seen_finished = true;
                index + 1 == events.len()
                    && matches!(
                        status,
                        "completed"
                            | "cancelled"
                            | "deadline_exceeded"
                            | "budget_exhausted"
                            | "provider_failed"
                            | "tool_failed"
                            | "policy_rejected"
                    )
            }
            _ => false,
        };
        if !allowed
            || !valid_event_shape(event, kind, status)
            || (seen_finished && index + 1 != events.len())
        {
            return Err(g209());
        }
    }
    if !seen_finished || string_array_member(top, "nonclaims", "trace", TRACE_SCHEMA)? != NONCLAIMS
    {
        return Err(g209());
    }
    validate_event_sequence(events, profile_digest, task_digest)?;
    let usage = parse_usage(&top["usage"])?;
    if usage.turns != maximum_turn
        || usage.provider_attempts != provider_attempts
        || usage.provider_input_bytes != usage_sum.provider_input_bytes
        || usage.provider_output_bytes != usage_sum.provider_output_bytes
        || usage.reported_model_input_tokens != usage_sum.reported_model_input_tokens
        || usage.reported_model_output_tokens != usage_sum.reported_model_output_tokens
        || usage.usd_microunits != usage_sum.usd_microunits
        || usage.tool_calls != tool_calls
        || usage.tool_argument_bytes != usage_sum.tool_argument_bytes
        || usage.tool_result_bytes != usage_sum.tool_result_bytes
        || usage.elapsed_ms != usage_sum.elapsed_ms
        || usage.max_concurrency != 1
    {
        return Err(g209());
    }
    let termination = object(&top["termination"], "trace", TRACE_SCHEMA)?;
    if !exact_keys(termination, &["status", "code", "message"])
        || termination["status"] != events.last().ok_or_else(g209)?["status"]
    {
        return Err(g209());
    }
    validate_termination(termination)?;
    Ok(())
}

fn validate_event_sequence(
    events: &[Value],
    profile_digest: &str,
    task_digest: &str,
) -> Result<(), Diagnostic> {
    let final_status = events
        .last()
        .and_then(Value::as_object)
        .and_then(|event| event["status"].as_str())
        .ok_or_else(g209)?;
    let first = events.first().and_then(Value::as_object).ok_or_else(g209)?;
    if first["input_digest"] != profile_digest || first["output_digest"] != task_digest {
        return Err(g209());
    }
    let mut current_turn = 0;
    let mut route: Option<(&str, &str, &str, u64)> = None;
    let mut provider_started = false;
    let mut accepted_tool: Option<(&str, &str)> = None;
    for pair in events.windows(2) {
        let left = pair[0].as_object().ok_or_else(g209)?;
        let right = pair[1].as_object().ok_or_else(g209)?;
        let kind = left["kind"].as_str().ok_or_else(g209)?;
        let next = right["kind"].as_str().ok_or_else(g209)?;
        let turn = left["turn"].as_u64().ok_or_else(g209)?;
        if kind != "run_started" && kind != "run_finished" {
            if turn < current_turn || turn > current_turn.saturating_add(1) {
                return Err(g209());
            }
            current_turn = turn;
        }
        match kind {
            "run_started" if next != "route_selected" && next != "run_finished" => {
                return Err(g209())
            }
            "route_selected" => {
                route = Some((
                    left["provider_id"].as_str().ok_or_else(g209)?,
                    left["model_id"].as_str().ok_or_else(g209)?,
                    left["input_digest"].as_str().ok_or_else(g209)?,
                    turn,
                ));
                if next != "provider_attempt_started"
                    && !(next == "run_finished" && final_status == "budget_exhausted")
                {
                    return Err(g209());
                }
            }
            "provider_attempt_started" => {
                let Some((provider, model, request, route_turn)) = route else {
                    return Err(g209());
                };
                if turn != route_turn
                    || left["provider_id"] != provider
                    || left["model_id"] != model
                    || left["input_digest"] != request
                    || (next != "provider_attempt_finished"
                        && !(next == "run_finished" && final_status == "budget_exhausted"))
                {
                    return Err(g209());
                }
                provider_started = next == "provider_attempt_finished";
            }
            "provider_attempt_finished" => {
                if !provider_started {
                    return Err(g209());
                }
                provider_started = false;
                let status = left["status"].as_str().ok_or_else(g209)?;
                if status == "definitely_not_started" {
                    if next != "provider_attempt_started" && next != "run_finished" {
                        return Err(g209());
                    }
                } else if status == "succeeded" {
                    if (next != "action_accepted" && next != "run_finished")
                        || left["output_digest"].is_null()
                    {
                        return Err(g209());
                    }
                } else if next != "run_finished" {
                    return Err(g209());
                }
            }
            "action_accepted" => match left["status"].as_str().ok_or_else(g209)? {
                "final" if next != "run_finished" => return Err(g209()),
                "tool" => {
                    if next != "tool_authorized" && next != "run_finished" {
                        return Err(g209());
                    }
                    if next == "tool_authorized" {
                        accepted_tool = Some((
                            left["tool_id"].as_str().ok_or_else(g209)?,
                            left["input_digest"].as_str().ok_or_else(g209)?,
                        ));
                    }
                }
                _ => {}
            },
            "tool_authorized" => {
                let Some((tool, action)) = accepted_tool else {
                    return Err(g209());
                };
                if left["tool_id"] != tool
                    || left["input_digest"] != action
                    || (next != "tool_finished" && next != "run_finished")
                {
                    return Err(g209());
                }
            }
            "tool_finished" => {
                accepted_tool = None;
                if left["status"] == "succeeded" {
                    if next != "route_selected"
                        && !(next == "run_finished" && final_status == "budget_exhausted")
                    {
                        return Err(g209());
                    }
                } else if next != "run_finished" {
                    return Err(g209());
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn valid_event_shape(event: &Map<String, Value>, kind: &str, status: &str) -> bool {
    let provider = event["provider_id"].as_str();
    let model = event["model_id"].as_str();
    let tool = event["tool_id"].as_str();
    let input = event["input_digest"].as_str();
    let output = event["output_digest"].as_str();
    let paired_model = provider.is_some() && model.is_some();
    let digest_ok = |value: Option<&str>| value.is_none_or(canonical_sha256);
    if !digest_ok(input) || !digest_ok(output) {
        return false;
    }
    match kind {
        "run_started" => {
            provider.is_none()
                && model.is_none()
                && tool.is_none()
                && input.is_some()
                && output.is_some()
        }
        "route_selected" | "provider_attempt_started" => {
            paired_model && tool.is_none() && input.is_some() && output.is_none()
        }
        "provider_attempt_finished" => {
            paired_model
                && tool.is_none()
                && input.is_none()
                && (status != "definitely_not_started" || output.is_none())
        }
        "action_accepted" => {
            paired_model
                && input.is_some()
                && match status {
                    "final" => tool.is_none() && output.is_some(),
                    "tool" => tool.is_some() && output.is_none(),
                    _ => false,
                }
        }
        "tool_authorized" => paired_model && tool.is_some() && input.is_some() && output.is_none(),
        "tool_finished" => {
            paired_model
                && tool.is_some()
                && input.is_none()
                && ((status == "succeeded") == output.is_some())
        }
        "run_finished" => {
            provider.is_none()
                && model.is_none()
                && tool.is_none()
                && input.is_none()
                && output.is_none()
        }
        _ => false,
    }
}

pub(super) fn canonical_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn parse_usage_delta(value: &Value) -> Result<UsageDelta, Diagnostic> {
    let row = value.as_object().ok_or_else(g209)?;
    if !exact_keys(
        row,
        &[
            "provider_input_bytes",
            "provider_output_bytes",
            "reported_model_input_tokens",
            "reported_model_output_tokens",
            "usd_microunits",
            "tool_argument_bytes",
            "tool_result_bytes",
            "elapsed_ms",
        ],
    ) {
        return Err(g209());
    }
    Ok(UsageDelta {
        provider_input_bytes: row["provider_input_bytes"].as_u64().ok_or_else(g209)?,
        provider_output_bytes: row["provider_output_bytes"].as_u64().ok_or_else(g209)?,
        reported_model_input_tokens: row["reported_model_input_tokens"]
            .as_u64()
            .ok_or_else(g209)?,
        reported_model_output_tokens: row["reported_model_output_tokens"]
            .as_u64()
            .ok_or_else(g209)?,
        usd_microunits: row["usd_microunits"].as_u64().ok_or_else(g209)?,
        tool_argument_bytes: row["tool_argument_bytes"].as_u64().ok_or_else(g209)?,
        tool_result_bytes: row["tool_result_bytes"].as_u64().ok_or_else(g209)?,
        elapsed_ms: row["elapsed_ms"].as_u64().ok_or_else(g209)?,
    })
}

fn add_usage_delta(total: &mut UsageDelta, value: UsageDelta) -> Result<(), Diagnostic> {
    macro_rules! add {
        ($field:ident) => {
            total.$field = total.$field.checked_add(value.$field).ok_or_else(g209)?;
        };
    }
    add!(provider_input_bytes);
    add!(provider_output_bytes);
    add!(reported_model_input_tokens);
    add!(reported_model_output_tokens);
    add!(usd_microunits);
    add!(tool_argument_bytes);
    add!(tool_result_bytes);
    add!(elapsed_ms);
    Ok(())
}

fn parse_usage(value: &Value) -> Result<Usage, Diagnostic> {
    let row = value.as_object().ok_or_else(g209)?;
    if !exact_keys(
        row,
        &[
            "turns",
            "provider_attempts",
            "provider_input_bytes",
            "provider_output_bytes",
            "reported_model_input_tokens",
            "reported_model_output_tokens",
            "usd_microunits",
            "tool_calls",
            "tool_argument_bytes",
            "tool_result_bytes",
            "retained_state_bytes",
            "elapsed_ms",
            "max_concurrency",
        ],
    ) {
        return Err(g209());
    }
    Ok(Usage {
        turns: row["turns"].as_u64().ok_or_else(g209)?,
        provider_attempts: row["provider_attempts"].as_u64().ok_or_else(g209)?,
        provider_input_bytes: row["provider_input_bytes"].as_u64().ok_or_else(g209)?,
        provider_output_bytes: row["provider_output_bytes"].as_u64().ok_or_else(g209)?,
        reported_model_input_tokens: row["reported_model_input_tokens"]
            .as_u64()
            .ok_or_else(g209)?,
        reported_model_output_tokens: row["reported_model_output_tokens"]
            .as_u64()
            .ok_or_else(g209)?,
        usd_microunits: row["usd_microunits"].as_u64().ok_or_else(g209)?,
        tool_calls: row["tool_calls"].as_u64().ok_or_else(g209)?,
        tool_argument_bytes: row["tool_argument_bytes"].as_u64().ok_or_else(g209)?,
        tool_result_bytes: row["tool_result_bytes"].as_u64().ok_or_else(g209)?,
        retained_state_bytes: row["retained_state_bytes"].as_u64().ok_or_else(g209)?,
        elapsed_ms: row["elapsed_ms"].as_u64().ok_or_else(g209)?,
        max_concurrency: row["max_concurrency"].as_u64().ok_or_else(g209)?,
    })
}

fn validate_termination(value: &Map<String, Value>) -> Result<(), Diagnostic> {
    let status = value["status"].as_str().ok_or_else(g209)?;
    if status == "completed" {
        return if value["code"].is_null() && value["message"].is_null() {
            Ok(())
        } else {
            Err(g209())
        };
    }
    let code = value["code"].as_str().ok_or_else(g209)?;
    let message = value["message"].as_str().ok_or_else(g209)?;
    let valid = match status {
        "cancelled" => code == "SPX-I220" && message == "Agent Runtime run was cancelled",
        "deadline_exceeded" => {
            code == "SPX-I221" && message == "Agent Runtime deadline was exceeded"
        }
        "provider_failed" => {
            code == "SPX-I218" && message.starts_with("Agent Runtime provider adapter failed: ")
        }
        "tool_failed" => {
            code == "SPX-I219" && message.starts_with("Agent Runtime tool adapter failed: ")
        }
        "budget_exhausted" => code == "SPX-G208" && canonical_g208_message(message),
        "policy_rejected" => {
            code == "SPX-G207"
                && message.starts_with("Agent Runtime action or tool authorization was rejected: ")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(g209())
    }
}

fn canonical_g208_message(message: &str) -> bool {
    let Some((field, maximum)) = message.split_once(" exceeds ") else {
        return false;
    };
    let cap = match field {
        "profile_bytes" => MAX_PROFILE_BYTES as u64,
        "task_bytes" => MAX_TASK_BYTES as u64,
        "models" => MAX_MODELS as u64,
        "tools" => MAX_TOOLS as u64,
        "capabilities" => MAX_CAPABILITIES as u64,
        "turns" => MAX_TURNS,
        "provider_attempts" => MAX_PROVIDER_ATTEMPTS,
        "retries_per_turn" => MAX_RETRIES_PER_TURN,
        "concurrency" => MAX_CONCURRENCY,
        "elapsed_ms" => MAX_ELAPSED_MS,
        "provider_request_bytes" => MAX_PROVIDER_REQUEST_BYTES,
        "provider_response_bytes" => MAX_PROVIDER_RESPONSE_BYTES,
        "stream_chunks" => MAX_STREAM_CHUNKS,
        "total_provider_input_bytes" => MAX_TOTAL_PROVIDER_INPUT_BYTES,
        "total_provider_output_bytes" => MAX_TOTAL_PROVIDER_OUTPUT_BYTES,
        "reported_model_input_tokens" => MAX_REPORTED_MODEL_INPUT_TOKENS,
        "reported_model_output_tokens" => MAX_REPORTED_MODEL_OUTPUT_TOKENS,
        "usd_microunits" => MAX_USD_MICROUNITS,
        "tool_calls" => MAX_TOOL_CALLS,
        "tool_arguments_bytes" => MAX_TOOL_ARGUMENT_BYTES,
        "tool_result_bytes" => MAX_TOOL_RESULT_BYTES,
        "total_tool_bytes" => MAX_TOTAL_TOOL_BYTES,
        "retained_state_bytes" => MAX_RETAINED_STATE_BYTES,
        "trace_events" => MAX_TRACE_EVENTS,
        "trace_bytes" => MAX_TRACE_BYTES,
        "evidence_bytes" => MAX_EVIDENCE_BYTES,
        "builder_bytes" => MAX_BUILDER_BYTES as u64,
        "json_depth" => MAX_JSON_DEPTH as u64,
        "identifier_bytes" => MAX_IDENTIFIER_BYTES as u64,
        "description_bytes" => MAX_DESCRIPTION_BYTES as u64,
        _ => return false,
    };
    !maximum.is_empty()
        && (maximum == "0" || !maximum.starts_with('0'))
        && maximum.parse::<u64>().is_ok_and(|maximum| maximum <= cap)
}

pub(super) fn replay_evidence_inner(
    source: &str,
    profile: &Profile,
    state: &RunState,
    expected_trace: &str,
    expected_budget: &EvidenceBudget,
) -> Result<(), Diagnostic> {
    let value = canonical_document(
        source,
        "evidence",
        EVIDENCE_SCHEMA,
        MAX_EVIDENCE_BYTES as usize,
    )?;
    let top = object(&value, "evidence", EVIDENCE_SCHEMA)?;
    if !exact_keys(
        top,
        &[
            "schema",
            "run_id",
            "profile",
            "task",
            "trace",
            "result",
            "limits",
            "budget",
            "nonclaims",
        ],
    ) {
        return Err(g204("evidence", EVIDENCE_SCHEMA));
    }
    if string_member(top, "run_id", "evidence", EVIDENCE_SCHEMA)?
        != run_id(&profile.digest, &state.task_digest, &state.task_nonce)?
    {
        return Err(g209());
    }
    let profile_ref = object(&top["profile"], "evidence", EVIDENCE_SCHEMA)?;
    let task_ref = object(&top["task"], "evidence", EVIDENCE_SCHEMA)?;
    let trace_ref = object(&top["trace"], "evidence", EVIDENCE_SCHEMA)?;
    if !exact_keys(profile_ref, &["schema", "digest", "bytes"])
        || profile_ref["schema"] != PROFILE_SCHEMA
        || profile_ref["digest"] != profile.digest
        || profile_ref["bytes"].as_u64() != Some(profile.source.len() as u64)
        || !exact_keys(task_ref, &["schema", "digest", "bytes"])
        || task_ref["schema"] != TASK_SCHEMA
        || task_ref["digest"] != state.task_digest
        || task_ref["bytes"].as_u64() != Some(state.task_bytes)
        || !exact_keys(trace_ref, &["schema", "digest", "bytes", "document"])
    {
        return Err(g209());
    }
    let document = trace_ref["document"].as_str().ok_or_else(g209)?;
    if document != expected_trace
        || trace_ref["bytes"].as_u64() != Some(document.len() as u64)
        || trace_ref["digest"] != digest(TRACE_DOMAIN, document.as_bytes())
    {
        return Err(g209());
    }
    replay_trace_expected(document, profile, state)?;
    let result = object(&top["result"], "evidence", EVIDENCE_SCHEMA)?;
    if !exact_keys(
        result,
        &[
            "status",
            "final_message_digest",
            "final_message_bytes",
            "last_turn",
        ],
    ) || result["status"] != state.termination.status.text()
        || result["last_turn"].as_u64() != Some(state.last_turn)
    {
        return Err(g209());
    }
    match &state.final_message {
        Some(message)
            if result["final_message_digest"]
                == digest(FINAL_MESSAGE_DOMAIN, message.as_bytes())
                && result["final_message_bytes"].as_u64() == Some(message.len() as u64) => {}
        None if result["final_message_digest"].is_null()
            && result["final_message_bytes"].as_u64() == Some(0) => {}
        _ => return Err(g209()),
    }
    let expected_limits: Value =
        serde_json::from_str(&render_production_limits()).map_err(|_| g209())?;
    let expected_budget_value: Value =
        serde_json::from_str(&render_budget(expected_budget)).map_err(|_| g209())?;
    if top["limits"] != expected_limits || top["budget"] != expected_budget_value {
        return Err(g209());
    }
    if string_array_member(top, "nonclaims", "evidence", EVIDENCE_SCHEMA)? != NONCLAIMS {
        return Err(g209());
    }
    Ok(())
}

fn replay_trace_expected(
    source: &str,
    profile: &Profile,
    state: &RunState,
) -> Result<(), Diagnostic> {
    replay_trace(source)?;
    let value: Value = serde_json::from_str(source.trim_end()).map_err(|_| g209())?;
    let top = value.as_object().ok_or_else(g209)?;
    if top["run_id"] != state.run_id
        || top["profile_digest"] != profile.digest
        || top["task_digest"] != state.task_digest
    {
        return Err(g209());
    }
    let events = top["events"].as_array().ok_or_else(g209)?;
    if events.len() != state.events.len() {
        return Err(g209());
    }
    for (actual, expected) in events.iter().zip(&state.events) {
        let rendered: Value = serde_json::from_str(&render_event(expected)).map_err(|_| g209())?;
        if actual != &rendered {
            return Err(g209());
        }
    }
    let expected_usage: Value =
        serde_json::from_str(&render_usage(&state.usage)).map_err(|_| g209())?;
    if top["usage"] != expected_usage {
        return Err(g209());
    }
    let termination = top["termination"].as_object().ok_or_else(g209)?;
    if termination["status"] != state.termination.status.text()
        || termination["code"]
            != state
                .termination
                .code
                .map_or(Value::Null, |value| Value::String(value.to_owned()))
        || termination["message"]
            != state
                .termination
                .message
                .as_ref()
                .map_or(Value::Null, |value| Value::String(value.clone()))
    {
        return Err(g209());
    }
    Ok(())
}

#[cfg(test)]
pub(in crate::agent_runtime) fn replay_evidence(
    source: &str,
    profile_source: &str,
    expected: &AgentRuntimeEvidence,
) -> Result<(), Diagnostic> {
    let profile = parse_profile(profile_source)?;
    replay_evidence_inner(
        source,
        &profile,
        &expected.replay.state,
        &expected.trace,
        &expected.replay.budget,
    )
}
