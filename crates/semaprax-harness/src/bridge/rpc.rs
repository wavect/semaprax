//! `semaprax.harness-bridge.v1` stdio server: LF-delimited JSON-RPC 2.0 frames.
//! Every method delegates to the existing single-source implementation.

use super::frame::{read_frame, FrameError};
use super::hostskills;
use super::inflight::{self, Invoker};
use super::negotiate::{self, Availability, HostDeclaration, Owner, DEPTH_VAR};
use super::skills_bridge::SkillsBridge;
use crate::cli::Environment;
use crate::command_view::{self, ExecOptions};
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::{CancelToken, HostConfig};
use crate::json::{canonical, parse_frame, JsonLimits};
use crate::profile::{status, HarnessConfig, LocalState, Mode};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

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
    /// Present after a v2 handshake.
    skills: Option<SkillsBridge<'a>>,
    session: Option<String>,
    host_skills_dir: Option<PathBuf>,
    log: Option<PathBuf>,
    invoker: Arc<Invoker>,
}

impl<'a> Server<'a> {
    /// Session identity from the launcher (`--session`); a v2 handshake `session` wins.
    pub fn with_session(mut self, s: Option<String>) -> Self {
        self.session = s;
        self
    }
    /// A project skills directory (for Claude Code `.claude/skills`) scanned read-only.
    pub fn with_host_skills_dir(mut self, d: Option<PathBuf>) -> Self {
        self.host_skills_dir = d;
        self
    }
    pub fn with_log(mut self, l: Option<PathBuf>) -> Self {
        self.log = l;
        self
    }
    /// Host knobs for `bridge/invoke` (isolation backend, grace periods).
    pub fn with_host_config(mut self, c: HostConfig) -> Self {
        self.invoker = Arc::new(Invoker::new(self.env, &self.project, c));
        self
    }
    /// Delivery observations recorded by the skills adapter (v2 sessions).
    pub fn deliveries(&self) -> Vec<Value> {
        self.skills
            .as_ref()
            .map(|s| s.deliveries().to_vec())
            .unwrap_or_default()
    }

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
        let invoker = Arc::new(Invoker::new(env, &project, HostConfig::default()));
        Self {
            env,
            project,
            host: None,
            avail,
            env_depth,
            skills: None,
            session: None,
            host_skills_dir: None,
            log: None,
            invoker,
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

    /// Gate for `bridge/invoke`: handshake first, no recursion.
    fn invoker(&self) -> HarnessResult<Arc<Invoker>> {
        if let Some(e) = self.recursion(0, &[]) {
            return Err(e);
        }
        if self.host.is_none() {
            return Err(diag("SPX-HPN004", "send `bridge/handshake` first"));
        }
        Ok(self.invoker.clone())
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
            let mut r = negotiate::response(&decl, &self.avail);
            if decl.protocol_version == 2 {
                let mut sb = SkillsBridge::new(
                    self.env,
                    &self.project,
                    decl.session.as_deref().or(self.session.as_deref()),
                    &format!("{}@{}", decl.name, decl.version),
                );
                let set = crate::skills::official::embedded_cached();
                sb.host_skills = hostskills::from_declared(set, &decl.host_skills);
                if let Some(dir) = &self.host_skills_dir {
                    for h in hostskills::scan_dir(set, &self.env.cwd.join(dir)) {
                        if !sb.host_skills.iter().any(|x| x.id == h.id) {
                            sb.host_skills.push(h);
                        }
                    }
                }
                sb.model_routing_delegated = decl.declared.get("model_routing") == Some(&true);
                sb.log = self.log.clone();
                r["skill_injection"] = sb.ownership();
                r["single_owner"]["skill_injection"] = r["skill_injection"]["owner"].clone();
                r["identity"] = json!({"project": sb.project_id, "session": sb.session});
                self.skills = Some(sb);
            }
            r["lifecycle"] = json!({"invoke": true, "cancel": true, "max_in_flight": inflight::MAX_IN_FLIGHT,
                "cancel_states": ["cancel-requested", "confirmed-terminated", "uncertain-external-effect"]});
            self.host = Some(decl);
            return Ok(r);
        }
        let Some(host) = self.host.clone() else {
            return Err(diag("SPX-HPN004", "send `bridge/handshake` first"));
        };
        let obj = params
            .as_object()
            .ok_or_else(|| diag("SPX-HPN005", "params must be an object"))?;
        if method.starts_with("bridge/skills/") {
            return match self.skills.as_mut() {
                Some(sb) => sb.handle(method, params),
                None => Err(diag(
                    "SPX-HPN004",
                    "skill methods need a `semaprax.harness-bridge.v2` handshake",
                )),
            };
        }
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
            "bridge/invoke" => Err(diag(
                "SPX-HPN004",
                "`bridge/invoke` runs only inside a serving session (`serve_with`)",
            )),
            "bridge/cancel" => {
                let id = obj
                    .get("id")
                    .filter(|i| i.is_string() || i.is_number())
                    .ok_or_else(|| diag("SPX-HPN005", "`id` must be the string or number id of the request to cancel"))?;
                Ok(self.invoker.registry.cancel(&canonical(id)))
            }
            other => Err(diag("SPX-HPN004", format!("unknown method `{other}`"))),
        }
    }
}

fn error_frame(id: &Value, d: &HarnessDiagnostic) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32000, "message": d.message, "data": {"code": d.code}}})
}

/// Serve frames until EOF or `bridge/shutdown`. `bridge/invoke` runs on a
/// worker thread (bounded, see `inflight::MAX_IN_FLIGHT`) so a later frame,
/// notably `bridge/cancel`, is read while it is blocked. At EOF (client gone)
/// or shutdown every live invocation is cancelled and reaped before returning.
pub fn serve<R: BufRead, W: Write + Send>(
    reader: R,
    out: W,
    env: &Environment,
    project: &Path,
) -> std::io::Result<()> {
    serve_with(reader, out, Server::new(env, project))
}

/// Maintenance tick that drives the idle reaper while the session waits for
/// input (MA-10). Bounded: a handful of cheap scans per second, never per ms.
const IDLE_TICK: Duration = Duration::from_millis(250);

/// Session-end signal for the maintenance tick.
#[derive(Default)]
struct Stop(Mutex<bool>, Condvar);

impl Stop {
    fn set(&self) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = true;
        self.1.notify_all();
    }
    /// Sleep one tick; `true` once the session has ended.
    fn wait_tick(&self) -> bool {
        let g = self.0.lock().unwrap_or_else(|p| p.into_inner());
        *self
            .1
            .wait_timeout_while(g, IDLE_TICK, |stopped| !*stopped)
            .unwrap_or_else(|p| p.into_inner())
            .0
    }
}

/// Runs on EVERY exit from the scope closure (EOF, shutdown, read, UTF-8,
/// oversize or write error, panic) before scoped workers are joined: nothing
/// may outlive the session, and the maintenance tick stops.
struct EndOfSession<'a> {
    invoker: &'a Invoker,
    stop: &'a Stop,
}

impl Drop for EndOfSession<'_> {
    fn drop(&mut self) {
        self.invoker.registry.cancel_all();
        self.stop.set();
    }
}

/// Serve with a preconfigured server (session, host skills directory, log).
pub fn serve_with<R: BufRead, W: Write + Send>(
    mut reader: R,
    out: W,
    mut server: Server,
) -> std::io::Result<()> {
    let limits = JsonLimits::frame(FRAME_LIMIT);
    let out = Mutex::new(out);
    let send = |r: &Value| -> std::io::Result<()> {
        let mut o = out.lock().unwrap_or_else(|p| p.into_inner());
        writeln!(o, "{}", canonical(r))?;
        o.flush()
    };
    let invoker = server.invoker.clone();
    let stop = Stop::default();
    // First worker write failure: the client can no longer hear us.
    let broken: Mutex<Option<std::io::Error>> = Mutex::new(None);
    let broken_flag = AtomicBool::new(false);
    std::thread::scope(|scope| -> std::io::Result<()> {
        let _end = EndOfSession {
            invoker: &invoker,
            stop: &stop,
        };
        scope.spawn(|| {
            while !stop.wait_tick() {
                invoker.reap_idle(Instant::now());
            }
        });
        loop {
            if broken_flag.load(Ordering::SeqCst) {
                let e = broken.lock().unwrap_or_else(|p| p.into_inner()).take();
                return Err(e.unwrap_or_else(|| std::io::ErrorKind::BrokenPipe.into()));
            }
            let line = match read_frame(&mut reader, FRAME_LIMIT) {
                Ok(Some(l)) => l,
                Ok(None) => break,
                Err(e) => {
                    if let FrameError::TooLarge { .. } = e {
                        let d = diag(
                            "SPX-HPA002",
                            format!("frame exceeds the {FRAME_LIMIT}-byte limit; session closed"),
                        );
                        let _ = send(&error_frame(&Value::Null, &d));
                    }
                    return Err(e.into_io());
                }
            };
            if line.trim().is_empty() {
                continue;
            }
            let parsed = parse_frame(line.as_bytes(), &limits);
            let shutdown = parsed
                .as_ref()
                .is_ok_and(|v| v.get("method").and_then(Value::as_str) == Some("bridge/shutdown"));
            let reply = match parsed {
                Err(d) => Some(error_frame(&Value::Null, &d)),
                Ok(v) if v.get("method").and_then(Value::as_str) == Some("bridge/invoke") => {
                    match start_invoke(&server, &v) {
                        Ok((key, id, params, token, inv)) => {
                            let (send, broken, broken_flag) = (&send, &broken, &broken_flag);
                            scope.spawn(move || {
                                let r = inv.run(&params, &token);
                                let state = r.as_ref().map_or("refused", inflight::settled_state);
                                inv.registry.settle(&key, state);
                                let sent = send(&match r {
                                    Ok(res) => json!({"jsonrpc": "2.0", "id": id, "result": res}),
                                    Err(d) => error_frame(&id, &d),
                                });
                                if let Err(e) = sent {
                                    // The output channel is gone: end the session.
                                    broken
                                        .lock()
                                        .unwrap_or_else(|p| p.into_inner())
                                        .get_or_insert(e);
                                    broken_flag.store(true, Ordering::SeqCst);
                                    inv.registry.cancel_all();
                                }
                            });
                            None
                        }
                        Err((id, d)) => Some(error_frame(&id, &d)),
                    }
                }
                Ok(v) => frame(&mut server, &v),
            };
            if let Some(r) = reply {
                send(&r)?;
            }
            if shutdown {
                break;
            }
        }
        Ok(())
    })
}

type Started = (String, Value, Value, CancelToken, Arc<Invoker>);

/// Validate and register one `bridge/invoke` frame before its worker starts.
fn start_invoke(server: &Server, v: &Value) -> Result<Started, (Value, HarnessDiagnostic)> {
    let id = v.get("id").cloned().unwrap_or(Value::Null);
    let fail = |d| (id.clone(), d);
    if v.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(fail(diag(
            "SPX-HPN004",
            "frame must be JSON-RPC 2.0 with a `method`",
        )));
    }
    if !(id.is_string() || id.is_number()) {
        return Err(fail(diag(
            "SPX-HPN005",
            "`bridge/invoke` needs a string or number `id`: it is the cancellation correlation id",
        )));
    }
    let inv = server.invoker().map_err(fail)?;
    let token = CancelToken::new();
    let key = canonical(&id);
    inv.registry.begin(&key, &token).map_err(fail)?;
    let params = v.get("params").cloned().unwrap_or_else(|| json!({}));
    Ok((key, id, params, token, inv))
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
