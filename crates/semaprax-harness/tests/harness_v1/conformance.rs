//! HP-16 conformance kit tests (fixture prefix `hp-hp16b`).

use crate::support::{fixture_dir, repo_root, write};
use semaprax_harness::cli::{run as cli_run, Environment};
use semaprax_harness::conformance::{self, Options, Report, Verdict};
use semaprax_harness::contract::{CapabilityKind, CapabilityRef, ProjectBinding, RequestEnvelope};
use semaprax_harness::host::grant::Grant;
use semaprax_harness::host::{
    AdapterManager, CancelToken, HostConfig, InvocationClass, IsolationRequest, LaunchSpec, Outcome,
};
use semaprax_harness::profile::{check_grant_current, resolve_project};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Resolve the test host's Node executable, then pass its absolute path to the
/// adapter. The adapter itself never searches PATH.
fn node() -> PathBuf {
    if let Some(path) = std::env::var_os("SEMAPRAX_TEST_NODE") {
        return PathBuf::from(path);
    }
    let output = std::process::Command::new("/usr/bin/which")
        .arg("node")
        .output()
        .expect("find test host Node interpreter");
    assert!(output.status.success(), "test host needs Node");
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

fn python() -> PathBuf {
    let out = std::process::Command::new("/usr/bin/which")
        .arg("python3")
        .output()
        .unwrap();
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

fn examples() -> PathBuf {
    repo_root().join("packages/semaprax-harness-adapters/examples")
}

fn env(tmp: &Path) -> Environment {
    let mut vars = BTreeMap::new();
    vars.insert("TMPDIR".to_string(), tmp.to_string_lossy().into_owned());
    Environment {
        harness_home: None,
        compiler: None,
        cwd: tmp.to_path_buf(),
        vars,
    }
}

fn opts(desc: &Path, suites: &[&str], runtime: Option<PathBuf>) -> Options {
    Options {
        descriptor: desc.to_path_buf(),
        suites: suites.iter().map(|s| s.to_string()).collect(),
        runtime,
        hostile_runtime: Some(python()),
        ..Default::default()
    }
}

fn go(o: &Options) -> Report {
    conformance::run(o, &env(&fixture_dir("hp-hp16b-tmp"))).expect("conformance run")
}

fn suite<'a>(r: &'a Report, name: &str) -> &'a conformance::Suite {
    r.suites
        .iter()
        .find(|s| s.name == name)
        .unwrap_or_else(|| panic!("no suite {name}: {}", r.render_human()))
}

fn assert_clean(r: &Report, name: &str) {
    let s = suite(r, name);
    assert_eq!(s.verdict(), Verdict::Pass, "{}", r.render_human());
    assert!(
        s.cases.iter().all(|c| c.verdict != Verdict::Fail),
        "{}",
        r.render_human()
    );
}

fn failing(r: &Report, name: &str) -> Vec<String> {
    suite(r, name)
        .cases
        .iter()
        .filter(|c| c.verdict == Verdict::Fail)
        .map(|c| c.name.clone())
        .collect()
}

#[test]
fn hp_hp16b_source_index_passes_context_suite() {
    let d = examples().join("source-index-python/harness-provider.json");
    let r = go(&opts(&d, &["context"], Some(python())));
    assert_clean(&r, "context.repository");
    assert_eq!(r.to_json()["support_decision"], "not-a-support-decision");
}

#[test]
fn hp_hp16b_output_view_passes_command_suite() {
    let d = examples().join("output-view-python/harness-provider.json");
    let r = go(&opts(&d, &["command"], Some(python())));
    assert_clean(&r, "command.view");
    let raw = suite(&r, "command.view")
        .cases
        .iter()
        .find(|c| c.name == "raw-recovery")
        .unwrap();
    assert_eq!(raw.verdict, Verdict::Unverified);
}

#[test]
fn hp_hp16b_decision_node_passes_decision_suite() {
    let d = examples().join("decision-node/harness-provider.json");
    let r = go(&opts(&d, &["decision"], Some(node())));
    assert_clean(&r, "decision.evaluate");
}

#[test]
fn hp_hp16b_skill_fixture_passes_skill_suite() {
    let d = repo_root().join(
        "crates/semaprax-harness/tests/fixtures/conformance/skill-python/harness-provider.json",
    );
    let r = go(&opts(&d, &["skill"], Some(python())));
    assert_clean(&r, "skill.catalog");
}

/// Locate the Rust example adapter next to the running test binary, building
/// it (offline, same target directory and profile) when it is missing, so a
/// clean target passes without a manual `cargo build --example` step.
fn built_rust_example() -> PathBuf {
    static BUILD: std::sync::Once = std::sync::Once::new();
    let exe = std::env::current_exe().unwrap();
    // <target>/<profile>/deps/<test-binary>
    let profile_dir = exe.parent().unwrap().parent().unwrap().to_path_buf();
    let target_dir = profile_dir.parent().unwrap().to_path_buf();
    let bin = profile_dir.join("examples/context_adapter_rust");
    BUILD.call_once(|| {
        if bin.is_file() {
            return;
        }
        let mut cmd = std::process::Command::new(env!("CARGO"));
        cmd.args([
            "build",
            "--offline",
            "-p",
            "semaprax-harness",
            "--example",
            "context_adapter_rust",
        ])
        .arg("--manifest-path")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir);
        if profile_dir.file_name().is_some_and(|n| n == "release") {
            cmd.arg("--release");
        }
        let out = cmd
            .output()
            .expect("run cargo to build the Rust example adapter");
        assert!(
            out.status.success(),
            "cargo build --example context_adapter_rust failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    });
    assert!(
        bin.is_file(),
        "example adapter missing after build: {}",
        bin.display()
    );
    bin
}

/// Copy the Rust example adapter next to a descriptor in a temp dir.
fn rust_example(tmp: &Path) -> PathBuf {
    let bin = built_rust_example();
    let dst = tmp.join("target/debug/examples");
    std::fs::create_dir_all(&dst).unwrap();
    std::fs::copy(&bin, dst.join("context_adapter_rust")).unwrap();
    std::fs::copy(
        examples().join("context-rust.harness-provider.json"),
        tmp.join("harness-provider.json"),
    )
    .unwrap();
    tmp.join("harness-provider.json")
}

#[test]
fn hp_hp16b_rust_example_passes_context_suite() {
    let tmp = fixture_dir("hp-hp16b-rust");
    let d = rust_example(&tmp);
    let r = go(&opts(&d, &["context"], None));
    assert_clean(&r, "context.repository");
}

#[test]
fn hp_hp16b_common_suites_pass_for_source_index() {
    let d = examples().join("source-index-python/harness-provider.json");
    let r = go(&opts(&d, &["common"], Some(python())));
    assert_clean(&r, "common");
    let s = suite(&r, "common.hostility");
    assert!(
        s.cases.iter().all(|c| c.verdict != Verdict::Fail),
        "{}",
        r.render_human()
    );
    let host: Vec<&str> = s.cases.iter().map(|c| c.name.as_str()).collect();
    for want in [
        "spoofed-invocation-id",
        "stale-revision-refused",
        "path-escape",
        "response-flood",
        "cancellation-group-kill-with-grandchild",
        "budget-abuse-job-cap",
        "stderr-flood-bounded",
    ] {
        assert!(host.contains(&want), "{want} missing");
    }
    let frame = suite(&r, "common")
        .cases
        .iter()
        .find(|c| c.name == "declared-frame-limit-enforced")
        .unwrap();
    assert_eq!(
        frame.verdict,
        Verdict::Pass,
        "declared max_frame_bytes must be enforced"
    );
    let rec = suite(&r, "common")
        .cases
        .iter()
        .find(|c| c.name == "recursive-invocation-refused")
        .unwrap();
    assert_eq!(rec.verdict, Verdict::Pass);
}

fn mutant(mode: &str, suite_flag: &str, suite_name: &str, expect_case: &str) {
    let d = examples().join("hostile-python/harness-provider.json");
    let mut o = opts(&d, &[suite_flag], Some(python()));
    o.forward_env.insert("HOSTILE_MODE".into(), mode.into());
    let r = go(&o);
    assert_eq!(
        suite(&r, suite_name).verdict(),
        Verdict::Fail,
        "{mode}: {}",
        r.render_human()
    );
    assert!(
        failing(&r, suite_name).iter().any(|c| c == expect_case),
        "{mode}: {:?}\n{}",
        failing(&r, suite_name),
        r.render_human()
    );
    assert_eq!(r.verdict(), Verdict::Fail);
}

#[test]
fn hp_hp16b_mutant_drop_critical_error_fails_command_suite() {
    mutant(
        "drop_critical_error",
        "command",
        "command.view",
        "critical-lines-survive-or-loss-is-declared",
    );
}

#[test]
fn hp_hp16b_mutant_fake_revision_fails_context_suite() {
    mutant(
        "fake_revision",
        "context",
        "context.repository",
        "finds-planted-symbol",
    );
}

#[test]
fn hp_hp16b_mutant_forbidden_model_fails_decision_suite() {
    mutant(
        "forbidden_model",
        "decision",
        "decision.evaluate",
        "choice-within-options",
    );
}

#[test]
fn hp_hp16b_mutant_ignore_cancel_fails_decision_suite() {
    mutant(
        "ignore_cancel",
        "decision",
        "decision.evaluate",
        "cancellation-cooperative",
    );
}

#[test]
fn hp_hp16b_unknown_capabilities_are_visible_but_inactive() {
    let tmp = fixture_dir("hp-hp16b-unknown");
    std::fs::copy(
        examples().join("hostile-python/adapter.py"),
        tmp.join("adapter.py"),
    )
    .unwrap();
    let mut v: Value = serde_json::from_slice(
        &std::fs::read(examples().join("hostile-python/harness-provider.json")).unwrap(),
    )
    .unwrap();
    v["capabilities"].as_array_mut().unwrap().extend([
        json!({"kind": "vector.search", "version": 1, "required": false, "operations": ["query"]}),
        json!({"kind": "skill.catalog", "version": 2, "required": false, "operations": ["list"]}),
    ]);
    v["extensions"] = json!([{"kind": "x.example/notes", "version": 1}]);
    write(&tmp, "harness-provider.json", &v.to_string());
    let r = go(&opts(
        &tmp.join("harness-provider.json"),
        &["skill"],
        Some(python()),
    ));
    let inactive: Vec<String> = r.to_json()["inactive_capabilities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| format!("{}@{}", i["kind"].as_str().unwrap(), i["version"]))
        .collect();
    for want in ["vector.search@1", "skill.catalog@2", "x.example/notes@1"] {
        assert!(
            inactive.contains(&want.to_string()),
            "{want} in {inactive:?}"
        );
    }
    let s = suite(&r, "skill.catalog");
    assert_eq!(
        s.verdict(),
        Verdict::Unverified,
        "an inactive capability is never exercised"
    );
    assert!(r.suites.iter().all(|s| !s.name.contains("vector")));
}

#[test]
fn hp_hp16b_cli_report_is_deterministic_and_marks_no_support() {
    let tmp = fixture_dir("hp-hp16b-cli");
    let d = examples().join("output-view-python/harness-provider.json");
    let args: Vec<String> = [
        "conformance",
        d.to_str().unwrap(),
        "--suite",
        "command",
        "--runtime",
        python().to_str().unwrap(),
        "--json",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let e = env(&tmp);
    let (a, b) = (cli_run(&args, &e), cli_run(&args, &e));
    assert_eq!(a.code, 0, "{}{}", a.stdout, a.stderr);
    assert_eq!(a.stdout, b.stdout, "report must be deterministic");
    let v: Value = serde_json::from_str(&a.stdout).unwrap();
    assert_eq!(v["schema"], "semaprax.harness-conformance-report.v1");
    assert_eq!(v["support_decision"], "not-a-support-decision");
    assert_eq!(v["subject"]["license"], "Apache-2.0");
    assert_eq!(v["subject"]["isolation"]["declared"], "subprocess");
    // usage and a node target without a runtime are refused
    assert_eq!(cli_run(&["conformance".into()], &e).code, 2);
    let node = examples().join("decision-node/harness-provider.json");
    let r = cli_run(
        &["conformance".into(), node.to_string_lossy().into_owned()],
        &e,
    );
    assert_eq!(r.code, 1);
    assert!(r.stderr.contains("SPX-HPP002"), "{}", r.stderr);
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn request(kind: CapabilityKind, op: &str, payload: Value, id: &str, pid: &str) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: id.into(),
        project: ProjectBinding {
            id: pid.into(),
            worktree: "w".into(),
            revision: "r".into(),
        },
        lock_digest: format!("sha256:{}", "0".repeat(64)),
        capability: CapabilityRef { kind, version: 1 },
        operation: op.into(),
        deadline_ms: 30_000,
        max_result_bytes: 1 << 20,
        remaining_calls: 8,
        lineage: vec![],
        payload,
    }
}

fn spec(
    l: &semaprax_harness::profile::ResolvedLaunch,
    runtime: Option<PathBuf>,
    root: &Path,
    work: &Path,
    grant: Grant,
) -> LaunchSpec {
    LaunchSpec {
        descriptor: l.descriptor.clone(),
        descriptor_dir: l.descriptor_path.parent().unwrap().to_path_buf(),
        runtime_executable: runtime,
        upstream_executable: l.upstream_path.clone(),
        grant,
        project_root: root.to_path_buf(),
        cache_dir: work.join(format!("cache-{}", l.provider_id.replace('/', "_"))),
        retention_dir: work.join("retention"),
        isolation: IsolationRequest::None,
        forward_env: BTreeMap::new(),
    }
}

/// A third context provider from a separate directory, selected purely by
/// project config; the same host also runs the Rust and Node adapters.
#[test]
fn hp_hp16b_third_provider_journey_without_core_edits() {
    let tmp = fixture_dir("hp-hp16b-journey");
    let tmp = tmp.canonicalize().unwrap();
    let repo = tmp.join("elsewhere");
    for (from, to) in [
        (
            "examples/source-index-python",
            "examples/source-index-python",
        ),
        ("sdk/python", "sdk/python"),
    ] {
        let src = repo_root()
            .join("packages/semaprax-harness-adapters")
            .join(from);
        std::fs::create_dir_all(repo.join(to)).unwrap();
        for f in std::fs::read_dir(&src).unwrap().flatten() {
            if f.path().is_file() {
                std::fs::copy(f.path(), repo.join(to).join(f.file_name())).unwrap();
            }
        }
    }
    // The adapter ships its own index: no separate upstream executable.
    let desc = repo.join("examples/source-index-python/harness-provider.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&desc).unwrap()).unwrap();
    v.as_object_mut().unwrap().remove("upstream");
    std::fs::write(&desc, v.to_string()).unwrap();

    let project = tmp.join("project");
    write(&project, "src/lib.rs", "pub fn journey_symbol() {}\n");
    write(&project, "semaprax.harness.toml", "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"context.repository\"]\nmode = \"required\"\nprovider = \"org.example/source-index\"\n");
    let home = tmp.join("home");
    let e = Environment {
        harness_home: Some(home),
        compiler: None,
        cwd: tmp.clone(),
        vars: BTreeMap::new(),
    };
    let p = project.to_str().unwrap();
    let r = cli_run(&s(&["adopt", desc.to_str().unwrap(), "--project", p]), &e);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = cli_run(&s(&["trust", "org.example/source-index"]), &e);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    let r = cli_run(&s(&["resolve", "--project", p]), &e);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(
        r.stdout
            .contains("context.repository selected org.example/source-index"),
        "{}",
        r.stdout
    );

    let res = resolve_project(&e, &project).unwrap();
    let launch = &res.launches[&CapabilityKind::ContextRepository];
    check_grant_current(&e, &launch.grant).unwrap();
    let m = AdapterManager::new(HostConfig::default());
    let h = m
        .prepare(
            "p1",
            spec(launch, Some(python()), &project, &tmp, launch.grant.clone()),
        )
        .unwrap();
    let req = request(
        CapabilityKind::ContextRepository,
        "search",
        json!({"query": "journey_symbol"}),
        "inv-1",
        "p1",
    );
    let Outcome::Completed(env) = h.invoke(&req, InvocationClass::SafeRead, &CancelToken::new())
    else {
        panic!("not completed")
    };
    let items = env.payload.unwrap()["items"].as_array().unwrap().clone();
    assert!(items.iter().any(|i| i["path"] == "src/lib.rs"));
    assert_eq!(env.provenance.provider_id, "org.example/source-index");

    // Same host, Rust adapter (native) with the same request/result behaviour.
    let rust_dir = tmp.join("rust");
    std::fs::create_dir_all(&rust_dir).unwrap();
    let rdesc = rust_example(&rust_dir);
    let r = cli_run(&s(&["adopt", rdesc.to_str().unwrap(), "--project", p]), &e);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    // Its bundled upstream cannot be trusted through `trust`; the conformance
    // runner derives the temporary grant instead and drives the same host.
    let rr = go(&opts(&rdesc, &["context"], None));
    assert_clean(&rr, "context.repository");
    let nr = go(&opts(
        &examples().join("decision-node/harness-provider.json"),
        &["decision"],
        Some(node()),
    ));
    assert_clean(&nr, "decision.evaluate");
    m.shutdown_all();
}

#[test]
fn hp_hp16b_adapters_do_not_enter_the_default_dependency_graph() {
    let root = repo_root();
    let a = root.join("packages/semaprax-harness-adapters");
    assert!(!a.join("Cargo.toml").exists() && !a.join("package.json").exists());
    let cargo = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(!cargo.contains("semaprax-harness-adapters"));
}
