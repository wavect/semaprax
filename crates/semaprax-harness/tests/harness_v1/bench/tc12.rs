//! TC-12: profile-arm campaign, offline. A deterministic fake model adapter
//! (`FakeStage`) answers through the production `HostModel` request/receipt
//! contract; nothing here is paid or live, so every record is origin `fixture`
//! until a test relabels rows to exercise the promotion logic.

use super::{corpus_dir, python};
use crate::support::fixture_dir;
use semaprax_harness::bench::apptask::arms::{self, ArmSet};
use semaprax_harness::bench::apptask::cache_state::{self, CacheState, CacheTracker};
use semaprax_harness::bench::apptask::campaign::{self, Selection};
use semaprax_harness::bench::apptask::model::{Generation, ModelError, Scripted, SpendLedger};
use semaprax_harness::bench::apptask::production::{
    Admission, ProductionClient, RawClient, TrialClient, PATH_PRODUCTION, PATH_RAW,
};
use semaprax_harness::bench::apptask::profile_arms::{
    self, CampaignSpec, Criterion, Pins, Policy, ProfileArm,
};
use semaprax_harness::bench::apptask::profile_campaign::{self, spend_of, ArmBackend};
use semaprax_harness::bench::apptask::profile_qualify::{content_free, metrics, qualify};
use semaprax_harness::bench::apptask::task::{self, TaskSet, Tools};
use semaprax_harness::bench::apptask::tokens::WordCounter;
use semaprax_harness::bench::apptask::trial::{ModelSpec, TrialEnv, TrialKey};
use semaprax_harness::contract::{
    validate_payload, CapabilityKind, CapabilityRef, Direction, ProjectBinding, RequestEnvelope,
    ResultEnvelope,
};
use semaprax_harness::host::Outcome;
use semaprax_harness::receipt::{
    GenerationControls, PriceBook, PriceRecord, Pricing, ProposalReceipt, Support,
};
use semaprax_harness::workflow::budget::BudgetConfig;
use semaprax_harness::workflow::lineage::Lineage;
use semaprax_harness::workflow::prompt_render::PromptRenderer;
use semaprax_harness::workflow::stages::{
    HostModel, ProposalRequest, ProposalStage, StageFailure, Task,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const TASKS: [&str; 2] = ["feature-py-shop", "failing-tests-py-ledger"];

fn tools() -> Tools {
    Tools::default().with("HARNESS_PYTHON", &python())
}

fn work(tag: &str) -> PathBuf {
    fixture_dir(&format!("hp-tc12-{tag}"))
        .canonicalize()
        .unwrap()
}

fn reference_answer(t: &task::Task, step: usize) -> String {
    t.reference_files_of(step)
        .into_iter()
        .map(|(p, c)| {
            format!(
                "=== FILE: {p} ===\n{}=== END FILE ===\n",
                String::from_utf8_lossy(&c)
            )
        })
        .collect()
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

/// Per-arm behaviour of the fake adapter.
struct Behaviour {
    uncached: u64,
    output: u64,
    /// (cache_read, cache_write) of the first and of later calls; `None`: no cache categories reported.
    cache: Option<((u64, u64), (u64, u64))>,
    correct: bool,
}

fn behaviour(arm: &str) -> Behaviour {
    match arm {
        "compact-skills" => Behaviour {
            uncached: 600,
            output: 300,
            cache: Some(((0, 0), (0, 0))),
            correct: true,
        },
        // Cheap and inferior: far fewer tokens, wrong answers.
        "context-target" => Behaviour {
            uncached: 200,
            output: 40,
            cache: Some(((0, 0), (0, 0))),
            correct: false,
        },
        "prompt-renderer" => Behaviour {
            uncached: 300,
            output: 300,
            cache: Some(((0, 700), (700, 0))),
            correct: true,
        },
        _ => Behaviour {
            uncached: 1000,
            output: 300,
            cache: Some(((0, 0), (0, 0))),
            correct: true,
        },
    }
}

struct FakeStage {
    arm: String,
    answers: Vec<(String, String)>,
    calls: u32,
    served: Arc<AtomicU32>,
    wire: Arc<Mutex<Vec<Value>>>,
    support: Support,
}

impl ProposalStage for FakeStage {
    fn id(&self) -> String {
        "org.example/fake-model".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.propose_receipted(r).0
    }
    fn propose_receipted(
        &mut self,
        r: &ProposalRequest,
    ) -> (Result<Vec<u8>, StageFailure>, ProposalReceipt) {
        self.calls += 1;
        let payload = HostModel::request_payload_cached(r, &self.id(), self.support);
        validate_payload(
            CapabilityKind::ModelGenerate,
            "generate",
            Direction::Request,
            &payload,
        )
        .expect("valid model.generate/v1 request");
        self.wire.lock().unwrap().push(payload.clone());
        let text = r.prompt["text"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| semaprax_harness::workflow::budget::request_text(&r.prompt));
        let b = behaviour(&self.arm);
        let answer = if b.correct {
            self.answers
                .iter()
                .find(|(q, _)| text.contains(q.as_str()))
                .map(|(_, a)| a.clone())
                .unwrap_or_default()
        } else {
            "I could not work this out.".to_string()
        };
        let n = self.served.fetch_add(1, Ordering::SeqCst);
        let (rd, wr) = b
            .cache
            .map(|(f, l)| if n == 0 { f } else { l })
            .unwrap_or((0, 0));
        let mut usage = json!({"input_tokens": b.uncached, "output_tokens": b.output});
        if b.cache.is_some() {
            usage["cache_read_input_tokens"] = json!(rd);
            usage["cache_creation_input_tokens"] = json!(wr);
        }
        let receipt = json!({"schema": "semaprax.harness-model-receipt.v1", "protocol": "anthropic_messages",
            "request_id": format!("r{n}"), "model": "fake-model-1", "finish_reason": "end_turn", "usage": usage,
            "controls": {"max_output_tokens": {"status": "applied", "effective": r.controls.max_output_tokens}}});
        let body = json!({"model": payload["model"], "output_base64": b64(answer.as_bytes()),
                          "usage": {"input_bytes": 1, "output_bytes": answer.len()}, "receipt": receipt});
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
            payload,
        };
        let env = ResultEnvelope::complete(&req, body, "org.example/fake-model", "0.1.0");
        let outcome = Outcome::Completed(
            ResultEnvelope::parse_for(&req, env.to_json().to_string().as_bytes())
                .expect("valid result frame"),
        );
        HostModel::interpret(outcome, r)
    }
    fn calls(&self) -> u32 {
        self.calls
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

fn prices() -> PriceBook {
    PriceBook::default().with(
        "fake-model",
        PriceRecord {
            version: "t1".into(),
            pricing: Pricing::Rates {
                input: Some(1_000_000),
                cache_read: Some(100_000),
                cache_write: Some(1_250_000),
                cache_write_1h: None,
                output: Some(5_000_000),
            },
        },
    )
}

struct Fake<'t> {
    set: &'t TaskSet,
    prices: PriceBook,
    served: Arc<AtomicU32>,
    ordered_served: Arc<AtomicU32>,
    wire: Arc<Mutex<Vec<Value>>>,
    missing_model: bool,
}

impl ArmBackend for Fake<'_> {
    fn origin(&self, _m: &ModelSpec) -> &'static str {
        "fixture"
    }
    fn client<'a>(
        &'a self,
        arm: &ProfileArm,
        model: &ModelSpec,
        tracker: &'a CacheTracker,
        key: &TrialKey,
    ) -> Result<Box<dyn TrialClient + 'a>, String> {
        if self.missing_model {
            return Err(format!("model `{}` is not available", model.id));
        }
        let t = self.set.task(&key.task).unwrap();
        let answers = t
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.request.clone(), reference_answer(t, i)))
            .collect();
        let ordered = arm.has(Policy::PromptRenderer);
        let stage = FakeStage {
            arm: arm.id.clone(),
            answers,
            calls: 0,
            served: if ordered {
                self.ordered_served.clone()
            } else {
                self.served.clone()
            },
            wire: self.wire.clone(),
            support: if ordered {
                Support::Supported
            } else {
                Support::Unknown
            },
        };
        let lineage = Lineage::new(
            ProjectBinding {
                id: "apptask".into(),
                worktree: "w".into(),
                revision: key.id(),
            },
            "sha256:lock",
            &key.task,
        );
        let c = ProductionClient::new(
            Box::new(stage),
            lineage,
            "fake-model-1",
            GenerationControls {
                max_output_tokens: Some(4096),
                ..Default::default()
            },
            Box::new(|m, t| {
                BudgetConfig::default()
                    .for_task(&Task::default())
                    .count(m, t)
            }),
            &self.prices,
            Admission {
                max_request_tokens: 1 << 20,
                protocol_overhead_tokens: 256,
            },
            tracker,
        )
        .with_renderer(
            if ordered {
                PromptRenderer::OrderedV1
            } else {
                PromptRenderer::Canonical
            },
            if ordered {
                Support::Supported
            } else {
                Support::Unknown
            },
        );
        Ok(Box::new(c))
    }
}

fn model() -> ModelSpec {
    ModelSpec {
        id: "fake-model-1".into(),
        size: "large".into(),
        billed: true,
    }
}

fn roster(ids: &[&str]) -> Vec<ProfileArm> {
    profile_arms::screening_roster("native")
        .into_iter()
        .filter(|a| a.id == "defaults" || ids.contains(&a.id.as_str()))
        .collect()
}

fn spec(arms: Vec<ProfileArm>, reps: u32, set: &TaskSet) -> CampaignSpec {
    let mut c = Criterion::predeclared();
    c.gate.min_items = TASKS.len() * reps as usize;
    CampaignSpec {
        id: "tc12-test".into(),
        pins: Pins {
            model: "fake-model-1".into(),
            tools: "sha256:tools".into(),
            taskset: set.digest.clone(),
        },
        arms,
        tasks: TASKS.iter().map(|s| s.to_string()).collect(),
        reps,
        criterion: c,
        max_usd: 1.0,
        max_calls: 1000,
    }
}

fn run_campaign(
    spec: &CampaignSpec,
    backend: &dyn ArmBackend,
    ledger: &SpendLedger,
    out: &Path,
    w: &Path,
) -> Vec<Value> {
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    let arm_set = ArmSet::load(&corpus_dir().join("apptasks")).unwrap();
    let t = tools();
    let sel = Selection {
        tasks: vec![],
        arms: vec![],
        reps: spec.reps,
    };
    let packs = campaign::build_packs(&set, &arm_set, &sel, &t, w);
    let blocks = arms::skill_blocks(&arm_set, &w.join("skillhome"));
    let env = TrialEnv {
        tasks: &set,
        tools: &t,
        work: w,
        counter: &WordCounter,
        packs: &packs,
        arm_set: &arm_set,
        skills: &blocks,
    };
    std::fs::create_dir_all(out).unwrap();
    profile_arms::declare(out, spec).unwrap();
    profile_campaign::run(&env, spec, &[model()], backend, ledger, out).unwrap();
    std::fs::read_to_string(out.join("trials.jsonl"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn fake(set: &TaskSet) -> Fake<'_> {
    Fake {
        set,
        prices: prices(),
        served: Arc::new(AtomicU32::new(0)),
        ordered_served: Arc::new(AtomicU32::new(0)),
        wire: Arc::default(),
        missing_model: false,
    }
}

fn rows_of<'a>(rows: &'a [Value], arm: &str) -> Vec<&'a Value> {
    rows.iter().filter(|r| r["arm"] == arm).collect()
}

fn as_real(rows: &[Value]) -> Vec<Value> {
    rows.iter()
        .map(|r| {
            let mut r = r.clone();
            if r["outcome"] != "unavailable" {
                r["origin"] = json!("real");
            }
            r
        })
        .collect()
}

#[test]
fn tc12_graders_fail_pristine_pass_reference_and_the_optimized_arm_cannot_touch_the_oracle() {
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    let t = tools();
    let w = work("oracle");
    let rows = semaprax_harness::bench::apptask::validate_tasks(
        &TaskSet {
            root: set.root.clone(),
            tasks: set
                .tasks
                .iter()
                .filter(|x| TASKS.contains(&x.id.as_str()))
                .cloned()
                .collect(),
            digest: set.digest.clone(),
        },
        &t,
        &w,
    );
    assert!(!rows.is_empty());
    for r in &rows {
        assert_eq!(
            (r["pristine_fails"].clone(), r["reference_passes"].clone()),
            (json!(true), json!(true)),
            "{r}"
        );
    }
    // An arm that answers by overwriting the protected test file is refused and not accepted.
    let shop = set.task("feature-py-shop").unwrap();
    let tampered = Scripted(|_p: &str, _s: u64| {
        Ok::<_, ModelError>(Generation {
            text: "=== FILE: tests/test_shop.py ===\ndef test_ok():\n    pass\n=== END FILE ===\n"
                .into(),
            provider_in: Some(1),
            provider_out: Some(1),
            cost_usd: Some(0.0),
            ..Generation::default()
        })
    });
    let tracker = CacheTracker::default();
    let raw = RawClient::new(&tampered, &tracker, "m", true);
    let one = TaskSet {
        root: set.root.clone(),
        tasks: vec![shop.clone()],
        digest: set.digest.clone(),
    };
    let arm_set = ArmSet::load(&corpus_dir().join("apptasks")).unwrap();
    let blocks = arms::skill_blocks(&arm_set, &w.join("skillhome"));
    let packs = Default::default();
    let env = TrialEnv {
        tasks: &one,
        tools: &t,
        work: &w,
        counter: &WordCounter,
        packs: &packs,
        arm_set: &arm_set,
        skills: &blocks,
    };
    let key = TrialKey {
        task: shop.id.clone(),
        arm: "defaults".into(),
        model: "m".into(),
        rep: 0,
    };
    let rec = semaprax_harness::bench::apptask::trial::run_trial(
        &env,
        &key,
        arm_set.arm("native").unwrap(),
        &model(),
        &raw,
    );
    assert_eq!(rec["passed"], false);
    assert!(rec["tamper_attempts"].as_u64().unwrap() >= 1, "{rec}");
}

#[test]
fn tc12_production_path_campaign_uses_adapter_contract_budget_observations_and_receipts() {
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    let w = work("prod");
    let out = w.join("out");
    let s = spec(
        roster(&["prompt-renderer", "spend-ledger", "cost-aware-routing"]),
        2,
        &set,
    );
    let b = fake(&set);
    let ledger = SpendLedger::new(1.0, 1000, 0.02);
    let rows = run_campaign(&s, &b, &ledger, &out, &w);
    let d = rows_of(&rows, "defaults");
    assert!(
        !d.is_empty()
            && d.iter()
                .all(|r| r["path"] == PATH_PRODUCTION && r["accepted"] == true),
        "{d:?}"
    );
    assert!(
        d.iter().all(|r| r["observations"].as_u64().unwrap() >= 1),
        "Observer events recorded"
    );
    let a0 = &d[0]["attempts"][0];
    assert_eq!(
        a0["usage"]["uncached_input"], 1000,
        "typed receipt usage, not local counts"
    );
    assert_eq!(a0["model_pin"], "fake-model-1");
    // Cost: 1000 input + 300 output priced from the price book (1 and 5 micro-units per token).
    assert_eq!(
        (a0["cost_micros"].clone(), a0["cost_basis"].clone()),
        (json!(2500), json!("estimate"))
    );
    // Ordered renderer: segments and prefix identity reached the wire; later calls are warm.
    let wire = b.wire.lock().unwrap();
    let seg = wire
        .iter()
        .find_map(|p| p.get("segments"))
        .expect("segments sent for the ordered arm");
    assert_eq!(seg["cache_boundary_after"], "task");
    drop(wire);
    let pr = rows_of(&rows, "prompt-renderer");
    assert!(pr
        .iter()
        .all(|r| r["accepted"] == true && r["attempts"][0]["prefix_identity"].is_string()));
    // Cache writes stay in the totals: the first call paid 700 tokens at the write rate.
    let states: Vec<&str> = rows
        .iter()
        .flat_map(|r| {
            r["attempts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a["cache_state"].as_str().unwrap())
        })
        .collect();
    assert!(
        states.contains(&"cold") && states.contains(&"warm"),
        "{states:?}"
    );
    for r in &rows {
        let t: u64 = r["attempts"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|a| a["cost_micros"].as_u64())
            .sum();
        if r["spend"]["dispatched"].as_u64().unwrap() > 0 {
            assert_eq!(
                r["spend"]["micros"].as_u64(),
                Some(t),
                "trial spend equals the sum of its attempts"
            );
        }
    }
    let write_attempt = rows
        .iter()
        .flat_map(|r| r["attempts"].as_array().unwrap().iter())
        .find(|a| a["usage"]["cache_write"] == 700)
        .unwrap();
    assert_eq!(
        write_attempt["cost_micros"],
        300 + 700 * 1_250_000 / 1_000_000 + 1500
    );
    // TC-03 and TC-10 landed: both are real arms that run, not placeholders.
    for id in ["spend-ledger", "cost-aware-routing"] {
        let u = rows_of(&rows, id);
        assert_eq!(u.len(), TASKS.len() * 2);
        assert!(u.iter().all(|r| r["outcome"] != "unavailable"), "{id}");
    }
    // Repo/index cache is labelled separately from the provider cache.
    assert!(
        d.iter().any(|r| r["cache"]["repo"] == "cold")
            && d.iter().any(|r| r["cache"]["repo"] == "warm")
    );
    // Records carry no task or answer text.
    let forbidden: Vec<String> = TASKS
        .iter()
        .flat_map(|t| set.task(t).unwrap().steps.iter().map(|s| s.request.clone()))
        .collect();
    assert!(rows.iter().all(|r| content_free(r, &forbidden)));
}

#[test]
fn tc12_cheap_inferior_arm_fails_qualification_despite_fewer_tokens_and_good_arm_is_reproducible_and_drift_invalidates(
) {
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    let w = work("qual");
    let out = w.join("out");
    let s = spec(roster(&["compact-skills", "context-target"]), 2, &set);
    let rows = run_campaign(
        &s,
        &fake(&set),
        &SpendLedger::new(1.0, 1000, 0.02),
        &out,
        &w,
    );
    let cheap = rows_of(&rows, "context-target");
    assert!(cheap
        .iter()
        .all(|r| r["accepted"] == false && r["outcome"] == "failed"));
    // Failed and recovery attempts stay in the cost totals.
    assert!(cheap.iter().all(|r| r["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["role"] == "recovery")));
    let (mc, md) = (metrics(&cheap), metrics(&rows_of(&rows, "defaults")));
    assert!(mc["total_spend_micros"].as_u64().unwrap() > 0);
    assert!(
        mc["spend_per_accepted_micros"].is_null(),
        "no accepted task: undefined, not zero"
    );
    let tok = |r: &[&Value]| -> u64 {
        r.iter()
            .flat_map(|x| x["attempts"].as_array().unwrap().iter())
            .filter_map(|a| a["usage"]["uncached_input"].as_u64())
            .sum()
    };
    assert!(
        tok(&cheap) < tok(&rows_of(&rows, "defaults")),
        "the cheap arm does emit fewer tokens"
    );
    assert!(md["spend_per_accepted_micros"].as_f64().unwrap() > 0.0);

    // Fixture evidence never promotes.
    let (declared, crit) = profile_arms::load(&out).unwrap();
    let live = s.pins.clone();
    let q = qualify(&declared, &crit, &rows, &live, &[]);
    assert!(q["arms"]
        .as_array()
        .unwrap()
        .iter()
        .all(|a| a["decision"] == "inconclusive"));
    assert_eq!(q["recommendation"]["action"], "leave-defaults-unchanged");

    // Relabelled as real production evidence, the gates decide.
    let real = as_real(&rows);
    let q = qualify(&declared, &crit, &real, &live, &[]);
    let by = |id: &str| {
        q["arms"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["arm"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(
        by("context-target")["decision"],
        "no-go",
        "{}",
        by("context-target")
    );
    assert_eq!(
        by("compact-skills")["decision"],
        "go",
        "{}",
        by("compact-skills")
    );
    assert_eq!(q["recommendation"]["profile"], "compact-skills");
    // Reproducible from the pinned evidence.
    let q2 = qualify(&declared, &crit, &real, &live, &[]);
    assert_eq!(q, q2);
    assert_eq!(
        by("compact-skills")["record_digest"],
        q2["arms"][0]["record_digest"]
    );
    // Drift in the model, tool or task-set pin invalidates the promotion.
    for drifted in [
        Pins {
            model: "fake-model-2".into(),
            ..live.clone()
        },
        Pins {
            tools: "sha256:other".into(),
            ..live.clone()
        },
        Pins {
            taskset: "sha256:other".into(),
            ..live.clone()
        },
    ] {
        let q = qualify(&declared, &crit, &real, &drifted, &[]);
        let c = q["arms"]
            .as_array()
            .unwrap()
            .iter()
            .find(|a| a["arm"] == "compact-skills")
            .unwrap();
        assert_eq!(c["decision"], "invalidated", "{c}");
        assert_eq!(q["recommendation"]["action"], "leave-defaults-unchanged");
    }
    // A declared criterion cannot be changed after results exist.
    let mut other = s.clone();
    other.criterion.gate.min_cost_saving = 0.0;
    assert!(profile_arms::declare(&out, &other).is_err());
    let mut t: Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("campaign.json")).unwrap()).unwrap();
    t["criterion"]["min_cost_saving"] = json!(0.0);
    std::fs::write(out.join("campaign.json"), t.to_string()).unwrap();
    assert!(
        profile_arms::load(&out).is_err(),
        "edited criterion no longer matches its digest"
    );
}

#[test]
fn tc12_missing_model_and_budget_abort_are_retained_and_raw_loop_is_labelled_and_never_promotes() {
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    // Missing model: every trial retained as unavailable.
    let w = work("missing");
    let s = spec(roster(&[]), 1, &set);
    let mut b = fake(&set);
    b.missing_model = true;
    let rows = run_campaign(
        &s,
        &b,
        &SpendLedger::new(1.0, 100, 0.02),
        &w.join("out"),
        &w,
    );
    assert!(
        !rows.is_empty()
            && rows.iter().all(|r| r["outcome"] == "unavailable"
                && r["accepted"] == false
                && r["spend"]["micros"].is_null())
    );

    // Raw loop with a cap below one call's ceiling: aborted, retained, never zero cost or success.
    struct Raw;
    impl ArmBackend for Raw {
        fn origin(&self, _m: &ModelSpec) -> &'static str {
            "real"
        }
        fn client<'a>(
            &'a self,
            _a: &ProfileArm,
            m: &ModelSpec,
            t: &'a CacheTracker,
            _k: &TrialKey,
        ) -> Result<Box<dyn TrialClient + 'a>, String> {
            static C: Scripted<fn(&str, u64) -> Result<Generation, ModelError>> =
                Scripted(|_p, _s| Ok(Generation::default()));
            Ok(Box::new(RawClient::new(&C, t, &m.id, m.billed)))
        }
    }
    let w = work("abort");
    let s = spec(roster(&["compact-skills"]), 1, &set);
    let rows = run_campaign(
        &s,
        &Raw,
        &SpendLedger::new(0.001, 100, 0.02),
        &w.join("out"),
        &w,
    );
    assert_eq!(rows.len(), TASKS.len() * s.arms.len());
    assert!(rows
        .iter()
        .all(|r| r["outcome"] == "budget_aborted" && r["accepted"] == false));
    assert!(
        rows.iter().any(|r| r["status"] == "budget")
            && rows.iter().any(|r| r["status"] == "not_run")
    );
    let m = metrics(&rows.iter().collect::<Vec<_>>());
    assert!(m["total_spend_micros"].is_null() && m["spend_per_accepted_micros"].is_null());
    // Raw-loop trials are labelled as such and a raw-only campaign cannot promote.
    let w = work("raw");
    let ok = Scripted(|_p: &str, _s: u64| {
        Ok::<_, ModelError>(Generation {
            text: "x".into(),
            provider_in: Some(5),
            provider_out: Some(5),
            cost_usd: Some(0.001),
            ..Generation::default()
        })
    });
    let tr = CacheTracker::default();
    let rc = RawClient::new(&ok, &tr, "m", true);
    assert_eq!(rc.path(), PATH_RAW);
    let _ = w;
    let mut real = as_real(&rows);
    for r in &mut real {
        r["path"] = json!(PATH_RAW);
    }
    let (declared, crit) = (
        json!({"pins": {"model":"","tools":"","taskset":""}, "arms": []}),
        Criterion::predeclared(),
    );
    let q = qualify(
        &declared,
        &crit,
        &real,
        &Pins {
            model: "".into(),
            tools: "".into(),
            taskset: "".into(),
        },
        &[],
    );
    assert_eq!(q["recommendation"]["action"], "leave-defaults-unchanged");
}

#[test]
fn tc12_cache_state_is_receipt_backed_or_unknown_and_never_leaks_answers() {
    use semaprax_harness::receipt::Usage;
    let t = CacheTracker::default();
    let u = |r: Option<u64>, w: Option<u64>| Usage {
        cache_read: r,
        cache_write: w,
        ..Usage::default()
    };
    assert_eq!(t.classify("m", &u(Some(0), Some(0))), CacheState::Cold);
    assert_eq!(t.classify("m", &u(Some(0), Some(500))), CacheState::Cold);
    assert_eq!(t.classify("m", &u(Some(500), Some(0))), CacheState::Warm);
    assert_eq!(
        t.classify("m", &u(Some(0), Some(0))),
        CacheState::Expired,
        "a cache existed and is no longer served"
    );
    assert_eq!(
        t.classify("m", &u(None, None)),
        CacheState::Unknown,
        "categories not reported"
    );
    assert_eq!(t.classify("other", &u(Some(0), None)), CacheState::Unknown);
    assert_eq!(
        cache_state::trial_label(&[CacheState::Cold, CacheState::Warm]),
        "mixed"
    );
    // A raw client whose provider reports no cache categories is labelled unknown.
    let m = Scripted(|_p: &str, _s: u64| {
        Ok::<_, ModelError>(Generation {
            text: "x".into(),
            provider_in: Some(5),
            provider_out: Some(5),
            ..Generation::default()
        })
    });
    let tr = CacheTracker::default();
    let rc = RawClient::new(&m, &tr, "m", true);
    semaprax_harness::bench::apptask::model::ModelClient::generate(&rc, "p", 1).unwrap();
    let a = rc.take_attempts();
    assert_eq!(
        (a[0].cache, a[0].cost_micros),
        (CacheState::Unknown, None),
        "unknown cost stays unknown"
    );
    assert!(spend_of(&a).micros.is_none() && !spend_of(&a).complete);
    // Warm-up carries only the shared prefix, never the task request or reference answers.
    let set = TaskSet::load(&corpus_dir().join("apptasks")).unwrap();
    let shop = set.task("feature-py-shop").unwrap();
    let prompt = arms::build_prompt("", "context", &shop.steps[0].request, None);
    let prefix = cache_state::warm_prefix(&prompt);
    assert!(!prefix.contains(&shop.steps[0].request));
    let reference: Vec<(String, String)> = shop
        .reference_files_of(0)
        .into_iter()
        .map(|(p, c)| (p, String::from_utf8_lossy(&c).into_owned()))
        .collect();
    assert!(!cache_state::leaks_reference(
        prefix,
        &reference,
        &shop.project
    ));
    let leaky = format!("{prefix}\n{}", reference[0].1);
    assert!(cache_state::leaks_reference(
        &leaky,
        &reference,
        &shop.project
    ));
}

#[test]
fn tc12_roster_is_bounded_with_placeholders_and_a_combined_profile() {
    let r = profile_arms::screening_roster("native");
    let ids: Vec<&str> = r.iter().map(|a| a.id.as_str()).collect();
    assert_eq!(
        ids.len(),
        10,
        "defaults + 8 single policies + combined, no sweep: {ids:?}"
    );
    let get = |id: &str| r.iter().find(|a| a.id == id).unwrap();
    assert!(
        get("prompt-renderer").unavailable.is_none(),
        "TC-04 landed: a real arm"
    );
    assert_eq!(
        get("prompt-renderer").overlay()["budget.prompt_renderer"],
        "ordered-v1"
    );
    assert!(
        get("spend-ledger").unavailable.is_none() && get("cost-aware-routing").unavailable.is_none()
    );
    assert_eq!(get("spend-ledger").overlay()["budget.strict_monetary"], true);
    assert_eq!(get("cost-aware-routing").overlay()["routing.cost_aware"], true);
    let c = get("combined");
    assert!(
        c.has(Policy::PromptRenderer) && c.has(Policy::CompactSkills) && c.has(Policy::Routing)
    );
    assert!(c.omitted.is_empty());
    assert!(get("defaults").overlay().is_empty());
}

#[test]
fn tc12_paid_command_requires_an_explicit_cap_and_dry_run_spends_and_runs_nothing() {
    use semaprax_harness::cli::{run, Environment};
    let env = Environment {
        harness_home: None,
        compiler: None,
        cwd: work("cli"),
        vars: Default::default(),
    };
    let tasks = corpus_dir().join("apptasks").display().to_string();
    let go = |extra: &[&str]| {
        let mut a: Vec<String> = [
            "bench",
            "app",
            "run",
            &tasks,
            "--out",
            "o",
            "--work",
            "w",
            "--profile-arms",
            "all",
            "--model",
            "id=m,name=n,addr=127.0.0.1:1,billed=1",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        a.extend(extra.iter().map(|s| s.to_string()));
        run(&a, &env)
    };
    let o = go(&["--dry-run"]);
    assert_ne!(o.code, 0, "no --max-usd: refused");
    let o = go(&["--dry-run", "--max-usd", "5"]);
    assert_eq!(o.code, 0, "{}", o.stdout);
    assert!(o.stdout.contains("nothing was run"), "{}", o.stdout);
    assert!(!env.cwd.join("o/trials.jsonl").exists() && !env.cwd.join("o/campaign.json").exists());
}
