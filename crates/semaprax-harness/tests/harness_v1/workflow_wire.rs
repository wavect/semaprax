//! HN-05/HN-10/HN-12/HN-13/HN-16 user-path wiring through `run_with` and the
//! pipeline, no real compiler (real-tool evidence lives in
//! tests/real_tools_v1/{workflow_compiler,graft,graphify}.rs). Fixture prefix `hp-hnwire`.

use super::*;
use semaprax_harness::profile::config::{parse, CONFIG_SCHEMA};
use semaprax_harness::skills::cli_defaults::project_id;
use semaprax_harness::skills::defaults::DefaultSkills;
use semaprax_harness::skills::official::OfficialSet;
use semaprax_harness::updates::cli_updates;
use semaprax_harness::updates::fixture::fake_sha;
use semaprax_harness::updates::state::State;

const CHANGED: &str = "module t.lib;\n@id(\"t.f\")\nfn f(x: i64) -> i64\n    requires x >= 0\n    ensures result == x\n    uses { clock.read }\n{\n    x + 0\n}\n";

fn cfg_text(body: &str) -> String {
    format!("schema = \"{CONFIG_SCHEMA}\"\n{body}")
}

// ---- shared driver: `harness run` in-process (run_with) over the fake compiler ----

struct Wire {
    e: Env,
    home: PathBuf,
    env: Environment,
}

fn wire(src: &str) -> Wire {
    let e = setup(src);
    let home = e.root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = Environment {
        harness_home: Some(home.clone()),
        cwd: e.project.clone(),
        ..Default::default()
    };
    Wire { e, home, env }
}

impl Wire {
    fn opts(&self) -> RunOptions {
        let task = write(
            &self.e.root,
            "host/task.json",
            &json!({"schema": "semaprax.harness-task.v1", "goal": "fix t.f", "task_family": "mechanical"})
                .to_string(),
        );
        let prop = write(
            &self.e.root,
            "host/proposal.json",
            &json!({"schema": "semaprax.harness-proposal.v1",
                    "intent": {"kind": "replace_function_body", "target": "t.f"}})
            .to_string(),
        );
        RunOptions {
            task: Some(task),
            proposal: Some(prop),
            apply_policy: None,
            python: None,
            node: None,
            compiler: None,
            observations: None,
            tokenizer_python: None,
            tokenizer_script: None,
            tokenizer_cache: None,
            tokenizers: vec![],
            cancel: None,
            cancel_file: None,
            frozen: false,
            offline: false,
            updates_fixture: None,
            updates_gh: None,
            updates_now: None,
            disable: false,
            json: true,
        }
    }

    fn run(&self, o: &RunOptions) -> Value {
        let fake = Fake::new(FIXED);
        let snapshot = Snapshot::capture(&self.e.project).unwrap();
        let cache = self.e.cache.clone();
        std::fs::create_dir_all(&cache).unwrap();
        // A fresh lineage per call: results are cached per lineage.
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();
        let out = semaprax_harness::workflow::run_with(o, &self.env, snapshot, cache, &fake)
            .unwrap_or_else(|e| panic!("{} {}", e.code, e.message));
        serde_json::from_str(out.stdout.trim()).unwrap_or_else(|_| panic!("{}", out.stdout))
    }

    fn updates(&self, args: &[&str]) -> semaprax_harness::cli::Outcome {
        let v: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        cli_updates(&v, &self.env)
    }
}

fn skill_entry(v: &Value) -> String {
    v["context"]["skills"]["loaded"][0]
        .as_str()
        .unwrap_or_else(|| panic!("no skill loaded: {v}"))
        .to_string()
}

/// A fixture upstream for the embedded ponytail: the current commit plus a
/// newer release whose SKILL.md carries a marker line.
fn ponytail_upstream(dir: &Path) -> (String, String) {
    let set = OfficialSet::embedded();
    let k = set.find("ponytail").unwrap().clone();
    let (owner_repo, c1) = (
        k.repo.trim_start_matches("https://github.com/").to_string(),
        k.commit.clone().unwrap(),
    );
    // The other curated skills stay at their embedded revision (up to date).
    for other in set
        .skills
        .iter()
        .filter(|o| o.id != "ponytail" && o.embedded)
    {
        let (r, t, c) = (
            other.repo.trim_start_matches("https://github.com/"),
            other.tag.clone().unwrap(),
            other.commit.clone().unwrap(),
        );
        let doc = json!({"releases": [{"tag": t}], "tags": {t.clone(): {"commit": c}},
                         "branches": {"main": c}, "revoked": []});
        write(dir, &format!("{r}/repo.json"), &doc.to_string());
    }
    let c2 = fake_sha("hnwire-ponytail-next");
    let doc = json!({
        "releases": [{"tag": "v4.10.4"}, {"tag": k.tag.clone().unwrap()}],
        "tags": {k.tag.clone().unwrap(): {"commit": c1}, "v4.10.4": {"commit": c2}},
        "branches": {"main": c2}, "revoked": []
    });
    write(dir, &format!("{owner_repo}/repo.json"), &doc.to_string());
    for f in &k.files {
        let bytes = set.file_bytes("ponytail", &f.path).unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        let sub = |c: &str| format!("{owner_repo}/commits/{c}/{}", f.upstream_path);
        write(dir, &sub(&c1), &text);
        let next = if f.path == "SKILL.md" {
            format!("{text}\nHNWIRE-NEWER-REVISION-MARKER\n")
        } else {
            text
        };
        write(dir, &sub(&c2), &next);
    }
    (c1, c2)
}

// ---- HN-05 ----------------------------------------------------------------

#[test]
fn hp_hnwire_next_run_uses_the_activated_skill_revision_pinned_session_keeps_old_and_rollback_restores(
) {
    let w = wire(LIB);
    let up = fixture_dir("hp-hnwire-up");
    let (_c1, c2) = ponytail_upstream(&up);
    let pid = project_id(&Snapshot::capture(&w.e.project).unwrap().root);
    // Before any update: the embedded revision, locked by a pinned session.
    let v0 = w.run(&w.opts());
    let old = skill_entry(&v0);
    assert!(old.starts_with("ponytail@v4.10.3:"), "{old}");
    let old_digest = old.rsplit(':').next().unwrap();
    let old_digest = format!("sha256:{old_digest}");
    let mut pinned = DefaultSkills::new(
        OfficialSet::embedded(),
        Some(w.home.clone()),
        &pid,
        "pinned",
    )
    .unwrap();
    let (rep, _) = pinned.load("ponytail").unwrap();
    assert_eq!(rep.locked_revision.as_deref(), Some(old_digest.as_str()));
    let user = write(&w.home, "skills/user.json", "{\"preset\":\"mine\"}\n");
    let user_before = std::fs::read(&user).unwrap();
    // Approve routine checks + content-only updates once; the upstream moves on.
    assert_eq!(
        w.updates(&["approve-policy", "--auto-content", "--ttl-secs", "0"])
            .code,
        0
    );
    // A run performs the session-start maintenance, activates the newer revision
    // for itself (a new session), and reports it.
    let mut o = w.opts();
    o.updates_fixture = Some(up.clone());
    let v1 = w.run(&o);
    let new = skill_entry(&v1);
    assert!(new.starts_with("ponytail@v4.10.4:"), "{new} / {v1}");
    assert_ne!(new, old);
    assert!(
        v1["notes"].to_string().contains("updated to"),
        "{}",
        v1["notes"]
    );
    // The NEXT run (offline, no fixture) keeps using the activated revision ...
    let mut o = w.opts();
    o.offline = true;
    let v2 = w.run(&o);
    assert_eq!(skill_entry(&v2), new);
    // ... while the pinned session keeps the revision it locked, from the immutable store.
    let mut pinned = DefaultSkills::new(
        semaprax_harness::updates::effective_set(&w.home).unwrap(),
        Some(w.home.clone()),
        &pid,
        "pinned",
    )
    .unwrap();
    let (rep, text) = pinned.load("ponytail").unwrap();
    assert_eq!(rep.locked_revision.as_deref(), Some(old_digest.as_str()));
    assert!(!text.contains("HNWIRE-NEWER-REVISION-MARKER"));
    // Rollback restores the previous revision for new runs; user settings are untouched.
    assert_eq!(w.updates(&["rollback", "ponytail"]).code, 0);
    let mut o = w.opts();
    o.offline = true;
    assert_eq!(skill_entry(&w.run(&o)), old);
    assert_eq!(std::fs::read(&user).unwrap(), user_before);
    let _ = c2;
}

#[test]
fn hp_hnwire_frozen_and_offline_runs_make_no_update_request_and_failures_are_a_notice() {
    let w = wire(LIB);
    let up = fixture_dir("hp-hnwire-up");
    ponytail_upstream(&up);
    assert_eq!(
        w.updates(&["approve-policy", "--auto-content", "--ttl-secs", "0"])
            .code,
        0
    );
    let before = State::load(&w.home).ok();
    for flag in ["frozen", "offline"] {
        let mut o = w.opts();
        o.updates_fixture = Some(up.clone());
        o.frozen = flag == "frozen";
        o.offline = flag == "offline";
        let v = w.run(&o);
        assert!(
            skill_entry(&v).starts_with("ponytail@v4.10.3:"),
            "{flag}: nothing was fetched or activated"
        );
        assert!(!v["notes"].to_string().contains("updated to"), "{flag}");
    }
    let after = State::load(&w.home).ok();
    assert_eq!(
        before.map(|s| s.last_check),
        after.map(|s| s.last_check),
        "no check ran under --frozen/--offline"
    );
    // An unreachable upstream (no such repo in the fixture) is a bounded notice and the
    // run proceeds with the cached embedded revision.
    let empty = fixture_dir("hp-hnwire-empty");
    let mut o = w.opts();
    o.updates_fixture = Some(empty);
    let v = w.run(&o);
    assert!(skill_entry(&v).starts_with("ponytail@v4.10.3:"), "{v}");
    let notes = v["notes"].to_string();
    assert!(notes.contains("update check"), "{notes}");
    assert_ne!(v["status"], "refused", "{v}");
    // Without an approved policy nothing is checked at all (no notice either).
    let w2 = wire(LIB);
    let mut o = w2.opts();
    o.updates_fixture = Some(up);
    assert!(!w2.run(&o)["notes"].to_string().contains("update check"));
}

// ---- HN-10: adapter config plumbing ------------------------------------------

#[test]
fn hp_hnwire_adapter_config_is_parsed_validated_and_forwarded_as_host_env() {
    use semaprax_harness::contract::{CapabilityKind, Descriptor};
    use semaprax_harness::workflow::adapter_config::config_env;
    let c = parse(
        cfg_text(
            "[capability.\"context.repository\"]\nmode = \"auto\"\n[capability.\"context.repository\".config]\nadopt_index = \"read-only\"\nuser_index = \"graft\"\ndeep = false\n",
        )
        .as_bytes(),
    )
    .unwrap();
    let cap = c.capability(CapabilityKind::ContextRepository);
    assert_eq!(cap.config["adopt_index"], "read-only");
    // Order of the two tables does not matter, and the digest sees the config.
    let c2 = parse(
        cfg_text(
            "[capability.\"context.repository\".config]\nadopt_index = \"read-only\"\nuser_index = \"graft\"\ndeep = false\n[capability.\"context.repository\"]\nmode = \"auto\"\n",
        )
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(c.digest(), c2.digest());
    let other = parse(
        cfg_text("[capability.\"context.repository\".config]\nadopt_index = \"copied-snapshot\"\n")
            .as_bytes(),
    )
    .unwrap();
    assert_ne!(other.digest(), c.digest());
    // Validated against the shipped Graft descriptor.
    let desc = Descriptor::parse(
        &std::fs::read(
            repo_root().join("packages/semaprax-harness-adapters/graft/harness-provider.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let env = config_env(&desc, &cap).unwrap();
    assert_eq!(env["SEMAPRAX_HARNESS_CFG_ADOPT_INDEX"], "read-only");
    assert_eq!(env["SEMAPRAX_HARNESS_CFG_USER_INDEX"], "graft");
    assert_eq!(env["SEMAPRAX_HARNESS_CFG_DEEP"], "false");
    // Refusals: unknown field, wrong type, escaping path, secret.
    for (body, why) in [
        ("nope = \"x\"\n", "not declared"),
        ("deep = \"yes\"\n", "must be a bool"),
        ("user_index = \"../other\"\n", "inside the project"),
    ] {
        let c = parse(
            cfg_text(&format!(
                "[capability.\"context.repository\".config]\n{body}"
            ))
            .as_bytes(),
        )
        .unwrap();
        let e = config_env(&desc, &c.capability(CapabilityKind::ContextRepository)).unwrap_err();
        assert!(e.message.contains(why), "{body}: {}", e.message);
    }
    // Absolute, home and secret-looking values never parse (committed configuration stays portable).
    for body in [
        "user_index = \"/etc\"\n",
        "user_index = \"~/x\"\n",
        "user_index = \"sk-abcdefghijklmnopqrstuvwxyz0123456789\"\n",
    ] {
        let e = parse(
            cfg_text(&format!(
                "[capability.\"context.repository\".config]\n{body}"
            ))
            .as_bytes(),
        )
        .unwrap_err();
        assert_eq!(e.code, "SPX-HPB007");
    }
}

// ---- HN-12: check stage tokenizer ----------------------------------------------

#[test]
fn hp_hnwire_cli_run_gives_the_check_stage_a_named_tokenizer() {
    let w = wire(LIB);
    let py = Path::new("/usr/bin/python3");
    if !py.exists() {
        return;
    }
    let stub = write(
        &w.e.root,
        "host/tok.py",
        "import json,sys\nsys.stdout.write(json.dumps({'name':'stub-words','fingerprint':'fp1'})+'\\n');sys.stdout.flush()\nfor line in sys.stdin:\n    t=json.loads(line)['text']\n    sys.stdout.write(json.dumps({'tokens':len(t.split())})+'\\n');sys.stdout.flush()\n",
    );
    let script = write(
        &w.e.project,
        "tools/check.sh",
        "#!/bin/sh\necho 'one two three four'\nexit 0\n",
    );
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    write(
        &w.e.project,
        "semaprax.harness.toml",
        &cfg_text("[workflow.check.unit]\nargv = [\"tools/check.sh\"]\n"),
    );
    let mut o = w.opts();
    o.tokenizer_python = Some(py.to_path_buf());
    o.tokenizer_script = Some(stub);
    o.tokenizers = vec!["stub-words".into()];
    let v = w.run(&o);
    let s = v["checks"].to_string();
    assert!(
        s.contains("stub-words"),
        "named-token measurement reached the report: {s}"
    );
    // Without tokenizer flags the same run reports no named tokenizer.
    let v = w.run(&w.opts());
    assert!(!v["checks"].to_string().contains("stub-words"));
}

// ---- HN-13: failed-candidate follow-up and continuation handles -------------------

use semaprax_harness::workflow::broker_stage::COMPILER_VERIFIED;

struct Script {
    items: Vec<Value>,
    prompts: RefCell<Vec<Value>>,
    calls: Cell<u32>,
}
struct ScriptRef<'a>(&'a Script);
impl ProposalStage for ScriptRef<'_> {
    fn id(&self) -> String {
        "org.example/script".into()
    }
    fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        let i = self.0.calls.get() as usize;
        self.0.calls.set(i as u32 + 1);
        self.0.prompts.borrow_mut().push(r.prompt.clone());
        Ok(self.0.items[i.min(self.0.items.len() - 1)]
            .to_string()
            .into_bytes())
    }
    fn calls(&self) -> u32 {
        self.0.calls.get()
    }
    fn side_effecting(&self) -> bool {
        false
    }
}

fn body(extra: Value) -> Value {
    let mut i = json!({"kind": "replace_function_body", "target": "t.f"});
    for (k, v) in extra.as_object().unwrap() {
        i[k] = v.clone();
    }
    json!({"schema": "semaprax.harness-proposal.v1", "intent": i})
}

/// A plan-capable repository stage (the shape of the broker) with counters.
#[derive(Default)]
struct PlanStage {
    collects: u32,
    follow_ups: u32,
    expands: u32,
    /// Material the first follow-up returns (`None` makes it find nothing new).
    follow_up_text: Option<&'static str>,
    handle: Option<String>,
}

fn packet(id: &str, label: &str, prov: &str, text: &str) -> ContextPacket {
    ContextPacket {
        provider: id.into(),
        items: vec![ContextItem {
            label: label.into(),
            provenance: prov.into(),
            text: text.into(),
        }],
        complete: true,
    }
}

impl ContextStage for PlanStage {
    fn id(&self) -> String {
        "org.example/plan-stage".into()
    }
    fn plans(&self) -> bool {
        true
    }
    fn collect(&mut self, r: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        self.collects += 1;
        Ok(if r.external == ExternalContext::Never {
            packet(&self.id(), "native:t.f", COMPILER_VERIFIED, "native facts")
        } else {
            packet(
                &self.id(),
                "web/app.ts:1-3",
                "external:structural",
                "initial external",
            )
        })
    }
    fn calls(&self) -> u32 {
        self.collects + self.follow_ups
    }
    fn follow_up(
        &mut self,
        _r: &ContextRequest,
        _f: &str,
    ) -> Result<Option<ContextPacket>, StageFailure> {
        self.follow_ups += 1;
        Ok(match (self.follow_ups, self.follow_up_text) {
            (1, Some(t)) => Some(packet(
                &self.id(),
                "web/api.ts:5-9",
                "external:structural",
                t,
            )),
            _ => None,
        })
    }
    fn expand(&mut self, _r: &ContextRequest, _h: &str) -> Result<ContextPacket, StageFailure> {
        self.expands += 1;
        Ok(packet(
            &self.id(),
            "src/a.ts:1-2",
            "external:inferred",
            "EXPANDED-SLICE",
        ))
    }
    fn take_plan_report(&mut self) -> Option<Value> {
        self.handle.as_ref().map(|h| json!({"continuation": [h]}))
    }
}

fn session_cfg(e: &Env) -> RunConfig {
    config(
        e,
        Task {
            schema_version: 2,
            mode: TaskMode::Change,
            goal: "rename f and keep behavior".into(),
            seed: Some("t.f".into()),
            external_context: ExternalContext::Always,
            session: Some(SessionBounds {
                max_attempts: 3,
                ..Default::default()
            }),
            ..Task::default()
        },
        None,
    )
}

fn drive_plan(cfg: &RunConfig, stage: &mut PlanStage, script: &Script) -> Report {
    let fake = Fake::new(CHANGED);
    let mut prop = ScriptRef(script);
    let mut view = RawCommandView;
    run(
        cfg,
        &fake,
        Stages {
            decision: None,
            native: stage,
            external: None,
            proposer: &mut prop,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

#[test]
fn hp_hnwire_a_failed_first_candidate_triggers_one_focused_follow_up_that_enables_the_second_attempt(
) {
    let e = setup(FIXED);
    let script = Script {
        items: vec![
            body(json!({"fake_refuse": "SPX-G225 unknown field on `Gadget`"})),
            body(json!({})),
        ],
        prompts: RefCell::default(),
        calls: Cell::new(0),
    };
    let mut stage = PlanStage {
        follow_up_text: Some("FOLLOWUP-FACT about Gadget"),
        ..Default::default()
    };
    let r = drive_plan(&session_cfg(&e), &mut stage, &script);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(stage.follow_ups, 1, "exactly one focused follow-up query");
    let p = script.prompts.borrow();
    assert!(!p[0].to_string().contains("FOLLOWUP-FACT"));
    assert!(
        p[1].to_string().contains("FOLLOWUP-FACT"),
        "the follow-up result reached attempt 2"
    );
    assert_eq!(r.context["plan"]["follow_up"]["added_items"], 1);
    assert!(r
        .steps
        .iter()
        .any(|(k, v)| k == "context-follow-up" && v == "added"));
    // The native facts were requested without a provider call; the provider slot was the same stage.
    assert!(stage.collects >= 2);
}

#[test]
fn hp_hnwire_a_failure_naming_a_handle_expands_it_without_a_provider_call() {
    let e = setup(FIXED);
    let script = Script {
        items: vec![
            body(json!({"fake_refuse": "SPX-G225 mismatch in src/a.ts"})),
            body(json!({})),
        ],
        prompts: RefCell::default(),
        calls: Cell::new(0),
    };
    let mut stage = PlanStage {
        follow_up_text: None,
        handle: Some("ctx:org.example/plan-stage:src/a.ts#1-2@sha256:00".into()),
        ..Default::default()
    };
    let r = drive_plan(&session_cfg(&e), &mut stage, &script);
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert_eq!(stage.expands, 1);
    assert!(script.prompts.borrow()[1]
        .to_string()
        .contains("EXPANDED-SLICE"));
    assert_eq!(
        r.context["plan"]["expanded_handles"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn hp_hnwire_a_stage_without_plans_makes_no_follow_up_call() {
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let script = Script {
        items: vec![body(json!({"fake_refuse": "SPX-G225 x"})), body(json!({}))],
        prompts: RefCell::default(),
        calls: Cell::new(0),
    };
    let mut native = NativeContext::new(&fake);
    let mut prop = ScriptRef(&script);
    let mut view = RawCommandView;
    let r = run(
        &session_cfg(&e),
        &fake,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut prop,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    );
    assert_eq!(r.status, "candidate-ready", "{:?}", r.refusals);
    assert!(r.context["plan"].get("follow_up").is_none());
}

// ---- HN-16: [routing] + evidence registry through `harness run` -------------------------

use semaprax_harness::contract::RequestEnvelope;
use semaprax_harness::decision::{
    DecisionCall, DecisionInvoker, Destination, EnablementGate, EvidenceKey, ModelPlan,
    ProviderMode, ProviderProfile,
};
use semaprax_harness::profile::config::RoutingSection;
use semaprax_harness::workflow::routing::{RoutingWiring, EVIDENCE_FILE, EVIDENCE_SCHEMA};

fn plan_of(id: &str, rank: u32, remote: bool) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: if remote {
            Destination::Remote {
                origin: "https://api.example".into(),
            }
        } else {
            Destination::Local
        },
        structured_output: true,
        tools: false,
        max_context: 1_000_000,
        est_cost_micros: 0,
        est_latency_ms: 10,
        strength_rank: rank,
    }
}

fn models(list: &[ModelPlan]) -> Value {
    json!(list.iter().map(ModelPlan::to_json).collect::<Vec<_>>())
}

impl Wire {
    fn routed(&self, routing: &str, catalog: &[ModelPlan]) -> Value {
        write(&self.e.project, "semaprax.harness.toml", &cfg_text(routing));
        // A changed configuration needs a fresh resolution (the lock binds its digest).
        let _ = std::fs::remove_file(self.e.project.join("semaprax.harness.lock"));
        let mut o = self.opts();
        let task = write(
            &self.e.root,
            "host/task-models.json",
            &json!({"schema": "semaprax.harness-task.v1", "goal": "fix t.f", "models": models(catalog)}).to_string(),
        );
        o.task = Some(task);
        self.run(&o)
    }
}

#[test]
fn hp_hnwire_project_pin_is_honored_in_every_mode_and_never_falls_back() {
    let cat = [plan_of("cheap", 1, false), plan_of("strong", 2, false)];
    let w = wire(LIB);
    let base = w.routed("", &cat);
    assert_eq!(base["route"]["mode"], "rules");
    assert_eq!(
        base["route"]["choice"], "cheap",
        "rules pick the cheap model"
    );
    for mode in [
        "",
        "mode = \"rules\"\n",
        "mode = \"pin\"\n",
        "mode = \"experimental\"\n",
        "mode = \"auto\"\n",
    ] {
        let v = w.routed(&format!("[routing]\n{mode}pin = \"strong\"\n"), &cat);
        assert_eq!(v["route"]["choice"], "strong", "{mode}: {}", v["route"]);
        assert_eq!(v["route"]["mode"], "pin", "{mode}");
        assert_eq!(v["route"]["policy"]["project_pin"], "strong");
        assert_eq!(v["route"]["explanation"]["rules_reason"], "pinned");
    }
    // A pin the policy cannot admit refuses; it never falls back to another model.
    let v = w.routed("[routing]\npin = \"ghost\"\n", &cat);
    assert_eq!(v["status"], "refused", "{v}");
    assert_eq!(v["refusals"][0]["code"], "SPX-HPJ016");
    // `mode = "pin"` without a pin is a configuration error with a stable code.
    let e = parse(cfg_text("[routing]\nmode = \"pin\"\n").as_bytes()).unwrap_err();
    assert_eq!(e.code, "SPX-HPB004");
    let e = parse(cfg_text("[routing]\nmode = \"turbo\"\n").as_bytes()).unwrap_err();
    assert_eq!(e.code, "SPX-HPB004");
}

#[test]
fn hp_hnwire_allow_remote_false_strips_remote_models_in_every_mode() {
    let mut local = plan_of("local", 1, false);
    local.est_cost_micros = 5000;
    let cat = [local, plan_of("cloud", 3, true)];
    let w = wire(LIB);
    let open = w.routed("[routing]\nallow_remote = true\n", &cat);
    assert_eq!(
        open["route"]["choice"], "cloud",
        "the cheapest admissible model wins when remote is allowed"
    );
    let default = w.routed("", &cat);
    assert_eq!(
        default["route"]["choice"], "local",
        "remote stays unapproved by default"
    );
    for mode in ["", "mode = \"auto\"\n", "mode = \"experimental\"\n"] {
        let v = w.routed(&format!("[routing]\n{mode}allow_remote = false\n"), &cat);
        assert_eq!(v["route"]["choice"], "local", "{mode}: {}", v["route"]);
        assert_eq!(v["route"]["policy"]["allow_remote"], false);
    }
    // A pin of the remote model works when remote is approved and refuses under the prohibition.
    let v = w.routed("[routing]\npin = \"cloud\"\nallow_remote = true\n", &cat);
    assert_eq!(
        (v["route"]["choice"].as_str(), v["route"]["mode"].as_str()),
        (Some("cloud"), Some("pin")),
        "{v}"
    );
    let v = w.routed("[routing]\npin = \"cloud\"\nallow_remote = false\n", &cat);
    assert_eq!(v["refusals"][0]["code"], "SPX-HPJ016", "{v}");
}

struct Pick(u32, &'static str);
impl DecisionInvoker for Pick {
    fn evaluate(&mut self, _r: &RequestEnvelope) -> DecisionCall {
        self.0 += 1;
        DecisionCall::Answered {
            result: json!({"choice": self.1, "scores": {self.1: 0.9}, "abstain": false}),
            elapsed_ms: 1,
        }
    }
}

fn profile() -> ProviderProfile {
    ProviderProfile {
        provider_id: "org.example/threshold-route".into(),
        model_id: "m".into(),
        checkpoint: "1".into(),
        min_confidence: None,
        max_context_tokens: None,
        supported_families: None,
    }
}

fn evidence_doc(cat: &[ModelPlan], origin: &str) -> String {
    let digest = semaprax_harness::json::digest(
        "semaprax.decision.catalog.v1",
        &Value::Array(cat.iter().map(ModelPlan::to_json).collect()),
    );
    let key = EvidenceKey::live(&profile(), &digest).to_json();
    let items: Vec<String> = (0..30).map(|i| format!("item-{i}")).collect();
    let mut outcomes = Vec::new();
    for it in &items {
        for (arm, cost) in [("rules", 1000u64), ("org.example/threshold-route", 500)] {
            outcomes.push(
                json!({"item": it, "arm": arm, "model": "cheap", "origin": origin,
                "verified_by": "tests", "completed": true, "regressions": 0, "attempts": 1,
                "cost_micros": cost, "latency_ms": 100, "router_cost_micros": 0,
                "context_cost_micros": 0, "retry_owner": "host"}),
            );
        }
    }
    json!({"schema": EVIDENCE_SCHEMA, "records": [{"key": key,
        "budget": {"max_cost_micros": 100000, "max_attempts": 3},
        "eval_items": items, "trained_on": [], "outcomes": outcomes}]})
    .to_string()
}

/// `run` over the fake compiler with an external decision provider and the
/// project's `[routing]` + machine-local evidence wired like the CLI does.
fn auto_run(w: &Wire, cat: &[ModelPlan], section: &RoutingSection, inv: &mut Pick) -> Report {
    let fake = Fake::new(FIXED);
    let task = Task {
        models: Some(models(cat)),
        ..Task::default()
    };
    let mut cfg = config(&w.e, task, None);
    cfg.routing = RoutingWiring::from_config(section, Some(&w.home)).unwrap();
    let mut native = NativeContext::new(&fake);
    let mut p = ScriptedProposer::from_bytes(proposal("replace_function_body"));
    let mut view = RawCommandView;
    let profile = profile();
    let gate = EnablementGate::not_evaluated("model-route/v1", &profile.provider_id);
    run(
        &cfg,
        &fake,
        Stages {
            decision: Some(DecisionStage {
                invoker: inv,
                profile,
                mode: ProviderMode::Auto,
                gate,
            }),
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut Observer::new(None, ObserverLimits::default()),
    )
}

#[test]
fn hp_hnwire_fixture_evidence_cannot_unlock_auto_but_real_matched_evidence_can_and_drift_falls_back(
) {
    let cat = [plan_of("cheap", 1, false), plan_of("strong", 2, false)];
    let auto = RoutingSection {
        mode: "auto".into(),
        pin: None,
        allow_remote: None,
        explicit: true,
    };
    let w = wire(LIB);
    // No registry: rules decide, the router is never consulted, and the report says why.
    let mut inv = Pick(0, "strong");
    let r = auto_run(&w, &cat, &auto, &mut inv);
    assert_eq!(inv.0, 0);
    assert!(
        r.route["rules_reason"]
            .as_str()
            .unwrap()
            .contains("no evidence registry"),
        "{}",
        r.route
    );
    // Fixture-origin evidence: only real verified runs unlock auto.
    write(&w.home, EVIDENCE_FILE, &evidence_doc(&cat, "fixture"));
    let mut inv = Pick(0, "strong");
    let r = auto_run(&w, &cat, &auto, &mut inv);
    assert_eq!(inv.0, 0, "a fixture cannot unlock auto mode");
    let why = r.route["rules_reason"].as_str().unwrap();
    assert!(why.contains("fixture or unavailable cells"), "{why}");
    assert_eq!(r.route["choice"], "cheap");
    // Real, matched, held-out evidence for the live key: the learned route is consulted and the
    // report names the evidence that applied.
    write(&w.home, EVIDENCE_FILE, &evidence_doc(&cat, "real"));
    let mut inv = Pick(0, "strong");
    let r = auto_run(&w, &cat, &auto, &mut inv);
    assert_eq!(inv.0, 1, "{}", r.route);
    assert_eq!(r.route["mode"], "qualified-auto");
    assert_eq!(r.route["choice"], "strong");
    let ev = &r.route["explanation"]["evidence"];
    assert!(
        ev["record_digest"].as_str().unwrap().starts_with("sha256:"),
        "{ev}"
    );
    assert_eq!(ev["key"]["provider"], "org.example/threshold-route");
    // A changed catalog is a different key: the evidence no longer applies and rules decide.
    let other = [
        plan_of("cheap", 1, false),
        plan_of("strong", 2, false),
        plan_of("extra", 3, false),
    ];
    let mut inv = Pick(0, "strong");
    let r = auto_run(&w, &other, &auto, &mut inv);
    assert_eq!(inv.0, 0);
    assert!(
        r.route["rules_reason"]
            .as_str()
            .unwrap()
            .contains("no qualified profile"),
        "{}",
        r.route
    );
    // Experimental mode consults the provider without evidence, visibly experimental.
    let exp = RoutingSection {
        mode: "experimental".into(),
        ..auto.clone()
    };
    let mut inv = Pick(0, "strong");
    let r = auto_run(&w, &cat, &exp, &mut inv);
    assert_eq!(
        (inv.0, r.route["status"].as_str()),
        (1, Some("experimental"))
    );
    // A malformed evidence file is a refusal-grade error, not silently ignored.
    write(&w.home, EVIDENCE_FILE, "{not json");
    assert!(RoutingWiring::from_config(&auto, Some(&w.home)).is_err());
}
