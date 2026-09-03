use std::io::{self, BufRead, Write};

use serde_json::{Map, Value};

use super::codec::{self, RequestId, RequestKind, RpcRequest};
use super::config::{ServerConfig, ServerProfile};
use super::framing::{Frame, FrameReader, FrameWriter, WriteDisposition};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::project::{PreparedProjectRename, ProjectExecutionOptions, ProjectSnapshot};
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
const PROJECT_RENAME_METHODS: [&str; 12] = [
    "check",
    "context",
    "graph",
    "ping",
    "protocol",
    "rename/apply",
    "rename/preview",
    "shutdown",
    "test",
    "workspace/open",
    "workspace/snapshot",
    "workspace/status",
];
const PROJECT_WORKFLOW_METHODS: [&str; 16] = [
    "build",
    "change/apply",
    "change/preview",
    "check",
    "context",
    "graph",
    "impact",
    "ping",
    "protocol",
    "rename/derive",
    "review",
    "shutdown",
    "test",
    "workspace/open",
    "workspace/snapshot",
    "workspace/status",
];
const PROJECT_OWNED_DATA_METHODS: [&str; 12] = [
    "check",
    "context",
    "graph",
    "ping",
    "project/api-describe",
    "project/npm-build-inline",
    "protocol",
    "shutdown",
    "test",
    "workspace/open",
    "workspace/snapshot",
    "workspace/status",
];
const PROJECT_PUBLIC_API_METHODS: [&str; 12] = PROJECT_OWNED_DATA_METHODS;

mod owned_data;
mod public_api;
mod rename;
mod workflow;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionState {
    Configured,
    Open,
    Derived,
    Prepared,
    Applying,
    Invalidated,
    Uncertain,
    Shutdown,
}

impl SessionState {
    const fn text(self) -> &'static str {
        match self {
            Self::Configured => "configured",
            Self::Open => "open",
            Self::Derived => "derived",
            Self::Prepared => "prepared",
            Self::Applying => "applying",
            Self::Invalidated => "invalidated",
            Self::Uncertain => "uncertain",
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
    let snapshot = crate::project::load_snapshot(&manifest_path)
        .map_err(|diagnostics| io::Error::other(diagnostic_message(&diagnostics)))?;
    if config.profile() == ServerProfile::ProjectOwnedDataV1
        && !config
            .profile()
            .accepts_project_profile(snapshot.manifest().project_profile())
    {
        return Err(io::Error::other(
            "SPX-J105: Agent Transport v5 requires Project v8 owned-data-api.v1",
        ));
    }
    if config.profile() == ServerProfile::ProjectPublicApiV1
        && !config
            .profile()
            .accepts_project_profile(snapshot.manifest().project_profile())
    {
        return Err(io::Error::other(
            "SPX-J105: Agent Transport v6 requires a Project v8-v11 public owned-data API profile",
        ));
    }
    let mut session = Session {
        snapshot: Some(snapshot),
        state: SessionState::Configured,
        limits,
        profile: config.profile(),
        manifest_path,
        pending_rename: None,
        terminal_diagnostics: None,
    };
    let session_result = (|| {
        loop {
            let response = match input.read_frame()? {
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
                let disposition = output.write_response(&response)?;
                if terminal_overflow || disposition == WriteDisposition::OverflowErrorWritten {
                    break;
                }
            }
            if matches!(
                session.state,
                SessionState::Shutdown | SessionState::Uncertain
            ) || session.terminal_diagnostics.is_some()
            {
                break;
            }
            // An oversized frame makes the reader terminal after its single
            // bounded error. The next loop observes EOF without allocating.
        }
        Ok(())
    })();
    let authority_result = session.finish_authority();
    match (session_result, authority_result) {
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(diagnostics)) => Err(io::Error::other(format!(
            "{error}; final Project authority check failed: {}",
            diagnostic_message(&diagnostics)
        ))),
        (Ok(()), Ok(())) => Ok(()),
        (Ok(()), Err(diagnostics)) => Err(io::Error::other(diagnostic_message(&diagnostics))),
    }
}

struct Session {
    snapshot: Option<ProjectSnapshot>,
    state: SessionState,
    limits: super::framing::StdioLimits,
    profile: ServerProfile,
    manifest_path: std::path::PathBuf,
    pending_rename: Option<PreparedProjectRename>,
    terminal_diagnostics: Option<Vec<Diagnostic>>,
}

impl Session {
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
            "rename/preview" => self.rename_preview(id, params),
            "rename/apply" => self.rename_apply(id, params),
            "rename/derive" => self.rename_derive(id, params),
            "change/preview" => self.change_preview(id, params),
            "change/apply" => self.change_apply(id, params),
            "impact" => self.change_impact(id, params),
            "review" => self.change_review(id, params),
            "build" => self.build(id, params),
            "project/api-describe" if self.profile == ServerProfile::ProjectPublicApiV1 => {
                self.public_api_describe(id, params)
            }
            "project/api-describe" => self.api_describe(id, params),
            "project/npm-build-inline" if self.profile == ServerProfile::ProjectPublicApiV1 => {
                self.public_npm_build_inline(id, params)
            }
            "project/npm-build-inline" => self.npm_build_inline(id, params),
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
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("an open session retains its authenticated snapshot");
        if let Err(message) = take_exact_revisions(snapshot, &mut params) {
            return self.error(id, codec::INVALID_PARAMS, &message);
        }
        let result = self
            .snapshot
            .as_mut()
            .expect("an open session retains its authenticated snapshot")
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
        if !matches!(self.state, SessionState::Configured | SessionState::Open) {
            return Err(lifecycle_diagnostic(&format!(
                "project session is {}",
                self.state.text()
            )));
        }
        let result = self
            .snapshot
            .as_mut()
            .expect("configured and open sessions retain their snapshot")
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
        let snapshot = self
            .snapshot
            .as_ref()
            .expect("configured and open sessions retain their snapshot");
        let (project, workspace) = if self.state == SessionState::Configured {
            ("null".to_owned(), "null".to_owned())
        } else {
            (
                quote_json(snapshot.project_revision()),
                quote_json(snapshot.workspace_revision()),
            )
        };
        Ok(format!(
            "{{\"state\":{},\"last_successful_project_revision\":{project},\"last_successful_workspace_revision\":{workspace}}}",
            quote_json(self.state.text()),
        ))
    }

    fn protocol(&self) -> Result<String, Vec<Diagnostic>> {
        let project_schema = self
            .snapshot
            .as_ref()
            .expect("configured sessions retain their authenticated snapshot")
            .manifest()
            .schema();
        let (schema, methods, nonclaims) = match self.profile {
            ServerProfile::ReadOnlyV2 => (
                super::TRANSPORT_SCHEMA,
                METHODS.as_slice(),
                "[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_or_arbitrary_filesystem_read\",\"no_native_build_process_tool_or_temp_authority\",\"no_source_write_patch_rename_or_change_authority\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]",
            ),
            ServerProfile::ProjectRenameV1 => (
                super::PROJECT_RENAME_TRANSPORT_SCHEMA,
                PROJECT_RENAME_METHODS.as_slice(),
                "[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_path_patch_evidence_or_temp_authority\",\"single_file_explicit_exported_function_display_rename_only\",\"no_general_multi_file_change_import_alias_or_managed_workspace_authority\",\"no_exactly_once_delivery_deduplication_or_output_delivery_guarantee\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]",
            ),
            ServerProfile::ProjectWorkflowV1 => (
                super::PROJECT_WORKFLOW_TRANSPORT_SCHEMA,
                PROJECT_WORKFLOW_METHODS.as_slice(),
                "[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_path_source_patch_evidence_output_tool_or_environment_authority\",\"single_file_explicit_exported_function_display_rename_only\",\"project_bound_structural_impact_and_fixed_review_not_general_change_analysis\",\"web_only_inline_build_no_filesystem_process_or_target_execution\",\"no_general_multi_file_change_import_alias_or_managed_workspace_authority\",\"no_exactly_once_delivery_deduplication_or_output_delivery_guarantee\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]",
            ),
            ServerProfile::ProjectOwnedDataV1 => (
                super::PROJECT_OWNED_DATA_TRANSPORT_SCHEMA,
                PROJECT_OWNED_DATA_METHODS.as_slice(),
                "[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_path_source_patch_output_target_tool_or_environment_authority\",\"project_v8_owned_data_descriptor_and_npm_carrier_only\",\"read_only_no_source_write_rename_change_or_publication_authority\",\"no_filesystem_write_process_launch_target_execution_or_package_materialization\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]",
            ),
            ServerProfile::ProjectPublicApiV1 => (
                super::PROJECT_PUBLIC_API_TRANSPORT_SCHEMA,
                PROJECT_PUBLIC_API_METHODS.as_slice(),
                "[\"no_network_socket_tls_or_peer_authentication\",\"no_request_selected_root_path_source_patch_output_target_tool_or_environment_authority\",\"project_v8_v11_public_api_descriptors_and_npm_carriers_only\",\"read_only_no_source_write_rename_change_or_publication_authority\",\"no_filesystem_write_process_launch_target_execution_or_package_materialization\",\"no_persistent_disk_cache_or_incremental_refresh\",\"no_concurrent_batch_or_out_of_order_processing\"]",
            ),
        };
        Ok(format!(
            "{{\"protocol\":{},\"version\":{},\"state\":{},\"methods\":[{}],\"limits\":{{\"max_request_bytes\":{},\"max_response_bytes\":{}}},\"bound_manifest\":{{\"path\":{},\"project_schema\":{}}},\"nonclaims\":{nonclaims}}}",
            quote_json(schema),
            quote_json(env!("CARGO_PKG_VERSION")),
            quote_json(self.state.text()),
            methods.iter().map(|method| quote_json(method)).collect::<Vec<_>>().join(","),
            self.limits.request_bytes(),
            self.limits.response_bytes(),
            quote_json(&self.manifest_path.display().to_string()),
            quote_json(project_schema),
        ))
    }

    fn finish_authority(mut self) -> Result<(), Vec<Diagnostic>> {
        if let Some(diagnostics) = self.terminal_diagnostics.take() {
            return Err(diagnostics);
        }
        match self.snapshot.take() {
            Some(snapshot) => snapshot.finish_session(),
            None => Ok(()),
        }
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
        quote_json(snapshot.manifest().schema()),
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
