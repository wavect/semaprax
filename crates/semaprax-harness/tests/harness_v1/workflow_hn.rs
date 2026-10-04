//! HN-01 (explicit tasks), HN-11 (request budget) and HN-02 (bounded session)
//! tests that need no real compiler. Real-compiler evidence is in
//! tests/real_tools_v1/workflow_compiler.rs. Fixture prefix `hp-hn`.

use super::*;
use semaprax_harness::decision::{Destination, ModelPlan};
use semaprax_harness::observe::Tokenizer;
use semaprax_harness::workflow::budget::{BudgetPolicy, ModelTokenizerMap, TokenizerSet};

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";

/// Records every request; yields scripted proposals in order (the last repeats).
struct Seq {
    items: Vec<Vec<u8>>,
    prompts: RefCell<Vec<Value>>,
    models: RefCell<Vec<String>>,
    calls: Cell<u32>,
    side: bool,
    uncertain_at: Option<usize>,
    cancel: Option<CancelFlag>,
    cancel_at: Option<usize>,
}

impl Seq {
    fn new(items: Vec<Value>) -> Self {
        Seq {
            items: items.iter().map(|v| v.to_string().into_bytes()).collect(),
            prompts: RefCell::default(),
            models: RefCell::default(),
            calls: Cell::new(0),
            side: false,
            uncertain_at: None,
            cancel: None,
            cancel_at: None,
        }
    }
}

struct SeqRef<'a>(&'a Seq);
impl ProposalStage for SeqRef<'_> {
    fn id(&self) -> String {
        "org.example/seq".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        let s = self.0;
        let i = s.calls.get() as usize;
        s.calls.set(s.calls.get() + 1);
        s.prompts.borrow_mut().push(r.prompt.clone());
        s.models.borrow_mut().push(r.model.clone());
        if s.cancel_at == Some(i) {
            s.cancel
                .as_ref()
                .unwrap()
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
        if s.uncertain_at == Some(i) {
            return Err(StageFailure::Uncertain(
                semaprax_harness::diag::HarnessDiagnostic::new(
                    "SPX-HPD072",
                    "connection lost after send",
                ),
            ));
        }
        Ok(s.items[i.min(s.items.len() - 1)].clone())
    }
    fn calls(&self) -> u32 {
        self.0.calls.get()
    }
    fn side_effecting(&self) -> bool {
        self.0.side
    }
}

fn intent(kind: &str, extra: Value) -> Value {
    let mut i = json!({"kind": kind, "target": "t.f"});
    for (k, v) in extra.as_object().unwrap() {
        i[k] = v.clone();
    }
    json!({"schema": "semaprax.harness-proposal.v1", "intent": i})
}

fn change_task(extra: impl FnOnce(&mut Task)) -> Task {
    let mut t = Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename f and keep behavior".into(),
        seed: Some("t.f".into()),
        ..Task::default()
    };
    extra(&mut t);
    t
}

fn drive_with(cfg: &RunConfig, fake: &Fake, seq: &Seq, obs: &mut Observer) -> Report {
    let mut native = NativeContext::new(fake);
    let mut p = SeqRef(seq);
    let mut view = RawCommandView;
    run(
        cfg,
        fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        obs,
    )
}

fn once(cfg: &RunConfig, fake: &Fake, seq: &Seq) -> Report {
    drive_with(
        cfg,
        fake,
        seq,
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

// ---- HN-01 -------------------------------------------------------------

#[test]
fn hp_hn01_healthy_change_task_reaches_proposal_preview_and_candidate_with_v2_report() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let cfg = config(&e, change_task(|_| {}), None);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(seq.calls.get(), 1);
    let log = fake.log.borrow();
    assert!(log.contains(&"preview".to_string()), "{log:?}");
    assert_eq!(r.candidate["changed_files"], json!(["src/lib.spx"]));
    assert_eq!(
        r.checks["tests"], "passed",
        "baseline tests still pass on the candidate"
    );
    let j = r.to_json();
    assert_eq!(j["schema"], "semaprax.harness-run.v2");
    assert_eq!(j["task"]["mode"], "change");
    assert_eq!(j["task"]["task_family"], "localized_debug");
    assert!(j["operations"]["kinds"].as_array().unwrap().len() >= 10);
    assert!(
        !j.to_string().contains("rename f and keep behavior"),
        "goal text stays out of reports"
    );
    assert_eq!(r.exit_code(), 0);
    // The goal reached the model request and the original is untouched.
    assert_eq!(
        seq.prompts.borrow()[0]["goal"],
        "rename f and keep behavior"
    );
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        FIXED
    );
}

#[test]
fn hp_hn01_no_task_and_explicit_repair_mode_keep_the_noop_and_make_no_model_call() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let r = once(&config(&e, Task::default(), None), &fake, &seq);
    assert_eq!(r.status, "no-repair-needed");
    assert_eq!(r.to_json()["schema"], "semaprax.harness-run.v1");
    assert_eq!(seq.calls.get(), 0);
    assert!(!fake.log.borrow().contains(&"preview".to_string()));
    // Explicit v2 repair mode: a distinct, named status; still no model call.
    let t = Task {
        schema_version: 2,
        mode: TaskMode::Repair,
        ..Task::default()
    };
    let r = once(&config(&e, t, None), &fake, &seq);
    assert_eq!(r.status, "unchanged-repair-baseline");
    assert_eq!(seq.calls.get(), 0);
    assert_eq!(r.exit_code(), 0);
    // A v1 task that carries a goal is still repair; it says so instead of pretending.
    let t = Task {
        goal: "add a feature".into(),
        ..Task::default()
    };
    let r = once(&config(&e, t, None), &fake, &seq);
    assert_eq!(r.status, "no-repair-needed");
    assert!(r.notes.iter().any(|n| n.contains("harness-task.v2")));
}

#[test]
fn hp_hn01_unsupported_goal_is_never_reported_as_completed_because_tests_are_green() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    // Task names an operation the installed compiler lacks: no model call at all.
    *fake.ops.borrow_mut() = Some(vec!["rename_declaration".into()]);
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let cfg = config(
        &e,
        change_task(|t| t.operation = Some("add_endpoint".into())),
        None,
    );
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "unsupported-goal");
    assert_eq!(codes(&r), ["SPX-HPD092"]);
    assert!(
        r.refusals[0].message.contains("rename_declaration"),
        "names the installed operations"
    );
    assert_eq!(seq.calls.get(), 0);
    assert_ne!(r.exit_code(), 0);
    // A proposal using an operation the compiler does not admit is the same status.
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let r = once(&config(&e, change_task(|_| {}), None), &fake, &seq);
    assert_eq!(
        (r.status, codes(&r)),
        ("unsupported-goal", vec!["SPX-HPD092"])
    );
    // An unknown kind and the proposer's own `unsupported` statement likewise.
    let fake = Fake::new(CHANGED);
    for body in [
        intent("add_endpoint", json!({})),
        json!({"schema": "semaprax.harness-proposal.v1", "unsupported": "needs an HTTP server"}),
    ] {
        let r = once(
            &config(&e, change_task(|_| {}), None),
            &fake,
            &Seq::new(vec![body]),
        );
        assert_eq!(r.status, "unsupported-goal", "{:?}", r.refusals);
    }
    // Raw source remains a refusal, not an unsupported goal.
    let raw = json!({"schema": "semaprax.harness-proposal.v1", "source": "x"});
    let r = once(
        &config(&e, change_task(|_| {}), None),
        &fake,
        &Seq::new(vec![raw]),
    );
    assert_eq!((r.status, codes(&r)), ("refused", vec!["SPX-HPD031"]));
}

#[test]
fn hp_hn01_plan_mode_is_read_only_and_a_failing_baseline_is_diagnosed_not_forced() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let cfg = config(
        &e,
        change_task(|t| t.mode = TaskMode::Plan),
        Some(policy_files(&e)),
    );
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "planned", "{:?}", r.refusals);
    let log = fake.log.borrow().clone();
    assert_eq!(
        log.iter().filter(|c| *c == "check").count(),
        1,
        "no candidate check ran: {log:?}"
    );
    assert_eq!(fake.publishes.get(), 0);
    assert!(r.approval.is_null() && r.publication.is_null());
    let capsules: Vec<_> = std::fs::read_dir(&e.cache)
        .map(|d| {
            d.flatten()
                .filter(|x| x.file_name().to_string_lossy().contains("capsule"))
                .collect()
        })
        .unwrap_or_default();
    assert!(capsules.is_empty(), "plan exports nothing");
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        FIXED
    );
    // Plan with no proposal source still plans (context + operations).
    let r = drive_plan_without_proposer(&e);
    assert_eq!(r.status, "planned");
    // Baseline tests failing: change mode is diagnosed truthfully, no model call.
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let seq = Seq::new(vec![intent("replace_function_body", json!({}))]);
    let r = once(&config(&e, change_task(|_| {}), None), &fake, &seq);
    assert_eq!(r.status, "diagnosed");
    assert_eq!(seq.calls.get(), 0);
    // Unverified baseline without a session block: diagnosed, delegated to HN-02.
    let e = setup(&LIB.replace("BUG", "SYNTAXERR"));
    let r = once(&config(&e, change_task(|_| {}), None), &fake, &seq);
    assert_eq!(r.status, "diagnosed");
    assert!(r.notes.iter().any(|n| n.contains("session")));
}

fn drive_plan_without_proposer(e: &Env) -> Report {
    let fake = Fake::new(CHANGED);
    let cfg = config(e, change_task(|t| t.mode = TaskMode::Plan), None);
    let mut native = NativeContext::new(&fake);
    let mut p = ScriptedProposer::empty();
    let mut view = RawCommandView;
    run(
        &cfg,
        &fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

#[test]
fn hp_hn01_task_v2_parse_is_additive_and_strict() {
    let v1 = br#"{"schema":"semaprax.harness-task.v1","goal":"g"}"#;
    let t = Task::parse(v1).unwrap();
    assert_eq!((t.schema_version, t.mode), (1, TaskMode::Repair));
    // v1 does not accept v2 members.
    assert_eq!(
        Task::parse(br#"{"schema":"semaprax.harness-task.v1","mode":"change"}"#)
            .unwrap_err()
            .code,
        "SPX-HPD081"
    );
    let ok = br#"{"schema":"semaprax.harness-task.v2","mode":"change","goal":"g","acceptance":["a",{"stable_id":"t.f","contains":"x"}],"operation":"rename_declaration","budget":{"max_task_tokens":900}}"#;
    let t = Task::parse(ok).unwrap();
    assert_eq!(
        (
            t.mode,
            t.acceptance.len(),
            t.budget.unwrap().max_task_tokens
        ),
        (TaskMode::Change, 2, Some(900))
    );
    for bad in [
        r#"{"schema":"semaprax.harness-task.v2","mode":"change"}"#,
        r#"{"schema":"semaprax.harness-task.v2","mode":"change","goal":""}"#,
        r#"{"schema":"semaprax.harness-task.v2","mode":"fly","goal":"g"}"#,
        r#"{"schema":"semaprax.harness-task.v2","goal":"g","acceptance":[1]}"#,
    ] {
        assert_eq!(
            Task::parse(bad.as_bytes()).unwrap_err().code,
            "SPX-HPD081",
            "{bad}"
        );
    }
    // The v1 digest is stable (lineage ids of old tasks do not move).
    assert_eq!(Task::default().digest(), Task::default().digest());
}

// ---- HN-11 -------------------------------------------------------------

/// Words are one token; every other character (punctuation, CJK, emoji) is one
/// token, as in byte-level BPE on code and non-Latin text.
struct Piece;
impl Tokenizer for Piece {
    fn name(&self) -> &str {
        "fake-piece"
    }
    fn fingerprint(&self) -> &str {
        "fp-1"
    }
    fn count(&self, text: &str) -> usize {
        let (mut n, mut word) = (0, false);
        for c in text.chars() {
            if c.is_ascii_alphanumeric() {
                if !word {
                    n += 1;
                }
                word = true;
            } else {
                word = false;
                if !c.is_whitespace() {
                    n += 1;
                }
            }
        }
        n
    }
}

fn plan(id: &str, max: u64, cost: u64) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: false,
        max_context: max,
        est_cost_micros: cost,
        est_latency_ms: 10,
        strength_rank: 1,
    }
}

fn budget_cfg(e: &Env, models: Vec<ModelPlan>, goal: &str, with_tok: bool) -> RunConfig {
    let task = change_task(|t| {
        t.goal = goal.into();
        t.models = Some(json!(models
            .iter()
            .map(ModelPlan::to_json)
            .collect::<Vec<_>>()));
        t.budget = Some(BudgetPolicy {
            output_reserve_tokens: 100,
            protocol_overhead_tokens: 20,
            ..Default::default()
        });
    });
    let mut cfg = config(e, task, None);
    cfg.budget.map = ModelTokenizerMap::empty().with("m-", "fake-piece");
    if with_tok {
        cfg.budget.tokenizers.add(Box::new(Piece));
    }
    cfg
}

fn ok_body() -> Value {
    intent("replace_function_body", json!({}))
}

#[test]
fn hp_hn11_the_count_is_the_exact_serialized_request_not_a_sum_of_boundaries() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let seq = Seq::new(vec![ok_body()]);
    let r = once(
        &budget_cfg(&e, vec![plan("m-a", 1_000_000, 0)], "g", true),
        &fake,
        &seq,
    );
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let sent = semaprax_harness::json::canonical(&seq.prompts.borrow()[0]);
    let b = &r.context["request_budget"];
    assert_eq!(b["request_tokens"], Piece.count(&sent));
    assert_eq!(b["request_bytes"], sent.len());
    assert_eq!(b["tokenizer"]["kind"], "named");
    assert_eq!(b["tokenizer"]["name"], "fake-piece");
    assert_eq!(b["measured"], true);
    assert_eq!(b["required_tokens"], Piece.count(&sent) + 120);
    // Summing separately counted parts differs: JSON framing is part of the request.
    let parts: usize = ["g", "diagnostics"].iter().map(|p| Piece.count(p)).sum();
    assert_ne!(b["request_tokens"].as_u64().unwrap() as usize, parts);
}

#[test]
fn hp_hn11_skill_plus_framing_over_the_limit_repacks_reroutes_or_refuses_before_generation() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let skill = "fn x(a: i64) -> i64 { (a + 1) * 2 } ".repeat(60);
    let with_skill = |cfg: &mut RunConfig| {
        cfg.skill_prompt = Some(SkillPromptUse {
            model_visible_bytes: skill.len(),
            loaded: vec!["official".into()],
            cost_report: None,
            text: skill.clone(),
        });
    };
    // Reference sizes: without and with the skill.
    let base = once(
        &budget_cfg(&e, vec![plan("m-a", 1_000_000, 0)], "g", true),
        &fake,
        &Seq::new(vec![ok_body()]),
    );
    let t0 = base.context["request_budget"]["required_tokens"]
        .as_u64()
        .unwrap();
    let mut cfg = budget_cfg(&e, vec![plan("m-a", 1_000_000, 0)], "g", true);
    with_skill(&mut cfg);
    let full = once(&cfg, &fake, &Seq::new(vec![ok_body()]));
    let t1 = full.context["request_budget"]["required_tokens"]
        .as_u64()
        .unwrap();
    assert!(t1 > t0 + 100, "the skill is real weight ({t0} vs {t1})");
    // Context fits, skill + framing does not: the optional skill is dropped whole.
    let mut cfg = budget_cfg(&e, vec![plan("m-a", t0 + 10, 0)], "g", true);
    with_skill(&mut cfg);
    let seq = Seq::new(vec![ok_body()]);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert!(
        seq.prompts.borrow()[0].get("skills").is_none(),
        "skill dropped, never truncated"
    );
    assert_eq!(
        r.context["request_budget"]["dropped_optional"],
        json!(["skills"])
    );
    assert!(r.context["request_budget"]["fits"].as_bool().unwrap());
    // A bigger model (same content) keeps the skill.
    let mut cfg = budget_cfg(&e, vec![plan("m-a", t1 + 10, 0)], "g", true);
    with_skill(&mut cfg);
    let seq = Seq::new(vec![ok_body()]);
    once(&cfg, &fake, &seq);
    assert!(seq.prompts.borrow()[0].get("skills").is_some());
    // Reroute: the cheapest model has no named tokenizer, so its bytes upper
    // bound does not fit; the request is revalidated and sent to the other model.
    let big_bytes = semaprax_harness::json::canonical(&seq.prompts.borrow()[0]).len() as u64;
    let mut cfg = budget_cfg(
        &e,
        vec![plan("z-unk", t0 + 10, 0), plan("m-a", t1 + 10, 10)],
        "g",
        true,
    );
    with_skill(&mut cfg);
    assert!(big_bytes > t0 + 10);
    let seq = Seq::new(vec![ok_body()]);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(seq.models.borrow()[0], "m-a");
    assert_eq!(
        r.context["request_budget"]["rerouted_from"][0]["model"],
        "z-unk"
    );
    // Refusal before any generation: the mandatory content cannot fit anywhere.
    let seq = Seq::new(vec![ok_body()]);
    let r = once(
        &budget_cfg(&e, vec![plan("m-a", 150, 0)], "g", true),
        &fake,
        &seq,
    );
    assert_eq!((r.status, codes(&r)), ("refused", vec!["SPX-HPD100"]));
    assert_eq!(
        seq.calls.get(),
        0,
        "no provider call before the request fits"
    );
    assert!(r.refusals[0].message.contains("protected content"));
}

#[test]
fn hp_hn11_code_and_unicode_input_defeat_the_byte_heuristic_and_required_parts_survive() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    for goal in [
        "{}[]();,:<>=+-*/&|!?".repeat(30),
        "日本語のコード、記号：".repeat(40),
    ] {
        let probe = once(
            &budget_cfg(&e, vec![plan("m-a", 10_000_000, 0)], &goal, true),
            &fake,
            &Seq::new(vec![ok_body()]),
        );
        let b = &probe.context["request_budget"];
        let (bytes, tokens) = (
            b["request_bytes"].as_u64().unwrap(),
            b["request_tokens"].as_u64().unwrap(),
        );
        let limit = bytes / 4 + 120 + 50;
        assert!(bytes / 4 + 120 <= limit, "bytes/4 would admit this request");
        assert!(tokens + 120 > limit, "but it really is {tokens} tokens");
        let seq = Seq::new(vec![ok_body()]);
        let r = once(
            &budget_cfg(&e, vec![plan("m-a", limit, 0)], &goal, true),
            &fake,
            &seq,
        );
        assert_eq!(codes(&r), ["SPX-HPD100"], "{goal:.12}");
        assert_eq!(seq.calls.get(), 0);
    }
    // Required facts and the output reserve survive trimming of optional context.
    let skill = "SKILL ".repeat(400);
    let mut cfg = budget_cfg(&e, vec![plan("m-a", 600, 0)], "g", true);
    cfg.skill_prompt = Some(SkillPromptUse {
        model_visible_bytes: skill.len(),
        loaded: vec![],
        cost_report: None,
        text: skill,
    });
    let seq = Seq::new(vec![ok_body()]);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let p = &seq.prompts.borrow()[0];
    assert_eq!(p["goal"], "g");
    assert!(p["context"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["provenance"] == "compiler-verified"));
    assert!(p["intents"].as_array().unwrap().len() > 3 && p["acceptance"].is_array());
    let b = &r.context["request_budget"];
    assert_eq!(b["output_reserve_tokens"], 100);
    assert!(b["required_tokens"].as_u64().unwrap() <= 600);
}

#[test]
fn hp_hn11_unknown_tokenizer_reports_unavailable_fields_and_admits_on_the_byte_upper_bound() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    // No tokenizer supplied: explicit unknown, never labelled measured.
    let r = once(
        &budget_cfg(&e, vec![plan("m-a", 1_000_000, 0)], "g", false),
        &fake,
        &Seq::new(vec![ok_body()]),
    );
    let b = &r.context["request_budget"];
    assert_eq!(b["measured"], false);
    assert!(b["request_tokens"].is_null());
    assert_eq!(b["tokenizer"]["kind"], "unknown");
    assert_eq!(b["admission_basis"], "utf8-bytes-upper-bound");
    assert_eq!(b["admission_tokens"], b["request_bytes"]);
    let l = &r.context["task_ledger"];
    assert_eq!(l["named_input_tokens"], json!({}));
    assert_eq!(l["unknown_tokenizer_upper_bound_bytes"], b["request_bytes"]);
    // A model with no mapping at all is unknown too, with the same policy.
    let r = once(
        &budget_cfg(&e, vec![plan("llama", 1_000_000, 0)], "g", true),
        &fake,
        &Seq::new(vec![ok_body()]),
    );
    assert_eq!(r.context["request_budget"]["tokenizer"]["kind"], "unknown");
    // The byte upper bound refuses where a real count would admit.
    let probe = once(
        &budget_cfg(&e, vec![plan("m-a", 1_000_000, 0)], "g", true),
        &fake,
        &Seq::new(vec![ok_body()]),
    );
    let tokens = probe.context["request_budget"]["required_tokens"]
        .as_u64()
        .unwrap();
    let seq = Seq::new(vec![ok_body()]);
    let r = once(
        &budget_cfg(&e, vec![plan("llama", tokens + 5, 0)], "g", true),
        &fake,
        &seq,
    );
    assert_eq!(codes(&r), ["SPX-HPD100"]);
    // Mapping is explicit data: prefixes are matched longest-first, nothing is guessed.
    let m = ModelTokenizerMap::default();
    assert_eq!(m.tokenizer_for("gpt-4o-mini"), Some("o200k_base"));
    assert_eq!(m.tokenizer_for("gpt-4-turbo"), Some("cl100k_base"));
    assert_eq!(m.tokenizer_for("claude-x"), None);
    assert!(ModelTokenizerMap::from_json(&json!({"x-": "made-up"})).is_err());
    let _ = TokenizerSet::default();
}

struct Pick(u32, &'static str);
impl semaprax_harness::decision::DecisionInvoker for Pick {
    fn evaluate(
        &mut self,
        _r: &semaprax_harness::contract::RequestEnvelope,
    ) -> semaprax_harness::decision::DecisionCall {
        self.0 += 1;
        semaprax_harness::decision::DecisionCall::Answered {
            result: json!({"choice": self.1, "scores": {self.1: 0.9}, "abstain": false}),
            elapsed_ms: 1,
        }
    }
}

#[test]
fn hp_hn11_repeated_turns_and_router_overhead_reconcile_with_observations_without_double_counting()
{
    use semaprax_harness::decision::{EnablementGate, ProviderMode, ProviderProfile};
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut cfg = budget_cfg(
        &e,
        vec![plan("m-a", 1_000_000, 0), plan("m-b", 1_000_000, 1)],
        "g",
        true,
    );
    cfg.task.session = Some(SessionBounds {
        max_attempts: 3,
        ..Default::default()
    });
    // Attempt one is refused by the compiler; attempt two is admitted.
    let seq = Seq::new(vec![
        intent(
            "replace_function_body",
            json!({"fake_refuse": "SPX-G225 bad"}),
        ),
        ok_body(),
    ]);
    let mut inv = Pick(0, "m-a");
    let profile = ProviderProfile {
        provider_id: "org.example/router".into(),
        model_id: "m".into(),
        checkpoint: "1".into(),
        min_confidence: None,
        max_context_tokens: None,
        supported_families: None,
    };
    let gate = EnablementGate::not_evaluated("model-route/v1", &profile.provider_id);
    let mut native = NativeContext::new(&fake);
    let mut p = SeqRef(&seq);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run(
        &cfg,
        &fake,
        Stages {
            decision: Some(DecisionStage {
                invoker: &mut inv,
                profile,
                mode: ProviderMode::Explicit,
                gate,
            }),
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(inv.0, 2, "one router call per generation turn");
    let ledger = &r.context["task_ledger"];
    let entries = ledger["entries"].as_array().unwrap();
    let kinds: Vec<_> = entries
        .iter()
        .map(|x| x["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["router", "generation", "router", "generation"]);
    // Every ledger entry has exactly one incurred observation with the same named count.
    let incurred: Vec<u64> = obs
        .events()
        .iter()
        .filter(|x| x.incurred.is_some() && x.stage.as_str() != "command_view")
        .map(|x| x.incurred.as_ref().unwrap().value)
        .collect();
    let (gen_events, dec_events): (Vec<_>, Vec<_>) = obs
        .events()
        .iter()
        .filter(|x| x.incurred.is_some())
        .partition(|x| x.stage.as_str() == "generation");
    assert_eq!((gen_events.len(), dec_events.len()), (2, 2));
    // The router provider is unmapped (unknown): byte-only; generation is named.
    assert!(dec_events
        .iter()
        .all(|x| x.incurred.as_ref().unwrap().tokenizer
            == semaprax_harness::observe::TokenizerId::ByteOnly));
    let named_sum: u64 = gen_events
        .iter()
        .map(|x| x.incurred.as_ref().unwrap().value)
        .sum();
    assert_eq!(ledger["named_input_tokens"]["fake-piece@fp-1"], named_sum);
    let byte_sum: u64 = dec_events
        .iter()
        .map(|x| x.incurred.as_ref().unwrap().value)
        .sum();
    assert_eq!(ledger["unknown_tokenizer_upper_bound_bytes"], byte_sum);
    assert_eq!(incurred.len(), 4);
    // The export keeps named and byte-only apart: measured rows carry tokens, others do not.
    let rows = semaprax_harness::observe::export::rows(obs.events(), "s");
    let tok_rows: Vec<&Value> = rows.iter().filter(|x| x["tokens"].is_u64()).collect();
    assert_eq!(
        tok_rows
            .iter()
            .map(|x| x["tokens"].as_u64().unwrap())
            .sum::<u64>(),
        named_sum
    );
    assert!(rows
        .iter()
        .filter(|x| x["boundary"] == "decision" && x["bytes"].is_u64())
        .all(|x| x["tokens"].is_null()));
    // The two turns differ only by the fed-back diagnostic.
    assert!(seq.prompts.borrow()[1]["feedback"]
        .to_string()
        .contains("SPX-G225"));
}

// ---- HN-02 (scripted proposers, no real compiler) -----------------------

fn session(e: &Env, extra: impl FnOnce(&mut Task)) -> RunConfig {
    config(
        e,
        change_task(|t| {
            t.session = Some(SessionBounds {
                max_attempts: 3,
                ..Default::default()
            });
            extra(t)
        }),
        None,
    )
}

#[test]
fn hp_hn02_diagnostic_reaches_the_next_attempt_and_whole_task_bounds_hold() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let refuse = |m: &str| intent("replace_function_body", json!({"fake_refuse": m}));
    let seq = Seq::new(vec![
        refuse("SPX-G225 candidate intention is missing a required field"),
        ok_body(),
    ]);
    let r = once(&session(&e, |_| {}), &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    let fb = seq.prompts.borrow()[1]["feedback"].to_string();
    assert!(
        fb.contains("SPX-G225 candidate intention is missing a required field"),
        "{fb}"
    );
    assert_eq!(seq.prompts.borrow()[1]["attempt"], 2);
    assert_eq!(r.session["attempts_spent"], 2);
    assert_eq!(r.session["attempts"][0]["outcome"], "rejected");
    assert_eq!(r.session["attempts"][1]["outcome"], "admitted");
    // Attempts exhausted: all spent attempts remain counted.
    let seq = Seq::new(vec![refuse("a"), refuse("b"), refuse("c"), refuse("d")]);
    let r = once(
        &session(&e, |t| t.session.as_mut().unwrap().max_attempts = 2),
        &fake,
        &seq,
    );
    assert_eq!((r.status, codes(&r)), ("exhausted", vec!["SPX-HPD111"]));
    assert_eq!(
        (seq.calls.get(), r.session["attempts_spent"].as_u64()),
        (2, Some(2))
    );
    // The same bound across other axes: tokens, candidates, elapsed time.
    for (key, val) in [
        ("max_candidates", 1u64),
        ("max_elapsed_ms", 0),
        ("max_tokens", 1),
    ] {
        let seq = Seq::new(vec![
            intent("replace_function_body", json!({"fake_source": LIB})),
            refuse("z"),
        ]);
        let cfg = session(&e, |t| {
            let s = t.session.as_mut().unwrap();
            match key {
                "max_candidates" => s.max_candidates = val as u32,
                "max_elapsed_ms" => s.max_elapsed_ms = val,
                _ => s.max_tokens = Some(val),
            }
        });
        let r = once(&cfg, &fake, &seq);
        assert_eq!(r.status, "exhausted", "{key}: {:?}", r.refusals);
    }
    // A whole-task token budget on the ledger refuses before the next call starts.
    let cfg = session(&e, |t| {
        t.budget = Some(BudgetPolicy {
            max_task_tokens: Some(5000),
            ..Default::default()
        })
    });
    let seq = Seq::new(vec![refuse("a"), refuse("b")]);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(codes(&r), ["SPX-HPD101"]);
    assert_eq!(seq.calls.get(), 1);
}

#[test]
fn hp_hn02_repeated_identical_bad_proposals_stop_and_candidates_failing_checks_feed_back() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    // The candidate still has BUG: tests fail; the exact failure is fed back.
    let bad = intent("replace_function_body", json!({"fake_source": LIB}));
    let seq = Seq::new(vec![bad.clone(), bad.clone()]);
    let r = once(
        &session(&e, |t| t.session.as_mut().unwrap().max_attempts = 5),
        &fake,
        &seq,
    );
    assert_eq!((r.status, codes(&r)), ("no-progress", vec!["SPX-HPD112"]));
    assert_eq!(
        seq.calls.get(),
        2,
        "the repeat is detected on the second identical proposal"
    );
    assert_eq!(r.session["attempts_spent"], 2);
    assert!(seq.prompts.borrow()[1]["feedback"]
        .to_string()
        .contains("tests failed"));
    // Three identical diagnostics from different proposals also stop.
    let diff = |n: u32| intent("replace_function_body", json!({"fake_source": LIB, "n": n}));
    let seq = Seq::new(vec![diff(1), diff(2), diff(3), diff(4)]);
    let r = once(
        &session(&e, |t| t.session.as_mut().unwrap().max_attempts = 6),
        &fake,
        &seq,
    );
    assert_eq!(r.status, "no-progress");
    assert_eq!(r.session["attempts_spent"], 3);
}

#[test]
fn hp_hn02_cancellation_and_uncertain_generation_are_recorded_and_never_replayed() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let refuse = |m: &str| intent("replace_function_body", json!({"fake_refuse": m}));
    // Cancellation during attempt one: stops before attempt two.
    let flag: CancelFlag = Default::default();
    let mut seq = Seq::new(vec![refuse("a"), ok_body()]);
    seq.cancel = Some(flag.clone());
    seq.cancel_at = Some(0);
    let mut cfg = session(&e, |_| {});
    cfg.cancel = Some(flag);
    let r = once(&cfg, &fake, &seq);
    assert_eq!((r.status, codes(&r)), ("cancelled", vec!["SPX-HPD113"]));
    assert_eq!(seq.calls.get(), 1);
    let j = std::fs::read_to_string(e.cache.join(format!("{}.journal.jsonl", r.lineage))).unwrap();
    assert!(j.contains("\"cancelled\""), "{j}");
    // A side-effecting generation whose outcome is unknown is recorded uncertain...
    let e = setup(FIXED);
    let mut seq = Seq::new(vec![ok_body()]);
    seq.side = true;
    seq.uncertain_at = Some(0);
    let r = once(&session(&e, |_| {}), &fake, &seq);
    assert_eq!((r.status, codes(&r)), ("uncertain", vec!["SPX-HPD072"]));
    assert!(
        std::fs::read_to_string(e.cache.join(format!("{}.journal.jsonl", r.lineage)))
            .unwrap()
            .contains("\"uncertain\"")
    );
    // ...and a restart (same lineage) does not call the provider again.
    let mut again = Seq::new(vec![ok_body()]);
    again.side = true;
    let r2 = once(&session(&e, |_| {}), &fake, &again);
    assert_eq!((r2.status, again.calls.get()), ("uncertain", 0));
    // A crash after `begin` (no terminal record) is the same: not replayed.
    let e = setup(FIXED);
    let cfg = session(&e, |_| {});
    let lineage = {
        let l = semaprax_harness::workflow::lineage::Lineage::new(
            cfg.snapshot.binding(),
            &cfg.lock_digest,
            &cfg.task.digest(),
        );
        l.id
    };
    write(
        &e.cache,
        &format!("{lineage}.journal.jsonl"),
        "{\"detail\":{},\"seq\":1,\"state\":\"begin\",\"step\":\"gen-1\"}\n",
    );
    let mut seq = Seq::new(vec![ok_body()]);
    seq.side = true;
    let r = once(&cfg, &fake, &seq);
    assert_eq!((r.status, seq.calls.get()), ("uncertain", 0));
}

fn setup_with_oracle() -> Env {
    let e = setup(FIXED);
    write(
        &e.project,
        "semaprax.toml",
        "schema = \"semaprax.manifest.v1\"\n[modules]\ntests = [\"t.tests\"]\n",
    );
    write(
        &e.project,
        "src/tests.spx",
        "module t.tests;\n@id(\"t.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    e
}

#[test]
fn hp_hn02_malicious_proposals_cannot_edit_the_oracle_remove_laws_expand_grants_or_fake_checks() {
    let e = setup_with_oracle();
    let fake = Fake::new(CHANGED);
    // Targeting the acceptance oracle is refused as feedback; the next honest attempt passes.
    let evil = json!({"schema": "semaprax.harness-proposal.v1", "intent": {"kind": "replace_function_body", "target": "t.tests.main", "body": {"kind": "i64", "value": 0}}});
    let seq = Seq::new(vec![evil, ok_body()]);
    let r = once(&session(&e, |_| {}), &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(r.session["attempts"][0]["code"], "SPX-HPD114");
    assert!(seq.prompts.borrow()[1]["feedback"]
        .to_string()
        .contains("acceptance oracle"));
    assert_eq!(
        fake.log.borrow().iter().filter(|c| *c == "preview").count(),
        2,
        /* validate + export of the honest attempt only */
        "the oracle edit never reached the compiler"
    );
    // Grants, authority and publication-like members never reach the compiler.
    for member in ["grant", "authority", "publish", "approved"] {
        let mut p = ok_body();
        p[member] = json!(true);
        let r = once(
            &session(&e, |_| {}),
            &Fake::new(CHANGED),
            &Seq::new(vec![p]),
        );
        assert_eq!(codes(&r), ["SPX-HPD033"], "{member}");
    }
    // Weakened requirements and a removed law in the candidate fail on the compiler's own preview.
    let nolaw = FIXED.replace("    ensures result == x\n", "");
    let r = once(
        &session(&e, |_| {}),
        &Fake::new(CHANGED),
        &Seq::new(vec![
            intent("replace_function_body", json!({"fake_source": nolaw})),
            ok_body(),
        ]),
    );
    assert_eq!(r.session["attempts"][0]["code"], "SPX-HPD042");
    let weak = Fake::new(CHANGED);
    weak.requirements.borrow_mut().pop();
    let r = once(&session(&e, |_| {}), &weak, &Seq::new(vec![ok_body()]));
    assert_eq!(r.session["attempts"][0]["code"], "SPX-HPD044");
    // A claim that tests passed is ignored; the compiler's verdict decides.
    let claim = json!({"schema": "semaprax.harness-proposal.v1", "claims": {"tests_passed": true},
        "intent": {"kind": "replace_function_body", "target": "t.f", "fake_source": LIB}});
    let r = once(
        &session(&e, |t| t.session.as_mut().unwrap().max_attempts = 1),
        &Fake::new(CHANGED),
        &Seq::new(vec![claim]),
    );
    assert_eq!(r.status, "exhausted");
    assert_eq!(r.ignored_claims, ["tests_passed"]);
    assert!(
        r.candidate["candidate_revision"].is_string()
            && r.session["steps"].as_array().unwrap().is_empty()
    );
}

fn patch(path: &str, find: &str, replace: &str) -> Value {
    json!({"schema": "semaprax.harness-proposal.v1", "source_patch": {"edits": [{"path": path, "find": find, "replace": replace}]}})
}

#[test]
fn hp_hn02_unverified_baseline_is_repaired_in_scratch_and_the_original_is_untouched_on_failure() {
    let broken = FIXED.replace("    x\n}", "    SYNTAXERR\n}");
    let e = setup_with_oracle();
    write(&e.project, "src/lib.spx", &broken);
    let fake = Fake::new(CHANGED);
    let repair_task = |t: &mut Task| {
        t.mode = TaskMode::Repair;
        t.goal = "make the project compile".into();
    };
    // A bad patch (law removal, oracle edit, ambiguous find) fails; the original is never written.
    let bad = [
        patch("src/lib.spx", "    ensures result == x\n", ""),
        patch("src/tests.spx", "0", "1"),
        patch("src/lib.spx", "i64", "i32"),
        patch("../outside.spx", "a", "b"),
        patch(
            "src/lib.spx",
            "uses { clock.read }",
            "uses { clock.read, fs.write }",
        ),
    ];
    let seq = Seq::new(bad.to_vec());
    let r = once(
        &session(&e, |t| {
            repair_task(t);
            t.session.as_mut().unwrap().max_attempts = 5;
        }),
        &fake,
        &seq,
    );
    assert_eq!(r.status, "exhausted", "{:?}", r.refusals);
    let codes_seen: Vec<_> = r.session["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["code"].as_str().unwrap())
        .collect();
    assert_eq!(
        codes_seen,
        [
            "SPX-HPD042",
            "SPX-HPD114",
            "SPX-HPD117",
            "SPX-HPD117",
            "SPX-HPD043"
        ]
    );
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        broken
    );
    // A valid patch is compiled in scratch; the compiler's exact diagnostic reached the model first.
    let seq = Seq::new(vec![
        patch("src/lib.spx", "    SYNTAXERR\n", "    SYNTAXERR2\n"),
        patch("src/lib.spx", "    SYNTAXERR2\n", "    x\n"),
    ]);
    let cfg = session(&e, repair_task);
    let r = once(&cfg, &fake, &seq);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert!(seq.prompts.borrow()[1]["feedback"]
        .to_string()
        .contains("unexpected token `SYNTAXERR`"));
    assert_eq!(seq.prompts.borrow()[0]["scratch_repair"], true);
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        broken,
        "the project is untouched"
    );
    let dir = PathBuf::from(r.session["result"]["dir"].as_str().unwrap());
    let rev = r.session["result"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        std::fs::read_to_string(dir.join("src/lib.spx")).unwrap(),
        FIXED
    );
    // Source drift rejects final application; the untouched project accepts it.
    write(&e.project, "src/lib.spx", &format!("{broken}// drift\n"));
    let snap = cfg.snapshot.clone();
    let err = apply_result(&snap, &dir, &rev, &fake).unwrap_err();
    assert_eq!(err.code, "SPX-HPD115");
    write(&e.project, "src/lib.spx", &broken);
    let applied = apply_result(&snap, &dir, &rev, &fake).unwrap();
    assert_eq!(applied, ["src/lib.spx"]);
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        FIXED
    );
}

#[path = "workflow_feedback.rs"]
mod feedback;
