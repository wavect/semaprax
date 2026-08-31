//! Strict JSON-RPC 2.0 request codec for the project stdio transport.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

pub(crate) const PARSE_ERROR: i64 = -32700;
pub(crate) const INVALID_REQUEST: i64 = -32600;
pub(crate) const INVALID_PARAMS: i64 = -32602;
const MAX_ID_BYTES: usize = 128;
const MAX_METHOD_BYTES: usize = 128;
const RESPONSE_OVERFLOW: &[u8] = b"{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32001,\"message\":\"response exceeds configured byte limit\"}}";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RequestId {
    Number(u64),
    Text(String),
}

impl RequestId {
    pub(crate) fn render(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => serde_json::to_string(value).expect("strings always serialize"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum RequestKind {
    Call(RequestId),
    Notification,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RpcRequest {
    pub(crate) kind: RequestKind,
    pub(crate) method: String,
    pub(crate) params: Option<Map<String, Value>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RpcError {
    pub(crate) code: i64,
    pub(crate) message: String,
    /// Authenticated call id for a request-level error. `None` renders a null
    /// id unless `suppress_response` identifies a notification.
    pub(crate) response_id: Option<RequestId>,
    pub(crate) suppress_response: bool,
}

impl RpcError {
    fn parse(message: &'static str) -> Self {
        Self {
            code: PARSE_ERROR,
            message: message.to_owned(),
            response_id: None,
            suppress_response: false,
        }
    }

    fn request(message: &'static str) -> Self {
        Self {
            code: INVALID_REQUEST,
            message: message.to_owned(),
            response_id: None,
            suppress_response: false,
        }
    }
}

/// Decode one raw NDJSON frame using a closed JSON-RPC 2.0 grammar.
///
/// The lexical top-level scan is intentional: deserializing directly to a
/// `serde_json::Value` would silently replace repeated object members.
pub(crate) fn decode_request(frame: &[u8]) -> Result<RpcRequest, RpcError> {
    if frame.contains(&b'\r') {
        return Err(RpcError::parse("request contains a raw carriage return"));
    }
    let text =
        std::str::from_utf8(frame).map_err(|_| RpcError::parse("request is not valid UTF-8"))?;
    let value = serde_json::from_str::<Value>(text)
        .map_err(|_| RpcError::parse("request is not valid JSON"))?;
    scan_closed_request(frame)?;

    let Value::Object(mut object) = value else {
        return Err(RpcError::request(
            "invalid request: expected one JSON object",
        ));
    };
    match object.remove("jsonrpc") {
        Some(Value::String(version)) if version == "2.0" => {}
        _ => {
            return Err(RpcError::request(
                "invalid request: jsonrpc must be exactly \"2.0\"",
            ));
        }
    }
    let method = match object.remove("method") {
        Some(Value::String(method))
            if !method.is_empty()
                && method.len() <= MAX_METHOD_BYTES
                && !method.chars().any(char::is_control) =>
        {
            method
        }
        _ => {
            return Err(RpcError::request(
                "invalid request: method must be a nonempty bounded string without control characters",
            ));
        }
    };
    let kind = match object.remove("id") {
        None => RequestKind::Notification,
        Some(Value::Number(number)) => number
            .as_u64()
            .map(RequestId::Number)
            .map(RequestKind::Call)
            .ok_or_else(|| {
                RpcError::request(
                    "invalid request: id must be an unsigned integer or bounded string",
                )
            })?,
        Some(Value::String(text))
            if !text.is_empty()
                && text.len() <= MAX_ID_BYTES
                && !text.chars().any(char::is_control) =>
        {
            RequestKind::Call(RequestId::Text(text))
        }
        Some(_) => {
            return Err(RpcError::request(
                "invalid request: id must be an unsigned integer or bounded string",
            ));
        }
    };
    let params = match object.remove("params") {
        None => None,
        Some(Value::Object(params)) => Some(params),
        Some(_) => {
            let (response_id, suppress_response) = match &kind {
                RequestKind::Call(id) => (Some(id.clone()), false),
                RequestKind::Notification => (None, true),
            };
            return Err(RpcError {
                code: INVALID_PARAMS,
                message: "invalid params: params must be an object".to_owned(),
                response_id,
                suppress_response,
            });
        }
    };
    debug_assert!(
        object.is_empty(),
        "closed member scan rejected unknown keys"
    );
    Ok(RpcRequest {
        kind,
        method,
        params,
    })
}

pub(crate) fn bounded_success_response(
    id: &RequestId,
    result_json: &str,
    max_frame_bytes: usize,
) -> Vec<u8> {
    const PREFIX: &str = "{\"jsonrpc\":\"2.0\",\"id\":";
    const MIDDLE: &str = ",\"result\":";
    let rendered_id = id.render();
    let required = PREFIX
        .len()
        .checked_add(rendered_id.len())
        .and_then(|value| value.checked_add(MIDDLE.len()))
        .and_then(|value| value.checked_add(result_json.len()))
        .and_then(|value| value.checked_add(2)); // closing brace plus LF
    if required.is_none_or(|required| required > max_frame_bytes) {
        return bounded_overflow_response(Some(id), max_frame_bytes);
    }
    format!("{PREFIX}{rendered_id}{MIDDLE}{result_json}}}").into_bytes()
}

/// Build one compact canonical error response. `None` renders the required
/// null id for a frame whose id could not be authenticated.
pub(crate) fn error_response(id: Option<&RequestId>, code: i64, message: &str) -> Vec<u8> {
    let id = id.map_or_else(|| "null".to_owned(), RequestId::render);
    let message = serde_json::to_string(message).expect("strings always serialize");
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id},\"error\":{{\"code\":{code},\"message\":{message}}}}}"
    )
    .into_bytes()
}

pub(crate) fn bounded_error_response(
    id: Option<&RequestId>,
    code: i64,
    message: &str,
    max_frame_bytes: usize,
) -> Vec<u8> {
    const PREFIX: &str = "{\"jsonrpc\":\"2.0\",\"id\":";
    const MIDDLE: &str = ",\"error\":{\"code\":";
    const MESSAGE: &str = ",\"message\":";
    let rendered_id = id.map_or_else(|| "null".to_owned(), RequestId::render);
    let rendered_code = code.to_string();
    let message_bytes = quoted_json_bytes(message);
    let required = PREFIX
        .len()
        .checked_add(rendered_id.len())
        .and_then(|value| value.checked_add(MIDDLE.len()))
        .and_then(|value| value.checked_add(rendered_code.len()))
        .and_then(|value| value.checked_add(MESSAGE.len()))
        .and_then(|value| value.checked_add(message_bytes))
        .and_then(|value| value.checked_add(3)); // two closing braces plus LF
    if required.is_none_or(|required| required > max_frame_bytes) {
        return bounded_overflow_response(id, max_frame_bytes);
    }
    error_response(id, code, message)
}

fn bounded_overflow_response(id: Option<&RequestId>, max_frame_bytes: usize) -> Vec<u8> {
    let correlated = error_response(id, -32001, "response exceeds configured byte limit");
    if correlated
        .len()
        .checked_add(1)
        .is_some_and(|required| required <= max_frame_bytes)
    {
        correlated
    } else {
        response_overflow_error().to_vec()
    }
}

fn quoted_json_bytes(value: &str) -> usize {
    value.chars().fold(2usize, |used, character| {
        used.saturating_add(match character {
            '"' | '\\' | '\n' | '\r' | '\t' => 2,
            value if value.is_control() => 6,
            value => value.len_utf8(),
        })
    })
}

pub(crate) const fn response_overflow_error() -> &'static [u8] {
    RESPONSE_OVERFLOW
}

pub(crate) fn is_overflow_response(response: &[u8]) -> bool {
    response
        .windows(b"\"code\":-32001,\"message\":\"response exceeds configured byte limit\"".len())
        .any(|window| {
            window == b"\"code\":-32001,\"message\":\"response exceeds configured byte limit\""
        })
}

/// Check duplicate members and the closed request envelope after bounded JSON
/// parsing. Alternate transports may retain their own JSON-RPC identifier grammar.
pub(crate) fn scan_closed_request(frame: &[u8]) -> Result<(), RpcError> {
    let mut cursor = skip_whitespace(frame, 0);
    if frame.get(cursor) != Some(&b'{') {
        return Ok(());
    }
    scan_object(frame, &mut cursor, true)?;
    cursor = skip_whitespace(frame, cursor);
    if cursor != frame.len() {
        return Err(RpcError::parse("request is not valid JSON"));
    }
    Ok(())
}

fn scan_object(frame: &[u8], cursor: &mut usize, top_level: bool) -> Result<(), RpcError> {
    *cursor += 1;
    let mut seen = BTreeSet::new();
    loop {
        *cursor = skip_whitespace(frame, *cursor);
        if frame.get(*cursor) == Some(&b'}') {
            *cursor += 1;
            return Ok(());
        }
        let (key, after_key) = parse_string_member(frame, *cursor)?;
        if top_level && !matches!(key.as_str(), "jsonrpc" | "id" | "method" | "params") {
            return Err(RpcError::request("invalid request: unknown member"));
        }
        if !seen.insert(key) {
            return Err(RpcError::request(
                "invalid request: duplicate object member",
            ));
        }
        *cursor = skip_whitespace(frame, after_key);
        if frame.get(*cursor) != Some(&b':') {
            return Err(RpcError::parse("request is not valid JSON"));
        }
        *cursor += 1;
        scan_value(frame, cursor)?;
        *cursor = skip_whitespace(frame, *cursor);
        match frame.get(*cursor) {
            Some(b',') => *cursor += 1,
            Some(b'}') => {
                *cursor += 1;
                return Ok(());
            }
            _ => return Err(RpcError::parse("request is not valid JSON")),
        }
    }
}

fn parse_string_member(frame: &[u8], start: usize) -> Result<(String, usize), RpcError> {
    if frame.get(start) != Some(&b'\"') {
        return Err(RpcError::parse("request is not valid JSON"));
    }
    let mut cursor = start + 1;
    let mut escaped = false;
    while let Some(&byte) = frame.get(cursor) {
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'\"' {
            let end = cursor + 1;
            let key = serde_json::from_slice::<String>(&frame[start..end])
                .map_err(|_| RpcError::parse("request is not valid JSON"))?;
            return Ok((key, end));
        }
        cursor += 1;
    }
    Err(RpcError::parse("request is not valid JSON"))
}

fn scan_value(frame: &[u8], cursor: &mut usize) -> Result<(), RpcError> {
    *cursor = skip_whitespace(frame, *cursor);
    match frame.get(*cursor) {
        Some(b'{') => scan_object(frame, cursor, false),
        Some(b'[') => scan_array(frame, cursor),
        Some(b'\"') => {
            let (_, end) = parse_string_member(frame, *cursor)?;
            *cursor = end;
            Ok(())
        }
        Some(_) => {
            while frame.get(*cursor).is_some_and(|byte| {
                !matches!(byte, b',' | b'}' | b']' | b' ' | b'\t' | b'\n' | b'\r')
            }) {
                *cursor += 1;
            }
            Ok(())
        }
        None => Err(RpcError::parse("request is not valid JSON")),
    }
}

fn scan_array(frame: &[u8], cursor: &mut usize) -> Result<(), RpcError> {
    *cursor += 1;
    loop {
        *cursor = skip_whitespace(frame, *cursor);
        if frame.get(*cursor) == Some(&b']') {
            *cursor += 1;
            return Ok(());
        }
        scan_value(frame, cursor)?;
        *cursor = skip_whitespace(frame, *cursor);
        match frame.get(*cursor) {
            Some(b',') => *cursor += 1,
            Some(b']') => {
                *cursor += 1;
                return Ok(());
            }
            _ => return Err(RpcError::parse("request is not valid JSON")),
        }
    }
}

fn skip_whitespace(bytes: &[u8], mut cursor: usize) -> usize {
    while bytes
        .get(cursor)
        .is_some_and(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
    {
        cursor += 1;
    }
    cursor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_request_grammar_preserves_calls_and_notifications() {
        let call = decode_request(
            br#"{"jsonrpc":"2.0","id":"agent","method":"run","params":{"max_steps":7}}"#,
        )
        .unwrap();
        assert_eq!(
            call.kind,
            RequestKind::Call(RequestId::Text("agent".into()))
        );
        assert_eq!(call.method, "run");
        assert_eq!(call.params.unwrap()["max_steps"], 7);

        let notification = decode_request(br#"{"jsonrpc":"2.0","method":"shutdown"}"#).unwrap();
        assert_eq!(notification.kind, RequestKind::Notification);
    }

    #[test]
    fn duplicate_unknown_and_non_utf8_frames_fail_closed() {
        for frame in [
            br#"{"jsonrpc":"2.0","id":1,"id":2,"method":"run"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","method":"run","extra":0}"#.as_slice(),
            br#"{"jsonrpc":"2.0","method":"run","params":{"options":{"fuel":1,"fuel":2}}}"#
                .as_slice(),
        ] {
            assert_eq!(decode_request(frame).unwrap_err().code, INVALID_REQUEST);
        }
        assert_eq!(decode_request(&[0xff]).unwrap_err().code, PARSE_ERROR);
        assert_eq!(
            decode_request(b"{\"jsonrpc\":\"2.0\",\"method\":\"run\"}\r")
                .unwrap_err()
                .code,
            PARSE_ERROR
        );
    }

    #[test]
    fn closed_id_params_and_method_grammar_rejects_ambiguous_values() {
        for frame in [
            br#"{"jsonrpc":"2.0","id":null,"method":"run"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":-1,"method":"run"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":1.5,"method":"run"}"#.as_slice(),
            br#"{"jsonrpc":"2.0","id":1,"method":"","params":[]}"#.as_slice(),
        ] {
            assert!(decode_request(frame).is_err());
        }
    }

    #[test]
    fn invalid_notification_params_are_explicitly_silent() {
        let notification =
            decode_request(br#"{"jsonrpc":"2.0","method":"run","params":[]}"#).unwrap_err();
        assert_eq!(notification.code, INVALID_PARAMS);
        assert!(notification.suppress_response);
        assert_eq!(notification.response_id, None);

        let call =
            decode_request(br#"{"jsonrpc":"2.0","id":7,"method":"run","params":[]}"#).unwrap_err();
        assert!(!call.suppress_response);
        assert_eq!(call.response_id, Some(RequestId::Number(7)));
    }

    #[test]
    fn response_helpers_escape_and_preserve_semantic_payload_exactly() {
        assert_eq!(
            bounded_success_response(&RequestId::Number(9), r#"{"z":1,"a":2}"#, 1024),
            br#"{"jsonrpc":"2.0","id":9,"result":{"z":1,"a":2}}"#
        );
        assert_eq!(
            error_response(
                Some(&RequestId::Text("a\"b".into())),
                INVALID_PARAMS,
                "bad\nvalue",
            ),
            br#"{"jsonrpc":"2.0","id":"a\"b","error":{"code":-32602,"message":"bad\nvalue"}}"#
        );
        assert!(!response_overflow_error().contains(&b'\n'));

        let result = format!("\"{}\"", "x".repeat(100));
        let expected = format!("{{\"jsonrpc\":\"2.0\",\"id\":9,\"result\":{result}}}");
        let required = expected.len() + 1;
        let exact = bounded_success_response(&RequestId::Number(9), &result, required);
        assert_eq!(exact, expected.as_bytes());
        let overflow = bounded_success_response(&RequestId::Number(9), &result, required - 1);
        assert_eq!(
            overflow,
            bounded_error_response(
                Some(&RequestId::Number(9)),
                -32001,
                "response exceeds configured byte limit",
                1024,
            )
        );
        assert!(is_overflow_response(&overflow));
    }
}
