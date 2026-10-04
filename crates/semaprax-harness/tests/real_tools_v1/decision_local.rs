//! Provisioned: a real loopback Laya server driven through the laya-local
//! adapter, the strict result parser and `decision::decide`.
//! Needs HARNESS_PYTHON (python3) and HARNESS_LAYA_ENDPOINT (http://127.0.0.1:PORT
//! of an already running, pinned `laya-serve`). The fallback test needs only
//! HARNESS_PYTHON. Neither starts a server or downloads anything.

use crate::support::{repo_root, required_tool};
use semaprax_harness::contract::{ProjectBinding, RequestEnvelope, ResultEnvelope};
use semaprax_harness::decision::*;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::Instant;

/// Drives the adapter over `semaprax.harness-rpc.v1` and parses every result
/// with the host's strict envelope parser.
struct AdapterInvoker {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    last: Option<ResultEnvelope>,
}

impl AdapterInvoker {
    fn spawn(endpoint: &str) -> Self {
        let adapter =
            repo_root().join("packages/semaprax-harness-adapters/systemone/laya-local/adapter.py");
        let mut child = Command::new(required_tool("HARNESS_PYTHON"))
            .arg(adapter)
            .env_clear()
            .env("SEMAPRAX_HARNESS_ENDPOINT", endpoint)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn laya-local adapter");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        let mut me = Self {
            child,
            stdin,
            stdout,
            last: None,
        };
        let init = json!({"jsonrpc": "2.0", "id": 1, "method": "harness/initialize", "params": {
            "protocol": "semaprax.harness-rpc.v1", "offered": [{"kind": "decision.evaluate", "version": 1}]}});
        let reply = me.rpc(&init);
        assert_eq!(reply["result"]["accepted"][0]["kind"], "decision.evaluate");
        me
    }

    fn rpc(&mut self, msg: &Value) -> Value {
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        serde_json::from_str(&line).expect("adapter frame")
    }
}

impl DecisionInvoker for AdapterInvoker {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        let started = Instant::now();
        let reply = self.rpc(&json!({"jsonrpc": "2.0", "id": 2, "method": "harness/invoke", "params": request.to_json()}));
        let elapsed_ms = started.elapsed().as_millis() as u64;
        let bytes = serde_json::to_vec(&reply["result"]).unwrap();
        let env = match ResultEnvelope::parse_for(request, &bytes) {
            Ok(e) => e,
            Err(_) => return DecisionCall::Unavailable,
        };
        let call = match (env.status.as_str(), &env.payload) {
            ("complete", Some(p)) => DecisionCall::Answered {
                result: p.clone(),
                elapsed_ms,
            },
            _ if env.diagnostics.iter().any(|(c, _)| c == "SPX-HPK011") => DecisionCall::Timeout,
            _ => DecisionCall::Unavailable,
        };
        self.last = Some(env);
        call
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

fn plan(id: &str, cost: u64, rank: u32) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: true,
        max_context: 200_000,
        est_cost_micros: cost,
        est_latency_ms: 1000,
        strength_rank: rank,
    }
}

fn inputs() -> RouteInputs {
    let features = TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 12_000,
        requires_structured_output: false,
        requires_tools: true,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let budget = Budget {
        max_cost_micros: 1_000_000,
        max_latency_ms: 60_000,
        max_router_calls: 1,
    };
    let request = RouteRequest::new(
        features,
        vec![
            plan("m-cheap", 10, 1),
            plan("m-mid", 30, 2),
            plan("m-strong", 100, 3),
        ],
        budget,
    )
    .unwrap();
    RouteInputs {
        request,
        policy: RoutePolicy::default(),
    }
}

fn ctx(id: &str) -> RouteContext {
    RouteContext {
        project: ProjectBinding {
            id: "p".repeat(64),
            worktree: "w".repeat(64),
            revision: "r".repeat(64),
        },
        lock_digest: "l".repeat(64),
        invocation_id: id.into(),
        lineage_id: "lineage-1".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    }
}

fn provider(inv: &mut AdapterInvoker) -> ConfiguredProvider<'_> {
    ConfiguredProvider {
        profile: ProviderProfile {
            provider_id: "ai.convai/laya-decision".into(),
            model_id: "laya-multilingual".into(),
            checkpoint: "convaiinnovations/laya@7b928d82:multilingual".into(),
            min_confidence: None,
            max_context_tokens: Some(100_000),
            supported_families: None,
        },
        invoker: inv,
        mode: ProviderMode::Explicit,
        gate: EnablementGate::not_evaluated("model-route/v1", "ai.convai/laya-decision"),
    }
}

#[test]
#[ignore = "provisioned: needs HARNESS_PYTHON, HARNESS_LAYA_ENDPOINT (running pinned laya-serve)"]
fn real_laya_decision_flows_through_the_pipeline() {
    let endpoint = std::env::var("HARNESS_LAYA_ENDPOINT")
        .expect("provisioned test requires HARNESS_LAYA_ENDPOINT=http://127.0.0.1:PORT");
    let mut inv = AdapterInvoker::spawn(&endpoint);
    let i = inputs();
    // Warm the checkpoint outside the 2 s router ceiling: a cold load is
    // seconds and is reported as a cost, not hidden (see HARNESS-LAYA-JEV-V1).
    let mut warm = provider(&mut inv);
    let _ = decide(&i, &ctx("warm-0001"), Some(&mut warm), &|| inputs(), None).unwrap();
    let mut p = provider(&mut inv);
    let d = decide(&i, &ctx("inv-0001"), Some(&mut p), &|| inputs(), None).unwrap();
    assert_eq!(d.source, DecisionSource::Provider, "{d:?}");
    assert_eq!(d.provider_id, "ai.convai/laya-decision");
    assert_eq!(d.provider_status, "experimental");
    assert_eq!(d.router_calls, 1);
    assert!(["m-cheap", "m-mid", "m-strong"].contains(&d.choice.as_str()));
    assert_eq!(d.plan.ordered[0].model_id, d.choice);
    println!("laya choice={} router_ms={}", d.choice, d.router_ms);
    let env = inv.last.as_ref().expect("parsed result");
    assert_eq!(env.provenance.provider_id, "ai.convai/laya-decision");
    assert!(env
        .diagnostics
        .iter()
        .any(|(c, m)| c == "SPX-HPK100" && m.contains("latency_ms=")));
}

#[test]
#[ignore = "provisioned: needs HARNESS_PYTHON"]
fn absent_laya_server_falls_back_to_rules_without_starting_anything() {
    let mut inv = AdapterInvoker::spawn("http://127.0.0.1:1");
    let i = inputs();
    let mut p = provider(&mut inv);
    let d = decide(&i, &ctx("inv-0002"), Some(&mut p), &|| inputs(), None).unwrap();
    assert_eq!(
        d.source,
        DecisionSource::Fallback(FallbackReason::Unavailable)
    );
    assert_eq!(d.provider_id, RULES_PROVIDER_ID);
    assert_eq!(d.choice, "m-cheap");
}
