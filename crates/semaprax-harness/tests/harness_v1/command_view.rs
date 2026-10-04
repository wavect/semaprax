//! HP-08 command-view tests (fixture prefix `hp-hp08`).

use crate::support::{fixture_dir, harness_bin, repo_root, write};
use semaprax_harness::cli::{run, Environment, Outcome};
use semaprax_harness::command_view::retention::Retention;
use serde_json::Value;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

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

struct Fx {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    adapter: PathBuf,
    counter: PathBuf,
    env: Environment,
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn policy(extra: &str) -> String {
    format!(
        r#"{{"schema":"semaprax.harness-command-view-policy.v1","runtimes":{{"python":"{}"}},"retention":{{"enabled":true,"ttl_secs":3600,"max_bytes":67108864}}{extra}}}"#,
        python().display()
    )
}

impl Fx {
    /// `kind`: `hostile` (temp copy of the fixture adapter) or `example`
    /// (the shipped output-view-python adapter, adopted in place).
    fn new(kind: &str, policy_extra: &str) -> Fx {
        let root = fixture_dir("hp-hp08").canonicalize().unwrap();
        let home = root.join("home");
        let project = root.join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(root.join("tmp")).unwrap();
        write(&home, "command-view.json", &policy(policy_extra));
        let fix = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/command_view");
        let sdk = repo_root().join("packages/semaprax-harness-adapters/sdk/python");
        // The shipped descriptors declare an `upstream` block whose version is not
        // dotted-numeric, so the profile cannot trust them as shipped; both
        // adapters are copied here with that block removed.
        let (src, desc) = if kind == "example" {
            let ex =
                repo_root().join("packages/semaprax-harness-adapters/examples/output-view-python");
            let py = std::fs::read_to_string(ex.join("adapter.py")).unwrap();
            (
                py.replace(
                    "os.path.join(HERE, \"..\", \"..\", \"sdk\", \"python\")",
                    &format!("{:?}", sdk.to_str().unwrap()),
                ),
                std::fs::read_to_string(ex.join("harness-provider.json")).unwrap(),
            )
        } else {
            (
                std::fs::read_to_string(fix.join("adapter.py"))
                    .unwrap()
                    .replace("@SDK@", sdk.to_str().unwrap()),
                std::fs::read_to_string(fix.join("harness-provider.json")).unwrap(),
            )
        };
        let mut d: Value = serde_json::from_str(&desc).unwrap();
        d.as_object_mut().unwrap().remove("upstream");
        write(&root, "adapter/adapter.py", &src);
        write(&root, "adapter/harness-provider.json", &d.to_string());
        write(
            &root,
            "adapter/mode.txt",
            &format!("ok\n{}", root.display()),
        );
        // mode.txt is test-driver data rewritten after adoption (HN-19 closure rules).
        write(
            &root,
            "adapter/harness-closure.json",
            r#"{"schema":"semaprax.harness-closure.v1","exclude":["mode.txt"]}"#,
        );
        let adapter = root.join("adapter");
        let mut vars = BTreeMap::new();
        vars.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        vars.insert("HARNESS_PYTHON".to_string(), python().display().to_string());
        let env = Environment {
            harness_home: Some(home.clone()),
            compiler: None,
            cwd: project.clone(),
            vars,
        };
        let fx = Fx {
            counter: root.join("counter"),
            root,
            home,
            project,
            adapter,
            env,
        };
        let d = fx.adapter.join("harness-provider.json");
        let o = run(&s(&["adopt", d.to_str().unwrap()]), &fx.env);
        assert_eq!(o.code, 0, "adopt: {}", o.stderr);
        let id = if kind == "example" {
            "org.example/output-view"
        } else {
            "org.example/hostile-view"
        };
        let o = run(&s(&["trust", id]), &fx.env);
        assert_eq!(o.code, 0, "trust: {}", o.stderr);
        fx
    }

    fn mode(&self, m: &str) {
        write(
            &self.root,
            "adapter/mode.txt",
            &format!("{m}\n{}", self.root.display()),
        );
    }

    /// Executable script that counts its runs, prints 2500 noise lines with one
    /// decisive error in the middle and a stderr line, and exits `$1`.
    fn script(&self, name: &str, body: &str) -> String {
        let p = write(&self.root, &format!("bin/{name}"), &format!("#!/bin/sh\nf=\"{}\"\nn=$(cat \"$f\" 2>/dev/null || echo 0)\necho $((n+1)) > \"$f\"\n{body}\n", self.counter.display()));
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_str().unwrap().to_string()
    }

    fn noisy(&self, name: &str) -> String {
        self.script(
            name,
            "i=0; while [ $i -lt 2000 ]; do echo \"repetitive noise line\"; i=$((i+1)); done\necho \"ERROR: decisive failure 7731\"\nj=0; while [ $j -lt 500 ]; do echo \"more noise\"; j=$((j+1)); done\necho \"warn on stderr\" >&2\nexit ${1:-0}",
        )
    }

    fn count(&self) -> u32 {
        std::fs::read_to_string(&self.counter).map_or(0, |t| t.trim().parse().unwrap())
    }

    fn exec(&self, flags: &[&str], argv: &[&str]) -> Outcome {
        let mut a = vec!["exec", self.project.to_str().unwrap()];
        a.extend(flags);
        a.push("--");
        a.extend(argv);
        run(&s(&a), &self.env)
    }

    fn exec_json(&self, flags: &[&str], argv: &[&str]) -> (Outcome, Value) {
        let mut f = vec!["--json"];
        f.extend(flags);
        let o = self.exec(&f, argv);
        let v = serde_json::from_str(&o.stdout).unwrap_or(Value::Null);
        (o, v)
    }

    fn recover(&self, handle: &str, extra: &[&str]) -> Outcome {
        let mut a = vec!["recover", self.project.to_str().unwrap(), handle];
        a.extend(extra);
        run(&s(&a), &self.env)
    }

    fn view_calls(&self) -> usize {
        std::fs::read_to_string(self.root.join("view-called")).map_or(0, |t| t.len())
    }
}

fn handle_of(v: &Value) -> String {
    v["result"]["recovery_handle"]
        .as_str()
        .expect("retained handle")
        .to_string()
}

#[test]
fn success_runs_once_and_recovery_never_reruns() {
    let fx = Fx::new("example", "");
    let cmd = fx.noisy("gen.sh");
    let (o, v) = fx.exec_json(&[], &[&cmd, "0"]);
    assert_eq!(o.code, 0, "{}", o.stderr);
    assert_eq!(fx.count(), 1);
    assert_eq!(v["result"]["executions"], 1);
    assert_eq!(v["view"]["route"], "provider");
    assert_eq!(v["view"]["provenance"], "org.example/output-view");
    assert!(v["view"]["text"]
        .as_str()
        .unwrap()
        .contains("decisive failure 7731"));
    assert!(
        v["view"]["omissions"].as_u64().unwrap() > 0,
        "repetitive output was compressed"
    );
    let h = handle_of(&v);
    for _ in 0..3 {
        let r = fx.recover(&h, &["--offset", "0", "--limit", "100000"]);
        assert_eq!(r.code, 0, "{}", r.stderr);
        assert!(r.stdout.contains("repetitive noise line"));
    }
    let r = fx.recover(&h, &["--stream", "stderr"]);
    assert!(r.stdout.contains("warn on stderr"));
    assert_eq!(fx.count(), 1, "recovery must not execute the command");
    // Authoritative digests are over the raw bytes, not the view.
    assert_eq!(v["result"]["stdout"]["retained_complete"], true);
    assert!(v["result"]["stdout"]["digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
}

#[test]
fn failed_run_keeps_exit_stderr_and_separate_retention() {
    let fx = Fx::new("example", "");
    let cmd = fx.noisy("gen.sh");
    let (o, v) = fx.exec_json(&[], &[&cmd, "3"]);
    assert_eq!(o.code, 3);
    assert_eq!(v["result"]["status"], "exit:3");
    assert_eq!(v["result"]["status_certain"], true);
    assert!(v["result"]["stderr"]["bytes"].as_u64().unwrap() > 0);
    assert_eq!(v["result"]["stderr"]["bytes"], 15);
    let failed = handle_of(&v);
    let (o2, v2) = fx.exec_json(&[], &[&cmd, "0"]);
    assert_eq!(o2.code, 0);
    let ok = handle_of(&v2);
    assert_ne!(failed, ok, "failed and successful runs retain separately");
    for h in [&failed, &ok] {
        assert!(fx.recover(h, &[]).stdout.contains("repetitive"));
    }
    assert_eq!(fx.count(), 2);
}

#[test]
fn hostile_providers_cannot_change_status_or_hide_errors() {
    let fx = Fx::new("hostile", "");
    let cmd = fx.noisy("gen.sh");
    for (i, m) in ["drop_critical", "crash", "claim_status", "short_lie"]
        .iter()
        .enumerate()
    {
        fx.mode(m);
        let (o, v) = fx.exec_json(&[], &[&cmd, "7"]);
        assert_eq!(o.code, 7, "{m}");
        assert_eq!(fx.count() as usize, i + 1, "{m}: executed exactly once");
        assert_eq!(v["result"]["status"], "exit:7", "{m}");
        let view = &v["view"];
        let text = view["text"].as_str().unwrap();
        let notes = view["notes"].to_string();
        match *m {
            "drop_critical" | "short_lie" => {
                assert_eq!(view["incomplete"], true, "{m}");
                assert!(
                    text.contains("ERROR: decisive failure 7731"),
                    "{m}: critical line re-attached"
                );
                assert!(view["recovery_handle"].is_string());
                assert_eq!(view["lossless"], false);
            }
            _ => {
                assert_eq!(view["route"], "raw", "{m}");
                assert!(notes.contains("raw view used"), "{m}: {notes}");
                assert!(
                    text.contains("decisive failure 7731") || view["recovery_handle"].is_string()
                );
            }
        }
    }
    // The claim of success inside a refused payload is discarded, never shown.
    fx.mode("claim_status");
    let (_, v) = fx.exec_json(&[], &[&cmd, "7"]);
    assert!(v["view"]["notes"].to_string().contains("SPX-HPA042"));
}

#[test]
fn timeout_is_uncertain_runs_once_and_kills_the_group() {
    let fx = Fx::new("example", "");
    let cmd = fx.script(
        "slow.sh",
        "echo started; echo oops error >&2; sleep 30 & sleep 30",
    );
    let t = std::time::Instant::now();
    // 6 s leaves room for process start-up when the whole suite runs in
    // parallel; the script still sleeps 30 s, so the timeout always fires.
    let (o, v) = fx.exec_json(&["--timeout-ms", "6000"], &[&cmd]);
    assert!(t.elapsed().as_secs() < 20);
    assert_eq!(o.code, 124);
    assert_eq!(v["result"]["status"], "timeout");
    assert_eq!(v["result"]["status_certain"], false);
    assert_eq!(v["view"]["incomplete"], true);
    assert_eq!(fx.count(), 1);
    assert!(v["view"]["text"].as_str().unwrap().contains("started"));
    assert!(
        v["view"]["recovery_handle"].is_string(),
        "partial output stays retrievable"
    );
}

#[test]
fn signal_death_is_reported_as_a_signal() {
    let fx = Fx::new("example", "");
    let cmd = fx.script("sig.sh", "echo before; kill -TERM $$");
    let (o, v) = fx.exec_json(&[], &[&cmd]);
    assert_eq!(v["result"]["status"], "signal:15");
    assert_eq!(o.code, 143);
    assert_eq!(fx.count(), 1);
}

#[test]
fn shell_syntax_and_free_form_shell_fail_before_launch() {
    let fx = Fx::new("example", "");
    let marker = fx.root.join("marker");
    let cmd = fx.script("never.sh", "touch marker-should-not-exist");
    let touch = format!("touch {}", marker.display());
    for argv in [
        vec!["sh", "-c", touch.as_str()],
        vec![cmd.as_str(), "|", "wc"],
        vec![cmd.as_str(), ">", "out"],
        vec![cmd.as_str(), ";", "id"],
        vec![cmd.as_str(), "$(id)"],
        vec![cmd.as_str(), "a&&b"],
        vec!["ssh", "host"],
    ] {
        let o = fx.exec(&[], &argv);
        assert_eq!(o.code, 1, "{argv:?}");
        assert!(o.stderr.contains("SPX-HPH01"), "{argv:?}: {}", o.stderr);
    }
    assert!(!marker.exists());
    assert_eq!(fx.count(), 0, "nothing launched");
    let o = fx.exec(&[], &["nonexistent-tool"]);
    assert!(o.stderr.contains("SPX-HPH013"));
}

#[test]
fn unsafe_wrapper_plans_fail_before_launch() {
    let fx = Fx::new("hostile", r#","allow_wrapper":true"#);
    let cmd = fx.noisy("gen.sh");
    for (m, code) in [
        ("wrap_subst", "SPX-HPH014"),
        ("wrap_shell", "SPX-HPH012"),
        ("wrap_widen", "SPX-HPH016"),
        ("wrap_norecovery", "SPX-HPH017"),
    ] {
        fx.mode(m);
        let o = fx.exec(&[], &[&cmd, "0"]);
        assert_eq!(o.code, 1, "{m}: {}", o.stderr);
        assert!(o.stderr.contains(code), "{m}: {}", o.stderr);
    }
    assert_eq!(fx.count(), 0, "no wrapper plan reached execution");
}

#[test]
fn truncated_display_is_incomplete_with_working_recovery() {
    let fx = Fx::new("hostile", "");
    let cmd = fx.noisy("gen.sh");
    let (_, v) = fx.exec_json(&["--raw"], &[&cmd, "0"]);
    let view = &v["view"];
    assert_eq!(view["route"], "raw");
    assert_eq!(view["incomplete"], true);
    assert_eq!(view["lossless"], false);
    assert!(
        view["text"]
            .as_str()
            .unwrap()
            .contains("decisive failure 7731"),
        "decisive error survives truncation"
    );
    assert!(view["text"].as_str().unwrap().len() < 12000);
    let h = handle_of(&v);
    let total = v["result"]["stdout"]["bytes"].as_u64().unwrap();
    let mut got = 0u64;
    let mut offset = 0u64;
    while offset < total {
        let r = fx.recover(
            &h,
            &[
                "--offset",
                &offset.to_string(),
                "--limit",
                "20000",
                "--json",
            ],
        );
        let j: Value = serde_json::from_str(&r.stdout).unwrap();
        got += j["bytes"].as_u64().unwrap();
        offset += j["bytes"].as_u64().unwrap();
    }
    assert_eq!(got, total);
    assert_eq!(fx.count(), 1);
    assert_eq!(fx.view_calls(), 0, "--raw never reaches a provider");
    assert!(fx
        .recover(&h, &["--offset", "999999999"])
        .stderr
        .contains("SPX-HPH042"));
    assert!(fx
        .recover("cv-000000000000000000000000", &[])
        .stderr
        .contains("SPX-HPH041"));
    assert!(fx
        .recover("../../etc/passwd", &[])
        .stderr
        .contains("SPX-HPH040"));
}

#[test]
fn external_owner_nested_host_and_wrappers_do_not_double_process() {
    let fx = Fx::new("hostile", r#","known_wrappers":["wrapme"]"#);
    let cmd = fx.noisy("gen.sh");
    let (_, v) = fx.exec_json(&["--external-owner", "other-host"], &[&cmd, "0"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("external owner"));
    assert_eq!(v["result"]["lineage"][0], "external:other-host");
    // A wrapper executable is left alone.
    let w = fx.script("wrapme", "echo hi");
    let (_, v) = fx.exec_json(&[], &[&w]);
    assert!(v["view"]["notes"].to_string().contains("wrapper"), "{v}");
    // A recursive host exec: the outer run takes no second look at the inner view.
    let p = fx.project.to_str().unwrap();
    let hb = harness_bin();
    let (_, v) = fx.exec_json(&[], &[hb.to_str().unwrap(), "exec", p, "--", &cmd, "0"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("host exec"));
    assert_eq!(fx.view_calls(), 0, "provider never consulted");
    assert_eq!(fx.count(), 3, "each command ran once");
    // The command sees the lineage marker so a nested host stays out too.
    let e = fx.script(
        "env.sh",
        "echo \"L=$SEMAPRAX_HARNESS_COMMAND_VIEW_LINEAGE\"",
    );
    let (_, v) = fx.exec_json(&["--raw"], &[&e]);
    assert!(v["view"]["text"].as_str().unwrap().contains("L=exec:"));
}

#[test]
fn machine_output_small_output_and_signatures_bypass() {
    let fx = Fx::new("hostile", "");
    let json = fx.script("j.sh", "printf '{\"items\":['; i=0; while [ $i -lt 400 ]; do printf '\"item-%s\",' $i; i=$((i+1)); done; printf '\"end\"]}'");
    let (_, v) = fx.exec_json(&[], &[&json]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["view"]["notes"].to_string().contains("machine-readable"));
    let t = v["view"]["text"].as_str().unwrap();
    assert!(
        t.starts_with("{\"items\":["),
        "JSON passes through unaltered: {t:.40}"
    );
    let small = fx.script("small.sh", "echo tiny");
    let (_, v) = fx.exec_json(&[], &[&small]);
    assert!(v["view"]["notes"].to_string().contains("small output"));
    assert_eq!(v["view"]["lossless"], true);
    let flag = fx.noisy("flag.sh");
    let (_, v) = fx.exec_json(&[], &[&flag, "--json"]);
    assert!(v["view"]["notes"]
        .to_string()
        .contains("machine-output-flag"));
    let sem = fx.noisy("semaprax-fake");
    let (_, v) = fx.exec_json(&[], &[&sem, "0"]);
    assert!(v["view"]["notes"]
        .to_string()
        .contains("authoritative-envelope"));
    assert_eq!(fx.view_calls(), 0);
}

#[test]
fn redaction_precedes_the_provider_and_retention_stays_private() {
    let fx = Fx::new("hostile", r#","redact_lines_containing":["api_secret="]"#);
    let body = "echo 'API_SECRET=hunter2'; i=0; while [ $i -lt 400 ]; do echo \"filler line $i\"; i=$((i+1)); done";
    let cmd = fx.script("sec.sh", body);
    let (_, v) = fx.exec_json(&[], &[&cmd]);
    let seen = std::fs::read_to_string(fx.root.join("seen.txt")).unwrap();
    assert!(!seen.contains("hunter2"), "provider saw a secret");
    assert!(seen.contains("[redacted by retention policy]"));
    let h = handle_of(&v);
    assert!(
        fx.recover(&h, &[]).stdout.contains("hunter2"),
        "private retention keeps raw bytes"
    );
    // Directory and files are private.
    let root = std::fs::canonicalize(&fx.project).unwrap();
    let pid = semaprax_harness::json::sha256_plain(root.to_string_lossy().as_bytes());
    let dir = Retention::dir_for(&fx.home, &pid);
    assert_eq!(
        std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        assert_eq!(
            e.metadata().unwrap().permissions().mode() & 0o777,
            0o600,
            "{:?}",
            e.path()
        );
    }
}

#[test]
fn retention_is_opt_in_and_bounded_by_ttl() {
    let fx = Fx::new("hostile", "");
    // Policy without retention: provider is not consulted (no raw recovery), no handle.
    write(
        &fx.home,
        "command-view.json",
        &format!(
            r#"{{"schema":"semaprax.harness-command-view-policy.v1","runtimes":{{"python":"{}"}}}}"#,
            python().display()
        ),
    );
    let cmd = fx.noisy("gen.sh");
    let (_, v) = fx.exec_json(&[], &[&cmd, "0"]);
    assert_eq!(v["view"]["route"], "raw");
    assert!(v["result"]["recovery_handle"].is_null());
    assert!(v["view"]["notes"]
        .to_string()
        .contains("not fully retained"));
    assert_eq!(v["view"]["incomplete"], true);
    assert!(v["view"]["text"]
        .as_str()
        .unwrap()
        .contains("decisive failure 7731"));
    assert_eq!(fx.view_calls(), 0);
    // With a 1 second TTL a handle expires.
    write(
        &fx.home,
        "command-view.json",
        &policy("").replace("3600", "1"),
    );
    let (_, v) = fx.exec_json(&["--raw"], &[&cmd, "0"]);
    let h = handle_of(&v);
    assert!(fx.recover(&h, &[]).code == 0);
    std::thread::sleep(std::time::Duration::from_millis(2500));
    assert!(fx.recover(&h, &[]).stderr.contains("SPX-HPH041"));
}

#[test]
fn observation_records_the_final_display_envelope() {
    let fx = Fx::new("example", "");
    let cmd = fx.noisy("gen.sh");
    let obs = fx.root.join("obs.jsonl");
    let o = fx.exec(&["--observations", obs.to_str().unwrap()], &[&cmd, "0"]);
    let text = std::fs::read_to_string(&obs).unwrap();
    let line: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    assert_eq!(line["stage"], "command_view");
    assert_eq!(line["model_visible"], true);
    assert_eq!(line["after"]["tokenizer"]["kind"], "byte_only");
    assert_eq!(
        line["after"]["value"].as_u64().unwrap() as usize,
        o.stdout.len()
    );
    assert!(line["before"]["value"].as_u64().unwrap() > line["after"]["value"].as_u64().unwrap());
    assert!(line.to_string().find("decisive").is_none(), "metadata only");
}

#[test]
fn unreadable_policy_and_unknown_flags_are_refused() {
    let fx = Fx::new("example", "");
    write(
        &fx.home,
        "command-view.json",
        r#"{"schema":"semaprax.harness-command-view-policy.v1","surprise":1}"#,
    );
    let o = fx.exec(&[], &["/bin/echo", "hi"]);
    assert!(o.stderr.contains("SPX-HPH030"));
    let o = run(
        &s(&[
            "exec",
            fx.project.to_str().unwrap(),
            "--bogus",
            "--",
            "/bin/echo",
        ]),
        &fx.env,
    );
    assert_eq!(o.code, 2);
    assert_eq!(
        run(&s(&["exec", fx.project.to_str().unwrap()]), &fx.env).code,
        2
    );
}
