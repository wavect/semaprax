//! Optional MCP stdio framing over an already authorized, owned v5 session.
use super::mcp_catalog::Catalog;
use super::*;

pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
pub const MAX_MCP_REQUEST_BYTES: usize = 128 * 1024;
pub const MAX_MCP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_DEPTH: usize = 128;
const MAX_NODES: usize = 32_768;
const MAX_ID_BYTES: usize = 128;

#[derive(Clone, Copy, Eq, PartialEq)]
enum Lifecycle {
    New,
    AwaitingInitialized,
    Ready,
}

/// An MCP adapter cannot add grants, approve a commit, or expose its inner host.
/// Construction consumes an unused, fully configured v5 session. Tool results
/// retain the exact inner JSON-RPC response as text, with inner request id zero.
pub struct McpSession {
    inner: VNextSession,
    catalog: Catalog,
    lifecycle: Lifecycle,
    terminal: bool,
}

impl McpSession {
    pub fn new(inner: VNextSession) -> Result<Self, Vec<Diagnostic>> {
        if inner.started || inner.is_terminal() {
            return Err(failure("SPX-G349", "MCP requires an unused v5 session"));
        }
        let catalog = Catalog::new_with_package(
            &inner.policy,
            inner.commit.is_some(),
            inner.package_graph.is_some(),
        )?;
        Ok(Self {
            inner,
            catalog,
            lifecycle: Lifecycle::New,
            terminal: false,
        })
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal || self.inner.is_terminal()
    }

    /// Handle one NDJSON frame without a trailing LF. Notifications never
    /// execute a tool. Invalid or oversized outer frames cannot reach v5.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        if self.is_terminal() || frame.is_empty() {
            return None;
        }
        if frame.len() > MAX_MCP_REQUEST_BYTES {
            return Some(self.oversized_response());
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
                "SPX-G349: MCP initialization is not complete".into(),
            )),
            "tools/list" => self.list(&params),
            "tools/call" => return Some(self.call(&id, &params)),
            _ => Err((-32601, "method not found".into())),
        };
        Some(match result {
            Ok(value) => rpc_result(&id, value),
            Err((code, message)) => rpc_error(&id, code, &message),
        })
    }

    /// Authenticate the original v5 snapshot even if initialization was never
    /// completed or the stream failed. Publication diagnostics are not erased.
    pub fn finish(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.terminal = true;
        self.inner.finish()
    }

    fn initialize(&mut self, params: &Map<String, Value>) -> RpcResult {
        if self.lifecycle != Lifecycle::New {
            return Err((-32000, "SPX-G349: MCP is already initialized".into()));
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
        // MCP negotiates by returning a supported version, including when the
        // requested version is unknown. The client decides whether to continue.
        self.lifecycle = Lifecycle::AwaitingInitialized;
        Ok(json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {"tools": {"listChanged": false}},
            "serverInfo": {"name": "semaprax", "version": env!("CARGO_PKG_VERSION")}
        }))
    }

    fn list(&self, params: &Map<String, Value>) -> RpcResult {
        checked_parameters(params, &["cursor"])?;
        let cursor = match params.get("cursor") {
            None => None,
            Some(Value::String(cursor)) => Some(cursor.as_str()),
            _ => return Err((-32602, "cursor must be a string".into())),
        };
        self.catalog
            .page(cursor)
            .map_err(|errors| (-32602, diagnostics(&errors)))
    }

    fn call(&mut self, id: &Value, params: &Map<String, Value>) -> Vec<u8> {
        if let Err((code, message)) = checked_parameters(params, &["name", "arguments"]) {
            return rpc_error(id, code, &message);
        }
        let method = match params
            .get("name")
            .and_then(Value::as_str)
            .and_then(|name| self.catalog.method(name))
        {
            Some(method) => method,
            None => return rpc_error(id, -32602, "unknown or unavailable tool name"),
        };
        let arguments = match params.get("arguments") {
            Some(Value::Object(arguments)) => arguments.clone(),
            None => Map::new(),
            _ => return rpc_error(id, -32602, "arguments must be an object"),
        };
        let request =
            json!({"jsonrpc":"2.0","id":0,"method":method,"params":arguments}).to_string();
        if request.len() > MAX_REQUEST_BYTES {
            return rpc_error(id, -32602, "forwarded request exceeds v5 byte limit");
        }

        // Reserve BEFORE any semantic action or publication. Every inner byte
        // expands to at most six JSON-string bytes; the bounded outer id expands
        // to <= 770 bytes and fixed syntax to < 256. Even including framing LF:
        // 6 * 1 MiB + 770 + 256 + 1 < 8 MiB. There is no post-action parse,
        // fallible serialization, or overflow replacement on this path. This
        // does not claim recovery from allocator failure or an overall heap cap.
        let mut output = Vec::new();
        if output.try_reserve_exact(MAX_MCP_RESPONSE_BYTES).is_err() {
            return rpc_error(id, -32000, "SPX-G351: MCP response reservation failed");
        }
        output.extend_from_slice(b"{\"jsonrpc\":\"2.0\",\"id\":");
        output.extend_from_slice(id.to_string().as_bytes());
        output.extend_from_slice(b",\"result\":{\"content\":[{\"type\":\"text\",\"text\":\"");

        // A valid request with id zero is always a call and this adapter has
        // already rejected a terminal inner session. The fallback preserves an
        // explicit tool error if that private v5 invariant ever changes.
        let response = self.inner.handle_frame(request.as_bytes());
        let response = response.as_deref().unwrap_or(
            b"{\"jsonrpc\":\"2.0\",\"id\":0,\"error\":{\"code\":-32000,\"message\":\"v5 session returned no response\"}}",
        );
        let is_error = !response.starts_with(b"{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":");
        escape_json_bytes(response, &mut output);
        output.extend_from_slice(if is_error {
            b"\"}],\"isError\":true}}"
        } else {
            b"\"}],\"isError\":false}}"
        });
        output
    }

    fn oversized_response(&mut self) -> Vec<u8> {
        self.terminal = true;
        rpc_error(&Value::Null, -32700, "request exceeds MCP byte limit")
    }
}

type RpcResult = Result<Value, (i64, String)>;
struct Request {
    id: Option<Value>,
    method: String,
    params: Option<Value>,
}

fn decode(frame: &[u8]) -> Result<Request, (i64, String)> {
    if frame.contains(&b'\r') || frame.contains(&b'\n') {
        return Err((-32700, "raw newline in MCP frame".into()));
    }
    let mut value: Value = serde_json::from_slice(frame)
        .map_err(|_| (-32700, "request is not valid bounded JSON".into()))?;
    check_tree(&value, 0, &mut 0)?;
    // Parsing precedes this existing lexical scanner, which assumes valid JSON.
    codec::scan_closed_request(frame).map_err(|error| (error.code, error.message))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| (-32600, "request must be one object".into()))?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err((-32600, "jsonrpc must be 2.0".into()));
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
        return Err((
            -32600,
            "id must be an integer or a string of at most 128 bytes".into(),
        ));
    }
    Ok(Request {
        id,
        method,
        params: object.remove("params"),
    })
}

fn check_tree(value: &Value, depth: usize, nodes: &mut usize) -> Result<(), (i64, String)> {
    *nodes += 1;
    if depth > MAX_DEPTH || *nodes > MAX_NODES {
        return Err((-32600, "SPX-G351: MCP JSON structure exceeds bounds".into()));
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

fn parameters(params: &Option<Value>, allowed: &[&str]) -> Result<(), (i64, String)> {
    match params {
        None => Ok(()),
        Some(Value::Object(params)) => checked_parameters(params, allowed),
        _ => Err((-32602, "params must be an object".into())),
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

/// Serve MCP over host-owned streams. No sockets, filesystem lookup, or new
/// permissions are acquired. Final authentication and publication uncertainty
/// use the same typed error path as the underlying v5 stdio host.
pub fn serve_mcp<R: BufRead, W: Write>(
    input: R,
    output: W,
    mut session: McpSession,
) -> io::Result<()> {
    let limits =
        StdioLimits::new(MAX_MCP_REQUEST_BYTES, MAX_MCP_RESPONSE_BYTES).expect("fixed MCP limits");
    let mut input = FrameReader::new(input, limits);
    let mut output = FrameWriter::new(output, limits);
    let result = (|| {
        loop {
            let response = match input.read_frame()? {
                Frame::Eof => break,
                Frame::OversizedTerminal => Some(session.oversized_response()),
                Frame::Data(frame) => session.handle_frame(&frame),
            };
            if let Some(response) = response {
                output.write_response(&response)?;
            }
            if session.is_terminal() {
                break;
            }
        }
        Ok(())
    })();
    let final_check = session.finish();
    let outcome = session
        .inner
        .commit
        .as_ref()
        .filter(|host| host.is_terminal())
        .map(|host| {
            if host.status()["state"] == "published" {
                PublicationOutcome::Published
            } else {
                PublicationOutcome::Uncertain
            }
        });
    finish_stream(result, final_check, outcome)
}
