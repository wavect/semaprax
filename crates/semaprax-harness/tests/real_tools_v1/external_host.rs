//! HP-14 real external host: Claude Code 2.1.289 through its documented
//! PreToolUse hook. Needs HARNESS_CLAUDE (absolute path to `claude`).
//! One headless `claude -p` session on the cheapest model; it uses the
//! caller's existing Claude login. Only a temp project's settings are used
//! (`--settings`, `--setting-sources project,local`); nothing global is read
//! by the hook or written.

use crate::support::{fixture_dir, harness_bin, required_tool, write};
use serde_json::Value;
use std::process::Command;

#[test]
#[ignore = "provisioned: needs HARNESS_CLAUDE (claude 2.1.289) and a Claude login"]
fn hp_hp14_claude_code_delegates_a_command_through_the_bridge() {
    let claude = required_tool("HARNESS_CLAUDE");
    let version = String::from_utf8(
        Command::new(&claude)
            .arg("--version")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    assert!(
        version.starts_with("2.1.289"),
        "pinned Claude Code 2.1.289, got {version}"
    );
    let root = fixture_dir("hp-hp14-claude").canonicalize().unwrap();
    let project = root.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let git = |a: &[&str]| {
        let o = Command::new("/usr/bin/git")
            .args(a)
            .current_dir(&project)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@e")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@e")
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    };
    git(&["init", "-q"]);
    for n in ["one", "two", "three"] {
        write(&project, &format!("{n}.txt"), n);
        git(&["add", "."]);
        git(&["commit", "-q", "-m", &format!("commit {n}")]);
    }
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    // Retention records one entry per `exec`, proving the rewritten command ran through the host.
    write(
        &home,
        "command-view.json",
        r#"{"schema":"semaprax.harness-command-view-policy.v1","retention":{"enabled":true,"ttl_secs":3600,"max_bytes":67108864}}"#,
    );
    let log = root.join("bridge.log");
    let bin = harness_bin();
    let hook = format!(
        "'{}' bridge '{}' --host claude-code --hook pre-tool-use --log '{}'",
        bin.display(),
        project.display(),
        log.display()
    );
    let settings = serde_json::json!({"hooks": {"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": hook}]}]}});
    let settings_path = write(&root, "settings.json", &settings.to_string());
    let out = Command::new(&claude)
        .current_dir(&project)
        .env("SEMAPRAX_HARNESS_HOME", &home)
        .args([
            "-p",
            "Run the shell command: git log --oneline -3",
            "--model",
            "haiku",
            "--max-turns",
            "3",
            "--output-format",
            "json",
            "--allowedTools",
            "Bash",
        ])
        .args([
            "--settings",
            settings_path.to_str().unwrap(),
            "--setting-sources",
            "project,local",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    eprintln!(
        "claude exit={:?}\nstdout={stdout}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    let log_text =
        std::fs::read_to_string(&log).expect("the hook must have been invoked by Claude Code");
    eprintln!("bridge log:\n{log_text}");
    let lines: Vec<Value> = log_text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let rewritten: Vec<_> = lines
        .iter()
        .filter(|l| l["decision"] == "rewritten")
        .collect();
    assert_eq!(rewritten.len(), 1, "exactly one rewrite: {log_text}");
    assert_eq!(rewritten[0]["original"], "git log --oneline -3");
    assert!(rewritten[0]["rewritten"]
        .as_str()
        .unwrap()
        .contains("'exec'"));
    let result: Value = serde_json::from_str(&stdout).expect("claude json result");
    let text = result["result"].as_str().unwrap_or_default();
    assert!(
        text.contains("commit three"),
        "result reached the session: {text}"
    );
    let mut retained = Vec::new();
    for project_dir in std::fs::read_dir(home.join("retention"))
        .expect("exec retained its result")
        .flatten()
    {
        for e in std::fs::read_dir(project_dir.path().join("command-view"))
            .unwrap()
            .flatten()
        {
            retained.push(e.path());
        }
    }
    let metas: Vec<_> = retained
        .iter()
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    assert_eq!(
        metas.len(),
        1,
        "git ran exactly once through `semaprax-harness exec`: {retained:?}"
    );
    let out_file = retained
        .iter()
        .find(|p| p.extension().is_some_and(|x| x == "stdout"))
        .unwrap();
    assert!(std::fs::read_to_string(out_file)
        .unwrap()
        .contains("commit three"));
    eprintln!("retained exec records: {}", metas.len());
}
