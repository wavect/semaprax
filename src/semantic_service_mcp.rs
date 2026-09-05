//! Bounded MCP stdio facade for one authority-free semantic workspace service.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::project::{ProjectRevision, MAX_SOURCES};
use crate::project_transport::codec;
use crate::semantic_service_transport::{
    SemanticWorkspaceStdioSession, MAX_SEMANTIC_SERVICE_REQUEST_BYTES,
    MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
};

pub const SEMANTIC_SERVICE_MCP_SCHEMA: &str = "semaprax.semantic-workspace-service-mcp.v1";
pub const SEMANTIC_SERVICE_MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MAX_SEMANTIC_SERVICE_MCP_REQUEST_BYTES: usize = MAX_SEMANTIC_SERVICE_REQUEST_BYTES;
// One inner byte can require six JSON string bytes. Reserve this complete bound
// before dispatch so refresh never mutates state and then loses its response.
pub const MAX_SEMANTIC_SERVICE_MCP_RESPONSE_BYTES: usize =
    6 * MAX_SEMANTIC_SERVICE_RESPONSE_BYTES + 4096;
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 32_768;
const MAX_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Lifecycle {
    New,
    AwaitingInitialized,
    Ready,
}

/// MCP owns no host adapter. The Project path has already been consumed by the
/// caller; subsequent calls can supply bytes but cannot name filesystem paths.
pub struct SemanticWorkspaceMcpSession {
    inner: SemanticWorkspaceStdioSession,
    lifecycle: Lifecycle,
}

impl SemanticWorkspaceMcpSession {
    pub fn open(
        revision: Arc<ProjectRevision>,
    ) -> Result<Self, Vec<crate::diagnostic::Diagnostic>> {
        let mut inner = SemanticWorkspaceStdioSession::open(revision)?;
        // Construction is the service-open boundary. This fixed private call
        // cannot select a path or alter the retained generation; MCP initialize
        // only negotiates the outer protocol.
        let opened = inner
            .handle_frame(br#"{"jsonrpc":"2.0","id":0,"method":"workspace/open","params":{}}"#)
            .expect("fixed workspace/open call has a response");
        if serde_json::from_slice::<Value>(&opened)
            .ok()
            .is_none_or(|response| response.get("result").is_none())
        {
            return Err(vec![crate::diagnostic::Diagnostic::io(
                "SPX-G548",
                "semantic workspace MCP could not open its retained service",
            )]);
        }
        Ok(Self {
            inner,
            lifecycle: Lifecycle::New,
        })
    }

    pub fn service(&self) -> &crate::project::SemanticWorkspaceService {
        self.inner.service()
    }

    pub fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        if frame.len() > MAX_SEMANTIC_SERVICE_MCP_REQUEST_BYTES {
            return Some(rpc_error(
                &Value::Null,
                -32700,
                "request exceeds semantic service MCP byte limit",
            ));
        }
        let request = match decode(frame) {
            Ok(request) => request,
            Err((code, message)) => return Some(rpc_error(&Value::Null, code, &message)),
        };
        let Request { id, method, params } = request;
        let Some(id) = id else {
            if method == "notifications/initialized"
                && self.lifecycle == Lifecycle::AwaitingInitialized
                && parameters(&params, &[]).is_ok()
            {
                self.lifecycle = Lifecycle::Ready;
            }
            return None;
        };
        let params = match params {
            Some(Value::Object(params)) => params,
            None => Map::new(),
            _ => return Some(rpc_error(&id, -32602, "params must be an object")),
        };
        let result = match method.as_str() {
            "initialize" => self.initialize(&params),
            "ping" => checked_parameters(&params, &[]).map(|()| json!({})),
            "tools/list" | "tools/call" if self.lifecycle != Lifecycle::Ready => Err((
                -32000,
                "SPX-G548: semantic service MCP initialization is not complete".into(),
            )),
            "tools/list" => list_tools(&params),
            "tools/call" => return Some(self.call(&id, &params)),
            _ => Err((-32601, "method not found".into())),
        };
        Some(match result {
            Ok(value) => rpc_result(&id, value),
            Err((code, message)) => rpc_error(&id, code, &message),
        })
    }

    fn initialize(&mut self, params: &Map<String, Value>) -> RpcResult {
        if self.lifecycle != Lifecycle::New {
            return Err((-32000, "SPX-G548: MCP is already initialized".into()));
        }
        checked_parameters(params, &["protocolVersion", "capabilities", "clientInfo"])?;
        if !params.get("protocolVersion").is_some_and(Value::is_string)
            || !params.get("capabilities").is_some_and(Value::is_object)
            || !params.get("clientInfo").is_some_and(|info| {
                info.is_object()
                    && info.get("name").is_some_and(Value::is_string)
                    && info.get("version").is_some_and(Value::is_string)
            })
        {
            return Err((
                -32602,
                "initialize requires protocolVersion, capabilities and clientInfo name/version"
                    .into(),
            ));
        }
        self.lifecycle = Lifecycle::AwaitingInitialized;
        Ok(json!({
            "protocolVersion": SEMANTIC_SERVICE_MCP_PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "semaprax-semantic-service", "version": env!("CARGO_PKG_VERSION")},
            "instructions": format!("Protocol schema: {SEMANTIC_SERVICE_MCP_SCHEMA}. Authority-free single-client semantic service. Startup reads exactly one Project; tools have no filesystem, process, network, commit, publication, or path-selection authority.")
        }))
    }

    fn call(&mut self, id: &Value, params: &Map<String, Value>) -> Vec<u8> {
        if let Err((code, message)) = checked_parameters(params, &["name", "arguments"]) {
            return rpc_error(id, code, &message);
        }
        let method = match params
            .get("name")
            .and_then(Value::as_str)
            .and_then(tool_method)
        {
            Some(method) => method,
            None => return rpc_error(id, -32602, "unknown semantic service tool name"),
        };
        let arguments = match params.get("arguments") {
            Some(Value::Object(arguments)) => arguments.clone(),
            None => Map::new(),
            _ => return rpc_error(id, -32602, "arguments must be an object"),
        };
        let forwarded =
            json!({"jsonrpc":"2.0","id":0,"method":method,"params":arguments}).to_string();
        if forwarded.len() > MAX_SEMANTIC_SERVICE_REQUEST_BYTES {
            return rpc_error(id, -32602, "forwarded request exceeds service byte limit");
        }
        let mut output = Vec::new();
        if output
            .try_reserve_exact(MAX_SEMANTIC_SERVICE_MCP_RESPONSE_BYTES)
            .is_err()
        {
            return rpc_error(id, -32000, "SPX-G549: MCP response reservation failed");
        }
        let response = self.inner.handle_frame(forwarded.as_bytes()).unwrap_or_else(|| {
            b"{\"jsonrpc\":\"2.0\",\"id\":0,\"error\":{\"code\":-32000,\"message\":\"service returned no response\"}}".to_vec()
        });
        let is_error = serde_json::from_slice::<Value>(&response)
            .ok()
            .is_none_or(|response| response.get("error").is_some());
        output.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":");
        output.extend_from_slice(id.to_string().as_bytes());
        output.extend_from_slice(b",\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"");
        escape_json_bytes(&response, &mut output);
        output.extend_from_slice(if is_error {
            b"\"}],\"isError\":true}}"
        } else {
            b"\"}],\"isError\":false}}"
        });
        debug_assert!(output.len() <= MAX_SEMANTIC_SERVICE_MCP_RESPONSE_BYTES);
        output
    }
}

/// Serve LF-delimited MCP frames. EOF is the only session termination method;
/// there is no socket, watcher, path reopen, or background lifecycle.
pub fn serve_semantic_workspace_mcp<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    revision: Arc<ProjectRevision>,
) -> io::Result<()> {
    let mut session = SemanticWorkspaceMcpSession::open(revision).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            diagnostics
                .first()
                .map_or("service MCP open failed", |diagnostic| {
                    diagnostic.message.as_str()
                }),
        )
    })?;
    loop {
        let Some(frame) = read_frame(&mut input)? else {
            return Ok(());
        };
        if let Some(response) = session.handle_frame(&frame) {
            output.write_all(&response)?;
            output.write_all(b"\n")?;
            output.flush()?;
        }
    }
}

type RpcResult = Result<Value, (i64, String)>;

struct Request {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn decode(frame: &[u8]) -> Result<Request, (i64, String)> {
    if frame.is_empty() || frame.contains(&b'\r') || frame.contains(&b'\n') {
        return Err((-32700, "invalid semantic service MCP frame".into()));
    }
    let mut value: Value = serde_json::from_slice(frame)
        .map_err(|_| (-32700, "request is not valid bounded JSON".into()))?;
    check_tree(&value, 0, &mut 0)?;
    // Reuse the established lexical closure/duplicate-key scanner without
    // changing its frozen transport's request or response bytes.
    codec::scan_closed_request(frame).map_err(|error| (error.code, error.message))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| (-32600, "request must be one object".into()))?;
    if object.len() > 4
        || object
            .keys()
            .any(|key| !["jsonrpc", "id", "method", "params"].contains(&key.as_str()))
        || object.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
    {
        return Err((
            -32600,
            "request members or jsonrpc version are invalid".into(),
        ));
    }
    let method = object
        .remove("method")
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|method| {
            !method.is_empty() && method.len() <= 128 && !method.chars().any(char::is_control)
        })
        .ok_or_else(|| (-32600, "method must be a bounded string".into()))?;
    let id = object.remove("id");
    if id.as_ref().is_some_and(|id| match id {
        Value::String(text) => text.len() > MAX_ID_BYTES,
        Value::Number(number) => !(number.is_i64() || number.is_u64()),
        _ => true,
    }) {
        return Err((-32600, "id must be a bounded string or integer".into()));
    }
    Ok(Request {
        id,
        method,
        params: object.remove("params"),
    })
}

fn list_tools(params: &Map<String, Value>) -> RpcResult {
    checked_parameters(params, &["cursor"])?;
    if params.contains_key("cursor") {
        return Err((
            -32602,
            "semantic service tool catalogue has one page".into(),
        ));
    }
    Ok(json!({"tools": tools()}))
}

fn tools() -> Vec<Value> {
    vec![
        tool("service__protocol", "Report the closed semantic service protocol, limits, and nonclaims.", json!({"type":"object","properties":{},"additionalProperties":false})),
        tool("workspace__status", "Report the retained active generation without acquiring authority.", json!({"type":"object","properties":{},"additionalProperties":false})),
        tool("workspace__query", "Run one canonical Universal Semantic Query v1 string against the retained generation.", one_string_schema("query")),
        tool("workspace__index_query", "Run one canonical retained semantic index query string against the retained generation.", one_string_schema("query")),
        tool("workspace__history_query", "Run one canonical revision-bound query over successful transaction-validation and refresh outcomes.", one_string_schema("query")),
        tool("workspace__validate_transaction", "Validate one canonical Universal Semantic Transaction v1 string without adopting its candidate.", one_string_schema("transaction")),
        tool("workspace__refresh", "Refresh from caller-owned canonical manifest and source bytes. Source paths are Project-relative identities, never host path selectors.", json!({
            "type":"object",
            "properties":{
                "expected_workspace_revision":{"type":"string"},
                "manifest":{"type":"string","maxLength":65536},
                "sources":{"type":"array","maxItems":MAX_SOURCES,"items":{"type":"object","properties":{"path":{"type":"string"},"source":{"type":"string"}},"required":["path","source"],"additionalProperties":false}}
            },
            "required":["expected_workspace_revision","manifest","sources"],
            "additionalProperties":false
        })),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({"name":name,"description":description,"inputSchema":input_schema})
}

fn one_string_schema(name: &str) -> Value {
    let mut properties = Map::new();
    properties.insert(name.to_owned(), json!({"type":"string"}));
    json!({"type":"object","properties":properties,"required":[name],"additionalProperties":false})
}

fn tool_method(name: &str) -> Option<&'static str> {
    match name {
        "service__protocol" => Some("service/protocol"),
        "workspace__status" => Some("workspace/status"),
        "workspace__query" => Some("workspace/query"),
        "workspace__index_query" => Some("workspace/index-query"),
        "workspace__history_query" => Some("workspace/history-query"),
        "workspace__validate_transaction" => Some("workspace/validate-transaction"),
        "workspace__refresh" => Some("workspace/refresh"),
        _ => None,
    }
}

fn checked_parameters(params: &Map<String, Value>, allowed: &[&str]) -> Result<(), (i64, String)> {
    if params
        .keys()
        .any(|key| key != "_meta" && !allowed.contains(&key.as_str()))
        || params.get("_meta").is_some_and(|meta| !meta.is_object())
    {
        return Err((
            -32602,
            "unexpected parameter or invalid _meta object".into(),
        ));
    }
    Ok(())
}

fn parameters(params: &Option<Value>, allowed: &[&str]) -> Result<(), (i64, String)> {
    match params {
        None => Ok(()),
        Some(Value::Object(params)) => checked_parameters(params, allowed),
        _ => Err((-32602, "params must be an object".into())),
    }
}

fn check_tree(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), (i64, String)> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err((-32600, "SPX-G549: MCP JSON structure exceeds bounds".into()));
    }
    match value {
        Value::Array(values) => {
            for value in values {
                check_tree(value, depth + 1, nodes)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                check_tree(value, depth + 1, nodes)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn rpc_result(id: &Value, result: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"result":result})
        .to_string()
        .into_bytes()
}

fn rpc_error(id: &Value, code: i64, message: &str) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}})
        .to_string()
        .into_bytes()
}

fn escape_json_bytes(bytes: &[u8], output: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for &byte in bytes {
        match byte {
            b'"' | b'\\' => {
                output.push(b'\\');
                output.push(byte);
            }
            0..=31 => {
                output.extend_from_slice(b"\\u00");
                output.push(HEX[(byte >> 4) as usize]);
                output.push(HEX[(byte & 15) as usize]);
            }
            _ => output.push(byte),
        }
    }
}

fn read_frame<R: BufRead>(input: &mut R) -> io::Result<Option<Vec<u8>>> {
    let mut frame = Vec::new();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Ok(Some(frame))
            };
        }
        let newline = available.iter().position(|byte| *byte == b'\n');
        let used = newline.map_or(available.len(), |index| index + 1);
        let content = newline.map_or(available, |index| &available[..index]);
        if frame.len().saturating_add(content.len()) <= MAX_SEMANTIC_SERVICE_MCP_REQUEST_BYTES {
            frame.extend_from_slice(content);
        } else {
            frame.resize(MAX_SEMANTIC_SERVICE_MCP_REQUEST_BYTES + 1, 0);
        }
        input.consume(used);
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}
