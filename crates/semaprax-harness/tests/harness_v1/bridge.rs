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
    assert!(!r[4]["result"]["native"].as_array().unwrap().is_empty());
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

// ---- HN-14: default skills through the bridge (protocol v2) and MCP ----

use semaprax_harness::bridge::negotiate::PROTOCOL_V2;
use semaprax_harness::skills::official::OfficialSet;

fn hello2(caps: Value, extra: Value) -> Value {
    let mut v = json!({"protocol": PROTOCOL_V2, "version": 2, "host": {"name": "t", "version": "1"}, "capabilities": caps});
    for (k, x) in extra.as_object().cloned().unwrap_or_default() {
        v[k] = x;
    }
    v
}

fn v2(w: &World, extra: Value) -> (Server<'_>, Value) {
    let mut srv = Server::new(&w.env, &w.project);
    let r = srv
        .handle("bridge/handshake", &hello2(json!({}), extra))
        .unwrap();
    (srv, r)
}

fn official_skill_md(id: &str) -> Vec<u8> {
    OfficialSet::embedded()
        .file_bytes(id, "SKILL.md")
        .unwrap()
        .to_vec()
}

fn facts(status: &Value) -> BTreeMap<String, (String, Value)> {
    status["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| {
            (
                k["id"].as_str().unwrap().to_string(),
                (
                    k["mode"].as_str().unwrap().to_string(),
                    k["locked_revision"].clone(),
                ),
            )
        })
        .collect()
}

#[test]
fn hn14_v2_serves_the_same_catalog_as_the_cli_without_a_prompt_copy() {
    let w = world();
    let (mut srv, hs) = v2(&w, json!({"session": "s-cat"}));
    assert_eq!(hs["protocol"], PROTOCOL_V2);
    assert!(hs["methods"]
        .as_array()
        .unwrap()
        .contains(&json!("bridge/skills/load")));
    let pid = hs["identity"]["project"].as_str().unwrap().to_string();
    assert_eq!(hs["identity"]["session"], "s-cat");
    let list = srv.handle("bridge/skills/list", &json!({})).unwrap();
    let ids: Vec<&str> = list["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["id"].as_str().unwrap())
        .collect();
    assert!(
        ids.contains(&"ponytail") && ids.contains(&"caveman"),
        "{ids:?}"
    );
    let loaded = srv
        .handle("bridge/skills/load", &json!({"name": "ponytail"}))
        .unwrap();
    let d = &loaded["delivery"];
    assert_eq!(d["state"], "delivered");
    let ponytail = list["skills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| k["id"] == "ponytail")
        .unwrap();
    assert_eq!(
        d["revision"], ponytail["bundle_digest"],
        "delivered revision is the catalog digest"
    );
    // The CLI renders byte-identical text for the same project/session.
    let cli = run(
        &s(&[
            "skills",
            "load",
            "ponytail",
            "--json",
            "--project",
            &pid,
            "--session",
            "s-cat",
        ]),
        &w.env,
    );
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    let cli_doc: Value = serde_json::from_str(&cli.stdout).unwrap();
    assert_eq!(cli_doc["text"], d["text"]);
    let skill_md = String::from_utf8(official_skill_md("ponytail")).unwrap();
    assert!(
        d["text"]
            .as_str()
            .unwrap()
            .contains(skill_md.lines().nth(5).unwrap().trim())
            || d["text"].as_str().unwrap().contains("HOST POLICY")
    );
    // resource retrieval is bounded and exact.
    let lic = srv
        .handle(
            "bridge/skills/resource",
            &json!({"name": "ponytail", "path": "LICENSE"}),
        )
        .unwrap();
    assert!(lic["text"]
        .as_str()
        .unwrap()
        .to_lowercase()
        .contains("license"));
    assert_eq!(
        srv.handle(
            "bridge/skills/resource",
            &json!({"name": "ponytail", "path": "../x"})
        )
        .unwrap_err()
        .code,
        "SPX-HPM033"
    );
}

#[test]
fn hn14_v1_sessions_and_unsupported_hosts_fail_clearly() {
    let w = world();
    let mut srv = Server::new(&w.env, &w.project);
    srv.handle("bridge/handshake", &hello(json!({}), Value::Null))
        .unwrap();
    assert_eq!(
        srv.handle("bridge/skills/list", &json!({}))
            .unwrap_err()
            .code,
        "SPX-HPN004"
    );
    let mut with_session = hello(json!({}), Value::Null);
    with_session["session"] = json!("s");
    assert_eq!(
        srv.handle("bridge/handshake", &with_session)
            .unwrap_err()
            .code,
        "SPX-HPN001"
    );
    // v2 protocol string needs version 2.
    let mut mismatch = hello2(json!({}), json!({}));
    mismatch["version"] = json!(1);
    assert_eq!(
        srv.handle("bridge/handshake", &mismatch).unwrap_err().code,
        "SPX-HPN001"
    );
    // Claude Code older than the 2.x line is refused with a version message.
    let mut old = hello2(json!({}), json!({}));
    old["host"] = json!({"name": "claude-code", "version": "1.0.3"});
    let e = srv.handle("bridge/handshake", &old).unwrap_err();
    assert_eq!(e.code, "SPX-HPN007");
    assert!(
        e.message.contains("1.0.3") && e.message.contains("2.1.289"),
        "{}",
        e.message
    );
    // Unknown params are refused, not ignored.
    let (mut srv2, _) = v2(&w, json!({}));
    assert_eq!(
        srv2.handle("bridge/skills/load", &json!({"name": "ponytail", "x": 1}))
            .unwrap_err()
            .code,
        "SPX-HPN005"
    );
    assert_eq!(
        srv2.handle("bridge/skills/load", &json!({"name": "no-such"}))
            .unwrap_err()
            .code,
        "SPX-HPM038"
    );
}

#[test]
fn hn14_modes_off_and_locked_revisions_propagate_with_identity() {
    let w = world();
    let (mut srv, hs) = v2(&w, json!({"session": "s-mode"}));
    let pid = hs["identity"]["project"].as_str().unwrap().to_string();
    let used = srv
        .handle(
            "bridge/skills/use",
            &json!({"name": "ponytail", "mode": "ultra"}),
        )
        .unwrap();
    assert_eq!(used["delivery"]["mode"], "ultra");
    assert!(used["delivery"]["text"]
        .as_str()
        .unwrap()
        .contains("active-mode: ultra"));
    let st = srv.handle("bridge/skills/status", &json!({})).unwrap();
    let f = facts(&st);
    assert_eq!(f["ponytail"].0, "ultra");
    assert_eq!(
        f["ponytail"].1, used["delivery"]["revision"],
        "the lock is the delivered revision"
    );
    assert_eq!(st["session"], "s-mode");
    assert_eq!(st["project"], pid.as_str());
    assert_eq!(
        st["delivery"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["event"] == "skills.delivered")
            .count(),
        1
    );
    let ev = st["delivery"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["event"] == "skills.delivered")
        .unwrap();
    assert_eq!(ev["revision"], used["delivery"]["revision"]);
    // Caveman: explicit on, then explicit stop; the stop is final for the session.
    srv.handle("bridge/skills/use", &json!({"name": "caveman"}))
        .unwrap();
    assert_eq!(
        facts(&srv.handle("bridge/skills/status", &json!({})).unwrap())["caveman"].0,
        "on"
    );
    srv.handle("bridge/skills/off", &json!({"name": "caveman"}))
        .unwrap();
    let after = srv.handle("bridge/skills/status", &json!({})).unwrap();
    assert_eq!(facts(&after)["caveman"].0, "off");
    assert!(
        after["status_lines"]
            .as_array()
            .unwrap()
            .iter()
            .any(|l| l.as_str().unwrap() == "Caveman mode: off"),
        "{after}"
    );
    // A mode the catalog does not declare is refused (never a silent switch).
    assert!(srv
        .handle(
            "bridge/skills/use",
            &json!({"name": "caveman", "mode": "ultracave"})
        )
        .is_err());
    // A second session has independent state.
    let (mut other, _) = v2(&w, json!({"session": "s-other"}));
    assert_eq!(
        facts(&other.handle("bridge/skills/status", &json!({})).unwrap())["ponytail"].1,
        Value::Null
    );
}

#[test]
fn hn14_status_agrees_across_cli_bridge_and_editor_helper() {
    let w = world();
    let (mut srv, hs) = v2(&w, json!({"session": "s-agree"}));
    let pid = hs["identity"]["project"].as_str().unwrap().to_string();
    srv.handle(
        "bridge/skills/use",
        &json!({"name": "ponytail", "mode": "lite"}),
    )
    .unwrap();
    srv.handle("bridge/skills/use", &json!({"name": "caveman"}))
        .unwrap();
    srv.handle("bridge/skills/off", &json!({"name": "caveman"}))
        .unwrap();
    let bridge = srv.handle("bridge/skills/status", &json!({})).unwrap();
    let cli = run(
        &s(&[
            "skills",
            "status",
            "--json",
            "--project",
            &pid,
            "--session",
            "s-agree",
        ]),
        &w.env,
    );
    assert_eq!(cli.code, 0, "{}", cli.stderr);
    let cli_doc: Value = serde_json::from_str(&cli.stdout).unwrap();
    assert_eq!(
        facts(&cli_doc),
        facts(&bridge),
        "CLI and bridge agree on mode and pinned revision"
    );
    assert_eq!(facts(&bridge)["ponytail"].0, "lite");
    assert!(facts(&bridge)["ponytail"].1.is_string());
    if Path::new(NODE).exists() {
        let harness_js =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../editors/vscode/harness.js");
        let script = format!(
            "const h=require({:?});let t='';process.stdin.on('data',d=>t+=d).on('end',()=>{{const [a,b]=t.split('\\n====\\n').map(h.parseSkillsStatus);console.log(JSON.stringify({{facts:h.skillsFacts(a),agree:h.skillsAgree(a,b).agree}}))}})",
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
            .write_all(format!("{}\n====\n{}", cli.stdout.trim(), bridge).as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        assert!(out.status.success());
        let editor: Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(editor["agree"], true);
        for (id, (mode, rev)) in facts(&bridge) {
            assert_eq!(editor["facts"][&id]["mode"], mode.as_str());
            assert_eq!(editor["facts"][&id]["lockedRevision"], rev);
        }
    }
    // The bridge can also report pending updates from `updates status` (offline, read-only).
    let with_updates = srv
        .handle("bridge/skills/status", &json!({"updates": true}))
        .unwrap();
    assert!(
        with_updates["updates"].get("available").is_some(),
        "{with_updates}"
    );
}

#[test]
fn hn14_host_installed_skill_is_not_injected_twice() {
    let w = world();
    let digest = semaprax_harness::json::sha256_plain(&official_skill_md("ponytail"));
    // 1. Declared in the handshake with the matching revision.
    let (mut srv, hs) = v2(
        &w,
        json!({"session": "s-own1", "host_skills": [{"name": "ponytail", "digest": digest}]}),
    );
    assert_eq!(
        hs["skill_injection"]["host_owned"][0]["revision"],
        "same-revision"
    );
    assert_eq!(hs["single_owner"]["skill_injection"], "per-skill");
    let r = srv
        .handle("bridge/skills/load", &json!({"name": "ponytail"}))
        .unwrap();
    assert_eq!(r["delivery"]["state"], "host-owned");
    assert!(r["delivery"].get("text").is_none());
    assert!(r["delivery"]["reason"]
        .as_str()
        .unwrap()
        .contains("no duplicate insertion"));
    // The other official skill is still ours to deliver.
    assert_eq!(
        srv.handle("bridge/skills/load", &json!({"name": "caveman"}))
            .unwrap()["delivery"]["state"],
        "delivered"
    );
    // `use` still records the explicit mode while the host owns the text.
    let u = srv
        .handle(
            "bridge/skills/use",
            &json!({"name": "ponytail", "mode": "ultra"}),
        )
        .unwrap();
    assert_eq!(u["delivery"]["state"], "host-owned");
    assert_eq!(
        facts(&srv.handle("bridge/skills/status", &json!({})).unwrap())["ponytail"].0,
        "ultra"
    );
    // Explicit force delivers anyway and says so.
    assert_eq!(
        srv.handle(
            "bridge/skills/load",
            &json!({"name": "ponytail", "force": true})
        )
        .unwrap()["delivery"]["state"],
        "delivered"
    );
    // 2. A different host revision is reported as different and still not duplicated.
    let (mut srv2, hs2) = v2(
        &w,
        json!({"session": "s-own2", "host_skills": [{"name": "ponytail", "digest": "sha256:".to_string() + &"0".repeat(64)}]}),
    );
    assert_eq!(
        hs2["skill_injection"]["host_owned"][0]["revision"],
        "different-revision"
    );
    assert!(srv2
        .handle("bridge/skills/load", &json!({"name": "ponytail"}))
        .unwrap()["delivery"]["reason"]
        .as_str()
        .unwrap()
        .contains("different revision"));
    // 3. Detected read-only from a user-named project `.claude/skills` directory.
    let dir = w.project.join(".claude/skills");
    std::fs::create_dir_all(dir.join("ponytail")).unwrap();
    std::fs::write(dir.join("ponytail/SKILL.md"), official_skill_md("ponytail")).unwrap();
    std::fs::create_dir_all(dir.join("unrelated")).unwrap();
    std::fs::write(
        dir.join("unrelated/SKILL.md"),
        "---\nname: unrelated\ndescription: x\n---\nbody\n",
    )
    .unwrap();
    let before = std::fs::read(dir.join("ponytail/SKILL.md")).unwrap();
    let mut s3 = Server::new(&w.env, &w.project).with_host_skills_dir(Some(dir.clone()));
    let h3 = s3
        .handle(
            "bridge/handshake",
            &hello2(json!({}), json!({"session": "s-own3"})),
        )
        .unwrap();
    let owned = h3["skill_injection"]["host_owned"].as_array().unwrap();
    assert_eq!(owned.len(), 1, "{owned:?}");
    assert_eq!(
        (
            owned[0]["id"].as_str(),
            owned[0]["source"].as_str(),
            owned[0]["revision"].as_str()
        ),
        (
            Some("ponytail"),
            Some("project-skills-dir"),
            Some("same-revision")
        )
    );
    assert_eq!(
        s3.handle("bridge/skills/load", &json!({"name": "ponytail"}))
            .unwrap()["delivery"]["state"],
        "host-owned"
    );
    assert_eq!(
        std::fs::read(dir.join("ponytail/SKILL.md")).unwrap(),
        before,
        "the host's skill is only read"
    );
    // An edited copy is a different revision.
    std::fs::write(
        dir.join("ponytail/SKILL.md"),
        String::from_utf8(before).unwrap() + "\nlocal edit\n",
    )
    .unwrap();
    let mut s4 = Server::new(&w.env, &w.project).with_host_skills_dir(Some(dir));
    let h4 = s4
        .handle("bridge/handshake", &hello2(json!({}), json!({})))
        .unwrap();
    assert_eq!(
        h4["skill_injection"]["host_owned"][0]["revision"],
        "different-revision"
    );
}

#[test]
fn hn14_model_routing_stays_not_delegated_unless_the_host_delegates() {
    let w = world();
    let (mut srv, hs) = v2(&w, json!({}));
    assert_eq!(hs["delegation"]["model_routing"], "not-delegated");
    assert_eq!(cap_owner(&hs, "model_routing"), "external-host");
    let st = srv.handle("bridge/skills/status", &json!({})).unwrap();
    assert_eq!(st["model_routing"], "not-delegated");
    assert!(!st.to_string().contains("Jev") && !st.to_string().contains("Laya"));
    let mut d = Server::new(&w.env, &w.project);
    let hs2 = d
        .handle(
            "bridge/handshake",
            &hello2(json!({"model_routing": true}), json!({})),
        )
        .unwrap();
    assert_eq!(hs2["delegation"]["model_routing"], "delegated");
    assert_eq!(
        d.handle("bridge/skills/status", &json!({})).unwrap()["model_routing"],
        "delegated"
    );
}

#[test]
fn hn14_delivery_observations_are_logged_with_the_exact_revision() {
    let w = world();
    let log = w.home.join("skills-delivery.log");
    let mut srv = Server::new(&w.env, &w.project).with_log(Some(log.clone()));
    srv.handle(
        "bridge/handshake",
        &hello2(json!({}), json!({"session": "s-log"})),
    )
    .unwrap();
    let r = srv
        .handle("bridge/skills/use", &json!({"name": "ponytail"}))
        .unwrap();
    let lines: Vec<Value> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let delivered = lines
        .iter()
        .find(|l| l["event"] == "skills.delivered")
        .unwrap();
    assert_eq!(delivered["revision"], r["delivery"]["revision"]);
    assert_eq!(delivered["session"], "s-log");
    assert_eq!(delivered["bytes"], r["delivery"]["bytes"]);
    // Delivery is not model consumption: applied_to_model stays false.
    let st = srv.handle("bridge/skills/status", &json!({})).unwrap();
    assert!(st["skills"]
        .as_array()
        .unwrap()
        .iter()
        .all(|k| k["applied_to_model"] == false));
    assert!(st["delivery_scope"]
        .as_str()
        .unwrap()
        .contains("not observed"));
}

// ---- MCP transport and setup ----

fn mcp_session(w: &World, frames: &[Value], skills_dir: Option<PathBuf>) -> Vec<Value> {
    let input: String = frames.iter().map(|f| format!("{f}\n")).collect();
    let mut out = Vec::new();
    semaprax_harness::bridge::mcp::serve(
        input.as_bytes(),
        &mut out,
        &w.env,
        &w.project,
        semaprax_harness::bridge::mcp::McpOptions {
            session: Some("s-mcp".into()),
            host_skills_dir: skills_dir,
            log: None,
        },
    )
    .unwrap();
    String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn mcp_init(version: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": version}}})
}

fn call(id: u64, tool: &str, args: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": "tools/call", "params": {"name": tool, "arguments": args}})
}

#[test]
fn hn14_mcp_exposes_tools_and_prompts_over_the_same_catalog() {
    let w = world();
    let r = mcp_session(
        &w,
        &[
            mcp_init("2.1.289"),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            call(3, "skills_list", json!({})),
            call(4, "skills_use", json!({"name": "ponytail", "mode": "lite"})),
            call(5, "skills_status", json!({})),
            call(6, "skills_off", json!({"name": "ponytail"})),
            call(7, "skills_load", json!({"name": "nope"})),
            json!({"jsonrpc": "2.0", "id": 8, "method": "prompts/list"}),
            json!({"jsonrpc": "2.0", "id": 9, "method": "prompts/get", "params": {"name": "caveman"}}),
        ],
        None,
    );
    assert_eq!(r.len(), 9, "the notification gets no reply: {r:?}");
    assert_eq!(r[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(r[0]["result"]["serverInfo"]["name"], "semaprax-skills");
    let names: Vec<&str> = r[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    for n in ["skills_list", "skills_load", "skills_use", "skills_status"] {
        assert!(names.contains(&n), "{names:?}");
    }
    assert!(r[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("ponytail"));
    let used = r[3]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        used.contains("HOST POLICY") && used.contains("active-mode: lite"),
        "{used}"
    );
    let status: Value =
        serde_json::from_str(r[4]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(facts(&status)["ponytail"].0, "lite");
    assert_eq!(r[5]["result"]["isError"], false);
    assert_eq!(
        r[6]["result"]["isError"], true,
        "unknown skill is a tool error"
    );
    let prompts: Vec<&str> = r[7]["result"]["prompts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert!(prompts.contains(&"ponytail") && prompts.contains(&"caveman"));
    assert!(r[8]["result"]["messages"][0]["content"]["text"]
        .as_str()
        .unwrap()
        .contains("HOST POLICY"));
}

#[test]
fn hn14_mcp_refuses_early_calls_and_unsupported_claude_versions() {
    let w = world();
    let early = mcp_session(
        &w,
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})],
        None,
    );
    assert_eq!(early[0]["error"]["data"]["code"], "SPX-HPN004");
    let old = mcp_session(
        &w,
        &[
            mcp_init("1.0.9"),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
        ],
        None,
    );
    assert_eq!(old[0]["error"]["data"]["code"], "SPX-HPN007");
    assert!(old[0]["error"]["message"]
        .as_str()
        .unwrap()
        .contains("unsupported Claude Code version"));
    assert_eq!(
        old[1]["error"]["data"]["code"], "SPX-HPN004",
        "no session after a refused initialize"
    );
    let unknown = mcp_session(
        &w,
        &[
            mcp_init("2.1.289"),
            json!({"jsonrpc": "2.0", "id": 2, "method": "resources/list"}),
        ],
        None,
    );
    assert_eq!(unknown[1]["error"]["code"], -32601);
}

#[test]
fn hn14_mcp_respects_a_host_installed_skill_directory() {
    let w = world();
    let dir = w.project.join(".claude/skills");
    std::fs::create_dir_all(dir.join("caveman")).unwrap();
    std::fs::write(dir.join("caveman/SKILL.md"), official_skill_md("caveman")).unwrap();
    let r = mcp_session(
        &w,
        &[
            mcp_init("2.1.289"),
            call(2, "skills_load", json!({"name": "caveman"})),
            call(3, "skills_load", json!({"name": "ponytail"})),
        ],
        Some(dir),
    );
    let owned = r[1]["result"]["content"][0]["text"].as_str().unwrap();
    assert!(owned.contains("no duplicate insertion"), "{owned}");
    assert!(r[2]["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .contains("HOST POLICY"));
}

#[test]
fn hn14_setup_prints_by_default_writes_only_with_write_and_preserves_user_config() {
    use semaprax_harness::bridge::cli_bridge;
    let before_global = global_fingerprint();
    let w = world();
    let p = w.project.to_str().unwrap();
    let bin = harness_bin();
    let args = |extra: &[&str]| {
        let mut a = s(&[
            p,
            "--setup",
            "claude-code",
            "--harness-bin",
            bin.to_str().unwrap(),
        ]);
        a.extend(s(extra));
        a
    };
    let plan = cli_bridge(&args(&["--session", "team"]), &w.env);
    assert_eq!(plan.code, 0, "{}", plan.stderr);
    let doc: Value = serde_json::from_str(&plan.stdout).unwrap();
    assert_eq!(
        (
            doc["mode"].as_str(),
            doc["action"].as_str(),
            doc["written"].as_bool()
        ),
        (Some("plan"), Some("create"), Some(false))
    );
    assert!(
        !w.project.join(".mcp.json").exists(),
        "plan changes nothing"
    );
    let entry = &doc["entry"];
    let flat = entry["args"].to_string();
    assert!(
        flat.contains("\"--mcp\"")
            && flat.contains("--host-skills-dir")
            && flat.contains("\"team\""),
        "{flat}"
    );
    assert!(
        !plan.stdout.contains("HOST POLICY"),
        "no skill text is copied"
    );
    // Existing user config is merged, not replaced.
    std::fs::write(w.project.join(".mcp.json"), r#"{"mcpServers":{"other":{"type":"http","url":"https://example.com/mcp"}},"custom":{"keep":[1,2]}}"#).unwrap();
    let written = cli_bridge(&args(&["--session", "team", "--write"]), &w.env);
    assert_eq!(written.code, 0, "{}", written.stderr);
    assert_eq!(
        serde_json::from_str::<Value>(&written.stdout).unwrap()["action"],
        "add"
    );
    let merged: Value =
        serde_json::from_str(&std::fs::read_to_string(w.project.join(".mcp.json")).unwrap())
            .unwrap();
    assert_eq!(
        merged["mcpServers"]["other"],
        json!({"type": "http", "url": "https://example.com/mcp"})
    );
    assert_eq!(merged["custom"], json!({"keep": [1, 2]}));
    assert_eq!(merged["mcpServers"]["semaprax-skills"], *entry);
    assert!(!w.project.join(".mcp.json.semaprax-tmp").exists());
    // Idempotent.
    let again = cli_bridge(&args(&["--session", "team", "--write"]), &w.env);
    let again: Value = serde_json::from_str(&again.stdout).unwrap();
    assert_eq!(
        (again["action"].as_str(), again["written"].as_bool()),
        (Some("noop"), Some(false))
    );
    // A malformed existing file is refused and left alone.
    std::fs::write(w.project.join(".mcp.json"), "{not json").unwrap();
    let bad = cli_bridge(&args(&["--write"]), &w.env);
    assert_eq!(bad.code, 1);
    assert!(bad.stderr.contains("SPX-HPN011"), "{}", bad.stderr);
    assert_eq!(
        std::fs::read_to_string(w.project.join(".mcp.json")).unwrap(),
        "{not json"
    );
    // Unknown host and --write without --setup fail clearly.
    let other = cli_bridge(&s(&[p, "--setup", "cursor"]), &w.env);
    assert!(
        other.stderr.contains("SPX-HPN007") && other.stderr.contains("claude-code"),
        "{}",
        other.stderr
    );
    assert_eq!(cli_bridge(&s(&[p, "--stdio", "--write"]), &w.env).code, 2);
    assert_eq!(global_fingerprint(), before_global);
}

#[test]
fn hn14_real_process_generic_mcp_client_over_stdio() {
    // A generic stdio MCP client (no Claude Code) against the real binary.
    let w = world();
    let mut child = Command::new(harness_bin())
        .args([
            "bridge",
            w.project.to_str().unwrap(),
            "--mcp",
            "--session",
            "s-proc",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("SEMAPRAX_HARNESS_HOME", &w.home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::{BufRead, BufReader, Write};
    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut ask = |f: Value| -> Value {
        writeln!(stdin, "{f}").unwrap();
        stdin.flush().unwrap();
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    };
    let init = ask(
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "generic-stdio-client", "version": "0.1"}}}),
    );
    assert_eq!(init["result"]["protocolVersion"], "2024-11-05");
    let used = ask(call(2, "skills_use", json!({"name": "caveman"})));
    let text = used["result"]["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        text.contains("HOST POLICY") && text.contains("caveman"),
        "{text}"
    );
    drop(stdin);
    assert!(child.wait().unwrap().success());
    // The CLI sees the state the generic client wrote (same project id, session).
    let status = run(
        &s(&[
            "skills",
            "status",
            "--json",
            "--session",
            "s-proc",
            "--project",
            &semaprax_harness::skills::cli_defaults::project_id(&w.project.canonicalize().unwrap()),
        ]),
        &w.env,
    );
    let doc: Value = serde_json::from_str(&status.stdout).unwrap();
    assert_eq!(facts(&doc)["caveman"].0, "on");
}
