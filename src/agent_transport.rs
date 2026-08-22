//! Bounded Graph Agent Transport v1.
//!
//! One deterministic JSON-RPC 2.0 session over newline-delimited stdin/stdout
//! frames serves the semantic projections of exactly one checked program. The
//! session binds its source at construction, so requests cannot redirect it to
//! other files and the transport gains no ambient read/write/process/network
//! authority. `graph`, `context`, and `context_v2` results embed the exact
//! unchanged payload bytes produced by [`crate::graph`]; every envelope is
//! canonical hand-rolled JSON.

use std::collections::BTreeSet;
use std::io::{BufRead, Write};
use std::path::Path;

use serde_json::Value;

use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph::{
    self, AgentContextDirection, AgentContextFilter, AgentContextOptions, AgentContextV2Options,
};

pub const TRANSPORT_SCHEMA: &str = "semaprax.agent-transport.v1";

pub const DEFAULT_MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MIN_MAX_REQUEST_BYTES: usize = 1024;
pub const MAX_MAX_REQUEST_BYTES: usize = 1024 * 1024;

const MAX_SYMBOL_BYTES: usize = 256;
const MAX_ID_STRING_BYTES: usize = 128;
const METHODS: [&str; 6] = [
    "context",
    "context_v2",
    "graph",
    "ping",
    "protocol",
    "shutdown",
];

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const APPLICATION_ERROR: i64 = -32000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportLimits {
    max_request_bytes: usize,
}

impl TransportLimits {
    pub fn new(max_request_bytes: usize) -> Result<Self, String> {
        if !(MIN_MAX_REQUEST_BYTES..=MAX_MAX_REQUEST_BYTES).contains(&max_request_bytes) {
            return Err(format!(
                "agent transport max_request_bytes {max_request_bytes} is outside {MIN_MAX_REQUEST_BYTES}..={MAX_MAX_REQUEST_BYTES}"
            ));
        }
        Ok(Self { max_request_bytes })
    }

    #[must_use]
    pub const fn max_request_bytes(&self) -> usize {
        self.max_request_bytes
    }
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            max_request_bytes: DEFAULT_MAX_REQUEST_BYTES,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
enum RequestId {
    Number(u64),
    Text(String),
}

impl RequestId {
    fn render(&self) -> String {
        match self {
            Self::Number(value) => value.to_string(),
            Self::Text(value) => quote_json(value),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ParsedMethod {
    Context,
    ContextV2,
    Graph,
    Ping,
    Protocol,
    Shutdown,
}

impl ParsedMethod {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "context" => Some(Self::Context),
            "context_v2" => Some(Self::ContextV2),
            "graph" => Some(Self::Graph),
            "ping" => Some(Self::Ping),
            "protocol" => Some(Self::Protocol),
            "shutdown" => Some(Self::Shutdown),
            _ => None,
        }
    }

    const fn takes_no_params(self) -> bool {
        !matches!(self, Self::Context | Self::ContextV2)
    }
}

pub struct Session {
    program: crate::ast::Program,
    revision: String,
    source_display: String,
    source_bytes: usize,
    limits: TransportLimits,
    shutdown: bool,
}

impl Session {
    pub fn open(path: &Path, limits: TransportLimits) -> Result<Self, Vec<Diagnostic>> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I001",
                format!("cannot read {}: {error}", path.display()),
            )]
        })?;
        Self::from_source(&source, path, limits)
    }

    pub fn from_source(
        source: &str,
        path: &Path,
        limits: TransportLimits,
    ) -> Result<Self, Vec<Diagnostic>> {
        let program = crate::parse(source, path).map_err(|error| vec![error])?;
        let diagnostics = crate::verify::verify(&program);
        if diagnostics.iter().any(|item| item.severity.is_error()) {
            return Err(diagnostics);
        }
        let revision = graph::revision(&program);
        Ok(Self {
            program,
            revision,
            source_display: path.display().to_string(),
            source_bytes: source.len(),
            limits,
            shutdown: false,
        })
    }

    #[must_use]
    pub fn stop_requested(&self) -> bool {
        self.shutdown
    }

    #[must_use]
    pub const fn limits(&self) -> TransportLimits {
        self.limits
    }

    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    #[must_use]
    pub fn source_bytes(&self) -> usize {
        self.source_bytes
    }

    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        if self.shutdown {
            return None;
        }
        let trimmed = line.trim_matches(|character: char| character.is_ascii_whitespace());
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.len() > self.limits.max_request_bytes {
            self.shutdown = true;
            return Some(error_envelope(
                "null",
                PARSE_ERROR,
                &format!(
                    "request exceeds agent transport max_request_bytes {}",
                    self.limits.max_request_bytes
                ),
            ));
        }
        match serde_json::from_str::<Value>(trimmed) {
            Ok(value) => self.handle_value(value),
            Err(_) => Some(error_envelope("null", PARSE_ERROR, "parse error")),
        }
    }

    fn handle_value(&mut self, value: Value) -> Option<String> {
        let Value::Object(object) = value else {
            return Some(error_envelope(
                "null",
                INVALID_REQUEST,
                "invalid request: expected one JSON object per line",
            ));
        };
        let mut jsonrpc = None;
        let mut id_value = None;
        let mut method_value = None;
        let mut params_value = None;
        for (key, value) in object {
            match key.as_str() {
                "jsonrpc" => jsonrpc = Some(value),
                "id" => id_value = Some(value),
                "method" => method_value = Some(value),
                "params" => params_value = Some(value),
                _ => {
                    return Some(error_envelope(
                        "null",
                        INVALID_REQUEST,
                        "invalid request: unknown member",
                    ));
                }
            }
        }
        let jsonrpc_ok = matches!(jsonrpc.as_ref(), Some(Value::String(text)) if text == "2.0");
        let method_ok = matches!(method_value.as_ref(), Some(Value::String(_)));
        if !jsonrpc_ok || !method_ok {
            return Some(error_envelope(
                "null",
                INVALID_REQUEST,
                "invalid request: requires jsonrpc \"2.0\" and a string method",
            ));
        }
        let id = id_value.as_ref().and_then(parse_id);
        if id_value.is_some() && id.is_none() {
            return Some(error_envelope(
                "null",
                INVALID_REQUEST,
                "invalid request: id must be an unsigned integer or bounded string",
            ));
        }
        let id_json = id
            .as_ref()
            .map_or_else(|| "null".to_owned(), RequestId::render);
        let notification = id.is_none();
        let Value::String(name) = method_value.expect("validated string method") else {
            unreachable!("validated string method")
        };
        let Some(parsed_method) = ParsedMethod::from_name(&name) else {
            return (!notification).then(|| {
                error_envelope(
                    &id_json,
                    METHOD_NOT_FOUND,
                    &format!("method not found: {name}"),
                )
            });
        };
        let params = match params_value {
            None => None,
            Some(Value::Object(entries)) if entries.is_empty() => None,
            Some(Value::Object(entries)) => Some(entries),
            Some(_) => {
                return (!notification).then(|| {
                    error_envelope(
                        &id_json,
                        INVALID_PARAMS,
                        "invalid params: params must be absent or an object",
                    )
                });
            }
        };
        if parsed_method.takes_no_params() && params.is_some() {
            return (!notification).then(|| {
                error_envelope(
                    &id_json,
                    INVALID_PARAMS,
                    "invalid params: this method takes no parameters",
                )
            });
        }
        self.dispatch(parsed_method, params, notification, &id_json)
    }

    fn dispatch(
        &mut self,
        method: ParsedMethod,
        params: Option<serde_json::Map<String, Value>>,
        notification: bool,
        id_json: &str,
    ) -> Option<String> {
        if notification && !matches!(method, ParsedMethod::Shutdown) {
            return None;
        }
        match method {
            ParsedMethod::Shutdown => {
                self.shutdown = true;
                (!notification).then(|| success_envelope(id_json, "{\"ok\":true}"))
            }
            ParsedMethod::Ping => Some(success_envelope(id_json, "{\"pong\":true}")),
            ParsedMethod::Protocol => Some(success_envelope(id_json, &self.protocol_result())),
            ParsedMethod::Graph => Some(match graph::to_json(&self.program) {
                Ok(payload) => success_envelope(id_json, &format!("{{\"graph\":{payload}}}")),
                Err(diagnostics) => application_error(id_json, &diagnostics),
            }),
            ParsedMethod::Context | ParsedMethod::ContextV2 => {
                Some(self.context_response(method, params, id_json))
            }
        }
    }

    fn protocol_result(&self) -> String {
        format!(
            "{{\"protocol\":{},\"revision\":{},\"version\":{},\"methods\":[{}],\"limits\":{{\"max_request_bytes\":{}}},\"source\":{{\"path\":{},\"bytes\":{}}}}}",
            quote_json(TRANSPORT_SCHEMA),
            quote_json(&self.revision),
            quote_json(env!("CARGO_PKG_VERSION")),
            METHODS
                .iter()
                .map(|method| quote_json(method))
                .collect::<Vec<_>>()
                .join(","),
            self.limits.max_request_bytes,
            quote_json(&self.source_display),
            self.source_bytes,
        )
    }

    fn context_response(
        &mut self,
        method: ParsedMethod,
        params: Option<serde_json::Map<String, Value>>,
        id_json: &str,
    ) -> String {
        let entries = params.unwrap_or_default();
        let mut symbol = None;
        let mut depth = None;
        let mut max_bytes = None;
        let mut max_nodes = None;
        let mut filters = None;
        let mut direction = None;
        for (key, value) in entries {
            match key.as_str() {
                "symbol" => match value {
                    Value::String(text) => symbol = Some(text),
                    _ => return invalid_params(id_json, "symbol must be a string"),
                },
                "depth" => match unsigned_param("depth", &value) {
                    Ok(parsed) => depth = parsed,
                    Err(message) => return invalid_params(id_json, &message),
                },
                "max_bytes" => match unsigned_param("max_bytes", &value) {
                    Ok(parsed) => max_bytes = parsed,
                    Err(message) => return invalid_params(id_json, &message),
                },
                "max_nodes" => match unsigned_param("max_nodes", &value) {
                    Ok(parsed) => max_nodes = parsed,
                    Err(message) => return invalid_params(id_json, &message),
                },
                "filters" => match parse_filters(&value) {
                    Ok(parsed) => filters = Some(parsed),
                    Err(message) => return invalid_params(id_json, &message),
                },
                "direction" => match value {
                    Value::String(text) => direction = Some(text),
                    _ => return invalid_params(id_json, "direction must be a string"),
                },
                unknown => {
                    return invalid_params(
                        id_json,
                        &format!("unknown context parameter `{unknown}`"),
                    );
                }
            }
        }
        let Some(symbol) = symbol.filter(|text| {
            !text.is_empty()
                && text.len() <= MAX_SYMBOL_BYTES
                && !text.chars().any(char::is_control)
        }) else {
            return invalid_params(
                id_json,
                "symbol must be a nonempty string of at most 256 bytes without control characters",
            );
        };
        let defaults = AgentContextOptions::default();
        let chosen_filters: BTreeSet<AgentContextFilter> = filters.unwrap_or_else(default_filters);
        let query = match method {
            ParsedMethod::ContextV2 => {
                let Some(direction_name) = direction else {
                    return invalid_params(id_json, "context_v2 requires a direction parameter");
                };
                let Some(parsed_direction) = AgentContextDirection::from_name(&direction_name)
                else {
                    return invalid_params(
                        id_json,
                        &format!("unknown context direction `{direction_name}`"),
                    );
                };
                AgentContextV2Options::new(
                    depth.unwrap_or_else(|| defaults.depth()),
                    max_bytes.unwrap_or_else(|| defaults.max_bytes()),
                    max_nodes.unwrap_or_else(|| defaults.max_nodes()),
                    chosen_filters,
                    parsed_direction,
                )
                .map(|options| graph::agent_context_v2_json(&self.program, &symbol, &options))
            }
            _ => {
                if direction.is_some() {
                    return invalid_params(
                        id_json,
                        "direction is only valid for context_v2; resend with method context_v2",
                    );
                }
                AgentContextOptions::new(
                    depth.unwrap_or_else(|| defaults.depth()),
                    max_bytes.unwrap_or_else(|| defaults.max_bytes()),
                    max_nodes.unwrap_or_else(|| defaults.max_nodes()),
                    chosen_filters,
                )
                .map(|options| graph::agent_context_json(&self.program, &symbol, &options))
            }
        };
        match query {
            Ok(outcome) => match outcome {
                Ok(Some(payload)) => {
                    success_envelope(id_json, &format!("{{\"context\":{payload}}}"))
                }
                Ok(None) => not_found_error(id_json, &symbol),
                Err(diagnostics) => application_error(id_json, &diagnostics),
            },
            Err(diagnostic) => invalid_params(id_json, &diagnostic.message),
        }
    }
}

fn unsigned_param(name: &str, value: &Value) -> Result<Option<usize>, String> {
    match value {
        Value::Number(number) => number
            .as_u64()
            .and_then(|parsed| usize::try_from(parsed).ok())
            .map(Some)
            .ok_or_else(|| format!("{name} must be an unsigned integer")),
        _ => Err(format!("{name} must be an unsigned integer")),
    }
}

fn parse_filters(value: &Value) -> Result<BTreeSet<AgentContextFilter>, String> {
    let Value::Array(items) = value else {
        return Err("filters must be an array".to_owned());
    };
    let mut parsed = BTreeSet::new();
    for item in items {
        let Value::String(name) = item else {
            return Err("filters must contain only strings".to_owned());
        };
        let Some(filter) = AgentContextFilter::from_name(name) else {
            return Err(format!("unknown context filter `{name}`"));
        };
        if !parsed.insert(filter) {
            return Err(format!("duplicate context filter `{name}`"));
        }
    }
    Ok(parsed)
}

fn default_filters() -> BTreeSet<AgentContextFilter> {
    [
        AgentContextFilter::Contracts,
        AgentContextFilter::Ownership,
        AgentContextFilter::Effects,
        AgentContextFilter::Types,
    ]
    .into_iter()
    .collect()
}

fn parse_id(value: &Value) -> Option<RequestId> {
    match value {
        Value::Number(number) if number.is_u64() => {
            Some(RequestId::Number(number.as_u64().expect("checked u64")))
        }
        Value::String(text)
            if !text.is_empty()
                && text.len() <= MAX_ID_STRING_BYTES
                && !text.chars().any(char::is_control) =>
        {
            Some(RequestId::Text(text.clone()))
        }
        _ => None,
    }
}

fn success_envelope(id_json: &str, result: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id_json},\"result\":{result}}}")
}

fn error_envelope(id_json: &str, code: i64, message: &str) -> String {
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id_json},\"error\":{{\"code\":{code},\"message\":{}}}}}",
        quote_json(message),
    )
}

fn application_error(id_json: &str, diagnostics: &[Diagnostic]) -> String {
    let rendered = diagnostics
        .iter()
        .map(Diagnostic::json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":{id_json},\"error\":{{\"code\":{APPLICATION_ERROR},\"message\":{},\"data\":{{\"diagnostics\":[{rendered}]}}}}}}",
        quote_json("semantic resolution failed for the bound source"),
    )
}

fn not_found_error(id_json: &str, symbol: &str) -> String {
    error_envelope(
        id_json,
        APPLICATION_ERROR,
        &format!("symbol `{symbol}` was not found"),
    )
}

fn invalid_params(id_json: &str, message: &str) -> String {
    error_envelope(id_json, INVALID_PARAMS, message)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServeOutcome {
    pub responses: usize,
    pub stopped_by_shutdown: bool,
}

pub fn serve(
    input: &mut dyn BufRead,
    output: &mut dyn Write,
    source_path: &Path,
    limits: TransportLimits,
) -> Result<ServeOutcome, Vec<Diagnostic>> {
    let mut session = Session::open(source_path, limits)?;
    let mut responses = 0usize;
    loop {
        if session.stop_requested() {
            break;
        }
        let mut line = Vec::new();
        let read = input.read_until(b'\n', &mut line).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I001",
                format!("cannot read agent transport request: {error}"),
            )]
        })?;
        if read == 0 {
            break;
        }
        let request = String::from_utf8_lossy(&line);
        if let Some(response) = session.handle_line(&request) {
            writeln!(output, "{response}").map_err(|error| {
                vec![Diagnostic::io(
                    "SPX-I001",
                    format!("cannot write agent transport response: {error}"),
                )]
            })?;
            output.flush().map_err(|error| {
                vec![Diagnostic::io(
                    "SPX-I001",
                    format!("cannot flush agent transport response: {error}"),
                )]
            })?;
            responses += 1;
        }
    }
    Ok(ServeOutcome {
        responses,
        stopped_by_shutdown: session.stop_requested(),
    })
}
