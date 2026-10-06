//! Routing and request construction: eligible-model routing, tool
//! authorization, provider request rendering and validation, and collection of
//! a closed provider sink.

use super::admission::validate_schema;
use super::*;

pub(super) fn route<H: AgentHost>(
    profile: &Profile,
    host: &mut H,
    cancellation: &AgentCancellation,
    task: &Task,
    state: &RunState,
    turn: u64,
) -> Result<Route, Diagnostic> {
    let remaining_output_tokens = profile
        .limits
        .max_reported_model_output_tokens
        .saturating_sub(state.usage.reported_model_output_tokens);
    if remaining_output_tokens == 0 {
        return Err(g208(
            "reported_model_output_tokens",
            profile.limits.max_reported_model_output_tokens,
        ));
    }
    let mut best: Option<(u64, String, String, usize, String, u64, u64)> = None;
    for (index, model) in profile.models.iter().enumerate() {
        if profile
            .policy
            .allowed_provider_ids
            .binary_search(&model.provider_id)
            .is_err()
            || profile
                .policy
                .allowed_model_ids
                .binary_search(&model.model_id)
                .is_err()
            || model.quality_tier < profile.policy.minimum_quality_tier
            || (profile.policy.required_locality == RequiredLocality::LocalOnly
                && model.locality != Locality::Local)
            || !profile
                .policy
                .required_model_capabilities
                .iter()
                .all(|cap| model.capabilities.binary_search(cap).is_ok())
        {
            continue;
        }
        let mut output_reservation = profile
            .limits
            .max_reported_model_output_tokens
            .min(remaining_output_tokens)
            .min(model.max_context_tokens);
        let mut request = String::new();
        let mut tokens = 0;
        let mut fitted = false;
        for _ in 0..8 {
            if output_reservation == 0 {
                break;
            }
            if cancellation.is_cancelled() {
                return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
            }
            let request_bound =
                provider_request_builder_bound(task, &state.history, &profile.tools)?;
            if crate::bounded_output::active_remaining().is_some_and(|value| request_bound > value)
                || !reserve_active(request_bound)
            {
                return Err(g208("builder_bytes", profile.limits.max_builder_bytes));
            }
            request = render_provider_request(
                &state.run_id,
                turn,
                model,
                output_reservation,
                &profile.tools,
                task,
                &state.history,
            );
            if request.len() as u64 > profile.limits.max_provider_request_bytes {
                break;
            }
            validate_provider_request(&request)?;
            tokens = host
                .tokenize(&model.tokenizer_id, &request)
                .ok_or_else(|| {
                    operational(
                        "SPX-I218",
                        "Agent Runtime provider adapter failed: usage invalid",
                    )
                })?;
            let next = profile
                .limits
                .max_reported_model_output_tokens
                .min(remaining_output_tokens)
                .min(model.max_context_tokens.saturating_sub(tokens));
            // The cap only ever shrinks, so a two-cycle cannot occur, and the
            // request/token pair is accepted only when it was rendered and
            // counted with the exact cap that is reserved and priced.
            if next >= output_reservation {
                fitted = true;
                break;
            }
            output_reservation = next;
        }
        if !fitted
            || request.len() as u64 > profile.limits.max_provider_request_bytes
            || output_reservation == 0
            || tokens
                .checked_add(output_reservation)
                .is_none_or(|total| total > model.max_context_tokens)
        {
            continue;
        }
        let cost = price(tokens, model.input_price)?
            .checked_add(price(output_reservation, model.output_price)?)
            .ok_or_else(|| g208("usd_microunits", profile.limits.max_usd_microunits))?;
        if state
            .usage
            .usd_microunits
            .checked_add(cost)
            .is_none_or(|total| total > profile.limits.max_usd_microunits)
        {
            continue;
        }
        let candidate = (
            cost,
            model.provider_id.clone(),
            model.model_id.clone(),
            index,
            request,
            tokens,
            output_reservation,
        );
        if best.as_ref().is_none_or(|current| {
            (&candidate.0, &candidate.1, &candidate.2) < (&current.0, &current.1, &current.2)
        }) {
            best = Some(candidate);
        }
    }
    let Some((reserved_cost, _, _, model_index, request, input_tokens, output_token_reservation)) =
        best
    else {
        return Err(g206());
    };
    Ok(Route {
        model_index,
        request_digest: digest(REQUEST_DOMAIN, request.as_bytes()),
        request,
        input_tokens,
        output_token_reservation,
        reserved_cost,
    })
}

pub(super) fn authorize_tool(
    profile: &Profile,
    tool_id: &str,
    arguments: &Value,
) -> Result<(), Diagnostic> {
    let tool = profile
        .tools
        .iter()
        .find(|tool| tool.tool_id == tool_id)
        .ok_or_else(|| g207("unknown tool"))?;
    if profile
        .policy
        .allowed_tool_ids
        .binary_search(&tool.tool_id)
        .is_err()
    {
        return Err(g207("tool not allowed"));
    }
    if !tool.required_capabilities.iter().all(|capability| {
        profile
            .policy
            .granted_capabilities
            .binary_search(capability)
            .is_ok()
    }) {
        return Err(g207("required capability missing"));
    }
    validate_schema(arguments, &tool.arguments_schema, MAX_TOOL_ARGUMENT_BYTES)
        .map_err(|_| g207("arguments schema mismatch"))?;
    Ok(())
}

fn price(tokens: u64, per_million: u64) -> Result<u64, Diagnostic> {
    tokens
        .checked_mul(per_million)
        .and_then(|value| value.checked_add(999_999))
        .map(|value| value / 1_000_000)
        .ok_or_else(|| g208("usd_microunits", MAX_USD_MICROUNITS))
}

fn render_provider_request(
    run_id: &str,
    turn: u64,
    model: &Model,
    max_output_tokens: u64,
    tools: &[Tool],
    task: &Task,
    history: &[(String, Option<String>)],
) -> String {
    let mut output = format!("{{\"schema\":\"{PROVIDER_REQUEST_SCHEMA}\",\"run_id\":{},\"turn\":{},\"provider_id\":{},\"model_id\":{},\"max_output_tokens\":{},\"segments\":[", quote_json(run_id), turn, quote_json(&model.provider_id), quote_json(&model.model_id), max_output_tokens);
    let mut segments = vec![
        (
            "system",
            "runtime_trusted",
            "Return exactly one canonical Agent Runtime action.",
        ),
        ("objective", "caller_trusted", task.objective.as_str()),
    ];
    for item in &task.context {
        segments.push(("context", item.provenance.text(), item.content.as_str()));
    }
    for (action, result) in history {
        segments.push(("history", "provider_untrusted", action));
        if let Some(result) = result {
            segments.push(("history", "tool_untrusted", result));
        }
    }
    for (index, (role, provenance, content)) in segments.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"role\":{},\"provenance\":{},\"content\":{}}}",
            quote_json(role),
            quote_json(provenance),
            quote_json(content)
        ));
    }
    output.push_str("],\"tools\":[");
    for (index, tool) in tools.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&render_tool(tool));
    }
    output.push_str("]}\n");
    output
}

fn provider_request_builder_bound(
    task: &Task,
    history: &[(String, Option<String>)],
    tools: &[Tool],
) -> Result<usize, Diagnostic> {
    let content = task
        .objective
        .len()
        .checked_add(
            task.context
                .iter()
                .try_fold(0usize, |sum, item| {
                    sum.checked_add(item.label.len())?
                        .checked_add(item.content.len())
                })
                .ok_or_else(|| g208("builder_bytes", MAX_BUILDER_BYTES as u64))?,
        )
        .and_then(|value| {
            history.iter().try_fold(value, |sum, (action, result)| {
                sum.checked_add(action.len())?
                    .checked_add(result.as_ref().map_or(0, String::len))
            })
        })
        .and_then(|value| {
            tools.iter().try_fold(value, |sum, tool| {
                sum.checked_add(tool.tool_id.len())?
                    .checked_add(tool.description.len())?
                    .checked_add(
                        tool.arguments_schema
                            .fields
                            .iter()
                            .chain(&tool.result_schema.fields)
                            .try_fold(0usize, |fields, field| {
                                fields.checked_add(field.name.len())?.checked_add(96)
                            })?,
                    )
                    .and_then(|sum| {
                        tool.required_capabilities
                            .iter()
                            .try_fold(sum, |caps, cap| caps.checked_add(cap.len() + 3))
                    })
            })
        })
        .ok_or_else(|| g208("builder_bytes", MAX_BUILDER_BYTES as u64))?;
    content
        .checked_mul(6)
        .and_then(|value| value.checked_add(8192))
        .and_then(|value| value.checked_mul(2))
        .ok_or_else(|| g208("builder_bytes", MAX_BUILDER_BYTES as u64))
}

fn validate_provider_request(source: &str) -> Result<(), Diagnostic> {
    let value = canonical_document(
        source,
        "provider request",
        PROVIDER_REQUEST_SCHEMA,
        MAX_PROVIDER_REQUEST_BYTES as usize,
    )?;
    let top = object(&value, "provider request", PROVIDER_REQUEST_SCHEMA)?;
    if !exact_keys(
        top,
        &[
            "schema",
            "run_id",
            "turn",
            "provider_id",
            "model_id",
            "max_output_tokens",
            "segments",
            "tools",
        ],
    ) {
        return Err(g204("provider request", PROVIDER_REQUEST_SCHEMA));
    }
    if !canonical_identifier(string_member(
        top,
        "provider_id",
        "provider request",
        PROVIDER_REQUEST_SCHEMA,
    )?) || !canonical_identifier(string_member(
        top,
        "model_id",
        "provider request",
        PROVIDER_REQUEST_SCHEMA,
    )?) || u64_member(top, "turn", "provider request", PROVIDER_REQUEST_SCHEMA)? == 0
    {
        return Err(g204("provider request", PROVIDER_REQUEST_SCHEMA));
    }
    for segment in top["segments"]
        .as_array()
        .ok_or_else(|| g204("provider request", PROVIDER_REQUEST_SCHEMA))?
    {
        let row = object(segment, "provider request", PROVIDER_REQUEST_SCHEMA)?;
        if !exact_keys(row, &["role", "provenance", "content"]) {
            return Err(g204("provider request", PROVIDER_REQUEST_SCHEMA));
        }
        if !matches!(
            string_member(row, "role", "provider request", PROVIDER_REQUEST_SCHEMA)?,
            "system" | "objective" | "context" | "history"
        ) || !matches!(
            string_member(
                row,
                "provenance",
                "provider request",
                PROVIDER_REQUEST_SCHEMA
            )?,
            "runtime_trusted"
                | "caller_trusted"
                | "caller_untrusted"
                | "retrieved_untrusted"
                | "provider_untrusted"
                | "tool_untrusted"
        ) {
            return Err(g204("provider request", PROVIDER_REQUEST_SCHEMA));
        }
    }
    Ok(())
}

fn render_tool(tool: &Tool) -> String {
    format!("{{\"tool_id\":{},\"description\":{},\"arguments_schema\":{},\"result_schema\":{},\"effects\":[\"read\"],\"required_capabilities\":{}}}", quote_json(&tool.tool_id), quote_json(&tool.description), render_schema(&tool.arguments_schema), render_schema(&tool.result_schema), json_string_array(&tool.required_capabilities))
}

pub(super) fn render_tool_result(call_id: &str, tool_id: &str, result: &str) -> String {
    format!("{{\"schema\":\"{TOOL_RESULT_SCHEMA}\",\"call_id\":{},\"tool_id\":{},\"status\":\"ok\",\"result\":{result}}}\n", quote_json(call_id), quote_json(tool_id))
}

fn collect_response(sink: ProviderSink, limits: EffectiveLimits) -> Result<String, Diagnostic> {
    if let Some(rejection) = sink.bounded.rejection {
        return Err(match rejection {
            SinkRejection::Builder => g208("builder_bytes", limits.max_builder_bytes),
            SinkRejection::Chunks => g208("stream_chunks", limits.max_stream_chunks),
            SinkRejection::Bytes => g208(
                "provider_response_bytes",
                limits.max_provider_response_bytes,
            ),
        });
    }
    String::from_utf8(sink.bounded.bytes).map_err(|_| {
        operational(
            "SPX-I218",
            "Agent Runtime provider adapter failed: response invalid",
        )
    })
}
