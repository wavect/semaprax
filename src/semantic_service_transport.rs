//! Bounded authority-free JSON-lines adapter for one persistent semantic service.

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectFrontendSource, ProjectManifest, ProjectRevision, SemanticWorkspaceService, MAX_SOURCES,
};
use crate::project_transport::codec::{self, RequestId, RequestKind, RpcRequest};

pub const SEMANTIC_SERVICE_TRANSPORT_SCHEMA: &str =
    "semaprax.semantic-workspace-service-transport.v1";
pub const SEMANTIC_SERVICE_TRANSPORT_RESULT_SCHEMA: &str =
    "semaprax.semantic-workspace-service-transport-result.v1";
pub const SEMANTIC_SERVICE_TRANSPORT_ERROR_SCHEMA: &str =
    "semaprax.semantic-workspace-service-transport-error.v1";
pub const MAX_SEMANTIC_SERVICE_REQUEST_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_SEMANTIC_SERVICE_RESPONSE_BYTES: usize = 128 * 1024 * 1024;
const MAX_MANIFEST_INPUT_BYTES: usize = 65_536;
const MAX_DIAGNOSTICS: usize = 64;

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Stateful single-client protocol session. All source and manifest bytes are
/// caller-owned; this type has no filesystem, network, process, or publication API.
pub struct SemanticWorkspaceStdioSession {
    service: SemanticWorkspaceService,
    opened: bool,
    terminal: bool,
}

impl SemanticWorkspaceStdioSession {
    pub fn open(revision: Arc<ProjectRevision>) -> Result<Self> {
        Ok(Self {
            service: SemanticWorkspaceService::open(revision)?,
            opened: false,
            terminal: false,
        })
    }

    pub fn service(&self) -> &SemanticWorkspaceService {
        &self.service
    }

    pub fn is_terminal(&self) -> bool {
        self.terminal
    }

    /// Handle one frame without its trailing LF. Notifications intentionally
    /// produce no response. Only a shutdown notification changes session state.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        if frame.len() > MAX_SEMANTIC_SERVICE_REQUEST_BYTES {
            return Some(codec::bounded_error_response(
                None,
                -32001,
                "request exceeds configured byte limit",
                MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
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
                        MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
                    )
                });
            }
        };
        if matches!(request.kind, RequestKind::Notification) {
            if request.method == "shutdown" {
                self.terminal = true;
            }
            return None;
        }
        let RequestKind::Call(id) = request.kind.clone() else {
            unreachable!()
        };
        if self.terminal {
            return Some(self.application_error(&id, invalid("session is shut down")));
        }
        match self.dispatch(request) {
            Ok(value) => Some(codec::bounded_success_response(
                &id,
                &value.to_string(),
                MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
            )),
            Err(diagnostics) => Some(self.application_error(&id, diagnostics)),
        }
    }

    fn dispatch(&mut self, request: RpcRequest) -> Result<Value> {
        match request.method.as_str() {
            "service/protocol" => {
                require_no_params(request.params)?;
                Ok(protocol())
            }
            "workspace/open" => {
                require_no_params(request.params)?;
                self.opened = true;
                let work = self.service.open_work();
                self.wrap(json!({
                    "receipt_digest": work.receipt_digest(),
                    "value": exact_json(work.to_json())?,
                }))
            }
            "workspace/status" => {
                require_no_params(request.params)?;
                self.wrap(json!({"opened": self.opened, "state": "ready"}))
            }
            "workspace/query" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["query"])?;
                let query = take_string(&mut params, "query")?;
                let result = self.service.query(query.as_bytes())?;
                self.wrap(json!({
                    "payload_digest": result.payload_digest(),
                    "query_digest": result.query_digest(),
                    "result_digest": result.result_digest(),
                    "value": exact_json(result.to_json())?,
                }))
            }
            "workspace/validate-transaction" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["transaction"])?;
                let transaction = take_string(&mut params, "transaction")?;
                let artifacts = self.service.validate_transaction(transaction.as_bytes())?;
                self.wrap(json!({
                    "candidate_revision": artifacts.candidate().revision().project_revision(),
                    "evidence": exact_json(artifacts.evidence())?,
                    "impact": exact_json(artifacts.impact())?,
                    "impact_digest": artifacts.impact_digest(),
                    "result": exact_json(artifacts.result())?,
                    "result_digest": artifacts.result_digest(),
                    "review": exact_json(artifacts.review())?,
                    "review_digest": artifacts.review_digest(),
                }))
            }
            "workspace/refresh" => self.refresh(request.params),
            "shutdown" => {
                require_no_params(request.params)?;
                self.terminal = true;
                self.wrap(json!({"shutdown": true}))
            }
            _ => Err(invalid("unknown semantic workspace service method")),
        }
    }

    fn refresh(&mut self, params: Option<Map<String, Value>>) -> Result<Value> {
        self.require_open()?;
        let mut params = closed_params(
            params,
            &["expected_workspace_revision", "manifest", "sources"],
        )?;
        let expected = take_string(&mut params, "expected_workspace_revision")?;
        let manifest_text = take_string(&mut params, "manifest")?;
        if manifest_text.len() > MAX_MANIFEST_INPUT_BYTES {
            return Err(capacity("manifest exceeds transport byte limit"));
        }
        let manifest = ProjectManifest::parse(&manifest_text)?;
        if manifest.to_canonical_toml() != manifest_text {
            return Err(invalid("manifest must be exact canonical TOML"));
        }
        let values = params
            .remove("sources")
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| invalid("sources must be an array"))?;
        if values.len() > MAX_SOURCES {
            return Err(capacity("sources exceed transport count limit"));
        }
        let mut sources = Vec::with_capacity(values.len());
        for value in values {
            let Value::Object(object) = value else {
                return Err(invalid("each source must be an object"));
            };
            let mut source = closed_params(Some(object), &["path", "source"])?;
            let path = take_string(&mut source, "path")?;
            let text = take_string(&mut source, "source")?;
            sources.push(ProjectFrontendSource::new(&path, &text)?);
        }
        let receipt = self
            .service
            .refresh_owned_sources(&manifest, &sources, &expected)?;
        self.wrap(json!({
            "generation_reused": receipt.generation_reused(),
            "old_workspace_revision": receipt.old_workspace_revision(),
            "receipt_digest": receipt.receipt_digest(),
            "value": exact_json(receipt.to_json())?,
        }))
    }

    fn require_open(&self) -> Result<()> {
        if self.opened {
            Ok(())
        } else {
            Err(invalid("workspace/open must succeed before this method"))
        }
    }

    fn wrap(&self, payload: Value) -> Result<Value> {
        let generation = self.service.active_generation();
        Ok(json!({
            "authority": false,
            "image_digest": generation.image().image_digest(),
            "payload": payload,
            "project_revision": generation.revision().project_revision(),
            "protocol": SEMANTIC_SERVICE_TRANSPORT_SCHEMA,
            "schema": SEMANTIC_SERVICE_TRANSPORT_RESULT_SCHEMA,
            "workspace_revision": generation.workspace_revision(),
        }))
    }

    fn application_error(&self, id: &RequestId, diagnostics: Vec<Diagnostic>) -> Vec<u8> {
        let diagnostics = diagnostics
            .into_iter()
            .take(MAX_DIAGNOSTICS)
            .map(|diagnostic| exact_json(&diagnostic.json()).unwrap_or(Value::Null))
            .collect::<Vec<_>>();
        let data = json!({
            "authority": false,
            "diagnostics": diagnostics,
            "protocol": SEMANTIC_SERVICE_TRANSPORT_SCHEMA,
            "schema": SEMANTIC_SERVICE_TRANSPORT_ERROR_SCHEMA,
        });
        codec::bounded_application_error_response_with_data(
            id,
            "semantic workspace service request failed",
            &data.to_string(),
            MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
        )
    }
}

/// Serve repeated LF-delimited JSON-RPC calls until EOF or shutdown.
pub fn serve_semantic_workspace_stdio<R: BufRead, W: Write>(
    mut input: R,
    mut output: W,
    revision: Arc<ProjectRevision>,
) -> io::Result<()> {
    let mut session = SemanticWorkspaceStdioSession::open(revision).map_err(|diagnostics| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            diagnostics
                .first()
                .map_or("service open failed", |diagnostic| {
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
        if session.is_terminal() {
            return Ok(());
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
        if frame.len().saturating_add(content.len()) <= MAX_SEMANTIC_SERVICE_REQUEST_BYTES {
            frame.extend_from_slice(content);
        } else {
            frame.resize(MAX_SEMANTIC_SERVICE_REQUEST_BYTES + 1, 0);
        }
        input.consume(used);
        if newline.is_some() {
            return Ok(Some(frame));
        }
    }
}

fn protocol() -> Value {
    json!({
        "authority": false,
        "host_grants": [],
        "limits": {
            "max_diagnostics": MAX_DIAGNOSTICS,
            "max_request_bytes": MAX_SEMANTIC_SERVICE_REQUEST_BYTES,
            "max_response_bytes": MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
        },
        "methods": [
            "service/protocol", "workspace/open", "workspace/status", "workspace/query",
            "workspace/validate-transaction", "workspace/refresh", "shutdown"
        ],
        "nonclaims": [
            "no_filesystem_network_process_or_publication_authority",
            "single_process_single_client_only",
            "not_socket_mcp_lsp_or_shared_multiprocess_service"
        ],
        "schema": SEMANTIC_SERVICE_TRANSPORT_SCHEMA,
    })
}

fn require_no_params(params: Option<Map<String, Value>>) -> Result<()> {
    if params.is_none_or(|params| params.is_empty()) {
        Ok(())
    } else {
        Err(invalid("method accepts no params"))
    }
}

fn closed_params(
    params: Option<Map<String, Value>>,
    allowed: &[&str],
) -> Result<Map<String, Value>> {
    let params = params.ok_or_else(|| invalid("method requires params"))?;
    if params.keys().any(|key| !allowed.contains(&key.as_str()))
        || allowed.iter().any(|key| !params.contains_key(*key))
    {
        return Err(invalid("params have missing or unknown members"));
    }
    Ok(params)
}

fn take_string(params: &mut Map<String, Value>, key: &str) -> Result<String> {
    match params.remove(key) {
        Some(Value::String(value)) if !value.as_bytes().contains(&0) => Ok(value),
        _ => Err(invalid("parameter must be a string without NUL bytes")),
    }
}

fn exact_json(text: &str) -> Result<Value> {
    serde_json::from_str(text).map_err(|_| invalid("core artifact is not valid JSON"))
}

fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G548", message)]
}

fn capacity(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G549", message)]
}
