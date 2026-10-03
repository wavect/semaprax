//! Optional MCP stdio projection of the same authenticated selected-law v7
//! operations. The adapter owns neither a Project mutation nor a proof token.
use super::{
    codec::{self, RequestId, RequestKind},
    config::ServerConfig,
    framing::{Frame, FrameReader, FrameWriter, StdioLimits},
    selected_law,
};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};

const MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MAX_MCP_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const STATUS_TOOL: &str = "law__status";
const CHECK_TOOL: &str = "law__check";

#[derive(Clone, Copy, Eq, PartialEq)]
enum Lifecycle {
    New,
    AwaitingInitialized,
    Ready,
}

pub(super) fn serve<R: BufRead, W: Write>(
    input: R,
    output: W,
    config: ServerConfig,
) -> io::Result<()> {
    selected_law::authenticate(config.manifest_path())?;
    let outer_limits = StdioLimits::new(config.limits().request_bytes(), MAX_MCP_RESPONSE_BYTES)
        .expect("fixed MCP response bound is within stdio limits");
    let mut input = FrameReader::new(input, outer_limits);
    let mut output = FrameWriter::new(output, outer_limits);
    let mut lifecycle = Lifecycle::New;
    loop {
        let frame = match input.read_frame()? {
            Frame::Eof => break,
            Frame::OversizedTerminal => {
                let response = codec::bounded_error_response(
                    None,
                    codec::PARSE_ERROR,
                    "MCP request exceeds configured byte limit",
                    outer_limits.response_bytes(),
                );
                output.write_response(&response)?;
                break;
            }
            Frame::Data(frame) if frame.is_empty() => continue,
            Frame::Data(frame) => frame,
        };
        let request = match codec::decode_request(&frame) {
            Ok(request) => request,
            Err(error) => {
                if !error.suppress_response {
                    output.write_response(&codec::bounded_error_response(
                        error.response_id.as_ref(),
                        error.code,
                        &error.message,
                        outer_limits.response_bytes(),
                    ))?;
                }
                continue;
            }
        };
        let Some(id) = (match request.kind {
            RequestKind::Call(id) => Some(id),
            RequestKind::Notification => None,
        }) else {
            if request.method == "notifications/initialized"
                && lifecycle == Lifecycle::AwaitingInitialized
                && request.params.as_ref().is_none_or(Map::is_empty)
            {
                lifecycle = Lifecycle::Ready;
            }
            continue;
        };
        let response = dispatch(
            &id,
            &request.method,
            request.params,
            &config,
            &mut lifecycle,
        );
        let overflow = codec::is_overflow_response(&response);
        output.write_response(&response)?;
        if overflow {
            break;
        }
    }
    Ok(())
}

fn dispatch(
    id: &RequestId,
    method: &str,
    params: Option<Map<String, Value>>,
    config: &ServerConfig,
    lifecycle: &mut Lifecycle,
) -> Vec<u8> {
    let bound = MAX_MCP_RESPONSE_BYTES;
    match method {
        "initialize" if *lifecycle == Lifecycle::New => {
            let Some(params) = params else {
                return invalid_params(id);
            };
            if params.keys().any(|key| {
                !matches!(
                    key.as_str(),
                    "protocolVersion" | "capabilities" | "clientInfo" | "_meta"
                )
            }) || !params.get("protocolVersion").is_some_and(Value::is_string)
                || !params.get("capabilities").is_some_and(Value::is_object)
                || !params.get("clientInfo").is_some_and(|info| {
                    info.get("name").is_some_and(Value::is_string)
                        && info.get("version").is_some_and(Value::is_string)
                })
                || params.get("_meta").is_some_and(|meta| !meta.is_object())
            {
                return invalid_params(id);
            }
            *lifecycle = Lifecycle::AwaitingInitialized;
            success(
                id,
                &json!({
                    "protocolVersion":MCP_PROTOCOL_VERSION,
                    "capabilities":{"tools":{"listChanged":false}},
                    "serverInfo":{"name":"semaprax-selected-law","version":env!("CARGO_PKG_VERSION")}
                }),
            )
        }
        "initialize" => error(id, -32000, "MCP session is already initialized"),
        "ping" if empty_or_meta(params.as_ref()) => success(id, &json!({})),
        "ping" => invalid_params(id),
        "tools/list" | "tools/call" if *lifecycle != Lifecycle::Ready => {
            error(id, -32000, "MCP initialization is not complete")
        }
        "tools/list" if empty_or_meta(params.as_ref()) => {
            success(id, &json!({"tools":tool_catalog()}))
        }
        "tools/list" => invalid_params(id),
        "tools/call" => {
            let Some(mut params) = params else {
                return invalid_params(id);
            };
            params.remove("_meta");
            if params
                .keys()
                .any(|key| !matches!(key.as_str(), "name" | "arguments"))
            {
                return invalid_params(id);
            }
            let method = match params.get("name").and_then(Value::as_str) {
                Some(STATUS_TOOL) => "law/status",
                Some(CHECK_TOOL) => "law/check",
                _ => return invalid_params(id),
            };
            let arguments = match params.get("arguments") {
                None => Map::new(),
                Some(Value::Object(arguments)) => arguments.clone(),
                _ => return invalid_params(id),
            };
            // The inner v7 response is preserved as one MCP text item. The
            // existing selected-law dispatch rechecks current host policy and
            // candidate bytes, and the exact inner result/error remains visible.
            let inner = selected_law::dispatch(
                &RequestId::Number(0),
                method,
                Some(arguments),
                config,
                config.manifest_path(),
                config.law_tool().expect("selected tool is startup-pinned"),
            );
            let decoded: Value =
                serde_json::from_slice(&inner).expect("bounded v7 dispatch always returns JSON");
            let is_error = decoded.get("error").is_some();
            let inner = String::from_utf8(inner).expect("bounded v7 response is UTF-8");
            success(
                id,
                &json!({
                    "content":[{"type":"text","text":inner}],
                    "isError":is_error
                }),
            )
        }
        _ => codec::bounded_error_response(Some(id), -32601, "method not found", bound),
    }
}

fn empty_or_meta(params: Option<&Map<String, Value>>) -> bool {
    params.is_none_or(|params| {
        params.is_empty() || params.len() == 1 && params.get("_meta").is_some_and(Value::is_object)
    })
}

fn tool_catalog() -> [Value; 2] {
    [
        json!({
            "name":STATUS_TOOL,
            "title":"Selected law status",
            "description":"Return current authenticated Project revision and selected law policy digests. No proof is granted.",
            "inputSchema":{"type":"object","properties":{},"additionalProperties":false}
        }),
        json!({
            "name":CHECK_TOOL,
            "title":"Check one selected law",
            "description":"Run the host-pinned installed tool against the exact current selected law; return bounded strict summary or detail with candidate-bound nonproof outcomes.",
            "inputSchema":{"type":"object","properties":{
                "candidate_revision":{"type":"string"},"law_id":{"type":"string"},
                "view":{"enum":["summary","detail"]},"source":{"type":"string"},
                "declaration":{"type":"string"},"ensures_index":{"type":"integer","minimum":0},
                "offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":0},
                "max_bytes":{"type":"integer","minimum":2048,"maximum":65536},
                "show_witness_values":{"type":"boolean"}
            },"required":["candidate_revision","law_id","view"],"additionalProperties":false}
        }),
    ]
}

fn success(id: &RequestId, value: &Value) -> Vec<u8> {
    codec::bounded_success_response(id, &value.to_string(), MAX_MCP_RESPONSE_BYTES)
}

fn error(id: &RequestId, code: i64, message: &str) -> Vec<u8> {
    codec::bounded_error_response(Some(id), code, message, MAX_MCP_RESPONSE_BYTES)
}

fn invalid_params(id: &RequestId) -> Vec<u8> {
    error(
        id,
        codec::INVALID_PARAMS,
        "invalid selected-law MCP parameters",
    )
}
