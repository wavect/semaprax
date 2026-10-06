//! MR-14: routing setup and explainability on the existing CLI paths.
//!
//! - A network-service upstream (laya-local and the other SystemOne-style
//!   decision adapters) adopts and trusts directly from the checkout; an
//!   executable upstream still needs `adopt --upstream` (`SPX-HPB033`).
//! - `status --routing` lists every ready-to-review profile with zero
//!   inference calls; `--check` is non-billable and actionable; `--probe` is
//!   announced, needs `--yes` and is metered.
//! - The bridge handshake states who controlled the parent model.
//! - An out-of-tree copy of the MR-15 starter is adopted and decides through
//!   the normal harness decision path, unchanged routing code.
//!
//! Needs python3 (HARNESS_PYTHON or PATH). Fixture prefix `hp-mr14`.

use crate::support::{fixture_dir, repo_root, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::decision::*;
use semaprax_harness::profile::{grant_for, LocalState};
use semaprax_harness::workflow::decision_open::open_decision;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const LAYA: &str = "ai.convai/laya-decision";
const SENTINEL: &str = "mr14-secret-sentinel-value-0123456789";

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn python() -> PathBuf {
    std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join("python3"))
                .find(|p| p.is_file())
        })
        .expect("python3 not found: the MR-14 cells cannot run here")
}

struct World {
    root: PathBuf,
    home: PathBuf,
    env: Environment,
}

fn world(tag: &str) -> World {
    let root = fixture_dir(&format!("hp-mr14-{tag}"))
        .canonicalize()
        .unwrap();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = Environment {
        harness_home: Some(home.clone()),
        compiler: None,
        cwd: root.clone(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    World { root, home, env }
}

fn laya_descriptor() -> PathBuf {
    repo_root()
        .join("packages/semaprax-harness-adapters/systemone/laya-local/harness-provider.json")
}

/// The bundled laya-local adapter adopted and trusted straight from the checkout.
fn adopt_laya(w: &World) -> semaprax_harness::cli::Outcome {
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            laya_descriptor().to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
        ]),
        &w.env,
    );
    assert_eq!(o.code, 0, "adopt: {}", o.stderr);
    o
}

fn project(w: &World, name: &str, provider: &str, cfg: &str) -> PathBuf {
    let p = w.root.join(name);
    write(&p, "semaprax.toml", "schema = \"semaprax.manifest.v1\"\n");
    write(&p, "src/lib.spx", "module app\n");
    write(
        &p,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nmode = \"auto\"\nprovider = \"{provider}\"\n[capability.\"decision.evaluate\".config]\n{cfg}"),
    );
    p
}

/// The Python fake Laya server (`systemone/tests/serve_fake.py`).
struct FakeLaya {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    port: u16,
}

impl FakeLaya {
    fn start(mode: &str) -> Self {
        let script =
            repo_root().join("packages/semaprax-harness-adapters/systemone/tests/serve_fake.py");
        let mut child = Command::new(python())
            .args([script.to_str().unwrap(), "laya", mode])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fake laya");
        let stdin = child.stdin.take().unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        let port = line.trim().parse().expect("fake server port");
        Self {
            child,
            stdin,
            stdout,
            port,
        }
    }

    /// Inference POSTs the fake has answered.
    fn posts(&mut self) -> u64 {
        writeln!(self.stdin, "posts").unwrap();
        self.stdin.flush().unwrap();
        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        line.trim().parse().unwrap()
    }
}

impl Drop for FakeLaya {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn laya_profile() -> String {
    json!({"profile_id": "laya-multilingual", "model": "multilingual", "checkpoint": "multilingual",
        "identity_kind": "local_declared", "score_kind": "option_distribution", "scoreless": false,
        "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"]})
    .to_string()
}

fn laya_project(w: &World, name: &str, port: u16) -> PathBuf {
    project(
        w,
        name,
        LAYA,
        &format!("model_profile = '{}'\ninstance_id = \"{name}\"\nendpoint = \"http://127.0.0.1:{port}\"\n", laya_profile()),
    )
}

fn codes(doc: &Value) -> Vec<String> {
    doc["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["code"].as_str().unwrap().to_string())
        .collect()
}

fn check(w: &World, proj: &Path, id: &str, task: Option<&str>) -> (i32, Value) {
    let mut a = s(&[
        "status",
        "--routing",
        "--check",
        id,
        "--json",
        "--project",
        proj.to_str().unwrap(),
    ]);
    if let Some(t) = task {
        a.extend(s(&["--task", t]));
    }
    let o = run(&a, &w.env);
    assert!(o.stderr.is_empty(), "{}", o.stderr);
    (o.code, serde_json::from_str(&o.stdout).unwrap())
}

#[test]
fn mr14_service_upstream_adopts_and_trusts_directly_from_the_checkout() {
    let w = world("svc");
    let o = adopt_laya(&w);
    assert!(o.stdout.contains("network service"), "{}", o.stdout);
    let o = run(&s(&["trust", LAYA]), &w.env);
    assert_eq!(o.code, 0, "trust: {}", o.stderr);
    let state = LocalState::load(&w.env).unwrap();
    let inst = &state.installations[LAYA];
    assert!(
        inst.upstream.is_none(),
        "no executable is adopted for a service"
    );
    let insp = inst.inspect().unwrap();
    assert!(!insp.current.requires_upstream);
    let g = grant_for(&state, LAYA, &insp.current).unwrap();
    assert_eq!(g.provider_id(), LAYA);
    // An executable path for a service upstream is refused.
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            laya_descriptor().to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
            "--upstream",
            "/bin/sh",
        ]),
        &w.env,
    );
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB021") && o.stderr.contains("network service"),
        "{}",
        o.stderr
    );
}

#[test]
fn mr14_executable_upstream_still_requires_an_adopted_executable() {
    let w = world("exe");
    let platform = semaprax_harness::profile::resolve::current_platform();
    let desc = json!({
        "schema": "semaprax.harness-provider.v1",
        "provider": {"id": "org.example/exe-tool", "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "upstream": {"name": "tool", "package": "npm:tool", "repository": "https://example.invalid/tool",
                     "versions": ["1.0.0"], "identity_probe": ["--version"]},
        "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
        "capabilities": [{"kind": "decision.evaluate", "version": 1, "required": true, "operations": ["evaluate"]}],
        "extensions": [], "platforms": [platform],
        "permissions": {"read": [], "write": [], "network": ["loopback:user-selected-endpoint"], "process": ["upstream"], "secrets": []},
        "resources": {"handshake_timeout_ms": 5000, "invoke_timeout_ms": 30000, "max_frame_bytes": 1048576,
                      "max_concurrency": 1, "idle_shutdown_ms": 60000},
        "cancellation": "cooperative",
        "support": {"license": "MIT", "isolation": "subprocess", "tested": []}
    });
    let d = write(&w.root, "exe/harness-provider.json", &desc.to_string());
    write(&w.root, "exe/adapter.py", "print('x')\n");
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            d.to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
        ]),
        &w.env,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o.stdout.contains("--upstream"), "{}", o.stdout);
    let o = run(&s(&["trust", "org.example/exe-tool"]), &w.env);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPB033"), "{}", o.stderr);
}

#[test]
fn mr14_status_routing_lists_reviewable_profiles_without_inference_or_secret_values() {
    let mut w = world("list");
    w.env
        .vars
        .insert("SEMAPRAX_HARNESS_SECRET_JEV".into(), SENTINEL.into());
    let o = run(&s(&["status", "--routing", "--json"]), &w.env);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(!o.stdout.contains(SENTINEL));
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["inference_calls"], 0);
    let ps = v["profiles"].as_array().unwrap();
    let ids: Vec<&str> = ps.iter().map(|p| p["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        [
            "rules",
            "jev",
            "laya",
            "minijev",
            "clef-hosted",
            "clef-flash-hosted",
            "clef-local"
        ]
    );
    let by = |id: &str| ps.iter().find(|p| p["id"] == id).unwrap().clone();
    assert_eq!(by("rules")["readiness"], "ready");
    let jev = by("jev");
    assert_eq!(
        jev["secrets"],
        json!([{"name": "SEMAPRAX_HARNESS_SECRET_JEV", "present": true}])
    );
    assert_eq!(jev["endpoint_owner"], "vendor-hosted");
    assert_eq!(by("clef-hosted")["model"], "@cf/cloudflare/clef");
    assert_eq!(
        by("clef-flash-hosted")["model"],
        "@cf/cloudflare/clef-flash"
    );
    assert_eq!(by("clef-hosted")["secrets"][0]["present"], false);
    assert_eq!(
        by("clef-local")["readiness"],
        "unavailable unless provisioned"
    );
    assert_eq!(
        by("laya")["tasks"],
        json!(["model-route/v1", "model-route/v2", "choice-select/v1"])
    );
    assert_eq!(
        by("minijev")["tasks"],
        json!(["model-route/v2", "choice-select/v1"])
    );
    for p in ps.iter().skip(1) {
        assert_eq!(p["adopted"], false);
        assert!(
            p["qualification"]
                .as_str()
                .unwrap()
                .starts_with("not-evaluated"),
            "{p}"
        );
    }
    // Human view: same rows, still no secret value.
    let o = run(&s(&["status", "--routing"]), &w.env);
    assert_eq!(o.code, 0);
    assert!(
        o.stdout.contains("clef-local") && o.stdout.contains("SEMAPRAX_HARNESS_SECRET_JEV=set")
    );
    assert!(!o.stdout.contains(SENTINEL));
    // Routing flags apply only with --routing.
    assert_eq!(run(&s(&["status", "--check", "laya"]), &w.env).code, 2);
}

#[test]
fn mr14_check_is_non_billable_and_names_each_gap() {
    let mut fake = FakeLaya::start("ok");
    let w = world("check");
    let empty = w.root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    // Not adopted, no endpoint.
    let (code, v) = check(&w, &empty, "laya", None);
    assert_eq!(code, 1);
    assert_eq!(codes(&v), ["SPX-HPB076", "SPX-HPB071"]);
    assert_eq!(
        (v["inference_calls"].clone(), v["billable"].clone()),
        (json!(0), json!(false))
    );
    // Missing key (hosted Jev).
    let (_, v) = check(&w, &empty, "jev", None);
    assert!(codes(&v).contains(&"SPX-HPB070".to_string()));
    assert!(v["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("export it"));
    // Unsupported task/version and an unknown profile.
    let (_, v) = check(&w, &empty, "minijev", Some("model-route/v1"));
    assert_eq!(codes(&v)[0], "SPX-HPB072");
    let o = run(&s(&["status", "--routing", "--check", "nope"]), &w.env);
    assert!(o.stderr.contains("SPX-HPB077"));

    adopt_laya(&w);
    assert_eq!(run(&s(&["trust", LAYA]), &w.env).code, 0);
    // Configured endpoint with nothing listening: unavailable worker.
    let dead = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_port = dead.local_addr().unwrap().port();
    drop(dead);
    let p = laya_project(&w, "proj-dead", dead_port);
    let (_, v) = check(&w, &p, "laya", None);
    assert_eq!(codes(&v), ["SPX-HPB071"]);
    assert!(v["findings"][0]["message"]
        .as_str()
        .unwrap()
        .contains("not accepting connections"));
    // A live fake server: ready, and still zero inference requests reached it.
    let p = laya_project(&w, "proj-live", fake.port);
    let (code, v) = check(&w, &p, "laya", Some("choice-select/v1"));
    assert_eq!((code, v["readiness"].as_str()), (0, Some("ready")), "{v}");
    assert_eq!(fake.posts(), 0, "the check made no inference call");
    // Stale evidence: recorded for another checkpoint.
    write(
        &w.home,
        "routing/evidence.json",
        &json!({"schema": "semaprax.harness-routing-evidence.v1", "records": [{
            "key": {"task": "model-route/v2", "provider": LAYA, "weights": "old-checkpoint", "catalog": "c",
                    "normalization": "n", "distribution": "d"},
            "budget": {"max_cost_micros": 1, "max_attempts": 1}, "eval_items": [], "trained_on": [], "outcomes": []}]})
        .to_string(),
    );
    let (_, v) = check(&w, &p, "laya", None);
    assert_eq!(codes(&v), ["SPX-HPB073"]);
    assert!(v["qualification"].as_str().unwrap().starts_with("stale"));
    assert_eq!(fake.posts(), 0);
}

#[test]
fn mr14_probe_is_announced_confirmed_and_metered_and_abstention_is_explained() {
    let w = world("probe");
    adopt_laya(&w);
    assert_eq!(run(&s(&["trust", LAYA]), &w.env).code, 0);
    let py = python();
    let mut fake = FakeLaya::start("ok");
    let p = laya_project(&w, "proj", fake.port);
    let base = s(&[
        "status",
        "--routing",
        "--probe",
        "laya",
        "--project",
        p.to_str().unwrap(),
        "--python",
        py.to_str().unwrap(),
    ]);
    // Unconfirmed: the notice, no call.
    let o = run(&base, &w.env);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB075") && o.stderr.contains("may incur a billable provider call"),
        "{}",
        o.stderr
    );
    assert_eq!(fake.posts(), 0);
    // Confirmed: exactly one metered call.
    let mut yes = base.clone();
    yes.push("--yes".into());
    let o = run(&yes, &w.env);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert!(o
        .stdout
        .starts_with("probe laya: this sends ONE decision request"));
    assert_eq!(fake.posts(), 1);
    let log = std::fs::read_to_string(w.home.join("routing/probes.jsonl")).unwrap();
    let rec: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    assert_eq!(
        (rec["router_calls"].clone(), rec["billable"].clone()),
        (json!(1), json!(true))
    );
    assert_eq!(rec["provider_id"], LAYA);
    drop(fake);
    // An abstaining provider: explained, rules decide, still metered.
    let fake = FakeLaya::start("abstain");
    let p = laya_project(&w, "proj-abstain", fake.port);
    let a = s(&[
        "status",
        "--routing",
        "--probe",
        "laya",
        "--yes",
        "--project",
        p.to_str().unwrap(),
        "--python",
        py.to_str().unwrap(),
    ]);
    let o = run(&a, &w.env);
    assert_eq!(o.code, 1);
    assert!(
        o.stderr.contains("SPX-HPB074") && o.stderr.contains("rules decide"),
        "{}",
        o.stderr
    );
    assert_eq!(
        std::fs::read_to_string(w.home.join("routing/probes.jsonl"))
            .unwrap()
            .lines()
            .count(),
        2
    );
}

#[test]
fn mr14_final_context_mismatch_is_actionable() {
    let plan = ModelPlan {
        id: "m-small".into(),
        destination: Destination::Local,
        structured_output: true,
        tools: true,
        max_context: 4_000,
        est_cost_micros: 1,
        est_latency_ms: 1,
        strength_rank: 1,
        descriptor: Default::default(),
    };
    let f = TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 100,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let b = Budget {
        max_cost_micros: 10,
        max_latency_ms: 10,
        max_router_calls: 1,
    };
    let i = RouteInputs {
        request: RouteRequest::new(f, vec![plan], b).unwrap(),
        policy: RoutePolicy::default(),
    };
    assert!(recheck_dispatch(&i, "m-small", 1_000).is_ok());
    let e = recheck_dispatch(&i, "m-small", 9_000).unwrap_err();
    assert_eq!(e.code, "SPX-HPJ018");
    assert!(
        e.message.contains("final context 9000 tokens") && e.message.contains("pin a model"),
        "{}",
        e.message
    );
}

#[test]
fn mr14_bridge_handshake_reports_who_controlled_the_parent_model() {
    use semaprax_harness::bridge::negotiate::PROTOCOL;
    use semaprax_harness::bridge::rpc::Server;
    let w = world("bridge");
    let hello = |caps: Value| json!({"protocol": PROTOCOL, "version": 1, "host": {"name": "claude-code", "version": "2.1.289"}, "capabilities": caps, "command_rewriter": null});
    let mut srv = Server::new(&w.env, &w.root);
    let r = srv.handle("bridge/handshake", &hello(json!({}))).unwrap();
    let pm = &r["parent_model"];
    assert_eq!(pm["mode"], "host-controlled");
    assert_eq!(pm["semaprax_controlled_parent_model"], false);
    assert_eq!(pm["changes_parent_model"], false);
    assert!(pm["statement"]
        .as_str()
        .unwrap()
        .contains("Semaprax did not control the parent model"));
    let mut srv = Server::new(&w.env, &w.root);
    let r = srv
        .handle("bridge/handshake", &hello(json!({"model_routing": true})))
        .unwrap();
    assert_eq!(r["parent_model"]["mode"], "delegated");
    assert_eq!(r["parent_model"]["changes_parent_model"], false);
    // The static Claude Code profile says the same.
    let o = run(
        &s(&["bridge", w.root.to_str().unwrap(), "--host", "claude-code"]),
        &w.env,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["parent_model"]["semaprax_controlled_parent_model"], false);
}

/// Copy the MR-15 starter out of tree under a fresh provider id.
fn starter_copy(root: &Path, id: &str) -> PathBuf {
    let pkg = repo_root().join("packages/semaprax-harness-adapters");
    let to = root.join("vendor");
    let dir = to.join("examples/oot-starter");
    std::fs::create_dir_all(&dir).unwrap();
    for f in ["adapter.py", "harness-provider.json"] {
        let text =
            std::fs::read_to_string(pkg.join("examples/decision-adapter-starter").join(f)).unwrap();
        std::fs::write(
            dir.join(f),
            text.replace("org.example/keyword-decision", id),
        )
        .unwrap();
    }
    let sdk = to.join("sdk/python");
    std::fs::create_dir_all(&sdk).unwrap();
    for e in std::fs::read_dir(pkg.join("sdk/python")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "py") {
            std::fs::copy(e.path(), sdk.join(e.file_name())).unwrap();
        }
    }
    dir.join("harness-provider.json")
}

#[test]
fn mr14_out_of_tree_starter_copy_decides_through_the_normal_harness_path() {
    const OOT: &str = "org.example/oot-starter";
    let w = world("oot");
    let d = starter_copy(&w.root, OOT);
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            d.to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
        ]),
        &w.env,
    );
    assert_eq!(o.code, 0, "adopt: {}", o.stderr);
    assert_eq!(run(&s(&["trust", OOT]), &w.env).code, 0);
    let p = project(&w, "proj", OOT, "");
    let mut o = open_decision(&w.env, &p, Some(&py), &w.root.join("cache")).unwrap();
    assert_eq!(o.profile.provider_id, OOT);
    let mut ctx = RouteContext {
        project: o.binding.clone(),
        lock_digest: o.lock_digest.clone(),
        invocation_id: "oot".into(),
        lineage_id: "oot".into(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    // model-route: the starter's keyword scorer prefers the frontier plan.
    let plan = |id: &str, tier| ModelPlan {
        id: id.into(),
        destination: Destination::Local,
        structured_output: true,
        tools: true,
        max_context: 32_000,
        est_cost_micros: 10,
        est_latency_ms: 10,
        strength_rank: 1,
        descriptor: PlanDescriptor {
            quality_tier: Some(tier),
            ..Default::default()
        },
    };
    let f = TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 100,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let b = Budget {
        max_cost_micros: 10_000,
        max_latency_ms: 30_000,
        max_router_calls: 2,
    };
    let i = RouteInputs {
        request: RouteRequest::new(
            f,
            vec![
                plan("m-econ", QualityTier::Economy),
                plan("m-front", QualityTier::Frontier),
            ],
            b,
        )
        .unwrap(),
        policy: RoutePolicy {
            router_max_calls: 2,
            ..RoutePolicy::default()
        },
    };
    let profile = o.profile.clone();
    let mut cp = ConfiguredProvider {
        gate: EnablementGate::not_evaluated("model-route/v2", OOT),
        profile: profile.clone(),
        invoker: &mut o.invoker,
        mode: ProviderMode::Explicit,
    };
    let dec = decide(&i, &ctx, Some(&mut cp), &|| i.clone(), None).unwrap();
    assert_eq!(dec.source, DecisionSource::Provider, "{:?}", dec.wire.note);
    assert_eq!(dec.provider_id, OOT);
    // choice-select/v1 through the same adapter (negotiated v3).
    ctx.invocation_id = "oot-choice".into();
    let opt =
        |id: &str, desc: &str| ChoiceOption::new(id, DestinationKind::Tool, desc, "q.v1", "a.v1");
    let ci = ChoiceInputs {
        question: ChoiceQuestion::new("oot.select.v1", DestinationKind::Tool, "q.v1", "a.v1"),
        options: vec![
            opt("tools/search", "search tools index"),
            opt("tools/status", "status page reader"),
        ],
        policy: ChoicePolicy::default(),
        excerpt: None,
    };
    let mut cp = ConfiguredProvider {
        gate: EnablementGate::not_evaluated(CHOICE_TASK, OOT),
        profile,
        invoker: &mut o.invoker,
        mode: ProviderMode::Explicit,
    };
    let out = select_choice(&ci, &ctx, Some(&mut cp));
    let sel = out.selection().unwrap_or_else(|| panic!("{out:?}"));
    assert_eq!(
        sel.id(),
        "tools/search",
        "label word `tools` matches the default keywords"
    );
    assert_eq!(out.report().wire.version, CHOICE_WIRE_VERSION);
}
