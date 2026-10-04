//! HP-14 real external host: Claude Code 2.1.289 through its documented
//! PreToolUse hook. Needs HARNESS_CLAUDE (absolute path to `claude`).
//! One headless `claude -p` session on the cheapest model; it uses the
//! caller's existing Claude login. Only a temp project's settings are used
//! (`--settings`, `--setting-sources project,local`); nothing global is read
//! by the hook or written.

use crate::support::{fixture_dir, harness_bin, required_tool, write};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

// ---- HN-14: default skills through real MCP clients ----

fn jsonl(path: &Path) -> Vec<Value> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn harness(args: &[&str], home: &Path) -> Value {
    let out = Command::new(harness_bin())
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SEMAPRAX_HARNESS_HOME", home)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}

fn mode_and_lock(status: &Value, id: &str) -> (String, Value) {
    let k = status["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == id)
        .unwrap();
    (
        k["mode"].as_str().unwrap().to_string(),
        k["locked_revision"].clone(),
    )
}

fn catalog_digest(home: &Path, id: &str) -> String {
    let list = harness(&["skills", "list", "--json"], home);
    list["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == id)
        .unwrap()["bundle_digest"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Run `claude -p` once; returns the parsed `--output-format json` result.
fn claude_turn(
    claude: &Path,
    project: &Path,
    prompt: &str,
    resume: Option<&str>,
    home: &Path,
) -> Value {
    let tools = ["list", "load", "use", "off", "status"]
        .iter()
        .map(|t| format!("mcp__semaprax-skills__skills_{t}"))
        .chain(["Read".to_string(), "Edit".to_string()])
        .collect::<Vec<_>>()
        .join(",");
    let mut cmd = Command::new(claude);
    cmd.current_dir(project)
        .env("SEMAPRAX_HARNESS_HOME", home)
        .args([
            "-p",
            prompt,
            "--model",
            "haiku",
            "--max-turns",
            "10",
            "--output-format",
            "json",
            "--allowedTools",
            &tools,
            "--setting-sources",
            "project,local",
            "--strict-mcp-config",
            "--mcp-config",
            project.join(".mcp.json").to_str().unwrap(),
        ]);
    if let Some(id) = resume {
        cmd.args(["--resume", id]);
    }
    let out = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    eprintln!(
        "claude exit={:?}\nstdout={stdout}\nstderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_str(&stdout).expect("claude json result")
}

#[test]
#[ignore = "provisioned: needs HARNESS_CLAUDE (claude 2.1.289) and a Claude login; at most two haiku turns"]
fn hn14_claude_code_discovers_uses_and_stops_default_skills_through_the_mcp_bridge() {
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
    let root = fixture_dir("hn-hn14-claude").canonicalize().unwrap();
    let project = root.join("project");
    let home = root.join("home");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    write(
        &project,
        "util.py",
        "def first(items):\n    return items[0]\n",
    );
    // The reviewed per-project setup writes the only config this session uses.
    let log = root.join("delivery.log");
    let session = "hn14-claude-real";
    let setup = harness(
        &[
            "bridge",
            project.to_str().unwrap(),
            "--setup",
            "claude-code",
            "--write",
            "--session",
            session,
            "--log",
            log.to_str().unwrap(),
            "--harness-home",
            home.to_str().unwrap(),
            "--harness-bin",
            harness_bin().to_str().unwrap(),
        ],
        &home,
    );
    assert_eq!(setup["written"], true, "{setup}");
    let mcp: Value =
        serde_json::from_str(&std::fs::read_to_string(project.join(".mcp.json")).unwrap()).unwrap();
    assert_eq!(mcp["mcpServers"]["semaprax-skills"], setup["entry"]);
    let (ponytail_rev, caveman_rev) = (
        catalog_digest(&home, "ponytail"),
        catalog_digest(&home, "caveman"),
    );

    let t1 = claude_turn(&claude, &project, "You have MCP tools from the semaprax-skills server. Step 1: call skills_list. Step 2: call skills_use with name ponytail and mode full, and follow the loaded skill. Step 3: add a function largest(items) to util.py that returns the biggest element of a non-empty list. Reply with one short sentence when done.", None, &home);
    let sid = t1["session_id"].as_str().unwrap().to_string();
    let t2 = claude_turn(&claude, &project, "Call skills_use with name caveman and then answer in one sentence: what is a mutex? After that call skills_off with name caveman and reply only with the word stopped.", Some(&sid), &home);

    let events = jsonl(&log);
    eprintln!(
        "bridge log:\n{}",
        events
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    let pos = |ev: &str, skill: &str| {
        events
            .iter()
            .position(|e| e["event"] == ev && e["skill"] == skill)
    };
    assert_eq!(events[0]["event"], "skills.session");
    assert_eq!(events[0]["client"], "claude-code");
    assert_eq!(events[0]["client_version"], "2.1.289");
    let p =
        pos("skills.delivered", "ponytail").expect("official Ponytail delivered to Claude Code");
    assert_eq!(
        events[p]["revision"],
        ponytail_rev.as_str(),
        "exact delivered revision"
    );
    assert_eq!(events[p]["mode"], "full");
    let c = pos("skills.delivered", "caveman").expect("Caveman explicitly invoked");
    assert_eq!(events[c]["revision"], caveman_rev.as_str());
    let off = pos("skills.off", "caveman").expect("Caveman explicitly stopped");
    assert!(p < c && c < off, "order: ponytail, caveman, stop");
    // Durable state agrees with the observations (same project/session as the CLI).
    let pid = semaprax_harness::skills::cli_defaults::project_id(&project);
    let status = harness(
        &[
            "skills",
            "status",
            "--json",
            "--project",
            &pid,
            "--session",
            session,
        ],
        &home,
    );
    assert_eq!(
        mode_and_lock(&status, "ponytail"),
        ("full".into(), json!(ponytail_rev))
    );
    assert_eq!(mode_and_lock(&status, "caveman").0, "off");
    let util = std::fs::read_to_string(project.join("util.py")).unwrap();
    assert!(util.contains("largest"), "the coding task ran: {util}");
    let cost = |t: &Value| t["total_cost_usd"].as_f64().unwrap_or(0.0);
    eprintln!(
        "cost_usd turn1={} turn2={} total={}",
        cost(&t1),
        cost(&t2),
        cost(&t1) + cost(&t2)
    );
}

fn opencode_ready(opencode: &Path) -> String {
    let v = String::from_utf8(
        Command::new(opencode)
            .arg("--version")
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    use std::io::{Read, Write};
    let mut sock = std::net::TcpStream::connect("127.0.0.1:11434").expect(
        "provisioned test needs a running local Ollama at 127.0.0.1:11434 with qwen2.5:0.5b",
    );
    sock.write_all(b"GET /api/tags HTTP/1.0\r\n\r\n").unwrap();
    let mut body = String::new();
    sock.read_to_string(&mut body).unwrap();
    assert!(
        body.contains("qwen2.5:0.5b"),
        "Ollama has no qwen2.5:0.5b: {body}"
    );
    v.trim().to_string()
}

#[test]
#[ignore = "provisioned: needs HARNESS_OPENCODE (opencode) and local Ollama qwen2.5:0.5b at 127.0.0.1:11434; no paid calls"]
fn hn14_opencode_reaches_the_same_catalog_through_mcp() {
    let opencode = required_tool("HARNESS_OPENCODE");
    let version = opencode_ready(&opencode);
    let root: PathBuf = fixture_dir("hn-hn14-opencode").canonicalize().unwrap();
    let project = root.join("project");
    let home = root.join("home");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let log = root.join("delivery.log");
    // Fixture config for a second client path: only a temp project config and temp XDG dirs.
    let config = json!({"$schema": "https://opencode.ai/config.json",
        "provider": {"ollama": {"npm": "@ai-sdk/openai-compatible", "name": "Ollama",
            "options": {"baseURL": "http://127.0.0.1:11434/v1"}, "models": {"qwen2.5:0.5b": {"name": "qwen"}}}},
        "mcp": {"semaprax-skills": {"type": "local", "enabled": true,
            "command": [harness_bin().to_str().unwrap(), "bridge", project.to_str().unwrap(), "--mcp", "--session", "hn14-opencode", "--log", log.to_str().unwrap()],
            "environment": {"SEMAPRAX_HARNESS_HOME": home.to_str().unwrap()}}}});
    write(&project, "opencode.json", &config.to_string());
    let (out_path, err_path) = (root.join("run.json"), root.join("run.err"));
    // The first run against fresh XDG state can fail while opencode initializes its database; retry (observed: the first runs fail until it settles).
    for attempt in 0..4 {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_secs(3));
        }
        let mut child = Command::new(&opencode)
            .current_dir(&project)
            .env("PWD", &project)
            .args(["run", "--pure", "-m", "ollama/qwen2.5:0.5b", "--format", "json", "--print-logs", "--log-level", "ERROR",
                   "Call the tool semaprax-skills_skills_use with name ponytail and mode lite, then say done."])
            .env("XDG_CONFIG_HOME", root.join("xdg/c")).env("XDG_DATA_HOME", root.join("xdg/d"))
            .env("XDG_CACHE_HOME", root.join("xdg/k")).env("XDG_STATE_HOME", root.join("xdg/s"))
            .stdout(std::fs::File::create(&out_path).unwrap())
            .stderr(std::fs::File::create(&err_path).unwrap())
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(240);
        while child.try_wait().unwrap().is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let _ = child.kill();
        let _ = child.wait();
        if !std::fs::read_to_string(&out_path)
            .unwrap()
            .contains("\"type\":\"error\"")
        {
            break;
        }
    }
    let run = std::fs::read_to_string(&out_path).unwrap();
    eprintln!("opencode {version} events:\n{run}");
    let events = jsonl(&log);
    eprintln!(
        "bridge log:\n{}",
        events
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(events[0]["event"], "skills.session");
    assert_eq!(events[0]["client"], "opencode");
    let delivered = events
        .iter()
        .find(|e| e["event"] == "skills.delivered")
        .expect("opencode's model called skills_use");
    assert_eq!(
        delivered["revision"],
        catalog_digest(&home, "ponytail").as_str()
    );
    assert_eq!(delivered["skill"], "ponytail");
    let pid = semaprax_harness::skills::cli_defaults::project_id(&project);
    let status = harness(
        &[
            "skills",
            "status",
            "--json",
            "--project",
            &pid,
            "--session",
            "hn14-opencode",
        ],
        &home,
    );
    assert_eq!(
        mode_and_lock(&status, "ponytail"),
        ("lite".into(), json!(delivered["revision"]))
    );
}
