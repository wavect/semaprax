//! Execution transitions: the bounded run state machine, provider turns,
//! tool execution, event mutation and termination selection.

use super::accounting::{
    account_partial_provider, account_uncertain, observe_provider_attempt, reserve_provider_attempt,
};
use super::admission::{parse_action, reserve_builder_copy, validate_schema};
use super::deadline::{boundary_termination, remaining_deadline_ms, termination_for_status};
use super::evidence::{preflight_current_terminal, preflight_external_capacity, ExternalBoundary};
use super::request::{authorize_tool, render_tool_result, route};
use super::*;

pub(super) fn run_bounded<H: AgentHost>(
    profile: &Profile,
    host: &mut H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
    task: Task,
) -> Result<RunState, Diagnostic> {
    let terminal_lane = usize::try_from(profile.limits.max_trace_bytes)
        .ok()
        .and_then(|trace| {
            usize::try_from(profile.limits.max_evidence_bytes)
                .ok()
                .and_then(|evidence| evidence.checked_mul(2))
                .and_then(|evidence| trace.checked_add(evidence))
        })
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| g208("builder_bytes", profile.limits.max_builder_bytes))?;
    if !reserve_active(terminal_lane) {
        return Err(g208("builder_bytes", profile.limits.max_builder_bytes));
    }
    let run_id = run_id(&profile.digest, &task.digest, &task.nonce)?;
    let mut state = RunState {
        run_id,
        events: Vec::new(),
        usage: Usage {
            max_concurrency: 1,
            ..Usage::default()
        },
        history: Vec::new(),
        final_message: None,
        last_turn: 0,
        termination: termination_from_diagnostic(g208("turns", profile.limits.max_turns)),
        task_digest: task.digest.clone(),
        task_bytes: task.source.len() as u64,
        task_nonce: task.nonce.clone(),
        external_effect_crossed: false,
        provider_accounting: Vec::new(),
    };
    push_event(
        &mut state,
        profile.limits,
        0,
        "run_started",
        None,
        None,
        Some(profile.digest.clone()),
        Some(task.digest.clone()),
        "started",
        UsageDelta::default(),
    )?;
    if profile.limits.max_trace_events < 2 {
        return Err(g208("trace_events", profile.limits.max_trace_events));
    }
    if cancellation.is_cancelled() {
        return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
    }
    let drive = drive(profile, host, cancellation, policy_epoch, &task, &mut state);
    if let Err(diagnostic) = drive {
        if !state.external_effect_crossed
            && ((diagnostic.code == "SPX-G208"
                && (diagnostic.message.starts_with("trace_bytes exceeds ")
                    || diagnostic.message.starts_with("evidence_bytes exceeds ")
                    || diagnostic.message.starts_with("trace_events exceeds ")
                    || diagnostic.message.starts_with("builder_bytes exceeds ")))
                || diagnostic.code == "SPX-I220")
        {
            return Err(diagnostic);
        }
        state.termination = termination_from_diagnostic(diagnostic);
    }
    state.usage.elapsed_ms = host.elapsed_ms();
    let status = state.termination.status.text();
    let last_turn = state.last_turn;
    push_final_event(&mut state, profile.limits, last_turn, status)?;
    Ok(state)
}

fn drive<H: AgentHost>(
    profile: &Profile,
    host: &mut H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
    task: &Task,
    state: &mut RunState,
) -> Result<(), Diagnostic> {
    for turn in 1..=profile.limits.max_turns {
        let previous_usage = state.usage.clone();
        let previous_turn = state.last_turn;
        state.last_turn = turn;
        if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
            if termination.status == RunStatus::Cancelled && !state.external_effect_crossed {
                return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
            }
            state.termination = termination;
            return Ok(());
        }
        state.usage.turns = turn;
        let route = match route(profile, host, cancellation, task, state, turn) {
            Ok(route) => route,
            Err(diagnostic) => {
                state.usage = previous_usage;
                state.last_turn = previous_turn;
                return Err(diagnostic);
            }
        };
        let model = &profile.models[route.model_index];
        if let Err(diagnostic) = push_internal_event(
            profile,
            state,
            turn,
            "route_selected",
            Some(model),
            None,
            Some(route.request_digest.clone()),
            None,
            "selected",
            UsageDelta::default(),
        ) {
            state.usage = previous_usage;
            state.last_turn = previous_turn;
            return Err(diagnostic);
        }
        let action = provider_turn(
            profile,
            host,
            cancellation,
            policy_epoch,
            state,
            turn,
            model,
            &route,
        )?;
        let Some(action) = action else {
            return Ok(());
        };
        match action {
            Action::Final { message, source } => {
                if cancellation.is_cancelled() {
                    state.termination = termination_from_diagnostic(operational(
                        "SPX-I220",
                        "Agent Runtime run was cancelled",
                    ));
                    return Ok(());
                }
                push_internal_event(
                    profile,
                    state,
                    turn,
                    "action_accepted",
                    Some(model),
                    None,
                    Some(digest(ACTION_DOMAIN, source.as_bytes())),
                    Some(digest(FINAL_MESSAGE_DOMAIN, message.as_bytes())),
                    "final",
                    UsageDelta::default(),
                )?;
                state.final_message = Some(message);
                state.termination = Termination {
                    status: RunStatus::Completed,
                    code: None,
                    message: None,
                };
                return Ok(());
            }
            Action::Tool {
                tool_id,
                arguments,
                source,
            } => {
                execute_tool(
                    profile,
                    host,
                    cancellation,
                    policy_epoch,
                    task,
                    state,
                    turn,
                    model,
                    tool_id,
                    arguments,
                    source,
                )?;
                if matches!(
                    state.termination.status,
                    RunStatus::Cancelled
                        | RunStatus::DeadlineExceeded
                        | RunStatus::ProviderFailed
                        | RunStatus::ToolFailed
                        | RunStatus::PolicyRejected
                ) || (state.termination.status == RunStatus::BudgetExhausted
                    && state
                        .termination
                        .message
                        .as_deref()
                        .is_some_and(|message| !message.starts_with("turns exceeds ")))
                {
                    return Ok(());
                }
            }
        }
    }
    state.termination = termination_from_diagnostic(g208("turns", profile.limits.max_turns));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn provider_turn<H: AgentHost>(
    profile: &Profile,
    host: &mut H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
    state: &mut RunState,
    turn: u64,
    model: &Model,
    route: &Route,
) -> Result<Option<Action>, Diagnostic> {
    let mut retry = 0;
    loop {
        if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
            if termination.status == RunStatus::Cancelled && !state.external_effect_crossed {
                return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
            }
            state.termination = termination;
            return Ok(None);
        }
        let response_remaining = profile
            .limits
            .max_total_provider_output_bytes
            .saturating_sub(state.usage.provider_output_bytes);
        let token_remaining = profile
            .limits
            .max_reported_model_output_tokens
            .saturating_sub(state.usage.reported_model_output_tokens);
        if response_remaining == 0 || token_remaining == 0 {
            return Err(if response_remaining == 0 {
                g208(
                    "total_provider_output_bytes",
                    profile.limits.max_total_provider_output_bytes,
                )
            } else {
                g208(
                    "reported_model_output_tokens",
                    profile.limits.max_reported_model_output_tokens,
                )
            });
        }
        reserve_builder_copy(
            usize::try_from(response_remaining.min(profile.limits.max_provider_response_bytes))
                .map_err(|_| g208("builder_bytes", profile.limits.max_builder_bytes))?,
            MAX_JSON_DEPTH + 4,
        )?;
        if !reserve_active(1024) {
            return Err(g208("builder_bytes", profile.limits.max_builder_bytes));
        }
        let previous_usage = state.usage.clone();
        let mut next_usage = previous_usage.clone();
        checked_add(
            &mut next_usage.provider_attempts,
            1,
            "provider_attempts",
            profile.limits.max_provider_attempts,
        )?;
        checked_add(
            &mut next_usage.provider_input_bytes,
            route.request.len() as u64,
            "total_provider_input_bytes",
            profile.limits.max_total_provider_input_bytes,
        )?;
        checked_add(
            &mut next_usage.reported_model_input_tokens,
            route.input_tokens,
            "reported_model_input_tokens",
            profile.limits.max_reported_model_input_tokens,
        )?;
        checked_add(
            &mut next_usage.usd_microunits,
            route.reserved_cost,
            "usd_microunits",
            profile.limits.max_usd_microunits,
        )?;
        let mut sink = ProviderSink::new(
            profile.limits,
            response_remaining,
            host.boundary_probe(),
            policy_epoch,
            cancellation.clone(),
        );
        preflight_external_capacity(profile, state, ExternalBoundary::Provider(model, route))?;
        if crate::bounded_output::active_remaining().is_some_and(|remaining| remaining == 0) {
            return Err(g208("builder_bytes", profile.limits.max_builder_bytes));
        }
        if cancellation.is_cancelled() {
            return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
        }
        state.usage = next_usage;
        if let Err(diagnostic) = push_event(
            state,
            profile.limits,
            turn,
            "provider_attempt_started",
            Some(model),
            None,
            Some(route.request_digest.clone()),
            None,
            "started",
            UsageDelta {
                provider_input_bytes: route.request.len() as u64,
                reported_model_input_tokens: route.input_tokens,
                usd_microunits: route.reserved_cost,
                ..UsageDelta::default()
            },
        ) {
            state.usage = previous_usage;
            return Err(diagnostic);
        }
        let remaining_deadline_ms =
            match remaining_deadline_ms(profile, host, cancellation, policy_epoch) {
                Ok(remaining) => remaining,
                Err(termination) if termination.status == RunStatus::Cancelled => {
                    state.events.pop();
                    state.usage = previous_usage;
                    return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
                }
                Err(termination) => {
                    state.events.pop();
                    state.usage = previous_usage;
                    state.termination = termination;
                    return Ok(None);
                }
            };
        if cancellation.is_cancelled() {
            state.events.pop();
            state.usage = previous_usage;
            return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
        }
        if let Err(diagnostic) = reserve_provider_attempt(state, route) {
            state.events.pop();
            state.usage = previous_usage;
            return Err(diagnostic);
        }
        state.external_effect_crossed = true;
        let attempt = host.attempt_provider(
            &model.provider_id,
            &model.model_id,
            &route.request,
            remaining_deadline_ms,
            &mut sink,
        );
        if cancellation.is_cancelled() && sink.bounded.boundary.is_none() {
            sink.bounded.boundary = Some(RunStatus::Cancelled);
        }
        if let Some(boundary) = sink.bounded.boundary {
            account_partial_provider(state, &sink, attempt.usage, route, profile.limits)?;
            state.termination = termination_for_status(boundary);
            let status = boundary.text();
            push_event(
                state,
                profile.limits,
                turn,
                "provider_attempt_finished",
                Some(model),
                None,
                None,
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bounded.bytes)),
                status,
                UsageDelta {
                    provider_output_bytes: sink.bounded.bytes.len() as u64,
                    reported_model_output_tokens: attempt.usage.output_tokens,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
            account_partial_provider(state, &sink, attempt.usage, route, profile.limits)?;
            state.termination = termination;
            let status = state.termination.status.text();
            push_event(
                state,
                profile.limits,
                turn,
                "provider_attempt_finished",
                Some(model),
                None,
                None,
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bounded.bytes)),
                status,
                UsageDelta {
                    provider_output_bytes: sink.bounded.bytes.len() as u64,
                    reported_model_output_tokens: attempt.usage.output_tokens,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        let exact_zero = sink.bounded.bytes.is_empty()
            && sink.chunks == 0
            && attempt.usage.input_tokens == 0
            && attempt.usage.output_tokens == 0
            && attempt.usage.usd_microunits == 0;
        match attempt.disposition {
            ProviderDisposition::DefinitelyNotStarted
                if exact_zero && retry < profile.limits.max_retries_per_turn =>
            {
                push_event(
                    state,
                    profile.limits,
                    turn,
                    "provider_attempt_finished",
                    Some(model),
                    None,
                    None,
                    None,
                    "definitely_not_started",
                    UsageDelta::default(),
                )?;
                retry += 1;
                continue;
            }
            ProviderDisposition::DefinitelyNotStarted if !exact_zero => {
                account_uncertain(
                    &mut state.usage,
                    &sink,
                    attempt.usage,
                    route,
                    profile.limits,
                )?;
                state.termination = termination_from_diagnostic(operational(
                    "SPX-I218",
                    "Agent Runtime provider adapter failed: start uncertain",
                ));
                push_event(
                    state,
                    profile.limits,
                    turn,
                    "provider_attempt_finished",
                    Some(model),
                    None,
                    None,
                    Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bounded.bytes)),
                    "failed_uncertain",
                    UsageDelta {
                        provider_output_bytes: sink.bounded.bytes.len() as u64,
                        reported_model_output_tokens: attempt.usage.output_tokens,
                        ..UsageDelta::default()
                    },
                )?;
                return Ok(None);
            }
            ProviderDisposition::DefinitelyNotStarted => {
                state.termination = termination_from_diagnostic(operational(
                    "SPX-I218",
                    "Agent Runtime provider adapter failed: definitely not started",
                ));
                push_event(
                    state,
                    profile.limits,
                    turn,
                    "provider_attempt_finished",
                    Some(model),
                    None,
                    None,
                    None,
                    "definitely_not_started",
                    UsageDelta::default(),
                )?;
                return Ok(None);
            }
            ProviderDisposition::FailedUncertain => {
                account_uncertain(
                    &mut state.usage,
                    &sink,
                    attempt.usage,
                    route,
                    profile.limits,
                )?;
                state.termination = termination_from_diagnostic(operational(
                    "SPX-I218",
                    "Agent Runtime provider adapter failed: start uncertain",
                ));
                push_event(
                    state,
                    profile.limits,
                    turn,
                    "provider_attempt_finished",
                    Some(model),
                    None,
                    None,
                    Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bounded.bytes)),
                    "failed_uncertain",
                    UsageDelta {
                        provider_output_bytes: sink.bounded.bytes.len() as u64,
                        reported_model_output_tokens: attempt.usage.output_tokens,
                        ..UsageDelta::default()
                    },
                )?;
                return Ok(None);
            }
            ProviderDisposition::Succeeded => {}
        }
        if let Some(rejection) = sink.bounded.rejection {
            account_partial_provider(state, &sink, attempt.usage, route, profile.limits)?;
            let diagnostic = match rejection {
                SinkRejection::Builder => g208("builder_bytes", profile.limits.max_builder_bytes),
                SinkRejection::Chunks => g208("stream_chunks", profile.limits.max_stream_chunks),
                SinkRejection::Bytes => g208(
                    "provider_response_bytes",
                    profile.limits.max_provider_response_bytes,
                ),
            };
            state.termination = termination_from_diagnostic(diagnostic);
            push_event(
                state,
                profile.limits,
                turn,
                "provider_attempt_finished",
                Some(model),
                None,
                None,
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bounded.bytes)),
                "failed_uncertain",
                UsageDelta {
                    provider_output_bytes: sink.bounded.bytes.len() as u64,
                    reported_model_output_tokens: attempt.usage.output_tokens,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        let response_bytes = sink.bounded.bytes;
        let response = match String::from_utf8(response_bytes) {
            Ok(response) => response,
            Err(error) => {
                let bytes = error.into_bytes();
                checked_add(
                    &mut state.usage.provider_output_bytes,
                    bytes.len() as u64,
                    "total_provider_output_bytes",
                    profile.limits.max_total_provider_output_bytes,
                )?;
                checked_add(
                    &mut state.usage.reported_model_output_tokens,
                    attempt.usage.output_tokens,
                    "reported_model_output_tokens",
                    profile.limits.max_reported_model_output_tokens,
                )?;
                state.termination = termination_from_diagnostic(operational(
                    "SPX-I218",
                    "Agent Runtime provider adapter failed: response invalid",
                ));
                push_event(
                    state,
                    profile.limits,
                    turn,
                    "provider_attempt_finished",
                    Some(model),
                    None,
                    None,
                    Some(digest(PROVIDER_RESPONSE_DOMAIN, &bytes)),
                    "failed_uncertain",
                    UsageDelta {
                        provider_output_bytes: bytes.len() as u64,
                        reported_model_output_tokens: attempt.usage.output_tokens,
                        ..UsageDelta::default()
                    },
                )?;
                return Ok(None);
            }
        };
        if attempt.usage.input_tokens > route.input_tokens
            || attempt.usage.output_tokens > route.output_token_reservation
            || attempt.usage.usd_microunits > route.reserved_cost
        {
            checked_add(
                &mut state.usage.provider_output_bytes,
                response.len() as u64,
                "total_provider_output_bytes",
                profile.limits.max_total_provider_output_bytes,
            )?;
            state.termination = termination_from_diagnostic(operational(
                "SPX-I218",
                "Agent Runtime provider adapter failed: usage invalid",
            ));
            push_event(
                state,
                profile.limits,
                turn,
                "provider_attempt_finished",
                Some(model),
                None,
                None,
                Some(digest(PROVIDER_RESPONSE_DOMAIN, response.as_bytes())),
                "failed_uncertain",
                UsageDelta {
                    provider_output_bytes: response.len() as u64,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        observe_provider_attempt(state, attempt.usage, route)?;
        checked_add(
            &mut state.usage.provider_output_bytes,
            response.len() as u64,
            "total_provider_output_bytes",
            profile.limits.max_total_provider_output_bytes,
        )?;
        checked_add(
            &mut state.usage.reported_model_output_tokens,
            attempt.usage.output_tokens,
            "reported_model_output_tokens",
            profile.limits.max_reported_model_output_tokens,
        )?;
        push_event(
            state,
            profile.limits,
            turn,
            "provider_attempt_finished",
            Some(model),
            None,
            None,
            Some(digest(PROVIDER_RESPONSE_DOMAIN, response.as_bytes())),
            "succeeded",
            UsageDelta {
                provider_output_bytes: response.len() as u64,
                reported_model_output_tokens: attempt.usage.output_tokens,
                ..UsageDelta::default()
            },
        )?;
        let action = parse_action(
            response,
            profile.limits.max_provider_response_bytes as usize,
        )
        .map_err(|diagnostic| {
            if diagnostic.code == "SPX-G208" {
                diagnostic
            } else {
                operational(
                    "SPX-I218",
                    "Agent Runtime provider adapter failed: response invalid",
                )
            }
        })?;
        return Ok(Some(action));
    }
}

#[allow(clippy::too_many_arguments)]
fn execute_tool<H: AgentHost>(
    profile: &Profile,
    host: &mut H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
    task: &Task,
    state: &mut RunState,
    turn: u64,
    model: &Model,
    tool_id: String,
    arguments: Value,
    source: String,
) -> Result<(), Diagnostic> {
    authorize_tool(profile, &tool_id, &arguments)?;
    let tool = profile
        .tools
        .iter()
        .find(|tool| tool.tool_id == tool_id)
        .ok_or_else(|| g207("unknown tool"))?;
    let arguments_json =
        validate_schema(&arguments, &tool.arguments_schema, MAX_TOOL_ARGUMENT_BYTES)
            .map_err(|_| g207("arguments schema mismatch"))?;
    if arguments_json.len() as u64 > profile.limits.max_tool_arguments_bytes {
        return Err(g208(
            "tool_arguments_bytes",
            profile.limits.max_tool_arguments_bytes,
        ));
    }
    let next_tool_calls = state
        .usage
        .tool_calls
        .checked_add(1)
        .ok_or_else(|| g208("tool_calls", profile.limits.max_tool_calls))?;
    if next_tool_calls > profile.limits.max_tool_calls {
        return Err(g208("tool_calls", profile.limits.max_tool_calls));
    }
    let new_arguments = state
        .usage
        .tool_argument_bytes
        .checked_add(arguments_json.len() as u64)
        .ok_or_else(|| g208("total_tool_bytes", profile.limits.max_total_tool_bytes))?;
    if new_arguments
        .checked_add(state.usage.tool_result_bytes)
        .is_none_or(|used| used > profile.limits.max_total_tool_bytes)
    {
        return Err(g208(
            "total_tool_bytes",
            profile.limits.max_total_tool_bytes,
        ));
    }
    let call_id = call_id(&state.run_id, turn, &tool_id, &arguments_json);
    let empty_envelope = render_tool_result(&call_id, &tool_id, "{}");
    if empty_envelope.len() as u64 > profile.limits.max_tool_result_bytes {
        state.termination = termination_from_diagnostic(g208(
            "tool_result_bytes",
            profile.limits.max_tool_result_bytes,
        ));
        return Ok(());
    }
    let previous_usage = state.usage.clone();
    state.usage.tool_calls = next_tool_calls;
    state.usage.tool_argument_bytes = new_arguments;
    if let Err(diagnostic) = push_internal_event(
        profile,
        state,
        turn,
        "action_accepted",
        Some(model),
        Some(&tool_id),
        Some(digest(ACTION_DOMAIN, source.as_bytes())),
        None,
        "tool",
        UsageDelta::default(),
    ) {
        state.usage = previous_usage;
        return Err(diagnostic);
    }
    if let Err(diagnostic) = push_internal_event(
        profile,
        state,
        turn,
        "tool_authorized",
        Some(model),
        Some(&tool_id),
        Some(digest(ACTION_DOMAIN, source.as_bytes())),
        None,
        "authorized",
        UsageDelta {
            tool_argument_bytes: arguments_json.len() as u64,
            ..UsageDelta::default()
        },
    ) {
        state.events.pop();
        state.usage = previous_usage;
        return Err(diagnostic);
    }
    if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
        state.termination = termination;
        return Ok(());
    }
    let payload_limit = profile
        .limits
        .max_tool_result_bytes
        .saturating_sub(empty_envelope.len() as u64 - 2);
    let total_remaining = profile
        .limits
        .max_total_tool_bytes
        .saturating_sub(state.usage.tool_argument_bytes)
        .saturating_sub(state.usage.tool_result_bytes);
    let envelope_overhead = empty_envelope.len() as u64 - 2;
    let payload_limit = payload_limit.min(total_remaining.saturating_sub(envelope_overhead));
    let mut sink = ToolResultSink::new(
        payload_limit,
        host.boundary_probe(),
        policy_epoch,
        profile.limits.max_elapsed_ms,
        cancellation.clone(),
    );
    reserve_builder_copy(
        usize::try_from(payload_limit)
            .map_err(|_| g208("builder_bytes", profile.limits.max_builder_bytes))?,
        MAX_JSON_DEPTH + 4,
    )?;
    preflight_external_capacity(profile, state, ExternalBoundary::Tool(model, &tool_id))?;
    let remaining_deadline_ms =
        match remaining_deadline_ms(profile, host, cancellation, policy_epoch) {
            Ok(remaining) => remaining,
            Err(termination) => {
                state.termination = termination;
                return Ok(());
            }
        };
    if cancellation.is_cancelled() {
        state.termination =
            termination_from_diagnostic(operational("SPX-I220", "Agent Runtime run was cancelled"));
        return Ok(());
    }
    state.external_effect_crossed = true;
    let invocation = host.invoke_tool_with_deadline(
        &call_id,
        &tool_id,
        &arguments_json,
        remaining_deadline_ms,
        &mut sink,
    );
    if cancellation.is_cancelled() && sink.bounded.boundary.is_none() {
        sink.bounded.boundary = Some(RunStatus::Cancelled);
    }
    if let Some(boundary) = sink.bounded.boundary {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bounded.bytes.len() as u64,
            "total_tool_bytes",
            profile.limits.max_total_tool_bytes,
        )?;
        state.termination = termination_for_status(boundary);
        let status = boundary.text();
        push_event(
            state,
            profile.limits,
            turn,
            "tool_finished",
            Some(model),
            Some(&tool_id),
            None,
            None,
            status,
            UsageDelta {
                tool_result_bytes: sink.bounded.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if let Some(rejection) = sink.bounded.rejection {
        state.usage.tool_result_bytes = state
            .usage
            .tool_result_bytes
            .checked_add(sink.bounded.bytes.len() as u64)
            .ok_or_else(|| g208("total_tool_bytes", profile.limits.max_total_tool_bytes))?;
        let diagnostic = match rejection {
            SinkRejection::Builder => g208("builder_bytes", profile.limits.max_builder_bytes),
            SinkRejection::Bytes | SinkRejection::Chunks => {
                g208("tool_result_bytes", profile.limits.max_tool_result_bytes)
            }
        };
        state.termination = termination_from_diagnostic(diagnostic);
        push_event(
            state,
            profile.limits,
            turn,
            "tool_finished",
            Some(model),
            Some(&tool_id),
            None,
            None,
            "failed",
            UsageDelta {
                tool_result_bytes: sink.bounded.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if !invocation {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bounded.bytes.len() as u64,
            "total_tool_bytes",
            profile.limits.max_total_tool_bytes,
        )?;
        state.termination = termination_from_diagnostic(operational(
            "SPX-I219",
            "Agent Runtime tool adapter failed: invocation failed",
        ));
        push_event(
            state,
            profile.limits,
            turn,
            "tool_finished",
            Some(model),
            Some(&tool_id),
            None,
            None,
            "failed",
            UsageDelta {
                tool_result_bytes: sink.bounded.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bounded.bytes.len() as u64,
            "total_tool_bytes",
            profile.limits.max_total_tool_bytes,
        )?;
        state.termination = termination;
        let status = state.termination.status.text();
        push_event(
            state,
            profile.limits,
            turn,
            "tool_finished",
            Some(model),
            Some(&tool_id),
            None,
            None,
            status,
            UsageDelta {
                tool_result_bytes: sink.bounded.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    let received_bytes = sink.bounded.bytes.len() as u64;
    let value: Value = match serde_json::from_slice(&sink.bounded.bytes) {
        Ok(value) => value,
        Err(_) => {
            return finish_failed_tool_result(
                profile,
                state,
                turn,
                model,
                &tool_id,
                received_bytes,
                operational(
                    "SPX-I219",
                    "Agent Runtime tool adapter failed: result invalid",
                ),
            );
        }
    };
    let result_json = match validate_schema(
        &value,
        &tool.result_schema,
        profile.limits.max_tool_result_bytes,
    ) {
        Ok(result) => result,
        Err(()) => {
            return finish_failed_tool_result(
                profile,
                state,
                turn,
                model,
                &tool_id,
                received_bytes,
                g207("result schema mismatch"),
            );
        }
    };
    let envelope = render_tool_result(&call_id, &tool_id, &result_json);
    if envelope.len() as u64 > profile.limits.max_tool_result_bytes {
        return finish_failed_tool_result(
            profile,
            state,
            turn,
            model,
            &tool_id,
            received_bytes,
            g208("tool_result_bytes", profile.limits.max_tool_result_bytes),
        );
    }
    let new_results = state
        .usage
        .tool_result_bytes
        .checked_add(envelope.len() as u64)
        .ok_or_else(|| g208("total_tool_bytes", profile.limits.max_total_tool_bytes))?;
    if state
        .usage
        .tool_argument_bytes
        .checked_add(new_results)
        .is_none_or(|used| used > profile.limits.max_total_tool_bytes)
    {
        return finish_failed_tool_result(
            profile,
            state,
            turn,
            model,
            &tool_id,
            received_bytes,
            g208("total_tool_bytes", profile.limits.max_total_tool_bytes),
        );
    }
    let prospective_retained = retained_state_bytes_with(task, &state.history, &source, &envelope)?;
    state.usage.tool_result_bytes = new_results;
    if prospective_retained > profile.limits.max_retained_state_bytes {
        state.termination = termination_from_diagnostic(g208(
            "retained_state_bytes",
            profile.limits.max_retained_state_bytes,
        ));
        push_event(
            state,
            profile.limits,
            turn,
            "tool_finished",
            Some(model),
            Some(&tool_id),
            None,
            None,
            "failed",
            UsageDelta {
                tool_result_bytes: envelope.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    push_event(
        state,
        profile.limits,
        turn,
        "tool_finished",
        Some(model),
        Some(&tool_id),
        None,
        Some(digest(TOOL_RESULT_DOMAIN, envelope.as_bytes())),
        "succeeded",
        UsageDelta {
            tool_result_bytes: envelope.len() as u64,
            ..UsageDelta::default()
        },
    )?;
    state.history.push((source, Some(envelope)));
    state.usage.retained_state_bytes = prospective_retained;
    Ok(())
}

fn retained_state_bytes_with(
    task: &Task,
    history: &[(String, Option<String>)],
    action: &str,
    result: &str,
) -> Result<u64, Diagnostic> {
    retained_state_bytes(task, history)?
        .checked_add(action.len() as u64)
        .and_then(|value| value.checked_add(result.len() as u64))
        .ok_or_else(|| g208("retained_state_bytes", MAX_RETAINED_STATE_BYTES))
}

#[allow(clippy::too_many_arguments)]
fn finish_failed_tool_result(
    profile: &Profile,
    state: &mut RunState,
    turn: u64,
    model: &Model,
    tool_id: &str,
    received_bytes: u64,
    diagnostic: Diagnostic,
) -> Result<(), Diagnostic> {
    checked_add(
        &mut state.usage.tool_result_bytes,
        received_bytes,
        "total_tool_bytes",
        profile.limits.max_total_tool_bytes,
    )?;
    state.termination = termination_from_diagnostic(diagnostic);
    push_event(
        state,
        profile.limits,
        turn,
        "tool_finished",
        Some(model),
        Some(tool_id),
        None,
        None,
        "failed",
        UsageDelta {
            tool_result_bytes: received_bytes,
            ..UsageDelta::default()
        },
    )
}

pub(super) fn push_final_event(
    state: &mut RunState,
    limits: EffectiveLimits,
    turn: u64,
    status: &'static str,
) -> Result<(), Diagnostic> {
    push_event(
        state,
        limits,
        turn,
        "run_finished",
        None,
        None,
        None,
        None,
        status,
        UsageDelta {
            elapsed_ms: state.usage.elapsed_ms,
            ..UsageDelta::default()
        },
    )
}

pub(super) fn checked_add(
    target: &mut u64,
    amount: u64,
    field: &str,
    maximum: u64,
) -> Result<(), Diagnostic> {
    let value = target
        .checked_add(amount)
        .ok_or_else(|| g208(field, maximum))?;
    if value > maximum {
        return Err(g208(field, maximum));
    }
    *target = value;
    Ok(())
}

fn retained_state_bytes(
    task: &Task,
    history: &[(String, Option<String>)],
) -> Result<u64, Diagnostic> {
    task.source
        .len()
        .checked_add(
            history
                .iter()
                .try_fold(0usize, |sum, (action, result)| {
                    sum.checked_add(action.len())?
                        .checked_add(result.as_ref().map_or(0, String::len))
                })
                .ok_or_else(|| g208("retained_state_bytes", MAX_RETAINED_STATE_BYTES))?,
        )
        .and_then(|value| u64::try_from(value).ok())
        .ok_or_else(|| g208("retained_state_bytes", MAX_RETAINED_STATE_BYTES))
}

#[allow(clippy::too_many_arguments)]
fn push_event(
    state: &mut RunState,
    limits: EffectiveLimits,
    turn: u64,
    kind: &'static str,
    model: Option<&Model>,
    tool_id: Option<&str>,
    input_digest: Option<String>,
    output_digest: Option<String>,
    status: &'static str,
    usage: UsageDelta,
) -> Result<(), Diagnostic> {
    let reserved = u64::from(kind != "run_finished");
    if state.events.len() as u64 >= limits.max_trace_events.saturating_sub(reserved) {
        return Err(g208("trace_events", limits.max_trace_events));
    }
    state.events.push(TraceEvent {
        index: state.events.len() as u64,
        turn,
        kind,
        provider_id: model.map(|value| value.provider_id.clone()),
        model_id: model.map(|value| value.model_id.clone()),
        tool_id: tool_id.map(str::to_owned),
        input_digest,
        output_digest,
        status,
        usage,
    });
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push_internal_event(
    profile: &Profile,
    state: &mut RunState,
    turn: u64,
    kind: &'static str,
    model: Option<&Model>,
    tool_id: Option<&str>,
    input_digest: Option<String>,
    output_digest: Option<String>,
    status: &'static str,
    usage: UsageDelta,
) -> Result<(), Diagnostic> {
    let saved_usage = state.usage.clone();
    let saved_turn = state.last_turn;
    push_event(
        state,
        profile.limits,
        turn,
        kind,
        model,
        tool_id,
        input_digest,
        output_digest,
        status,
        usage,
    )?;
    if state.external_effect_crossed {
        if let Err(diagnostic) = preflight_current_terminal(profile, state) {
            state.events.pop();
            state.usage = saved_usage;
            state.last_turn = saved_turn;
            return Err(diagnostic);
        }
    }
    Ok(())
}

pub(super) fn termination_from_diagnostic(diagnostic: Diagnostic) -> Termination {
    let status = match diagnostic.code {
        "SPX-I220" => RunStatus::Cancelled,
        "SPX-I221" => RunStatus::DeadlineExceeded,
        "SPX-I218" => RunStatus::ProviderFailed,
        "SPX-I219" => RunStatus::ToolFailed,
        "SPX-G208" => RunStatus::BudgetExhausted,
        _ => RunStatus::PolicyRejected,
    };
    Termination {
        status,
        code: Some(diagnostic.code),
        message: Some(diagnostic.message),
    }
}
