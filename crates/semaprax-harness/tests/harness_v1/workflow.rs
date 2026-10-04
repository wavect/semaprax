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
}

impl Fake {
    fn new(src: &str) -> Self {
        Fake {
            preview_source: RefCell::new(src.into()),
            requirements: RefCell::new(REQUIREMENTS.iter().map(|s| s.to_string()).collect()),
            publish_result: RefCell::new(Ok(())),
            publishes: Cell::new(0),
            log: RefCell::default(),
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
        _c: &[u8],
    ) -> semaprax_harness::diag::HarnessResult<CandidatePreview> {
        self.log.borrow_mut().push("preview".into());
        let base = std::fs::read(p.join("src/lib.spx")).unwrap();
        let src = self.preview_source.borrow().clone();
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
        notes: vec![],
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
    let mut always = Task::default();
    always.external_context = ExternalContext::Always;
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
    let mut never = Task::default();
    never.external_context = ExternalContext::Never;
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
    let mut t = Task::default();
    t.external_context = ExternalContext::Always;
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
    let mut task = Task::default();
    task.goal = "use key sk-live-SECRET-123 to fix".into();
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
