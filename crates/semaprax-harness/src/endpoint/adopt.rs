//! Explicit endpoint adoption: list models, then actually probe each admitted
//! wire protocol. An "OpenAI-compatible" label implies nothing; every verdict
//! is backed by an observed reply.

use super::catalog::{CatalogModel, EndpointRecord};
use super::ownership::{AttemptOwnership, Disclosure};
use super::probe::{HttpReply, ProbeClient, StreamReply, Target};
use super::types::*;
use super::usage::{assess_reply, assess_stream, detect_shape, Outcome, UsageEvidence};
use crate::decision::Destination;
use crate::diag::HarnessResult;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub struct AdoptRequest {
    pub id: Option<String>,
    pub url: String,
    pub kind: EndpointKind,
    /// Environment variable NAME; the value is passed separately and never stored.
    pub credential_env: Option<String>,
    pub credential_value: Option<String>,
    pub disclosure: Option<Disclosure>,
    /// Model used for protocol probes (default: first listed).
    pub probe_model: Option<String>,
}

const MAX_MODELS: usize = 64;

/// Probe an endpoint and build its record. Nothing is downloaded or changed
/// on the server; probes are tiny bounded requests.
pub fn adopt(req: &AdoptRequest) -> HarnessResult<EndpointRecord> {
    let target = Target::parse(&req.url)?;
    if let Some(c) = &req.credential_env {
        validate_credential_name(c)?;
    }
    let client = ProbeClient::new(target.clone(), req.credential_value.clone());
    let models = discover(&client, req.kind)?;
    let probe_model = match &req.probe_model {
        Some(m) if models.iter().any(|x| &x.name == m) => m.clone(),
        Some(m) => {
            return Err(err(
                "SPX-HPL006",
                format!("model `{m}` is not in the endpoint catalog"),
            ))
        }
        None => models.first().map(|m| m.name.clone()).ok_or_else(|| {
            err(
                "SPX-HPL006",
                "endpoint lists no models; nothing to adopt (no model is downloaded)",
            )
        })?,
    };
    let (probes, returned_model) = probe_protocols(&client, &probe_model);
    let mut models = models;
    for m in &mut models {
        if m.name == probe_model && req.kind != EndpointKind::Ollama {
            if let Some(r) = &returned_model {
                m.identity = ModelIdentity::Reported(r.clone());
            }
        }
    }
    let id = req
        .id
        .clone()
        .unwrap_or_else(|| format!("{}-{}", req.kind.as_str(), target.port));
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        || id.is_empty()
        || id.len() > 64
    {
        return Err(err(
            "SPX-HPL001",
            "endpoint id must be 1-64 chars of [A-Za-z0-9._-]",
        ));
    }
    let (ownership, destinations, disclosed) = match (&req.disclosure, req.kind.is_gateway()) {
        (Some(d), _) => (d.ownership.clone(), d.destinations.clone(), true),
        (None, false) => (AttemptOwnership::direct(), vec![Destination::Local], false),
        (None, true) => (AttemptOwnership::undisclosed(), Vec::new(), false),
    };
    Ok(EndpointRecord {
        id,
        kind: req.kind,
        url: target.origin(),
        credential_env: req.credential_env.clone(),
        probe_model,
        returned_model,
        models,
        probes,
        ownership,
        destinations,
        disclosed,
    })
}

fn discover(client: &ProbeClient, kind: EndpointKind) -> HarnessResult<Vec<CatalogModel>> {
    let mut out = Vec::new();
    if kind == EndpointKind::Ollama {
        let reply = client.get("/api/tags")?;
        let v = reply
            .json()
            .filter(|_| reply.status == 200)
            .ok_or_else(|| {
                err(
                    "SPX-HPL003",
                    format!(
                        "GET /api/tags -> {} (not an Ollama model list)",
                        reply.status
                    ),
                )
            })?;
        for m in v
            .get("models")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(MAX_MODELS)
        {
            let Some(name) = m.get("name").and_then(Value::as_str) else {
                continue;
            };
            let mut cm = CatalogModel {
                name: name.to_string(),
                identity: m
                    .get("digest")
                    .and_then(Value::as_str)
                    .map_or(ModelIdentity::Unknown, |d| {
                        ModelIdentity::Digest(d.to_string())
                    }),
                context_length: m.pointer("/details/context_length").and_then(Value::as_u64),
                remote_host: m
                    .get("remote_host")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                notes: Vec::new(),
            };
            if let Ok(show) = client.post_json("/api/show", &json!({"model": name})) {
                if let Some(s) = show.json().filter(|_| show.status == 200) {
                    cm.notes = s
                        .get("capabilities")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .map(|c| format!("ollama_capability:{c}"))
                        .collect();
                    cm.notes.sort();
                    if cm.remote_host.is_none() {
                        cm.remote_host = s
                            .get("remote_host")
                            .and_then(Value::as_str)
                            .map(str::to_string);
                    }
                }
            }
            out.push(cm);
        }
    } else {
        let reply = client.get("/v1/models")?;
        let v = reply
            .json()
            .filter(|_| reply.status == 200)
            .ok_or_else(|| {
                err(
                    "SPX-HPL003",
                    format!(
                        "GET /v1/models -> {} (not an OpenAI-style model list)",
                        reply.status
                    ),
                )
            })?;
        for m in v
            .get("data")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(MAX_MODELS)
        {
            if let Some(id) = m.get("id").and_then(Value::as_str) {
                out.push(CatalogModel {
                    name: id.to_string(),
                    identity: ModelIdentity::Unknown,
                    context_length: None,
                    remote_host: None,
                    notes: Vec::new(),
                });
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn status_verdict(reply: &HttpReply, what: &str) -> Option<ProtocolVerdict> {
    match reply.status {
        401 | 403 | 429 | 500..=599 => Some(ProtocolVerdict::new(
            Verdict::Unverified,
            format!("{what} -> {} (not a protocol verdict)", reply.status),
        )),
        s if !(200..300).contains(&s) => Some(ProtocolVerdict::new(
            Verdict::Unsupported,
            format!("{what} -> {s}: {}", reply.excerpt()),
        )),
        _ => None,
    }
}

fn unreachable_verdict(what: &str, e: &crate::diag::HarnessDiagnostic) -> ProtocolVerdict {
    ProtocolVerdict::new(Verdict::Unverified, format!("{what}: {}", e.message))
}

/// Probe every admitted protocol on one model. Returns verdicts keyed by
/// `responses`, `responses_streaming`, `chat_completions`, `chat_streaming`,
/// `anthropic_messages`, `tool_calls`, `usage`, `structured_output`, plus the
/// model label the server returned.
pub fn probe_protocols(
    client: &ProbeClient,
    model: &str,
) -> (BTreeMap<String, ProtocolVerdict>, Option<String>) {
    let mut p = BTreeMap::new();
    let mut returned = None;
    let mut usage_seen: Vec<(&str, UsageEvidence)> = Vec::new();

    // Responses, buffered.
    let body =
        json!({"model": model, "input": "Reply with the single word ok.", "max_output_tokens": 16});
    let v = match client.post_json("/v1/responses", &body) {
        Err(e) => unreachable_verdict("POST /v1/responses", &e),
        Ok(r) => status_verdict(&r, "POST /v1/responses").unwrap_or_else(|| {
            let a = assess_reply(Protocol::Responses, &r, false, None);
            usage_seen.push(("responses", a.usage.clone()));
            returned = returned.take().or(a.returned_model.clone());
            match a.outcomes.iter().find(|o| o.is_refusal()) {
                Some(o) => ProtocolVerdict::new(
                    Verdict::Unsupported,
                    format!(
                        "POST /v1/responses -> {} {}",
                        r.status,
                        o.to_json()["outcome"].as_str().unwrap_or("")
                    ),
                ),
                None => ProtocolVerdict::new(
                    Verdict::Supported,
                    format!(
                        "POST /v1/responses -> 200 object=response usage={}",
                        usage_label(&a.usage)
                    ),
                ),
            }
        }),
    };
    p.insert("responses".to_string(), v);

    // Responses, streaming.
    let body = json!({"model": model, "input": "Reply with the single word ok.", "max_output_tokens": 16, "stream": true});
    p.insert(
        "responses_streaming".into(),
        stream_verdict(client, "/v1/responses", &body, Protocol::Responses),
    );

    // Chat, buffered.
    let chat = json!({"model": model, "messages": [{"role": "user", "content": "Reply with the single word ok."}], "max_tokens": 16});
    let v = match client.post_json("/v1/chat/completions", &chat) {
        Err(e) => unreachable_verdict("POST /v1/chat/completions", &e),
        Ok(r) => status_verdict(&r, "POST /v1/chat/completions").unwrap_or_else(|| {
            let a = assess_reply(Protocol::ChatCompletions, &r, false, None);
            usage_seen.push(("chat_completions", a.usage.clone()));
            returned = returned.take().or(a.returned_model.clone());
            match a.outcomes.iter().find(|o| o.is_refusal()) {
                Some(o) => ProtocolVerdict::new(
                    Verdict::Unsupported,
                    format!(
                        "POST /v1/chat/completions -> {} {}",
                        r.status,
                        o.to_json()["outcome"].as_str().unwrap_or("")
                    ),
                ),
                None => ProtocolVerdict::new(
                    Verdict::Supported,
                    format!(
                        "POST /v1/chat/completions -> 200 choices usage={}",
                        usage_label(&a.usage)
                    ),
                ),
            }
        }),
    };
    p.insert("chat_completions".into(), v);

    let mut chat_s = chat.clone();
    chat_s["stream"] = json!(true);
    chat_s["stream_options"] = json!({"include_usage": true});
    p.insert(
        "chat_streaming".into(),
        stream_verdict(
            client,
            "/v1/chat/completions",
            &chat_s,
            Protocol::ChatCompletions,
        ),
    );

    // Anthropic Messages.
    let am = json!({"model": model, "max_tokens": 16, "messages": [{"role": "user", "content": "Reply with the single word ok."}]});
    let v = match client.send(
        "POST",
        "/v1/messages",
        &[("anthropic-version", "2023-06-01")],
        Some(am.to_string().as_bytes()),
    ) {
        Err(e) => unreachable_verdict("POST /v1/messages", &e),
        Ok(r) => status_verdict(&r, "POST /v1/messages").unwrap_or_else(|| {
            match r.json().as_ref().and_then(detect_shape) {
                Some(Protocol::AnthropicMessages) => ProtocolVerdict::new(
                    Verdict::Supported,
                    "POST /v1/messages -> 200 type=message",
                ),
                other => ProtocolVerdict::new(
                    Verdict::Unsupported,
                    format!(
                        "POST /v1/messages -> 200 but shape {}",
                        other.map_or("unrecognised", Protocol::as_str)
                    ),
                ),
            }
        }),
    };
    p.insert("anthropic_messages".into(), v);

    // Tool-call schema.
    let mut tools = chat.clone();
    tools["tools"] = json!([{"type": "function", "function": {"name": "get_word", "description": "Return a word.",
        "parameters": {"type": "object", "properties": {"word": {"type": "string"}}, "required": ["word"], "additionalProperties": false}}}]);
    tools["tool_choice"] = json!("auto");
    let v = match client.post_json("/v1/chat/completions", &tools) {
        Err(e) => unreachable_verdict("tools via /v1/chat/completions", &e),
        Ok(r) => status_verdict(&r, "tools via /v1/chat/completions")
            .map(|mut v| {
                let a = assess_reply(Protocol::ChatCompletions, &r, true, None);
                if let Some(Outcome::UnsupportedToolSchema { detail }) = a.outcomes.first() {
                    v = ProtocolVerdict::new(
                        Verdict::Unsupported,
                        format!("tool schema refused: {detail}"),
                    );
                }
                v
            })
            .unwrap_or_else(|| {
                let called = r.json().is_some_and(|j| {
                    j.pointer("/choices/0/message/tool_calls")
                        .is_some_and(|t| t.is_array())
                });
                ProtocolVerdict::new(
                    Verdict::Supported,
                    format!("tool schema accepted (200); tool_call_emitted={called}"),
                )
            }),
    };
    p.insert("tool_calls".into(), v);

    // Structured output.
    let mut so = chat.clone();
    so["messages"] = json!([{"role": "user", "content": "Name one color."}]);
    so["max_tokens"] = json!(48);
    so["response_format"] = json!({"type": "json_schema", "json_schema": {"name": "color", "strict": true,
        "schema": {"type": "object", "properties": {"color": {"type": "string"}}, "required": ["color"], "additionalProperties": false}}});
    let v = match client.post_json("/v1/chat/completions", &so) {
        Err(e) => unreachable_verdict("json_schema via /v1/chat/completions", &e),
        Ok(r) => status_verdict(&r, "json_schema via /v1/chat/completions").unwrap_or_else(|| {
            let content = r.json().and_then(|j| {
                j.pointer("/choices/0/message/content")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            });
            let ok = content
                .as_deref()
                .and_then(|c| serde_json::from_str::<Value>(c).ok())
                .is_some_and(|j| j.get("color").is_some_and(Value::is_string));
            if ok {
                ProtocolVerdict::new(
                    Verdict::Supported,
                    "json_schema accepted; reply parsed against the schema",
                )
            } else {
                ProtocolVerdict::new(
                    Verdict::Unsupported,
                    "json_schema accepted but reply did not satisfy the schema (not enforced)",
                )
            }
        }),
    };
    p.insert("structured_output".into(), v);

    // Usage reporting, from the buffered probes.
    let reported = usage_seen
        .iter()
        .filter(|(_, u)| *u != UsageEvidence::Unknown)
        .map(|(n, _)| *n)
        .collect::<Vec<_>>();
    let v = if !reported.is_empty() {
        ProtocolVerdict::new(
            Verdict::Supported,
            format!("provider-reported usage on: {}", reported.join(",")),
        )
    } else if usage_seen.is_empty() {
        ProtocolVerdict::new(
            Verdict::Unverified,
            "no buffered probe succeeded; usage unknown",
        )
    } else {
        ProtocolVerdict::new(
            Verdict::Unsupported,
            "buffered replies carried no usage object; usage stays unknown, never zero",
        )
    };
    p.insert("usage".into(), v);
    (p, returned)
}

fn usage_label(u: &UsageEvidence) -> &'static str {
    if *u == UsageEvidence::Unknown {
        "unknown"
    } else {
        "provider_reported"
    }
}

fn stream_verdict(
    client: &ProbeClient,
    path: &str,
    body: &Value,
    protocol: Protocol,
) -> ProtocolVerdict {
    let what = format!("POST {path} stream");
    let r: StreamReply = match client.post_stream(path, body, None, 512) {
        Err(e) => return unreachable_verdict(&what, &e),
        Ok(r) => r,
    };
    let reply = HttpReply {
        status: r.status,
        content_type: r.content_type.clone(),
        body: r.body.clone(),
    };
    if let Some(v) = status_verdict(&reply, &what) {
        return v;
    }
    if !r.content_type.starts_with("text/event-stream") || r.events.is_empty() {
        return ProtocolVerdict::new(
            Verdict::Unsupported,
            format!(
                "{what} -> {} content-type={} events=0",
                r.status, r.content_type
            ),
        );
    }
    let a = assess_stream(protocol, &r.events, None);
    match a.outcomes.iter().find(|o| o.is_refusal()) {
        Some(o) => ProtocolVerdict::new(
            Verdict::Unsupported,
            format!(
                "{what} -> 200 {}",
                o.to_json()["outcome"].as_str().unwrap_or("")
            ),
        ),
        None => ProtocolVerdict::new(
            Verdict::Supported,
            format!(
                "{what} -> 200 sse events={} usage={}",
                r.events.len(),
                usage_label(&a.usage)
            ),
        ),
    }
}
