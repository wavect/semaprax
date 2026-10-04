//! Provisioned real-compiler evidence for the HP-04 workflow (ignored by default).
//! Needs SEMAPRAX_COMPILER=<absolute path of a built `semaprax`>; python3 is
//! taken from HARNESS_PYTHON or `/usr/bin/which python3`, git from `/usr/bin/which git`.

use crate::support::*;
use semaprax_harness::cli::{self, Environment};
use semaprax_harness::json::sha256_plain;
use semaprax_harness::observe::{Observer, ObserverLimits};
use semaprax_harness::workflow::stages::*;
use semaprax_harness::workflow::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

const NEEDS: &str = "provisioned: needs SEMAPRAX_COMPILER";

fn which(tool: &str) -> PathBuf {
    let out = Command::new("/usr/bin/which")
        .arg(tool)
        .output()
        .expect("which");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn python() -> PathBuf {
    std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .unwrap_or_else(|| which("python3"))
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to.join(e.file_name()));
        } else {
            std::fs::copy(e.path(), to.join(e.file_name())).unwrap();
        }
    }
}

fn fixtures() -> PathBuf {
    repo_root().join("crates/semaprax-harness/tests/fixtures/workflow")
}

struct World {
    root: PathBuf,
    project: PathBuf,
    env: Environment,
    compiler: PathBuf,
    repo: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let o = Command::new(which("git"))
        .current_dir(dir)
        .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
        .args(args)
        .output()
        .expect("git");
    assert!(
        o.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&o.stderr)
    );
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

fn world() -> World {
    let root = fixture_dir("hp-hp04r").canonicalize().unwrap();
    let project = root.join("project");
    copy_dir(&fixtures().join("downstream"), &project);
    git(&project, &["init", "-q"]);
    git(&project, &["add", "semaprax.toml", "src"]);
    git(&project, &["commit", "-qm", "base"]);
    let repo = root.join("host/repo.git");
    std::fs::create_dir_all(&repo).unwrap();
    git(&repo, &["init", "-q", "--bare"]);
    git(
        &project,
        &["push", "-q", repo.to_str().unwrap(), "HEAD:refs/heads/main"],
    );
    std::fs::write(
        repo.join("config"),
        "[core]\n\trepositoryformatversion = 0\n\tfilemode = true\n\tbare = true\n",
    )
    .unwrap();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = Environment {
        harness_home: Some(home),
        compiler: None,
        cwd: project.clone(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    World {
        root,
        project,
        env,
        compiler: required_tool("SEMAPRAX_COMPILER"),
        repo,
    }
}

impl World {
    fn harness(&self, args: &[&str]) -> cli::Outcome {
        cli::run(
            &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            &self.env,
        )
    }

    fn ok(&self, args: &[&str]) {
        let o = self.harness(args);
        assert_eq!(o.code, 0, "{args:?}: {}{}", o.stdout, o.stderr);
    }

    /// Copy an adapter fixture (with the SDK) and adopt + trust it through the profile CLI.
    fn provision(&self, name: &str, src: &Path, sdk_into_sibling: bool) {
        self.provision_rt(name, src, sdk_into_sibling, None)
    }

    /// Like `provision`, recording an explicit adapter runtime at adopt (`--runtime`).
    fn provision_rt(&self, name: &str, src: &Path, sdk_into_sibling: bool, runtime: Option<&Path>) {
        let dir = self.root.join("adapters").join(name);
        copy_dir(src, &dir);
        let sdk = repo_root()
            .join("packages/semaprax-harness-adapters/sdk/python/semaprax_harness_adapter.py");
        if sdk_into_sibling {
            std::fs::copy(&sdk, dir.join("semaprax_harness_adapter.py")).unwrap();
        } else {
            let to = self.root.join("adapters/sdk/python");
            std::fs::create_dir_all(&to).unwrap();
            std::fs::copy(&sdk, to.join("semaprax_harness_adapter.py")).unwrap();
        }
        let node_sdk = repo_root().join("packages/semaprax-harness-adapters/sdk/node");
        copy_dir(&node_sdk, &self.root.join("adapters/sdk/node"));
        let desc_path = dir.join("harness-provider.json");
        let mut d: Value = serde_json::from_slice(&std::fs::read(&desc_path).unwrap()).unwrap();
        let up = &d["upstream"];
        let bundled = up.is_null()
            || up["package"]
                .as_str()
                .is_some_and(|p| p.starts_with("local:"))
                && up["identity_probe"]
                    .as_array()
                    .is_some_and(|a| a.is_empty());
        if !bundled {
            d["upstream"]["identity_probe"] = json!(["--version"]);
            d["upstream"]["versions"] = json!(["0.1.0"]);
            std::fs::write(&desc_path, d.to_string()).unwrap();
        }
        let tool = write(
            &self.root,
            &format!("tools/{name}"),
            "#!/bin/sh\necho tool 0.1.0\n",
        );
        std::fs::set_permissions(&tool, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
        let id = d["provider"]["id"].as_str().unwrap().to_string();
        let mut args = vec!["adopt", desc_path.to_str().unwrap()];
        if !bundled {
            args.extend(["--upstream", tool.to_str().unwrap()]);
        }
        if let Some(rt) = runtime {
            args.extend(["--runtime", rt.to_str().unwrap()]);
        }
        self.ok(&args);
        self.ok(&["trust", &id]);
    }

    fn provision_source_index(&self) {
        let src =
            repo_root().join("packages/semaprax-harness-adapters/examples/source-index-python");
        self.provision("examples/source-index-python", &src, false);
    }

    fn provision_fake_model(&self) {
        self.provision("fake-model", &fixtures().join("adapters/fake-model"), true);
    }

    fn run(&self, extra: &[&str]) -> (i32, Value) {
        let mut args = vec![
            "run",
            self.project.to_str().unwrap(),
            "--compiler",
            self.compiler.to_str().unwrap(),
            "--python",
            "",
            "--json",
        ];
        let py = python();
        args[5] = py.to_str().unwrap();
        args.extend_from_slice(extra);
        let o = self.harness(&args);
        let v: Value = serde_json::from_str(o.stdout.trim())
            .unwrap_or_else(|_| panic!("not JSON: {}{}", o.stdout, o.stderr));
        (o.code, v)
    }

    fn policy(&self) -> PathBuf {
        let head = git(&self.project, &["rev-parse", "HEAD"]);
        let gp = write(&self.root, "host/git-policy.json", &json!({
            "schema": "semaprax.candidate-git-host-policy.v1", "git_executable": which("git"),
            "repository": self.repo, "reference": "refs/heads/main", "base_commit": head,
            "project_prefix": "", "author_name": "Host", "author_email": "host@example.invalid",
            "unix_seconds": 0, "message": "Apply the approved candidate.\n", "max_commands": 512, "timeout_ms": 60000
        }).to_string());
        write(&self.root, "host/apply.json", &json!({"schema": "semaprax.harness-apply-policy.v1", "auto_apply": true, "publication_policy": gp}).to_string())
    }

    fn tree_digest(&self) -> String {
        let s = Snapshot::capture(&self.project).unwrap();
        sha256_plain(format!("{:?}", s.files).as_bytes())
    }

    fn ref_head(&self) -> String {
        git(&self.repo, &["rev-parse", "refs/heads/main"])
    }
}

fn prop(name: &str) -> String {
    fixtures()
        .join("proposals")
        .join(format!("{name}.json"))
        .to_string_lossy()
        .into_owned()
}

fn task() -> String {
    fixtures().join("task.json").to_string_lossy().into_owned()
}

fn provider<'a>(v: &'a Value, cap: &str) -> &'a Value {
    v["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["capability"] == cap)
        .unwrap()
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hp04_real_cli_workflow_selects_fixture_provider_and_judges_candidates() {
    let _ = NEEDS;
    let w = world();
    w.provision_source_index();
    let before = w.tree_digest();
    // Valid repair: real diagnostic, fixture provider invoked, candidate accepted.
    let (code, v) = w.run(&["--task", &task(), "--proposal", &prop("valid")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "approved-candidate-ready");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "test-failure"
                && d["message"].as_str().unwrap().contains("ledger.line_total")),
        "{v}"
    );
    let ctx = provider(&v, "context.repository");
    assert_eq!(ctx["provider"], "org.example/source-index");
    assert_eq!(ctx["state"], "selected");
    assert_eq!(ctx["invoked"], 1, "{v}");
    assert_eq!(v["checks"]["tests"], "passed");
    assert_eq!(v["candidate"]["changed_files"], json!(["src/lib.spx"]));
    for c in [
        "check --json",
        "test --json",
        "project-candidate-preview",
        "project-candidate-export",
    ] {
        assert!(
            v["compiler_commands"]
                .as_array()
                .unwrap()
                .iter()
                .any(|x| x == c),
            "{c} in {v}"
        );
    }
    // Invalid candidates are rejected; nothing is published or written.
    let (code, v) = w.run(&["--task", &task(), "--proposal", &prop("invalid-tests")]);
    assert_eq!(code, 1);
    assert_eq!(v["status"], "rejected", "{v}");
    assert_eq!(v["refusals"][0]["code"], "SPX-HPD050");
    let (_, v) = w.run(&["--task", &task(), "--proposal", &prop("invalid-type")]);
    assert_eq!(v["status"], "refused");
    assert!(
        v["refusals"][0]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-T208"),
        "{v}"
    );
    // Unsupported change kind and raw source: precise diagnostics, no compiler preview.
    let (_, v) = w.run(&["--task", &task(), "--proposal", &prop("unsupported-kind")]);
    assert_eq!(v["refusals"][0]["code"], "SPX-HPD031");
    let (_, v) = w.run(&["--task", &task(), "--proposal", &prop("raw-source")]);
    assert_eq!(v["refusals"][0]["code"], "SPX-HPD031");
    let (_, v) = w.run(&["--task", &task(), "--proposal", &prop("weak-requirements")]);
    assert_eq!(v["refusals"][0]["code"], "SPX-HPD032");
    assert_eq!(
        w.tree_digest(),
        before,
        "the project is never written by the workflow"
    );
    assert_eq!(
        w.ref_head(),
        git(&w.project, &["rev-parse", "HEAD"]),
        "no publication without a policy"
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hp04_real_builtins_only_and_disabled_plugin_run_the_same_workflow() {
    let w = world();
    // Builtins only (nothing adopted): native context for the seed, zero provider calls.
    let (code, v) = w.run(&["--proposal", &prop("valid")]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "approved-candidate-ready");
    assert_eq!(v["external_provider_calls"], 0);
    assert_eq!(
        provider(&v, "context.repository")["provider"],
        "semaprax/native-context"
    );
    assert!(v["compiler_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c.as_str().unwrap().starts_with("context ")));
    // An adopted optional plugin disabled for the run (single switch): same result, never invoked.
    w.provision_source_index();
    let (code, v) = w.run(&["--task", &task(), "--proposal", &prop("valid"), "--disable"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["external_provider_calls"], 0);
    assert_eq!(provider(&v, "context.repository")["state"], "disabled");
    // And disabled in the committed profile.
    std::fs::write(
        w.project.join("semaprax.harness.toml"),
        "schema = \"semaprax.harness-config.v1\"\n[profile]\nenabled = false\n",
    )
    .unwrap();
    let (code, v) = w.run(&["--task", &task(), "--proposal", &prop("valid")]);
    assert_eq!(
        (code, v["external_provider_calls"].as_u64()),
        (0, Some(0)),
        "{v}"
    );
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hp04_real_publication_only_under_host_policy_and_resume_never_replays() {
    let w = world();
    let head = w.ref_head();
    let (_, v) = w.run(&["--proposal", &prop("valid")]);
    assert_eq!(v["status"], "approved-candidate-ready");
    assert_eq!(w.ref_head(), head, "no policy, no publication");
    let policy = w.policy();
    let (code, v) = w.run(&[
        "--proposal",
        &prop("valid"),
        "--apply-policy",
        policy.to_str().unwrap(),
    ]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["status"], "published");
    let new_head = w.ref_head();
    assert_ne!(new_head, head);
    assert_eq!(v["publication"]["published_commit"], new_head);
    // Restart: same lineage, nothing is replayed (the reference does not move again).
    let (_, v2) = w.run(&[
        "--proposal",
        &prop("valid"),
        "--apply-policy",
        policy.to_str().unwrap(),
    ]);
    assert_eq!(v2["status"], "published");
    assert_eq!(w.ref_head(), new_head);
    assert!(!v2["compiler_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "project-candidate-git-publish"));
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hp04_real_model_provider_claims_ignored_and_provider_publication_refused() {
    let w = world();
    w.provision_fake_model();
    let policy = w.policy();
    let head = w.ref_head();
    let goal = |mode: &str| {
        write(&w.root, &format!("task-{mode}.json"), &json!({"schema": "semaprax.harness-task.v1", "goal": format!("MODE:{mode} key sk-live-SECRET-9"), "external_context": "never"}).to_string())
    };
    // Good generation through the host: one invocation, accepted by the compiler.
    let (code, v) = w.run(&["--task", goal("good").to_str().unwrap()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(provider(&v, "model.generate")["invoked"], 1);
    assert_eq!(v["status"], "approved-candidate-ready");
    // Bad candidate with a fabricated claim: the compiler's verdict wins.
    let (_, v) = w.run(&["--task", goal("bad").to_str().unwrap()]);
    assert_eq!(v["status"], "rejected", "{v}");
    // A claim of passing tests on a valid candidate is ignored (and reported).
    let (_, v) = w.run(&["--task", goal("claims").to_str().unwrap()]);
    assert_eq!(v["ignored_provider_claims"], json!(["tests_passed"]), "{v}");
    // Provider-initiated publication: refused by the contract (HPA036), never published.
    let (code, v) = w.run(&[
        "--task",
        goal("publish").to_str().unwrap(),
        "--apply-policy",
        policy.to_str().unwrap(),
    ]);
    assert_eq!(code, 1);
    assert!(
        v["refusals"][0]["message"]
            .as_str()
            .unwrap()
            .contains("SPX-HPA036")
            || v["refusals"][0]["code"] == "SPX-HPA036",
        "{v}"
    );
    assert!(!v["compiler_commands"]
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c == "project-candidate-git-publish"));
    assert_eq!(w.ref_head(), head);
    // Observability: no goal text or secret in the report or the observation log.
    let cache = w.env.harness_home.clone().unwrap().join("cache/workflow");
    let mut seen = 0;
    for dir in std::fs::read_dir(&cache).unwrap() {
        for f in std::fs::read_dir(dir.unwrap().path()).unwrap() {
            let p = f.unwrap().path();
            if p.to_string_lossy().ends_with(".observations.jsonl") {
                let text = std::fs::read_to_string(&p).unwrap();
                assert!(
                    !text.contains("SECRET") && !text.contains("MODE:"),
                    "{text}"
                );
                seen += 1;
            }
        }
    }
    assert!(seen >= 1);
    assert!(!v.to_string().contains("SECRET"));
}

/// Mutates the project while the "model" is generating.
struct Racing(PathBuf);
impl ProposalStage for Racing {
    fn id(&self) -> String {
        "org.example/racing".into()
    }
    fn propose(&mut self, _r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        let lib = self.0.join("src/lib.spx");
        let mut s = std::fs::read_to_string(&lib).unwrap();
        s.push('\n');
        std::fs::write(&lib, s).unwrap();
        Ok(std::fs::read(fixtures().join("proposals/valid.json")).unwrap())
    }
    fn calls(&self) -> u32 {
        1
    }
    fn side_effecting(&self) -> bool {
        false
    }
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hp04_real_stale_revision_during_generation_is_refused() {
    let w = world();
    let snapshot = Snapshot::capture(&w.project).unwrap();
    let cache = w.root.join("cache");
    let compiler = SubprocessCompiler::new(w.compiler.clone(), cache.join("compiler")).unwrap();
    let cfg = RunConfig {
        snapshot,
        task: Task::default(),
        context_max_bytes: 16384,
        cache_dir: cache,
        lock_digest: "disabled".into(),
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
        context_target: None,
    };
    let mut native = NativeContext::new(&compiler);
    let mut p = Racing(w.project.clone());
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let r = run(
        &cfg,
        &compiler,
        Stages {
            decision: None,
            native: &mut native,
            external: None,
            proposer: &mut p,
            command: &mut view,
        },
        &mut obs,
    );
    assert_eq!(r.status, "refused");
    assert_eq!(r.refusals[0].code, "SPX-HPD005", "{:?}", r.refusals);
    assert!(
        !r.compiler_commands
            .iter()
            .any(|c| c.starts_with("project-candidate")),
        "no candidate operation ran on a stale revision"
    );
}

// ---- hpwire: broker-backed context, RTK checks, external decision ----

use semaprax_harness::context::{Broker, BrokerRequest};
use semaprax_harness::workflow::lineage::Lineage;

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hp_hpwire_workflow_context_is_broker_backed_native_plus_structural_external_bounded() {
    let mut w = world();
    let py = python();
    w.env
        .vars
        .insert("HARNESS_PYTHON".into(), py.to_string_lossy().into_owned());
    // The runtime is recorded at adopt: the run below passes no --python.
    w.provision_rt(
        "examples/source-index-python",
        &repo_root().join("packages/semaprax-harness-adapters/examples/source-index-python"),
        false,
        Some(&py),
    );
    let o = w.harness(&[
        "run",
        w.project.to_str().unwrap(),
        "--compiler",
        w.compiler.to_str().unwrap(),
        "--task",
        &task(),
        "--proposal",
        &prop("valid"),
        "--json",
    ]);
    let v: Value = serde_json::from_str(o.stdout.trim())
        .unwrap_or_else(|_| panic!("{}{}", o.stdout, o.stderr));
    assert_eq!(v["status"], "approved-candidate-ready", "{v}");
    assert_eq!(provider(&v, "context.repository")["invoked"], 1, "{v}");
    assert!(
        v["context"]["used_bytes"].as_u64().unwrap()
            <= v["context"]["budget_bytes"].as_u64().unwrap()
    );
    assert_eq!(
        v["context"]["providers"],
        json!(["org.example/source-index"])
    );
    // HN-13: provider answers are cached machine-locally; the identical second run is a
    // cache hit (no provider invocation), not a second provider call.
    let again = w.harness(&[
        "run",
        w.project.to_str().unwrap(),
        "--compiler",
        w.compiler.to_str().unwrap(),
        "--task",
        &task(),
        "--proposal",
        &prop("valid"),
        "--json",
    ]);
    let v2: Value = serde_json::from_str(again.stdout.trim())
        .unwrap_or_else(|_| panic!("{}{}", again.stdout, again.stderr));
    assert_eq!(provider(&v2, "context.repository")["invoked"], 0, "{v2}");
    assert!(w.root.join("home/cache/context").is_dir());
    assert_eq!(v2["status"], "approved-candidate-ready", "{v2}");
    // The packet itself: compiler-verified facts first, structural external hints after.
    let res = semaprax_harness::profile::resolve_project(&w.env, &w.project).unwrap();
    let launch = res.launches.values().next().unwrap().clone();
    let mut env = w.env.clone();
    env.vars
        .insert("HARNESS_PYTHON".into(), py.to_string_lossy().into_owned());
    let snapshot = Snapshot::capture(&w.project).unwrap();
    let t = Task::default();
    let lineage = Lineage::new(snapshot.binding(), "sha256:lock", &t.digest());
    let mut stage = BrokerContext::new(
        w.compiler.clone(),
        launch,
        env,
        res.profile.lock_digest(),
        res.profile.config_digest.clone(),
        vec![],
    )
    .unwrap();
    let req = ContextRequest {
        lineage: &lineage,
        project: w.project.clone(),
        seed: Some("ledger.line_total"),
        query: "ledger.line_total".into(),
        max_bytes: 12000,
        external: ExternalContext::Always,
    };
    if std::env::var_os("HPWIRE_DEBUG").is_some() {
        let mut b = Broker::new(
            Some(Box::new(semaprax_harness::context::SubprocessNative::new(
                w.compiler.clone(),
            ))),
            None,
        );
        let mut env2 = w.env.clone();
        env2.vars
            .insert("HARNESS_PYTHON".into(), py.to_string_lossy().into_owned());
        let l = res.launches.values().next().unwrap().clone();
        b.add_provider(Box::new(semaprax_harness::context::HostExternal::new(
            l,
            env2,
            "x".into(),
            "y".into(),
            vec![],
        )))
        .unwrap();
        let o = b
            .context(&w.project, &BrokerRequest::new("ledger.line_total", 12000))
            .unwrap();
        eprintln!("DEBUG {}", o.rendered);
    }
    let packet = stage.collect(&req).unwrap();
    let note = stage.take_note();
    let native: Vec<_> = packet
        .items
        .iter()
        .filter(|i| i.provenance == "compiler-verified")
        .collect();
    let external: Vec<_> = packet
        .items
        .iter()
        .filter(|i| i.provenance.starts_with("external:"))
        .collect();
    assert!(!native.is_empty(), "{packet:?}");
    assert!(!external.is_empty(), "{note:?} {packet:?}");
    assert!(native[0].text.contains("ledger.line_total"));
    assert!(packet.items.iter().map(|i| i.bytes()).sum::<usize>() <= 12000 + 4096);
    assert_eq!(stage.calls(), 1);
    let _ = Broker::new(None, None).cache();
    let _ = BrokerRequest::new("x", 1);
}

const RTK_ID: &str = "ai.rtk/rtk-command-view";

fn libtest_script(counter: &Path, exit: i32) -> String {
    format!(
        "#!/bin/sh\nn=$(cat \"{c}\" 2>/dev/null || echo 0); echo $((n+1)) > \"{c}\"\n\
         echo 'running 220 tests'; i=0; while [ $i -lt 220 ]; do echo \"test mod::case_$i ... ok\"; i=$((i+1)); done\n\
         echo 'test mod::bad ... FAILED'; echo 'CRITICAL-PLANTED-7731: assertion failed' >&2\n\
         echo 'test result: FAILED. 220 passed; 1 failed'\nexit {exit}\n",
        c = counter.display(), exit = exit
    )
}

fn rtk_world(exit: i32) -> (World, PathBuf) {
    let mut w = world();
    let rtk = required_tool("HARNESS_RTK");
    let py = python();
    std::fs::create_dir_all(w.root.join("userhome")).unwrap();
    w.env.vars.insert(
        "HOME".into(),
        w.root.join("userhome").to_string_lossy().into_owned(),
    );
    let home = w.env.harness_home.clone().unwrap();
    write(
        &home,
        "command-view.json",
        r#"{"schema":"semaprax.harness-command-view-policy.v1","min_bytes":512,"retention":{"enabled":true,"ttl_secs":3600,"max_bytes":268435456}}"#,
    );
    let tool = w.root.join("tools/rtk");
    std::fs::create_dir_all(tool.parent().unwrap()).unwrap();
    std::fs::copy(&rtk, &tool).unwrap();
    // The shipped descriptor is adopted unmodified; the runtime is recorded, not passed per run.
    let desc = repo_root().join("packages/semaprax-harness-adapters/rtk/harness-provider.json");
    w.ok(&[
        "adopt",
        desc.to_str().unwrap(),
        "--upstream",
        tool.to_str().unwrap(),
        "--runtime",
        py.to_str().unwrap(),
    ]);
    w.ok(&["trust", RTK_ID]);
    let counter = w.root.join("check-counter");
    let script = write(&w.project, "tools/cargo", &libtest_script(&counter, exit));
    std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    write(&w.project, "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n[workflow.check.unit]\nargv = [\"tools/cargo\", \"test\"]\n");
    (w, counter)
}

fn run_no_runtime_flags(w: &World, extra: &[&str]) -> (i32, Value) {
    let mut args = vec![
        "run",
        w.project.to_str().unwrap(),
        "--compiler",
        w.compiler.to_str().unwrap(),
        "--json",
    ];
    args.extend_from_slice(extra);
    let o = w.harness(&args);
    let v: Value = serde_json::from_str(o.stdout.trim())
        .unwrap_or_else(|_| panic!("{}{}", o.stdout, o.stderr));
    (o.code, v)
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_RTK HARNESS_PYTHON"]
fn hp_hpwire_workflow_check_uses_rtk_automatically_and_status_stays_authoritative() {
    // Failing command: the verdict is the exit status, never RTK's summary.
    let (w, counter) = rtk_world(1);
    let (code, v) = run_no_runtime_flags(&w, &["--proposal", &prop("valid")]);
    assert_eq!(code, 1, "{v}");
    assert_eq!(v["status"], "rejected", "{v}");
    assert_eq!(v["refusals"][0]["code"], "SPX-HPD050");
    let n: u32 = std::fs::read_to_string(&counter)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(n, 1, "the check command ran exactly once");
    // Passing command: RTK's view reaches the model-facing report.
    let (w, counter) = rtk_world(0);
    let obs = w.root.join("obs.jsonl");
    let (code, v) = run_no_runtime_flags(
        &w,
        &[
            "--proposal",
            &prop("valid"),
            "--observations",
            obs.to_str().unwrap(),
        ],
    );
    assert_eq!(code, 0, "{v}");
    let cmd = &v["checks"]["commands"][0];
    assert_eq!(cmd["passed"], true, "{v}");
    assert_eq!(cmd["view_route"], "provider", "{cmd}");
    assert_eq!(cmd["view_provenance"], RTK_ID);
    assert_eq!(cmd["executions"], 1);
    assert!(
        cmd["view"]
            .as_str()
            .unwrap()
            .contains("CRITICAL-PLANTED-7731"),
        "{cmd}"
    );
    assert_eq!(std::fs::read_to_string(&counter).unwrap().trim(), "1");
    // One metadata-only observation per stage, no payload text.
    let text = std::fs::read_to_string(&obs).unwrap();
    for stage in ["context_select", "decision", "command_view"] {
        assert!(
            text.contains(&format!("\"stage\":\"{stage}\"")),
            "{stage} in {text}"
        );
    }
    assert!(
        !text.contains("CRITICAL-PLANTED") && !text.contains("fix ledger"),
        "{text}"
    );
    // Export to the token-observation schema consumed by scripts/token_report.py.
    let rows = w.root.join("rows.jsonl");
    w.ok(&[
        "report",
        obs.to_str().unwrap(),
        "--export",
        "token-observation",
        "--output",
        rows.to_str().unwrap(),
    ]);
    let first: Value = serde_json::from_str(
        std::fs::read_to_string(&rows)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(first["schema"], "semaprax.token-observation.v1");
    if let Some(dst) = std::env::var_os("HPWIRE_EXPORT_COPY") {
        std::fs::copy(&rows, dst).unwrap();
    }
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER HARNESS_NODE"]
fn hp_hpwire_workflow_consults_external_decision_node_only_when_explicit() {
    let node = PathBuf::from(std::env::var("HARNESS_NODE").expect("HARNESS_NODE"));
    let w = world();
    w.provision_rt(
        "examples/decision-node",
        &repo_root().join("packages/semaprax-harness-adapters/examples/decision-node"),
        false,
        Some(&node),
    );
    let models = json!([
        {"id": "cheap", "destination": {"kind": "local"}, "capabilities": ["structured_output"], "max_context": 1000000, "est_cost_micros": 0, "est_latency_ms": 10, "strength_rank": 1},
        {"id": "strong", "destination": {"kind": "local"}, "capabilities": ["structured_output"], "max_context": 1000000, "est_cost_micros": 0, "est_latency_ms": 10, "strength_rank": 2}
    ]);
    let t = write(
        &w.root,
        "task-models.json",
        &json!({"schema": "semaprax.harness-task.v1", "goal": "g", "models": models}).to_string(),
    );
    // Auto-selected and not evaluated: rules decide, the provider is not consulted.
    let (_, v) = run_no_runtime_flags(
        &w,
        &["--task", t.to_str().unwrap(), "--proposal", &prop("valid")],
    );
    assert_eq!(v["route"]["router_calls"], 0, "{v}");
    assert!(
        v["route"]["status"].as_str().unwrap().starts_with("rules"),
        "{v}"
    );
    // Explicit pin: experimental, one real node invocation under the host deadline.
    write(&w.project, "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nprovider = \"org.example/threshold-route\"\n");
    std::fs::remove_file(w.project.join("semaprax.harness.lock")).ok();
    let (_, v) = run_no_runtime_flags(
        &w,
        &["--task", t.to_str().unwrap(), "--proposal", &prop("valid")],
    );
    assert_eq!(v["route"]["status"], "experimental", "{v}");
    assert_eq!(v["route"]["router_calls"], 1, "{v}");
    assert_eq!(provider(&v, "decision.evaluate")["invoked"], 1, "{v}");
}

// ---- HN-01 / HN-02 / HN-11: the pipeline and session over the real compiler ----

#[path = "workflow_compiler/hn.rs"]
mod hn;

fn skill_state(w: &World) -> Value {
    let o = w.harness(&["skills", "status", "--json"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v: Value = serde_json::from_str(o.stdout.trim()).unwrap();
    v["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["id"] == "ponytail")
        .unwrap()
        .clone()
}

#[test]
#[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
fn hn_hn04_real_run_delivers_official_ponytail_by_task_family_and_honors_the_switch() {
    let _ = NEEDS;
    let task_file = |w: &World, family: &str| {
        write(
            &w.root,
            &format!("host/task-{family}.json"),
            &json!({"schema": "semaprax.harness-task.v1", "goal": "fix ledger.line_total so the contract holds",
                    "task_family": family, "external_context": "always"})
            .to_string(),
        )
        .to_string_lossy()
        .into_owned()
    };
    let w = world();
    // A coding task: official Ponytail is selected, loaded, applied, locked and counted.
    let (code, v) = w.run(&[
        "--task",
        &task_file(&w, "mechanical"),
        "--proposal",
        &prop("valid"),
    ]);
    assert_eq!(code, 0, "{v}");
    let skills = &v["context"]["skills"];
    let entry = skills["loaded"][0].as_str().unwrap();
    assert!(entry.starts_with("ponytail@v4.10.3:full:sha256:"), "{v}");
    assert!(
        skills["model_visible_bytes"].as_u64().unwrap() > 5000,
        "{v}"
    );
    let st = skill_state(&w);
    assert_eq!(st["selected"], true);
    assert_eq!(st["applied_to_model"], true);
    assert!(entry.ends_with(st["locked_revision"].as_str().unwrap()));
    // A prose task in the same project: no skill bytes, and status follows the latest turn.
    let (_, v) = w.run(&[
        "--task",
        &task_file(&w, "translation"),
        "--proposal",
        &prop("valid"),
    ]);
    assert!(v["context"].get("skills").is_none(), "{v}");
    let st = skill_state(&w);
    assert_eq!(st["selected"], false);
    assert_eq!(st["applied_to_model"], false);
    // The project switch disables it and keeps it out of the prompt.
    let w2 = world();
    write(
        &w2.project,
        "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n[skills]\nofficial = false\n",
    );
    let (_, v) = w2.run(&[
        "--task",
        &task_file(&w2, "mechanical"),
        "--proposal",
        &prop("valid"),
    ]);
    assert!(v["context"].get("skills").is_none(), "{v}");
    let st = skill_state(&w2);
    assert!(
        st["disabled"]
            .as_str()
            .unwrap()
            .starts_with("official-skills-switch-off"),
        "{st}"
    );
    assert_eq!(st["applied_to_model"], false);
}

/// HN-13 / HN-16 user-path wiring against the real compiler.
mod hnwire {
    use super::*;
    use semaprax_harness::context::external::{Coverage, ExternalQuery, ExternalResponse, RawItem};
    use semaprax_harness::context::identity::Snapshot as CtxSnapshot;
    use semaprax_harness::context::item::{Span, Tier};
    use semaprax_harness::context::{
        ExternalSource, NativeContextSource, ProviderIdentity, SubprocessNative,
    };
    use semaprax_harness::diag::HarnessResult;
    use semaprax_harness::workflow::compiler::SubprocessCompiler;
    use semaprax_harness::workflow::stages::{ProposalRequest, RawCommandView, TaskMode};
    use semaprax_harness::workflow::{
        BrokerContext, Composition, RunConfig, SessionBounds, Snapshot, Stages,
    };
    use std::cell::{Cell, RefCell};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    const ID: &str = "org.example/ident-search";

    fn idents(s: &str) -> Vec<String> {
        s.split(|c: char| !(c.is_alphanumeric() || c == '_'))
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Identifier search over the non-`.spx` files; records every query.
    struct Search {
        seen: Arc<Mutex<Vec<String>>>,
        n: Arc<AtomicUsize>,
    }

    impl ExternalSource for Search {
        fn identity(&self) -> ProviderIdentity {
            ProviderIdentity {
                provider_id: ID.into(),
                provider_version: "1.0.0".into(),
                adapter_version: "0.1.0".into(),
                upstream_version: None,
                descriptor_digest: "sha256:d".into(),
                config_digest: "sha256:c".into(),
                permission_scope: json!({"read": ["project"]}),
            }
        }
        fn scope(&self) -> Vec<String> {
            vec![]
        }
        fn recheck_authority(&self) -> HarnessResult<()> {
            Ok(())
        }
        fn query(
            &self,
            snap: &CtxSnapshot,
            q: &ExternalQuery,
            _max: usize,
        ) -> HarnessResult<ExternalResponse> {
            self.n.fetch_add(1, Ordering::SeqCst);
            let text = q.payload["query"].as_str().unwrap_or("").to_string();
            self.seen.lock().unwrap().push(text.clone());
            let want = idents(&text);
            let mut items = Vec::new();
            for rel in snap.files.keys().filter(|r| !r.ends_with(".spx")) {
                let Ok(t) = std::fs::read_to_string(snap.root.join(rel)) else {
                    continue;
                };
                for (i, l) in t.split('\n').enumerate() {
                    if idents(l).iter().any(|w| want.contains(w)) {
                        items.push(RawItem {
                            path: rel.clone(),
                            span: Span {
                                start_line: i as u64 + 1,
                                end_line: i as u64 + 1,
                            },
                            digest: sha256_plain(l.as_bytes()),
                            tier: Tier::Structural,
                            language: "typescript".into(),
                            rank: 1.0,
                            text: Some(l.to_string()),
                            span_kind: Some("start-line".into()),
                            edges: vec![],
                        });
                    }
                }
            }
            Ok(ExternalResponse {
                status: "complete".into(),
                no_references: items.is_empty(),
                items,
                coverage: Coverage {
                    complete: true,
                    exhaustive: false,
                    indexed_files: 1,
                    skipped: vec![],
                },
                upstream_version: None,
                provider_id: ID.into(),
                diagnostics: vec![],
            })
        }
    }

    struct Seq {
        items: Vec<Value>,
        prompts: RefCell<Vec<Value>>,
        calls: Cell<u32>,
    }
    struct SeqRef<'a>(&'a Seq);
    impl ProposalStage for SeqRef<'_> {
        fn id(&self) -> String {
            "org.example/seq".into()
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

    #[test]
    #[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
    fn hn_hn13_real_failed_first_candidate_triggers_exactly_one_focused_follow_up_that_enables_attempt_two(
    ) {
        let root = fixture_dir("hp-hnwire-r").canonicalize().unwrap();
        let project = root.join("project");
        copy_dir(&fixtures().join("healthy"), &project);
        std::fs::create_dir_all(project.join("web")).unwrap();
        std::fs::write(
            project.join("web/app.ts"),
            "export const renderLineCost = (n: number) => n; // TypeScript wrapper\nexport const ledgerGlue = 'FOLLOWUP-MARKER glue for the G225 intention diagnostic';\n",
        )
        .unwrap();
        let exe = required_tool("SEMAPRAX_COMPILER");
        let cache = root.join("cache");
        let svc = SubprocessCompiler::new(exe.clone(), root.join("compiler")).unwrap();
        let nat =
            || Some(Box::new(SubprocessNative::new(exe.clone())) as Box<dyn NativeContextSource>);
        let (seen, n) = (Arc::new(Mutex::new(vec![])), Arc::new(AtomicUsize::new(0)));
        let mut stage = BrokerContext::with_sources(
            nat(),
            nat(),
            Box::new(Search {
                seen: seen.clone(),
                n: n.clone(),
            }),
            ID.into(),
            None,
        )
        .unwrap();
        let bad = json!({"schema": "semaprax.harness-proposal.v1", "intent":
            {"kind": "rename_declaration", "target": "ledger.line_totall", "name": "line_cost"}});
        let good = json!({"schema": "semaprax.harness-proposal.v1", "intent":
            {"kind": "rename_declaration", "target": "ledger.line_total", "name": "line_cost"}});
        let seq = Seq {
            items: vec![bad, good],
            prompts: RefCell::default(),
            calls: Cell::new(0),
        };
        let cfg = RunConfig {
            snapshot: Snapshot::capture(&project).unwrap(),
            task: Task {
                schema_version: 2,
                mode: TaskMode::Change,
                goal: "rename line_total to line_cost and keep the TypeScript wrapper in step"
                    .into(),
                seed: Some("ledger.line_total".into()),
                external_context: ExternalContext::WhenNeeded,
                session: Some(SessionBounds {
                    max_attempts: 3,
                    ..Default::default()
                }),
                ..Task::default()
            },
            context_max_bytes: 16384,
            cache_dir: cache,
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
            context_target: None,
        };
        let mut prop = SeqRef(&seq);
        let mut view = RawCommandView;
        // Like the CLI: the broker is the native slot and also the provider slot.
        let r = semaprax_harness::workflow::run(
            &cfg,
            &svc,
            Stages {
                decision: None,
                native: &mut stage,
                external: None,
                proposer: &mut prop,
                command: &mut view,
            },
            &mut Observer::new(None, ObserverLimits::default()),
        );
        assert_eq!(
            r.status, "candidate-ready",
            "{:?} {:?}",
            r.refusals, r.notes
        );
        assert_eq!(seq.calls.get(), 2);
        let q = seen.lock().unwrap().clone();
        assert_eq!(
            q.len(),
            2,
            "one planned query and exactly one focused follow-up: {q:?}"
        );
        assert!(
            q[1].contains("G225"),
            "the follow-up is worded by the compiler's failure: {q:?}"
        );
        assert!(!q[1].contains("FOLLOWUP"), "not a dump: {q:?}");
        let p = seq.prompts.borrow();
        assert!(!p[0].to_string().contains("FOLLOWUP-MARKER"));
        assert!(
            p[1].to_string().contains("FOLLOWUP-MARKER"),
            "follow-up result reached attempt 2"
        );
        assert!(r.context["plan"]["follow_up"]["added_items"]
            .as_u64()
            .is_some());
        assert!(r.steps.iter().any(|(k, _)| k == "context-follow-up"));
    }

    #[test]
    #[ignore = "provisioned: needs SEMAPRAX_COMPILER"]
    fn hn_hn16_real_cli_run_honors_the_project_pin_and_explains_the_route() {
        let w = super::world();
        use semaprax_harness::decision::{Destination, ModelPlan};
        let m = |id: &str, rank: u32| {
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
            .to_json()
        };
        let task = write(
            &w.root,
            "host/task-route.json",
            &json!({"schema": "semaprax.harness-task.v1", "goal": "fix ledger.line_total so the contract holds",
                    "models": [m("cheap", 1), m("strong", 2)]})
            .to_string(),
        );
        let run = |w: &super::World| {
            let o = w.harness(&[
                "run",
                w.project.to_str().unwrap(),
                "--compiler",
                w.compiler.to_str().unwrap(),
                "--task",
                task.to_str().unwrap(),
                "--proposal",
                &super::prop("valid"),
                "--json",
            ]);
            serde_json::from_str::<Value>(o.stdout.trim())
                .unwrap_or_else(|_| panic!("{}{}", o.stdout, o.stderr))
        };
        let base = run(&w);
        assert_eq!(base["route"]["choice"], "cheap", "{base}");
        write(
            &w.project,
            "semaprax.harness.toml",
            "schema = \"semaprax.harness-config.v1\"\n[routing]\nmode = \"auto\"\npin = \"strong\"\nallow_remote = false\n",
        );
        let _ = std::fs::remove_file(w.project.join("semaprax.harness.lock"));
        let v = run(&w);
        assert_eq!(v["route"]["choice"], "strong", "{}", v["route"]);
        assert_eq!(v["route"]["mode"], "pin");
        assert_eq!(v["route"]["policy"]["allow_remote"], false);
        assert_eq!(v["route"]["explanation"]["rules_reason"], "pinned");
        assert_eq!(v["status"], "approved-candidate-ready", "{v}");
    }
}
