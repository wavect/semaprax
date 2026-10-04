//! HP-09 real-RTK evidence through the host's automatic command-view route.
//! Needs HARNESS_RTK (pinned rtk 0.51.0) and HARNESS_PYTHON.
//!
//! The SHIPPED descriptor `packages/semaprax-harness-adapters/rtk/harness-provider.json`
//! is adopted unmodified through the profile CLI (`adopt`, `trust`); the host then
//! plans (`plan`) and views (`view`) through the real adapter and real rtk, with
//! no test shim and no payload translation.

use crate::support::{fixture_dir, repo_root, required_tool, write};
use semaprax_harness::cli::{run, Environment, Outcome};
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::Command;

const ID: &str = "ai.rtk/rtk-command-view";

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
            let desc =
                repo_root().join("packages/semaprax-harness-adapters/rtk/harness-provider.json");
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
        v["view"]["notes"].to_string().contains("already-wrapped"),
        "adapter plan reports already-wrapped: {v}"
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
        v["view"]["notes"]
            .to_string()
            .contains("unsupported-command"),
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

/// Wait for `path` to appear, then read the pid inside.
fn wait_pid(path: &Path) -> i32 {
    for _ in 0..400 {
        if let Ok(t) = std::fs::read_to_string(path) {
            if let Ok(n) = t.trim().parse() {
                return n;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    panic!("command never reported its pid");
}

/// Signal the whole process group of `pid` (a group leader), like a terminal does.
fn signal_group(sig: &str, pid: i32) {
    let o = Command::new("/bin/sh")
        .args(["-c", &format!("kill -{sig} -- -{pid}")])
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
}

/// Long-running cargo-shaped command that reports its pid once its output is written.
fn sleeper(fx: &Fx, pidf: &Path) -> String {
    let p = pidf.display();
    fx.cargo(&format!(
        "echo 'running 1 test'; echo 'error: partial CRITICAL-SIG-31' >&2; echo $$ > \"{p}.tmp\"; mv \"{p}.tmp\" \"{p}\"; sleep 30"
    ))
}

/// Unwrapped ground truth: the same command, own process group, signalled mid-run.
fn direct_signalled(fx: &Fx, cargo: &str, pidf: &Path, sig: &str) -> (i32, String, String) {
    let _ = std::fs::remove_file(pidf);
    let child = Command::new(cargo)
        .arg("test")
        .current_dir(&fx.project)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .process_group(0)
        .spawn()
        .unwrap();
    signal_group(sig, wait_pid(pidf));
    let o = child.wait_with_output().unwrap();
    (
        o.status.signal().expect("died by signal"),
        semaprax_harness::json::sha256_plain(&o.stdout),
        semaprax_harness::json::sha256_plain(&o.stderr),
    )
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn real_group_sigint_and_sigterm_agree_with_the_unwrapped_outcome() {
    for (sig, num) in [("INT", 2), ("TERM", 15)] {
        let fx = Fx::new(0, true);
        let pidf = fx.root.join("pid");
        let cargo = sleeper(&fx, &pidf);
        let truth = direct_signalled(&fx, &cargo, &pidf, sig);
        assert_eq!(truth.0, num, "unwrapped command dies by SIG{sig}");
        let _ = std::fs::remove_file(&pidf);
        let (env, project) = (fx.env.clone(), fx.project.display().to_string());
        let argv = s(&[
            "exec",
            &project,
            "--json",
            "--timeout-ms",
            "60000",
            "--",
            &cargo,
            "test",
        ]);
        let t = std::thread::spawn(move || run(&argv, &env));
        signal_group(sig, wait_pid(&pidf));
        let o = t.join().unwrap();
        let v: Value = serde_json::from_str(&o.stdout).unwrap();
        assert_eq!(o.code, 128 + num, "{}", o.stderr);
        assert_eq!(v["result"]["status"], format!("signal:{num}"));
        assert_eq!(v["result"]["status_certain"], true);
        assert_eq!(v["result"]["executions"], 1);
        assert_eq!(v["result"]["stdout"]["digest"], truth.1.as_str());
        assert_eq!(v["result"]["stderr"]["digest"], truth.2.as_str());
        assert_eq!(v["view"]["route"], "provider", "{v}");
        assert!(v["view"]["text"]
            .as_str()
            .unwrap()
            .contains("CRITICAL-SIG-31"));
        assert_eq!(fx.count(), 2, "ground truth once, host run once");
    }
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn host_cancel_path_is_uncertain_and_bypasses_rtk() {
    let fx = Fx::new(0, true);
    let pidf = fx.root.join("pid");
    let cargo = sleeper(&fx, &pidf);
    let cancel = semaprax_harness::host::CancelToken::new();
    let (env, project, argv, c2) = (
        fx.env.clone(),
        fx.project.clone(),
        s(&[&cargo, "test"]),
        cancel.clone(),
    );
    let t = std::thread::spawn(move || {
        let opts = semaprax_harness::command_view::ExecOptions {
            cancel: c2,
            timeout_ms: Some(60_000),
            ..Default::default()
        };
        semaprax_harness::command_view::execute(&env, &project, &argv, &opts, None)
    });
    wait_pid(&pidf);
    cancel.cancel();
    let v = t.join().unwrap().unwrap().envelope.to_json();
    assert_eq!(v["result"]["status"], "cancelled");
    assert_eq!(v["result"]["status_certain"], false);
    assert_eq!(v["result"]["executions"], 1);
    assert_eq!(v["view"]["route"], "raw", "{v}");
    assert_eq!(v["view"]["incomplete"], true);
    assert_eq!(fx.count(), 1);
}

fn cargo_path() -> String {
    let o = Command::new("/usr/bin/which")
        .arg("cargo")
        .output()
        .expect("which cargo");
    let p = String::from_utf8_lossy(&o.stdout).trim().to_string();
    assert!(Path::new(&p).is_absolute(), "no cargo on PATH");
    p
}

/// Tiny dependency-free crate: `passing` ok tests and one failing test that
/// reports through `Result` so libtest output carries no thread id.
fn crate_at(dir: &Path, passing: usize) {
    write(
        dir,
        "Cargo.toml",
        "[package]\nname = \"hpdemo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\n",
    );
    let mut lib = String::from("#[cfg(test)]\nmod tests {\n");
    for i in 0..passing {
        lib += &format!("    #[test]\n    fn t_{i:03}() {{ assert_eq!(1 + 1, 2); }}\n");
    }
    lib += "    #[test]\n    fn planted_failure() -> Result<(), String> { Err(\"CRITICAL-CARGO-4471\".to_string()) }\n}\n";
    write(dir, "src/lib.rs", &lib);
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON and a cargo on PATH"]
fn real_cargo_test_raw_and_rtk_agree_on_the_authoritative_outcome() {
    let fx = Fx::new(0, true);
    crate_at(&fx.project, 400);
    let cargo = cargo_path();
    let cdir = Path::new(&cargo).parent().unwrap().display().to_string();
    let envs = [
        format!("PATH={cdir}:/usr/bin:/bin"),
        format!("HOME={}", fx.root.join("userhome").display()),
        format!("CARGO_HOME={}", fx.root.join("cargo-home").display()),
        format!(
            "CARGO_TARGET_DIR={}",
            fx.root.join("cargo-target").display()
        ),
        "CARGO_NET_OFFLINE=true".to_string(),
        "RUST_TEST_THREADS=1".to_string(),
    ];
    let mut base: Vec<&str> = Vec::new();
    for e in &envs {
        base.extend(["--env", e.as_str()]);
    }
    let mut measured = Vec::new();
    // (label, cargo args): quiet output is byte-deterministic (digests compared
    // across modes); the verbose run measures the favourable per-test-line shape.
    for (label, args) in [
        ("quiet", vec!["test", "-q", "--color", "never"]),
        ("verbose", vec!["test", "--color", "never"]),
    ] {
        let mut argv = vec![cargo.as_str()];
        argv.extend(args.iter().copied());
        let mut raw_flags = base.clone();
        raw_flags.push("--raw");
        let obs = fx.root.join(format!("obs-{label}.jsonl"));
        let mut rtk_flags = base.clone();
        rtk_flags.extend(["--observations", obs.to_str().unwrap()]);
        // libtest prints `finished in 0.0Ns`; under load that can differ between two
        // real runs, so a pair is repeated (at most 4 times) until the digests are comparable.
        let mut pair = None;
        for _ in 0..4 {
            let _ = std::fs::remove_file(&obs);
            let r = fx.exec(&raw_flags, &argv);
            let t = fx.exec(&rtk_flags, &argv);
            let same = r.1["result"]["stdout"]["digest"] == t.1["result"]["stdout"]["digest"];
            pair = Some((r, t));
            if same {
                break;
            }
        }
        let ((o_raw, raw), (o_rtk, rtk)) = pair.unwrap();
        assert_eq!(o_raw.code, 101, "{label}: {}", o_raw.stderr);
        assert_eq!(o_rtk.code, 101, "{label}: {}", o_rtk.stderr);
        assert_eq!(raw["result"]["status"], "exit:101");
        assert_eq!(rtk["result"]["status"], "exit:101");
        assert_eq!(raw["view"]["route"], "raw");
        assert_eq!(rtk["view"]["route"], "provider", "{label}: {rtk}");
        assert_eq!(rtk["view"]["provenance"], ID);
        if label == "quiet" {
            for k in ["stdout", "stderr"] {
                assert_eq!(
                    raw["result"][k]["digest"], rtk["result"][k]["digest"],
                    "{label} {k} digest equals the raw-mode run"
                );
            }
        } else {
            // Timing text may differ between two real runs; the recovered raw
            // streams must agree line for line except the cargo `Finished` line.
            assert_eq!(
                raw["result"]["stdout"]["digest"],
                rtk["result"]["stdout"]["digest"]
            );
            let rec = |v: &Value, st: &str| {
                let o = run(
                    &s(&[
                        "recover",
                        fx.project.to_str().unwrap(),
                        v["result"]["recovery_handle"].as_str().unwrap(),
                        "--stream",
                        st,
                        "--limit",
                        "1000000",
                    ]),
                    &fx.env,
                );
                o.stdout
                    .lines()
                    .filter(|l| !l.contains("Finished"))
                    .map(String::from)
                    .collect::<Vec<_>>()
            };
            assert_eq!(rec(&raw, "stderr"), rec(&rtk, "stderr"));
        }
        let text = rtk["view"]["text"].as_str().unwrap();
        assert!(
            text.contains("planted_failure"),
            "{label}: failing test name in view: {text}"
        );
        assert!(
            text.contains("CRITICAL-CARGO-4471"),
            "{label}: planted message in view"
        );
        assert!(
            text.contains("400 passed") && text.contains("1 failed"),
            "{label}: counts reported: {text}"
        );
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
        println!(
            "real cargo test ({label}): raw {before} B -> display {after} B ({:+} B, byte_only, not tokens)",
            after as i64 - before as i64
        );
        measured.push((label, before, after));
    }
    // The favourable (per-test lines) shape saves bytes; whichever way the quiet
    // shape goes it is printed above and kept, not filtered out.
    let verbose = measured.iter().find(|m| m.0 == "verbose").unwrap();
    assert!(
        verbose.2 < verbose.1,
        "verbose cargo output shrinks: {measured:?}"
    );
}

#[test]
#[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
fn wrapped_route_without_complete_raw_recovery_runs_unwrapped_once() {
    let mut fx = Fx::new(1024, true);
    // Opt in to wrapper routes: RTK's plan answers `wrapped`, but its recall store
    // keeps raw output only for failures/truncations, so the host must not wrap.
    let policy = policy(&required_tool("HARNESS_PYTHON"), 1024)
        .replace("\"retention\"", "\"allow_wrapper\":true,\"retention\"");
    write(&fx.home, "command-view.json", &policy);
    // A bare name resolved through the supplied PATH: the only shape RTK may wrap.
    let cargo = fx.cargo(TEST_OUT);
    let bin = Path::new(&cargo).parent().unwrap().display().to_string();
    fx.env
        .vars
        .insert("PATH".into(), format!("{bin}:/usr/bin:/bin"));
    let (o, v) = fx.exec(&[], &["cargo", "test", "101"]);
    assert_eq!(o.code, 101, "{}", o.stderr);
    assert_eq!(v["result"]["executions"], 1);
    assert!(
        v["result"]["effective_argv"].is_null(),
        "never wrapped: {v}"
    );
    assert_eq!(v["result"]["executable"], cargo.as_str());
    assert_eq!(v["view"]["route"], "provider", "{v}");
    assert!(v["view"]["notes"]
        .to_string()
        .contains("lacks complete raw recovery"));
    assert_eq!(fx.count(), 1);
}

// ---- HN-12: a real RTK view of a failing check reaches the next model attempt --------

mod hn12 {
    use super::*;
    use semaprax_harness::command_view::retention::StreamName;
    use semaprax_harness::command_view::{recover_by_id, ViewTokenizer};
    use semaprax_harness::observe::{ExternalTokenizer, Observer, ObserverLimits};
    use semaprax_harness::workflow::compiler::SubprocessCompiler;
    use semaprax_harness::workflow::stages::{
        NativeContext, ProposalRequest, ProposalStage, StageFailure, Task, TaskMode,
    };
    use semaprax_harness::workflow::{
        CheckSpec, Composition, HostCommandChecks, RunConfig, SessionBounds, Snapshot, Stages,
    };
    use std::cell::{Cell, RefCell};

    /// Reads the failing test name out of the delivered check output (the only place
    /// it appears) and proposes the rename it names; otherwise repeats a wrong proposal.
    struct Parser {
        calls: Cell<u32>,
        feedback: RefCell<Vec<Value>>,
    }
    impl Parser {
        fn rename(name: &str) -> Vec<u8> {
            serde_json::json!({"schema": "semaprax.harness-proposal.v1", "intent":
                {"kind": "rename_declaration", "target": "ledger.line_total", "name": name}})
            .to_string()
            .into_bytes()
        }
    }
    impl ProposalStage for &Parser {
        fn id(&self) -> String {
            "org.example/parser".into()
        }
        fn propose(&mut self, r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
            let n = self.calls.get();
            self.calls.set(n + 1);
            let fb = r.prompt["feedback"].clone();
            self.feedback.borrow_mut().push(fb.clone());
            let text = fb.to_string();
            const KEY: &str = "must_be_named_";
            let name = text.find(KEY).map(|i| {
                text[i + KEY.len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect::<String>()
            });
            Ok(Parser::rename(name.as_deref().unwrap_or("line_cost")))
        }
        fn calls(&self) -> u32 {
            self.calls.get()
        }
        fn side_effecting(&self) -> bool {
            false
        }
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

    fn tokenizer() -> Option<ViewTokenizer> {
        let py = std::env::var_os("HARNESS_TOKENIZER_PYTHON")?;
        let cache = std::env::var("HARNESS_TIKTOKEN_CACHE").ok()?;
        let script = repo_root().join("scripts/harness_tokenize.py");
        let env = BTreeMap::from([
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("TIKTOKEN_CACHE_DIR".to_string(), cache),
        ]);
        let t = ExternalTokenizer::spawn(
            Path::new(&py),
            &[script.to_string_lossy().into_owned(), "cl100k_base".into()],
            &env,
        )
        .ok()?;
        Some(ViewTokenizer(std::rc::Rc::new(t)))
    }

    struct Outcome {
        report: semaprax_harness::workflow::Report,
        prompts: Vec<Value>,
        runs: u32,
        fx: Fx,
    }

    fn scenario(tok: Option<ViewTokenizer>, owner: Option<&str>) -> Outcome {
        let fx = Fx::new(0, true);
        let python = required_tool("HARNESS_PYTHON");
        let compiler_exe = required_tool("SEMAPRAX_COMPILER");
        let cargo = cargo_path();
        let cdir = Path::new(&cargo).parent().unwrap().display().to_string();
        let grant = [
            "PATH",
            "HOME",
            "CARGO_HOME",
            "CARGO_TARGET_DIR",
            "CARGO_NET_OFFLINE",
            "RUST_TEST_THREADS",
        ];
        write(
            &fx.home,
            "command-view.json",
            &format!(
                r#"{{"schema":"semaprax.harness-command-view-policy.v1","min_bytes":512,"runtimes":{{"python":"{}"}},"retention":{{"enabled":true,"ttl_secs":3600,"max_bytes":268435456}},"env_grant":{}}}"#,
                python.display(),
                serde_json::json!(grant)
            ),
        );
        let fixtures = repo_root().join("crates/semaprax-harness/tests/fixtures/workflow/healthy");
        copy_dir(&fixtures, &fx.project);
        write(&fx.project, "rcheck/Cargo.toml", "[package]\nname = \"rcheck\"\nversion = \"0.1.0\"\nedition = \"2021\"\n[dependencies]\n");
        let mut lib = String::from("#[cfg(test)]\nmod tests {\n");
        for i in 0..400 {
            lib += &format!("    #[test]\n    fn t_{i:03}() {{ assert_eq!(1 + 1, 2); }}\n");
        }
        lib += "    #[test]\n    fn must_be_named_zeta_9f3a() -> Result<(), String> {\n        let src = std::fs::read_to_string(\"../src/lib.spx\").map_err(|e| e.to_string())?;\n        if src.contains(\"fn zeta_9f3a(\") { Ok(()) } else { Err(\"the declaration under test has the wrong name\".to_string()) }\n    }\n}\n";
        write(&fx.project, "rcheck/src/lib.rs", &lib);
        let counter = fx.counter();
        let shim = write(
            &fx.project,
            "tools/cargo",
            &format!(
                "#!/bin/sh\nf=\"{}\"\nn=$(cat \"$f\" 2>/dev/null || echo 0)\necho $((n+1)) > \"$f\"\nexec \"{cargo}\" \"$@\"\n",
                counter.display()
            ),
        );
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut env = fx.env.clone();
        env.cwd = fx.project.clone();
        for (k, v) in [
            ("PATH", format!("{cdir}:/usr/bin:/bin")),
            ("HOME", fx.root.join("userhome").display().to_string()),
            (
                "CARGO_HOME",
                fx.root.join("cargo-home").display().to_string(),
            ),
            (
                "CARGO_TARGET_DIR",
                fx.root.join("cargo-target").display().to_string(),
            ),
            ("CARGO_NET_OFFLINE", "true".into()),
            ("RUST_TEST_THREADS", "1".into()),
        ] {
            env.vars.insert(k.into(), v);
        }
        if let Some(o) = owner {
            env.vars
                .insert("SEMAPRAX_HARNESS_EXTERNAL_VIEW_OWNER".into(), o.into());
        }
        let mut stage = HostCommandChecks::new(env);
        stage.tokenizer = tok;
        let root = fx.root.clone();
        let compiler = SubprocessCompiler::new(compiler_exe, root.join("compiler")).unwrap();
        let cfg = RunConfig {
            snapshot: Snapshot::capture(&fx.project).unwrap(),
            task: Task {
                schema_version: 2,
                mode: TaskMode::Change,
                goal: "rename line_total so the failing unit check passes".into(),
                seed: Some("ledger.line_total".into()),
                session: Some(SessionBounds {
                    max_attempts: 3,
                    ..Default::default()
                }),
                ..Task::default()
            },
            context_max_bytes: 16384,
            cache_dir: root.join("cache"),
            lock_digest: "sha256:lock".into(),
            providers: vec![],
            composition: Composition::from_profile(None, true, vec![], &[]).unwrap(),
            apply_policy: None,
            checks: vec![CheckSpec {
                name: "unit".into(),
                argv: s(&[
                    "tools/cargo",
                    "test",
                    "--manifest-path",
                    "rcheck/Cargo.toml",
                    "--color",
                    "never",
                ]),
            }],
            skill_prompt: None,
            endpoint_policy: Default::default(),
            model_plans: None,
            notes: vec![],
            budget: Default::default(),
            cancel: None,
            routing: Default::default(),
        };
        let parser = Parser {
            calls: Cell::new(0),
            feedback: RefCell::default(),
        };
        let mut native = NativeContext::new(&compiler);
        let mut p = &parser;
        let report = semaprax_harness::workflow::run(
            &cfg,
            &compiler,
            Stages {
                decision: None,
                native: &mut native,
                external: None,
                proposer: &mut p,
                command: &mut stage,
            },
            &mut Observer::new(None, ObserverLimits::default()),
        );
        let prompts = parser.feedback.borrow().clone();
        let runs = fx.count();
        Outcome {
            report,
            prompts,
            runs,
            fx,
        }
    }

    fn raw_of(o: &Outcome, rec: &Value, stream: StreamName) -> String {
        let (pid, h) = (
            rec["project_id"].as_str().unwrap(),
            rec["handle"].as_str().unwrap(),
        );
        let before = o.fx.count();
        let r = recover_by_id(&o.fx.env, pid, h, stream, 0, 4 << 20).unwrap();
        assert_eq!(o.fx.count(), before, "recovery never re-executes the check");
        r.text
    }

    #[test]
    #[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON SEMAPRAX_COMPILER and a cargo on PATH (named tokenizer optional: HARNESS_TOKENIZER_PYTHON HARNESS_TIKTOKEN_CACHE)"]
    fn real_rtk_view_of_a_failing_check_reaches_the_next_attempt_and_the_fix_is_derived_from_it() {
        let named = tokenizer();
        for tok in [named.clone(), None] {
            let o = scenario(tok.clone(), None);
            let r = &o.report;
            assert_eq!(
                r.status, "candidate-ready",
                "{:?} {}",
                r.refusals, r.session
            );
            assert_eq!(r.session["attempts"][0]["stage"], "checks");
            assert_eq!(r.session["attempts"][1]["outcome"], "admitted");
            assert_eq!(o.runs, 2, "one execution per attempt, none for recovery");
            // The second request carried the RTK-transformed failing output.
            let fb = &o.prompts[1][0];
            let out = fb["check_output"]["output"].as_str().unwrap();
            assert_eq!(fb["check_output"]["route"], "provider", "{fb}");
            assert!(
                out.contains("must_be_named_zeta_9f3a"),
                "seeded failure in the view: {out}"
            );
            assert!(
                out.contains("1 failed") && out.contains("400 passed"),
                "{out}"
            );
            assert!(
                out.matches("... ok").count() < 20,
                "the ok lines were compressed away"
            );
            let d = &fb["check_output"]["delivered"];
            assert_eq!(d["decision"], "provider-smaller");
            let dm = &r.session["delivered_to_model"];
            if tok.is_some() {
                assert_eq!(d["basis"], "tokens");
                let (raw, del) = (
                    d["raw_tokens"].as_u64().unwrap(),
                    d["delivered_tokens"].as_u64().unwrap(),
                );
                assert!(del < raw && d["saved_tokens"].as_i64().unwrap() > 0);
                assert_eq!(dm["saved_tokens"], d["saved_tokens"]);
                println!("HN-12 real session: cl100k_base raw {raw} -> delivered {del} tokens (saved {})", raw - del);
            } else {
                assert_eq!(d["basis"], "bytes-only");
                assert!(
                    d["saved_tokens"].is_null() && dm["saved_tokens"].is_null(),
                    "unavailable counts claim nothing"
                );
            }
            // Raw recovery after the failing run (handle bound in the feedback) and after the passing one.
            let fail_rec = serde_json::json!({"project_id": fb["check_output"]["recovery_project_id"], "handle": fb["check_output"]["recovery_handle"]});
            let raw = raw_of(&o, &fail_rec, StreamName::Stdout);
            assert!(
                raw.contains("test tests::t_000 ... ok")
                    && raw.contains("must_be_named_zeta_9f3a ... FAILED")
            );
            let pass = &r.checks["commands"][0];
            assert_eq!(pass["passed"], true);
            let raw = raw_of(&o, &pass["recovery"], StreamName::Stdout);
            assert!(
                raw.contains("test result: ok. 401 passed"),
                "{}",
                &raw[raw.len().saturating_sub(300)..]
            );
            assert_eq!(o.fx.count(), 2);
        }
    }

    #[test]
    #[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON SEMAPRAX_COMPILER and a cargo on PATH"]
    fn external_rtk_ownership_delivers_raw_and_the_session_still_finds_the_failure() {
        let o = scenario(None, Some("rtk-hook"));
        let r = &o.report;
        assert_eq!(
            r.status, "candidate-ready",
            "{:?} {}",
            r.refusals, r.session
        );
        let co = &o.prompts[1][0]["check_output"];
        assert_eq!(
            co["route"], "raw",
            "no second transform under external ownership: {co}"
        );
        assert!(co["output"]
            .as_str()
            .unwrap()
            .contains("must_be_named_zeta_9f3a"));
        assert_eq!(co["delivered"]["decision"], "raw-by-policy");
        assert_eq!(o.runs, 2);
    }

    #[test]
    #[ignore = "provisioned: needs HARNESS_RTK HARNESS_PYTHON"]
    fn real_rtk_expanded_tiny_structured_and_unknown_outputs_keep_raw_status_and_content() {
        use semaprax_harness::command_view::{execute, ExecOptions};
        let fx = Fx::new(0, true);
        fx.git_repo();
        let tok = tokenizer();
        let opts = ExecOptions {
            tokenizer: tok.clone(),
            ..Default::default()
        };
        let go = |argv: &[&str]| execute(&fx.env, &fx.project, &s(argv), &opts, None).unwrap();
        // Expanded view: real RTK turns a tiny match plus a stderr line into more bytes; raw is delivered.
        let rg = "/opt/homebrew/bin/rg";
        if Path::new(rg).exists() {
            let r = go(&[
                rg,
                "-n",
                "line 3 of file 1 ",
                "src/f1.txt",
                "src/missing.txt",
            ]);
            let (v, m) = (
                &r.envelope.view,
                r.envelope.view.measurement.clone().unwrap(),
            );
            assert_eq!(r.envelope.result.termination.label(), "exit:2");
            assert_eq!(
                (v.route.as_str(), m.decision),
                ("raw", "provider-grew-raw-used"),
                "{:?}",
                v.notes
            );
            assert!(v.text.contains("missing.txt") && v.text.contains("line 3 of file 1"));
            assert_eq!(m.saved_tokens().map(|n| n <= 0), tok.as_ref().map(|_| true));
            println!("expanded view kept raw: {}", m.to_json());
        }
        // Structured data: JSON on stdout never reaches the lossy filter, even for an admitted family.
        let json = fx.script(
            "cargo",
            "printf '{\"tests\":['; i=0; while [ $i -lt 300 ]; do printf '\"t-%s\",' $i; i=$((i+1)); done; printf '\"end\"]}'",
        );
        let r = go(&[&json, "test"]);
        assert_eq!(r.envelope.view.route, "raw");
        assert!(r.envelope.view.text.starts_with("{\"tests\":["));
        assert!(r
            .envelope
            .view
            .notes
            .iter()
            .any(|n| n.contains("machine-readable")));
        // Unknown family with a nonzero exit: raw content, authoritative status.
        let unk = fx.script("mystery-tool", "i=0; while [ $i -lt 200 ]; do echo \"unknown line $i\"; i=$((i+1)); done\necho BAD >&2\nexit 3");
        let r = go(&[&unk]);
        assert_eq!(r.envelope.result.termination.label(), "exit:3");
        assert_eq!(r.envelope.view.route, "raw");
        assert!(
            r.envelope.view.text.contains("unknown line 199")
                && r.envelope.view.text.contains("BAD")
        );
        assert_eq!(fx.count(), 2, "each counted script ran once");
    }
}
