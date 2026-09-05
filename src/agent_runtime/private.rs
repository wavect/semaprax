use super::*;
use std::fmt;
use std::fmt::Write as _;

#[cfg(test)]
mod economic_tests;

#[derive(Clone, Default, Eq, PartialEq)]
struct EvidenceBudget {
    used_models: u64,
    used_tools: u64,
    used_capabilities: u64,
    used_turns: u64,
    used_provider_attempts: u64,
    used_provider_input_bytes: u64,
    used_provider_output_bytes: u64,
    used_reported_model_input_tokens: u64,
    used_reported_model_output_tokens: u64,
    used_usd_microunits: u64,
    used_tool_calls: u64,
    used_tool_argument_bytes: u64,
    used_tool_result_bytes: u64,
    used_retained_state_bytes: u64,
    used_trace_events: u64,
    used_trace_bytes: u64,
    used_evidence_bytes: u64,
    used_builder_bytes: u64,
    used_elapsed_ms: u64,
    used_concurrency: u64,
}

#[derive(Clone)]
struct RunState {
    run_id: String,
    events: Vec<TraceEvent>,
    usage: Usage,
    history: Vec<(String, Option<String>)>,
    final_message: Option<String>,
    last_turn: u64,
    termination: Termination,
    task_digest: String,
    task_bytes: u64,
    task_nonce: String,
    external_effect_crossed: bool,
}

pub(super) struct EvidenceReplay {
    state: RunState,
    budget: EvidenceBudget,
}

impl EvidenceReplay {
    pub(super) fn final_message(&self) -> Option<&str> {
        self.state.final_message.as_deref()
    }

    pub(super) fn run_id(&self) -> &str {
        &self.state.run_id
    }
}

struct Route {
    model_index: usize,
    request: String,
    request_digest: String,
    input_tokens: u64,
    output_token_reservation: u64,
    reserved_cost: u64,
}

pub(super) fn parse_profile(source: &str) -> Result<Profile, Diagnostic> {
    canonical_document(source, "profile", PROFILE_SCHEMA, MAX_PROFILE_BYTES)?;
    let value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g204("profile", PROFILE_SCHEMA))?;
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

pub(super) fn parse_task(source: &str) -> Result<Task, Diagnostic> {
    canonical_document(source, "task", TASK_SCHEMA, MAX_TASK_BYTES)?;
    let value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g204("task", TASK_SCHEMA))?;
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

pub(super) fn render_task(task: &Task) -> String {
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

fn parse_action(source: String, maximum: usize) -> Result<Action, Diagnostic> {
    canonical_document(&source, "action", ACTION_SCHEMA, maximum).map_err(|diagnostic| {
        if crate::bounded_output::active_remaining() == Some(0) {
            g208("builder_bytes", MAX_BUILDER_BYTES as u64)
        } else {
            diagnostic
        }
    })?;
    let value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g204("action", ACTION_SCHEMA))?;
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

fn reserve_builder_copy(bytes: usize, multiplier: usize) -> Result<(), Diagnostic> {
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

fn validate_schema(value: &Value, schema: &ClosedSchema, maximum: u64) -> Result<String, ()> {
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

#[cfg(test)]
pub(super) struct TestAgent<H: AgentHost>(Agent<H>);

#[cfg(test)]
impl<H: AgentHost> TestAgent<H> {
    pub(super) fn run(mut self, task: &str) -> Result<AgentRun, Vec<Diagnostic>> {
        self.0.run(task)
    }
}

#[cfg(test)]
pub(super) fn new_agent<H: AgentHost>(profile_source: &str, host: H) -> TestAgent<H> {
    TestAgent(Agent::new(profile_source, host, AgentCancellation::new()).unwrap())
}

#[cfg(test)]
pub(crate) fn completed_run_for_economic_test(message: &str) -> AgentRun {
    struct Probe;
    impl AgentBoundaryProbe for Probe {
        fn policy_epoch(&self) -> u64 {
            1
        }
        fn elapsed_ms(&self) -> u64 {
            0
        }
    }
    struct Host {
        response: Vec<u8>,
    }
    impl AgentHost for Host {
        fn policy_epoch(&self) -> u64 {
            1
        }
        fn elapsed_ms(&self) -> u64 {
            0
        }
        fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
            Box::new(Probe)
        }
        fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
            Some(request.len() as u64)
        }
        fn attempt_provider(
            &mut self,
            _: &str,
            _: &str,
            request: &str,
            _: u64,
            sink: &mut AgentProviderSink,
        ) -> AgentProviderAttempt {
            assert!(sink.push(&self.response));
            AgentProviderAttempt::new(
                AgentProviderDisposition::Succeeded,
                AgentProviderUsage::new(request.len() as u64, self.response.len() as u64, 0),
            )
        }
        fn invoke_tool(&mut self, _: &str, _: &str, _: &str, _: &mut AgentToolResultSink) -> bool {
            false
        }
    }
    let profile = Profile {
        agent_id: "economic.fixture.agent".to_owned(),
        models: vec![Model {
            provider_id: "fixture.local".to_owned(),
            model_id: "fixture-economic".to_owned(),
            locality: Locality::Local,
            quality_tier: QualityTier::Basic,
            tokenizer_id: "fixture.bytes-v1".to_owned(),
            max_context_tokens: 1_048_576,
            input_price: 0,
            output_price: 0,
            capabilities: vec!["text".to_owned()],
        }],
        tools: vec![],
        policy: Policy {
            allowed_provider_ids: vec!["fixture.local".to_owned()],
            allowed_model_ids: vec!["fixture-economic".to_owned()],
            required_locality: RequiredLocality::LocalOnly,
            minimum_quality_tier: QualityTier::Basic,
            required_model_capabilities: vec!["text".to_owned()],
            granted_capabilities: vec![],
            allowed_tool_ids: vec![],
        },
        limits: EffectiveLimits {
            max_turns: 1,
            max_provider_attempts: 1,
            max_retries_per_turn: 0,
            max_concurrency: 1,
            max_elapsed_ms: 10_000,
            max_provider_request_bytes: 2_097_152,
            max_provider_response_bytes: 1_048_576,
            max_stream_chunks: 4,
            max_total_provider_input_bytes: 2_097_152,
            max_total_provider_output_bytes: 1_048_576,
            max_reported_model_input_tokens: 2_097_152,
            max_reported_model_output_tokens: 262_144,
            max_usd_microunits: 0,
            max_tool_calls: 0,
            max_tool_arguments_bytes: 1,
            max_tool_result_bytes: 1,
            max_total_tool_bytes: 1,
            max_retained_state_bytes: 2_097_152,
            max_trace_events: 32,
            max_trace_bytes: 262_144,
            max_evidence_bytes: 2_097_152,
            max_builder_bytes: 67_108_864,
        },
        source: String::new(),
        digest: String::new(),
    };
    let profile_source = render_profile(&profile);
    let task = Task {
        nonce: "0".repeat(64),
        objective: "Return the exact economic proposal.".to_owned(),
        context: vec![],
        source: String::new(),
        digest: String::new(),
    };
    let task_source = render_task(&task);
    let response = format!(
        "{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"final\",\"message\":{}}}\n",
        quote_json(message)
    )
    .into_bytes();
    economic_tests::run(&profile_source, &task_source, Host { response })
}

impl<H: AgentHost> Agent<H> {
    /// Parses and owns one canonical Agent Runtime Profile before observing the host.
    pub fn new(
        profile_source: &str,
        host: H,
        cancellation: AgentCancellation,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (profile, overflowed, used) = with_limit_usage(MAX_BUILDER_BYTES, || {
            reserve_parse_bound(profile_source)?;
            parse_profile(profile_source)
        });
        if overflowed {
            return Err(vec![g208("builder_bytes", MAX_BUILDER_BYTES as u64)]);
        }
        Ok(Self {
            profile: profile.map_err(|diagnostic| vec![diagnostic])?,
            profile_builder_bytes: used as u64,
            host,
            cancellation,
        })
    }

    /// Runs one canonical task through the bounded injected-host state machine.
    pub fn run(&mut self, task_source: &str) -> Result<AgentRun, Vec<Diagnostic>> {
        let (result, overflowed, _) = with_limit_usage(MAX_BUILDER_BYTES, || {
            if !reserve_active(self.profile_builder_bytes as usize) {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            reserve_parse_bound(task_source)?;
            let task = parse_task(task_source)?;
            let parse_used = MAX_BUILDER_BYTES
                .saturating_sub(crate::bounded_output::active_remaining().unwrap_or(0))
                as u64;
            if parse_used > self.profile.limits.max_builder_bytes {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            let remaining = usize::try_from(self.profile.limits.max_builder_bytes - parse_used)
                .map_err(|_| g208("builder_bytes", self.profile.limits.max_builder_bytes))?;
            let (run, child_overflowed, child_used) = with_limit_usage(remaining, || {
                let admitted_policy_epoch = self.host.policy_epoch();
                let state = run_bounded(
                    &self.profile,
                    &mut self.host,
                    &self.cancellation,
                    admitted_policy_epoch,
                    task,
                )?;
                render_bundle(&self.profile, state, parse_used, remaining as u64)
            });
            let artifact = run?;
            let expected_builder_message = format!(
                "builder_bytes exceeds {}",
                self.profile.limits.max_builder_bytes
            );
            if child_overflowed
                && !(artifact.status == RunStatus::BudgetExhausted
                    && artifact.replay.state.termination.code == Some("SPX-G208")
                    && artifact.replay.state.termination.message.as_deref()
                        == Some(expected_builder_message.as_str()))
            {
                return Err(g208("builder_bytes", self.profile.limits.max_builder_bytes));
            }
            let sealed_builder_overflow = artifact.status == RunStatus::BudgetExhausted
                && artifact.replay.state.termination.code == Some("SPX-G208")
                && artifact.replay.state.termination.message.as_deref()
                    == Some(expected_builder_message.as_str());
            Ok((
                artifact,
                parse_used.saturating_add(child_used as u64),
                self.profile.limits.max_builder_bytes,
                sealed_builder_overflow,
            ))
        });
        let result = result.map_err(|diagnostic| vec![diagnostic])?;
        let outer_builder_message = format!("builder_bytes exceeds {}", result.2);
        if overflowed
            && !result.3
            && !(result.0.status == RunStatus::BudgetExhausted
                && result.0.replay.state.termination.code == Some("SPX-G208")
                && result.0.replay.state.termination.message.as_deref()
                    == Some(outer_builder_message.as_str()))
        {
            return Err(vec![g208("builder_bytes", MAX_BUILDER_BYTES as u64)]);
        }
        if result.1 > result.2
            && !(result.0.status == RunStatus::BudgetExhausted
                && result.0.replay.state.termination.code == Some("SPX-G208")
                && result.0.replay.state.termination.message.as_deref()
                    == Some(outer_builder_message.as_str()))
        {
            return Err(vec![g208("builder_bytes", result.2)]);
        }
        Ok(result.0)
    }
}

fn reserve_parse_bound(source: &str) -> Result<(), Diagnostic> {
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

fn run_bounded<H: AgentHost>(
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

#[derive(Clone, Copy)]
enum ExternalBoundary<'a> {
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

fn preflight_external_capacity(
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

fn preflight_current_terminal(profile: &Profile, state: &mut RunState) -> Result<(), Diagnostic> {
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
pub(super) fn terminal_diagnostics_for_test() -> Vec<(&'static str, &'static str, &'static str)> {
    terminal_diagnostics().collect()
}

#[cfg(test)]
pub(super) fn preflight_terminal_for_test(
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
        if cancellation.is_cancelled() {
            state.events.pop();
            state.usage = previous_usage;
            return Err(operational("SPX-I220", "Agent Runtime run was cancelled"));
        }
        state.external_effect_crossed = true;
        let attempt = host.attempt_provider(
            &model.provider_id,
            &model.model_id,
            &route.request,
            profile.limits.max_elapsed_ms,
            &mut sink,
        );
        if cancellation.is_cancelled() && sink.boundary.is_none() {
            sink.boundary = Some(RunStatus::Cancelled);
        }
        if let Some(boundary) = sink.boundary {
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
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bytes)),
                status,
                UsageDelta {
                    provider_output_bytes: sink.bytes.len() as u64,
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
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bytes)),
                status,
                UsageDelta {
                    provider_output_bytes: sink.bytes.len() as u64,
                    reported_model_output_tokens: attempt.usage.output_tokens,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        let exact_zero = sink.bytes.is_empty()
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
                    Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bytes)),
                    "failed_uncertain",
                    UsageDelta {
                        provider_output_bytes: sink.bytes.len() as u64,
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
                    Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bytes)),
                    "failed_uncertain",
                    UsageDelta {
                        provider_output_bytes: sink.bytes.len() as u64,
                        reported_model_output_tokens: attempt.usage.output_tokens,
                        ..UsageDelta::default()
                    },
                )?;
                return Ok(None);
            }
            ProviderDisposition::Succeeded => {}
        }
        if let Some(rejection) = sink.rejection {
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
                Some(digest(PROVIDER_RESPONSE_DOMAIN, &sink.bytes)),
                "failed_uncertain",
                UsageDelta {
                    provider_output_bytes: sink.bytes.len() as u64,
                    reported_model_output_tokens: attempt.usage.output_tokens,
                    ..UsageDelta::default()
                },
            )?;
            return Ok(None);
        }
        let response_bytes = sink.bytes;
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
    if cancellation.is_cancelled() {
        state.termination =
            termination_from_diagnostic(operational("SPX-I220", "Agent Runtime run was cancelled"));
        return Ok(());
    }
    state.external_effect_crossed = true;
    let invocation = host.invoke_tool(&call_id, &tool_id, &arguments_json, &mut sink);
    if cancellation.is_cancelled() && sink.boundary.is_none() {
        sink.boundary = Some(RunStatus::Cancelled);
    }
    if let Some(boundary) = sink.boundary {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bytes.len() as u64,
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
                tool_result_bytes: sink.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if let Some(rejection) = sink.rejection {
        state.usage.tool_result_bytes = state
            .usage
            .tool_result_bytes
            .checked_add(sink.bytes.len() as u64)
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
                tool_result_bytes: sink.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if !invocation {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bytes.len() as u64,
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
                tool_result_bytes: sink.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
        checked_add(
            &mut state.usage.tool_result_bytes,
            sink.bytes.len() as u64,
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
                tool_result_bytes: sink.bytes.len() as u64,
                ..UsageDelta::default()
            },
        )?;
        return Ok(());
    }
    let received_bytes = sink.bytes.len() as u64;
    let value: Value = match serde_json::from_slice(&sink.bytes) {
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

fn boundary_termination<H: AgentHost>(
    profile: &Profile,
    host: &H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
) -> Option<Termination> {
    if cancellation.is_cancelled() {
        return Some(termination_from_diagnostic(operational(
            "SPX-I220",
            "Agent Runtime run was cancelled",
        )));
    }
    if host.elapsed_ms() > profile.limits.max_elapsed_ms {
        return Some(termination_from_diagnostic(operational(
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        )));
    }
    if host.policy_epoch() != policy_epoch {
        return Some(termination_from_diagnostic(g207("policy revoked")));
    }
    None
}

fn termination_for_status(status: RunStatus) -> Termination {
    match status {
        RunStatus::Cancelled => {
            termination_from_diagnostic(operational("SPX-I220", "Agent Runtime run was cancelled"))
        }
        RunStatus::DeadlineExceeded => termination_from_diagnostic(operational(
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        )),
        RunStatus::PolicyRejected => termination_from_diagnostic(g207("policy revoked")),
        _ => termination_from_diagnostic(g209()),
    }
}

fn route<H: AgentHost>(
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
        for _ in 0..8 {
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
            if next == output_reservation {
                break;
            }
            output_reservation = next;
        }
        if request.len() as u64 > profile.limits.max_provider_request_bytes
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

fn authorize_tool(profile: &Profile, tool_id: &str, arguments: &Value) -> Result<(), Diagnostic> {
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

fn account_uncertain(
    usage: &mut Usage,
    sink: &ProviderSink,
    reported: ProviderUsage,
    route: &Route,
    limits: EffectiveLimits,
) -> Result<(), Diagnostic> {
    if reported.input_tokens > route.input_tokens
        || reported.output_tokens > route.output_token_reservation
        || reported.usd_microunits > route.reserved_cost
    {
        return Err(operational(
            "SPX-I218",
            "Agent Runtime provider adapter failed: usage invalid",
        ));
    }
    checked_add(
        &mut usage.provider_output_bytes,
        sink.bytes.len() as u64,
        "total_provider_output_bytes",
        limits.max_total_provider_output_bytes,
    )?;
    checked_add(
        &mut usage.reported_model_output_tokens,
        reported.output_tokens,
        "reported_model_output_tokens",
        limits.max_reported_model_output_tokens,
    )?;
    Ok(())
}

fn account_partial_provider(
    state: &mut RunState,
    sink: &ProviderSink,
    reported: ProviderUsage,
    route: &Route,
    limits: EffectiveLimits,
) -> Result<(), Diagnostic> {
    account_uncertain(&mut state.usage, sink, reported, route, limits)
}

fn push_final_event(
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

fn price(tokens: u64, per_million: u64) -> Result<u64, Diagnostic> {
    tokens
        .checked_mul(per_million)
        .and_then(|value| value.checked_add(999_999))
        .map(|value| value / 1_000_000)
        .ok_or_else(|| g208("usd_microunits", MAX_USD_MICROUNITS))
}

fn checked_add(target: &mut u64, amount: u64, field: &str, maximum: u64) -> Result<(), Diagnostic> {
    let value = target
        .checked_add(amount)
        .ok_or_else(|| g208(field, maximum))?;
    if value > maximum {
        return Err(g208(field, maximum));
    }
    *target = value;
    Ok(())
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
    canonical_document(
        source,
        "provider request",
        PROVIDER_REQUEST_SCHEMA,
        MAX_PROVIDER_REQUEST_BYTES as usize,
    )?;
    let value: Value = serde_json::from_str(source.trim_end())
        .map_err(|_| g204("provider request", PROVIDER_REQUEST_SCHEMA))?;
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

fn render_tool_result(call_id: &str, tool_id: &str, result: &str) -> String {
    format!("{{\"schema\":\"{TOOL_RESULT_SCHEMA}\",\"call_id\":{},\"tool_id\":{},\"status\":\"ok\",\"result\":{result}}}\n", quote_json(call_id), quote_json(tool_id))
}

fn collect_response(sink: ProviderSink, limits: EffectiveLimits) -> Result<String, Diagnostic> {
    if let Some(rejection) = sink.rejection {
        return Err(match rejection {
            SinkRejection::Builder => g208("builder_bytes", limits.max_builder_bytes),
            SinkRejection::Chunks => g208("stream_chunks", limits.max_stream_chunks),
            SinkRejection::Bytes => g208(
                "provider_response_bytes",
                limits.max_provider_response_bytes,
            ),
        });
    }
    String::from_utf8(sink.bytes).map_err(|_| {
        operational(
            "SPX-I218",
            "Agent Runtime provider adapter failed: response invalid",
        )
    })
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

fn termination_from_diagnostic(diagnostic: Diagnostic) -> Termination {
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

fn render_bundle(
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
    Ok(AgentRuntimeEvidence {
        trace,
        trace_digest,
        evidence_digest: digest(EVIDENCE_DOMAIN, evidence.as_bytes()),
        evidence,
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

fn render_event(event: &TraceEvent) -> String {
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
fn render_usage(usage: &Usage) -> String {
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

fn render_production_limits() -> String {
    let mut output = String::new();
    write_production_limits(&mut output).expect("String writes are infallible");
    output
}
fn write_production_limits<W: fmt::Write>(output: &mut W) -> fmt::Result {
    write!(output, "{{\"max_profile_bytes\":{MAX_PROFILE_BYTES},\"max_task_bytes\":{MAX_TASK_BYTES},\"max_models\":{MAX_MODELS},\"max_tools\":{MAX_TOOLS},\"max_capabilities\":{MAX_CAPABILITIES},\"max_turns\":{MAX_TURNS},\"max_provider_attempts\":{MAX_PROVIDER_ATTEMPTS},\"max_retries_per_turn\":{MAX_RETRIES_PER_TURN},\"max_concurrency\":{MAX_CONCURRENCY},\"max_elapsed_ms\":{MAX_ELAPSED_MS},\"max_provider_request_bytes\":{MAX_PROVIDER_REQUEST_BYTES},\"max_provider_response_bytes\":{MAX_PROVIDER_RESPONSE_BYTES},\"max_stream_chunks\":{MAX_STREAM_CHUNKS},\"max_total_provider_input_bytes\":{MAX_TOTAL_PROVIDER_INPUT_BYTES},\"max_total_provider_output_bytes\":{MAX_TOTAL_PROVIDER_OUTPUT_BYTES},\"max_reported_model_input_tokens\":{MAX_REPORTED_MODEL_INPUT_TOKENS},\"max_reported_model_output_tokens\":{MAX_REPORTED_MODEL_OUTPUT_TOKENS},\"max_usd_microunits\":{MAX_USD_MICROUNITS},\"max_tool_calls\":{MAX_TOOL_CALLS},\"max_tool_arguments_bytes\":{MAX_TOOL_ARGUMENT_BYTES},\"max_tool_result_bytes\":{MAX_TOOL_RESULT_BYTES},\"max_total_tool_bytes\":{MAX_TOTAL_TOOL_BYTES},\"max_retained_state_bytes\":{MAX_RETAINED_STATE_BYTES},\"max_trace_events\":{MAX_TRACE_EVENTS},\"max_trace_bytes\":{MAX_TRACE_BYTES},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_builder_bytes\":{MAX_BUILDER_BYTES},\"max_json_depth\":{MAX_JSON_DEPTH},\"max_identifier_bytes\":{MAX_IDENTIFIER_BYTES},\"max_description_bytes\":{MAX_DESCRIPTION_BYTES}}}")
}

fn render_budget(budget: &EvidenceBudget) -> String {
    let mut output = String::new();
    write_budget(&mut output, budget).expect("String writes are infallible");
    output
}
fn write_budget<W: fmt::Write>(output: &mut W, budget: &EvidenceBudget) -> fmt::Result {
    write!(output, "{{\"used_models\":{},\"used_tools\":{},\"used_capabilities\":{},\"used_turns\":{},\"used_provider_attempts\":{},\"used_provider_input_bytes\":{},\"used_provider_output_bytes\":{},\"used_reported_model_input_tokens\":{},\"used_reported_model_output_tokens\":{},\"used_usd_microunits\":{},\"used_tool_calls\":{},\"used_tool_argument_bytes\":{},\"used_tool_result_bytes\":{},\"used_retained_state_bytes\":{},\"used_trace_events\":{},\"used_trace_bytes\":{},\"used_evidence_bytes\":{},\"used_builder_bytes\":{},\"used_elapsed_ms\":{},\"used_concurrency\":{}}}", budget.used_models, budget.used_tools, budget.used_capabilities, budget.used_turns, budget.used_provider_attempts, budget.used_provider_input_bytes, budget.used_provider_output_bytes, budget.used_reported_model_input_tokens, budget.used_reported_model_output_tokens, budget.used_usd_microunits, budget.used_tool_calls, budget.used_tool_argument_bytes, budget.used_tool_result_bytes, budget.used_retained_state_bytes, budget.used_trace_events, budget.used_trace_bytes, budget.used_evidence_bytes, budget.used_builder_bytes, budget.used_elapsed_ms, budget.used_concurrency)
}

pub(super) fn replay_trace(source: &str) -> Result<(), Diagnostic> {
    canonical_document(source, "trace", TRACE_SCHEMA, MAX_TRACE_BYTES as usize)?;
    let value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g204("trace", TRACE_SCHEMA))?;
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

fn canonical_sha256(value: &str) -> bool {
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

fn replay_evidence_inner(
    source: &str,
    profile: &Profile,
    state: &RunState,
    expected_trace: &str,
    expected_budget: &EvidenceBudget,
) -> Result<(), Diagnostic> {
    canonical_document(
        source,
        "evidence",
        EVIDENCE_SCHEMA,
        MAX_EVIDENCE_BYTES as usize,
    )?;
    let value: Value =
        serde_json::from_str(source.trim_end()).map_err(|_| g204("evidence", EVIDENCE_SCHEMA))?;
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
pub(super) fn replay_evidence(
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
