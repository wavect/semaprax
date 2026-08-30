//! Self-describing, read-only Image Agent Protocol v1.
//!
//! The host binds one manifest and explicitly selects read-only authority.
//! Requests cannot name files, change capability, write source, or run targets.
//! Existing Graph and Project transport method sets remain unchanged.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{ImageFacet, ImageFacetOptions, ProjectSemanticImage, ProjectSnapshot};
use crate::project_transport::codec::{self, RequestKind};
use crate::project_transport::framing::{Frame, FrameReader, FrameWriter, StdioLimits};
use crate::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};

pub const PROTOCOL_SCHEMA: &str = "semaprax.image-agent-protocol.v1";
pub const RESULT_SCHEMA: &str = "semaprax.image-agent-result.v1";
pub const MAX_REQUEST_BYTES: usize = 64 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const MAX_QUERY_BYTES: usize = 512 * 1024;

/// Selected only by the trusted host at session construction. There is no
/// request parameter or method capable of adding authority to this profile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageHostCapability {
    ReadOnly,
}

#[derive(Clone, Copy)]
enum ParameterKind {
    Text(usize),
    Digest,
    Choice(&'static [&'static str]),
    Integer(usize, usize),
}

#[derive(Clone, Copy)]
struct Parameter {
    name: &'static str,
    kind: ParameterKind,
    required: bool,
}

const REVISION: Parameter = Parameter {
    name: "image_revision",
    kind: ParameterKind::Digest,
    required: true,
};
const TARGET: Parameter = Parameter {
    name: "target",
    kind: ParameterKind::Text(4096),
    required: true,
};
const TARGET_KIND: Parameter = Parameter {
    name: "target_kind",
    kind: ParameterKind::Choice(&["declaration", "capability"]),
    required: true,
};
const DEPTH: Parameter = Parameter {
    name: "depth",
    kind: ParameterKind::Integer(0, 1024),
    required: false,
};
const BYTES: Parameter = Parameter {
    name: "max_bytes",
    kind: ParameterKind::Integer(4096, MAX_QUERY_BYTES),
    required: false,
};
const NODES: Parameter = Parameter {
    name: "max_nodes",
    kind: ParameterKind::Integer(1, 8208),
    required: false,
};

#[derive(Clone, Copy)]
enum Operation {
    Capabilities,
    Schemas,
    Instructions,
    Client,
    Open,
    Status,
    Catalog,
    Symbol,
    Context,
    Impact,
    FunctionSummary,
    Facet,
}

struct Method {
    name: &'static str,
    operation: Operation,
    parameters: &'static [Parameter],
    query: bool,
    payload_schema: &'static str,
}

// The same catalog admits dispatch parameters, generates request/response
// schemas, and supplies the SDK method names. No second writable registry.
const METHODS: &[Method] = &[
    Method {
        name: "image/context",
        operation: Operation::Context,
        parameters: &[
            REVISION,
            TARGET_KIND,
            TARGET,
            Parameter {
                name: "direction",
                kind: ParameterKind::Choice(&["forward", "reverse", "both"]),
                required: false,
            },
            DEPTH,
            BYTES,
            NODES,
        ],
        query: true,
        payload_schema: "semaprax.project-semantic-context.v1",
    },
    Method {
        name: "image/facet",
        operation: Operation::Facet,
        parameters: &[
            REVISION,
            TARGET,
            Parameter {
                name: "facet",
                kind: ParameterKind::Choice(&[
                    "signature",
                    "contracts",
                    "callers",
                    "ownership",
                    "loans",
                    "cleanup",
                    "relationships",
                ]),
                required: true,
            },
            Parameter {
                name: "handle",
                kind: ParameterKind::Digest,
                required: true,
            },
            Parameter {
                name: "cursor",
                kind: ParameterKind::Text(100),
                required: false,
            },
            Parameter {
                name: "page_size",
                kind: ParameterKind::Integer(1, 128),
                required: false,
            },
            Parameter {
                name: "max_bytes",
                kind: ParameterKind::Integer(1024, MAX_QUERY_BYTES),
                required: false,
            },
        ],
        query: true,
        payload_schema: "semaprax.image-facet.v1",
    },
    Method {
        name: "image/function-summary",
        operation: Operation::FunctionSummary,
        parameters: &[REVISION, TARGET],
        query: true,
        payload_schema: "semaprax.image-function-summary.v1",
    },
    Method {
        name: "image/impact",
        operation: Operation::Impact,
        parameters: &[REVISION, TARGET_KIND, TARGET, DEPTH, BYTES, NODES],
        query: true,
        payload_schema: "semaprax.project-semantic-impact.v1",
    },
    Method {
        name: "image/symbol",
        operation: Operation::Symbol,
        parameters: &[
            REVISION,
            Parameter {
                name: "stable_id",
                kind: ParameterKind::Text(4096),
                required: true,
            },
        ],
        query: true,
        payload_schema: "semaprax.semantic-workspace-image-symbol.v1",
    },
    Method {
        name: "protocol/capabilities",
        operation: Operation::Capabilities,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-capabilities.v1",
    },
    Method {
        name: "protocol/client",
        operation: Operation::Client,
        parameters: &[Parameter {
            name: "language",
            kind: ParameterKind::Choice(&["typescript", "python", "rust"]),
            required: true,
        }],
        query: false,
        payload_schema: "semaprax.image-agent-client.v1",
    },
    Method {
        name: "protocol/instructions",
        operation: Operation::Instructions,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-instructions.v1",
    },
    Method {
        name: "protocol/schemas",
        operation: Operation::Schemas,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-schemas.v1",
    },
    Method {
        name: "query/catalog",
        operation: Operation::Catalog,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-query-catalog.v1",
    },
    Method {
        name: "workspace/open",
        operation: Operation::Open,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-workspace.v1",
    },
    Method {
        name: "workspace/status",
        operation: Operation::Status,
        parameters: &[],
        query: false,
        payload_schema: "semaprax.image-agent-workspace.v1",
    },
];

/// One retained authenticated Project and its authority-free immutable image.
pub struct ImageSession {
    snapshot: ProjectSnapshot,
    image: Arc<ProjectSemanticImage>,
    terminal: bool,
}

impl ImageSession {
    pub fn open(manifest: &Path, capability: ImageHostCapability) -> Result<Self, Vec<Diagnostic>> {
        let ImageHostCapability::ReadOnly = capability;
        let mut snapshot = crate::project::load_snapshot(manifest)?;
        let image = snapshot.with_authenticated_request(|snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })?;
        Ok(Self {
            snapshot,
            image: Arc::new(image),
            terminal: false,
        })
    }

    pub fn image_revision(&self) -> &str {
        self.image.image_digest()
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Handle one raw frame without LF. A successful payload is rendered fully
    /// before the final held-input recheck; only then is a response returned.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        if self.terminal || frame.is_empty() {
            return None;
        }
        if frame.len() > MAX_REQUEST_BYTES {
            self.terminal = true;
            return Some(codec::bounded_error_response(
                None,
                codec::PARSE_ERROR,
                "request exceeds configured byte limit",
                MAX_RESPONSE_BYTES,
            ));
        }
        let request = match codec::decode_request(frame) {
            Ok(request) => request,
            Err(error) => {
                return (!error.suppress_response).then(|| {
                    codec::bounded_error_response(
                        error.response_id.as_ref(),
                        error.code,
                        &error.message,
                        MAX_RESPONSE_BYTES,
                    )
                })
            }
        };
        // Notifications are silent and cannot trigger semantic work.
        let RequestKind::Call(id) = request.kind else {
            return None;
        };
        let Some(method) = METHODS.iter().find(|method| method.name == request.method) else {
            return Some(codec::bounded_error_response(
                Some(&id),
                -32601,
                "method is not available in the read-only image profile",
                MAX_RESPONSE_BYTES,
            ));
        };
        let params = request.params.unwrap_or_default();
        if let Err(message) = validate_parameters(method, &params) {
            return Some(codec::bounded_error_response(
                Some(&id),
                codec::INVALID_PARAMS,
                &message,
                MAX_RESPONSE_BYTES,
            ));
        }
        let image = Arc::clone(&self.image);
        let result = self.snapshot.with_authenticated_request(|_| {
            let payload = dispatch(method, &params, &image)?;
            let mut result = json!({
                "schema": RESULT_SCHEMA,
                "protocol": PROTOCOL_SCHEMA,
                "image_revision": image.image_digest(),
                "project_revision": image.revision().project_revision(),
                "payload": payload,
            });
            // Keep envelope, schemas, and nested semantic payloads canonical
            // if a downstream crate enables serde_json's preserve_order.
            result.sort_all_objects();
            Ok(result.to_string())
        });
        let response = match result {
            Ok(result) => codec::bounded_success_response(&id, &result, MAX_RESPONSE_BYTES),
            Err(errors) => codec::bounded_error_response(
                Some(&id),
                -32000,
                &diagnostics(&errors),
                MAX_RESPONSE_BYTES,
            ),
        };
        if codec::is_overflow_response(&response) {
            self.terminal = true;
        }
        Some(response)
    }

    /// Reauthenticate at the host's final session boundary as well.
    pub fn finish(&mut self) -> Result<(), Vec<Diagnostic>> {
        self.snapshot.with_authenticated_request(|_| Ok(()))
    }
}

/// Serve bounded NDJSON from host-supplied streams; stdout has no banner.
pub fn serve<R: BufRead, W: Write>(
    input: R,
    output: W,
    manifest: &Path,
    capability: ImageHostCapability,
) -> io::Result<()> {
    let mut session = ImageSession::open(manifest, capability)
        .map_err(|errors| io::Error::other(diagnostics(&errors)))?;
    let limits =
        StdioLimits::new(MAX_REQUEST_BYTES, MAX_RESPONSE_BYTES).expect("fixed limits are admitted");
    let mut input = FrameReader::new(input, limits);
    let mut output = FrameWriter::new(output, limits);
    let result = (|| {
        loop {
            let response = match input.read_frame()? {
                Frame::Eof => break,
                Frame::OversizedTerminal => {
                    output.write_response(&codec::bounded_error_response(
                        None,
                        codec::PARSE_ERROR,
                        "request exceeds configured byte limit",
                        MAX_RESPONSE_BYTES,
                    ))?;
                    break;
                }
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
    let final_check = session
        .finish()
        .map_err(|errors| io::Error::other(diagnostics(&errors)));
    result.and(final_check)
}

fn diagnostics(errors: &[Diagnostic]) -> String {
    errors
        .iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn validate_parameters(method: &Method, params: &Map<String, Value>) -> Result<(), String> {
    if params.keys().any(|key| {
        !method
            .parameters
            .iter()
            .any(|parameter| parameter.name == key)
    }) {
        return Err("unknown parameter for image protocol method".to_owned());
    }
    for parameter in method.parameters {
        let Some(value) = params.get(parameter.name) else {
            if parameter.required {
                return Err(format!("missing parameter {}", parameter.name));
            }
            continue;
        };
        let valid = match parameter.kind {
            ParameterKind::Text(limit) => value.as_str().is_some_and(|text| {
                !text.is_empty() && text.len() <= limit && !text.chars().any(char::is_control)
            }),
            ParameterKind::Digest => value.as_str().is_some_and(|text| {
                text.len() == 71
                    && text.starts_with("sha256:")
                    && text.as_bytes()[7..]
                        .iter()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
            }),
            ParameterKind::Choice(choices) => {
                value.as_str().is_some_and(|text| choices.contains(&text))
            }
            ParameterKind::Integer(min, max) => value
                .as_u64()
                .is_some_and(|number| number >= min as u64 && number <= max as u64),
        };
        if !valid {
            return Err(format!("invalid parameter {}", parameter.name));
        }
    }
    Ok(())
}

fn text<'a>(params: &'a Map<String, Value>, key: &str) -> &'a str {
    params[key].as_str().expect("catalog validated string")
}

fn number(params: &Map<String, Value>, key: &str, default: usize) -> usize {
    params
        .get(key)
        .and_then(Value::as_u64)
        .map_or(default, |number| number as usize)
}

fn dispatch(
    method: &Method,
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    let value = match method.operation {
        Operation::Capabilities => {
            json!({"schema": method.payload_schema, "protocol": PROTOCOL_SCHEMA, "capabilities": ["semantic_read"], "methods": method_names(), "max_request_bytes": MAX_REQUEST_BYTES, "max_response_bytes": MAX_RESPONSE_BYTES, "source_authority": false})
        }
        Operation::Open | Operation::Status => {
            json!({"schema": method.payload_schema, "state": "open", "image_revision": image.image_digest(), "project_revision": image.revision().project_revision(), "workspace_revision": image.revision().workspace_revision()})
        }
        Operation::Catalog => {
            json!({"schema": method.payload_schema, "queries": METHODS.iter().filter(|method| method.query).map(method_description).collect::<Vec<_>>()})
        }
        Operation::Schemas => {
            json!({"schema": method.payload_schema, "protocol": PROTOCOL_SCHEMA, "methods": METHODS.iter().map(method_description).collect::<Vec<_>>()})
        }
        Operation::Instructions => {
            json!({"schema": method.payload_schema, "protocol": PROTOCOL_SCHEMA, "instructions": "Read protocol/capabilities and protocol/schemas, then workspace/open. Select only query/catalog methods and send the exact image_revision on every image query. Treat results as derived read-only facts. Source drift invalidates this session; ask the host to open a new session. Requests cannot change files, select paths, execute targets, or elevate authority. Close stdin to end the session."})
        }
        Operation::Client => {
            json!({"schema": method.payload_schema, "protocol": PROTOCOL_SCHEMA, "language": text(params, "language"), "source": client_source(text(params, "language"))})
        }
        Operation::Symbol => {
            return parse_payload(
                image.symbol(text(params, "image_revision"), text(params, "stable_id"))?,
            )
        }
        Operation::FunctionSummary => {
            return parse_payload(
                image.function_summary(text(params, "image_revision"), text(params, "target"))?,
            )
        }
        Operation::Facet => {
            return parse_payload(image.expand_facet(
                text(params, "image_revision"),
                text(params, "target"),
                ImageFacet::parse(text(params, "facet"))?,
                text(params, "handle"),
                params.get("cursor").and_then(Value::as_str),
                ImageFacetOptions::new(
                    number(params, "page_size", 32),
                    number(params, "max_bytes", 65_536),
                )?,
            )?)
        }
        Operation::Context | Operation::Impact => {
            let kind = if text(params, "target_kind") == "declaration" {
                WorkspaceAnalysisTargetKind::Declaration
            } else {
                WorkspaceAnalysisTargetKind::Capability
            };
            let depth = number(
                params,
                "depth",
                if matches!(method.operation, Operation::Context) {
                    4
                } else {
                    16
                },
            );
            let bytes = number(params, "max_bytes", MAX_QUERY_BYTES);
            let nodes = number(params, "max_nodes", 1024);
            let result = if matches!(method.operation, Operation::Context) {
                let direction = match params
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("both")
                {
                    "forward" => WorkspaceAnalysisDirection::Forward,
                    "reverse" => WorkspaceAnalysisDirection::Reverse,
                    _ => WorkspaceAnalysisDirection::Both,
                };
                let options = WorkspaceContextOptions::new(direction, depth, bytes, nodes)
                    .map_err(|error| vec![error])?;
                image.context(
                    text(params, "image_revision"),
                    kind,
                    text(params, "target"),
                    options,
                )?
            } else {
                let options = WorkspaceImpactOptions::new(depth, bytes, nodes)
                    .map_err(|error| vec![error])?;
                image.impact(
                    text(params, "image_revision"),
                    kind,
                    text(params, "target"),
                    options,
                )?
            };
            return parse_payload(result);
        }
    };
    Ok(value)
}

fn parse_payload(payload: String) -> Result<Value, Vec<Diagnostic>> {
    serde_json::from_str(&payload).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G219",
            "compiler image payload is invalid",
        )]
    })
}

fn method_names() -> Vec<&'static str> {
    METHODS.iter().map(|method| method.name).collect()
}

fn method_description(method: &Method) -> Value {
    let properties = method.parameters.iter().map(|parameter| {
        let schema = match parameter.kind {
            ParameterKind::Text(max) => json!({"type":"string", "minLength":1, "maxLength":max, "x-max-utf8-bytes":max, "pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$"}),
            ParameterKind::Digest => json!({"type":"string", "pattern":"^sha256:[0-9a-f]{64}$"}),
            ParameterKind::Choice(choices) => json!({"type":"string", "enum":choices}),
            ParameterKind::Integer(min, max) => json!({"type":"integer", "minimum":min, "maximum":max}),
        };
        (parameter.name.to_owned(), schema)
    }).collect::<Map<_, _>>();
    let required = method
        .parameters
        .iter()
        .filter(|parameter| parameter.required)
        .map(|parameter| parameter.name)
        .collect::<Vec<_>>();
    let request_required = if required.is_empty() {
        vec!["jsonrpc", "method"]
    } else {
        vec!["jsonrpc", "method", "params"]
    };
    let params = json!({"type":"object", "additionalProperties":false, "properties":properties, "required":required});
    let id = json!({"oneOf":[{"type":"integer", "minimum":0, "maximum":u64::MAX}, {"type":"string", "minLength":1, "maxLength":128, "x-max-utf8-bytes":128, "pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$"}]});
    json!({
        "method":method.name,
        "capability":"semantic_read",
        "request_schema": {"$schema":"https://json-schema.org/draft/2020-12/schema", "type":"object", "additionalProperties":false, "required":request_required, "properties":{"jsonrpc":{"const":"2.0"}, "method":{"const":method.name}, "id":id, "params":params}},
        "success_response_schema": {"$schema":"https://json-schema.org/draft/2020-12/schema", "type":"object", "additionalProperties":false, "required":["jsonrpc","id","result"], "properties":{"jsonrpc":{"const":"2.0"},"id":id,"result":{"type":"object","additionalProperties":false,"required":["schema","protocol","image_revision","project_revision","payload"],"properties":{"schema":{"const":RESULT_SCHEMA},"protocol":{"const":PROTOCOL_SCHEMA},"image_revision":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"},"project_revision":{"type":"string","pattern":"^sha256:[0-9a-f]{64}$"},"payload":{"$ref":format!("urn:{}",method.payload_schema)}}}}},
        "error_response_schema": {"type":"object", "additionalProperties":false, "required":["jsonrpc","id","error"],"properties":{"jsonrpc":{"const":"2.0"},"id":{"anyOf":[id,{"type":"null"}]},"error":{"type":"object","additionalProperties":false,"required":["code","message"],"properties":{"code":{"type":"integer"},"message":{"type":"string"}}}}},
    })
}

fn client_source(language: &str) -> String {
    let methods = serde_json::to_string(&method_names()).expect("catalog strings serialize");
    match language {
        "python" => format!("# {PROTOCOL_SCHEMA}; host supplies transport, no file or network authority.\nimport json\nPROTOCOL = {PROTOCOL_SCHEMA:?}\nMETHODS = {methods}\ndef request(request_id, method, params=None):\n    if method not in METHODS:\n        raise ValueError('method unavailable')\n    return json.dumps(dict(jsonrpc='2.0', id=request_id, method=method, params=params or {{}}), separators=(',', ':')) + '\\n'\ndef result(line):\n    response = json.loads(line)\n    if 'error' in response:\n        raise ValueError(response['error'])\n    value = response['result']\n    if value['protocol'] != PROTOCOL:\n        raise ValueError('protocol mismatch')\n    return value\n"),
        "typescript" => format!("// {PROTOCOL_SCHEMA}; host supplies transport.\nexport const protocol = {PROTOCOL_SCHEMA:?};\nexport const methods = {methods} as const;\nexport type Method = typeof methods[number];\nexport function request(id: number | string, method: Method, params: Record<string, unknown> = {{}}): string {{\n  return JSON.stringify({{jsonrpc: '2.0', id, method, params}}) + '\\n';\n}}\nexport function result(line: string) {{\n  const response = JSON.parse(line);\n  if (response.error) throw new Error(JSON.stringify(response.error));\n  if (response.result.protocol !== protocol) throw new Error('protocol mismatch');\n  return response.result;\n}}\n"),
        _ => format!("// {PROTOCOL_SCHEMA}; requires serde_json, host supplies transport.\npub const PROTOCOL: &str = {PROTOCOL_SCHEMA:?};\npub const METHODS: &[&str] = &{methods};\npub fn request(id: u64, method: &str, params: serde_json::Value) -> Result<String, &'static str> {{\n    if !METHODS.contains(&method) {{ return Err(\"method unavailable\"); }}\n    Ok(serde_json::json!({{\"jsonrpc\":\"2.0\",\"id\":id,\"method\":method,\"params\":params}}).to_string() + \"\\n\")\n}}\npub fn result(line: &str) -> Result<serde_json::Value, String> {{\n    let value: serde_json::Value = serde_json::from_str(line).map_err(|error| error.to_string())?;\n    if let Some(error) = value.get(\"error\") {{ return Err(error.to_string()); }}\n    if value[\"result\"][\"protocol\"] != PROTOCOL {{ return Err(\"protocol mismatch\".into()); }}\n    Ok(value[\"result\"].clone())\n}}\n"),
    }
}
