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
                call: None,
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
        descriptor: Default::default(),
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
            ..Default::default()
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

// ---- HN-16: held-out matched routing evaluation (rules vs shadow) -----------

mod routing_eval {
    use super::*;
    use std::collections::BTreeSet;
    use std::io::Read;
    use std::net::TcpStream;

    /// Deterministic fixture shadow provider: a hand-written table, not a model.
    struct ShadowFixture;
    impl DecisionInvoker for ShadowFixture {
        fn evaluate(&mut self, r: &RequestEnvelope) -> DecisionCall {
            let fam = r.payload["features"]["task_family"].as_str().unwrap_or("");
            let pick = match fam {
                "mechanical" | "tests_docs" => "m-cheap",
                _ => "m-mid",
            };
            let opts = r.payload["options"].as_array().unwrap();
            let pick = if opts.iter().any(|o| o == pick) {
                pick
            } else {
                "m-cheap"
            };
            DecisionCall::Answered {
                result: json!({"choice": pick, "scores": {pick: 0.7}, "abstain": false}),
                elapsed_ms: 1,
                call: None,
            }
        }
    }

    /// `(prompt, expected exact reply)`; graded by string equality, never by a model.
    fn task(id: &str) -> (&'static str, &'static str) {
        match id {
            "seed-004" => ("Reply with only the result of 17+25.", "42"),
            "seed-005" => (
                "Reply with only the word `harness` in uppercase.",
                "HARNESS",
            ),
            "seed-006" => ("Reply with only the result of 9*8.", "72"),
            "seed-010" => (
                "Reply with only the first letter of the word `compiler`.",
                "c",
            ),
            "seed-011" => ("Reply with only the result of 100-37.", "63"),
            "seed-012" => ("Reply with only the word `route` reversed.", "etuor"),
            "seed-016" => ("Reply with only the larger number of 14 and 41.", "41"),
            "seed-017" => ("Reply with only the result of 6+7.", "13"),
            "seed-018" => ("Reply with only the word `yes` or `no`: is 12 even?", "yes"),
            "seed-022" => ("Reply with only the result of 3*3*3.", "27"),
            "seed-023" => ("Reply with only the result of 50/5.", "10"),
            _ => ("Reply with only the result of 2+2.", "4"),
        }
    }

    fn ollama(endpoint: &str, prompt: &str) -> Option<(String, u64, u64)> {
        let host = endpoint.trim_start_matches("http://");
        let body = json!({"model": "qwen2.5:0.5b", "prompt": prompt, "stream": false,
            "options": {"temperature": 0, "seed": 0, "num_predict": 16}})
        .to_string();
        let mut s = TcpStream::connect(host).ok()?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(120)))
            .ok()?;
        write!(
            s,
            "POST /api/generate HTTP/1.1\r\nHost: {host}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .ok()?;
        let mut raw = String::new();
        s.read_to_string(&mut raw).ok()?;
        let j: Value = serde_json::from_str(raw.split("\r\n\r\n").nth(1)?).ok()?;
        let tokens = j["prompt_eval_count"].as_u64()? + j["eval_count"].as_u64()?;
        Some((
            j["response"].as_str()?.trim().to_string(),
            tokens,
            j["total_duration"].as_u64()? / 1_000_000,
        ))
    }

    fn run_model(
        endpoint: &str,
        model: &str,
        item: &str,
        arm: &str,
        router_cost: u64,
        provider_real: bool,
    ) -> Outcome {
        let (prompt, want) = task(item);
        let mut o = Outcome {
            item: item.into(),
            arm: arm.into(),
            model: model.into(),
            origin: Origin::Unavailable,
            verified_by: "exact-match-grader".into(),
            completed: false,
            regressions: 0,
            attempts: 1,
            cost_micros: None,
            latency_ms: None,
            router_cost_micros: router_cost,
            context_cost_micros: 0,
            retry_owner: RetryOwner::Host,
        };
        // Only the cheap logical model is backed by a real local model.
        if model == "m-cheap" {
            if let Some((reply, tokens, ms)) = ollama(endpoint, prompt) {
                o.origin = if provider_real {
                    Origin::Real
                } else {
                    Origin::Fixture
                };
                o.completed = reply
                    .trim_matches(|c: char| !c.is_alphanumeric())
                    .eq_ignore_ascii_case(want);
                o.cost_micros = Some(tokens);
                o.latency_ms = Some(ms);
            }
        }
        o
    }

    fn mk_plan(id: &str, cost: u64, rank: u32) -> ModelPlan {
        plan(id, cost, rank)
    }

    #[test]
    #[ignore = "provisioned: needs HARNESS_OLLAMA_ENDPOINT (running Ollama with qwen2.5:0.5b); optional HARNESS_ROUTING_OUT"]
    fn real_matched_heldout_routing_evaluation_records_a_gate_decision() {
        let endpoint = std::env::var("HARNESS_OLLAMA_ENDPOINT")
            .expect("HARNESS_OLLAMA_ENDPOINT=http://127.0.0.1:11434");
        let corpus = repo_root().join("crates/semaprax-harness/tests/fixtures/decision_corpus");
        let splits: Value =
            serde_json::from_str(&std::fs::read_to_string(corpus.join("splits.json")).unwrap())
                .unwrap();
        let (mut eval, mut seen): (Vec<Value>, BTreeSet<String>) = (vec![], BTreeSet::new());
        for l in std::fs::read_to_string(corpus.join("seed.jsonl"))
            .unwrap()
            .lines()
        {
            let it: Value = serde_json::from_str(l).unwrap();
            let id = it["id"].as_str().unwrap().to_string();
            if splits["splits"][it["project"].as_str().unwrap()] == "eval" {
                eval.push(it);
            } else {
                seen.insert(id);
            }
        }
        let catalog = vec![
            mk_plan("m-cheap", 10, 1),
            mk_plan("m-mid", 30, 2),
            mk_plan("m-strong", 100, 3),
        ];
        let profile = ProviderProfile {
            provider_id: "semaprax/shadow-fixture".into(),
            model_id: "table".into(),
            checkpoint: "fixture-table-v1".into(),
            min_confidence: None,
            max_context_tokens: None,
            supported_families: None,
            ..Default::default()
        };
        let cfg = RoutingConfig {
            mode: RoutingMode::Experimental,
            shadow_max_calls: 1,
            ..RoutingConfig::default()
        };
        // Shadow evaluation: the actual route is rules; the fixture only recommends.
        let cfg_shadow = RoutingConfig {
            mode: RoutingMode::QualifiedAuto,
            ..cfg.clone()
        };
        let spec = GateSpec::default();
        let reg = EvidenceRegistry::default();
        let (mut outcomes, mut key_catalog) = (vec![], String::new());
        for it in &eval {
            let id = it["id"].as_str().unwrap();
            let features = TaskFeatures::from_json(&it["features"]).unwrap();
            let request = RouteRequest::new(
                features,
                catalog.clone(),
                Budget {
                    max_cost_micros: 1_000_000,
                    max_latency_ms: 60_000,
                    max_router_calls: 1,
                },
            )
            .unwrap();
            key_catalog = request.catalog_digest();
            let inputs = RouteInputs {
                request,
                policy: RoutePolicy::default(),
            };
            let mut shadow = ShadowFixture;
            let mut p = ConfiguredProvider {
                profile: profile.clone(),
                invoker: &mut shadow,
                mode: ProviderMode::Explicit,
                gate: EnablementGate::not_evaluated("model-route/v1", &profile.provider_id),
            };
            let g = Governor {
                cfg: &cfg_shadow,
                registry: Some(&reg),
                spec: &spec,
                lock: None,
                router_headroom_tokens: None,
                router_request_tokens: 0,
            };
            let r = governed_decide(
                &g,
                &inputs,
                &ctx(&format!("eval-{id}")),
                Some(&mut p),
                &|| inputs.clone(),
                None,
            )
            .unwrap();
            assert_eq!(r.shadow.as_ref().unwrap()["changes_route"], false);
            let rec = r.shadow.as_ref().unwrap()["recommended"]
                .as_str()
                .unwrap()
                .to_string();
            // Router overhead: byte upper bound of the shadow request (no tokenizer).
            let router_cost = json!({"features": inputs.request.features.to_json(), "options": ["m-cheap", "m-mid", "m-strong"]}).to_string().len() as u64;
            outcomes.push(run_model(
                &endpoint,
                &r.decision.choice,
                id,
                RULES_ARM,
                0,
                true,
            ));
            outcomes.push(run_model(
                &endpoint,
                &rec,
                id,
                &profile.provider_id,
                router_cost,
                false,
            ));
        }
        let key = EvidenceKey {
            catalog_digest: key_catalog,
            ..EvidenceKey::live(&profile, "")
        };
        let record = EvidenceRecord {
            key,
            budget: MatchedBudget {
                max_cost_micros: 1000,
                max_attempts: 1,
            },
            eval_items: eval
                .iter()
                .map(|i| i["id"].as_str().unwrap().to_string())
                .collect(),
            trained_on: seen,
            outcomes,
            calibration: None,
        };
        let dec = evaluate(&spec, &record);
        assert!(
            !dec.go,
            "an honest no-go is the expected result here: {dec:?}"
        );
        let doc = json!({
            "schema": "semaprax.routing-eval.v1",
            "decision": dec.to_json(),
            "gate_spec": {"min_items": spec.min_items, "completion_margin": spec.completion_margin,
                "min_cost_saving": spec.min_cost_saving, "max_extra_regressions": spec.max_extra_regressions,
                "max_latency_ratio": spec.max_latency_ratio},
            "key": record.key.to_json(),
            "active_mode": "rules (no-go keeps rules active)",
            "real_model": "ollama qwen2.5:0.5b (backs logical m-cheap only; m-mid and m-strong are unavailable cells, never successes)",
            "cost_unit": "ollama prompt+eval tokens as micros; router overhead is the request byte upper bound",
            "providers": {
                "rules": "builtin, real",
                "shadow-fixture": "fixture table; cells from it are origin=fixture and cannot unlock auto",
                "laya": "unavailable: venv removed for disk, not reinstalled; no learned-provider evidence recorded",
                "jev": "fixture-only: no key, no real inference result"
            },
            "outcomes": record.outcomes.iter().map(|o| json!({"item": o.item, "arm": o.arm, "model": o.model,
                "origin": o.origin.as_str(), "completed": o.completed, "cost_micros": o.cost_micros,
                "latency_ms": o.latency_ms, "router_cost_micros": o.router_cost_micros})).collect::<Vec<_>>(),
        });
        let text = serde_json::to_string_pretty(&doc).unwrap();
        println!("{text}");
        if let Ok(dir) = std::env::var("HARNESS_ROUTING_OUT") {
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                std::path::Path::new(&dir).join("gate-decision.json"),
                text + "\n",
            )
            .unwrap();
        }
    }
}
