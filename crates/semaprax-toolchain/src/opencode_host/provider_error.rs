//! Closed classification of the OpenCode v1 provider-error event.
//!
//! Wire profile: OpenCode `v1.18.27`'s `opencode run --format json` emitter
//! writes `{"type":"error", ..., "error": props.error}` for a
//! `session.error` event. `props.error` is the generated SDK's
//! `{name,data}` union; its `APIError` declares an optional non-negative
//! `statusCode`. The primary sources are
//! <https://github.com/anomalyco/opencode/blob/v1.18.27/packages/opencode/src/cli/cmd/run.ts>
//! and
//! <https://github.com/anomalyco/opencode/blob/v1.18.27/packages/sdk/js/src/gen/types.gen.ts>.
//!
//! This module deliberately retains no provider message, response body,
//! response headers, metadata, or session identifier. The host's bounded
//! stdout capture is its input; this second bound also keeps direct callers
//! from parsing an unbounded event stream.

use serde_json::Value;

const MAX_ERROR_EVENTS_BYTES: usize = 1_048_576;

/// A stable, non-sensitive classification of a terminal OpenCode provider
/// event. It carries no provider-controlled diagnostic text or metadata.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCodeProviderFailure {
    RateLimited,
    Authentication,
    Refused,
    Server,
    Incomplete,
    Provider,
}

/// Classifies an `opencode run --format json` `error` JSON-lines event.
///
/// Unknown event/error shapes return `None`: callers keep their ordinary
/// generic provider failure instead of treating an undocumented shape as an
/// authoritative category. Malformed non-error lines are ignored so a valid
/// later terminal event in the same bounded capture remains observable.
#[must_use]
pub fn classify_provider_failure(events: &[u8]) -> Option<OpenCodeProviderFailure> {
    if events.len() > MAX_ERROR_EVENTS_BYTES {
        return None;
    }

    events
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice::<Value>(line).ok())
        .find_map(classify_event)
}

fn classify_event(event: Value) -> Option<OpenCodeProviderFailure> {
    let error = event
        .as_object()?
        .get("type")?
        .as_str()
        .filter(|kind| *kind == "error")
        .and_then(|_| event.get("error"))?
        .as_object()?;
    let name = error.get("name")?.as_str()?;
    let data = error.get("data")?.as_object()?;

    match name {
        "ProviderAuthError" if required_string(data, "providerID") && required_string(data, "message") => {
            Some(OpenCodeProviderFailure::Authentication)
        }
        "UnknownError" if required_string(data, "message") => Some(OpenCodeProviderFailure::Provider),
        "MessageOutputLengthError" => Some(OpenCodeProviderFailure::Incomplete),
        "MessageAbortedError" if required_string(data, "message") => {
            Some(OpenCodeProviderFailure::Provider)
        }
        "APIError"
            if required_string(data, "message")
                && data.get("isRetryable").is_some_and(Value::is_boolean) =>
        {
            Some(classify_status(data.get("statusCode").and_then(Value::as_u64)))
        }
        _ => None,
    }
}

fn required_string(data: &serde_json::Map<String, Value>, field: &str) -> bool {
    data.get(field).is_some_and(Value::is_string)
}

fn classify_status(status: Option<u64>) -> OpenCodeProviderFailure {
    match status {
        Some(429) => OpenCodeProviderFailure::RateLimited,
        Some(401 | 403) => OpenCodeProviderFailure::Authentication,
        Some(400..=499) => OpenCodeProviderFailure::Refused,
        Some(500..=599) => OpenCodeProviderFailure::Server,
        _ => OpenCodeProviderFailure::Provider,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_provider_failure, OpenCodeProviderFailure};

    fn event(error: serde_json::Value) -> Vec<u8> {
        serde_json::json!({
            "type": "error",
            "timestamp": 1,
            "sessionID": "session-1",
            "error": error,
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn classifies_rate_limit_without_retaining_provider_payload() {
        let marker = "credential-leak-sentinel";
        let bytes = event(serde_json::json!({
            "name": "APIError",
            "data": {
                "message": marker,
                "statusCode": 429,
                "isRetryable": true,
                "responseHeaders": {"authorization": marker},
                "responseBody": marker,
            },
        }));

        let failure = classify_provider_failure(&bytes).expect("recognized event");
        assert_eq!(failure, OpenCodeProviderFailure::RateLimited);
        assert!(!format!("{failure:?}").contains(marker));
    }

    #[test]
    fn classifies_server_and_client_refusal_statuses() {
        let server = event(serde_json::json!({
            "name": "APIError",
            "data": {"message": "provider failed", "statusCode": 500, "isRetryable": true},
        }));
        let refused = event(serde_json::json!({
            "name": "APIError",
            "data": {"message": "request rejected", "statusCode": 400, "isRetryable": false},
        }));

        assert_eq!(classify_provider_failure(&server), Some(OpenCodeProviderFailure::Server));
        assert_eq!(classify_provider_failure(&refused), Some(OpenCodeProviderFailure::Refused));
    }

    #[test]
    fn classifies_authentication_and_partial_output() {
        let authentication = event(serde_json::json!({
            "name": "ProviderAuthError",
            "data": {"providerID": "opencode", "message": "not authorized"},
        }));
        let partial = event(serde_json::json!({
            "name": "MessageOutputLengthError",
            "data": {},
        }));

        assert_eq!(
            classify_provider_failure(&authentication),
            Some(OpenCodeProviderFailure::Authentication)
        );
        assert_eq!(classify_provider_failure(&partial), Some(OpenCodeProviderFailure::Incomplete));
    }

    #[test]
    fn ignores_malformed_lines_but_requires_the_versioned_envelope() {
        let valid = event(serde_json::json!({
            "name": "UnknownError",
            "data": {"message": "provider failure"},
        }));
        let mut events = b"not-json\n".to_vec();
        events.extend(valid);

        assert_eq!(classify_provider_failure(&events), Some(OpenCodeProviderFailure::Provider));
        assert_eq!(
            classify_provider_failure(br#"{"type":"session.error","properties":{"error":{}}}"#),
            None
        );
        assert_eq!(classify_provider_failure(&vec![b'x'; 1_048_577]), None);
    }
}
