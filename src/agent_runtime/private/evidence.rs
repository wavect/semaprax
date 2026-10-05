//! Evidence rendering: external-boundary capacity preflight, minimum
//! terminal trace/evidence sizing, and canonical trace, evidence and bundle
//! rendering.

use std::fmt;
use std::fmt::Write as _;

use super::accounting::{receipt_digest, reconcile_accounting_receipt, render_accounting_receipt};
use super::admission::parse_profile;
use super::execution::push_final_event;
use super::replay::{replay_evidence_inner, replay_trace};
use super::request::route;
use super::*;

#[derive(Clone, Copy)]
pub(super) enum ExternalBoundary<'a> {
    Provider(&'a Model, &'a Route),
    Tool(&'a Model, &'a str),
}

#[derive(Default)]
struct CountSink {
    bytes: u64,
    escaped_bytes: u64,
}

impl fmt::Write for CountSink {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.bytes = self
            .bytes
            .checked_add(value.len() as u64)
            .ok_or(fmt::Error)?;
        for character in value.chars() {
            self.escaped_bytes = self
                .escaped_bytes
                .checked_add(match character {
                    '"' | '\\' | '\u{08}' | '\u{0c}' | '\n' | '\r' | '\t' => 2,
                    '\u{00}'..='\u{1f}' => 6,
                    _ => character.len_utf8() as u64,
                })
                .ok_or(fmt::Error)?;
        }
        Ok(())
    }
}

pub(super) fn preflight_external_capacity(
    profile: &Profile,
    state: &RunState,
    boundary: ExternalBoundary<'_>,
) -> Result<(), Diagnostic> {
    let mandatory_events = match boundary {
        ExternalBoundary::Provider(_, _) => 4,
        ExternalBoundary::Tool(_, _) => 2,
    };
    if state.events.len() as u64 + mandatory_events > profile.limits.max_trace_events {
        return Err(g208("trace_events", profile.limits.max_trace_events));
    }
    let (minimum_trace, escaped_trace) = minimum_terminal_trace_bytes(profile, state, boundary)?;
    if minimum_trace > profile.limits.max_trace_bytes {
        return Err(g208("trace_bytes", profile.limits.max_trace_bytes));
    }
    let minimum_evidence =
        minimum_terminal_evidence_bytes(profile, state, boundary, minimum_trace, escaped_trace)?;
    if minimum_evidence > profile.limits.max_evidence_bytes {
        return Err(g208("evidence_bytes", profile.limits.max_evidence_bytes));
    }
    Ok(())
}

fn minimum_terminal_trace_bytes(
    profile: &Profile,
    state: &RunState,
    boundary: ExternalBoundary<'_>,
) -> Result<(u64, u64), Diagnostic> {
    const HASH: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let mut sink = CountSink::default();
    write!(sink, "{{\"schema\":\"{TRACE_SCHEMA}\",\"run_id\":").map_err(|_| g209())?;
    write_json_string(&mut sink, &state.run_id).map_err(|_| g209())?;
    sink.write_str(",\"profile_digest\":").map_err(|_| g209())?;
    write_json_string(&mut sink, &profile.digest).map_err(|_| g209())?;
    sink.write_str(",\"task_digest\":").map_err(|_| g209())?;
    write_json_string(&mut sink, &state.task_digest).map_err(|_| g209())?;
    sink.write_str(",\"events\":[").map_err(|_| g209())?;
    let mut index = 0u64;
    for event in &state.events {
        if index > 0 {
            sink.write_char(',').map_err(|_| g209())?;
        }
        write_event(&mut sink, event).map_err(|_| g209())?;
        index += 1;
    }
    let (model, tool, finish_kind, finish_status, result_status, code, message) = match boundary {
        ExternalBoundary::Provider(model, route) => {
            if index > 0 {
                sink.write_char(',').map_err(|_| g209())?;
            }
            write_event_parts(
                &mut sink,
                index,
                state.last_turn,
                "provider_attempt_started",
                Some(&model.provider_id),
                Some(&model.model_id),
                None,
                Some(&route.request_digest),
                None,
                "started",
                UsageDelta {
                    provider_input_bytes: route.request.len() as u64,
                    reported_model_input_tokens: route.input_tokens,
                    usd_microunits: route.reserved_cost,
                    ..UsageDelta::default()
                },
            )
            .map_err(|_| g209())?;
            index += 1;
            (
                model,
                None,
                "provider_attempt_finished",
                "failed_uncertain",
                "provider_failed",
                "SPX-I218",
                "Agent Runtime provider adapter failed: start uncertain",
            )
        }
        ExternalBoundary::Tool(model, tool_id) => (
            model,
            Some(tool_id),
            "tool_finished",
            "failed",
            "tool_failed",
            "SPX-I219",
            "Agent Runtime tool adapter failed: invocation failed",
        ),
    };
    sink.write_char(',').map_err(|_| g209())?;
    write_event_parts(
        &mut sink,
        index,
        state.last_turn,
        finish_kind,
        Some(&model.provider_id),
        Some(&model.model_id),
        tool,
        None,
        Some(HASH),
        finish_status,
        UsageDelta {
            provider_output_bytes: profile.limits.max_provider_response_bytes,
            reported_model_output_tokens: profile.limits.max_reported_model_output_tokens,
            tool_result_bytes: profile.limits.max_tool_result_bytes,
            elapsed_ms: profile.limits.max_elapsed_ms,
            ..UsageDelta::default()
        },
    )
    .map_err(|_| g209())?;
    index += 1;
    if matches!(boundary, ExternalBoundary::Provider(_, _)) {
        sink.write_char(',').map_err(|_| g209())?;
        write_event_parts(
            &mut sink,
            index,
            state.last_turn,
            "action_accepted",
            Some(&model.provider_id),
            Some(&model.model_id),
            None,
            Some(HASH),
            Some(HASH),
            "final",
            UsageDelta::default(),
        )
        .map_err(|_| g209())?;
        index += 1;
    }
    sink.write_char(',').map_err(|_| g209())?;
    write_event_parts(
        &mut sink,
        index,
        state.last_turn,
        "run_finished",
        None,
        None,
        None,
        None,
        None,
        result_status,
        UsageDelta {
            elapsed_ms: profile.limits.max_elapsed_ms,
            ..UsageDelta::default()
        },
    )
    .map_err(|_| g209())?;
    let mut usage = state.usage.clone();
    if let ExternalBoundary::Provider(_, route) = boundary {
        usage.provider_attempts = usage.provider_attempts.saturating_add(1);
        usage.provider_input_bytes = usage
            .provider_input_bytes
            .saturating_add(route.request.len() as u64);
        usage.reported_model_input_tokens = usage
            .reported_model_input_tokens
            .saturating_add(route.input_tokens);
        usage.usd_microunits = usage.usd_microunits.saturating_add(route.reserved_cost);
    }
    usage.provider_output_bytes = profile.limits.max_total_provider_output_bytes;
    usage.reported_model_output_tokens = profile.limits.max_reported_model_output_tokens;
    usage.tool_result_bytes = profile.limits.max_total_tool_bytes;
    usage.elapsed_ms = profile.limits.max_elapsed_ms;
    sink.write_str("],\"usage\":").map_err(|_| g209())?;
    write_usage(&mut sink, &usage).map_err(|_| g209())?;
    sink.write_str(",\"termination\":{\"status\":")
        .map_err(|_| g209())?;
    write_json_string(&mut sink, result_status).map_err(|_| g209())?;
    sink.write_str(",\"code\":").map_err(|_| g209())?;
    write_json_string(&mut sink, code).map_err(|_| g209())?;
    sink.write_str(",\"message\":").map_err(|_| g209())?;
    write_json_string(&mut sink, message).map_err(|_| g209())?;
    sink.write_str("},\"nonclaims\":[").map_err(|_| g209())?;
    for (position, value) in NONCLAIMS.iter().enumerate() {
        if position > 0 {
            sink.write_char(',').map_err(|_| g209())?;
        }
        write_json_string(&mut sink, value).map_err(|_| g209())?;
    }
    sink.write_str("]}\n").map_err(|_| g209())?;
    Ok((sink.bytes, sink.escaped_bytes))
}

fn minimum_terminal_evidence_bytes(
    profile: &Profile,
    state: &RunState,
    boundary: ExternalBoundary<'_>,
    trace_bytes: u64,
    escaped_trace_bytes: u64,
) -> Result<u64, Diagnostic> {
    let mut sink = CountSink::default();
    let (provider_attempts, provider_input_bytes, reported_model_input_tokens, usd_microunits) =
        match boundary {
            ExternalBoundary::Provider(_, route) => (
                state.usage.provider_attempts.checked_add(1),
                state
                    .usage
                    .provider_input_bytes
                    .checked_add(route.request.len() as u64),
                state
                    .usage
                    .reported_model_input_tokens
                    .checked_add(route.input_tokens),
                state.usage.usd_microunits.checked_add(route.reserved_cost),
            ),
            ExternalBoundary::Tool(_, _) => (
                Some(state.usage.provider_attempts),
                Some(state.usage.provider_input_bytes),
                Some(state.usage.reported_model_input_tokens),
                Some(state.usage.usd_microunits),
            ),
        };
    let budget = EvidenceBudget {
        used_models: profile.models.len() as u64,
        used_tools: profile.tools.len() as u64,
        used_capabilities: distinct_capability_count(profile) as u64,
        used_turns: state.usage.turns,
        used_provider_attempts: provider_attempts.ok_or_else(g209)?,
        used_provider_input_bytes: provider_input_bytes.ok_or_else(g209)?,
        used_provider_output_bytes: profile.limits.max_total_provider_output_bytes,
        used_reported_model_input_tokens: reported_model_input_tokens.ok_or_else(g209)?,
        used_reported_model_output_tokens: profile.limits.max_reported_model_output_tokens,
        used_usd_microunits: usd_microunits.ok_or_else(g209)?,
        used_tool_calls: state.usage.tool_calls,
        used_tool_argument_bytes: state.usage.tool_argument_bytes,
        used_tool_result_bytes: profile.limits.max_total_tool_bytes,
        used_retained_state_bytes: state.usage.retained_state_bytes,
        used_trace_events: profile.limits.max_trace_events,
        used_trace_bytes: trace_bytes,
        used_evidence_bytes: u64::MAX,
        used_builder_bytes: profile.limits.max_builder_bytes,
        used_elapsed_ms: profile.limits.max_elapsed_ms,
        used_concurrency: 1,
    };
    write_evidence(
        &mut sink,
        profile,
        state,
        "",
        "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
        &budget,
    )
    .map_err(|_| g209())?;
    let empty_quoted_trace = 2u64;
    let base = sink
        .bytes
        .checked_sub(20)
        .and_then(|value| value.checked_sub(empty_quoted_trace))
        .and_then(|value| value.checked_add(escaped_trace_bytes))
        .and_then(|value| value.checked_add(2))
        .and_then(|value| value.checked_add(digits_u64(trace_bytes).saturating_sub(1)))
        .and_then(|value| {
            if matches!(boundary, ExternalBoundary::Provider(_, _)) {
                value.checked_add(73u64.saturating_sub(4))?.checked_add(
                    digits_u64(profile.limits.max_provider_response_bytes).saturating_sub(1),
                )
            } else {
                Some(value)
            }
        })
        .ok_or_else(g209)?;
    evidence_fixed_point(base)
}

fn digits_u64(value: u64) -> u64 {
    value.checked_ilog10().unwrap_or(0) as u64 + 1
}

pub(super) fn preflight_current_terminal(
    profile: &Profile,
    state: &mut RunState,
) -> Result<(), Diagnostic> {
    let result = (|| {
        let mut maximum_trace = 0;
        let mut maximum_evidence = 0;
        for (status, code, message) in terminal_diagnostics() {
            push_final_event(state, profile.limits, state.last_turn, status)?;
            let mut trace = CountSink::default();
            write_trace_termination(
                &mut trace,
                profile,
                state,
                status,
                Some(code),
                Some(message),
            )
            .map_err(|_| g209())?;
            let evidence =
                count_evidence_bytes(profile, state, trace.bytes, trace.escaped_bytes, status)?;
            state.events.pop();
            maximum_trace = maximum_trace.max(trace.bytes);
            maximum_evidence = maximum_evidence.max(evidence);
        }
        if maximum_trace > profile.limits.max_trace_bytes {
            return Err(g208("trace_bytes", profile.limits.max_trace_bytes));
        }
        if maximum_evidence > profile.limits.max_evidence_bytes {
            return Err(g208("evidence_bytes", profile.limits.max_evidence_bytes));
        }
        Ok(())
    })();
    if state
        .events
        .last()
        .is_some_and(|event| event.kind == "run_finished")
    {
        state.events.pop();
    }
    result
}

fn terminal_diagnostics() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    [
        (
            "policy_rejected",
            "SPX-G204",
            "Agent Runtime provider request is not canonical semaprax.agent-runtime-provider-request.v1 JSON",
        ),
        (
            "policy_rejected",
            "SPX-G206",
            "Agent Runtime has no eligible model under the frozen routing policy",
        ),
        (
            "policy_rejected",
            "SPX-G207",
            "Agent Runtime action or tool authorization was rejected: required capability missing",
        ),
        (
            "budget_exhausted",
            "SPX-G208",
            "reported_model_output_tokens exceeds 262144",
        ),
        (
            "policy_rejected",
            "SPX-G209",
            "Agent Runtime trace or Evidence disagrees with the replayed state machine",
        ),
        (
            "provider_failed",
            "SPX-I218",
            "Agent Runtime provider adapter failed: definitely not started",
        ),
        (
            "tool_failed",
            "SPX-I219",
            "Agent Runtime tool adapter failed: invocation failed",
        ),
        ("cancelled", "SPX-I220", "Agent Runtime run was cancelled"),
        (
            "deadline_exceeded",
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        ),
    ]
    .into_iter()
}

#[cfg(test)]
pub(in crate::agent_runtime) fn terminal_diagnostics_for_test(
) -> Vec<(&'static str, &'static str, &'static str)> {
    terminal_diagnostics().collect()
}

#[cfg(test)]
pub(in crate::agent_runtime) fn preflight_terminal_for_test(
    profile_source: &str,
    evidence: &AgentRuntimeEvidence,
    max_trace_bytes: u64,
    max_evidence_bytes: u64,
) -> Result<(), Diagnostic> {
    let mut profile = parse_profile(profile_source)?;
    profile.limits.max_trace_bytes = max_trace_bytes;
    profile.limits.max_evidence_bytes = max_evidence_bytes;
    let mut state = evidence.replay.state.clone();
    if state
        .events
        .last()
        .is_some_and(|event| event.kind == "run_finished")
    {
        state.events.pop();
    }
    preflight_current_terminal(&profile, &mut state)
}

fn count_evidence_bytes(
    profile: &Profile,
    state: &RunState,
    trace_bytes: u64,
    escaped_trace_bytes: u64,
    result_status: &str,
) -> Result<u64, Diagnostic> {
    const HASH: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
    let budget = EvidenceBudget {
        used_models: profile.models.len() as u64,
        used_tools: profile.tools.len() as u64,
        used_capabilities: distinct_capability_count(profile) as u64,
        used_turns: state.usage.turns,
        used_provider_attempts: state.usage.provider_attempts,
        used_provider_input_bytes: state.usage.provider_input_bytes,
        used_provider_output_bytes: state.usage.provider_output_bytes,
        used_reported_model_input_tokens: state.usage.reported_model_input_tokens,
        used_reported_model_output_tokens: state.usage.reported_model_output_tokens,
        used_usd_microunits: state.usage.usd_microunits,
        used_tool_calls: state.usage.tool_calls,
        used_tool_argument_bytes: state.usage.tool_argument_bytes,
        used_tool_result_bytes: state.usage.tool_result_bytes,
        used_retained_state_bytes: state.usage.retained_state_bytes,
        used_trace_events: state.events.len() as u64,
        used_trace_bytes: trace_bytes,
        used_evidence_bytes: u64::MAX,
        used_builder_bytes: profile.limits.max_builder_bytes,
        used_elapsed_ms: profile.limits.max_elapsed_ms,
        used_concurrency: 1,
    };
    let mut sink = CountSink::default();
    write_evidence_status(&mut sink, profile, state, "", HASH, &budget, result_status)
        .map_err(|_| g209())?;
    let base = sink
        .bytes
        .checked_sub(20)
        .and_then(|value| value.checked_sub(2))
        .and_then(|value| value.checked_add(escaped_trace_bytes + 2))
        .and_then(|value| value.checked_add(digits_u64(trace_bytes).saturating_sub(1)))
        .ok_or_else(g209)?;
    evidence_fixed_point(base)
}

pub(super) fn render_bundle(
    profile: &Profile,
    state: RunState,
    parse_used: u64,
    child_limit: u64,
) -> Result<AgentRuntimeEvidence, Diagnostic> {
    let child_remaining = crate::bounded_output::active_remaining().unwrap_or(0) as u64;
    let builder_bytes = parse_used.saturating_add(child_limit.saturating_sub(child_remaining));
    let trace = render_trace(profile, &state)?;
    if trace.len() as u64 > profile.limits.max_trace_bytes {
        return Err(g208("trace_bytes", profile.limits.max_trace_bytes));
    }
    replay_trace(&trace)?;
    let trace_digest = digest(TRACE_DOMAIN, trace.as_bytes());
    let mut budget = EvidenceBudget {
        used_models: profile.models.len() as u64,
        used_tools: profile.tools.len() as u64,
        used_capabilities: distinct_capability_count(profile) as u64,
        used_turns: state.usage.turns,
        used_provider_attempts: state.usage.provider_attempts,
        used_provider_input_bytes: state.usage.provider_input_bytes,
        used_provider_output_bytes: state.usage.provider_output_bytes,
        used_reported_model_input_tokens: state.usage.reported_model_input_tokens,
        used_reported_model_output_tokens: state.usage.reported_model_output_tokens,
        used_usd_microunits: state.usage.usd_microunits,
        used_tool_calls: state.usage.tool_calls,
        used_tool_argument_bytes: state.usage.tool_argument_bytes,
        used_tool_result_bytes: state.usage.tool_result_bytes,
        used_retained_state_bytes: state.usage.retained_state_bytes,
        used_trace_events: state.events.len() as u64,
        used_trace_bytes: trace.len() as u64,
        used_builder_bytes: builder_bytes,
        used_elapsed_ms: state.usage.elapsed_ms,
        used_concurrency: 1,
        ..EvidenceBudget::default()
    };
    budget.used_evidence_bytes = u64::MAX;
    let mut evidence = render_evidence(profile, &state, &trace, &trace_digest, &budget);
    let marker = "\"used_evidence_bytes\":18446744073709551615";
    let marker_start = evidence.find(marker).ok_or_else(g209)? + marker.len() - 20;
    let base_length = evidence.len().checked_sub(20).ok_or_else(g209)? as u64;
    budget.used_evidence_bytes = evidence_fixed_point(base_length)?;
    evidence.replace_range(
        marker_start..marker_start + 20,
        &budget.used_evidence_bytes.to_string(),
    );
    if evidence.len() as u64 != budget.used_evidence_bytes {
        return Err(g209());
    }
    if evidence.len() as u64 > profile.limits.max_evidence_bytes {
        return Err(g208("evidence_bytes", profile.limits.max_evidence_bytes));
    }
    replay_evidence_inner(&evidence, profile, &state, &trace, &budget)?;
    let evidence_digest = digest(EVIDENCE_DOMAIN, evidence.as_bytes());
    let accounting_receipt = render_accounting_receipt(&state, &trace_digest, &evidence_digest)?;
    reconcile_accounting_receipt(&accounting_receipt, &state, &trace_digest, &evidence_digest)?;
    let accounting_receipt_digest = receipt_digest(&accounting_receipt);
    Ok(AgentRuntimeEvidence {
        trace,
        trace_digest,
        evidence,
        evidence_digest,
        accounting_receipt,
        accounting_receipt_digest,
        status: state.termination.status,
        replay: EvidenceReplay { state, budget },
    })
}

fn evidence_fixed_point(base_length: u64) -> Result<u64, Diagnostic> {
    let mut value = base_length.checked_add(1).ok_or_else(g209)?;
    for _ in 0..24 {
        let digits = value.checked_ilog10().unwrap_or(0) as u64 + 1;
        let next = base_length.checked_add(digits).ok_or_else(g209)?;
        if next == value {
            return Ok(value);
        }
        value = next;
    }
    Err(g209())
}

fn distinct_capability_count(profile: &Profile) -> usize {
    let mut values = BTreeSet::new();
    for model in &profile.models {
        values.extend(model.capabilities.iter());
    }
    for tool in &profile.tools {
        values.extend(tool.required_capabilities.iter());
    }
    values.extend(profile.policy.required_model_capabilities.iter());
    values.extend(profile.policy.granted_capabilities.iter());
    values.len()
}

fn render_trace(profile: &Profile, state: &RunState) -> Result<String, Diagnostic> {
    let mut output = String::new();
    write_trace(&mut output, profile, state).map_err(|_| g209())?;
    Ok(output)
}

fn write_trace<W: fmt::Write>(output: &mut W, profile: &Profile, state: &RunState) -> fmt::Result {
    write_trace_termination(
        output,
        profile,
        state,
        state.termination.status.text(),
        state.termination.code,
        state.termination.message.as_deref(),
    )
}

fn write_trace_termination<W: fmt::Write>(
    output: &mut W,
    profile: &Profile,
    state: &RunState,
    termination_status: &str,
    termination_code: Option<&str>,
    termination_message: Option<&str>,
) -> fmt::Result {
    let task_digest = state
        .events
        .first()
        .and_then(|event| event.output_digest.as_deref())
        .ok_or(fmt::Error)?;
    write!(output, "{{\"schema\":\"{TRACE_SCHEMA}\",\"run_id\":")?;
    write_json_string(output, &state.run_id)?;
    output.write_str(",\"profile_digest\":")?;
    write_json_string(output, &profile.digest)?;
    output.write_str(",\"task_digest\":")?;
    write_json_string(output, task_digest)?;
    output.write_str(",\"events\":[")?;
    for (index, event) in state.events.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_event(output, event)?;
    }
    write!(output, "],\"usage\":")?;
    write_usage(output, &state.usage)?;
    output.write_str(",\"termination\":{\"status\":")?;
    write_json_string(output, termination_status)?;
    output.write_str(",\"code\":")?;
    write_optional_string(output, termination_code)?;
    output.write_str(",\"message\":")?;
    write_optional_string(output, termination_message)?;
    output.write_str("},\"nonclaims\":[")?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_char(']')?;
    output.write_str("}\n")
}

pub(super) fn render_event(event: &TraceEvent) -> String {
    let mut output = String::new();
    write_event(&mut output, event).expect("String writes are infallible");
    output
}

fn write_event<W: fmt::Write>(output: &mut W, event: &TraceEvent) -> fmt::Result {
    write_event_parts(
        output,
        event.index,
        event.turn,
        event.kind,
        event.provider_id.as_deref(),
        event.model_id.as_deref(),
        event.tool_id.as_deref(),
        event.input_digest.as_deref(),
        event.output_digest.as_deref(),
        event.status,
        event.usage,
    )
}

#[allow(clippy::too_many_arguments)]
fn write_event_parts<W: fmt::Write>(
    output: &mut W,
    index: u64,
    turn: u64,
    kind: &str,
    provider_id: Option<&str>,
    model_id: Option<&str>,
    tool_id: Option<&str>,
    input_digest: Option<&str>,
    output_digest: Option<&str>,
    status: &str,
    usage: UsageDelta,
) -> fmt::Result {
    write!(output, "{{\"index\":{},\"turn\":{},\"kind\":", index, turn)?;
    write_json_string(output, kind)?;
    output.write_str(",\"provider_id\":")?;
    write_optional_string(output, provider_id)?;
    output.write_str(",\"model_id\":")?;
    write_optional_string(output, model_id)?;
    output.write_str(",\"tool_id\":")?;
    write_optional_string(output, tool_id)?;
    output.write_str(",\"input_digest\":")?;
    write_optional_string(output, input_digest)?;
    output.write_str(",\"output_digest\":")?;
    write_optional_string(output, output_digest)?;
    output.write_str(",\"status\":")?;
    write_json_string(output, status)?;
    output.write_str(",\"usage\":")?;
    write_usage_delta(output, usage)?;
    output.write_char('}')
}

fn write_json_string<W: fmt::Write>(output: &mut W, value: &str) -> fmt::Result {
    output.write_char('"')?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"")?,
            '\\' => output.write_str("\\\\")?,
            '\u{08}' => output.write_str("\\b")?,
            '\u{0c}' => output.write_str("\\f")?,
            '\n' => output.write_str("\\n")?,
            '\r' => output.write_str("\\r")?,
            '\t' => output.write_str("\\t")?,
            '\u{00}'..='\u{1f}' => write!(output, "\\u{:04x}", character as u32)?,
            _ => output.write_char(character)?,
        }
    }
    output.write_char('"')
}

fn write_optional_string<W: fmt::Write>(output: &mut W, value: Option<&str>) -> fmt::Result {
    match value {
        Some(value) => write_json_string(output, value),
        None => output.write_str("null"),
    }
}

fn optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "null".to_owned(), quote_json)
}
fn render_usage_delta(usage: UsageDelta) -> String {
    let mut output = String::new();
    write_usage_delta(&mut output, usage).expect("String writes are infallible");
    output
}
fn write_usage_delta<W: fmt::Write>(output: &mut W, usage: UsageDelta) -> fmt::Result {
    write!(output, "{{\"provider_input_bytes\":{},\"provider_output_bytes\":{},\"reported_model_input_tokens\":{},\"reported_model_output_tokens\":{},\"usd_microunits\":{},\"tool_argument_bytes\":{},\"tool_result_bytes\":{},\"elapsed_ms\":{}}}", usage.provider_input_bytes, usage.provider_output_bytes, usage.reported_model_input_tokens, usage.reported_model_output_tokens, usage.usd_microunits, usage.tool_argument_bytes, usage.tool_result_bytes, usage.elapsed_ms)
}
pub(super) fn render_usage(usage: &Usage) -> String {
    let mut output = String::new();
    write_usage(&mut output, usage).expect("String writes are infallible");
    output
}
fn write_usage<W: fmt::Write>(output: &mut W, usage: &Usage) -> fmt::Result {
    write!(output, "{{\"turns\":{},\"provider_attempts\":{},\"provider_input_bytes\":{},\"provider_output_bytes\":{},\"reported_model_input_tokens\":{},\"reported_model_output_tokens\":{},\"usd_microunits\":{},\"tool_calls\":{},\"tool_argument_bytes\":{},\"tool_result_bytes\":{},\"retained_state_bytes\":{},\"elapsed_ms\":{},\"max_concurrency\":{}}}", usage.turns, usage.provider_attempts, usage.provider_input_bytes, usage.provider_output_bytes, usage.reported_model_input_tokens, usage.reported_model_output_tokens, usage.usd_microunits, usage.tool_calls, usage.tool_argument_bytes, usage.tool_result_bytes, usage.retained_state_bytes, usage.elapsed_ms, usage.max_concurrency)
}

fn render_evidence(
    profile: &Profile,
    state: &RunState,
    trace: &str,
    trace_digest: &str,
    budget: &EvidenceBudget,
) -> String {
    let mut output = String::new();
    write_evidence(&mut output, profile, state, trace, trace_digest, budget)
        .expect("String writes are infallible");
    output
}

fn write_evidence<W: fmt::Write>(
    output: &mut W,
    profile: &Profile,
    state: &RunState,
    trace: &str,
    trace_digest: &str,
    budget: &EvidenceBudget,
) -> fmt::Result {
    write_evidence_status(
        output,
        profile,
        state,
        trace,
        trace_digest,
        budget,
        state.termination.status.text(),
    )
}

#[allow(clippy::too_many_arguments)]
fn write_evidence_status<W: fmt::Write>(
    output: &mut W,
    profile: &Profile,
    state: &RunState,
    trace: &str,
    trace_digest: &str,
    budget: &EvidenceBudget,
    result_status: &str,
) -> fmt::Result {
    write!(output, "{{\"schema\":\"{EVIDENCE_SCHEMA}\",\"run_id\":")?;
    write_json_string(output, &state.run_id)?;
    write!(
        output,
        ",\"profile\":{{\"schema\":\"{PROFILE_SCHEMA}\",\"digest\":"
    )?;
    write_json_string(output, &profile.digest)?;
    write!(
        output,
        ",\"bytes\":{}}},\"task\":{{\"schema\":\"{TASK_SCHEMA}\",\"digest\":",
        profile.source.len()
    )?;
    write_json_string(output, &state.task_digest)?;
    write!(
        output,
        ",\"bytes\":{}}},\"trace\":{{\"schema\":\"{TRACE_SCHEMA}\",\"digest\":",
        state.task_bytes
    )?;
    write_json_string(output, trace_digest)?;
    write!(output, ",\"bytes\":{},\"document\":", trace.len())?;
    write_json_string(output, trace)?;
    output.write_str("},\"result\":{\"status\":")?;
    write_json_string(output, result_status)?;
    output.write_str(",\"final_message_digest\":")?;
    if let Some(message) = &state.final_message {
        write_json_string(output, &digest(FINAL_MESSAGE_DOMAIN, message.as_bytes()))?;
        write!(output, ",\"final_message_bytes\":{}", message.len())?;
    } else {
        output.write_str("null,\"final_message_bytes\":0")?;
    }
    write!(output, ",\"last_turn\":{}}},\"limits\":", state.last_turn)?;
    write_production_limits(output)?;
    output.write_str(",\"budget\":")?;
    write_budget(output, budget)?;
    output.write_str(",\"nonclaims\":[")?;
    for (index, value) in NONCLAIMS.iter().enumerate() {
        if index > 0 {
            output.write_char(',')?;
        }
        write_json_string(output, value)?;
    }
    output.write_str("]}\n")
}

pub(super) fn render_production_limits() -> String {
    let mut output = String::new();
    write_production_limits(&mut output).expect("String writes are infallible");
    output
}
fn write_production_limits<W: fmt::Write>(output: &mut W) -> fmt::Result {
    write!(output, "{{\"max_profile_bytes\":{MAX_PROFILE_BYTES},\"max_task_bytes\":{MAX_TASK_BYTES},\"max_models\":{MAX_MODELS},\"max_tools\":{MAX_TOOLS},\"max_capabilities\":{MAX_CAPABILITIES},\"max_turns\":{MAX_TURNS},\"max_provider_attempts\":{MAX_PROVIDER_ATTEMPTS},\"max_retries_per_turn\":{MAX_RETRIES_PER_TURN},\"max_concurrency\":{MAX_CONCURRENCY},\"max_elapsed_ms\":{MAX_ELAPSED_MS},\"max_provider_request_bytes\":{MAX_PROVIDER_REQUEST_BYTES},\"max_provider_response_bytes\":{MAX_PROVIDER_RESPONSE_BYTES},\"max_stream_chunks\":{MAX_STREAM_CHUNKS},\"max_total_provider_input_bytes\":{MAX_TOTAL_PROVIDER_INPUT_BYTES},\"max_total_provider_output_bytes\":{MAX_TOTAL_PROVIDER_OUTPUT_BYTES},\"max_reported_model_input_tokens\":{MAX_REPORTED_MODEL_INPUT_TOKENS},\"max_reported_model_output_tokens\":{MAX_REPORTED_MODEL_OUTPUT_TOKENS},\"max_usd_microunits\":{MAX_USD_MICROUNITS},\"max_tool_calls\":{MAX_TOOL_CALLS},\"max_tool_arguments_bytes\":{MAX_TOOL_ARGUMENT_BYTES},\"max_tool_result_bytes\":{MAX_TOOL_RESULT_BYTES},\"max_total_tool_bytes\":{MAX_TOTAL_TOOL_BYTES},\"max_retained_state_bytes\":{MAX_RETAINED_STATE_BYTES},\"max_trace_events\":{MAX_TRACE_EVENTS},\"max_trace_bytes\":{MAX_TRACE_BYTES},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_json_depth\":{MAX_JSON_DEPTH},\"max_identifier_bytes\":{MAX_IDENTIFIER_BYTES},\"max_description_bytes\":{MAX_DESCRIPTION_BYTES}}}")
}

pub(super) fn render_budget(budget: &EvidenceBudget) -> String {
    let mut output = String::new();
    write_budget(&mut output, budget).expect("String writes are infallible");
    output
}
fn write_budget<W: fmt::Write>(output: &mut W, budget: &EvidenceBudget) -> fmt::Result {
    write!(output, "{{\"used_models\":{},\"used_tools\":{},\"used_capabilities\":{},\"used_turns\":{},\"used_provider_attempts\":{},\"used_provider_input_bytes\":{},\"used_provider_output_bytes\":{},\"used_reported_model_input_tokens\":{},\"used_reported_model_output_tokens\":{},\"used_usd_microunits\":{},\"used_tool_calls\":{},\"used_tool_argument_bytes\":{},\"used_tool_result_bytes\":{},\"used_retained_state_bytes\":{},\"used_trace_events\":{},\"used_trace_bytes\":{},\"used_evidence_bytes\":{},\"used_builder_bytes\":{},\"used_elapsed_ms\":{},\"used_concurrency\":{}}}", budget.used_models, budget.used_tools, budget.used_capabilities, budget.used_turns, budget.used_provider_attempts, budget.used_provider_input_bytes, budget.used_provider_output_bytes, budget.used_reported_model_input_tokens, budget.used_reported_model_output_tokens, budget.used_usd_microunits, budget.used_tool_calls, budget.used_tool_argument_bytes, budget.used_tool_result_bytes, budget.used_retained_state_bytes, budget.used_trace_events, budget.used_trace_bytes, budget.used_evidence_bytes, budget.used_builder_bytes, budget.used_elapsed_ms, budget.used_concurrency)
}
