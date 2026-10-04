//! `semaprax.harness-bridge.v1` stdio server: LF-delimited JSON-RPC 2.0 frames.
//! Every method delegates to the existing single-source implementation.

use super::negotiate::{self, Availability, HostDeclaration, Owner, DEPTH_VAR};
use crate::cli::Environment;
use crate::command_view::{self, ExecOptions};
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{canonical, parse_frame, JsonLimits};
use crate::profile::{status, HarnessConfig, LocalState, Mode};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

const FRAME_LIMIT: usize = 1 << 20;

fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

pub struct Server<'a> {
    env: &'a Environment,
    project: PathBuf,
    host: Option<HostDeclaration>,
    avail: Availability,
    env_depth: u64,
}

impl<'a> Server<'a> {
    pub fn new(env: &'a Environment, project: &Path) -> Self {
        let project = env.cwd.join(project);
        let command_view_enabled = HarnessConfig::load(&project)
            .map(|c| c.capability(CapabilityKind::CommandView).mode != Mode::Disabled)
            .unwrap_or(true);
        let avail = Availability {
            compiler: env.compiler.is_some(),
            command_view_enabled,
        };
        // The bridge's own marker, or the command-view lineage marker the host sets in
        // every command it runs, shows this process was started from Semaprax.
        let env_depth = env
            .vars
            .get(DEPTH_VAR)
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
            + u64::from(
                env.vars
                    .get(crate::command_view::lineage::LINEAGE_VAR)
                    .is_some_and(|v| !v.is_empty()),
            );
        Self {
            env,
            project,
            host: None,
            avail,
            env_depth,
        }
    }

    fn recursion(&self, depth: u64, lineage: &[String]) -> Option<HarnessDiagnostic> {
        let d = depth.max(self.env_depth);
        if d >= 1 || lineage.iter().any(|l| l.starts_with("semaprax")) {
            Some(diag(
                "SPX-HPN002",
                format!("refused: invoked from Semaprax's own bridge/MCP (depth {d}, lineage {lineage:?}); calling back through the bridge would recurse"),
            ))
        } else {
            None
        }
    }

    /// Handle one method; `Err` becomes a JSON-RPC error.
    pub fn handle(&mut self, method: &str, params: &Value) -> HarnessResult<Value> {
        if let Some(e) = self.recursion(0, &[]) {
            return Err(e);
        }
        if method == "bridge/handshake" {
            let decl = negotiate::parse_declaration(params)?;
            if let Some(e) = self.recursion(decl.depth, &decl.lineage) {
                return Err(e);
            }
            let r = negotiate::response(&decl, &self.avail);
            self.host = Some(decl);
            return Ok(r);
        }
        let Some(host) = self.host.clone() else {
            return Err(diag("SPX-HPN004", "send `bridge/handshake` first"));
        };
        let obj = params
            .as_object()
            .ok_or_else(|| diag("SPX-HPN005", "params must be an object"))?;
        match method {
            "bridge/status" => {
                let config = HarnessConfig::load(&self.project)?;
                let state = LocalState::load(self.env)?;
                let (text, _) = status::status(&self.project, &config, &state, true);
                serde_json::from_str(&text).map_err(|e| diag("SPX-HPN008", format!("status document: {e}")))
            }
            "bridge/context" => {
                let query = obj.get("query").and_then(Value::as_str).ok_or_else(|| diag("SPX-HPN005", "`query` must be a string"))?;
                let mut args = vec![self.project.to_string_lossy().into_owned(), query.to_string(), "--json".into()];
                if let Some(n) = obj.get("max_bytes").and_then(Value::as_u64) {
                    args.extend(["--max-bytes".into(), n.to_string()]);
                }
                if let Some(s) = obj.get("symbol").and_then(Value::as_str) {
                    args.extend(["--symbol".into(), s.to_string()]);
                }
                if obj.get("references").and_then(Value::as_bool) == Some(true) {
                    args.push("--references".into());
                }
                let out = crate::context::cli_context(&args, self.env);
                if out.code != 0 {
                    return Err(diag("SPX-HPN008", format!("context refused: {}", out.stderr.trim())));
                }
                serde_json::from_str(&out.stdout).map_err(|e| diag("SPX-HPN008", format!("context document: {e}")))
            }
            "bridge/command_view" => {
                let argv: Vec<String> = obj
                    .get("argv")
                    .and_then(Value::as_array)
                    .and_then(|a| a.iter().map(|x| x.as_str().map(String::from)).collect())
                    .filter(|a: &Vec<String>| !a.is_empty())
                    .ok_or_else(|| diag("SPX-HPN005", "`argv` must be a non-empty array of strings"))?;
                let mut opts = ExecOptions {
                    raw: obj.get("raw").and_then(Value::as_bool) == Some(true),
                    timeout_ms: obj.get("timeout_ms").and_then(Value::as_u64),
                    ..ExecOptions::default()
                };
                if negotiate::owner_of(&host, &self.avail, "command_wrapper") != Owner::Semaprax {
                    opts.external_owner = Some(host.command_rewriter.clone().unwrap_or_else(|| "external-host".into()));
                }
                let rep = command_view::execute(self.env, &self.project, &argv, &opts, None)?;
                Ok(json!({"exit_code": rep.envelope.result.termination.code(), "envelope": rep.envelope.to_json(), "display": rep.display}))
            }
            "bridge/publish" => Err(diag(
                "SPX-HPN003",
                "publication is refused: it stays with the host-authorized compiler route, never the bridge",
            )),
            "bridge/cancel" => Ok(json!({"cancelled": false, "reason": "no request is in flight: the bridge serves one request at a time"})),
            other => Err(diag("SPX-HPN004", format!("unknown method `{other}`"))),
        }
    }
}

fn error_frame(id: &Value, d: &HarnessDiagnostic) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": d.message, "data": {"code": d.code}}})
}

/// Serve frames until EOF or `bridge/shutdown`.
pub fn serve<R: BufRead, W: Write>(
    reader: R,
    mut out: W,
    env: &Environment,
    project: &Path,
) -> std::io::Result<()> {
    let mut server = Server::new(env, project);
    let limits = JsonLimits::frame(FRAME_LIMIT);
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let parsed = parse_frame(line.as_bytes(), &limits);
        let shutdown = parsed
            .as_ref()
            .is_ok_and(|v| v.get("method").and_then(Value::as_str) == Some("bridge/shutdown"));
        let reply = match parsed {
            Err(d) => Some(error_frame(&Value::Null, &d)),
            Ok(v) => frame(&mut server, &v),
        };
        if let Some(r) = reply {
            writeln!(out, "{}", canonical(&r))?;
            out.flush()?;
        }
        if shutdown {
            break;
        }
    }
    Ok(())
}

fn frame(server: &mut Server, v: &Value) -> Option<Value> {
    let id = v.get("id").cloned();
    let rid = id.clone().unwrap_or(Value::Null);
    let (Some("2.0"), Some(method)) = (
        v.get("jsonrpc").and_then(Value::as_str),
        v.get("method").and_then(Value::as_str),
    ) else {
        return Some(error_frame(
            &rid,
            &diag("SPX-HPN004", "frame must be JSON-RPC 2.0 with a `method`"),
        ));
    };
    if method == "bridge/shutdown" {
        return id.map(|i| json!({"jsonrpc": "2.0", "id": i, "result": {}}));
    }
    let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
    let r = server.handle(method, &params);
    id.map(|i| match r {
        Ok(res) => json!({"jsonrpc": "2.0", "id": i, "result": res}),
        Err(d) => error_frame(&i, &d),
    })
}
