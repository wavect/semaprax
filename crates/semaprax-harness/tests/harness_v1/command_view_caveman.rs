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
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

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
        let fake = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/caveman/fake_runtime.py"),
        )
        .unwrap();
        write(&root, "tools/fake_runtime.py", &fake);
        let up = write(
            &root,
            "tools/caveman",
            &format!(
                "#!/bin/sh\nexec \"{}\" \"{}/tools/fake_runtime.py\" \"$@\"\n",
                python().display(),
                root.display()
            ),
        );
        std::fs::set_permissions(&up, std::fs::Permissions::from_mode(0o755)).unwrap();
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
        let fx = Fx { root, project, env };
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
    let fx = Fx::new(true, "");
    let noisy = fx.noisy();
    let tiny = fx.script("tiny.sh", "echo \"ERROR: small 1\"");
    let diff = fx.script(
        "diff.sh",
        "echo 'diff --git a/x b/x'; i=0; while [ $i -lt 400 ]; do echo \"+line $i of patch text here\"; i=$((i+1)); done",
    );
    let cases: [(&str, &str, &str, &str); 6] = [
        ("compress", &tiny, "tiny", "small output"),
        ("compress", &diff, "unsupported-format", "unsupported"),
        ("record", &noisy, "record-only", "unsupported"),
        ("grow", &noisy, "enlarged", "unsupported"),
        ("drop_error", &noisy, "dropped-error", "failed"),
        ("compress", &noisy, "missing-runtime", "unavailable"),
    ];
    for (mode, cmd, label, status) in cases {
        fx.mode(mode);
        if label == "missing-runtime" {
            std::fs::remove_file(fx.root.join("tools/caveman")).unwrap();
        }
        let before = fx.runtime_calls("input-compress");
        let r = fx.exec(cmd, true);
        let v = &r.envelope.view;
        assert_eq!(v.route, "raw", "{label}: {:?}", v.notes);
        let m = v.measurement.clone().unwrap();
        assert!(
            m.saved_tokens().is_none_or(|t| t == 0),
            "{label}: raw delivered, nothing saved"
        );
        assert!(!v.lossless || v.omissions == 0);
        // An ineligible (missing) runtime resolves to no provider at all.
        assert!(
            label == "missing-runtime"
                || v.notes
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
        let calls = fx.runtime_calls("input-compress") - before;
        if label == "tiny" || label == "missing-runtime" {
            assert_eq!(calls, 0, "{label}: runtime not consulted");
        }
    }
}

#[test]
fn caveman_runs_with_a_closed_loopback_telemetry_off_environment_and_refuses_egress() {
    let d: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            repo_root().join("packages/semaprax-harness-adapters/caveman/harness-provider.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(d["permissions"]["network"], serde_json::json!([]));
    assert_eq!(d["permissions"]["secrets"], serde_json::json!([]));
    assert_eq!(d["capabilities"][0]["required"], false);
    let fx = Fx::new(true, "");
    let cmd = fx.noisy();
    let r = fx.exec(&cmd, true);
    assert_eq!(r.envelope.view.route, "provider");
    let env: BTreeMap<String, String> =
        serde_json::from_slice(&std::fs::read(fx.root.join("tools/env.json")).unwrap()).unwrap();
    assert_eq!(env["CAVEMAN_TELEMETRY"], "0");
    assert_eq!(env["DO_NOT_TRACK"], "1");
    assert_eq!(env["CAVEMAN_BIND"], "127.0.0.1");
    assert_eq!(env["CAVEMAN_OFFLINE"], "1");
    assert!(
        !env.contains_key("SECRET_TOKEN"),
        "no ambient secret reaches the runtime"
    );
    assert!(
        env["HOME"].contains("caveman-home"),
        "state is private, not the user's home"
    );
    // A runtime that reports telemetry or a non-loopback bind is refused.
    for mode in ["telemetry_on", "egress_bind"] {
        fx.mode(mode);
        let r = fx.exec(&cmd, true);
        assert_eq!(r.envelope.view.route, "raw", "{mode}");
        assert!(
            r.envelope
                .view
                .notes
                .iter()
                .any(|n| n.contains("raw view used")),
            "{mode}"
        );
    }
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
    assert_eq!(fx.runtime_calls("input-compress"), 0);
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
    assert_eq!(fx.runtime_calls("input-compress"), 0);
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
