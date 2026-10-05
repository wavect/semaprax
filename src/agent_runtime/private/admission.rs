//! Runtime v1 document admission: profile, task and action parsing,
//! canonical JSON and closed-schema checks, and builder-budgeted pure profile
//! admission. Nothing here observes a host.

use super::*;

pub(in crate::agent_runtime) fn parse_profile(source: &str) -> Result<Profile, Diagnostic> {
    let value = canonical_document(source, "profile", PROFILE_SCHEMA, MAX_PROFILE_BYTES)?;
    let top = object(&value, "profile", PROFILE_SCHEMA)?;
    if !exact_keys(
        top,
        &[
            "schema",
            "agent_id",
            "models",
            "tools",
            "policy",
            "limits",
            "nonclaims",
        ],
    ) {
        return Err(g204("profile", PROFILE_SCHEMA));
    }
    let agent_id = string_member(top, "agent_id", "profile", PROFILE_SCHEMA)?.to_owned();
    if !canonical_identifier(&agent_id) {
        return Err(g205("agent_id"));
    }
    let model_values = top
        .get("models")
        .and_then(Value::as_array)
        .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?;
    if model_values.is_empty() || model_values.len() > MAX_MODELS {
        return Err(g205("models"));
    }
    let mut models = Vec::with_capacity(model_values.len());
    for value in model_values {
        let row = object(value, "profile", PROFILE_SCHEMA)?;
        if !exact_keys(
            row,
            &[
                "provider_id",
                "model_id",
                "locality",
                "quality_tier",
                "tokenizer_id",
                "max_context_tokens",
                "input_usd_microunits_per_million_tokens",
                "output_usd_microunits_per_million_tokens",
                "capabilities",
            ],
        ) {
            return Err(g204("profile", PROFILE_SCHEMA));
        }
        let provider_id = string_member(row, "provider_id", "profile", PROFILE_SCHEMA)?.to_owned();
        let model_id = string_member(row, "model_id", "profile", PROFILE_SCHEMA)?.to_owned();
        let tokenizer_id =
            string_member(row, "tokenizer_id", "profile", PROFILE_SCHEMA)?.to_owned();
        if !canonical_identifier(&provider_id)
            || !canonical_identifier(&model_id)
            || !canonical_identifier(&tokenizer_id)
        {
            return Err(g205("models.identifiers"));
        }
        let locality = match string_member(row, "locality", "profile", PROFILE_SCHEMA)? {
            "local" => Locality::Local,
            "remote" => Locality::Remote,
            _ => return Err(g204("profile", PROFILE_SCHEMA)),
        };
        let quality_tier = parse_quality(string_member(
            row,
            "quality_tier",
            "profile",
            PROFILE_SCHEMA,
        )?)?;
        let capabilities = string_array_member(row, "capabilities", "profile", PROFILE_SCHEMA)?;
        if capabilities.len() > MAX_CAPABILITIES
            || !sorted_unique(&capabilities)
            || capabilities
                .iter()
                .any(|value| !canonical_identifier(value))
        {
            return Err(g205("models.capabilities"));
        }
        let max_context_tokens = u64_member(row, "max_context_tokens", "profile", PROFILE_SCHEMA)?;
        if max_context_tokens == 0 {
            return Err(g205("models.max_context_tokens"));
        }
        models.push(Model {
            provider_id,
            model_id,
            locality,
            quality_tier,
            tokenizer_id,
            max_context_tokens,
            input_price: u64_member(
                row,
                "input_usd_microunits_per_million_tokens",
                "profile",
                PROFILE_SCHEMA,
            )?,
            output_price: u64_member(
                row,
                "output_usd_microunits_per_million_tokens",
                "profile",
                PROFILE_SCHEMA,
            )?,
            capabilities,
        });
    }
    if !models.windows(2).all(|pair| {
        (&pair[0].provider_id, &pair[0].model_id) < (&pair[1].provider_id, &pair[1].model_id)
    }) {
        return Err(g205("models"));
    }

    let tool_values = top
        .get("tools")
        .and_then(Value::as_array)
        .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?;
    if tool_values.len() > MAX_TOOLS {
        return Err(g205("tools"));
    }
    let mut tools = Vec::with_capacity(tool_values.len());
    for value in tool_values {
        let row = object(value, "profile", PROFILE_SCHEMA)?;
        if !exact_keys(
            row,
            &[
                "tool_id",
                "description",
                "arguments_schema",
                "result_schema",
                "effects",
                "required_capabilities",
            ],
        ) {
            return Err(g204("profile", PROFILE_SCHEMA));
        }
        let tool_id = string_member(row, "tool_id", "profile", PROFILE_SCHEMA)?.to_owned();
        let description = string_member(row, "description", "profile", PROFILE_SCHEMA)?.to_owned();
        if !canonical_identifier(&tool_id)
            || description.is_empty()
            || description.len() > MAX_DESCRIPTION_BYTES
        {
            return Err(g205("tools.identifiers"));
        }
        if string_array_member(row, "effects", "profile", PROFILE_SCHEMA)? != ["read"] {
            return Err(g205("tools.effects"));
        }
        let required_capabilities =
            string_array_member(row, "required_capabilities", "profile", PROFILE_SCHEMA)?;
        if required_capabilities.len() > MAX_CAPABILITIES
            || !sorted_unique(&required_capabilities)
            || required_capabilities
                .iter()
                .any(|value| !canonical_identifier(value))
        {
            return Err(g205("tools.required_capabilities"));
        }
        tools.push(Tool {
            tool_id,
            description,
            arguments_schema: parse_schema(&row["arguments_schema"])?,
            result_schema: parse_schema(&row["result_schema"])?,
            required_capabilities,
        });
    }
    if !tools
        .windows(2)
        .all(|pair| pair[0].tool_id < pair[1].tool_id)
    {
        return Err(g205("tools"));
    }

    let policy_value = object(
        top.get("policy")
            .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?,
        "profile",
        PROFILE_SCHEMA,
    )?;
    if !exact_keys(
        policy_value,
        &[
            "allowed_provider_ids",
            "allowed_model_ids",
            "required_locality",
            "minimum_quality_tier",
            "required_model_capabilities",
            "granted_capabilities",
            "allowed_tool_ids",
        ],
    ) {
        return Err(g204("profile", PROFILE_SCHEMA));
    }
    let policy = Policy {
        allowed_provider_ids: validated_policy_list(policy_value, "allowed_provider_ids")?,
        allowed_model_ids: validated_policy_list(policy_value, "allowed_model_ids")?,
        required_locality: match string_member(
            policy_value,
            "required_locality",
            "profile",
            PROFILE_SCHEMA,
        )? {
            "local_only" => RequiredLocality::LocalOnly,
            "remote_allowed" => RequiredLocality::RemoteAllowed,
            _ => return Err(g204("profile", PROFILE_SCHEMA)),
        },
        minimum_quality_tier: parse_quality(string_member(
            policy_value,
            "minimum_quality_tier",
            "profile",
            PROFILE_SCHEMA,
        )?)?,
        required_model_capabilities: validated_policy_list(
            policy_value,
            "required_model_capabilities",
        )?,
        granted_capabilities: validated_policy_list(policy_value, "granted_capabilities")?,
        allowed_tool_ids: validated_policy_list(policy_value, "allowed_tool_ids")?,
    };
    if !policy
        .allowed_provider_ids
        .iter()
        .all(|id| models.iter().any(|model| &model.provider_id == id))
        || !policy
            .allowed_model_ids
            .iter()
            .all(|id| models.iter().any(|model| &model.model_id == id))
        || !policy
            .allowed_tool_ids
            .iter()
            .all(|id| tools.iter().any(|tool| &tool.tool_id == id))
    {
        return Err(g205("policy"));
    }
    let limits = parse_effective_limits(
        top.get("limits")
            .ok_or_else(|| g204("profile", PROFILE_SCHEMA))?,
    )?;
    let nonclaims = string_array_member(top, "nonclaims", "profile", PROFILE_SCHEMA)?;
    if nonclaims != NONCLAIMS {
        return Err(g205("nonclaims"));
    }
    let mut profile = Profile {
        agent_id,
        models,
        tools,
        policy,
        limits,
        source: source.to_owned(),
        digest: digest(PROFILE_DOMAIN, source.as_bytes()),
    };
    if render_profile(&profile) != source {
        return Err(g204("profile", PROFILE_SCHEMA));
    }
    profile.source.shrink_to_fit();
    Ok(profile)
}

fn parse_quality(value: &str) -> Result<QualityTier, Diagnostic> {
    match value {
        "basic" => Ok(QualityTier::Basic),
        "standard" => Ok(QualityTier::Standard),
        "advanced" => Ok(QualityTier::Advanced),
        "frontier" => Ok(QualityTier::Frontier),
        _ => Err(g204("profile", PROFILE_SCHEMA)),
    }
}

fn validated_policy_list(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Vec<String>, Diagnostic> {
    let values = string_array_member(object, key, "profile", PROFILE_SCHEMA)?;
    if values.len() > MAX_CAPABILITIES
        || !sorted_unique(&values)
        || values
            .iter()
            .any(|value| !canonical_identifier(value) || value == "*")
    {
        return Err(g205(&format!("policy.{key}")));
    }
    Ok(values)
}

fn parse_effective_limits(value: &Value) -> Result<EffectiveLimits, Diagnostic> {
    let object = object(value, "profile", PROFILE_SCHEMA)?;
    let keys = [
        "max_turns",
        "max_provider_attempts",
        "max_retries_per_turn",
        "max_concurrency",
        "max_elapsed_ms",
        "max_provider_request_bytes",
        "max_provider_response_bytes",
        "max_stream_chunks",
        "max_total_provider_input_bytes",
        "max_total_provider_output_bytes",
        "max_reported_model_input_tokens",
        "max_reported_model_output_tokens",
        "max_usd_microunits",
        "max_tool_calls",
        "max_tool_arguments_bytes",
        "max_tool_result_bytes",
        "max_total_tool_bytes",
        "max_retained_state_bytes",
        "max_trace_events",
        "max_trace_bytes",
        "max_evidence_bytes",
        "max_builder_bytes",
    ];
    if !exact_keys(object, &keys) {
        return Err(g204("profile", PROFILE_SCHEMA));
    }
    let limits = EffectiveLimits {
        max_turns: u64_member(object, keys[0], "profile", PROFILE_SCHEMA)?,
        max_provider_attempts: u64_member(object, keys[1], "profile", PROFILE_SCHEMA)?,
        max_retries_per_turn: u64_member(object, keys[2], "profile", PROFILE_SCHEMA)?,
        max_concurrency: u64_member(object, keys[3], "profile", PROFILE_SCHEMA)?,
        max_elapsed_ms: u64_member(object, keys[4], "profile", PROFILE_SCHEMA)?,
        max_provider_request_bytes: u64_member(object, keys[5], "profile", PROFILE_SCHEMA)?,
        max_provider_response_bytes: u64_member(object, keys[6], "profile", PROFILE_SCHEMA)?,
        max_stream_chunks: u64_member(object, keys[7], "profile", PROFILE_SCHEMA)?,
        max_total_provider_input_bytes: u64_member(object, keys[8], "profile", PROFILE_SCHEMA)?,
        max_total_provider_output_bytes: u64_member(object, keys[9], "profile", PROFILE_SCHEMA)?,
        max_reported_model_input_tokens: u64_member(object, keys[10], "profile", PROFILE_SCHEMA)?,
        max_reported_model_output_tokens: u64_member(object, keys[11], "profile", PROFILE_SCHEMA)?,
        max_usd_microunits: u64_member(object, keys[12], "profile", PROFILE_SCHEMA)?,
        max_tool_calls: u64_member(object, keys[13], "profile", PROFILE_SCHEMA)?,
        max_tool_arguments_bytes: u64_member(object, keys[14], "profile", PROFILE_SCHEMA)?,
        max_tool_result_bytes: u64_member(object, keys[15], "profile", PROFILE_SCHEMA)?,
        max_total_tool_bytes: u64_member(object, keys[16], "profile", PROFILE_SCHEMA)?,
        max_retained_state_bytes: u64_member(object, keys[17], "profile", PROFILE_SCHEMA)?,
        max_trace_events: u64_member(object, keys[18], "profile", PROFILE_SCHEMA)?,
        max_trace_bytes: u64_member(object, keys[19], "profile", PROFILE_SCHEMA)?,
        max_evidence_bytes: u64_member(object, keys[20], "profile", PROFILE_SCHEMA)?,
        max_builder_bytes: u64_member(object, keys[21], "profile", PROFILE_SCHEMA)?,
    };
    let bounded = [
        ("max_turns", limits.max_turns, MAX_TURNS),
        (
            "max_provider_attempts",
            limits.max_provider_attempts,
            MAX_PROVIDER_ATTEMPTS,
        ),
        (
            "max_retries_per_turn",
            limits.max_retries_per_turn,
            MAX_RETRIES_PER_TURN,
        ),
        ("max_concurrency", limits.max_concurrency, MAX_CONCURRENCY),
        ("max_elapsed_ms", limits.max_elapsed_ms, MAX_ELAPSED_MS),
        (
            "max_provider_request_bytes",
            limits.max_provider_request_bytes,
            MAX_PROVIDER_REQUEST_BYTES,
        ),
        (
            "max_provider_response_bytes",
            limits.max_provider_response_bytes,
            MAX_PROVIDER_RESPONSE_BYTES,
        ),
        (
            "max_stream_chunks",
            limits.max_stream_chunks,
            MAX_STREAM_CHUNKS,
        ),
        (
            "max_total_provider_input_bytes",
            limits.max_total_provider_input_bytes,
            MAX_TOTAL_PROVIDER_INPUT_BYTES,
        ),
        (
            "max_total_provider_output_bytes",
            limits.max_total_provider_output_bytes,
            MAX_TOTAL_PROVIDER_OUTPUT_BYTES,
        ),
        (
            "max_reported_model_input_tokens",
            limits.max_reported_model_input_tokens,
            MAX_REPORTED_MODEL_INPUT_TOKENS,
        ),
        (
            "max_reported_model_output_tokens",
            limits.max_reported_model_output_tokens,
            MAX_REPORTED_MODEL_OUTPUT_TOKENS,
        ),
        (
            "max_usd_microunits",
            limits.max_usd_microunits,
            MAX_USD_MICROUNITS,
        ),
        ("max_tool_calls", limits.max_tool_calls, MAX_TOOL_CALLS),
        (
            "max_tool_arguments_bytes",
            limits.max_tool_arguments_bytes,
            MAX_TOOL_ARGUMENT_BYTES,
        ),
        (
            "max_tool_result_bytes",
            limits.max_tool_result_bytes,
            MAX_TOOL_RESULT_BYTES,
        ),
        (
            "max_total_tool_bytes",
            limits.max_total_tool_bytes,
            MAX_TOTAL_TOOL_BYTES,
        ),
        (
            "max_retained_state_bytes",
            limits.max_retained_state_bytes,
            MAX_RETAINED_STATE_BYTES,
        ),
        (
            "max_trace_events",
            limits.max_trace_events,
            MAX_TRACE_EVENTS,
        ),
        ("max_trace_bytes", limits.max_trace_bytes, MAX_TRACE_BYTES),
        (
            "max_evidence_bytes",
            limits.max_evidence_bytes,
            MAX_EVIDENCE_BYTES,
        ),
        (
            "max_builder_bytes",
            limits.max_builder_bytes,
            MAX_BUILDER_BYTES as u64,
        ),
    ];
    for (field, used, maximum) in bounded {
        if used > maximum {
            return Err(g208(field, maximum));
        }
    }
    if limits.max_concurrency != 1 {
        return Err(g205("limits.max_concurrency"));
    }
    for (field, used) in [
        ("max_turns", limits.max_turns),
        ("max_provider_attempts", limits.max_provider_attempts),
        ("max_elapsed_ms", limits.max_elapsed_ms),
        (
            "max_provider_request_bytes",
            limits.max_provider_request_bytes,
        ),
        (
            "max_provider_response_bytes",
            limits.max_provider_response_bytes,
        ),
        ("max_stream_chunks", limits.max_stream_chunks),
        ("max_retained_state_bytes", limits.max_retained_state_bytes),
        ("max_trace_events", limits.max_trace_events),
        ("max_trace_bytes", limits.max_trace_bytes),
        ("max_evidence_bytes", limits.max_evidence_bytes),
        ("max_builder_bytes", limits.max_builder_bytes),
    ] {
        if used == 0 {
            return Err(g205(&format!("limits.{field}")));
        }
    }
    Ok(limits)
}

pub(in crate::agent_runtime) fn parse_task(source: &str) -> Result<Task, Diagnostic> {
    let value = canonical_document(source, "task", TASK_SCHEMA, MAX_TASK_BYTES)?;
    let top = object(&value, "task", TASK_SCHEMA)?;
    if !exact_keys(top, &["schema", "nonce", "objective", "context"]) {
        return Err(g204("task", TASK_SCHEMA));
    }
    let nonce = string_member(top, "nonce", "task", TASK_SCHEMA)?.to_owned();
    if decode_hex_32(&nonce).is_none() {
        return Err(g204("task", TASK_SCHEMA));
    }
    let objective = string_member(top, "objective", "task", TASK_SCHEMA)?.to_owned();
    let values = top
        .get("context")
        .and_then(Value::as_array)
        .ok_or_else(|| g204("task", TASK_SCHEMA))?;
    let mut context = Vec::with_capacity(values.len());
    for value in values {
        let row = object(value, "task", TASK_SCHEMA)?;
        if !exact_keys(row, &["label", "provenance", "content"]) {
            return Err(g204("task", TASK_SCHEMA));
        }
        let label = string_member(row, "label", "task", TASK_SCHEMA)?.to_owned();
        if !canonical_identifier(&label) {
            return Err(g204("task", TASK_SCHEMA));
        }
        let provenance = match string_member(row, "provenance", "task", TASK_SCHEMA)? {
            "caller_trusted" => Provenance::CallerTrusted,
            "caller_untrusted" => Provenance::CallerUntrusted,
            "retrieved_untrusted" => Provenance::RetrievedUntrusted,
            _ => return Err(g204("task", TASK_SCHEMA)),
        };
        context.push(ContextItem {
            label,
            provenance,
            content: string_member(row, "content", "task", TASK_SCHEMA)?.to_owned(),
        });
    }
    if !context.windows(2).all(|pair| pair[0].label < pair[1].label) {
        return Err(g204("task", TASK_SCHEMA));
    }
    let mut task = Task {
        nonce,
        objective,
        context,
        source: source.to_owned(),
        digest: digest(TASK_DOMAIN, source.as_bytes()),
    };
    if render_task(&task) != source {
        return Err(g204("task", TASK_SCHEMA));
    }
    task.source.shrink_to_fit();
    Ok(task)
}

pub(in crate::agent_runtime) fn render_task(task: &Task) -> String {
    let mut output = format!(
        "{{\"schema\":\"{TASK_SCHEMA}\",\"nonce\":{},\"objective\":{},\"context\":[",
        quote_json(&task.nonce),
        quote_json(&task.objective)
    );
    for (index, item) in task.context.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!(
            "{{\"label\":{},\"provenance\":{},\"content\":{}}}",
            quote_json(&item.label),
            quote_json(item.provenance.text()),
            quote_json(&item.content)
        ));
    }
    output.push_str("]}\n");
    output
}

pub(super) fn parse_action(source: String, maximum: usize) -> Result<Action, Diagnostic> {
    let value =
        canonical_document(&source, "action", ACTION_SCHEMA, maximum).map_err(|diagnostic| {
            if crate::bounded_output::active_remaining() == Some(0) {
                g208("builder_bytes", MAX_BUILDER_BYTES as u64)
            } else {
                diagnostic
            }
        })?;
    let Value::Object(mut top) = value else {
        return Err(g204("action", ACTION_SCHEMA));
    };
    let kind = string_member(&top, "kind", "action", ACTION_SCHEMA)?.to_owned();
    let action = match kind.as_str() {
        "final" if exact_keys(&top, &["schema", "kind", "message"]) => Action::Final {
            message: string_member(&top, "message", "action", ACTION_SCHEMA)?.to_owned(),
            source,
        },
        "tool" if exact_keys(&top, &["schema", "kind", "tool_id", "arguments"]) => Action::Tool {
            tool_id: string_member(&top, "tool_id", "action", ACTION_SCHEMA)?.to_owned(),
            arguments: top
                .remove("arguments")
                .ok_or_else(|| g204("action", ACTION_SCHEMA))?,
            source,
        },
        _ => return Err(g204("action", ACTION_SCHEMA)),
    };
    let original = match &action {
        Action::Final { source, .. } | Action::Tool { source, .. } => source,
    };
    if render_action(&action)? != *original {
        return Err(g204("action", ACTION_SCHEMA));
    }
    Ok(action)
}

pub(super) fn reserve_builder_copy(bytes: usize, multiplier: usize) -> Result<(), Diagnostic> {
    let bound = bytes
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(256))
        .ok_or_else(|| g208("builder_bytes", MAX_BUILDER_BYTES as u64))?;
    if crate::bounded_output::active_remaining().is_some_and(|remaining| bound > remaining) {
        return Err(g208("builder_bytes", MAX_BUILDER_BYTES as u64));
    }
    if reserve_active(bound) {
        Ok(())
    } else {
        Err(g208("builder_bytes", MAX_BUILDER_BYTES as u64))
    }
}

fn render_action(action: &Action) -> Result<String, Diagnostic> {
    match action {
        Action::Final { message, .. } => Ok(format!("{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"final\",\"message\":{}}}\n", quote_json(message))),
        Action::Tool { tool_id, arguments, .. } => Ok(format!("{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"tool\",\"tool_id\":{},\"arguments\":{}}}\n", quote_json(tool_id), canonical_json(arguments)?)),
    }
}

fn canonical_json(value: &Value) -> Result<String, Diagnostic> {
    match value {
        Value::Null => Ok("null".to_owned()),
        Value::Bool(value) => Ok(value.to_string()),
        Value::Number(value) => Ok(value.to_string()),
        Value::String(value) => Ok(quote_json(value)),
        Value::Array(values) => {
            let mut output = String::from("[");
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&canonical_json(value)?);
            }
            output.push(']');
            Ok(output)
        }
        Value::Object(values) => {
            let mut output = String::from("{");
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(&quote_json(key));
                output.push(':');
                output.push_str(&canonical_json(value)?);
            }
            output.push('}');
            Ok(output)
        }
    }
}

pub(super) fn validate_schema(
    value: &Value,
    schema: &ClosedSchema,
    maximum: u64,
) -> Result<String, ()> {
    let object = value.as_object().ok_or(())?;
    if object
        .keys()
        .any(|key| !schema.fields.iter().any(|field| &field.name == key))
    {
        return Err(());
    }
    let mut output = String::from("{");
    for (index, field) in schema.fields.iter().enumerate() {
        let value = object.get(&field.name);
        if value.is_none() && field.required {
            return Err(());
        }
        let Some(value) = value else {
            continue;
        };
        if index > 0 && output.len() > 1 {
            output.push(',');
        }
        output.push_str(&quote_json(&field.name));
        output.push(':');
        let rendered = match (field.kind, value) {
            (ScalarKind::String, Value::String(value)) if value.len() as u64 <= field.max_bytes => {
                quote_json(value)
            }
            (ScalarKind::Integer, Value::Number(value)) if value.as_i64().is_some() => {
                value.to_string()
            }
            (ScalarKind::Boolean, Value::Bool(value)) => value.to_string(),
            _ => return Err(()),
        };
        if rendered.len() as u64 > field.max_bytes.saturating_add(2) {
            return Err(());
        }
        output.push_str(&rendered);
    }
    output.push('}');
    if output.len() as u64 > maximum {
        return Err(());
    }
    Ok(output)
}

/// One Runtime v1 profile admitted without a host, cancellation or Agent.
///
/// The fields stay private to the runtime's `private` module tree, so the
/// admitted representation is only ever attached to an execution host by
/// `Agent::new`.
pub(in crate::agent_runtime) struct AdmittedProfile {
    pub(super) profile: Profile,
    pub(super) builder_bytes: u64,
}

#[cfg(test)]
impl AdmittedProfile {
    pub(in crate::agent_runtime) fn builder_bytes(&self) -> u64 {
        self.builder_bytes
    }
}

/// Pure profile admission shared by `Agent::new` and AgentDefinition
/// validation: builder-budgeted parsing and every profile invariant, with no
/// host observation and no cancellation handle.
pub(in crate::agent_runtime) fn admit_profile(
    profile_source: &str,
) -> Result<AdmittedProfile, Vec<Diagnostic>> {
    let (profile, overflowed, used) = with_limit_usage(MAX_BUILDER_BYTES, || {
        reserve_parse_bound(profile_source)?;
        parse_profile(profile_source)
    });
    if overflowed {
        return Err(vec![g208("builder_bytes", MAX_BUILDER_BYTES as u64)]);
    }
    Ok(AdmittedProfile {
        profile: profile.map_err(|diagnostic| vec![diagnostic])?,
        builder_bytes: used as u64,
    })
}

pub(super) fn reserve_parse_bound(source: &str) -> Result<(), Diagnostic> {
    let bound = source
        .len()
        .checked_mul(8)
        .and_then(|value| value.checked_add(4096))
        .ok_or_else(|| g208("builder_bytes", MAX_BUILDER_BYTES as u64))?;
    if !reserve_active(bound) {
        return Err(g208("builder_bytes", MAX_BUILDER_BYTES as u64));
    }
    Ok(())
}
