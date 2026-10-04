use super::*;
use semaprax_harness::observe::{Observer, ObserverLimits};
use semaprax_harness::workflow::compiler::SubprocessCompiler;
use semaprax_harness::workflow::stages::{ProposalRequest, ProposalStage, StageFailure, TaskMode};
use semaprax_harness::workflow::{
    apply_result, Composition, Report, RunConfig, SessionBounds, Snapshot,
};
use std::cell::{Cell, RefCell};

struct Script {
    items: Vec<Value>,
    prompts: RefCell<Vec<Value>>,
    calls: Cell<u32>,
    side: bool,
    uncertain: bool,
}
impl Script {
    fn new(items: Vec<Value>) -> Self {
        Script {
            items,
            prompts: RefCell::default(),
            calls: Cell::new(0),
            side: false,
            uncertain: false,
        }
    }
}
struct Ref<'a>(&'a Script);
impl ProposalStage for Ref<'_> {
    fn id(&self) -> String {
        "org.example/script".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        let i = self.0.calls.get() as usize;
        self.0.calls.set(i as u32 + 1);
        self.0.prompts.borrow_mut().push(r.prompt.clone());
        if self.0.uncertain {
            return Err(StageFailure::Uncertain(
                semaprax_harness::diag::HarnessDiagnostic::new(
                    "SPX-HPD072",
                    "link lost after send",
                ),
            ));
        }
        Ok(self.0.items[i.min(self.0.items.len() - 1)]
            .to_string()
            .into_bytes())
    }
    fn calls(&self) -> u32 {
        self.0.calls.get()
    }
    fn side_effecting(&self) -> bool {
        self.0.side
    }
}

struct Rig {
    project: PathBuf,
    cache: PathBuf,
    compiler: SubprocessCompiler,
}

fn rig(tag: &str, patch_lib: Option<(&str, &str)>) -> Rig {
    let root = fixture_dir(tag).canonicalize().unwrap();
    let project = root.join("project");
    copy_dir(&fixtures().join("healthy"), &project);
    if let Some((from, to)) = patch_lib {
        let p = project.join("src/lib.spx");
        let s = std::fs::read_to_string(&p).unwrap().replace(from, to);
        std::fs::write(p, s).unwrap();
    }
    let exe = required_tool("SEMAPRAX_COMPILER");
    let compiler = SubprocessCompiler::new(exe, root.join("compiler")).unwrap();
    Rig {
        project,
        cache: root.join("cache"),
        compiler,
    }
}

fn cfg(r: &Rig, task: Task) -> RunConfig {
    RunConfig {
        snapshot: Snapshot::capture(&r.project).unwrap(),
        task,
        context_max_bytes: 16384,
        cache_dir: r.cache.clone(),
        lock_digest: "sha256:lock".into(),
        providers: vec![],
        composition: Composition::from_profile(None, true, vec![], &[]).unwrap(),
        apply_policy: None,
        checks: vec![],
        skill_prompt: None,
        endpoint_policy: Default::default(),
        model_plans: None,
        notes: vec![],
        budget: Default::default(),
        cancel: None,
        routing: Default::default(),
    }
}

fn go(r: &Rig, cfg: &RunConfig, s: &Script) -> Report {
    let mut native = semaprax_harness::workflow::stages::NativeContext::new(&r.compiler);
    let mut p = Ref(s);
    let mut view = semaprax_harness::workflow::stages::RawCommandView;
    semaprax_harness::workflow::run(
        cfg,
        &r.compiler,
        semaprax_harness::workflow::Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

fn change(extra: impl FnOnce(&mut Task)) -> Task {
    let mut t = Task {
        schema_version: 2,
        mode: TaskMode::Change,
        goal: "rename line_total to line_cost".into(),
        seed: Some("ledger.line_total".into()),
        ..Task::default()
    };
    extra(&mut t);
    t
}

fn intent(i: Value) -> Value {
    json!({"schema": "semaprax.harness-proposal.v1", "intent": i})
}
fn rename() -> Value {
    intent(
        json!({"kind": "rename_declaration", "target": "ledger.line_total", "name": "line_cost"}),
    )
}
fn add_discount() -> Value {
    intent(
        json!({"kind": "change_function_signature", "target": "ledger.invoice_total",
            "append_parameters": [{"name": "discount", "type": "i64", "argument": {"kind": "i64", "value": 0}}]}),
    )
}
fn patch(find: &str, replace: &str) -> Value {
    json!({"schema": "semaprax.harness-proposal.v1", "source_patch": {"edits": [{"path": "src/lib.spx", "find": find, "replace": replace}]}})
}
fn read(r: &Rig, rel: &str) -> String {
    std::fs::read_to_string(r.project.join(rel)).unwrap()
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn01_real_healthy_multifile_rename_reaches_validated_candidate_and_noop_stays_noop() {
    let r = rig("hp-hn01r", None);
    let before = [read(&r, "src/lib.spx"), read(&r, "src/tests.spx")];
    let accept = json!({"stable_id": "ledger.line_total", "contains": "line_cost"});
    let s = Script::new(vec![rename()]);
    let rep = go(
        &r,
        &cfg(&r, change(|t| t.acceptance = vec![accept.clone()])),
        &s,
    );
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    // Installed operations were discovered from the compiler, not hard-coded.
    let kinds = rep.operations["kinds"].as_array().unwrap();
    assert!(kinds.iter().any(|k| k == "rename_declaration"), "{kinds:?}");
    assert!(!rep.candidate["changed_files"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        rep.checks["tests"], "passed",
        "baseline tests pass on the candidate"
    );
    assert_eq!(
        rep.checks["acceptance_verified"], 1,
        "the requested change is present"
    );
    assert!(r
        .cache
        .join(format!("{}.capsule.json", rep.lineage))
        .is_file());
    assert_eq!(
        s.prompts.borrow()[0]["goal"],
        "rename line_total to line_cost"
    );
    assert_eq!(
        [read(&r, "src/lib.spx"), read(&r, "src/tests.spx")],
        before,
        "the project is untouched"
    );
    // No task: the old no-op, no model call.
    let s = Script::new(vec![rename()]);
    let rep = go(&r, &cfg(&r, Task::default()), &s);
    assert_eq!((rep.status, s.calls.get()), ("no-repair-needed", 0));
    // Unsupported operation: precise, no model call; read-only plan writes no capsule.
    let s = Script::new(vec![rename()]);
    let rep = go(
        &r,
        &cfg(
            &r,
            change(|t| t.operation = Some("add_http_endpoint".into())),
        ),
        &s,
    );
    assert_eq!((rep.status, s.calls.get()), ("unsupported-goal", 0));
    let _ = std::fs::remove_dir_all(&r.cache);
    let rep = go(
        &r,
        &cfg(&r, change(|t| t.mode = TaskMode::Plan)),
        &Script::new(vec![rename()]),
    );
    assert_eq!(rep.status, "planned", "{:?}", rep.refusals);
    assert!(!r
        .cache
        .join(format!("{}.capsule.json", rep.lineage))
        .exists());
    assert!(rep.checks.is_null() && rep.approval.is_null());
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_attempt_one_fails_the_oracle_its_diagnostic_reaches_attempt_two() {
    let r = rig("hp-hn02a", None);
    let body = |l: &str, rr: &str| {
        intent(
            json!({"kind": "replace_function_body", "target": "ledger.line_total",
            "body": {"kind": "binary", "op": "*", "left": {"kind": "place", "name": l}, "right": {"kind": "place", "name": rr}}}),
        )
    };
    let wrong = intent(
        json!({"kind": "replace_function_body", "target": "ledger.line_total",
            "body": {"kind": "binary", "op": "+", "left": {"kind": "place", "name": "price"}, "right": {"kind": "place", "name": "qty"}}}),
    );
    let s = Script::new(vec![wrong, body("qty", "price")]);
    let accept = json!({"stable_id": "ledger.line_total", "contains": "qty * price"});
    let t = change(|t| {
        t.goal = "reorder the factors".into();
        t.acceptance = vec![accept];
        t.session = Some(SessionBounds::default());
    });
    let rep = go(&r, &cfg(&r, t), &s);
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    assert_eq!(rep.session["attempts"][0]["outcome"], "rejected");
    assert_eq!(rep.session["attempts"][0]["stage"], "checks");
    let fb = s.prompts.borrow()[1]["feedback"].to_string();
    assert!(
        fb.contains("tests failed") && fb.contains("ledger"),
        "the exact oracle failure is fed back: {fb}"
    );
    assert_eq!(rep.session["attempts"][1]["outcome"], "admitted");
    assert_eq!(rep.checks["tests"], "passed");
    // A compiler refusal diagnostic is fed back verbatim too.
    let bad = intent(
        json!({"kind": "change_function_signature", "target": "ledger.invoice_total",
            "append_parameters": [{"name": "d", "type": "widget", "argument": {"kind": "i64", "value": 0}}]}),
    );
    let s = Script::new(vec![bad, add_discount()]);
    let t = change(|t| {
        t.goal = "add a discount".into();
        t.session = Some(SessionBounds::default());
    });
    let rep = go(&r, &cfg(&r, t), &s);
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    assert!(s.prompts.borrow()[1]["feedback"]
        .to_string()
        .contains("SPX-G225"));
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_invalid_project_is_repaired_in_scratch_and_drift_rejects_application() {
    let r = rig("hp-hn02r", Some(("    price * qty\n", "    price * \n")));
    let broken = read(&r, "src/lib.spx");
    let t = change(|t| {
        t.mode = TaskMode::Repair;
        t.goal = "make the project compile".into();
        t.session = Some(SessionBounds::default());
    });
    // Without a session block the invalid baseline is only diagnosed.
    let rep = go(
        &r,
        &cfg(
            &r,
            Task {
                session: None,
                ..t.clone()
            },
        ),
        &Script::new(vec![patch("a", "b")]),
    );
    assert_eq!(rep.status, "diagnosed");
    // A wrong first patch keeps failing in scratch; the compiler's diagnostic feeds the second.
    let s = Script::new(vec![
        patch("    price * \n", "    price + \n"),
        patch("    price + \n", "    price * qty\n"),
    ]);
    let c = cfg(&r, t.clone());
    let rep = go(&r, &c, &s);
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    assert!(!s.prompts.borrow()[1]["feedback"].to_string().is_empty());
    assert_eq!(
        read(&r, "src/lib.spx"),
        broken,
        "the original is untouched by the repair"
    );
    let dir = PathBuf::from(rep.session["result"]["dir"].as_str().unwrap());
    let rev = rep.session["result"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    // Drift rejects final application; then the exact baseline accepts it.
    std::fs::write(r.project.join("src/lib.spx"), format!("{broken}// drift\n")).unwrap();
    assert_eq!(
        apply_result(&c.snapshot, &dir, &rev, &r.compiler)
            .unwrap_err()
            .code,
        "SPX-HPD115"
    );
    std::fs::write(r.project.join("src/lib.spx"), &broken).unwrap();
    assert_eq!(
        apply_result(&c.snapshot, &dir, &rev, &r.compiler).unwrap(),
        ["src/lib.spx"]
    );
    assert!(read(&r, "src/lib.spx").contains("price * qty"));
    assert!(
        r.compiler.check(&r.project).unwrap().ok && r.compiler.test(&r.project).unwrap().passed
    );
    // A failing repair never rewrites the original; malicious patches are refused.
    let r = rig("hp-hn02r2", Some(("    price * qty\n", "    price * \n")));
    let broken = read(&r, "src/lib.spx");
    let evil = [
        patch("    ensures result == price * qty\n", ""),
        json!({"schema": "semaprax.harness-proposal.v1", "source_patch": {"edits": [{"path": "src/tests.spx", "find": "12", "replace": "7"}]}}),
        patch("    price * \n", "    price * 0\n"),
    ];
    let rep = go(
        &r,
        &cfg(
            &r,
            change(|x| {
                x.mode = TaskMode::Repair;
                x.session = Some(SessionBounds {
                    max_attempts: 3,
                    ..Default::default()
                });
            }),
        ),
        &Script::new(evil.to_vec()),
    );
    let codes: Vec<_> = rep.session["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["code"].as_str().unwrap_or("-"))
        .collect();
    assert_eq!(codes[..2], ["SPX-HPD042", "SPX-HPD114"], "{codes:?}");
    assert_ne!(
        rep.status, "candidate-ready",
        "price * 0 breaks the law/tests so the compiler rejects it"
    );
    assert_eq!(read(&r, "src/lib.spx"), broken);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_multifile_task_needs_two_admitted_steps_and_satisfies_old_and_new_checks() {
    let r = rig("hp-hn02m", None);
    let t = change(|t| {
        t.goal =
            "rename line_total to line_cost and add a discount parameter to invoice_total".into();
        t.acceptance = vec![
            json!({"stable_id": "ledger.line_total", "contains": "line_cost"}),
            json!({"stable_id": "ledger.invoice_total", "contains": "discount"}),
        ];
        t.session = Some(SessionBounds::default());
    });
    let s = Script::new(vec![rename(), add_discount()]);
    let c = cfg(&r, t);
    let rep = go(&r, &c, &s);
    assert_eq!(
        rep.status, "candidate-ready",
        "{:?} {}",
        rep.refusals, rep.session
    );
    let steps = rep.session["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 2, "{steps:?}");
    assert_eq!(steps[1]["base_revision"], steps[0]["candidate_revision"]);
    let dir = PathBuf::from(rep.session["result"]["dir"].as_str().unwrap());
    // New checks (both acceptance items) and the original oracle on the final tree.
    assert!(std::fs::read_to_string(dir.join("src/lib.spx"))
        .unwrap()
        .contains("line_cost"));
    assert!(std::fs::read_to_string(dir.join("src/report.spx"))
        .unwrap()
        .contains("discount"));
    assert!(r.compiler.check(&dir).unwrap().ok);
    assert!(r.compiler.test(&dir).unwrap().passed);
    assert!(
        !read(&r, "src/report.spx").contains("discount"),
        "the project is untouched"
    );
    assert!(rep.session["result"]["apply"]
        .as_str()
        .unwrap()
        .contains("apply_result"));
    assert_eq!(
        rep.publication,
        Value::Null,
        "an internal success never publishes"
    );
    // The drift-checked authority applies the verified final tree.
    let rev = rep.session["result"]["revision"].as_str().unwrap();
    let applied = apply_result(&c.snapshot, &dir, rev, &r.compiler).unwrap();
    assert!(
        applied.contains(&"src/report.spx".to_string()),
        "{applied:?}"
    );
    assert!(r.compiler.test(&r.project).unwrap().passed);
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_repeated_bad_proposals_stop_oracle_edits_are_refused_and_uncertain_is_not_replayed()
{
    let r = rig("hp-hn02s", None);
    let bad = intent(
        json!({"kind": "change_function_signature", "target": "ledger.invoice_total",
            "append_parameters": [{"name": "d", "type": "widget", "argument": {"kind": "i64", "value": 0}}]}),
    );
    let s = Script::new(vec![bad.clone(), bad]);
    let rep = go(
        &r,
        &cfg(
            &r,
            change(|t| {
                t.session = Some(SessionBounds {
                    max_attempts: 6,
                    ..Default::default()
                })
            }),
        ),
        &s,
    );
    assert_eq!(
        (
            rep.status,
            rep.session["attempts_spent"].as_u64(),
            s.calls.get()
        ),
        ("no-progress", Some(2), 2)
    );
    // The oracle (tests module) cannot be a semantic target.
    let evil = intent(
        json!({"kind": "replace_function_body", "target": "ledger.tests.main", "body": {"kind": "i64", "value": 0}}),
    );
    let s = Script::new(vec![evil, rename()]);
    let rep = go(
        &r,
        &cfg(&r, change(|t| t.session = Some(SessionBounds::default()))),
        &s,
    );
    assert_eq!(rep.session["attempts"][0]["code"], "SPX-HPD114");
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    // An uncertain generation is recorded and a restart does not call the model again.
    let r2 = rig("hp-hn02u", None);
    let mut s = Script::new(vec![rename()]);
    s.side = true;
    s.uncertain = true;
    let c = cfg(&r2, change(|t| t.session = Some(SessionBounds::default())));
    let rep = go(&r2, &c, &s);
    assert_eq!((rep.status, s.calls.get()), ("uncertain", 1));
    let mut s2 = Script::new(vec![rename()]);
    s2.side = true;
    let rep = go(&r2, &c, &s2);
    assert_eq!((rep.status, s2.calls.get()), ("uncertain", 0));
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HN11_PYTHON HN11_TIKTOKEN_CACHE"]
fn hp_hn11_real_o200k_counts_the_exact_request_and_bytes_div_4_would_admit_an_oversized_one() {
    use semaprax_harness::decision::{Destination, ModelPlan};
    use semaprax_harness::observe::{ExternalTokenizer, Tokenizer};
    let py = required_tool("HN11_PYTHON");
    let cache = required_tool("HN11_TIKTOKEN_CACHE");
    let script = repo_root().join("scripts/harness_tokenize.py");
    let env = BTreeMap::from([
        (
            "TIKTOKEN_CACHE_DIR".to_string(),
            cache.to_string_lossy().into_owned(),
        ),
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
    ]);
    let args = vec![
        script.to_string_lossy().into_owned(),
        "o200k_base".to_string(),
    ];
    let tok = ExternalTokenizer::spawn(&py, &args, &env).expect("o200k_base from the local cache");
    assert_eq!(tok.name(), "o200k_base");
    // The byte heuristic under-counts code punctuation and non-Latin text.
    for text in [
        "{}[]();,:<>=+-*/&|!?~^%$#@".repeat(40),
        "日本語のコード、記号：数値".repeat(40),
        "🙂🚀🔥".repeat(80),
    ] {
        let n = tok.try_count(&text).unwrap();
        assert!(
            n > text.len() / 4 || text.len() / 4 - n < 5,
            "{n} vs {}",
            text.len() / 4
        );
    }
    let punct = "{}[]();,:<>=+-*/&|!?~^%$#@".repeat(40);
    assert!(
        tok.try_count(&punct).unwrap() > punct.len() / 3,
        "punctuation is ~1 token per few bytes, not 4 bytes per token"
    );
    let model = |max: u64| {
        serde_json::json!([ModelPlan {
            id: "gpt-4o".into(),
            destination: Destination::Local,
            structured_output: true,
            tools: false,
            max_context: max,
            est_cost_micros: 0,
            est_latency_ms: 1,
            strength_rank: 1
        }
        .to_json()])
    };
    let r = rig("hp-hn11r", None);
    let mk = |max: u64, goal: &str| {
        let mut c = cfg(
            &r,
            change(|t| {
                t.goal = goal.into();
                t.models = Some(model(max));
                t.budget = Some(semaprax_harness::workflow::budget::BudgetPolicy {
                    output_reserve_tokens: 200,
                    protocol_overhead_tokens: 50,
                    ..Default::default()
                });
            }),
        );
        let t = ExternalTokenizer::spawn(&py, &args, &env).unwrap();
        c.budget.tokenizers.add(Box::new(t));
        c
    };
    let s = Script::new(vec![rename()]);
    let rep = go(&r, &mk(10_000_000, &punct), &s);
    assert_eq!(rep.status, "candidate-ready", "{:?}", rep.refusals);
    let b = &rep.context["request_budget"];
    let sent = semaprax_harness::json::canonical(&s.prompts.borrow()[0]);
    assert_eq!(
        b["request_tokens"].as_u64().unwrap() as usize,
        tok.try_count(&sent).unwrap()
    );
    assert_eq!(b["tokenizer"]["name"], "o200k_base");
    assert!(b["tokenizer"]["fingerprint"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    let (bytes, tokens) = (
        b["request_bytes"].as_u64().unwrap(),
        b["request_tokens"].as_u64().unwrap(),
    );
    let limit = bytes / 4 + 250 + 100;
    assert!(
        tokens + 250 > limit,
        "the request is {tokens} tokens; bytes/4 admits it under {limit}"
    );
    let s = Script::new(vec![rename()]);
    let rep = go(&r, &mk(limit, &punct), &s);
    assert_eq!((rep.status, s.calls.get()), ("refused", 0));
    assert_eq!(rep.refusals[0].code, "SPX-HPD100");
}

/// Run the built binary with an isolated harness home and the real compiler.
fn bin(r: &Rig, args: &[&str]) -> (i32, String) {
    let home = r.cache.parent().unwrap().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let o = Command::new(harness_bin())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SEMAPRAX_HARNESS_HOME", &home)
        .env("SEMAPRAX_COMPILER", required_tool("SEMAPRAX_COMPILER"))
        .current_dir(&r.project)
        .output()
        .expect("run the harness binary");
    (
        o.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        ),
    )
}

fn task_file(r: &Rig, name: &str, v: Value) -> String {
    let p = r.cache.parent().unwrap().join(name);
    std::fs::write(&p, v.to_string()).unwrap();
    p.to_string_lossy().into_owned()
}

fn session_task() -> Value {
    json!({"schema": "semaprax.harness-task.v2", "mode": "change", "goal": "add a discount parameter",
            "seed": "ledger.invoice_total", "session": {"max_attempts": 3},
            "acceptance": [{"stable_id": "ledger.invoice_total", "contains": "discount"}]})
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_cli_run_session_then_apply_succeeds_and_drift_is_refused_without_publishing() {
    let r = rig("hp-hn02cli", None);
    let proposal = task_file(&r, "proposal.json", add_discount());
    let task = task_file(&r, "task.json", session_task());
    let project = r.project.to_str().unwrap().to_string();
    let (code, out) = bin(
        &r,
        &[
            "run",
            &project,
            "--task",
            &task,
            "--proposal",
            &proposal,
            "--json",
        ],
    );
    assert_eq!(code, 0, "{out}");
    let rep: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(rep["status"], "candidate-ready");
    let rev = rep["session"]["result"]["revision"]
        .as_str()
        .unwrap()
        .to_string();
    let report = task_file(&r, "report.json", rep.clone());
    assert!(
        !read(&r, "src/report.spx").contains("discount"),
        "run never writes the project"
    );
    // Drift: the project changed after the session captured its baseline.
    let before = read(&r, "src/lib.spx");
    std::fs::write(r.project.join("src/lib.spx"), format!("{before}// drift\n")).unwrap();
    let (code, out) = bin(
        &r,
        &[
            "apply",
            &project,
            "--session",
            &report,
            "--expected-revision",
            &rev,
            "--json",
        ],
    );
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("SPX-HPD115"), "{out}");
    assert!(!read(&r, "src/report.spx").contains("discount"));
    // A wrong expected revision is refused too.
    std::fs::write(r.project.join("src/lib.spx"), &before).unwrap();
    let (code, out) = bin(
        &r,
        &[
            "apply",
            &project,
            "--session",
            &report,
            "--expected-revision",
            "sha256:00",
            "--json",
        ],
    );
    assert_eq!((code, out.contains("SPX-HPD115")), (1, true), "{out}");
    // The undrifted project accepts the verified result; nothing is published.
    let (code, out) = bin(
        &r,
        &[
            "apply",
            &project,
            "--session",
            &report,
            "--expected-revision",
            &rev,
            "--json",
        ],
    );
    assert_eq!(code, 0, "{out}");
    let v: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(
        (v["status"].as_str(), v["published"].as_bool()),
        (Some("applied"), Some(false))
    );
    assert!(read(&r, "src/report.spx").contains("discount"));
    assert!(
        r.compiler.check(&r.project).unwrap().ok && r.compiler.test(&r.project).unwrap().passed
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hn02_real_cli_run_cancel_file_records_a_cancelled_session() {
    let r = rig("hp-hn02cancel", None);
    let proposal = task_file(&r, "proposal.json", add_discount());
    let task = task_file(&r, "task.json", session_task());
    let cancel = task_file(&r, "cancel.flag", json!("stop"));
    let project = r.project.to_str().unwrap().to_string();
    let (code, out) = bin(
        &r,
        &[
            "run",
            &project,
            "--task",
            &task,
            "--proposal",
            &proposal,
            "--cancel-file",
            &cancel,
            "--json",
        ],
    );
    assert_eq!(code, 1, "{out}");
    let rep: Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(rep["status"], "cancelled");
    assert_eq!(rep["refusals"][0]["code"], "SPX-HPD113");
    assert_eq!(
        rep["session"]["attempts_spent"], 0,
        "no generation started after the cancel"
    );
    let home = r.cache.parent().unwrap().join("home/cache/workflow");
    let lineage = rep["lineage"].as_str().unwrap();
    let journal = std::fs::read_dir(&home)
        .unwrap()
        .flatten()
        .map(|d| d.path().join(format!("{lineage}.journal.jsonl")))
        .find(|p| p.is_file())
        .expect("journal");
    assert!(std::fs::read_to_string(journal)
        .unwrap()
        .contains("\"cancelled\""));
    assert!(!read(&r, "src/report.spx").contains("discount"));
}
