//! MR-12 cross-language cell: the Rust host adopts the bundled laya-local
//! adapter from the checkout through the normal adopt -> trust -> resolve ->
//! negotiate path, negotiates `decision.evaluate` v2 and routes against the
//! Python loopback fake Laya server (`systemone/tests/fake_servers.py`). The
//! rendered digest round-trips, the typed call metadata lands in the
//! decision, and an identical second decision is a session cache hit with no
//! new adapter or upstream call. Needs python3 (HARNESS_PYTHON or PATH).

use super::*;
use crate::support::{fixture_dir, repo_root, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::workflow::decision_open::open_decision;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

const ID: &str = "ai.convai/laya-decision";

fn python() -> PathBuf {
    std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join("python3"))
                .find(|p| p.is_file())
        })
        .expect("python3 not found: the laya-local cross-language cell cannot run here")
}

/// The Python fake Laya server, alive for the test.
struct FakeLaya {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    port: u16,
}

impl FakeLaya {
    fn start() -> Self {
        let script =
            repo_root().join("packages/semaprax-harness-adapters/systemone/tests/serve_fake.py");
        let mut child = Command::new(python())
            .arg(script)
            .arg("laya")
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

    /// Upstream inference POSTs the fake has answered.
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

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

#[test]
fn mr12_laya_local_adapter_negotiates_v2_routes_and_a_repeat_is_a_session_cache_hit() {
    let mut fake = FakeLaya::start();
    let root = fixture_dir("hp-mr12").canonicalize().unwrap();
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let env = Environment {
        harness_home: Some(home),
        compiler: None,
        cwd: root.clone(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    // Adopt the bundled adapter from the checkout path (no copy, no install).
    let desc = repo_root()
        .join("packages/semaprax-harness-adapters/systemone/laya-local/harness-provider.json");
    let py = python();
    let o = run(
        &s(&[
            "adopt",
            desc.to_str().unwrap(),
            "--runtime",
            py.to_str().unwrap(),
        ]),
        &env,
    );
    assert_eq!(o.code, 0, "adopt: {}", o.stderr);
    let o = run(&s(&["trust", ID]), &env);
    assert_eq!(o.code, 0, "trust: {}", o.stderr);
    let profile = json!({"profile_id": "laya-multilingual", "model": "multilingual", "checkpoint": "multilingual",
        "identity_kind": "local_declared", "score_kind": "option_distribution", "scoreless": false,
        "max_options": 16, "max_state_bytes": 4096, "modalities": ["text"]});
    let project = root.join("proj");
    write(
        &project,
        "semaprax.toml",
        "schema = \"semaprax.manifest.v1\"\n",
    );
    write(&project, "src/lib.spx", "module app\n");
    write(
        &project,
        "semaprax.harness.toml",
        &format!(
            "schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n[capability.\"decision.evaluate\".config]\nmodel_profile = '{profile}'\ninstance_id = \"laya-e2e\"\nendpoint = \"http://127.0.0.1:{}\"\n",
            fake.port
        ),
    );
    let mut o = open_decision(&env, &project, Some(&py), &root.join("cache")).unwrap();
    assert_eq!(o.profile.instance.as_ref().unwrap().instance_id, "laya-e2e");
    let i = inputs(RouteSignals::default());
    let mut c = ctx();
    c.project = o.binding.clone();
    c.lock_digest = o.lock_digest.clone();
    let cfg = rc(RoutingMode::Experimental);
    let prof = o.profile.clone();
    let mut sd = SessionDecisions::default();

    let (a, ra) = go(&mut sd, &cfg, &i, &c, &prof, &mut o.invoker, &|| i.clone()).unwrap();
    let d = &a.decision;
    assert_eq!(d.source, DecisionSource::Provider, "{:?}", d.wire.note);
    assert_eq!(d.wire.version, 2, "negotiated decision.evaluate v2");
    let call = d.wire.call.as_ref().expect("typed call metadata");
    assert_eq!(
        Some(call.rendered_digest.as_str()),
        d.wire.rendered_digest.as_deref(),
        "digest round-trips"
    );
    assert_eq!(call.identity_kind, IdentityKind::LocalDeclared);
    assert_eq!(call.billing, Billing::Local);
    assert_eq!(call.adapter, "ai.convai/laya-decision@0.2.0");
    assert_eq!(call.checkpoint.as_deref(), Some("multilingual"));
    assert!(call.wire_bytes <= d.wire.max_wire_bytes.unwrap());
    assert_eq!((ra.outcome, ra.router_calls), ("miss", 1));
    assert_eq!((o.invoker.calls, fake.posts()), (1, 1));

    let (b, rb) = go(&mut sd, &cfg, &i, &c, &prof, &mut o.invoker, &|| i.clone()).unwrap();
    assert_eq!(b.decision.source, DecisionSource::Cache);
    assert_eq!(b.decision.choice, d.choice);
    assert_eq!(
        (rb.outcome, rb.router_calls, rb.readiness_before),
        ("hit", 0, Readiness::Warm)
    );
    assert_eq!(
        (o.invoker.calls, fake.posts()),
        (1, 1),
        "zero new adapter or upstream calls"
    );
}
