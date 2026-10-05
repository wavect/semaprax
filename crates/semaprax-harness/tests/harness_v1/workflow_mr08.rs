//! MR-08: optional plan / implement / advisory-review role policies over the
//! existing session, one task budget, verified-failure routing and resume.
//! A role-aware fake generator answers by the request's `phase`; the fake
//! compiler judges every candidate. Fixture prefix `hp-hp04`.

use super::*;
use semaprax_harness::decision::{
    DecisionCall, DecisionInvoker, Destination, EnablementGate, ModelPlan, ProviderMode,
    ProviderProfile,
};
use semaprax_harness::diag::HarnessDiagnostic;
use semaprax_harness::profile::config::PhaseConfig;
use semaprax_harness::receipt::{
    GenerationSupport, PriceBook, PriceRecord, Pricing, ProposalReceipt, Support,
};
use semaprax_harness::workflow::phases::{parent_model_routing, PLAN_SCHEMA, REVIEW_SCHEMA};

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";
const FINDING: &str = "consider naming the zero literal";

/// One scripted reply.
#[derive(Clone)]
enum R {
    Doc(Value),
    Uncertain,
}

/// Billable generator that answers by the request's role (`phase`).
struct Roles {
    plan: Vec<R>,
    implement: Vec<R>,
    review: Vec<R>,
    side: bool,
    /// (phase, model) of every outbound call.
    calls: RefCell<Vec<(String, String)>>,
    prompts: RefCell<Vec<Value>>,
}

impl Roles {
    fn new(plan: Vec<R>, implement: Vec<R>, review: Vec<R>) -> Self {
        Roles {
            plan,
            implement,
            review,
            side: true,
            calls: RefCell::default(),
            prompts: RefCell::default(),
        }
    }
    fn calls(&self) -> Vec<(String, String)> {
        self.calls.borrow().clone()
    }
    fn implement_prompts(&self) -> Vec<Value> {
        self.prompts
            .borrow()
            .iter()
            .filter(|p| p.get("phase").is_none())
            .cloned()
            .collect()
    }
}

struct RolesRef<'a>(&'a Roles);

impl ProposalStage for RolesRef<'_> {
    fn id(&self) -> String {
        "org.example/role-model".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.propose_receipted(r).0
    }
    fn propose_receipted(
        &mut self,
        r: &ProposalRequest,
    ) -> (Result<Vec<u8>, StageFailure>, ProposalReceipt) {
        let p = self.0;
        let phase = r.prompt["phase"]
            .as_str()
            .unwrap_or("implement")
            .to_string();
        let n = p
            .calls
            .borrow()
            .iter()
            .filter(|(ph, _)| *ph == phase)
            .count();
        p.calls.borrow_mut().push((phase.clone(), r.model.clone()));
        p.prompts.borrow_mut().push(r.prompt.clone());
        let list = match phase.as_str() {
            "plan" => &p.plan,
            "review" => &p.review,
            _ => &p.implement,
        };
        match list[n.min(list.len() - 1)].clone() {
            R::Doc(v) => (
                Ok(v.to_string().into_bytes()),
                ProposalReceipt::from_result(&r.controls, &json!({"receipt": receipt()})),
            ),
            R::Uncertain => (
                Err(StageFailure::Uncertain(HarnessDiagnostic::new(
                    "SPX-HPD072",
                    "connection lost after send",
                ))),
                ProposalReceipt::unavailable("outcome_unknown"),
            ),
        }
    }
    fn generation_support(&self) -> GenerationSupport {
        GenerationSupport {
            output_cap: Support::Supported,
            reasoning: Support::Unknown,
        }
    }
    fn calls(&self) -> u32 {
        self.0.calls.borrow().len() as u32
    }
    fn side_effecting(&self) -> bool {
        self.0.side
    }
}

fn receipt() -> Value {
    json!({"protocol": "anthropic_messages", "finish_reason": "end_turn", "provider_cost_micros": 500,
           "usage": {"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 100, "output_tokens": 10}})
}

fn plan_doc(extra: Value) -> R {
    let mut v = json!({"schema": PLAN_SCHEMA, "steps": ["rewrite t.f body", "keep the contract"]});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    R::Doc(v)
}

fn review_doc(extra: Value) -> R {
    let mut v = json!({"schema": REVIEW_SCHEMA, "findings": [{"severity": "info", "message": FINDING, "path": "src/lib.spx"}]});
    for (k, x) in extra.as_object().unwrap() {
        v[k] = x.clone();
    }
    R::Doc(v)
}

fn intent(extra: Value) -> R {
    let mut i = json!({"kind": "replace_function_body", "target": "t.f"});
    for (k, x) in extra.as_object().unwrap() {
        i[k] = x.clone();
    }
    R::Doc(json!({"schema": "semaprax.harness-proposal.v1", "intent": i}))
}

fn model(id: &str, rank: u32, cost: u64) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: cost,
        est_latency_ms: 10,
        strength_rank: rank,
        descriptor: Default::default(),
    }
}

fn catalog() -> Vec<ModelPlan> {
    vec![
        model("m-cheap", 1, 10),
        model("m-plan", 3, 900),
        model("m-review", 2, 500),
        model("m-strong", 4, 1000),
    ]
}

fn task(cat: &[ModelPlan], attempts: u32) -> Task {
    Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f and keep behavior".into(),
        seed: Some("t.f".into()),
        family: "mechanical".into(),
        models: Some(json!(cat
            .iter()
            .map(ModelPlan::to_json)
            .collect::<Vec<_>>())),
        budget: Some(semaprax_harness::workflow::budget::BudgetPolicy {
            output_reserve_tokens: 100,
            protocol_overhead_tokens: 20,
            ..Default::default()
        }),
        session: Some(SessionBounds {
            max_attempts: attempts,
            ..Default::default()
        }),
        ..Task::default()
    }
}

fn role(models: &[&str], enabled: bool) -> PhaseConfig {
    PhaseConfig {
        enabled,
        models: Some(models.iter().map(|m| m.to_string()).collect()),
        decision: "rules".into(),
        ..PhaseConfig::default()
    }
}

/// Planner, low-cost implementer and advisory reviewer on distinct profiles.
fn three_roles(e: &Env, t: Task) -> RunConfig {
    let mut c = config(e, t, None);
    let rates = PriceRecord {
        version: "synthetic-v1".into(),
        pricing: Pricing::Rates {
            input: Some(1_000_000),
            cache_read: Some(100_000),
            cache_write: Some(2_000_000),
            cache_write_1h: None,
            output: Some(4_000_000),
        },
    };
    c.budget.prices = PriceBook::default().with("m-", rates);
    c.budget.spend.max_task_cost_micros = Some(50_000_000);
    let ph = &mut c.routing.phases;
    ph.insert("plan".into(), role(&["m-plan"], true));
    ph.insert("implement".into(), role(&["m-cheap", "m-strong"], false));
    ph.insert("review".into(), role(&["m-review"], true));
    c
}

/// Counting v1 router (always picks `m-strong`).
struct Router(u32);
impl DecisionInvoker for Router {
    fn evaluate(&mut self, r: &semaprax_harness::contract::RequestEnvelope) -> DecisionCall {
        self.0 += 1;
        let opts: Vec<String> = r.payload["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o.as_str().unwrap().to_string())
            .collect();
        let pick = opts
            .iter()
            .find(|o| *o == "m-strong")
            .unwrap_or(&opts[0])
            .clone();
        let scores: serde_json::Map<String, Value> = opts
            .iter()
            .map(|o| (o.clone(), json!(if *o == pick { 1.0 } else { 0.0 })))
            .collect();
        DecisionCall::Answered {
            result: json!({"choice": pick, "scores": scores, "abstain": false}),
            elapsed_ms: 1,
            call: None,
        }
    }
}

fn exec(cfg: &RunConfig, fake: &Fake, roles: &Roles, router: Option<&mut Router>) -> Report {
    let mut native = NativeContext::new(fake);
    let mut view = RawCommandView;
    let mut p = RolesRef(roles);
    let decision = router.map(|inv| {
        let profile = ProviderProfile {
            provider_id: "org.example/route".into(),
            model_id: "router-m".into(),
            checkpoint: "1".into(),
            ..Default::default()
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
        fake,
        Stages {
            decision,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

fn entries(r: &Report) -> Vec<Value> {
    r.phases["entries"].as_array().cloned().unwrap_or_default()
}

fn entry<'a>(es: &'a [Value], phase: &str) -> &'a Value {
    es.iter()
        .find(|e| e["phase"] == phase)
        .unwrap_or_else(|| panic!("no `{phase}` entry in {es:?}"))
}

fn pairs(v: &[(&str, &str)]) -> Vec<(String, String)> {
    v.iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
}

#[test]
fn mr08_planner_low_cost_implementer_and_reviewer_share_one_budget_on_distinct_profiles() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = three_roles(&e, task(&catalog(), 3));
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert!(
        ["candidate-ready", "approved-candidate-ready"].contains(&r.status),
        "{} {:?}",
        r.status,
        r.refusals
    );
    assert_eq!(
        roles.calls(),
        pairs(&[
            ("plan", "m-plan"),
            ("implement", "m-cheap"),
            ("review", "m-review")
        ])
    );
    // One ledger: every role's dispatch was reserved and settled against the one task cap.
    let att = r.context["spend"]["attempts"].as_array().unwrap();
    let rows: Vec<(String, String)> = att
        .iter()
        .map(|a| {
            (
                a["label"].as_str().unwrap().into(),
                a["model"].as_str().unwrap().into(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        pairs(&[
            ("phase-plan", "m-plan"),
            ("gen-1", "m-cheap"),
            ("phase-review-1", "m-review")
        ]),
        "{att:?}"
    );
    assert!(att
        .iter()
        .all(|a| a["kind"] == "generation" && a["state"] == "settled"));
    let committed: u64 = att
        .iter()
        .map(|a| a["committed_cost_micros"].as_u64().unwrap())
        .sum();
    assert!(committed > 0 && committed <= 50_000_000);
    // The implementer saw the bounded plan reference; acceptance stayed host-fixed.
    let es = entries(&r);
    let plan = entry(&es, "plan");
    assert_eq!(
        (plan["source"].as_str(), plan["model"].as_str()),
        (Some("generated"), Some("m-plan"))
    );
    let ip = &roles.implement_prompts()[0];
    assert_eq!(ip["plan"]["digest"], plan["artifact_digest"]);
    assert_eq!(ip["acceptance"], json!([]));
    let imp = entry(&es, "implement");
    assert_eq!(
        (imp["model"].as_str(), imp["retry"].as_str()),
        (Some("m-cheap"), Some("none"))
    );
    let rev = entry(&es, "review");
    assert_eq!(rev["advisory"], true);
    assert_eq!(rev["findings"]["info"], 1);
    assert_eq!(
        rev["candidate_revision"],
        json!(sha256_plain(CHANGED.as_bytes()))
    );
    // The report carries digests and counts, never the model-written text.
    let text = r.to_json().to_string();
    assert!(!text.contains(FINDING) && !text.contains("keep the contract"));
    assert!(r.phases["parent_model"]
        .as_str()
        .unwrap()
        .starts_with("not changed"));
}

#[test]
fn mr08_a_default_or_trivial_task_makes_no_added_planner_reviewer_or_router_calls() {
    // Default: no role policy, the single-proposer loop and no phase log.
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = config(&e, task(&catalog(), 3), None);
    cfg.budget.prices = PriceBook::default();
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(roles.calls().len(), 1, "{:?}", r.refusals);
    assert!(r.phases.is_null());
    assert!(r.to_json().get("phases").is_none());

    // Configured but not triggered: the risk rule names another family, review
    // is off, and a one-candidate implement subset never consults the router.
    let e = setup(FIXED);
    let mut cfg = config(&e, task(&catalog(), 3), None);
    cfg.routing.explicit_mode = true;
    cfg.routing.cfg.mode = semaprax_harness::decision::RoutingMode::Experimental;
    cfg.routing.phases.insert(
        "plan".into(),
        PhaseConfig {
            risk_families: vec!["semantic_law".into()],
            decision: "rules".into(),
            ..PhaseConfig::default()
        },
    );
    cfg.routing.phases.insert(
        "implement".into(),
        PhaseConfig {
            models: Some(vec!["m-cheap".into()]),
            decision: "router".into(),
            ..PhaseConfig::default()
        },
    );
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let mut router = Router(0);
    let r = exec(&cfg, &fake, &roles, Some(&mut router));
    assert_eq!(
        roles.calls(),
        pairs(&[("implement", "m-cheap")]),
        "{:?}",
        r.refusals
    );
    assert_eq!(
        router.0, 0,
        "one candidate suffices: no learned router call"
    );
    let kinds: Vec<_> = r.context["spend"]["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["kind"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(kinds, ["generation"], "no router reservation either");
    // The same family under the risk rule plans exactly once.
    let e = setup(FIXED);
    let mut cfg2 = config(&e, task(&catalog(), 3), None);
    cfg2.routing.phases = cfg.routing.phases.clone();
    cfg2.task.family = "semantic_law".into();
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let r = exec(&cfg2, &fake, &roles, None);
    assert_eq!(roles.calls().len(), 2, "{:?}", r.refusals);
    assert!(entry(&entries(&r), "plan")["why"]
        .as_str()
        .unwrap()
        .starts_with("risk rule"));
}

#[test]
fn mr08_a_verified_reasoning_failure_changes_route_and_handoff_and_transport_uncertainty_is_not_retried(
) {
    // TC-10 ladder stays the single decision owner: a compiler-rejected
    // candidate escalates cheap -> strong and the next worker gets a compact handoff.
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = three_roles(&e, task(&catalog(), 3));
    cfg.routing.phases.remove("plan");
    cfg.routing.phases.remove("review");
    cfg.routing.cost_aware = true;
    cfg.routing.ladders.insert(
        "mechanical".into(),
        semaprax_harness::profile::config::LadderConfig {
            models: vec!["m-cheap".into(), "m-strong".into()],
            max_escalations: 1,
            min_tasks: 5,
        },
    );
    let roles = Roles::new(
        vec![],
        vec![
            intent(json!({"fake_refuse": "law `ensures` violated"})),
            intent(json!({})),
        ],
        vec![],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert!(r.refusals.is_empty(), "{:?}", r.refusals);
    assert_eq!(
        roles.calls(),
        pairs(&[("implement", "m-cheap"), ("implement", "m-strong")])
    );
    let ips = roles.implement_prompts();
    let h = &ips[1]["handoff"];
    assert_eq!(h["schema"], "semaprax.harness-handoff.v1");
    assert_eq!(h["revision"], json!(sha256_plain(FIXED.as_bytes())));
    assert_eq!(
        h["goal_digest"],
        json!(sha256_plain(b"rename f and keep behavior"))
    );
    assert_eq!(h["verified_diagnostics"][0]["code"], "SPX-HPD040");
    assert!(
        h["verified_diagnostics"][0].get("proposed").is_none(),
        "no earlier model output"
    );
    assert!(
        ips[1].get("feedback").is_none(),
        "the handoff replaces raw feedback"
    );
    let es = entries(&r);
    let rep = entry(&es, "repair");
    assert_eq!(
        (
            rep["previous_failure"].as_str(),
            rep["retry"].as_str(),
            rep["model"].as_str()
        ),
        (Some("semantic_law"), Some("reasoning"), Some("m-strong"))
    );
    assert!(
        rep["why"]
            .as_str()
            .unwrap()
            .contains("changed from `m-cheap`"),
        "{rep}"
    );

    // A transport-uncertain first attempt: stopped, one billable dispatch, no
    // escalation, and a rerun of the lineage does not replay it.
    let e = setup(FIXED);
    let roles = Roles::new(vec![], vec![R::Uncertain, intent(json!({}))], vec![]);
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(r.status, "uncertain", "{:?}", r.refusals);
    assert_eq!(roles.calls(), pairs(&[("implement", "m-cheap")]));
    let stop = entry(&entries(&r), "implement");
    assert_eq!(
        (stop["outcome"].as_str(), stop["retry"].as_str()),
        (Some("stopped"), Some("none"))
    );
    let again = exec(&cfg, &fake, &roles, None);
    assert_eq!(codes(&again), ["SPX-HPD072"]);
    assert_eq!(roles.calls().len(), 1, "never replayed");
    let _ = e;
}

#[test]
fn mr08_model_written_confidence_or_tests_passed_cannot_finish_or_bypass_compiler_checks() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = three_roles(&e, task(&catalog(), 4));
    let roles = Roles::new(
        vec![plan_doc(
            json!({"claims": {"tests_passed": true, "confidence": 0.99}}),
        )],
        vec![
            // "Done, tests passed" before any admitted step: not a finish.
            R::Doc(
                json!({"schema": "semaprax.harness-proposal.v1", "done": true,
                          "claims": {"tests_passed": true, "confidence": 1.0}}),
            ),
            // A candidate whose tests fail: the compiler, not the claim, decides.
            intent(
                json!({"fake_source": "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n{\n    BUG\n}\n"}),
            ),
            intent(json!({})),
        ],
        vec![review_doc(json!({"approve": true, "tests_passed": true}))],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert!(r.refusals.is_empty(), "{:?}", r.refusals);
    let attempts = r.session["attempts"].as_array().unwrap();
    assert_eq!(attempts.len(), 3);
    assert_eq!(attempts[0]["code"], "SPX-HPD116");
    assert_eq!(attempts[1]["stage"], "checks");
    assert_eq!(attempts[2]["outcome"], "admitted");
    assert!(fake.log.borrow().iter().filter(|c| *c == "test").count() >= 2);
    let es = entries(&r);
    let plan = entry(&es, "plan");
    assert_eq!(plan["source"], "generated");
    assert_eq!(
        plan["ignored_claims"],
        json!(["confidence", "tests_passed"])
    );
    let rev = entry(&es, "review");
    assert_eq!(rev["source"], "rejected");
    assert!(rev["reason"].as_str().unwrap().contains("cannot"), "{rev}");

    // A plan that tries to rewrite acceptance is rejected; the task keeps its own.
    let e = setup(FIXED);
    let mut t = task(&catalog(), 3);
    t.acceptance = vec![json!("keep behavior")];
    let cfg = three_roles(&e, t);
    let roles = Roles::new(
        vec![plan_doc(json!({"acceptance": []}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let r = exec(&cfg, &fake, &roles, None);
    let plan = entry(&entries(&r), "plan").clone();
    assert_eq!(plan["source"], "rejected", "{plan}");
    let ip = &roles.implement_prompts()[0];
    assert!(ip.get("plan").is_none());
    assert_eq!(ip["acceptance"], json!(["keep behavior"]));
}

#[test]
fn mr08_project_pin_allowlists_no_progress_confidentiality_and_cancellation_stay_enforced() {
    // The project pin keeps precedence over every role subset.
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = three_roles(&e, task(&catalog(), 3));
    cfg.routing.cfg.project_pin = Some("m-strong".into());
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert!(
        roles.calls().iter().all(|(_, m)| m == "m-strong"),
        "{:?} {:?}",
        roles.calls(),
        r.refusals
    );
    assert_eq!(roles.calls().len(), 3);

    // A role allowlist that names no approved model refuses before any call.
    let e = setup(FIXED);
    let mut cfg = three_roles(&e, task(&catalog(), 3));
    cfg.routing
        .phases
        .insert("implement".into(), role(&["ghost"], false));
    cfg.routing.phases.remove("plan");
    let roles = Roles::new(vec![], vec![intent(json!({}))], vec![]);
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(codes(&r), ["SPX-HPD118"]);
    assert!(roles.calls().is_empty());

    // No-progress detection still stops the session.
    let e = setup(FIXED);
    let mut cfg = three_roles(&e, task(&catalog(), 6));
    cfg.routing.phases.remove("plan");
    let roles = Roles::new(
        vec![],
        (0..5)
            .map(|i| intent(json!({"fake_refuse": "same law violation", "n": i})))
            .collect(),
        vec![],
    );
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(r.status, "no-progress", "{:?}", r.refusals);
    assert_eq!(roles.calls().len(), 3);

    // Confidentiality: a remote reviewer is not approved for project data, so the
    // advisory review is skipped and the remote model is never called.
    let e = setup(FIXED);
    let mut cat = catalog();
    cat.push(ModelPlan {
        destination: Destination::Remote {
            origin: "https://api.example".into(),
        },
        ..model("m-cloud", 5, 1)
    });
    let mut cfg = three_roles(&e, task(&cat, 3));
    cfg.routing.phases.remove("plan");
    cfg.routing
        .phases
        .insert("review".into(), role(&["m-cloud"], true));
    let roles = Roles::new(vec![], vec![intent(json!({}))], vec![review_doc(json!({}))]);
    let r = exec(&cfg, &fake, &roles, None);
    assert!(
        roles.calls().iter().all(|(_, m)| m != "m-cloud"),
        "{:?}",
        roles.calls()
    );
    let rev = entry(&entries(&r), "review").clone();
    assert_eq!(rev["source"], "skipped", "{rev}");

    // Cancellation before the session: no planner, no implementer.
    let e = setup(FIXED);
    let mut cfg = three_roles(&e, task(&catalog(), 3));
    let flag: CancelFlag = Default::default();
    flag.store(true, std::sync::atomic::Ordering::SeqCst);
    cfg.cancel = Some(flag);
    let roles = Roles::new(vec![plan_doc(json!({}))], vec![intent(json!({}))], vec![]);
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(r.status, "cancelled");
    assert!(roles.calls().is_empty());
}

#[test]
fn mr08_resume_reuses_recorded_phase_artifacts_and_a_non_delegating_bridge_reports_host_control() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cfg = three_roles(&e, task(&catalog(), 3));
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let first = exec(&cfg, &fake, &roles, None);
    assert_eq!(roles.calls().len(), 3, "{:?}", first.refusals);
    // Same lineage again: the plan and review artifacts and the generation come
    // from the journal; nothing is regenerated.
    let again = exec(&cfg, &fake, &roles, None);
    assert_eq!(roles.calls().len(), 3, "{:?}", again.refusals);
    let es = entries(&again);
    let plan = entry(&es, "plan");
    assert_eq!(
        (plan["source"].as_str(), plan["model_calls"].as_u64()),
        (Some("journal"), Some(0))
    );
    assert_eq!(
        plan["artifact_digest"],
        entry(&entries(&first), "plan")["artifact_digest"]
    );
    assert_eq!(entry(&es, "review")["source"], "journal");
    // A tampered artifact is refused, never silently regenerated.
    let art = e
        .cache
        .join(format!("{}.phase-plan.artifact.json", first.lineage));
    std::fs::write(&art, "{}").unwrap();
    let r = exec(&cfg, &fake, &roles, None);
    assert_eq!(codes(&r), ["SPX-HPD072"]);
    assert_eq!(roles.calls().len(), 3);

    // Bridge: a host that does not delegate model routing stays host-controlled.
    use semaprax_harness::bridge::negotiate::{decide, Availability, HostDeclaration, Owner};
    let none = HostDeclaration::default();
    let owner = decide(&none, &Availability::default())["model_routing"].owner;
    assert_eq!(owner, Owner::ExternalHost);
    let v = parent_model_routing(owner);
    assert_eq!(
        (v["mode"].as_str(), v["advisory_only"].as_bool()),
        (Some("host-controlled"), Some(true))
    );
    assert_eq!(v["changes_parent_model"], false);
    let mut del = HostDeclaration::default();
    del.declared.insert("model_routing".into(), true);
    let owner = decide(&del, &Availability::default())["model_routing"].owner;
    let v = parent_model_routing(owner);
    assert_eq!(
        (v["mode"].as_str(), v["changes_parent_model"].as_bool()),
        (Some("delegated"), Some(false))
    );
}

#[test]
fn mr08_phase_tables_parse_validate_and_leave_the_default_digest_unchanged() {
    use semaprax_harness::profile::config::parse;
    const HDR: &str = "schema = \"semaprax.harness-config.v1\"\n";
    let ok = parse(format!("{HDR}[routing.phase.plan]\nenabled = true\nmodels = [\"m-plan\"]\ndecision = \"router\"\n[routing.phase.review]\nrisk_families = [\"semantic_law\"]\npin = \"m-review\"\n[routing.phase.implement]\nmodels = [\"m-cheap\", \"m-strong\"]\n").as_bytes()).unwrap();
    let p = &ok.routing.phases;
    assert!(p["plan"].enabled && p["plan"].decision == "router");
    assert_eq!(p["review"].risk_families, ["semantic_law"]);
    assert_eq!(p["review"].pin.as_deref(), Some("m-review"));
    assert_eq!(p["implement"].decision, "rules");
    let w =
        semaprax_harness::workflow::routing::RoutingWiring::from_config(&ok.routing, None).unwrap();
    assert_eq!(w.phases.len(), 3);
    assert!(ok.to_json()["routing"]["phases"]["plan"]["enabled"]
        .as_bool()
        .unwrap());
    for (bad, code) in [
        ("[routing.phase.deploy]\nenabled = true\n", "SPX-HPB004"),
        ("[routing.phase.implement]\nenabled = true\n", "SPX-HPB004"),
        (
            "[routing.phase.plan]\nrisk_families = [\"nonsense\"]\n",
            "SPX-HPB004",
        ),
        (
            "[routing.phase.plan]\nmodels = [\"a\"]\npin = \"b\"\n",
            "SPX-HPB004",
        ),
        ("[routing.phase.plan]\ndecision = \"magic\"\n", "SPX-HPB004"),
        ("[routing.phase.plan]\nturbo = true\n", "SPX-HPB003"),
    ] {
        let err = parse(format!("{HDR}{bad}").as_bytes()).unwrap_err();
        assert_eq!(err.code, code, "{bad}: {}", err.message);
    }
    assert!(parse(HDR.as_bytes())
        .unwrap()
        .to_json()
        .get("routing")
        .is_none());
}

// ---- MR-12 through the real route path ---------------------------------------

#[test]
fn mr12_the_session_cache_in_the_route_path_reuses_an_identical_route_and_charges_nothing() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let cat = [model("m-cheap", 1, 10), model("m-strong", 2, 20)];
    let mut t = task(&cat, 3);
    t.session = None;
    let mut cfg = config(&e, t, None);
    cfg.routing.explicit_mode = true;
    cfg.routing.cfg.mode = semaprax_harness::decision::RoutingMode::Experimental;
    let mut router = Router(0);
    let roles = Roles::new(vec![], vec![intent(json!({}))], vec![]);
    let first = exec(&cfg, &fake, &roles, Some(&mut router));
    assert_eq!(router.0, 1, "{:?}", first.refusals);
    assert_eq!(first.route["choice"], "m-strong");
    assert_eq!(first.route["reuse"]["cache"], "miss");
    assert_eq!(first.context["decision_reuse"]["misses"], 1);
    // The host keeps the session (the wiring) and runs the same task again.
    std::fs::remove_dir_all(&e.cache).unwrap();
    let again = exec(&cfg, &fake, &roles, Some(&mut router));
    assert_eq!(
        router.0, 1,
        "the identical route is reused: no second inference"
    );
    assert_eq!(again.route["choice"], "m-strong");
    assert_eq!(again.route["reuse"]["cache"], "hit");
    assert_eq!(again.route["router_calls"], 0);
    assert_eq!(again.context["decision_reuse"]["hits"], 1);
    let routers: Vec<&Value> = again.context["spend"]["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["kind"] == "router")
        .collect();
    assert!(
        routers.iter().all(|a| a["state"] == "released"),
        "{routers:?}"
    );
    // Scripted proposals and cancellation still make zero router calls.
    std::fs::remove_dir_all(&e.cache).unwrap();
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert!(r.route.is_null() || r.route["router_calls"] == 0);
    let mut cancelled = three_roles(&e, task(&catalog(), 3));
    let flag: CancelFlag = Default::default();
    flag.store(true, std::sync::atomic::Ordering::SeqCst);
    cancelled.cancel = Some(flag);
    let mut router = Router(0);
    let r = exec(&cancelled, &fake, &roles, Some(&mut router));
    assert_eq!((r.status, router.0), ("cancelled", 0));
}
