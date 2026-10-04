//! HP-09 real-RTK evidence through the host's automatic command-view route.
//! Needs HARNESS_RTK (pinned rtk 0.51.0) and HARNESS_PYTHON.
//!
//! The shipped RTK descriptor declares operation `plan` and its `view` takes
//! `stdout_b64`; contract v1 admits neither (HPA022 / HPA040). These tests run
//! the real adapter module and real rtk behind a tiny test-only shim that
//! declares `view` only and translates the contract payload to the adapter's
//! fields. The host code under test is unchanged.

use crate::support::{fixture_dir, repo_root, required_tool, write};
use semaprax_harness::cli::{run, Environment, Outcome};
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const ID: &str = "ai.rtk/rtk-command-view";

const SHIM: &str = r#"import base64, os, sys
sys.path.insert(0, "@SDK@")
sys.path.insert(0, "@RTK@")
import adapter as rtk
from semaprax_harness_adapter import serve

def view(req):
    p = req["payload"]
    q = dict(req)
    q["payload"] = {"argv": p["argv"], "stdout_b64": base64.b64encode(p["stdout"].encode()).decode(),
                    "stderr_b64": base64.b64encode(p["stderr"].encode()).decode()}
    return rtk.view(q)

serve([{"kind": "command.view", "version": 1, "operations": ["view"]}], {("command.view", "view"): view},
      {"provider_id": rtk.PROVIDER, "adapter_version": "0.1.0", "upstream_version": rtk.PINNED_VERSION})
"#;

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

struct Fx {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    rtk: PathBuf,
    env: Environment,
}

fn policy(python: &Path, min_bytes: u64) -> String {
    format!(
        r#"{{"schema":"semaprax.harness-command-view-policy.v1","min_bytes":{min_bytes},"runtimes":{{"python":"{}"}},"retention":{{"enabled":true,"ttl_secs":3600,"max_bytes":268435456}}}}"#,
        python.display()
    )
}

impl Fx {
    fn new(min_bytes: u64, adopt: bool) -> Fx {
        let python = required_tool("HARNESS_PYTHON");
        let pinned = required_tool("HARNESS_RTK");
        let root = fixture_dir("hp-hp08-rtk").canonicalize().unwrap();
        let home = root.join("home");
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        write(&home, "command-view.json", &policy(&python, min_bytes));
        // Adopt a private copy of the pinned binary so tests can remove it.
        let rtk = root.join("tools/rtk");
        std::fs::create_dir_all(rtk.parent().unwrap()).unwrap();
        std::fs::copy(&pinned, &rtk).unwrap();
        let rtk_dir = repo_root().join("packages/semaprax-harness-adapters/rtk");
        let sdk = repo_root().join("packages/semaprax-harness-adapters/sdk/python");
        write(
            &root,
            "adapter/shim.py",
            &SHIM
                .replace("@SDK@", sdk.to_str().unwrap())
                .replace("@RTK@", rtk_dir.to_str().unwrap()),
        );
        let mut d: Value =
            serde_json::from_slice(&std::fs::read(rtk_dir.join("harness-provider.json")).unwrap())
                .unwrap();
        d["adapter"]["entry"] = serde_json::json!(["shim.py"]);
        d["capabilities"][0]["operations"] = serde_json::json!(["view"]);
        write(&root, "adapter/harness-provider.json", &d.to_string());
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        vars.insert(
            "HOME".to_string(),
            root.join("userhome").display().to_string(),
        );
        let env = Environment {
            harness_home: Some(home.clone()),
            compiler: None,
            cwd: project.clone(),
            vars,
        };
        let fx = Fx {
            root,
            home,
            project,
            rtk,
            env,
        };
        if adopt {
            let desc = fx.root.join("adapter/harness-provider.json");
            let o = run(
                &s(&[
                    "adopt",
                    desc.to_str().unwrap(),
                    "--upstream",
                    fx.rtk.to_str().unwrap(),
                ]),
                &fx.env,
            );
            assert_eq!(o.code, 0, "adopt: {}", o.stderr);
            let o = run(&s(&["trust", ID]), &fx.env);
            assert_eq!(o.code, 0, "trust: {}", o.stderr);
        }
        fx
    }

    fn counter(&self) -> PathBuf {
        self.root.join("counter")
    }

    fn count(&self) -> u32 {
        std::fs::read_to_string(self.counter()).map_or(0, |t| t.trim().parse().unwrap())
    }

    fn script(&self, name: &str, body: &str) -> String {
        let p = write(&self.root, &format!("bin/{name}"), &format!("#!/bin/sh\nf=\"{}\"\nn=$(cat \"$f\" 2>/dev/null || echo 0)\necho $((n+1)) > \"$f\"\n{body}\n", self.counter().display()));
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_str().unwrap().to_string()
    }

    /// Fake `cargo` printing libtest-shaped output; `$1` after `test` is the exit code.
    fn cargo(&self, body: &str) -> String {
        self.script("cargo", body)
    }

    fn exec(&self, flags: &[&str], argv: &[&str]) -> (Outcome, Value) {
        let mut a = vec!["exec", self.project.to_str().unwrap(), "--json"];
        a.extend(flags);
        a.push("--");
        a.extend(argv);
        let o = run(&s(&a), &self.env);
        let v = serde_json::from_str(&o.stdout).unwrap_or(Value::Null);
        (o, v)
    }

    fn git_repo(&self) {
        let g = |args: &[&str]| {
            let o = Command::new("/usr/bin/git")
                .args(args)
                .current_dir(&self.project)
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("HOME", self.root.join("userhome"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "t")
                .env("GIT_AUTHOR_EMAIL", "t@e.x")
                .env("GIT_COMMITTER_NAME", "t")
                .env("GIT_COMMITTER_EMAIL", "t@e.x")
                .output()
                .unwrap();
            assert!(
                o.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&o.stderr)
            );
        };
        std::fs::create_dir_all(self.root.join("userhome")).unwrap();
        g(&["init", "-q"]);
        for i in 0..40 {
            write(
                &self.project,
                &format!("src/f{i}.txt"),
                &(0..30)
                    .map(|l| format!("line {l} of file {i}\n"))
                    .collect::<String>(),
            );
        }
        g(&["add", "."]);
        g(&["commit", "-q", "-m", "one"]);
        for i in 0..40 {
            write(
                &self.project,
                &format!("src/f{i}.txt"),
                &(0..30)
                    .map(|l| format!("line {l} of file {i} changed\n"))
                    .collect::<String>(),
            );
        }
    }
}

/// Run `argv` directly (no host, no rtk) for ground truth.
fn direct(fx: &Fx, argv: &[&str]) -> (i32, String, String) {
    let o = Command::new(argv[0])
        .args(&argv[1..])
        .current_dir(&fx.project)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", fx.root.join("userhome"))
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        semaprax_harness::json::sha256_plain(&o.stdout),
        semaprax_harness::json::sha256_plain(&o.stderr),
    )
}

fn agree(v: &Value, truth: &(i32, String, String)) {
    assert_eq!(v["result"]["status"], format!("exit:{}", truth.0));
    assert_eq!(
        v["result"]["stdout"]["digest"],
        truth.1.as_str(),
        "stdout digest equals unwrapped run"
    );
    assert_eq!(
        v["result"]["stderr"]["digest"],
        truth.2.as_str(),
        "stderr digest equals unwrapped run"
    );
}

const TEST_OUT: &str = "echo 'running 400 tests'; i=0; while [ $i -lt 400 ]; do echo \"test mod::case_$i ... ok\"; i=$((i+1)); done\n\
echo 'test mod::bad_case ... FAILED'; echo; echo 'failures:'; echo; echo '---- mod::bad_case stdout ----'\n\
echo 'assertion failed: CRITICAL-PLANTED-7731'; echo; echo 'failures:'; echo '    mod::bad_case'; echo\n\
echo 'test result: FAILED. 400 passed; 1 failed; 0 ignored'\n\
j=0; while [ $j -lt 800 ]; do echo \"   Compiling crate-$j v0.1.0\" >&2; j=$((j+1)); done; echo 'error: CRITICAL-STDERR-55 aborting' >&2\n\
exit ${2:-101}";

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn rtk_is_used_automatically_for_build_test_with_authoritative_outcome() {
    let fx = Fx::new(1024, true);
    let cargo = fx.cargo(TEST_OUT);
    let obs = fx.root.join("obs.jsonl");
    let (o, v) = fx.exec(
        &["--observations", obs.to_str().unwrap()],
        &[&cargo, "test", "101"],
    );
    assert_eq!(o.code, 101, "{}", o.stderr);
    assert_eq!(fx.count(), 1);
    assert_eq!(v["view"]["route"], "provider", "{v}");
    assert_eq!(v["view"]["provenance"], ID);
    let text = v["view"]["text"].as_str().unwrap();
    assert!(
        text.contains("CRITICAL-PLANTED-7731"),
        "planted failure survives: {text}"
    );
    assert!(
        text.contains("CRITICAL-STDERR-55"),
        "stderr critical line kept or re-attached"
    );
    // Same authoritative outcome as the unwrapped command (second run, counter 2).
    let truth = direct(&fx, &[&cargo, "test", "101"]);
    assert_eq!(truth.0, 101);
    agree(&v, &truth);
    // Measured at the final display boundary, byte-only, separate from RTK's own gain claims.
    let line: Value = serde_json::from_str(
        std::fs::read_to_string(&obs)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(line["provider"], ID);
    assert_eq!(line["after"]["tokenizer"]["kind"], "byte_only");
    let (before, after) = (
        line["before"]["value"].as_u64().unwrap(),
        line["after"]["value"].as_u64().unwrap(),
    );
    println!("rtk cargo-test family: raw {before} B -> display {after} B (byte_only, not tokens)");
    assert!(after < before);
    // Failed-run retention: recoverable without re-running.
    let h = v["result"]["recovery_handle"].as_str().unwrap();
    let r = run(
        &s(&[
            "recover",
            fx.project.to_str().unwrap(),
            h,
            "--limit",
            "1000000",
        ]),
        &fx.env,
    );
    assert!(r.stdout.contains("test mod::case_399 ... ok"));
    assert_eq!(fx.count(), 2, "only the ground-truth run added a count");
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn successful_run_retention_and_malformed_utf8() {
    let fx = Fx::new(1024, true);
    let cargo = fx.cargo("echo 'running 300 tests'; i=0; while [ $i -lt 300 ]; do echo \"test mod::case_$i ... ok\"; i=$((i+1)); done\nprintf 'bad bytes \\377\\376 here\\n'\necho 'test result: ok. 300 passed; 0 failed'\nexit 0");
    let (o, v) = fx.exec(&[], &[&cargo, "test"]);
    assert_eq!(o.code, 0);
    assert_eq!(v["view"]["route"], "provider", "{v}");
    assert_eq!(
        v["view"]["lossless"], false,
        "undecodable bytes are accounted as omissions"
    );
    assert!(v["view"]["omissions"].as_u64().unwrap() >= 1);
    agree(&v, &direct(&fx, &[&cargo, "test"]));
    let h = v["result"]["recovery_handle"].as_str().unwrap();
    let r = run(
        &s(&[
            "recover",
            fx.project.to_str().unwrap(),
            h,
            "--limit",
            "1000000",
            "--json",
        ]),
        &fx.env,
    );
    assert!(
        r.stdout.contains("test mod::case_150 ... ok"),
        "successful output is retained too"
    );
    assert_eq!(fx.count(), 2);
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn rtk_is_used_for_repository_reads() {
    let fx = Fx::new(1024, true);
    fx.git_repo();
    let (o, v) = fx.exec(&[], &["/usr/bin/git", "diff"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(v["view"]["route"], "provider", "{v}");
    assert_eq!(v["view"]["provenance"], ID);
    agree(&v, &direct(&fx, &["/usr/bin/git", "diff"]));
    let (_, v) = fx.exec(&[], &["/usr/bin/git", "diff", "--name-only"]);
    assert_eq!(
        v["view"]["route"], "raw",
        "machine-shaped output stays raw: {v}"
    );
    let rg = Path::new("/opt/homebrew/bin/rg");
    if rg.exists() {
        let (_, v) = fx.exec(
            &[],
            &[
                rg.to_str().unwrap(),
                "-n",
                "--sort",
                "path",
                "changed",
                "src",
            ],
        );
        assert_eq!(v["view"]["route"], "provider", "{v}");
        agree(
            &v,
            &direct(
                &fx,
                &[
                    rg.to_str().unwrap(),
                    "-n",
                    "--sort",
                    "path",
                    "changed",
                    "src",
                ],
            ),
        );
    }
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn interruption_is_uncertain_and_bypasses_rtk() {
    let fx = Fx::new(1024, true);
    let cargo = fx.cargo("echo 'running 1 test'; echo 'error: partial' >&2; sleep 30 & sleep 30");
    let (o, v) = fx.exec(&["--timeout-ms", "2500"], &[&cargo, "test"]);
    assert_eq!(o.code, 124);
    assert_eq!(v["result"]["status"], "timeout");
    assert_eq!(v["result"]["status_certain"], false);
    assert_eq!(v["view"]["incomplete"], true);
    assert_eq!(v["view"]["route"], "raw");
    assert_eq!(fx.count(), 1);
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn each_route_runs_the_command_exactly_once() {
    let fx = Fx::new(1024, true);
    fx.git_repo();
    // Existing RTK wrapper in argv: no double interception.
    let rtk = fx.rtk.to_str().unwrap();
    let (_, v) = fx.exec(&[], &[rtk, "git", "diff"]);
    assert_eq!(v["view"]["route"], "raw", "{v}");
    assert!(
        v["view"]["notes"].to_string().contains("provider failed"),
        "adapter reports already-wrapped: {v}"
    );
    assert_eq!(v["result"]["executions"], 1);
    // An existing hook elsewhere owns rewriting.
    let cargo = fx.cargo(TEST_OUT);
    let (_, v) = fx.exec(&["--external-owner", "agent-hook"], &[&cargo, "test"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("external owner"));
    assert_eq!(fx.count(), 1);
    // Small output bypasses.
    let (_, v) = fx.exec(&[], &["/usr/bin/git", "log", "-1"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("small output"));
    // Unsupported command: the adapter's allowlist declines.
    let big = fx.script(
        "mytool",
        "i=0; while [ $i -lt 600 ]; do echo \"some output line $i\"; i=$((i+1)); done",
    );
    let (_, v) = fx.exec(&[], &[&big]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(
        v["view"]["notes"].to_string().contains("provider failed"),
        "{v}"
    );
    assert_eq!(fx.count(), 2, "cargo once, mytool once");
    // Escape hatch.
    let (_, v) = fx.exec(&["--raw"], &["/usr/bin/git", "diff"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("raw-requested"));
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn absent_rtk_falls_back_unless_required() {
    let fx = Fx::new(1024, true);
    std::fs::remove_file(&fx.rtk).unwrap();
    let cargo = fx.cargo(TEST_OUT);
    let (o, v) = fx.exec(&[], &[&cargo, "test"]);
    assert_eq!(o.code, 101);
    assert_eq!(v["view"]["route"], "raw", "{v}");
    assert_eq!(fx.count(), 1);
    // Nothing was installed or configured on the way.
    assert!(!fx.rtk.exists());
    assert!(
        !fx.root.join("userhome").exists()
            || std::fs::read_dir(fx.root.join("userhome"))
                .unwrap()
                .next()
                .is_none()
    );
    // `required`: refused before launch with a clear error.
    write(&fx.project, "semaprax.harness.toml", "schema = \"semaprax.harness-config.v1\"\n[capability.\"command.view\"]\nmode = \"required\"\n");
    let (o, _) = fx.exec(&[], &[&cargo, "test"]);
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HP"), "{}", o.stderr);
    assert_eq!(
        fx.count(),
        1,
        "required-but-missing did not launch the command"
    );
    // Never adopted at all: also raw.
    let fresh = Fx::new(1024, false);
    let cargo = fresh.cargo(TEST_OUT);
    let (_, v) = fresh.exec(&[], &[&cargo, "test"]);
    assert_eq!(v["view"]["route"], "raw");
    assert_eq!(fresh.count(), 1);
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn negative_saving_case_is_kept_in_the_evidence() {
    let fx = Fx::new(0, true);
    fx.git_repo();
    let rg = "/opt/homebrew/bin/rg";
    if !Path::new(rg).exists() {
        return;
    }
    let obs = fx.root.join("obs.jsonl");
    // One tiny match plus a stderr line: RTK's stream header and the envelope outweigh the raw bytes.
    let argv = [
        rg,
        "-n",
        "line 3 of file 1 ",
        "src/f1.txt",
        "src/missing.txt",
    ];
    let (o, v) = fx.exec(&["--observations", obs.to_str().unwrap()], &argv);
    assert_eq!(o.code, 2, "rg exits 2 when one path is missing");
    assert_eq!(v["result"]["status"], "exit:2");
    agree(&v, &direct(&fx, &argv));
    let line: Value = serde_json::from_str(
        std::fs::read_to_string(&obs)
            .unwrap()
            .lines()
            .next()
            .unwrap(),
    )
    .unwrap();
    let (before, after) = (
        line["before"]["value"].as_u64().unwrap(),
        line["after"]["value"].as_u64().unwrap(),
    );
    println!("negative-saving case: raw {before} B -> display {after} B");
    assert!(
        after > before,
        "display is larger than raw; the loss is recorded, not hidden"
    );
    assert!(v["result"]["recovery_handle"].is_string());
}
