//! TC-03: one durable task spend budget admits every billable dispatch.
//! Counting router and generator fakes prove refusals happen before any
//! outbound call; journal states simulate crashes. Fixture prefix `hp-hp04`.

use super::*;
use semaprax_harness::decision::{
    Destination, EnablementGate, ModelPlan, ProviderMode, ProviderProfile,
};
use semaprax_harness::diag::HarnessDiagnostic;
use semaprax_harness::receipt::{
    GenerationSupport, PriceBook, PriceRecord, Pricing, ProposalReceipt, Support,
};
use semaprax_harness::workflow::lineage::Lineage;

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";

/// What the counting generator answers on one call.
#[derive(Clone)]
enum Reply {
    /// Proposal bytes (optionally rejected by the fake compiler) and a receipt.
    Ok(Option<&'static str>, Value),
    Refused(Value),
    Uncertain,
}

/// Side-effecting (billable) generator that counts its calls.
struct Paid {
    replies: Vec<Reply>,
    calls: Cell<u32>,
    support: GenerationSupport,
}

impl Paid {
    fn new(replies: Vec<Reply>) -> Self {
        Paid {
            replies,
            calls: Cell::new(0),
            support: GenerationSupport {
                output_cap: Support::Supported,
                reasoning: Support::Unknown,
            },
        }
    }
}

struct PaidRef<'a>(&'a Paid);

impl ProposalStage for PaidRef<'_> {
    fn id(&self) -> String {
        "org.example/paid-model".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.propose_receipted(r).0
    }
    fn propose_receipted(
        &mut self,
        r: &ProposalRequest,
    ) -> (Result<Vec<u8>, StageFailure>, ProposalReceipt) {
        let p = self.0;
        let i = p.calls.get() as usize;
        p.calls.set(p.calls.get() + 1);
        let reply = p.replies[i.min(p.replies.len() - 1)].clone();
        let rc = |v: &Value| ProposalReceipt::from_result(&r.controls, &json!({ "receipt": v }));
        match reply {
            Reply::Ok(reject, v) => {
                let mut intent = json!({"kind": "replace_function_body", "target": "t.f"});
                if let Some(m) = reject {
                    intent["fake_refuse"] = json!(m);
                }
                let b = json!({"schema": "semaprax.harness-proposal.v1", "intent": intent})
                    .to_string()
                    .into_bytes();
                (Ok(b), rc(&v))
            }
            Reply::Refused(v) => (
                Err(StageFailure::Refused(HarnessDiagnostic::new(
                    "SPX-HPD090",
                    "provider refused the request",
                ))),
                rc(&v),
            ),
            Reply::Uncertain => (
                Err(StageFailure::Uncertain(HarnessDiagnostic::new(
                    "SPX-HPD072",
                    "connection lost after send",
                ))),
                ProposalReceipt::unavailable("outcome_unknown"),
            ),
        }
    }
    fn generation_support(&self) -> GenerationSupport {
        self.0.support
    }
    fn calls(&self) -> u32 {
        self.0.calls.get()
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

fn plan(id: &str, rank: u32) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: 0,
        est_latency_ms: 10,
        strength_rank: rank,
    }
}

fn task(session: Option<SessionBounds>) -> Task {
    Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f and keep behavior".into(),
        seed: Some("t.f".into()),
        models: Some(json!([
            plan("m-cheap", 1).to_json(),
            plan("m-strong", 2).to_json()
        ])),
        budget: Some(semaprax_harness::workflow::budget::BudgetPolicy {
            output_reserve_tokens: 100,
            protocol_overhead_tokens: 20,
            ..Default::default()
        }),
        session,
        ..Task::default()
    }
}

fn rates(input: u64, read: u64, write: u64, output: u64) -> PriceRecord {
    PriceRecord {
        version: "synthetic-v1".into(),
        pricing: Pricing::Rates {
            input: Some(input),
            cache_read: Some(read),
            cache_write: Some(write),
            cache_write_1h: None,
            output: Some(output),
        },
    }
}

/// `m-*` generation: 1/token input, 0.1 cache read, 2 cache write, 4 output.
fn gen_prices() -> PriceBook {
    PriceBook::default().with("m-", rates(1_000_000, 100_000, 2_000_000, 4_000_000))
}

fn cfg(e: &Env, t: Task) -> RunConfig {
    let mut c = config(e, t, None);
    c.budget.prices = gen_prices();
    c
}

fn exec(cfg: &RunConfig, paid: &Paid, router: Option<&mut Router>) -> Report {
    let fake = Fake::new(CHANGED);
    let mut native = NativeContext::new(&fake);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let mut p = PaidRef(paid);
    let decision = router.map(|inv| {
        let profile = ProviderProfile {
            provider_id: "org.example/paid-route".into(),
            model_id: "router-m".into(),
            checkpoint: "1".into(),
            min_confidence: None,
            max_context_tokens: None,
            supported_families: None,
        };
        let gate = EnablementGate::not_evaluated("model-route/v1", &profile.provider_id);
        DecisionStage {
            invoker: inv,
            profile,
            mode: ProviderMode::Explicit,
            gate,
        }
    });
    run(
        cfg,
        &fake,
        Stages {
            decision,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    )
}

fn receipt(cost: Option<u64>, output: u64) -> Value {
    let mut v = json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
        "usage": {"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 100, "output_tokens": output}});
    if let Some(c) = cost {
        v["provider_cost_micros"] = json!(c);
    }
    v
}

fn spend(r: &Report) -> &Value {
    &r.context["spend"]
}

fn lineage(cfg: &RunConfig) -> String {
    Lineage::new(cfg.snapshot.binding(), &cfg.lock_digest, &cfg.task.digest()).id
}

#[test]
fn tc03_a_paid_router_above_the_remaining_cap_is_never_invoked_and_rules_still_route() {
    // Control: an affordable priced router is consulted and its reservation journaled.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.prices = gen_prices().with("router-m", rates(1_000_000, 0, 0, 1_000_000));
    c.budget.spend.max_task_cost_micros = Some(10_000_000);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(500), 10))]);
    let mut router = Router(0, "m-strong");
    let r = exec(&c, &paid, Some(&mut router));
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!((router.0, paid.calls.get()), (1, 1));
    let att = spend(&r)["attempts"].as_array().unwrap();
    assert_eq!(att[0]["kind"], "router");
    assert!(att[0]["reserved_cost_micros"].as_u64().unwrap() > 0);
    // A billable router without a receipt stays counted at its bound.
    assert_eq!(att[0]["state"], "uncertain");
    assert_eq!(att[1]["state"], "settled");

    // The same router priced far above the remaining cap: zero router calls,
    // rules route, and the generation is still admitted and dispatched.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.prices = gen_prices().with("router-m", rates(1_000_000_000, 0, 0, 1_000_000_000));
    c.budget.spend.max_task_cost_micros = Some(100_000);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(500), 10))]);
    let mut router = Router(0, "m-strong");
    let r = exec(&c, &paid, Some(&mut router));
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!((router.0, paid.calls.get()), (0, 1), "router never invoked");
    assert_eq!(r.route["router_calls"], 0);
    assert!(
        r.notes
            .iter()
            .any(|n| n.starts_with("router not consulted")),
        "{:?}",
        r.notes
    );
    let kinds: Vec<_> = spend(&r)["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["generation"], "no router reservation was taken");
    let j = std::fs::read_to_string(e.cache.join(format!("{}.journal.jsonl", r.lineage))).unwrap();
    assert!(!j.contains("router"), "{j}");
}

#[test]
fn tc03_strict_mode_refuses_unpriced_billable_work_and_admits_explicit_non_billed() {
    // Strict + unpriced router: no zero-cost reservation, no router call; rules route.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.spend.strict_monetary = true;
    let paid = Paid::new(vec![Reply::Ok(None, receipt(None, 10))]);
    let mut router = Router(0, "m-strong");
    let r = exec(&c, &paid, Some(&mut router));
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!((router.0, paid.calls.get()), (0, 1));
    assert!(r.notes.iter().any(|n| n.contains("strict monetary")));

    // Strict + unpriced generator: refused before dispatch.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.prices = PriceBook::default();
    c.budget.spend.strict_monetary = true;
    let paid = Paid::new(vec![Reply::Ok(None, receipt(None, 10))]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"], "{:?}", r.refusals);
    assert!(r.refusals[0].message.contains("strict monetary"));
    assert_eq!(paid.calls.get(), 0);

    // Strict + priced generator whose provider does not enforce the cap: no bound, refused.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.spend.strict_monetary = true;
    let mut paid = Paid::new(vec![Reply::Ok(None, receipt(None, 10))]);
    paid.support.output_cap = Support::Unknown;
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"]);
    assert_eq!(paid.calls.get(), 0);

    // Explicitly non-billed local source: admitted (even without a declared cap),
    // distinguishable in the report...
    let non_billed = PriceBook::default().with(
        "m-",
        PriceRecord {
            version: "local-v1".into(),
            pricing: Pricing::NonBilled,
        },
    );
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.prices = non_billed.clone();
    c.budget.spend.strict_monetary = true;
    let mut paid = Paid::new(vec![Reply::Ok(None, json!({"protocol": "responses"}))]);
    paid.support.output_cap = Support::Unknown;
    let r = exec(&c, &paid, None);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(paid.calls.get(), 1);
    let s = spend(&r);
    assert_eq!(s["attempts"][0]["billing"]["kind"], "non_billed");
    assert_eq!(s["non_billed_attempts"], 1);
    assert_eq!(s["known_actual_cost_micros"], 0);
    assert_eq!(s["unknown_spend_attempts"], 0);
    // ...and still bounded by tokens.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    c.budget.prices = non_billed;
    c.budget.spend.strict_monetary = true;
    c.budget.spend.max_task_tokens = Some(50);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(None, 10))]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"]);
    assert_eq!(paid.calls.get(), 0);
}

fn session(max_tokens: Option<u64>) -> Task {
    task(Some(SessionBounds {
        max_attempts: 3,
        max_tokens,
        ..Default::default()
    }))
}

#[test]
fn tc03_a_session_below_max_tokens_refuses_an_oversized_next_request_before_sending_it() {
    let reject = Reply::Ok(
        Some("SPX-G225 candidate intention is missing a required field"),
        json!({"protocol": "responses"}),
    );
    let ok = Reply::Ok(None, json!({"protocol": "responses"}));
    // Probe: how much the first attempt commits (unknown cost keeps it all).
    let e = setup(FIXED);
    let paid = Paid::new(vec![reject.clone(), ok.clone()]);
    let r = exec(&cfg(&e, session(None)), &paid, None);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let first = spend(&r)["attempts"][0]["committed_tokens"]
        .as_u64()
        .unwrap();
    // Below the bound after attempt one, so the old turn check passes; the
    // next request (with feedback) does not fit and is never sent.
    let e = setup(FIXED);
    let paid = Paid::new(vec![reject, ok]);
    let r = exec(&cfg(&e, session(Some(first + 1))), &paid, None);
    assert_eq!(
        (r.status, codes(&r)),
        ("exhausted", vec!["SPX-HPD111"]),
        "{:?}",
        r.refusals
    );
    assert!(r.refusals[0].message.contains("refused before dispatch"));
    assert_eq!(paid.calls.get(), 1, "the oversized request was not sent");
    assert_eq!(spend(&r)["limits"]["session_tokens"], first + 1);
}

#[test]
fn tc03_a_smaller_completed_reply_returns_headroom_while_an_unknown_cost_does_not() {
    let reject = |cost: Option<u64>| {
        Reply::Ok(
            Some("SPX-G225 candidate intention is missing a required field"),
            receipt(cost, 5),
        )
    };
    let ok = Reply::Ok(None, receipt(Some(10), 5));
    // Probe the first attempt's cost bound.
    let e = setup(FIXED);
    let paid = Paid::new(vec![reject(Some(10)), ok.clone()]);
    let r = exec(&cfg(&e, session(None)), &paid, None);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let a = &spend(&r)["attempts"];
    let b1 = a[0]["reserved_cost_micros"].as_u64().unwrap();
    let b2 = a[1]["reserved_cost_micros"].as_u64().unwrap();
    assert_eq!(a[0]["committed_cost_micros"], 10, "settled at the receipt");
    // A cap that holds both bounds only when the first returns its headroom.
    let cap = b1.max(b2) + 20;
    let e = setup(FIXED);
    let mut c = cfg(&e, session(None));
    c.budget.spend.max_task_cost_micros = Some(cap);
    let paid = Paid::new(vec![reject(Some(10)), ok.clone()]);
    let r = exec(&c, &paid, None);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(paid.calls.get(), 2, "headroom returned for a later attempt");
    // Same cap, but the first reply reports no usable usage or charge: its
    // cost is unknown, so it stays reserved and the second is refused.
    let e = setup(FIXED);
    let mut c = cfg(&e, session(None));
    c.budget.spend.max_task_cost_micros = Some(cap);
    let unknown = Reply::Ok(
        Some("SPX-G225 candidate intention is missing a required field"),
        json!({"protocol": "responses"}),
    );
    let paid = Paid::new(vec![unknown, ok]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"], "{:?}", r.refusals);
    assert_eq!(paid.calls.get(), 1, "uncertain spend keeps its reservation");
    assert_eq!(spend(&r)["attempts"][0]["state"], "uncertain");
    assert_eq!(spend(&r)["unknown_spend_attempts"], 1);
    assert_eq!(spend(&r)["outstanding_upper_bound_micros"], b1);
}

#[test]
fn tc03_resume_restores_spend_and_cannot_bypass_the_task_cost_cap() {
    // Run 1: the provider refuses after billing; the step may run again.
    let e = setup(FIXED);
    let mut c = cfg(&e, task(None));
    let paid = Paid::new(vec![Reply::Refused(receipt(Some(900), 5))]);
    let r1 = exec(&c, &paid, None);
    assert_eq!(codes(&r1), ["SPX-HPD090"], "{:?}", r1.refusals);
    let bound = spend(&r1)["attempts"][0]["reserved_cost_micros"]
        .as_u64()
        .unwrap();
    assert_eq!(spend(&r1)["known_actual_cost_micros"], 900);
    // The cap admits one fresh attempt but not one on top of the earlier spend.
    c.budget.spend.max_task_cost_micros = Some(bound + 899);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r2 = exec(&c, &paid, None);
    assert_eq!(codes(&r2), ["SPX-HPD101"], "{:?}", r2.refusals);
    assert_eq!(paid.calls.get(), 0, "resume did not bypass the cap");
    let s = spend(&r2);
    assert_eq!(s["attempts"][0]["restored"], true);
    assert_eq!(
        s["known_actual_cost_micros"], 900,
        "settled once, not reset"
    );
    // Control: a fresh lineage under the same cap dispatches.
    let e2 = setup(FIXED);
    let mut c2 = cfg(&e2, task(None));
    c2.budget.spend.max_task_cost_micros = Some(bound + 899);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r3 = exec(&c2, &paid, None);
    assert_eq!(r3.status, "candidate-ready", "{:?}", r3.refusals);
    assert_eq!(paid.calls.get(), 1);
}

#[test]
fn tc03_crash_points_are_restored_conservatively_and_malformed_records_fail_closed() {
    // The bound of a fresh attempt (probe on another project copy).
    let probe = setup(FIXED);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r = exec(&cfg(&probe, task(None)), &paid, None);
    let bound = spend(&r)["attempts"][0]["reserved_cost_micros"]
        .as_u64()
        .unwrap();
    let e = setup(FIXED);
    let c = cfg(&e, task(None));
    let l = lineage(&c);
    let path = e.cache.join(format!("{l}.journal.jsonl"));
    let reserve = |id: &str, cost: u64| {
        json!({"seq": 1, "step": format!("spend.{id}"), "state": "reserve",
               "detail": {"id": id, "kind": "generation", "label": "generate", "model": "m-cheap",
                          "reserved_tokens": 100, "reserved_cost_micros": cost,
                          "billing": {"kind": "priced", "price_version": "synthetic-v1"}}})
    };
    let line = |v: Value| format!("{}\n", semaprax_harness::json::canonical(&v));
    // Crash before dispatch: an outstanding reservation of an earlier invocation
    // still counts, so a new attempt over the cap is refused without a call.
    std::fs::create_dir_all(&e.cache).unwrap();
    std::fs::write(&path, line(reserve("old.generation.1", 1_000))).unwrap();
    let mut capped = cfg(&e, task(None));
    // Fits alone (control: the probe), not beside the restored reservation.
    capped.budget.spend.max_task_cost_micros = Some(bound + 500);
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r = exec(&capped, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"], "{:?}", r.refusals);
    assert_eq!(paid.calls.get(), 0);
    assert_eq!(spend(&r)["outstanding_upper_bound_micros"], 1_000);
    // Crash after dispatch and after the receipt (settled, step never completed):
    // the generation is not replayed and the settlement is not double-counted.
    let settle = json!({"seq": 2, "step": "spend.old.generation.1", "state": "settle",
        "detail": {"id": "old.generation.1", "actual_cost_micros": 300, "settled_tokens": 90, "breach": null, "basis": "provider_reported"}});
    let begin = json!({"seq": 3, "step": "generate", "state": "begin", "detail": {}});
    std::fs::write(
        &path,
        [
            line(reserve("old.generation.1", 1_000)),
            line(settle.clone()),
            line(settle.clone()),
            line(begin),
        ]
        .concat(),
    )
    .unwrap();
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD072"]);
    assert_eq!(paid.calls.get(), 0);
    assert_eq!(spend(&r)["known_actual_cost_micros"], 300);
    // Malformed accounting refuses the run before any call.
    let mut bad = reserve("old.generation.1", 1_000);
    bad["detail"]["reserved_tokens"] = json!("lots");
    std::fs::write(&path, line(bad)).unwrap();
    let paid = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD070"], "{:?}", r.refusals);
    assert_eq!(paid.calls.get(), 0);
    // Contradictory settlements likewise.
    let mut other = settle.clone();
    other["detail"]["actual_cost_micros"] = json!(301);
    std::fs::write(
        &path,
        [
            line(reserve("old.generation.1", 1_000)),
            line(settle),
            line(other),
        ]
        .concat(),
    )
    .unwrap();
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD070"]);
    assert_eq!(paid.calls.get(), 0);
    // An uncertain outcome is recorded uncertain and kept at its reservation.
    let e = setup(FIXED);
    let c = cfg(&e, task(None));
    let paid = Paid::new(vec![Reply::Uncertain]);
    let r = exec(&c, &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD072"]);
    let a = &spend(&r)["attempts"][0];
    assert_eq!(a["state"], "uncertain");
    assert_eq!(a["committed_cost_micros"], a["reserved_cost_micros"]);
    let paid2 = Paid::new(vec![Reply::Ok(None, receipt(Some(10), 5))]);
    let r = exec(&c, &paid2, None);
    assert_eq!(paid2.calls.get(), 0, "not replayed");
    assert_eq!(spend(&r)["attempts"][0]["state"], "uncertain");
}

#[test]
fn tc03_reports_reconcile_receipts_and_a_breach_blocks_further_paid_work() {
    // Two attempts; the first is charged above its declared bound (an
    // undisclosed gateway retry). The breach is surfaced and the second,
    // paid attempt is refused before dispatch.
    let e = setup(FIXED);
    let paid = Paid::new(vec![
        Reply::Ok(
            Some("SPX-G225 candidate intention is missing a required field"),
            receipt(Some(10_000_000), 5),
        ),
        Reply::Ok(None, receipt(Some(10), 5)),
    ]);
    let r = exec(&cfg(&e, session(None)), &paid, None);
    assert_eq!(codes(&r), ["SPX-HPD101"], "{:?}", r.refusals);
    assert!(r.refusals[0].message.contains("breach"));
    assert_eq!(paid.calls.get(), 1);
    let s = spend(&r);
    assert!(s["breach"]
        .as_str()
        .unwrap()
        .contains("above the declared bound"));
    assert_eq!(s["known_actual_cost_micros"], 10_000_000);
    // The same charge with one disclosed gateway retry is within the bound
    // only when the bound covers it; here the report reconciles every receipt.
    let e = setup(FIXED);
    let paid = Paid::new(vec![
        Reply::Ok(
            Some("SPX-G225 candidate intention is missing a required field"),
            receipt(None, 5),
        ),
        Reply::Ok(None, receipt(Some(10), 5)),
    ]);
    let mut c = cfg(&e, session(None));
    c.budget.spend.gateway_retries = 1;
    let r = exec(&c, &paid, None);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let s = spend(&r);
    let receipts = &r.context["usage_receipts"];
    assert_eq!(receipts["attempts"], 2);
    assert_eq!(s["attempts"].as_array().unwrap().len(), 2);
    // First: local estimate (100 cache-write tokens at 2 + 5 out at 4 = 220); second: provider charge.
    let est = receipts["entries"][0]["receipt"]["cost"]["estimated"]["micros"].clone();
    assert_eq!(est, 220);
    assert_eq!(s["attempts"][0]["actual_cost_micros"], est);
    assert_eq!(s["attempts"][1]["actual_cost_micros"], 10);
    assert_eq!(s["known_actual_cost_micros"], 230);
    assert_eq!(s["outstanding_upper_bound_micros"], 0);
    assert!(s["breach"].is_null());
    let led = &r.context["task_ledger"];
    assert_eq!(led["reserved_cost_micros"], 230);
    assert_eq!(led["entries"][0]["state"], "settled");
    // Gateway retry disclosure doubles the reserved bound.
    let gen_bound = s["attempts"][1]["reserved_cost_micros"].as_u64().unwrap();
    assert_eq!(gen_bound % 2, 0, "{s}");
}

#[test]
fn tc03_monetary_budget_config_is_opt_in() {
    use semaprax_harness::workflow::spend::SpendPolicy;
    let parse = |b: &str| {
        semaprax_harness::profile::config::parse(
            format!("schema = \"semaprax.harness-config.v1\"\n[budget]\n{b}\n").as_bytes(),
        )
    };
    let plain = parse("context_max_bytes = 4096").unwrap();
    assert_eq!(
        SpendPolicy::from_section(&plain.budget.generation),
        SpendPolicy::default(),
        "no members, no monetary limit"
    );
    let c = parse("strict_monetary = true\ntask_max_cost_micros = 250000\ntask_max_tokens = 90000\ngateway_max_retries = 2").unwrap();
    assert_eq!(
        SpendPolicy::from_section(&c.budget.generation),
        SpendPolicy {
            strict_monetary: true,
            max_task_cost_micros: Some(250_000),
            max_task_tokens: Some(90_000),
            gateway_retries: 2,
        }
    );
    assert_eq!(
        c.to_json()["budget"]["generation"]["task_max_cost_micros"],
        250_000
    );
    assert!(parse("task_max_cost_micros = -1").is_err());
    assert!(parse("strict_monetary = \"yes\"").is_err());
}
