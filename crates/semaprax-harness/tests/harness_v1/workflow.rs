//! HP-04 workflow tests that need no real compiler: a fake `CompilerService`
//! drives the pipeline (fixture prefix `hp-hp04`). Real-compiler evidence is
//! in tests/real_tools_v1/workflow_compiler.rs.

use crate::support::*;
use semaprax_harness::json::sha256_plain;
use semaprax_harness::observe::{Observer, ObserverLimits};
use semaprax_harness::workflow::compiler::parse_text_diagnostic;
use semaprax_harness::workflow::pipeline::change_bytes;
use semaprax_harness::workflow::policy::REQUIREMENTS;
use semaprax_harness::workflow::stages::*;
use semaprax_harness::workflow::*;
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

const LIB: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    BUG\n}\n";
const FIXED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x\n}\n";

fn rev_of(s: &str) -> String {
    sha256_plain(s.as_bytes())
}

struct Fake {
    preview_source: RefCell<String>,
    requirements: RefCell<Vec<String>>,
    publish_result: RefCell<Result<(), PublishError>>,
    publishes: Cell<u32>,
    log: RefCell<Vec<String>>,
    ops: RefCell<Option<Vec<String>>>,
}

impl Fake {
    fn new(src: &str) -> Self {
        Fake {
            preview_source: RefCell::new(src.into()),
            requirements: RefCell::new(REQUIREMENTS.iter().map(|s| s.to_string()).collect()),
            publish_result: RefCell::new(Ok(())),
            publishes: Cell::new(0),
            log: RefCell::default(),
            ops: RefCell::default(),
        }
    }
    fn lib(&self, p: &Path) -> String {
        std::fs::read_to_string(p.join("src/lib.spx")).unwrap()
    }
}

impl CompilerService for Fake {
    fn version(&self) -> String {
        "fake".into()
    }
    fn check(&self, p: &Path) -> semaprax_harness::diag::HarnessResult<CheckReport> {
        self.log.borrow_mut().push("check".into());
        if self.lib(p).contains("SYNTAXERR") {
            return Ok(CheckReport {
                ok: false,
                revision: None,
                diagnostics: vec![CompilerDiagnostic {
                    code: "SPX-P001".into(),
                    message: "unexpected token `SYNTAXERR`".into(),
                    path: Some("src/lib.spx".into()),
                    line: Some(8),
                }],
                raw_digest: "x".into(),
            });
        }
        Ok(CheckReport {
            ok: true,
            revision: Some(rev_of(&self.lib(p))),
            diagnostics: vec![],
            raw_digest: "x".into(),
        })
    }
    fn test(&self, p: &Path) -> semaprax_harness::diag::HarnessResult<TestReport> {
        self.log.borrow_mut().push("test".into());
        let src = self.lib(p);
        let bug = src.contains("BUG");
        Ok(TestReport {
            passed: !bug,
            project_revision: rev_of(&src),
            outcome: if bug {
                "language_failure".into()
            } else {
                "returned".into()
            },
            failing_function: bug.then(|| "t.f".to_string()),
            failure: bug.then(|| "t.f ensures: result == x".to_string()),
            report_digest: "d".into(),
        })
    }
    fn context(
        &self,
        _p: &Path,
        seed: &str,
        _m: usize,
    ) -> semaprax_harness::diag::HarnessResult<String> {
        self.log.borrow_mut().push("context".into());
        Ok(format!("{{\"target\":\"{seed}\"}}"))
    }
    fn candidate_preview(
        &self,
        p: &Path,
        c: &[u8],
    ) -> semaprax_harness::diag::HarnessResult<CandidatePreview> {
        self.log.borrow_mut().push("preview".into());
        let base = std::fs::read(p.join("src/lib.spx")).unwrap();
        // Test hooks in the intent: `fake_refuse` (compiler refusal text) and
        // `fake_source` (the candidate the "compiler" produces).
        let intent: Value = serde_json::from_slice::<Value>(c).unwrap()["intent"].clone();
        if let Some(m) = intent["fake_refuse"].as_str() {
            return Err(semaprax_harness::diag::HarnessDiagnostic::new(
                "SPX-HPD040",
                format!("compiler refused `project-candidate-preview`: {m}"),
            ));
        }
        let src = intent["fake_source"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| self.preview_source.borrow().clone());
        let reqs = self.requirements.borrow().clone();
        Ok(CandidatePreview {
            base_revision: rev_of(&String::from_utf8_lossy(&base)),
            candidate_revision: rev_of(&src),
            source_changes: vec![SourceChange {
                path: "src/lib.spx".into(),
                base_digest: semaprax_harness::workflow::policy::source_digest(&base),
                candidate_digest: rev_of(&src),
                replacement_source: src,
            }],
            requirements: reqs.clone(),
            change_requirements: vec![reqs],
            unresolved_holes: 0,
            tests_state: "not_run".into(),
            digest: "pd".into(),
        })
    }
    fn candidate_export(
        &self,
        p: &Path,
        c: &[u8],
    ) -> semaprax_harness::diag::HarnessResult<Capsule> {
        let pv = self.candidate_preview(p, c)?;
        Ok(Capsule {
            bytes: b"{}".to_vec(),
            candidate_digest: "sha256:cand".into(),
            base_revision: pv.base_revision,
            candidate_project_revision: pv.candidate_revision,
        })
    }
    fn publish(
        &self,
        _p: &Path,
        _c: &Capsule,
        _d: &str,
        _pol: &Path,
    ) -> Result<PublishReceipt, PublishError> {
        self.publishes.set(self.publishes.get() + 1);
        self.publish_result
            .borrow()
            .clone()
            .map(|_| PublishReceipt {
                published_commit: "c0ffee".into(),
                reference: "refs/heads/main".into(),
                raw: json!({}),
            })
    }
    fn commands(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
    fn supported_intents(
        &self,
        _p: &Path,
        _r: &str,
    ) -> semaprax_harness::diag::HarnessResult<Vec<String>> {
        Ok(self
            .ops
            .borrow()
            .clone()
            .unwrap_or_else(|| INTENT_KINDS.iter().map(|s| s.to_string()).collect()))
    }
}

struct Counting(u32, bool);
impl ContextStage for Counting {
    fn id(&self) -> String {
        "org.example/counting".into()
    }
    fn collect(&mut self, _r: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        self.0 += 1;
        let text = "x".repeat(if self.1 { 100_000 } else { 10 });
        Ok(ContextPacket {
            provider: self.id(),
            items: vec![ContextItem {
                label: "p:1-2".into(),
                provenance: "external:inferred".into(),
                text,
            }],
            complete: true,
        })
    }
    fn calls(&self) -> u32 {
        self.0
    }
}

struct Env {
    root: PathBuf,
    project: PathBuf,
    cache: PathBuf,
}

fn setup(src: &str) -> Env {
    let root = fixture_dir("hp-hp04").canonicalize().unwrap();
    let project = root.join("project");
    write(
        &project,
        "semaprax.toml",
        "schema = \"semaprax.manifest.v1\"\n",
    );
    write(&project, "src/lib.spx", src);
    Env {
        cache: root.join("cache"),
        root,
        project,
    }
}

fn config(e: &Env, task: Task, policy: Option<ApplyPolicy>) -> RunConfig {
    RunConfig {
        snapshot: Snapshot::capture(&e.project).unwrap(),
        task,
        context_max_bytes: 8192,
        cache_dir: e.cache.clone(),
        lock_digest: "sha256:lock".into(),
        providers: vec![],
        composition: Composition::from_profile(None, true, vec![], &[]).unwrap(),
        apply_policy: policy,
        checks: vec![],
        skill_prompt: None,
        endpoint_policy: Default::default(),
        model_plans: None,
        notes: vec![],
        budget: Default::default(),
        cancel: None,
        routing: Default::default(),
        context_target: None,
    }
}

fn proposal(kind: &str) -> Vec<u8> {
    json!({"schema": "semaprax.harness-proposal.v1", "intent": {"kind": kind, "target": "t.f"}})
        .to_string()
        .into_bytes()
}

fn go(cfg: &RunConfig, fake: &Fake, ext: Option<&mut Counting>, prop: Vec<u8>) -> Report {
    let mut native = NativeContext::new(fake);
    let mut p = ScriptedProposer::from_bytes(prop);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    run(
        cfg,
        fake,
        Stages {
            decision: None,
            native: &mut native,
            external: ext.map(|e| e as &mut dyn ContextStage),
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    )
}

fn codes(r: &Report) -> Vec<&'static str> {
    r.refusals.iter().map(|d| d.code).collect()
}

fn policy_files(e: &Env) -> ApplyPolicy {
    let pub_pol = write(&e.root, "host/git-policy.json", "{}");
    let pol = write(&e.root, "host/apply.json", &json!({"schema": "semaprax.harness-apply-policy.v1", "auto_apply": true, "publication_policy": pub_pol}).to_string());
    ApplyPolicy::load(&pol, &Snapshot::capture(&e.project).unwrap().root).unwrap()
}

#[test]
fn hp_hp04_valid_candidate_stops_at_approved_ready_with_zero_external_calls() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let mut ext = Counting(0, false);
    let cfg = config(&e, Task::default(), None);
    let r = go(
        &cfg,
        &fake,
        Some(&mut ext),
        proposal("replace_function_body"),
    );
    assert_eq!(r.status, "approved-candidate-ready", "{:?}", r.refusals);
    assert_eq!(
        ext.0, 0,
        "native context was sufficient: no host invocation"
    );
    assert_eq!(r.external_calls, 0);
    assert_eq!(fake.publishes.get(), 0);
    assert_eq!(r.checks["tests"], "passed");
    // The project source is untouched.
    assert_eq!(
        std::fs::read_to_string(e.project.join("src/lib.spx")).unwrap(),
        LIB
    );
}

#[test]
fn hp_hp04_external_context_only_when_requested_or_needed() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let always = Task {
        external_context: ExternalContext::Always,
        ..Task::default()
    };
    let mut ext = Counting(0, false);
    let r = go(
        &config(&e, always, None),
        &fake,
        Some(&mut ext),
        proposal("replace_function_body"),
    );
    assert_eq!(r.status, "approved-candidate-ready");
    assert_eq!(ext.0, 1);
    // Seed unknown (no failing function named) is not the case here, so force `never`.
    let never = Task {
        external_context: ExternalContext::Never,
        ..Task::default()
    };
    let mut ext = Counting(0, false);
    go(
        &config(&e, never, None),
        &fake,
        Some(&mut ext),
        proposal("replace_function_body"),
    );
    assert_eq!(ext.0, 0);
}

#[test]
fn hp_hp04_context_budget_is_enforced_after_collection() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let t = Task {
        external_context: ExternalContext::Always,
        ..Task::default()
    };
    let mut ext = Counting(0, true);
    let r = go(
        &config(&e, t, None),
        &fake,
        Some(&mut ext),
        proposal("replace_function_body"),
    );
    assert_eq!(r.context["dropped_external_items"], 1);
    assert!(r.context["used_bytes"].as_u64().unwrap() <= 8192);
    // Protected native context larger than the budget is refused, not truncated.
    let mut cfg = config(&e, Task::default(), None);
    cfg.context_max_bytes = 5;
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(codes(&r), ["SPX-HPD020"]);
}

#[test]
fn hp_hp04_stale_revision_is_refused() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let cfg = config(&e, Task::default(), None);
    write(
        &e.project,
        "src/lib.spx",
        &format!("{LIB}// edited after the snapshot\n"),
    );
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(codes(&r), ["SPX-HPD005"]);
    assert_eq!(r.status, "refused");
    assert!(
        fake.log.borrow().is_empty(),
        "no compiler call happens on a stale snapshot"
    );
}

#[test]
fn hp_hp04_deleted_law_and_widened_effect_and_weakened_requirements_fail() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), None);
    let deleted = FIXED.replace("    ensures result == x\n", "");
    let r = go(
        &cfg,
        &Fake::new(&deleted),
        None,
        proposal("replace_function_body"),
    );
    assert_eq!(codes(&r), ["SPX-HPD042"]);
    let widened = FIXED.replace("uses { clock.read }", "uses { clock.read, fs.write }");
    let r = go(
        &cfg,
        &Fake::new(&widened),
        None,
        proposal("replace_function_body"),
    );
    assert_eq!(codes(&r), ["SPX-HPD043"]);
    let weak = Fake::new(FIXED);
    weak.requirements.borrow_mut().pop();
    let r = go(&cfg, &weak, None, proposal("replace_function_body"));
    assert_eq!(codes(&r), ["SPX-HPD044"]);
    // An added law is allowed.
    let added = FIXED.replace(
        "    ensures result == x\n",
        "    ensures result == x\n    ensures result >= 0\n",
    );
    let r = go(
        &cfg,
        &Fake::new(&added),
        None,
        proposal("replace_function_body"),
    );
    assert_eq!(r.status, "approved-candidate-ready", "{:?}", r.refusals);
}

#[test]
fn hp_hp04_fabricated_passing_tests_are_ignored_and_failures_never_become_success() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), None);
    let p = json!({"schema": "semaprax.harness-proposal.v1", "claims": {"tests_passed": true},
                   "intent": {"kind": "replace_function_body", "target": "t.f"}});
    // The compiler's verdict on the candidate (still BUG) is what counts.
    let fake = Fake::new(LIB);
    let r = go(&cfg, &fake, None, p.to_string().into_bytes());
    assert_eq!(r.status, "rejected");
    assert_eq!(codes(&r), ["SPX-HPD050"]);
    assert_eq!(r.ignored_claims, ["tests_passed"]);
    // Even with a policy present, nothing is published.
    let cfg = config(&e, Task::default(), Some(policy_files(&e)));
    let fake = Fake::new(LIB);
    go(&cfg, &fake, None, p.to_string().into_bytes());
    assert_eq!(fake.publishes.get(), 0);
}

#[test]
fn hp_hp04_provider_initiated_publication_never_reaches_publication() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), Some(policy_files(&e)));
    let fake = Fake::new(FIXED);
    let p = json!({"schema": "semaprax.harness-proposal.v1", "publish": true,
                   "intent": {"kind": "replace_function_body", "target": "t.f"}});
    let r = go(&cfg, &fake, None, p.to_string().into_bytes());
    assert_eq!(codes(&r), ["SPX-HPD033"]);
    assert_eq!(fake.publishes.get(), 0);
    assert!(fake.log.borrow().iter().all(|c| c != "preview"));
}

#[test]
fn hp_hp04_proposal_boundaries_raw_source_unsupported_kind_protected_members() {
    for (body, code) in [
        (
            json!({"schema": "semaprax.harness-proposal.v1", "source": "x", "intent": {"kind": "replace_function_body"}}),
            "SPX-HPD031",
        ),
        (
            json!({"schema": "semaprax.harness-proposal.v1", "intent": {"kind": "rewrite_file"}}),
            "SPX-HPD031",
        ),
        (
            json!({"schema": "semaprax.harness-proposal.v1", "requirements": [], "intent": {"kind": "add_contract"}}),
            "SPX-HPD032",
        ),
        (
            json!({"schema": "semaprax.harness-proposal.v1", "base_revision": "x", "intent": {"kind": "add_contract"}}),
            "SPX-HPD032",
        ),
        (
            json!({"schema": "other", "intent": {"kind": "add_contract"}}),
            "SPX-HPD030",
        ),
        (
            json!({"schema": "semaprax.harness-proposal.v1", "zzz": 1, "intent": {"kind": "add_contract"}}),
            "SPX-HPD030",
        ),
    ] {
        assert_eq!(
            parse_proposal(body.to_string().as_bytes())
                .unwrap_err()
                .code,
            code,
            "{body}"
        );
    }
    let unsupported = parse_proposal(&proposal("rewrite_file"));
    assert!(
        unsupported
            .unwrap_err()
            .message
            .contains("replace_function_body"),
        "names the admitted kinds"
    );
}

#[test]
fn hp_hp04_publication_needs_a_policy_outside_the_project() {
    let e = setup(LIB);
    let inside = write(&e.project, "apply.json", "{}");
    assert_eq!(
        ApplyPolicy::load(&inside, &Snapshot::capture(&e.project).unwrap().root)
            .unwrap_err()
            .code,
        "SPX-HPD082"
    );
    let pub_in = write(&e.project, "gp.json", "{}");
    let pol = write(&e.root, "host/a.json", &json!({"schema": "semaprax.harness-apply-policy.v1", "auto_apply": true, "publication_policy": pub_in}).to_string());
    assert_eq!(
        ApplyPolicy::load(&pol, &Snapshot::capture(&e.project).unwrap().root)
            .unwrap_err()
            .code,
        "SPX-HPD060"
    );
}

#[test]
fn hp_hp04_policy_publishes_and_resume_never_replays() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), Some(policy_files(&e)));
    let fake = Fake::new(FIXED);
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(r.status, "published", "{:?}", r.refusals);
    assert_eq!(fake.publishes.get(), 1);
    let r2 = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(r2.status, "published");
    assert_eq!(fake.publishes.get(), 1, "restart does not publish again");
}

#[test]
fn hp_hp04_uncertain_publication_is_never_retried() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), Some(policy_files(&e)));
    let fake = Fake::new(FIXED);
    *fake.publish_result.borrow_mut() = Err(PublishError::Uncertain(
        "SPX-G267 publication may have occurred".into(),
    ));
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(r.status, "uncertain");
    assert_eq!(codes(&r), ["SPX-HPD062"]);
    *fake.publish_result.borrow_mut() = Ok(());
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    assert_eq!(r.status, "uncertain");
    assert!(r.refusals[0]
        .message
        .contains("manual reconciliation required"));
    assert_eq!(fake.publishes.get(), 1, "the second run did not retry");
    // A refusal before any reference update is not uncertain and may be retried in a new run.
    let e2 = setup(LIB);
    let cfg2 = config(&e2, Task::default(), Some(policy_files(&e2)));
    let f2 = Fake::new(FIXED);
    *f2.publish_result.borrow_mut() = Err(PublishError::Refused(parse_text_diagnostic(
        "error[SPX-G265]: moved",
    )));
    assert_eq!(
        codes(&go(&cfg2, &f2, None, proposal("replace_function_body"))),
        ["SPX-HPD061"]
    );
}

struct Hostile(u32);
impl ProposalStage for Hostile {
    fn id(&self) -> String {
        "org.example/model".into()
    }
    fn propose(&mut self, _r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.0 += 1;
        Err(StageFailure::Uncertain(
            semaprax_harness::diag::HarnessDiagnostic::new("SPX-HPC018", "cancelled after send"),
        ))
    }
    fn calls(&self) -> u32 {
        self.0
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

#[test]
fn hp_hp04_uncertain_generation_is_not_replayed_on_restart() {
    let e = setup(LIB);
    let cfg = config(&e, Task::default(), None);
    let fake = Fake::new(FIXED);
    let mut native = NativeContext::new(&fake);
    let mut model = Hostile(0);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    for _ in 0..2 {
        let r = run(
            &cfg,
            &fake,
            Stages {
                decision: None,
                native: &mut native,
                external: None,
                proposer: &mut model,
                command: &mut view,
            },
            &mut obs,
        );
        assert_eq!(r.status, "uncertain");
    }
    assert_eq!(
        model.0, 1,
        "the side-effecting call was made once across two runs"
    );
}

#[test]
fn hp_hp04_composition_is_typed_deterministic_and_refuses_violations() {
    let base = Composition::from_profile(None, false, vec![], &[]).unwrap();
    assert_eq!(base.order, StageId::ALL.to_vec());
    assert_eq!(
        base,
        Composition::from_profile(None, false, vec![], &[]).unwrap()
    );
    let dup = Slot {
        stage: StageId::Context,
        owner: "org.example/other".into(),
        after: vec![],
    };
    assert_eq!(
        Composition::from_profile(None, false, vec![dup], &[])
            .unwrap_err()
            .code,
        "SPX-HPD011"
    );
    // A cycle: build slots by hand with context depending on generate.
    let mut slots: Vec<Slot> = base.slots.values().cloned().collect();
    slots
        .iter_mut()
        .find(|s| s.stage == StageId::Context)
        .unwrap()
        .after = vec![StageId::Generate];
    assert_eq!(
        Composition::build(slots, &[]).unwrap_err().code,
        "SPX-HPD010"
    );
    let edge = |a: &str, b: &str| Interception {
        from: a.into(),
        to: b.into(),
    };
    assert_eq!(
        Composition::from_profile(None, false, vec![], &[edge("rtk", "rtk")])
            .unwrap_err()
            .code,
        "SPX-HPD012"
    );
    assert_eq!(
        Composition::from_profile(None, false, vec![], &[edge("a", "b"), edge("b", "a")])
            .unwrap_err()
            .code,
        "SPX-HPD012"
    );
    assert!(
        Composition::from_profile(None, false, vec![], &[edge("a", "b"), edge("b", "c")]).is_ok()
    );
}

#[test]
fn hp_hp04_report_and_observations_carry_no_goal_text() {
    let e = setup(LIB);
    let task = Task {
        goal: "use key sk-live-SECRET-123 to fix".into(),
        ..Task::default()
    };
    let cfg = config(&e, task, None);
    let fake = Fake::new(FIXED);
    let r = go(&cfg, &fake, None, proposal("replace_function_body"));
    let text = format!("{}{}", r.to_json(), r.to_text());
    assert!(!text.contains("SECRET"));
    assert_eq!(r.to_json()["schema"], "semaprax.harness-run.v1");
    // Deterministic rendering.
    assert_eq!(
        r.to_json().to_string(),
        go(
            &cfg,
            &Fake::new(FIXED),
            None,
            proposal("replace_function_body")
        )
        .to_json()
        .to_string()
    );
}

#[test]
fn hp_hp04_change_bytes_are_canonical_and_host_owned() {
    let b = change_bytes(
        "sha256:r",
        &json!({"kind": "add_contract", "target": "t.f"}),
    );
    let s = String::from_utf8(b).unwrap();
    assert!(s.ends_with("}\n") && s.starts_with("{\"base_revision\":\"sha256:r\",\"intent\":"));
    for req in REQUIREMENTS {
        assert!(s.contains(req));
    }
}

#[test]
fn hp_hp04_cli_requires_a_compiler_and_a_known_option() {
    let home = fixture_dir("hp-hp04");
    let env = semaprax_harness::cli::Environment {
        harness_home: Some(home.clone()),
        compiler: None,
        cwd: home.clone(),
        vars: Default::default(),
    };
    let o = semaprax_harness::cli::run(&["run".into(), home.to_string_lossy().into_owned()], &env);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPD001"), "{}", o.stderr);
    let o = semaprax_harness::cli::run(&["run".into(), "--bogus".into()], &env);
    assert_eq!(o.code, 2);
    let _: Value = json!(null);
}

// ---- hpwire: checks, skills, external decision, observations ----

use semaprax_harness::cli::Environment;
use semaprax_harness::contract::RequestEnvelope;
use semaprax_harness::decision::{
    DecisionCall, DecisionInvoker, Destination, ModelPlan, ProviderMode, ProviderProfile,
};

struct Captured(RefCell<Option<Value>>);
struct Capture<'a>(&'a Captured, Vec<u8>);
impl ProposalStage for Capture<'_> {
    fn id(&self) -> String {
        "org.example/capture".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        *self.0 .0.borrow_mut() = Some(r.prompt.clone());
        Ok(self.1.clone())
    }
    fn calls(&self) -> u32 {
        1
    }
    fn side_effecting(&self) -> bool {
        false
    }
}

/// Command stage whose checks are scripted (the pipeline's verdict contract).
struct ScriptedChecks(Vec<bool>);
impl CommandStage for ScriptedChecks {
    fn id(&self) -> String {
        "org.example/scripted-checks".into()
    }
    fn view(&mut self, _l: &str, raw: &str, _m: usize) -> String {
        raw.into()
    }
    fn run_check(
        &mut self,
        c: &CheckSpec,
        _w: &Path,
        _o: &mut Observer,
    ) -> Option<Result<CheckRun, semaprax_harness::diag::HarnessDiagnostic>> {
        let ok = self.0.remove(0);
        Some(Ok(CheckRun {
            name: c.name.clone(),
            argv_digest: "d".into(),
            passed: ok,
            status: if ok { "exit:0" } else { "exit:1" }.into(),
            status_certain: true,
            executions: 1,
            view: "model-facing summary".into(),
            view_route: "provider".into(),
            view_provenance: "ai.rtk/rtk-command-view".into(),
            view_incomplete: false,
            recovery: None,
            measurement: None,
        }))
    }
}

fn check(name: &str) -> CheckSpec {
    CheckSpec {
        name: name.into(),
        argv: vec!["true".into()],
    }
}

fn run_with_command(
    cfg: &RunConfig,
    fake: &Fake,
    command: &mut dyn CommandStage,
    obs: &mut Observer,
) -> Report {
    let mut native = NativeContext::new(fake);
    let mut p = ScriptedProposer::from_bytes(proposal("replace_function_body"));
    run(
        cfg,
        fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command,
        },
        obs,
    )
}

#[test]
fn hp_hpwire_authorized_check_verdict_is_the_commands_not_the_views() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let mut cfg = config(&e, Task::default(), None);
    cfg.checks = vec![check("unit"), check("lint")];
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run_with_command(&cfg, &fake, &mut ScriptedChecks(vec![true, true]), &mut obs);
    assert_eq!(r.status, "approved-candidate-ready", "{:?}", r.refusals);
    let cmds = r.checks["commands"].as_array().unwrap();
    assert_eq!(cmds.len(), 2);
    assert_eq!(cmds[0]["view"], "model-facing summary");
    assert_eq!(cmds[0]["executions"], 1);
    // A failing check rejects the candidate even though the view looks fine.
    let fake = Fake::new(FIXED);
    let r = run_with_command(
        &cfg,
        &fake,
        &mut ScriptedChecks(vec![true, false]),
        &mut obs,
    );
    assert_eq!(r.status, "rejected");
    assert_eq!(codes(&r), ["SPX-HPD050"]);
    assert!(r.refusals[0].message.contains("`lint`"));
    // A stage that cannot run checks refuses rather than skipping them.
    let fake = Fake::new(FIXED);
    let r = run_with_command(&cfg, &fake, &mut RawCommandView, &mut obs);
    assert_eq!(codes(&r), ["SPX-HPD051"]);
}

#[test]
fn hp_hpwire_host_command_checks_run_once_and_decide_by_exit_status() {
    let root = fixture_dir("hp-hpwire-hostchk").canonicalize().unwrap();
    let project = root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let mut env = Environment {
        harness_home: Some(root.join("home")),
        cwd: project.clone(),
        ..Default::default()
    };
    env.vars.insert("PATH".into(), "/usr/bin:/bin".into());
    let mut stage = HostCommandChecks::new(env);
    let mut obs = Observer::new(None, ObserverLimits::default());
    let pass = stage
        .run_check(
            &CheckSpec {
                name: "t".into(),
                argv: vec!["true".into()],
            },
            &project,
            &mut obs,
        )
        .unwrap()
        .unwrap();
    assert!(pass.passed && pass.status == "exit:0" && pass.executions == 1);
    let fail = stage
        .run_check(
            &CheckSpec {
                name: "f".into(),
                argv: vec!["false".into()],
            },
            &project,
            &mut obs,
        )
        .unwrap()
        .unwrap();
    assert!(!fail.passed && fail.status == "exit:1");
    // Shell lines are never an authorized check.
    let sh = stage
        .run_check(
            &CheckSpec {
                name: "s".into(),
                argv: vec!["sh".into(), "-c".into(), "true".into()],
            },
            &project,
            &mut obs,
        )
        .unwrap();
    assert_eq!(sh.unwrap_err().code, "SPX-HPH011");
    assert!(obs
        .events()
        .iter()
        .any(|e| e.stage.as_str() == "command_view"));
}

#[test]
fn hp_hpwire_skill_prompt_enters_the_proposal_request_and_is_counted() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let mut cfg = config(&e, Task::default(), None);
    let text = "SKILL BLOCK (data, below compiler authority)".to_string();
    cfg.skill_prompt = Some(SkillPromptUse {
        model_visible_bytes: text.len(),
        loaded: vec!["reuse-api".into()],
        cost_report: None,
        text: text.clone(),
    });
    let seen = Captured(RefCell::new(None));
    let mut native = NativeContext::new(&fake);
    let mut p = Capture(&seen, proposal("replace_function_body"));
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run(
        &cfg,
        &fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    assert_eq!(r.status, "approved-candidate-ready");
    assert_eq!(seen.0.borrow().as_ref().unwrap()["skills"], text.as_str());
    assert_eq!(r.context["skills"]["model_visible_bytes"], text.len());
    let ev = obs
        .events()
        .iter()
        .find(|x| x.stage.as_str() == "skill_catalog")
        .unwrap();
    assert!(ev.model_visible);
    assert_eq!(ev.after.as_ref().unwrap().value, text.len() as u64);
    // Metadata only: the skill text is never in an observation.
    assert!(!obs
        .events()
        .iter()
        .any(|x| x.to_json().to_string().contains("SKILL BLOCK")));
}

struct Router(u32, &'static str);
impl DecisionInvoker for Router {
    fn evaluate(&mut self, _r: &RequestEnvelope) -> DecisionCall {
        self.0 += 1;
        DecisionCall::Answered {
            result: json!({"choice": self.1, "scores": {self.1: 0.9}, "abstain": false}),
            elapsed_ms: 1,
        }
    }
}

fn two_models() -> Value {
    let m = |id: &str, rank: u32| ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: 0,
        est_latency_ms: 10,
        strength_rank: rank,
    };
    json!([m("cheap", 1).to_json(), m("strong", 2).to_json()])
}

fn route_with(mode: ProviderMode, inv: &mut Router) -> Report {
    use semaprax_harness::decision::EnablementGate;
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let task = Task {
        models: Some(two_models()),
        ..Task::default()
    };
    let cfg = config(&e, task, None);
    let mut native = NativeContext::new(&fake);
    // A model-like (non-scripted) proposer: a scripted one is taken locally
    // before routing (TC-09), so it would never consult the router.
    let captured = Captured(RefCell::new(None));
    let mut p = Capture(&captured, proposal("replace_function_body"));
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let profile = ProviderProfile {
        provider_id: "org.example/threshold-route".into(),
        model_id: "m".into(),
        checkpoint: "1".into(),
        min_confidence: None,
        max_context_tokens: None,
        supported_families: None,
    };
    let gate = EnablementGate::not_evaluated("model-route/v1", &profile.provider_id);
    run(
        &cfg,
        &fake,
        Stages {
            decision: Some(DecisionStage {
                invoker: inv,
                profile,
                mode,
                gate,
            }),
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    )
}

#[test]
fn hp_hpwire_external_decision_is_consulted_only_when_explicit_or_gate_passed() {
    // Explicit pin: experimental, consulted once, its admissible choice is used.
    let mut inv = Router(0, "strong");
    let r = route_with(ProviderMode::Explicit, &mut inv);
    assert_eq!(inv.0, 1);
    assert_eq!(r.route["router_calls"], 1);
    assert_eq!(r.route["choice"], "strong");
    assert_eq!(r.route["status"], "experimental");
    // Automatic selection without a passed gate: rules decide, zero calls.
    let mut inv = Router(0, "strong");
    let r = route_with(ProviderMode::Auto, &mut inv);
    assert_eq!(inv.0, 0);
    assert_eq!(r.route["router_calls"], 0);
    assert!(r.route["status"].as_str().unwrap().starts_with("rules"));
    // A choice outside the admissible set falls back to rules.
    let mut inv = Router(0, "ghost");
    let r = route_with(ProviderMode::Explicit, &mut inv);
    assert_ne!(r.route["choice"], "ghost");
    assert!(r.route["source"].as_str().unwrap().starts_with("Fallback"));
}

#[test]
fn hp_hpwire_model_policy_refuses_a_remote_model_under_local_only() {
    let e = setup(LIB);
    let fake = Fake::new(FIXED);
    let remote = ModelPlan {
        id: "cloud".into(),
        destination: Destination::Remote {
            origin: "https://api.example".into(),
        },
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: 0,
        est_latency_ms: 10,
        strength_rank: 1,
    };
    let mut cfg = config(&e, Task::default(), None);
    cfg.model_plans = Some(vec![remote]);
    cfg.endpoint_policy = semaprax_harness::endpoint::EndpointPolicy {
        local_only: true,
        strict_one_attempt: false,
    };
    // Model-like proposer: a scripted one bypasses routing (TC-09).
    let captured = Captured(RefCell::new(None));
    let mut p = Capture(&captured, proposal("replace_function_body"));
    let mut native = NativeContext::new(&fake);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run(
        &cfg,
        &fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    assert_eq!(codes(&r), ["SPX-HPL011"]);
}

#[test]
fn hp_hpwire_host_sets_bridge_depth_for_children_but_callers_cannot() {
    use semaprax_harness::command_view::{execute, ExecOptions};
    let root = fixture_dir("hp-hpwire-depth").canonicalize().unwrap();
    let project = root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let mut env = Environment {
        harness_home: Some(root.join("home")),
        cwd: project.clone(),
        ..Default::default()
    };
    env.vars.insert("PATH".into(), "/usr/bin:/bin".into());
    let argv = vec!["env".to_string()];
    let out = execute(
        &env,
        &project,
        &argv,
        &ExecOptions {
            raw: true,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    assert!(
        out.envelope
            .view
            .text
            .contains("SEMAPRAX_HARNESS_BRIDGE_DEPTH=1"),
        "{}",
        out.envelope.view.text
    );
    // A nested host (depth 2 in its own environment) hands its child depth 3.
    env.vars
        .insert("SEMAPRAX_HARNESS_BRIDGE_DEPTH".into(), "2".into());
    let out = execute(
        &env,
        &project,
        &argv,
        &ExecOptions {
            raw: true,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    assert!(out
        .envelope
        .view
        .text
        .contains("SEMAPRAX_HARNESS_BRIDGE_DEPTH=3"));
    // Caller-supplied SEMAPRAX_HARNESS_* extra env stays rejected.
    let mut opts = ExecOptions {
        raw: true,
        ..Default::default()
    };
    opts.extra_env
        .insert("SEMAPRAX_HARNESS_BRIDGE_DEPTH".into(), "0".into());
    assert_eq!(
        execute(&env, &project, &argv, &opts, None)
            .err()
            .unwrap()
            .code,
        "SPX-HPH010"
    );
}

#[path = "workflow_hn.rs"]
mod hn;

#[path = "workflow_wire.rs"]
mod wire;

#[path = "workflow_tc09.rs"]
mod tc09;

#[path = "workflow_tc.rs"]
mod tc;
