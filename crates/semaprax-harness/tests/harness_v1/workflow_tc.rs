//! TC-01 (usage receipts through the real workflow) tests that need no real compiler. The stage
//! below drives `HostModel`'s own request builder and outcome interpreter
//! through real `model.generate/v1` envelopes and a capture adapter; only the
//! adapter process itself is replaced. Fixture prefix `hp-tc`.

use super::*;
use semaprax_harness::contract::{
    validate_payload, CapabilityKind, CapabilityRef, Direction, RequestEnvelope, ResultEnvelope,
    ResultStatus,
};
use semaprax_harness::decision::{Destination, ModelPlan};
use semaprax_harness::host::Outcome;
use semaprax_harness::observe::Observation;
use semaprax_harness::receipt::{PriceBook, PriceRecord, Pricing};
use semaprax_harness::workflow::budget::BudgetPolicy;

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";

/// Each run gets its own project copy and cache: a side-effecting proposal is
/// journaled per lineage, and these tests drive several runs of one task. A
/// remote model of the task's own catalog is approved (`[routing] allow_remote`).
fn config(e: &Env, t: Task, p: Option<ApplyPolicy>) -> RunConfig {
    let fresh = setup(&std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap());
    let remote = t
        .models
        .as_ref()
        .is_some_and(|m| m.to_string().contains("origin"));
    let mut c = super::config(&fresh, t, p);
    c.routing.approve_remote = remote;
    c
}

type AdapterFn = Box<dyn Fn(&Value) -> (ResultStatus, Value)>;

/// Capture adapter: sees the wire request payload, answers with a status and payload.
struct Adapter {
    replies: RefCell<Vec<AdapterFn>>,
    wire: RefCell<Vec<Value>>,
    calls: Cell<u32>,
}

impl Adapter {
    fn new(replies: Vec<AdapterFn>) -> Self {
        Adapter {
            replies: RefCell::new(replies),
            wire: RefCell::default(),
            calls: Cell::new(0),
        }
    }
}

struct Via<'a>(&'a Adapter);

impl ProposalStage for Via<'_> {
    fn id(&self) -> String {
        "org.example/capture-model".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.propose_receipted(r).0
    }
    fn propose_receipted(
        &mut self,
        r: &ProposalRequest,
    ) -> (
        Result<Vec<u8>, StageFailure>,
        semaprax_harness::receipt::ProposalReceipt,
    ) {
        let a = self.0;
        a.calls.set(a.calls.get() + 1);
        let payload = HostModel::request_payload(r);
        // The wire request is a valid model.generate/v1 request.
        validate_payload(
            CapabilityKind::ModelGenerate,
            "generate",
            Direction::Request,
            &payload,
        )
        .expect("valid request payload");
        a.wire.borrow_mut().push(payload.clone());
        let req = RequestEnvelope {
            invocation_id: r.lineage.next_invocation(),
            project: r.lineage.project.clone(),
            lock_digest: r.lineage.lock_digest.clone(),
            capability: CapabilityRef {
                kind: CapabilityKind::ModelGenerate,
                version: 1,
            },
            operation: "generate".into(),
            deadline_ms: 30_000,
            max_result_bytes: 1 << 20,
            remaining_calls: 8,
            lineage: r.lineage.parents(),
            payload: payload.clone(),
        };
        let reply = {
            let q = a.replies.borrow_mut();
            let i = (a.calls.get() as usize - 1).min(q.len() - 1);
            (q[i])(&payload)
        };
        let outcome = match reply {
            (ResultStatus::Failed, _) => {
                Outcome::Uncertain(semaprax_harness::diag::HarnessDiagnostic::new(
                    "SPX-HPD072",
                    "connection lost after send",
                ))
            }
            (status, body) => {
                let mut env =
                    ResultEnvelope::complete(&req, body, "org.example/capture-model", "0.1.0");
                env.status = status;
                let bytes = env.to_json().to_string().into_bytes();
                Outcome::Completed(
                    ResultEnvelope::parse_for(&req, &bytes).expect("valid result frame"),
                )
            }
        };
        HostModel::interpret(outcome, r)
    }
    fn calls(&self) -> u32 {
        self.0.calls.get()
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

fn body(extra: Value) -> Vec<u8> {
    let mut v = json!({"schema": "semaprax.harness-proposal.v1",
                       "intent": {"kind": "replace_function_body", "target": "t.f"}});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    v.to_string().into_bytes()
}

fn wire_result(model: &Value, out: &[u8], receipt: Option<Value>) -> Value {
    let mut p = json!({"model": model, "output_base64": b64(out),
                       "usage": {"input_bytes": 1, "output_bytes": out.len()}});
    if let Some(r) = receipt {
        p["receipt"] = r;
    }
    p
}

fn b64(b: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        s.push(A[(n >> 18) as usize & 63] as char);
        s.push(A[(n >> 12) as usize & 63] as char);
        s.push(if c.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        s.push(if c.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    s
}

fn ok_reply(out: Vec<u8>, receipt: Value) -> AdapterFn {
    Box::new(move |req| {
        (
            ResultStatus::Complete,
            wire_result(&req["model"], &out, Some(receipt.clone())),
        )
    })
}

fn usage_receipt() -> Value {
    json!({"schema": "semaprax.harness-model-receipt.v1", "protocol": "responses", "request_id": "req_1",
           "model": "provider-model-2026", "finish_reason": "completed", "provider_cost_micros": 4242,
           "usage": {"input_tokens": 100, "input_tokens_details": {"cached_tokens": 60},
                     "output_tokens": 50, "output_tokens_details": {"reasoning_tokens": 20}},
           "controls": {"max_output_tokens": {"status": "applied", "effective": 200}}})
}

fn plan(id: &str, destination: Destination) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination,
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: 0,
        est_latency_ms: 10,
        strength_rank: 1,
    }
}

fn task(reserve: Option<u64>, destination: Destination) -> Task {
    Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f and keep behavior".into(),
        seed: Some("t.f".into()),
        models: Some(json!([plan("m-a", destination).to_json()])),
        budget: reserve.map(|n| BudgetPolicy {
            output_reserve_tokens: n,
            protocol_overhead_tokens: 20,
            ..Default::default()
        }),
        ..Task::default()
    }
}

fn drive(cfg: &RunConfig, fake: &Fake, a: &Adapter) -> (Report, Observer) {
    let mut native = NativeContext::new(fake);
    let mut p = Via(a);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run(
        cfg,
        fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    (r, obs)
}

fn generation_obs(obs: &Observer) -> Vec<&Observation> {
    obs.events()
        .iter()
        .filter(|o| o.capability == "model.generate")
        .collect()
}

// ---- TC-01 ---------------------------------------------------------------

#[test]
fn tc01_usage_survives_output_extraction_and_forged_usage_in_the_proposal_cannot_override_it() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let forged = body(
        json!({"claims": {"usage": {"output_tokens": 1}, "cost_micros": 0, "billing": "free"},
                             "summary": "usage: 1 token, cost 0"}),
    );
    let a = Adapter::new(vec![ok_reply(forged, usage_receipt())]);
    let mut cfg = config(&e, task(Some(200), Destination::Local), None);
    cfg.budget.prices = PriceBook::default().with(
        "m-",
        PriceRecord {
            version: "synthetic-v1".into(),
            pricing: Pricing::Rates {
                input: Some(3_000_000),
                cache_read: Some(300_000),
                cache_write: Some(0),
                cache_write_1h: None,
                output: Some(15_000_000),
            },
        },
    );
    let (r, obs) = drive(&cfg, &fake, &a);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(
        r.ignored_claims.len(),
        3,
        "claims stay ignored: {:?}",
        r.ignored_claims
    );
    let log = &r.context["usage_receipts"];
    assert_eq!(log["attempts"], 1);
    let rec = &log["entries"][0]["receipt"];
    assert_eq!(rec["availability"]["state"], "observed");
    assert_eq!(
        rec["usage"]["output"], 50,
        "the provider's receipt, not the forged claim: {rec}"
    );
    assert_eq!(rec["usage"]["uncached_input"], 40);
    assert_eq!(rec["usage"]["cache_read"], 60);
    assert_eq!(rec["usage"]["input_total"], 100);
    assert_eq!(rec["usage"]["reasoning"], 20);
    assert_eq!(rec["returned_model"], "provider-model-2026");
    assert_eq!(rec["cost"]["provider_reported_micros"], 4242);
    // 40*3 + 60*0.3 + 50*15 = 888, a local estimate beside (not instead of) the provider charge.
    assert_eq!(rec["cost"]["estimated"]["micros"], 888);
    assert_eq!(rec["cost"]["estimated"]["price_version"], "synthetic-v1");
    // Measured, preflight and reserved figures are reported separately.
    assert_eq!(log["measured"]["output"]["known_sum"], 50);
    assert!(log["preflight_input_tokens"].as_u64().unwrap() > 0);
    assert_eq!(log["reserved_output_tokens"], 200);
    // The observation carries the same numbers.
    let g = generation_obs(&obs);
    assert_eq!(g.len(), 1);
    let u = g[0].usage.expect("usage on the observation");
    assert_eq!(
        (u.output, u.cache_read, u.uncached_input),
        (Some(50), Some(60), Some(40))
    );
    assert_eq!(g[0].cost.provider_billed, Some(4242));
    assert_eq!(g[0].estimated_cost, Some(888));
    assert_eq!(g[0].upstream_model.as_deref(), Some("provider-model-2026"));
    // And it round-trips through the metadata-only event schema.
    let back = Observation::from_json(&g[0].to_json()).unwrap();
    assert_eq!(back.usage, g[0].usage);
    assert_eq!(back.estimated_cost, Some(888));
    // The task ledger reservation stays a separate record.
    assert_eq!(
        r.context["task_ledger"]["entries"][0]["output_reserve"],
        200
    );
}

#[test]
fn tc01_a_length_limited_reply_keeps_known_usage_and_leaves_unobserved_categories_unknown() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cut = br#"{"schema":"semaprax.harness-proposal.v1","intent":{"kind":"replace_function_bo"#
        .to_vec();
    let receipt = json!({"protocol": "anthropic_messages", "finish_reason": "max_tokens",
                         "usage_events": [{"type": "message_delta", "usage": {"output_tokens": 200}}]});
    let a = Adapter::new(vec![Box::new(move |req: &Value| {
        (
            ResultStatus::Partial,
            wire_result(&req["model"], &cut, Some(receipt.clone())),
        )
    })]);
    let cfg = config(&e, task(Some(200), Destination::Local), None);
    let (r, obs) = drive(&cfg, &fake, &a);
    assert_eq!(codes(&r), ["SPX-HPD030"], "{:?}", r.refusals);
    assert!(
        r.refusals[0].message.starts_with("incomplete model output"),
        "{}",
        r.refusals[0].message
    );
    assert!(
        !fake.log.borrow().contains(&"preview".to_string()),
        "never applied or previewed"
    );
    let rec = &r.context["usage_receipts"]["entries"][0]["receipt"];
    assert_eq!(rec["finish"], "length_limited");
    assert_eq!(rec["usage"]["output"], 200);
    assert_eq!(
        rec["usage"]["uncached_input"], "unknown",
        "input was never observed: {rec}"
    );
    assert_eq!(rec["usage"]["input_total"], "unknown");
    assert_eq!(rec["cost"]["provider_reported_micros"], "unknown");
    let g = generation_obs(&obs);
    assert_eq!(g[0].usage.unwrap().output, Some(200));
    assert_eq!(g[0].usage.unwrap().uncached_input, None);
}

#[test]
fn tc01_an_uncertain_failure_has_an_unavailable_receipt_and_is_never_replayed() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let a = Adapter::new(vec![Box::new(|_| (ResultStatus::Failed, Value::Null))]);
    let cfg = config(&e, task(Some(200), Destination::Local), None);
    let (r, obs) = drive(&cfg, &fake, &a);
    assert_eq!(codes(&r), ["SPX-HPD072"]);
    assert_eq!(a.calls.get(), 1);
    let rec = &r.context["usage_receipts"]["entries"][0]["receipt"];
    assert_eq!(rec["availability"]["state"], "unavailable");
    assert_eq!(
        rec["usage"]["output"], "unknown",
        "unknown, never an invented zero"
    );
    assert!(generation_obs(&obs)[0].usage.is_none());
}

#[test]
fn tc01_scripted_providers_report_unavailable_and_older_observations_still_load() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let cfg = config(&e, Task::default(), None);
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(r.status, "approved-candidate-ready", "{:?}", r.refusals);
    let rec = &r.context["usage_receipts"]["entries"][0]["receipt"];
    assert_eq!(rec["availability"]["state"], "unavailable");
    assert_eq!(rec["availability"]["reason"], "scripted_or_legacy_provider");
    assert_eq!(rec["cost"]["estimated"]["basis"], "unpriced_model");
    // An observation written before TC-01 has neither member and still loads.
    let mut o = Observation::new(
        "p",
        "model.generate",
        semaprax_harness::observe::Stage::Generation,
        semaprax_harness::observe::Role::Incurred,
        "inv-1",
    );
    o.incurred = Some(semaprax_harness::observe::TokenCount::bytes(10));
    let j = o.to_json();
    assert!(j.get("usage").is_none() && j.get("estimated_cost").is_none());
    let back = Observation::from_json(&j).unwrap();
    assert!(back.usage.is_none() && back.estimated_cost.is_none());
}

#[test]
fn tc01_a_non_billed_source_is_distinct_from_an_unpriced_remote_and_from_a_failure() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let reply = || {
        ok_reply(
            body(json!({})),
            json!({"protocol": "chat_completions", "finish_reason": "stop",
        "usage": {"prompt_tokens": 10, "prompt_tokens_details": {"cached_tokens": 0}, "completion_tokens": 3}}),
        )
    };
    // Explicitly non-billed local source: known zero billing.
    let mut cfg = config(&e, task(Some(200), Destination::Local), None);
    cfg.budget.prices = PriceBook::default().with(
        "m-",
        PriceRecord {
            version: "local-v1".into(),
            pricing: Pricing::NonBilled,
        },
    );
    let (r, _) = drive(&cfg, &fake, &Adapter::new(vec![reply()]));
    let est = &r.context["usage_receipts"]["entries"][0]["receipt"]["cost"]["estimated"];
    assert_eq!(
        (est["micros"].as_u64(), est["basis"].as_str()),
        (Some(0), Some("non_billed_source"))
    );
    // No price record for the remote model: unknown, not zero.
    let cfg = config(
        &e,
        task(
            Some(200),
            Destination::Remote {
                origin: "https://gw.example".into(),
            },
        ),
        None,
    );
    let (r, _) = drive(&cfg, &fake, &Adapter::new(vec![reply()]));
    let est = &r.context["usage_receipts"]["entries"][0]["receipt"]["cost"]["estimated"];
    assert_eq!(
        (est["micros"].as_str(), est["basis"].as_str()),
        (Some("unknown"), Some("unpriced_model"))
    );
}
