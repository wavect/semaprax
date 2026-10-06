//! Opt-in selected-law agent transport over the existing bounded Project
//! NDJSON codec. Requests can select a law and view, never a tool or root.
use super::{
    codec::{self, RequestId, RequestKind},
    config::ServerConfig,
    framing::{Frame, FrameReader, FrameWriter},
    PROJECT_LAW_WORKFLOW_TRANSPORT_SCHEMA,
};
use crate::{
    agent_runtime::AgentCancellation,
    assurance_manifest::law_set::installed_workflow::{self, Request, SourceGoal, View},
    diagnostic::Diagnostic,
    project::with_selected_law_diagnostics,
    proof_export::installed::{HostProfile, InstalledProofTool, Limits},
};
use serde_json::{json, Map, Value};
use std::io::{self, BufRead, Write};

const APPLICATION_ERROR: i64 = -32000;
const METHOD_NOT_FOUND: i64 = -32601;
const METHODS: [&str; 4] = ["law/check", "law/status", "protocol", "shutdown"];

pub(super) fn serve<R: BufRead, W: Write>(
    input: R,
    output: W,
    config: ServerConfig,
) -> io::Result<()> {
    let manifest = config.manifest_path().to_path_buf();
    let tool = config
        .law_tool()
        .expect("selected profile has pinned tool")
        .clone();
    // Startup proves the exact host policy exists and has no unreviewed law
    // drift. It does not acquire or execute the installed process tool.
    authenticate(&manifest)?;
    serve_authenticated(input, output, &config, &manifest, &tool)
}

/// The frame loop after startup authentication. Split out so the shared
/// reader's terminal paths are testable without an installed law policy.
pub(super) fn serve_authenticated<R: BufRead, W: Write>(
    input: R,
    output: W,
    config: &ServerConfig,
    manifest: &std::path::Path,
    tool: &super::config::LawToolConfig,
) -> io::Result<()> {
    let limits = config.limits();
    let mut input = FrameReader::new(input, limits);
    let mut output = FrameWriter::new(output, limits);
    loop {
        let frame = match input.read_frame()? {
            Frame::Eof => break,
            Frame::OversizedTerminal => {
                let response = codec::bounded_error_response(
                    None,
                    codec::PARSE_ERROR,
                    "request exceeds configured byte limit",
                    limits.response_bytes(),
                );
                output.write_response(&response)?;
                break;
            }
            Frame::Data(frame) if frame.is_empty() => continue,
            Frame::Data(frame) => frame,
        };
        let call = match codec::decode_request(&frame) {
            Ok(call) => call,
            Err(error) => {
                if !error.suppress_response {
                    let response = codec::bounded_error_response(
                        error.response_id.as_ref(),
                        error.code,
                        &error.message,
                        limits.response_bytes(),
                    );
                    output.write_response(&response)?;
                }
                continue;
            }
        };
        let RequestKind::Call(id) = call.kind else {
            continue;
        };
        let shutting_down = call.method == "shutdown";
        let response = dispatch(&id, &call.method, call.params, config, manifest, tool);
        let terminal_overflow = codec::is_overflow_response(&response);
        output.write_response(&response)?;
        if shutting_down || terminal_overflow {
            break;
        }
    }
    Ok(())
}

pub(super) fn authenticate(manifest: &std::path::Path) -> io::Result<()> {
    with_selected_law_diagnostics(manifest, |_, _, _| Ok(()))
        .map_err(|errors| io::Error::other(error_text(&errors)))
}

pub(super) fn dispatch(
    id: &RequestId,
    method: &str,
    params: Option<Map<String, Value>>,
    config: &ServerConfig,
    manifest: &std::path::Path,
    tool_config: &super::config::LawToolConfig,
) -> Vec<u8> {
    let max_response = config.limits().response_bytes();
    let result = match method {
        "protocol" => {
            if !params.as_ref().is_none_or(Map::is_empty) {
                return invalid_params(id, max_response);
            }
            let status = status(manifest);
            status.map(|status| {
                json!({
                    "protocol":PROJECT_LAW_WORKFLOW_TRANSPORT_SCHEMA,
                    "methods":METHODS,"bound_manifest":manifest,"selected":status,
                    "limits":{"max_request_bytes":config.limits().request_bytes(),
                        "max_response_bytes":max_response},
                    "source_authority":false,"publication_authority":false,
                    "nonclaims":["request_cannot_select_root_tool_or_process_profile",
                        "diagnostic_result_is_not_a_proof_token","no_editor_required"]
                })
            })
        }
        "law/status" => {
            if !params.as_ref().is_none_or(Map::is_empty) {
                return invalid_params(id, max_response);
            }
            status(manifest)
        }
        "law/check" => {
            let Some(params) = params else {
                return invalid_params(id, max_response);
            };
            let request = match parse_check(&params) {
                Ok(request) => request,
                Err(_) => return invalid_params(id, max_response),
            };
            with_selected_law_diagnostics(manifest, |revision, laws, policy| {
                // The manifest, protected baseline and exact current Project
                // are checked before this held process capability is acquired.
                let tool = InstalledProofTool::open(
                    &tool_config.executable,
                    manifest.parent().expect("absolute manifest has parent"),
                    tool_config.kind,
                    &tool_config.version_line,
                    HostProfile::TrustedLocal,
                    Limits::default(),
                    AgentCancellation::new(),
                )
                .map_err(|error| vec![error])?;
                let checked = installed_workflow::check(revision, laws, policy, &tool, &request)?;
                serde_json::from_str(&checked.document).map_err(|_| {
                    vec![Diagnostic::io(
                        "SPX-LW130",
                        "checked law workflow envelope is malformed",
                    )]
                })
            })
        }
        "shutdown" => {
            if !params.as_ref().is_none_or(Map::is_empty) {
                return invalid_params(id, max_response);
            }
            Ok(json!({"ok":true}))
        }
        _ => {
            return codec::bounded_error_response(
                Some(id),
                METHOD_NOT_FOUND,
                "method not found",
                max_response,
            )
        }
    };
    match result {
        Ok(value) => codec::bounded_success_response(
            id,
            &serde_json::to_string(&value).expect("bounded result JSON"),
            max_response,
        ),
        Err(errors) => {
            let data = json!({"diagnostics":diagnostic_rows(&errors),"source_authority":false});
            codec::bounded_application_error_response_with_data(
                id,
                &error_text(&errors),
                &data.to_string(),
                max_response,
            )
        }
    }
}

fn status(manifest: &std::path::Path) -> Result<Value, Vec<Diagnostic>> {
    with_selected_law_diagnostics(manifest, |revision, laws, policy| {
        Ok(json!({
            "schema":"semaprax.selected-law-agent-status.v1",
            "candidate_revision":revision.project_revision(),"law_digest":laws.digest(),
            "policy_digest":policy.digest(),"source_authority":false,
            "publication_authority":false,
        }))
    })
}

fn parse_check(params: &Map<String, Value>) -> Result<Request<'_>, ()> {
    if params.keys().any(|key| {
        !matches!(
            key.as_str(),
            "candidate_revision"
                | "law_id"
                | "view"
                | "source"
                | "declaration"
                | "ensures_index"
                | "offset"
                | "limit"
                | "max_bytes"
                | "show_witness_values"
        )
    }) {
        return Err(());
    }
    let text = |name| {
        params
            .get(name)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or(())
    };
    let law_id = text("law_id")?;
    let expected_candidate_revision = Some(text("candidate_revision")?);
    let source_fields = ["source", "declaration", "ensures_index"]
        .iter()
        .filter(|name| params.contains_key(**name))
        .count();
    let source_goal = if source_fields == 0 {
        None
    } else if source_fields == 3 {
        Some(SourceGoal {
            path: text("source")?,
            declaration: text("declaration")?,
            ensures_index: number(params, "ensures_index", 0)?,
        })
    } else {
        return Err(());
    };
    let view = match text("view")? {
        "detail" => View::Detail,
        "summary" => View::Summary {
            offset: number(params, "offset", 0)?,
            limit: number(params, "limit", 16)?,
        },
        _ => return Err(()),
    };
    let show_witness_values = params
        .get("show_witness_values")
        .map(Value::as_bool)
        .unwrap_or(Some(false))
        .ok_or(())?;
    Ok(Request {
        law_id,
        source_goal,
        view,
        max_bytes: number(params, "max_bytes", 65_536)?,
        show_witness_values,
        expected_candidate_revision,
    })
}

fn number(params: &Map<String, Value>, name: &str, default: usize) -> Result<usize, ()> {
    params.get(name).map_or(Ok(default), |value| {
        value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(())
    })
}

fn invalid_params(id: &RequestId, max_response: usize) -> Vec<u8> {
    codec::bounded_error_response(
        Some(id),
        codec::INVALID_PARAMS,
        "invalid selected law parameters",
        max_response,
    )
}

fn diagnostic_rows(errors: &[Diagnostic]) -> Vec<Value> {
    errors
        .iter()
        .map(|error| json!({"code":error.code,"message":error.message}))
        .collect()
}

fn error_text(errors: &[Diagnostic]) -> String {
    errors
        .iter()
        .map(|error| format!("{}: {}", error.code, error.message))
        .collect::<Vec<_>>()
        .join("; ")
}
