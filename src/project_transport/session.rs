use std::io::{self, BufRead, Write};

use serde_json::{Map, Value};

use super::codec::{self, RequestId, RequestKind, RpcRequest};
use super::config::ServerConfig;
use super::framing::{Frame, FrameReader, FrameWriter, WriteDisposition};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{ProjectExecutionOptions, ProjectSnapshot, PROJECT_SCHEMA};
use crate::workspace_analysis::{
    WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
};

const APPLICATION_ERROR: i64 = -32000;
const METHOD_NOT_FOUND: i64 = -32601;
const METHODS: [&str; 10] = [
    "check",
    "context",
    "graph",
    "ping",
    "protocol",
    "shutdown",
    "test",
    "workspace/open",
    "workspace/snapshot",
    "workspace/status",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionState {
    Configured,
    Open,
    Invalidated,
    Shutdown,
}

impl SessionState {
    const fn text(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Open => "open",
            Self::Invalidated => "invalidated",
            Self::Shutdown => "shutdown",
        }
    }
}

pub(super) fn serve<R: BufRead, W: Write>(
    input: R,
    output: W,
    config: ServerConfig,
) -> io::Result<()> {
    let limits = config.limits();
    let manifest_path = config.manifest_path().to_path_buf();
    let mut input = FrameReader::new(input, limits);
    let mut output = FrameWriter::new(output, limits);
    crate::project::with_authenticated_project(&manifest_path, |snapshot| {
        let mut session = Session {
            snapshot,
            state: SessionState::Configured,
            limits,
            manifest_display: manifest_path.display().to_string(),
        };
        loop {
            let response = match input.read_frame().map_err(transport_io)? {
                Frame::Eof => break,
                Frame::OversizedTerminal => Some(codec::bounded_error_response(
                    None,
                    codec::PARSE_ERROR,
                    "request exceeds configured byte limit",
                    limits.response_bytes(),
                )),
                Frame::Data(frame) if frame.is_empty() => continue,
                Frame::Data(frame) => session.handle_frame(&frame),
            };
            if let Some(response) = response {
                let terminal_overflow = codec::is_overflow_response(&response);
                let disposition = output.write_response(&response).map_err(transport_io)?;
                if terminal_overflow || disposition == WriteDisposition::OverflowErrorWritten {
                    break;
                }
            }
            if session.state == SessionState::Shutdown {
                break;
            }
            // An oversized frame makes the reader terminal after its single
            // bounded error. The next loop observes EOF without allocating.
        }
        Ok(())
    })
    .map_err(|diagnostics| io::Error::other(diagnostic_message(&diagnostics)))
}

struct Session<'a> {
    snapshot: &'a mut ProjectSnapshot,
    state: SessionState,
    limits: super::framing::StdioLimits,
    manifest_display: String,
}

impl Session<'_> {
    fn handle_frame(&mut self, frame: &[u8]) -> Option<Vec<u8>> {
        let request = match codec::decode_request(frame) {
            Ok(request) => request,
            Err(error) => {
                if error.suppress_response {
                    return None;
                }
                return Some(codec::bounded_error_response(
                    error.response_id.as_ref(),
                    error.code,
                    &error.message,
                    self.limits.response_bytes(),
                ));
            }
        };
        let RpcRequest {
            kind,
            method,
            params,
        } = request;
        let RequestKind::Call(id) = kind else {
            if method == "shutdown" && params.as_ref().is_none_or(Map::is_empty) {
                self.state = SessionState::Shutdown;
            }
            return None;
        };
        Some(self.dispatch(&id, &method, params))
    }

    fn dispatch(
        &mut self,
        id: &RequestId,
        method: &str,
        params: Option<Map<String, Value>>,
    ) -> Vec<u8> {
        match method {
            "protocol" => self.no_params(id, params, |session| session.protocol()),
            "ping" => self.no_params(id, params, |session| {
                Ok(format!(
                    "{{\"pong\":true,\"state\":{}}}",
                    quote_json(session.state.text())
                ))
            }),
            "shutdown" => self.no_params(id, params, |session| {
                session.state = SessionState::Shutdown;
                Ok("{\"ok\":true}".to_owned())
            }),
            "workspace/status" => self.no_params(id, params, |session| session.status()),
            "workspace/open" => self.no_params(id, params, |session| session.open()),
            "workspace/snapshot" => self.subject(id, params, |snapshot, params| {
                reject_unknown(&params)?;
                Ok(render_snapshot(snapshot))
            }),
            "check" => self.subject(id, params, |snapshot, params| {
                reject_unknown(&params)?;
                snapshot.check()?;
                Ok("{\"ok\":true}".to_owned())
            }),
            "graph" => self.subject(id, params, |snapshot, params| {
                reject_unknown(&params)?;
                Ok(format!("{{\"graph\":{}}}", snapshot.semantic_graph()))
            }),
            "context" => self.context(id, params),
            "test" => self.test(id, params),
            unknown => self.error(
                id,
                METHOD_NOT_FOUND,
                &format!("method not found: {unknown}"),
            ),
        }
    }

    fn no_params(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        operation: impl FnOnce(&mut Self) -> Result<String, Vec<Diagnostic>>,
    ) -> Vec<u8> {
        if params.as_ref().is_some_and(|params| !params.is_empty()) {
            return self.error(
                id,
                codec::INVALID_PARAMS,
                "invalid params: this method takes no parameters",
            );
        }
        let result = operation(self);
        self.finish(id, result)
    }

    fn subject(
        &mut self,
        id: &RequestId,
        params: Option<Map<String, Value>>,
        operation: impl FnOnce(&ProjectSnapshot, Map<String, Value>) -> Result<String, Vec<Diagnostic>>,
    ) -> Vec<u8> {
        if self.state != SessionState::Open {
            return self.lifecycle_error(id);
        }
        let mut params = params.unwrap_or_default();
        if let Err(message) = take_exact_revisions(self.snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let result = self
            .snapshot
            .with_authenticated_request(|snapshot| operation(snapshot, params));
        if result
            .as_ref()
            .is_err_and(|diagnostics| invalidates(diagnostics))
        {
            self.state = SessionState::Invalidated;
        }
        self.finish(id, result)
    }

    fn context(&mut self, id: &RequestId, params: Option<Map<String, Value>>) -> Vec<u8> {
        self.subject(id, params, |snapshot, mut params| {
            let target_kind = match take_string(&mut params, "target_kind")?.as_str() {
                "declaration" => WorkspaceAnalysisTargetKind::Declaration,
                "capability" => WorkspaceAnalysisTargetKind::Capability,
                _ => {
                    return Err(parameter_diagnostic(
                        "target_kind must be declaration or capability",
                    ))
                }
            };
            let target = take_string(&mut params, "target")?;
            let direction = match take_optional_string(&mut params, "direction")?
                .as_deref()
                .unwrap_or("both")
            {
                "forward" => WorkspaceAnalysisDirection::Forward,
                "reverse" => WorkspaceAnalysisDirection::Reverse,
                "both" => WorkspaceAnalysisDirection::Both,
                _ => {
                    return Err(parameter_diagnostic(
                        "direction must be forward, reverse, or both",
                    ))
                }
            };
            let depth = take_optional_usize(&mut params, "depth")?.unwrap_or(4);
            let max_bytes = take_optional_usize(&mut params, "max_bytes")?.unwrap_or(1024 * 1024);
            let max_nodes = take_optional_usize(&mut params, "max_nodes")?.unwrap_or(1024);
            reject_unknown(&params)?;
            let options = WorkspaceContextOptions::new(direction, depth, max_bytes, max_nodes)
                .map_err(|diagnostic| vec![diagnostic])?;
            let context = snapshot.semantic_context(target_kind, &target, options)?;
            Ok(format!("{{\"context\":{context}}}"))
        })
    }

    fn test(&mut self, id: &RequestId, params: Option<Map<String, Value>>) -> Vec<u8> {
        self.subject(id, params, |snapshot, mut params| {
            let defaults = ProjectExecutionOptions::default();
            let max_steps =
                take_optional_usize(&mut params, "max_steps")?.unwrap_or(defaults.max_steps);
            let max_bytes =
                take_optional_usize(&mut params, "max_bytes")?.unwrap_or(defaults.max_bytes);
            reject_unknown(&params)?;
            let options = ProjectExecutionOptions::new(max_bytes, max_steps)
                .map_err(|diagnostic| vec![diagnostic])?;
            let execution = snapshot.execute_test(&options)?;
            Ok(format!(
                "{{\"command_succeeded\":{},\"execution\":{}}}",
                execution.command_succeeded(),
                execution.envelope()
            ))
        })
    }

    fn open(&mut self) -> Result<String, Vec<Diagnostic>> {
        if self.state == SessionState::Invalidated {
            return Err(lifecycle_diagnostic("project session is invalidated"));
        }
        if self.state == SessionState::Shutdown {
            return Err(lifecycle_diagnostic("project session is shut down"));
        }
        let result = self
            .snapshot
            .with_authenticated_request(|snapshot| Ok(render_open(snapshot)));
        if result
            .as_ref()
            .is_err_and(|diagnostics| invalidates(diagnostics))
        {
            self.state = SessionState::Invalidated;
        } else if result.is_ok() {
            self.state = SessionState::Open;
        }
        result
    }

    fn status(&self) -> Result<String, Vec<Diagnostic>> {
        let (project, workspace) = if self.state == SessionState::Configured {
            ("null".to_owned(), "null".to_owned())
        } else {
            (
                quote_json(self.snapshot.project_revision()),
                quote_json(self.snapshot.workspace_revision()),
            )
        };
        Ok(format!(
            "{{\"state\":{},\"last_successful_project_revision\":{project},\"last_successful_workspace_revision\":{workspace}}}",
            quote_json(self.state.text()),
        ))
    }

    fn protocol(&self) -> Result<String, Vec<Diagnostic>> {
        Ok(format!(
            "{{\"protocol\":{},\"version\":{},\"state\":{},\"methods\":[{}],\"limits\":{{\"max_request_bytes\":{},\"max_response_bytes\":{}}},\"bound_manifest\":{{\"path\":{},\"project_schema\":{}}},\"nonclaims\":[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_or_arbitrary_filesystem_read\",\"no_native_build_process_tool_or_temp_authority\",\"no_source_write_patch_rename_or_change_authority\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]}}",
            quote_json(super::TRANSPORT_SCHEMA),
            quote_json(env!("CARGO_PKG_VERSION")),
            quote_json(self.state.text()),
            METHODS.iter().map(|method| quote_json(method)).collect::<Vec<_>>().join(","),
            self.limits.request_bytes(),
            self.limits.response_bytes(),
            quote_json(&self.manifest_display),
            quote_json(PROJECT_SCHEMA),
        ))
    }

    fn finish(&self, id: &RequestId, result: Result<String, Vec<Diagnostic>>) -> Vec<u8> {
        match result {
            Ok(result) => {
                codec::bounded_success_response(id, &result, self.limits.response_bytes())
            }
            Err(diagnostics) => {
                let code = if diagnostics.iter().all(parameter_error) {
                    codec::INVALID_PARAMS
                } else {
                    APPLICATION_ERROR
                };
                self.error(id, code, &diagnostic_message(&diagnostics))
            }
        }
    }

    fn lifecycle_error(&self, id: &RequestId) -> Vec<u8> {
        self.error(
            id,
            APPLICATION_ERROR,
            &format!("SPX-J104: project session is {}", self.state.text()),
        )
    }

    fn error(&self, id: &RequestId, code: i64, message: &str) -> Vec<u8> {
        codec::bounded_error_response(Some(id), code, message, self.limits.response_bytes())
    }
}

fn render_open(snapshot: &ProjectSnapshot) -> String {
    format!(
        "{{\"opened\":true,\"project_revision\":{},\"workspace_revision\":{}}}",
        quote_json(snapshot.project_revision()),
        quote_json(snapshot.workspace_revision()),
    )
}

fn render_snapshot(snapshot: &ProjectSnapshot) -> String {
    let sources = snapshot
        .sources()
        .iter()
        .map(|source| {
            format!(
                "{{\"path\":{},\"source_graph_schema\":{},\"source_revision\":{},\"source_digest\":{},\"bytes\":{}}}",
                quote_json(source.path()),
                quote_json(source.source_graph_schema()),
                quote_json(source.source_revision()),
                quote_json(source.source_digest()),
                source.source().len(),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"semaprax.project-snapshot.v1\",\"project_schema\":{},\"name\":{},\"entry\":{},\"test_module\":{},\"project_revision\":{},\"workspace_revision\":{},\"manifest_bytes\":{},\"sources\":[{sources}]}}",
        quote_json(PROJECT_SCHEMA),
        quote_json(snapshot.manifest().name()),
        quote_json(snapshot.manifest().entry()),
        quote_json(snapshot.manifest().test_module()),
        quote_json(snapshot.project_revision()),
        quote_json(snapshot.workspace_revision()),
        snapshot.manifest().to_canonical_toml().len(),
    )
}

fn take_exact_revisions(
    snapshot: &ProjectSnapshot,
    params: &mut Map<String, Value>,
) -> Result<(), String> {
    let project = take_string_value(params, "project_revision")?;
    let workspace = take_string_value(params, "workspace_revision")?;
    if project != snapshot.project_revision() || workspace != snapshot.workspace_revision() {
        return Err("stale project_revision or workspace_revision".to_owned());
    }
    Ok(())
}

fn take_string(params: &mut Map<String, Value>, name: &str) -> Result<String, Vec<Diagnostic>> {
    take_string_value(params, name).map_err(|message| parameter_diagnostic(&message))
}

fn take_string_value(params: &mut Map<String, Value>, name: &str) -> Result<String, String> {
    match params.remove(name) {
        Some(Value::String(value)) if !value.is_empty() && !value.chars().any(char::is_control) => {
            Ok(value)
        }
        _ => Err(format!(
            "{name} must be a nonempty string without control characters"
        )),
    }
}

fn take_optional_string(
    params: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<String>, Vec<Diagnostic>> {
    match params.remove(name) {
        None => Ok(None),
        Some(Value::String(value)) if !value.is_empty() && !value.chars().any(char::is_control) => {
            Ok(Some(value))
        }
        _ => Err(parameter_diagnostic(&format!(
            "{name} must be a nonempty string without control characters"
        ))),
    }
}

fn take_optional_usize(
    params: &mut Map<String, Value>,
    name: &str,
) -> Result<Option<usize>, Vec<Diagnostic>> {
    match params.remove(name) {
        None => Ok(None),
        Some(Value::Number(number)) => number
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(Some)
            .ok_or_else(|| {
                parameter_diagnostic(&format!("{name} must be an unsigned host integer"))
            }),
        _ => Err(parameter_diagnostic(&format!(
            "{name} must be an unsigned host integer"
        ))),
    }
}

fn reject_unknown(params: &Map<String, Value>) -> Result<(), Vec<Diagnostic>> {
    match params.keys().next() {
        Some(name) => Err(parameter_diagnostic(&format!("unknown parameter `{name}`"))),
        None => Ok(()),
    }
}

fn parameter_diagnostic(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J105", message)]
}

fn lifecycle_diagnostic(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J104", message)]
}

fn invalidates(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.code, "SPX-J102" | "SPX-J103" | "SPX-J108"))
}

fn parameter_error(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic.code,
        "SPX-J105" | "SPX-G176" | "SPX-G178" | "SPX-F101"
    )
}

fn diagnostic_message(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect::<Vec<_>>()
        .join("; ")
}

fn transport_io(error: io::Error) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-J106",
        format!("project transport I/O failed: {error}"),
    )]
}
