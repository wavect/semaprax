//! Bounded authority-free JSON-lines adapter for one persistent semantic service.

mod compact;

use std::io::{self, BufRead, Write};
use std::sync::Arc;

use serde_json::{json, Map, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{
    ProjectFrontendSource, ProjectManifest, ProjectRevision, SemanticWorkspaceService,
    MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS,
    MAX_SEMANTIC_WORKSPACE_SERVICE_PATCH_RECEIPT_COMPARISON_INPUTS, MAX_SOURCES,
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
            "workspace/index-query" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["query"])?;
                let query = take_string(&mut params, "query")?;
                let result = self.service.index_query(query.as_bytes())?;
                self.wrap(json!({
                    "query_digest": result.query_digest(),
                    "result_digest": result.result_digest(),
                    "value": exact_json(result.to_json())?,
                }))
            }
            "workspace/history-query" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["query"])?;
                let query = take_string(&mut params, "query")?;
                let result = self.service.history_query(query.as_bytes())?;
                self.wrap(json!({
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
            "workspace/validate-transaction-v2" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["transaction"])?;
                let transaction = take_string(&mut params, "transaction")?;
                let artifacts = self
                    .service
                    .validate_transaction_v2(transaction.as_bytes())?;
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
            "workspace/validate-transaction-v2-workflow" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["steps"])?;
                let steps = take_step_array(&mut params)?;
                let workflow = self.service.validate_transaction_v2_workflow(&steps)?;
                self.wrap(json!({
                    "candidate_revision": workflow.candidate().revision().project_revision(),
                    "digest": workflow.digest(),
                    "value": exact_json(workflow.to_json())?,
                }))
            }
            "workspace/patch-receipt" => {
                self.require_open()?;
                let mut params =
                    closed_params(request.params, &["transaction", "candidate_digest"])?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "candidate_digest")?;
                let receipt = self
                    .service
                    .patch_receipt(transaction.as_bytes(), &candidate)?;
                self.wrap(json!({"value": exact_json(&receipt)?}))
            }
            "workspace/verify-patch-receipt" => {
                self.require_open()?;
                let mut params = closed_params(
                    request.params,
                    &["transaction", "candidate_digest", "receipt"],
                )?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "candidate_digest")?;
                let receipt = take_string(&mut params, "receipt")?;
                let verified = self.service.verify_patch_receipt(
                    transaction.as_bytes(),
                    &candidate,
                    receipt.as_bytes(),
                )?;
                self.wrap(json!({"value": exact_json(&verified)?}))
            }
            "workspace/patch-receipt-refusal" => {
                self.require_open()?;
                let mut params = closed_params(
                    request.params,
                    &["transaction", "requested_candidate_digest"],
                )?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "requested_candidate_digest")?;
                let receipt = self
                    .service
                    .patch_receipt_refusal(transaction.as_bytes(), &candidate)?;
                self.wrap(json!({"value": exact_json(&receipt)?}))
            }
            "workspace/verify-patch-receipt-refusal" => {
                self.require_open()?;
                let mut params = closed_params(
                    request.params,
                    &["transaction", "requested_candidate_digest", "receipt"],
                )?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "requested_candidate_digest")?;
                let receipt = take_string(&mut params, "receipt")?;
                let verified = self.service.verify_patch_receipt_refusal(
                    transaction.as_bytes(),
                    &candidate,
                    receipt.as_bytes(),
                )?;
                self.wrap(json!({"value": exact_json(&verified)?}))
            }
            "workspace/patch-receipt-evidence-summary" => {
                self.require_open()?;
                let mut params =
                    closed_params(request.params, &["transaction", "candidate_digest"])?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "candidate_digest")?;
                let summary = self
                    .service
                    .patch_receipt_evidence_summary(transaction.as_bytes(), &candidate)?;
                self.wrap(json!({"value": exact_json(&summary)?}))
            }
            "workspace/patch-receipt-evidence-page" => {
                self.require_open()?;
                let mut params = closed_params(
                    request.params,
                    &[
                        "transaction",
                        "candidate_digest",
                        "evidence_id",
                        "handle",
                        "cursor",
                        "page_size",
                        "max_bytes",
                    ],
                )?;
                let transaction = take_string(&mut params, "transaction")?;
                let candidate = take_string(&mut params, "candidate_digest")?;
                let evidence_id = take_string(&mut params, "evidence_id")?;
                let handle = take_string(&mut params, "handle")?;
                let cursor = match params.remove("cursor") {
                    Some(Value::Null) => None,
                    Some(Value::String(value)) => Some(value),
                    _ => return Err(invalid("cursor must be a string or null")),
                };
                let page_size = take_usize(&mut params, "page_size")?;
                let max_bytes = take_usize(&mut params, "max_bytes")?;
                let page = self.service.patch_receipt_evidence_page(
                    transaction.as_bytes(),
                    &candidate,
                    &evidence_id,
                    &handle,
                    cursor.as_deref(),
                    page_size,
                    max_bytes,
                )?;
                self.wrap(json!({"value": exact_json(&page)?}))
            }
            "workspace/compare-patch-receipts" => {
                self.require_open()?;
                let mut params = closed_params(
                    request.params,
                    &[
                        "left_transaction",
                        "left_candidate_digest",
                        "left_receipt",
                        "right_transaction",
                        "right_candidate_digest",
                        "right_receipt",
                    ],
                )?;
                let left_transaction = take_string(&mut params, "left_transaction")?;
                let left_candidate = take_string(&mut params, "left_candidate_digest")?;
                let left_receipt = take_string(&mut params, "left_receipt")?;
                let right_transaction = take_string(&mut params, "right_transaction")?;
                let right_candidate = take_string(&mut params, "right_candidate_digest")?;
                let right_receipt = take_string(&mut params, "right_receipt")?;
                let comparison = self.service.compare_patch_receipts(
                    left_transaction.as_bytes(),
                    &left_candidate,
                    left_receipt.as_bytes(),
                    right_transaction.as_bytes(),
                    &right_candidate,
                    right_receipt.as_bytes(),
                )?;
                self.wrap(json!({"value": exact_json(&comparison)?}))
            }
            "workspace/compare-patch-receipt-set" => {
                self.require_open()?;
                let mut params = closed_params(request.params, &["receipts"])?;
                let receipts = params
                    .remove("receipts")
                    .and_then(|value| value.as_array().cloned())
                    .ok_or_else(|| invalid("receipts must be an array"))?;
                if receipts.len() < 2
                    || receipts.len()
                        > MAX_SEMANTIC_WORKSPACE_SERVICE_PATCH_RECEIPT_COMPARISON_INPUTS
                {
                    return Err(capacity("receipts exceed transport comparison count limit"));
                }
                let entries = receipts
                    .into_iter()
                    .map(|receipt| {
                        let mut receipt = receipt
                            .as_object()
                            .cloned()
                            .ok_or_else(|| invalid("each receipt must be an object"))?;
                        if receipt.len() != 3
                            || !receipt.contains_key("transaction")
                            || !receipt.contains_key("candidate_digest")
                            || !receipt.contains_key("receipt")
                        {
                            return Err(invalid("receipt has missing or unknown members"));
                        }
                        Ok((
                            take_string(&mut receipt, "transaction")?,
                            take_string(&mut receipt, "candidate_digest")?,
                            take_string(&mut receipt, "receipt")?,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?;
                let inputs = entries
                    .iter()
                    .map(|(transaction, candidate, receipt)| {
                        (
                            transaction.as_bytes(),
                            candidate.as_str(),
                            receipt.as_bytes(),
                        )
                    })
                    .collect::<Vec<_>>();
                let comparison = self.service.compare_patch_receipt_set(&inputs)?;
                self.wrap(json!({"value": exact_json(&comparison)?}))
            }
            "workspace/compact-projection" => {
                self.require_open()?;
                self.wrap(compact::project(&self.service, request.params)?)
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
            "workspace/index-query", "workspace/history-query", "workspace/validate-transaction",
            "workspace/validate-transaction-v2", "workspace/validate-transaction-v2-workflow",
            "workspace/patch-receipt", "workspace/verify-patch-receipt",
            "workspace/patch-receipt-refusal", "workspace/verify-patch-receipt-refusal",
            "workspace/patch-receipt-evidence-summary", "workspace/patch-receipt-evidence-page",
            "workspace/compare-patch-receipts", "workspace/compare-patch-receipt-set",
            "workspace/compact-projection",
            "workspace/refresh", "shutdown"
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

fn take_usize(params: &mut Map<String, Value>, key: &str) -> Result<usize> {
    params
        .remove(key)
        .and_then(|value| value.as_u64())
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| invalid("parameter must be a nonnegative bounded integer"))
}

/// Take `steps` as a bounded array of canonical v2 transaction strings, each
/// one caller-owned bytes exactly like `take_string`'s own NUL-byte bound.
fn take_step_array(params: &mut Map<String, Value>) -> Result<Vec<Vec<u8>>> {
    let values = params
        .remove("steps")
        .and_then(|value| value.as_array().cloned())
        .ok_or_else(|| invalid("steps must be an array"))?;
    if values.is_empty() || values.len() > MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS {
        return Err(capacity("steps exceed transport count limit"));
    }
    values
        .into_iter()
        .map(|value| match value {
            Value::String(step) if !step.as_bytes().contains(&0) => Ok(step.into_bytes()),
            _ => Err(invalid(
                "each workflow step must be a string without NUL bytes",
            )),
        })
        .collect()
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
