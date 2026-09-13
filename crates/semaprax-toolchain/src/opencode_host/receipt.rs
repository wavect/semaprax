//! Bind the observed OpenCode 1.18.27 stream to its exported session.
use super::{ModelFailure, OpenCodeReceipt, MAX_EVENTS_BYTES, MAX_EXPORT_BYTES};
use serde_json::Value;

// OpenCode v1.18.27 cli/cmd/run.ts quotes a positional argument containing
// an ASCII space and escapes only double quotes before storing user text.
// Reproduce that exact transform, rather than accepting either spelling.
pub(super) fn cli_prompt(prompt: &str) -> String {
    if prompt.contains(' ') {
        format!("\"{}\"", prompt.replace('"', "\\\""))
    } else {
        prompt.to_owned()
    }
}

fn malformed() -> ModelFailure {
    ModelFailure::MalformedResponse
}

fn rows(events: &[u8]) -> Result<Vec<Value>, ModelFailure> {
    if events.len() > MAX_EVENTS_BYTES {
        return Err(malformed());
    }
    let text = std::str::from_utf8(events).map_err(|_| malformed())?;
    let rows: Vec<Value> = text
        .lines()
        .filter(|line| !line.is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| malformed())?;
    if rows.len() != 3 {
        return Err(malformed());
    }
    for (row, (event, part)) in rows.iter().zip([
        ("step_start", "step-start"),
        ("text", "text"),
        ("step_finish", "step-finish"),
    ]) {
        if row["type"] != event || row["part"]["type"] != part {
            return Err(malformed());
        }
    }
    Ok(rows)
}

pub(super) fn event_text(events: &[u8]) -> Result<(String, String, String), ModelFailure> {
    let rows = rows(events)?;
    let session = rows[0]["sessionID"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(malformed)?;
    let message = rows[0]["part"]["messageID"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(malformed)?;
    let answer = rows[1]["part"]["text"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(malformed)?;
    if !session.starts_with("ses_")
        || session.len() > 128
        || !session
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        || !message.starts_with("msg_")
        || message.len() > 128
    {
        return Err(malformed());
    }
    let mut ids = std::collections::BTreeSet::new();
    for row in &rows {
        let id = row["part"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(malformed)?;
        if !ids.insert(id)
            || row["sessionID"] != session
            || row["part"]["sessionID"] != session
            || row["part"]["messageID"] != message
        {
            return Err(malformed());
        }
    }
    if rows[2]["part"]["reason"] != "stop" {
        return Err(malformed());
    }
    Ok((session.into(), message.into(), answer.into()))
}

pub(super) fn validate_export(
    export: &[u8],
    events: &[u8],
    session: &str,
    message: &str,
    prompt: &str,
    answer: &str,
) -> Result<OpenCodeReceipt, ModelFailure> {
    if export.len() > MAX_EXPORT_BYTES {
        return Err(malformed());
    }
    let export: Value = serde_json::from_slice(export).map_err(|_| malformed())?;
    if export["info"]["id"] != session
        || export["info"]["model"]["providerID"] != "opencode"
        || export["info"]["model"]["id"] != "muse-spark-1.3-contributor-free"
    {
        return Err(malformed());
    }
    let messages = export["messages"].as_array().ok_or_else(malformed)?;
    // A new run has exactly one request and one response; extra turns are not this profile.
    if messages.len() != 2 {
        return Err(malformed());
    }
    let user = &messages[0];
    let assistant = &messages[1];
    let info = &assistant["info"];
    if info["id"] != message
        || info["role"] != "assistant"
        || info["sessionID"] != session
        || info["modelID"] != "muse-spark-1.3-contributor-free"
        || info["providerID"] != "opencode"
        || info["finish"] != "stop"
        || info.get("error").is_some()
        || user["info"]["role"] != "user"
        || user["info"]["sessionID"] != session
        || user["info"]["id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .is_none()
        || info["parentID"] != user["info"]["id"]
    {
        return Err(malformed());
    }
    let user_parts = user["parts"].as_array().ok_or_else(malformed)?;
    if user_parts.len() != 1
        || user_parts[0]["type"] != "text"
        || user_parts[0]["text"] != cli_prompt(prompt)
        || user_parts[0]["sessionID"] != session
        || user_parts[0]["messageID"] != user["info"]["id"]
    {
        return Err(malformed());
    }
    let parts = assistant["parts"].as_array().ok_or_else(malformed)?;
    let streamed = match parts.as_slice() {
        [start, text, finish] => vec![start, text, finish],
        [start, reasoning, text, finish]
            if reasoning["type"] == "reasoning"
                && reasoning["text"] == ""
                && reasoning["sessionID"] == session
                && reasoning["messageID"] == message =>
        {
            vec![start, text, finish]
        }
        _ => return Err(malformed()),
    };
    let observed = rows(events)?;
    if streamed
        .iter()
        .zip(&observed)
        .any(|(part, row)| **part != row["part"])
        || streamed[1]["text"] != answer
    {
        return Err(malformed());
    }
    let counter = |value: Option<&Value>| -> Result<Option<u64>, ModelFailure> {
        value.map(|v| v.as_u64().ok_or_else(malformed)).transpose()
    };
    let usage = match info.get("tokens") {
        None => None,
        Some(tokens) if tokens.is_object() => {
            let cache = tokens.get("cache");
            if cache.is_some_and(|v| !v.is_object()) {
                return Err(malformed());
            }
            Some(super::OpenCodeUsage {
                total: counter(tokens.get("total"))?,
                input: counter(tokens.get("input"))?,
                output: counter(tokens.get("output"))?,
                reasoning: counter(tokens.get("reasoning"))?,
                cache_read: counter(cache.and_then(|v| v.get("read")))?,
                cache_write: counter(cache.and_then(|v| v.get("write")))?,
            })
        }
        Some(_) => return Err(malformed()),
    };
    let reported_cost = match info.get("cost") {
        None => None,
        Some(Value::Number(n)) if n.as_f64().is_some_and(|v| v.is_finite() && v >= 0.0) => {
            Some(n.clone())
        }
        Some(_) => return Err(malformed()),
    };
    Ok(OpenCodeReceipt {
        session_id: session.into(),
        message_id: message.into(),
        model: super::OPENCODE_MODEL,
        usage_total: usage.as_ref().and_then(|u| u.total),
        usage,
        reported_cost,
    })
}
