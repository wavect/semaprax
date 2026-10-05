//! TC-07: Caveman input compression as an opt-in `command.view` provider
//! (fixture prefix `hp-tc07`). The shipped descriptor and adapter are adopted
//! unmodified; the upstream is a fake host-provisioned runtime emulating the
//! pinned `input-compress` CLI. No network and no live upstream are used.
//! The paid raw/RTK/Caveman comparison is TC-12's job, not asserted here.

use crate::support::{fixture_dir, repo_root, write};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::command_view::retention::StreamName;
use semaprax_harness::command_view::{execute, recover_by_id, ExecOptions, ViewTokenizer};
use semaprax_harness::observe::Tokenizer;
use semaprax_harness::workflow::stages::CommandStage;
use semaprax_harness::workflow::{CheckSpec, HostCommandChecks};
use std::collections::BTreeMap;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const ID: &str = "ai.caveman/caveman-command-view";

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn python() -> PathBuf {
    if let Some(p) = std::env::var_os("HARNESS_PYTHON") {
        return PathBuf::from(p);
    }
    let out = std::process::Command::new("/usr/bin/which")
        .arg("python3")
        .output()
        .expect("which");
    PathBuf::from(String::from_utf8_lossy(&out.stdout).trim())
}

struct Words;
impl Tokenizer for Words {
    fn name(&self) -> &str {
        "test-words"
    }
    fn fingerprint(&self) -> &str {
        "t1"
    }
    fn count(&self, text: &str) -> usize {
        text.split_whitespace().count()
    }
}

fn vt() -> Option<ViewTokenizer> {
    Some(ViewTokenizer(std::rc::Rc::new(Words)))
}

struct Fx {
    root: PathBuf,
    project: PathBuf,
    env: Environment,
    server: Option<std::process::Child>,
}

impl Drop for Fx {
    fn drop(&mut self) {
        if let Some(c) = self.server.as_mut() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

/// Raw stdout of the noisy command: 2000 repeats, one decisive error, 500 repeats.
fn raw_stdout() -> String {
    format!(
        "{}ERROR: decisive failure 7731\n{}",
        "repetitive noise line\n".repeat(2000),
        "more noise\n".repeat(500)
    )
}

impl Fx {
    fn new(adopt: bool, policy_extra: &str) -> Fx {
        let root = fixture_dir("hp-tc07").canonicalize().unwrap();
        let home = root.join("home");
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        write(
            &home,
            "command-view.json",
            &format!(
                r#"{{"schema":"semaprax.harness-command-view-policy.v1","runtimes":{{"python":"{}"}},"retention":{{"enabled":true,"ttl_secs":3600,"max_bytes":67108864}}{policy_extra}}}"#,
                python().display()
            ),
        );
        let fake =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/caveman/fake_runtime.py");
        // The adopted upstream only answers the identity probe; the runtime is a separate loopback server.
        let up = write(
            &root,
            "tools/caveman",
            "#!/bin/sh\necho \"caveman 3.1.0\"\n",
        );
        std::fs::set_permissions(&up, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::create_dir_all(root.join("tools")).unwrap();
        let mut server = std::process::Command::new(python())
            .arg(&fake)
            .arg(root.join("tools"))
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let portf = root.join("tools/port");
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        loop {
            if std::fs::read_to_string(&portf).is_ok_and(|port| !port.is_empty()) {
                break;
            }
            if let Some(status) = server.try_wait().unwrap() {
                let mut stderr = String::new();
                server
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                panic!(
                    "fake Caveman runtime exited before publishing its port ({status}): {stderr}"
                );
            }
            if std::time::Instant::now() >= deadline {
                let _ = server.kill();
                let _ = server.wait();
                panic!("fake Caveman runtime did not publish its port within 60 seconds");
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        vars.insert("HARNESS_PYTHON".to_string(), python().display().to_string());
        vars.insert("SECRET_TOKEN".to_string(), "hunter2".to_string());
        let env = Environment {
            harness_home: Some(home),
            compiler: None,
            cwd: project.clone(),
            vars,
        };
        let mut fx = Fx {
            root,
            project,
            env,
            server: Some(server),
        };
        fx.provision_endpoint();
        if adopt {
            let d = repo_root()
                .join("packages/semaprax-harness-adapters/caveman/harness-provider.json");
            let o = run(
                &s(&[
                    "adopt",
                    d.to_str().unwrap(),
                    "--upstream",
                    fx.root.join("tools/caveman").to_str().unwrap(),
                ]),
                &fx.env,
            );
            assert_eq!(o.code, 0, "adopt: {}", o.stderr);
            let o = run(&s(&["trust", ID]), &fx.env);
            assert_eq!(o.code, 0, "trust: {}", o.stderr);
        }
        fx
    }

    /// Host provisioning: point the adapter at the fixture server (loopback, ephemeral port).
    fn provision_endpoint(&mut self) {
        let port = std::fs::read_to_string(self.root.join("tools/port")).unwrap();
        self.provision(&format!("127.0.0.1:{port}"));
    }

    fn provider_dir(&self) -> PathBuf {
        let pid = semaprax_harness::json::sha256_plain(
            self.project
                .canonicalize()
                .unwrap()
                .to_string_lossy()
                .as_bytes(),
        );
        let dir = self
            .env
            .harness_home
            .clone()
            .unwrap()
            .join("retention")
            .join(pid.trim_start_matches("sha256:"))
            .join("providers")
            .join("ai.caveman_caveman-command-view");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
        dir
    }

    fn provision(&self, endpoint: &str) {
        std::fs::write(self.provider_dir().join("caveman-endpoint"), endpoint).unwrap();
        std::fs::write(self.provider_dir().join("caveman-token"), "tok-1").unwrap();
    }

    fn stop_runtime(&mut self) {
        if let Some(mut c) = self.server.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    fn mode(&self, m: &str) {
        write(&self.root, "tools/mode.txt", m);
    }

    fn runtime_calls(&self, what: &str) -> usize {
        std::fs::read_to_string(self.root.join("tools/calls.log"))
            .map_or(0, |t| t.lines().filter(|l| *l == what).count())
    }

    fn script(&self, name: &str, body: &str) -> String {
        let p = write(
            &self.root,
            &format!("bin/{name}"),
            &format!(
                "#!/bin/sh\necho x >> \"{}/counter\"\n{body}\n",
                self.root.display()
            ),
        );
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_str().unwrap().to_string()
    }

    fn noisy(&self) -> String {
        self.script(
            "noisy.sh",
            "i=0; while [ $i -lt 2000 ]; do echo \"repetitive noise line\"; i=$((i+1)); done\necho \"ERROR: decisive failure 7731\"\nj=0; while [ $j -lt 500 ]; do echo \"more noise\"; j=$((j+1)); done\necho \"warn on stderr\" >&2\nexit ${1:-0}",
        )
    }

    fn count(&self) -> usize {
        std::fs::read_to_string(self.root.join("counter")).map_or(0, |t| t.lines().count())
    }

    fn exec(&self, cmd: &str, tk: bool) -> semaprax_harness::command_view::ExecReport {
        execute(
            &self.env,
            &self.project,
            &s(&[cmd, "0"]),
            &ExecOptions {
                tokenizer: if tk { vt() } else { None },
                ..Default::default()
            },
            None,
        )
        .unwrap()
    }
}

#[test]
fn caveman_command_runs_once_whether_the_adapter_succeeds_fails_times_out_or_returns_garbage() {
    let fx = Fx::new(true, r#","provider_timeout_ms":1500"#);
    let cmd = fx.noisy();
    let mut n = 0;
    for (mode, route) in [
        ("compress", "provider"),
        ("crash", "raw"),
        ("hang", "raw"),
        ("garbage", "raw"),
    ] {
        fx.mode(mode);
        let r = fx.exec(&cmd, false);
        n += 1;
        assert_eq!(fx.count(), n, "{mode}: command executed exactly once");
        assert_eq!(r.envelope.result.executions, 1, "{mode}");
        assert_eq!(
            r.envelope.view.route, route,
            "{mode}: {:?}",
            r.envelope.view.notes
        );
        assert!(
            r.envelope.view.text.contains("decisive failure 7731"),
            "{mode}"
        );
        assert!(r.envelope.view.recovery_handle.is_some() || route == "raw");
    }
}

#[test]
fn caveman_keeps_planted_errors_saves_named_tokens_and_raw_is_exactly_recoverable() {
    let fx = Fx::new(true, "");
    let cmd = fx.noisy();
    let mut st = HostCommandChecks::new(fx.env.clone());
    st.tokenizer = vt();
    let mut obs = semaprax_harness::observe::Observer::new(
        None,
        semaprax_harness::observe::ObserverLimits::default(),
    );
    let run = st
        .run_check(
            &CheckSpec {
                name: "unit".into(),
                argv: s(&[&cmd, "1"]),
            },
            &fx.project,
            &mut obs,
        )
        .unwrap()
        .unwrap();
    assert_eq!((run.passed, run.executions, fx.count()), (false, 1, 1));
    assert_eq!(run.view_route, "provider");
    assert_eq!(run.view_provenance, ID);
    assert!(run.view.contains("ERROR: decisive failure 7731"));
    assert!(run.view.contains("(x2000)"), "Caveman compressed the noise");
    let m = run.measurement.clone().unwrap();
    assert_eq!((m.basis, m.decision), ("tokens", "provider-smaller"));
    assert!(m.delivered_tokens.unwrap() < m.raw_tokens.unwrap());
    assert!(m.saved_tokens().unwrap() > 0);
    // The consuming path recovers the exact raw bytes, in bounded pages, without rerunning.
    let raw = raw_stdout();
    let mut got = String::new();
    let mut off = 0;
    loop {
        let r = st.recover_raw(&run, StreamName::Stdout, off, 4096).unwrap();
        assert!(r.bytes <= 4096);
        got.push_str(&r.text);
        match r.next_offset {
            Some(n) => off = n,
            None => break,
        }
    }
    assert_eq!(got, raw);
    let e = st.recover_raw(&run, StreamName::Stderr, 0, 4096).unwrap();
    assert_eq!(e.text, "warn on stderr\n");
    assert_eq!(fx.count(), 1, "recovery never re-executes");
}

#[test]
fn caveman_fallbacks_deliver_raw_with_no_false_saving() {
    let mut fx = Fx::new(true, "");
    let noisy = fx.noisy();
    let tiny = fx.script("tiny.sh", "echo \"ERROR: small 1\"");
    let diff = fx.script(
        "diff.sh",
        "echo 'diff --git a/x b/x'; i=0; while [ $i -lt 400 ]; do echo \"+line $i of patch text here\"; i=$((i+1)); done",
    );
    let cases: [(&str, &str, &str, &str); 8] = [
        ("compress", &tiny, "tiny", "small output"),
        ("compress", &diff, "unsupported-format", "unsupported"),
        ("record", &noisy, "record-only", "unsupported"),
        (
            "bypassed",
            &noisy,
            "runtime-bypassed-decision",
            "unsupported",
        ),
        ("grow", &noisy, "enlarged", "unsupported"),
        ("drop_error", &noisy, "dropped-error", "failed"),
        ("bad_sha", &noisy, "bad-replacement-digest", "failed"),
        ("compress", &noisy, "missing-runtime", "unavailable"),
    ];
    for (mode, cmd, label, status) in cases {
        fx.mode(mode);
        if label == "missing-runtime" {
            fx.stop_runtime();
        }
        let before = fx.runtime_calls("POST optimize");
        let r = fx.exec(cmd, true);
        let v = &r.envelope.view;
        assert_eq!(v.route, "raw", "{label}: {:?}", v.notes);
        let m = v.measurement.clone().unwrap();
        assert!(
            m.saved_tokens().is_none_or(|t| t == 0),
            "{label}: raw delivered, nothing saved"
        );
        assert!(!v.lossless || v.omissions == 0);
        assert!(
            v.notes
                .iter()
                .any(|n| n.contains("raw view used") || n.contains("provider not consulted")),
            "{label}: {status}: {:?}",
            v.notes
        );
        assert!(
            v.text.contains("ERROR: decisive failure 7731")
                || v.text.contains("ERROR: small 1")
                || v.text.contains("patch text"),
            "{label}"
        );
        let calls = fx.runtime_calls("POST optimize") - before;
        assert!(calls <= 1, "{label}: at most one optimize call");
        if label == "tiny" {
            assert_eq!(calls, 0, "{label}: runtime not consulted");
        }
    }
}

#[test]
fn caveman_sends_captured_output_to_loopback_in_compress_mode_and_refuses_egress() {
    let d: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            repo_root().join("packages/semaprax-harness-adapters/caveman/harness-provider.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        d["permissions"]["network"],
        serde_json::json!(["loopback:127.0.0.1:8787"])
    );
    assert_eq!(d["permissions"]["secrets"], serde_json::json!([]));
    assert_eq!(d["capabilities"][0]["required"], false);
    let fx = Fx::new(true, "");
    let cmd = fx.noisy();
    let r = fx.exec(&cmd, true);
    assert_eq!(r.envelope.view.route, "provider");
    assert!(
        !r.envelope.view.text.contains("caveman_retrieve"),
        "only Semaprax's own recovery reference is shown"
    );
    let sent: serde_json::Value =
        serde_json::from_slice(&std::fs::read(fx.root.join("tools/last_optimize.json")).unwrap())
            .unwrap();
    let (b, h) = (&sent["body"], &sent["headers"]);
    assert_eq!(b["mode"], "compress");
    assert_eq!(b["schema_version"], 1);
    assert_eq!(b["scope"]["namespace"], "semaprax");
    assert_eq!(b["segments"][0]["kind"], "tool_result");
    assert_eq!(b["recovery_binding"]["kind"], "host_tool");
    assert!(b["segments"][0]["content"]
        .as_str()
        .unwrap()
        .starts_with(&raw_stdout()));
    assert_eq!(h["Authorization"], "Bearer tok-1");
    assert_eq!(
        h["Caveman-Middleware-Features"],
        "http_status_v2, revision_tolerant"
    );
    assert!(h.get("Origin").is_none() && h.get("Sec-Fetch-Site").is_none());
    let log = std::fs::read_to_string(fx.root.join("tools/calls.log")).unwrap();
    assert_eq!(fx.runtime_calls("GET capabilities"), 1);
    assert_eq!(
        fx.runtime_calls("POST sessions/delete"),
        1,
        "upstream copy revoked"
    );
    assert_eq!(log.lines().filter(|l| l.contains("receipts")).count(), 0);
    // §2: with no runtime credential no request is made at all.
    std::fs::remove_file(fx.provider_dir().join("caveman-token")).unwrap();
    let before = log.lines().count();
    let r = fx.exec(&cmd, true);
    assert_eq!(r.envelope.view.route, "raw");
    assert!(
        r.envelope.view.notes.iter().any(
            |n| n.contains("provider plan: bypass") && n.contains("credential not provisioned")
        ),
        "{:?}",
        r.envelope.view.notes
    );
    let log2 = std::fs::read_to_string(fx.root.join("tools/calls.log")).unwrap();
    assert_eq!(
        log2.lines().count(),
        before,
        "zero requests without a credential"
    );
    // A wrong credential is refused by the runtime (401) and falls back to raw.
    std::fs::write(fx.provider_dir().join("caveman-token"), "wrong").unwrap();
    assert_eq!(fx.exec(&cmd, true).envelope.view.route, "raw");
    // A non-loopback endpoint is refused without any connection; raw is delivered.
    let n = std::fs::read_to_string(fx.root.join("tools/calls.log"))
        .unwrap()
        .lines()
        .count();
    fx.provision("example.com:80");
    let r = fx.exec(&cmd, true);
    assert_eq!(r.envelope.view.route, "raw");
    assert_eq!(
        std::fs::read_to_string(fx.root.join("tools/calls.log"))
            .unwrap()
            .lines()
            .count(),
        n
    );
}

#[test]
fn caveman_recovery_is_scoped_to_the_task_that_ran_the_command() {
    let fx = Fx::new(true, "");
    let cmd = fx.noisy();
    let r = fx.exec(&cmd, false);
    let h = r.envelope.result.recovery_handle.clone().unwrap();
    let own = semaprax_harness::json::sha256_plain(
        fx.project
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .as_bytes(),
    );
    assert!(recover_by_id(&fx.env, &own, &h, StreamName::Stdout, 0, 64).is_ok());
    let other = semaprax_harness::json::sha256_plain(b"/another/task");
    assert!(recover_by_id(&fx.env, &other, &h, StreamName::Stdout, 0, 64).is_err());
    let o = run(
        &s(&["recover", fx.project.to_str().unwrap(), &h, "--limit", "16"]),
        &fx.env,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
}

#[test]
fn caveman_is_opt_in_unadopted_or_disabled_means_raw_and_the_runtime_is_never_called() {
    let fx = Fx::new(false, "");
    let cmd = fx.noisy();
    let r = fx.exec(&cmd, false);
    assert_eq!(r.envelope.view.route, "raw");
    assert_eq!(fx.runtime_calls("POST optimize"), 0);
    // Adopted, then switched off by explicit project configuration.
    let fx = Fx::new(true, "");
    write(
        &fx.project,
        "semaprax.harness.toml",
        "schema = \"semaprax.harness-config.v1\"\n[capability.\"command.view\"]\nmode = \"disabled\"\n",
    );
    let cmd = fx.noisy();
    let r = fx.exec(&cmd, false);
    assert_eq!(r.envelope.view.route, "raw");
    assert_eq!(fx.runtime_calls("POST optimize"), 0);
    // An explicit pin selects it.
    write(
        &fx.project,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"command.view\"]\nprovider = \"{ID}\"\n"),
    );
    assert_eq!(fx.exec(&cmd, false).envelope.view.route, "provider");
    // `--raw` still bypasses it.
    let r = execute(
        &fx.env,
        &fx.project,
        &s(&[&cmd, "0"]),
        &ExecOptions {
            raw: true,
            ..Default::default()
        },
        None,
    )
    .unwrap();
    assert_eq!(r.envelope.view.route, "raw");
}

/// Replays the exchange recorded from a real Caveman v3.1.0 runtime (fixtures/caveman/recorded/): the adapter
/// must reach the same decision it reached live, keep both planted ERROR lines, and still recover raw exactly.
#[test]
fn caveman_replays_a_recorded_real_runtime_exchange_to_the_same_view() {
    let rec = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/caveman/recorded");
    let ex: serde_json::Value =
        serde_json::from_slice(&std::fs::read(rec.join("exchange.json")).unwrap()).unwrap();
    let live_view = ex["exchanges"][1]["response_body"]["replacements"][0]["text"]
        .as_str()
        .unwrap();
    let live_view = live_view.split_once('\n').unwrap().1; // the Caveman marker is stripped
    let raw = std::fs::read_to_string(rec.join("raw.log")).unwrap();
    let fx = Fx::new(true, "");
    fx.mode("replay");
    let cmd = fx.script(
        "replay.sh",
        &format!("cat '{}'", rec.join("raw.log").display()),
    );
    let r = fx.exec(&cmd, true);
    assert_eq!(r.envelope.view.route, "provider");
    assert_eq!(r.envelope.view.text, live_view);
    for planted in ["decisive failure 7731", "planted failure 4410"] {
        assert!(r.envelope.view.text.contains(planted), "{planted}");
    }
    assert!(r.envelope.view.text.len() < raw.len() / 10);
    assert_eq!(
        (
            fx.runtime_calls("GET capabilities"),
            fx.runtime_calls("POST optimize"),
            fx.runtime_calls("POST sessions/delete")
        ),
        (1, 1, 1)
    );
    // A plan the adapter cannot verify (fake bad_sha mode) still falls back to raw.
    fx.mode("bad_sha");
    let r = fx.exec(&cmd, true);
    assert_ne!(r.envelope.view.route, "provider");
}
