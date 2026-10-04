//! HP-14 bridge tests (fixture prefix `hp-hp14`).

use crate::support::{fixture_dir, harness_bin, write};
use semaprax_harness::bridge::negotiate::{DEPTH_VAR, PROTOCOL};
use semaprax_harness::bridge::rpc::Server;
use semaprax_harness::cli::{run, Environment};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const NODE: &str = "/Users/kevin/.nvm/versions/node/v24.3.0/bin/node";
const PREBUILT: &str = "/Users/kevin/Documents/ChatGPT/AI-Lang-v090/target/debug/semaprax";

struct World {
    home: PathBuf,
    project: PathBuf,
    env: Environment,
}

fn compiler() -> Option<PathBuf> {
    std::env::var_os("SEMAPRAX_COMPILER")
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(PREBUILT)).filter(|p| p.exists()))
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let (p, q) = (e.path(), to.join(e.file_name()));
        if p.is_dir() {
            copy_tree(&p, &q);
        } else {
            std::fs::copy(&p, &q).unwrap();
        }
    }
}

fn world() -> World {
    let root = fixture_dir("hp-hp14").canonicalize().unwrap();
    let project = root.join("project");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bridge/project"),
        &project,
    );
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let vars = BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]);
    let env = Environment {
        harness_home: Some(home.clone()),
        compiler: compiler(),
        cwd: root,
        vars,
    };
    World { home, project, env }
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

fn hello(caps: Value, rewriter: Value) -> Value {
    json!({"protocol": PROTOCOL, "version": 1, "host": {"name": "t", "version": "1"}, "capabilities": caps, "command_rewriter": rewriter})
}

fn cap_owner(r: &Value, cap: &str) -> String {
    r["capabilities"][cap]["owner"]
        .as_str()
        .unwrap()
        .to_string()
}

/// Global Claude settings fingerprint (read-only): bytes and mtime, or absent.
fn global_fingerprint() -> Option<(Vec<u8>, std::time::SystemTime)> {
    let p = PathBuf::from(std::env::var_os("HOME")?).join(".claude/settings.json");
    Some((
        std::fs::read(&p).ok()?,
        std::fs::metadata(&p).ok()?.modified().ok()?,
    ))
}

fn hook_input(tool: &str, cmd: &str) -> String {
    json!({"session_id": "s", "cwd": "/", "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": {"command": cmd, "description": "d"}}).to_string()
}

fn hook_opts(w: &World, settings: Vec<String>) -> semaprax_harness::bridge::claude::HookOptions {
    semaprax_harness::bridge::claude::HookOptions {
        project: w.project.clone(),
        harness_bin: harness_bin(),
        settings,
    }
}

#[test]
fn hp_hp14_handshake_gives_one_owner_per_capability() {
    let w = world();
    let mut srv = Server::new(&w.env, &w.project);
    let r = srv
        .handle("bridge/handshake", &hello(json!({"semantic_query": true, "command_wrapper": true, "cancellation": true, "publication": true}), Value::Null))
        .unwrap();
    assert_eq!(cap_owner(&r, "command_wrapper"), "semaprax");
    assert_eq!(cap_owner(&r, "cancellation"), "semaprax");
    // Undeclared capabilities are host-owned and never claimed optimized.
    for c in ["tool_result_observation", "model_routing"] {
        assert_eq!(cap_owner(&r, c), "external-host", "{c}");
        assert!(r["not_claimed_optimized"]
            .as_array()
            .unwrap()
            .contains(&json!(c)));
    }
    // Publication is never ours, even if the host claims to delegate it.
    assert_eq!(cap_owner(&r, "publication"), "external-host");
    assert_eq!(r["single_owner"]["retries"], "external-host");
    assert_eq!(
        r["single_owner"]["command_interception"],
        r["single_owner"]["compression"]
    );
    assert!(r["capabilities"]["model_routing"]["reason"]
        .as_str()
        .unwrap()
        .contains("host-controlled"));
    // Observed scope is labelled, no whole-session claim.
    assert_eq!(
        r["observed_scope"]["observed"],
        "semaprax-routed-calls-only"
    );
    assert_eq!(r["observed_scope"]["whole_session_savings_claimed"], false);
}

#[test]
fn hp_hp14_existing_rewriter_keeps_command_wrapper_with_the_host() {
    let w = world();
    let mut srv = Server::new(&w.env, &w.project);
    let r = srv
        .handle(
            "bridge/handshake",
            &hello(json!({"command_wrapper": true}), json!("rtk")),
        )
        .unwrap();
    assert_eq!(cap_owner(&r, "command_wrapper"), "external-host");
    assert_eq!(r["single_owner"]["command_interception"], "external-host");
    assert!(r["capabilities"]["command_wrapper"]["reason"]
        .as_str()
        .unwrap()
        .contains("does not wrap again"));
    // The delegated command still runs once, but Semaprax leaves the view to the owner.
    let out = srv
        .handle("bridge/command_view", &json!({"argv": ["echo", "once"]}))
        .unwrap();
    assert_eq!(out["exit_code"], 0);
    assert_eq!(out["envelope"]["result"]["executions"], 1);
    assert!(
        out["envelope"]["result"]["lineage"]
            .to_string()
            .contains("external:rtk"),
        "{out}"
    );
}

#[test]
fn hp_hp14_failures_are_safe_and_leave_global_settings_alone() {
    let before = global_fingerprint();
    let w = world();
    // incompatible protocol / version / unknown capability / missing handshake
    let mut srv = Server::new(&w.env, &w.project);
    let mut bad = hello(json!({}), Value::Null);
    bad["protocol"] = json!("semaprax.harness-bridge.v0");
    assert_eq!(
        srv.handle("bridge/handshake", &bad).unwrap_err().code,
        "SPX-HPN001"
    );
    let mut v2 = hello(json!({}), Value::Null);
    v2["version"] = json!(2);
    assert_eq!(
        srv.handle("bridge/handshake", &v2).unwrap_err().code,
        "SPX-HPN001"
    );
    assert_eq!(
        srv.handle(
            "bridge/handshake",
            &hello(json!({"teleport": true}), Value::Null)
        )
        .unwrap_err()
        .code,
        "SPX-HPN001"
    );
    assert_eq!(
        srv.handle("bridge/status", &json!({})).unwrap_err().code,
        "SPX-HPN004"
    );
    // denied publication
    srv.handle(
        "bridge/handshake",
        &hello(json!({"publication": true}), Value::Null),
    )
    .unwrap();
    assert_eq!(
        srv.handle("bridge/publish", &json!({"artifact": "x"}))
            .unwrap_err()
            .code,
        "SPX-HPN003"
    );
    assert_eq!(
        srv.handle("bridge/nonsense", &json!({})).unwrap_err().code,
        "SPX-HPN004"
    );
    // recursive call: environment marker, handshake depth and lineage all refuse
    let mut nested = w.env.clone();
    nested.vars.insert(DEPTH_VAR.into(), "1".into());
    let mut rec = Server::new(&nested, &w.project);
    assert_eq!(
        rec.handle("bridge/handshake", &hello(json!({}), Value::Null))
            .unwrap_err()
            .code,
        "SPX-HPN002"
    );
    let mut fresh = Server::new(&w.env, &w.project);
    let mut deep = hello(json!({}), Value::Null);
    deep["bridge_depth"] = json!(1);
    assert_eq!(
        fresh.handle("bridge/handshake", &deep).unwrap_err().code,
        "SPX-HPN002"
    );
    let mut lin = hello(json!({}), Value::Null);
    lin["lineage"] = json!(["semaprax-mcp"]);
    assert_eq!(
        fresh.handle("bridge/handshake", &lin).unwrap_err().code,
        "SPX-HPN002"
    );
    // existing RTK hook: hook passes through and print-config refuses a competitor
    let rtk = r#"{"hooks":{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"rtk hook claude"}]}]}}"#;
    let r = semaprax_harness::bridge::claude::pre_tool_use(
        &hook_input("Bash", "git log -3"),
        &hook_opts(&w, vec![rtk.into()]),
        &w.env,
    )
    .unwrap();
    assert!(r.stdout.is_none());
    assert!(r.record["reason"].as_str().unwrap().contains("RTK"));
    let sf = write(&w.home, "settings.json", rtk);
    let o = run(
        &s(&[
            "bridge",
            w.project.to_str().unwrap(),
            "--host",
            "claude-code",
            "--print-config",
            "--settings-file",
            sf.to_str().unwrap(),
        ]),
        &w.env,
    );
    assert_eq!(o.code, 1);
    assert!(o.stderr.contains("SPX-HPN009"), "{}", o.stderr);
    // unknown host
    let o = run(
        &s(&["bridge", w.project.to_str().unwrap(), "--host", "cursor"]),
        &w.env,
    );
    assert!(o.stderr.contains("SPX-HPN007"));
    assert_eq!(
        global_fingerprint(),
        before,
        "global Claude settings must be untouched"
    );
}

#[test]
fn hp_hp14_hook_rewrites_only_admitted_bash_commands() {
    let w = world();
    let o = hook_opts(&w, vec![]);
    let hook = |tool: &str, cmd: &str| {
        semaprax_harness::bridge::claude::pre_tool_use(&hook_input(tool, cmd), &o, &w.env).unwrap()
    };
    let r = hook("Bash", "echo 'a b'");
    let out = r.stdout.expect("rewritten");
    let cmd = out["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PreToolUse");
    assert!(
        out["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none(),
        "the hook grants no permission"
    );
    assert_eq!(
        out["hookSpecificOutput"]["updatedInput"]["description"], "d",
        "other input fields are kept"
    );
    assert!(
        cmd.starts_with(&format!(
            "'{}' 'exec' '{}'",
            harness_bin().display(),
            w.project.display()
        )),
        "{cmd}"
    );
    assert!(cmd.ends_with("'--' 'echo' 'a b'"), "{cmd}");
    for (tool, c, why) in [
        ("Read", "x", "not a Bash"),
        ("Bash", "ls | wc", "not an admitted plain command"),
        ("Bash", "echo $HOME", "not an admitted plain command"),
        ("Bash", "definitely-not-a-command x", "not resolvable"),
        ("Bash", "sh -c ls", "not admitted by command_view"),
        ("Bash", "/bin/echo --json", "excluded"),
    ] {
        let r = hook(tool, c);
        assert!(r.stdout.is_none(), "{c}");
        assert!(
            r.record["reason"].as_str().unwrap().contains(why),
            "{c}: {}",
            r.record
        );
    }
    // nested inside a Semaprax-owned command: no second wrap
    let mut nested = w.env.clone();
    nested.vars.insert(DEPTH_VAR.into(), "1".into());
    let r =
        semaprax_harness::bridge::claude::pre_tool_use(&hook_input("Bash", "echo hi"), &o, &nested)
            .unwrap();
    assert!(r.stdout.is_none());
    // malformed hook input is refused, not guessed
    assert_eq!(
        semaprax_harness::bridge::claude::pre_tool_use("{", &o, &w.env)
            .unwrap_err()
            .code,
        "SPX-HPN006"
    );
}

#[test]
fn hp_hp14_print_config_is_read_only_output() {
    let before = global_fingerprint();
    let w = world();
    let o = run(
        &s(&[
            "bridge",
            w.project.to_str().unwrap(),
            "--host",
            "claude-code",
            "--print-config",
            "--harness-bin",
            harness_bin().to_str().unwrap(),
        ]),
        &w.env,
    );
    assert_eq!(o.code, 0, "{}", o.stderr);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    let cmd = v["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert_eq!(v["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    assert!(cmd.contains("'--hook' 'pre-tool-use'"), "{cmd}");
    assert!(o.stderr.contains("never writes agent configuration"));
    assert!(
        !w.project.join(".claude").exists(),
        "nothing is written into the project"
    );
    assert_eq!(global_fingerprint(), before);
}

#[test]
fn hp_hp14_claude_code_profile_is_host_controlled_for_models() {
    let w = world();
    let o = run(
        &s(&[
            "bridge",
            w.project.to_str().unwrap(),
            "--host",
            "claude-code",
        ]),
        &w.env,
    );
    assert_eq!(o.code, 0);
    let v: Value = serde_json::from_str(&o.stdout).unwrap();
    assert_eq!(v["pinned_version"], "2.1.289");
    assert_eq!(v["capabilities"]["model_routing"]["owner"], "external-host");
    assert!(v["capabilities"]["model_routing"]["reason"]
        .as_str()
        .unwrap()
        .contains("host-controlled"));
    assert_eq!(
        v["observed_scope"]["observed"],
        "semaprax-routed-bash-calls-only"
    );
    assert_eq!(v["observed_scope"]["whole_session_savings_claimed"], false);
}

fn node_selected(status: &str) -> Option<Value> {
    if !Path::new(NODE).exists() {
        eprintln!("skipped: node not found at {NODE}");
        return None;
    }
    let harness_js = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editors/vscode/harness.js");
    let script = format!(
        "const h=require({:?});let t='';process.stdin.on('data',d=>t+=d).on('end',()=>console.log(JSON.stringify(h.selectedProviders(h.parseStatus(t)))))",
        harness_js.canonicalize().unwrap().to_str().unwrap()
    );
    let mut child = Command::new(NODE)
        .args(["-e", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(status.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    Some(serde_json::from_slice(&out.stdout).unwrap())
}

#[test]
fn hp_hp14_cli_bridge_and_editor_helper_agree_on_one_profile() {
    let w = world();
    write(&w.project, "semaprax.harness.toml", "schema = \"semaprax.harness-config.v1\"\n\n[capability.\"skill.catalog\"]\nmode = \"disabled\"\n");
    let cli = run(
        &s(&["status", "--json", "--project", w.project.to_str().unwrap()]),
        &w.env,
    );
    let mut srv = Server::new(&w.env, &w.project);
    srv.handle("bridge/handshake", &hello(json!({}), Value::Null))
        .unwrap();
    let via_bridge = srv.handle("bridge/status", &json!({})).unwrap();
    let cli_doc: Value = serde_json::from_str(&cli.stdout).unwrap();
    assert_eq!(
        via_bridge, cli_doc,
        "bridge/status is the same document as `status --json`"
    );
    let mut expected = serde_json::Map::new();
    for b in cli_doc["bindings"].as_array().unwrap() {
        if !b["provider_id"].as_str().unwrap().is_empty() {
            expected.insert(
                b["kind"].as_str().unwrap().to_string(),
                b["provider_id"].clone(),
            );
        }
    }
    assert!(expected.contains_key("command.view"), "{cli_doc}");
    if let Some(editor) = node_selected(&cli.stdout) {
        assert_eq!(
            editor,
            Value::Object(expected),
            "editor helper selects the same providers"
        );
    }
}

#[test]
fn hp_hp14_headless_stdio_fake_host_session() {
    let Some(compiler) = compiler() else {
        eprintln!("skipped: needs a built semaprax (SEMAPRAX_COMPILER)");
        return;
    };
    let python = std::env::var("HARNESS_PYTHON").unwrap_or_else(|_| "/usr/bin/python3".into());
    let w = world();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bridge/fake_host.py");
    let before = global_fingerprint();
    let out = Command::new(python)
        .arg(fixture)
        .args([
            harness_bin().to_str().unwrap(),
            "bridge",
            w.project.to_str().unwrap(),
            "--stdio",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SEMAPRAX_HARNESS_HOME", &w.home)
        .env("SEMAPRAX_COMPILER", compiler)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    let r = doc["replies"].as_array().unwrap();
    assert_eq!(doc["exit"], 0);
    assert_eq!(
        r[0]["error"]["data"]["code"], "SPX-HPN004",
        "status before handshake"
    );
    assert_eq!(
        r[1]["error"]["data"]["code"], "SPX-HPN001",
        "incompatible protocol"
    );
    assert_eq!(cap_owner(&r[2]["result"], "command_wrapper"), "semaprax");
    assert_eq!(cap_owner(&r[2]["result"], "semantic_query"), "semaprax");
    assert_eq!(r[3]["result"]["schema"], "semaprax.harness-status.v1");
    assert_eq!(
        r[4]["result"]["schema"], "semaprax.harness-context.v1",
        "{}",
        r[4]
    );
    assert!(r[4]["result"]["native"].as_array().unwrap().len() > 0);
    assert_eq!(r[5]["result"]["exit_code"], 0);
    assert_eq!(r[5]["result"]["envelope"]["result"]["executions"], 1);
    assert!(r[5]["result"]["display"]
        .as_str()
        .unwrap()
        .contains("hello-bridge"));
    assert_eq!(r[6]["error"]["data"]["code"], "SPX-HPN003");
    assert_eq!(r[7]["result"]["cancelled"], false);
    assert_eq!(global_fingerprint(), before);
}

#[test]
fn hp_hp14_hook_process_logs_and_rewrites_over_real_stdin() {
    let w = world();
    let log = w.home.join("bridge.log");
    let mut child = Command::new(harness_bin())
        .args([
            "bridge",
            w.project.to_str().unwrap(),
            "--host",
            "claude-code",
            "--hook",
            "pre-tool-use",
            "--log",
            log.to_str().unwrap(),
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child
        .stdin
        .take()
        .unwrap()
        .write_all(hook_input("Bash", "echo hi").as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap()
        .contains("'exec'"));
    let line = std::fs::read_to_string(&log).unwrap();
    assert_eq!(line.lines().count(), 1);
    assert_eq!(
        serde_json::from_str::<Value>(line.trim()).unwrap()["decision"],
        "rewritten"
    );
}
