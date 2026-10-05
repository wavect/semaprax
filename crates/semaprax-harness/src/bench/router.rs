//! Decision (route) step: the same `decision::decide` the host uses, with an
//! optional external `decision.evaluate/v1` adapter spoken over
//! `semaprax.harness-rpc.v1`. The adapter is identified only by its descriptor
//! and runtime variable; no product name appears here.

use super::corpus::{bad, RouterSpec};
use crate::cli::Environment;
use crate::contract::{ProjectBinding, RequestEnvelope, ResultEnvelope};
use crate::decision::*;
use crate::diag::HarnessResult;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub struct RouteOutcome {
    pub choice: String,
    pub source: String,
    pub provider_id: String,
    pub router_calls: u32,
    pub router_ms: u64,
    /// Size of the router request that actually left the host (incurred).
    pub request_bytes: u64,
}

/// Spawns the adapter named by a descriptor and drives `harness/invoke`.
pub struct AdapterInvoker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    pub request_bytes: u64,
}

impl AdapterInvoker {
    pub fn spawn(spec: &RouterSpec, descriptor: &Path, env: &Environment) -> HarnessResult<Self> {
        let q = |m: String| bad("SPX-HPQ010", m);
        let d: Value = serde_json::from_slice(
            &std::fs::read(descriptor).map_err(|e| q(format!("router descriptor: {e}")))?,
        )
        .map_err(|e| q(format!("router descriptor: {e}")))?;
        let entry = d["adapter"]["entry"][0]
            .as_str()
            .ok_or_else(|| q("descriptor has no adapter.entry".into()))?;
        let dir = descriptor.parent().unwrap_or(Path::new("."));
        let exe = env.vars.get(&spec.runtime_env).ok_or_else(|| {
            q(format!(
                "runtime variable `{}` not provided",
                spec.runtime_env
            ))
        })?;
        let mut cmd = Command::new(exe);
        cmd.arg(dir.join(entry)).env_clear();
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| q(format!("spawn router adapter: {e}")))?;
        let stdin = child.stdin.take().expect("piped");
        let stdout = BufReader::new(child.stdout.take().expect("piped"));
        let mut me = Self {
            child,
            stdin,
            stdout,
            request_bytes: 0,
        };
        let init = json!({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 1}]}});
        let reply = me
            .rpc(&init)
            .ok_or_else(|| q("router adapter closed during initialize".into()))?;
        if reply["result"]["accepted"][0]["kind"] != "decision.evaluate" {
            return Err(q("router adapter did not accept decision.evaluate".into()));
        }
        Ok(me)
    }

    fn rpc(&mut self, msg: &Value) -> Option<Value> {
        writeln!(self.stdin, "{msg}").ok()?;
        self.stdin.flush().ok()?;
        let mut line = String::new();
        self.stdout.read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    }
}

impl DecisionInvoker for AdapterInvoker {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        let msg = json!({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": request.to_json()});
        self.request_bytes += msg.to_string().len() as u64;
        let started = Instant::now();
        let Some(reply) = self.rpc(&msg) else {
            return DecisionCall::Unavailable;
        };
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let bytes = serde_json::to_vec(&reply["result"]).unwrap_or_default();
        match ResultEnvelope::parse_for(request, &bytes) {
            Ok(env) => match (env.status.as_str(), &env.payload) {
                ("complete", Some(p)) => DecisionCall::Answered {
                    result: p.clone(),
                    elapsed_ms,
                    call: None,
                },
                _ if env.diagnostics.iter().any(|(c, _)| c == "SPX-HPK011") => {
                    DecisionCall::Timeout
                }
                _ => DecisionCall::Unavailable,
            },
            Err(_) => DecisionCall::Unavailable,
        }
    }
}

impl Drop for AdapterInvoker {
    fn drop(&mut self) {
        let _ = writeln!(
            self.stdin,
            r#"{{"jsonrpc":"2.0","id":9,"method":"harness/shutdown"}}"#
        );
        let _ = self.stdin.flush();
        let _ = self.child.wait();
    }
}

/// Test and adversarial invoker: always answers with a fixed payload.
pub struct ScriptedInvoker {
    pub answer: Value,
    pub calls: u32,
}

impl DecisionInvoker for ScriptedInvoker {
    fn evaluate(&mut self, _request: &RequestEnvelope) -> DecisionCall {
        self.calls += 1;
        DecisionCall::Answered {
            result: self.answer.clone(),
            elapsed_ms: 1,
            call: None,
        }
    }
}

/// Run `decide` over a corpus route request, with or without a router.
pub fn route(
    doc: &Value,
    router: Option<(&mut dyn DecisionInvoker, &RouterSpec)>,
    lineage: &str,
) -> HarnessResult<RouteOutcome> {
    let request = RouteRequest::from_json(doc)?;
    let policy = match doc.get("policy") {
        Some(p) => RoutePolicy::from_json(p)?,
        None => RoutePolicy::default(),
    };
    let inputs = RouteInputs { request, policy };
    let ctx = RouteContext {
        project: ProjectBinding {
            id: "b".repeat(64),
            worktree: "b".repeat(64),
            revision: "b".repeat(64),
        },
        lock_digest: "b".repeat(64),
        invocation_id: "inv-bench".into(),
        lineage_id: lineage.into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let d = match router {
        None => decide(&inputs, &ctx, None, &|| inputs.clone(), None)?,
        Some((invoker, spec)) => {
            let mut p = ConfiguredProvider {
                profile: ProviderProfile {
                    provider_id: spec.provider_id.clone(),
                    model_id: spec.model_id.clone(),
                    checkpoint: spec.checkpoint.clone(),
                    min_confidence: None,
                    max_context_tokens: Some(100_000),
                    supported_families: None,
                    ..Default::default()
                },
                invoker,
                mode: ProviderMode::Explicit,
                gate: EnablementGate::not_evaluated("model-route/v1", &spec.provider_id),
            };
            decide(&inputs, &ctx, Some(&mut p), &|| inputs.clone(), None)?
        }
    };
    Ok(RouteOutcome {
        choice: d.choice.clone(),
        source: format!("{:?}", d.source),
        provider_id: d.provider_id.clone(),
        router_calls: d.router_calls,
        router_ms: d.router_ms,
        request_bytes: 0,
    })
}
