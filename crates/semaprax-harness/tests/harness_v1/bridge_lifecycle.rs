//! HN-18 bridge lifecycle: a real hostile adapter blocked behind `bridge/invoke`
//! is cancelled by a second frame over stdio (fixture prefix `hp-hn18`).

use crate::support::{fixture_dir, harness_bin, write};
use semaprax_harness::bridge::negotiate::PROTOCOL;
use semaprax_harness::bridge::rpc::{serve_with, Server};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::host::{HostConfig, IsolationBackend};
use semaprax_harness::profile::resolve::current_platform;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const ID: &str = "org.example/hostile";

fn python() -> PathBuf {
    let path = std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join("python3"))
                .find(|p| p.is_file())
        });
    path.expect("python3 not found: the adapter-dependent bridge cells cannot run here")
}

struct World {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    env: Environment,
}

fn pid_alive(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

fn assert_gone(pid: i32) {
    let end = Instant::now() + Duration::from_secs(8);
    while pid_alive(pid) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!pid_alive(pid), "pid {pid} survived");
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

/// Adopt and trust a copy of the hostile adapter fixed to `mode`; the project
/// selects it for `decision.evaluate`.
fn world(mode: &str) -> World {
    let root = fixture_dir("hp-hn18").canonicalize().unwrap();
    let ex = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/semaprax-harness-adapters/examples/hostile-python");
    let pidfile = root.join("pids");
    let src = std::fs::read_to_string(ex.join("adapter.py"))
        .unwrap()
        .replace(
            "os.environ.get(\"HOSTILE_MODE\", \"\")",
            &format!("\"{mode}\""),
        )
        .replace(
            "\"decision.evaluate\": [\"evaluate\"]}",
            "\"decision.evaluate\": [\"evaluate\"], \"model.generate\": [\"generate\"]}",
        )
        .replace(
            "os.environ.get(\"HOSTILE_PIDFILE\")",
            &format!("\"{}\"", pidfile.display()),
        );
    write(&root, "adapter/adapter.py", &src);
    let mut d: Value =
        serde_json::from_slice(&std::fs::read(ex.join("harness-provider.json")).unwrap()).unwrap();
    d["platforms"]
        .as_array_mut()
        .unwrap()
        .push(json!(current_platform()));
    d["capabilities"].as_array_mut().unwrap().push(
        json!({"kind": "model.generate", "version": 1, "required": false, "operations": ["generate"]}),
    );
    d["resources"]["invoke_timeout_ms"] = json!(120_000);
    d["resources"]["handshake_timeout_ms"] = json!(10_000);
    write(&root, "adapter/harness-provider.json", &d.to_string());
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    write(&project, "src/lib.rs", "pub fn a() {}\n");
    write(
        &project,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n[capability.\"model.generate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n"),
    );
    let env = Environment {
        harness_home: Some(home.clone()),
        compiler: None,
        cwd: root.clone(),
        vars: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    let desc = root.join("adapter/harness-provider.json");
    let o = run(
        &s(&[
            "adopt",
            desc.to_str().unwrap(),
            "--runtime",
            python().to_str().unwrap(),
        ]),
        &env,
    );
    assert_eq!(o.code, 0, "adopt: {}", o.stderr);
    let o = run(&s(&["trust", ID]), &env);
    assert_eq!(o.code, 0, "trust: {}", o.stderr);
    World {
        root,
        home,
        project,
        env,
    }
}

struct Bridge {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: Receiver<Value>,
    seen: Vec<Value>,
}

impl Bridge {
    fn start(w: &World) -> Bridge {
        let mut child = Command::new(harness_bin())
            .args(["bridge", w.project.to_str().unwrap(), "--stdio"])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("SEMAPRAX_HARNESS_HOME", &w.home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let out = child.stdout.take().unwrap();
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for l in BufReader::new(out).lines().map_while(Result::ok) {
                if let Ok(v) = serde_json::from_str::<Value>(&l) {
                    if tx.send(v).is_err() {
                        break;
                    }
                }
            }
        });
        let mut b = Bridge {
            child,
            stdin,
            rx,
            seen: vec![],
        };
        b.send(json!({"jsonrpc":"2.0","id":0,"method":"bridge/handshake","params":
            {"protocol": PROTOCOL, "version": 1, "host": {"name":"t","version":"1"}, "capabilities": {}, "command_rewriter": null}}));
        let r = b.reply(json!(0));
        assert_eq!(r["result"]["lifecycle"]["invoke"], true, "{r}");
        b
    }
    fn send(&mut self, f: Value) {
        let i = self.stdin.as_mut().unwrap();
        writeln!(i, "{f}").unwrap();
        i.flush().unwrap();
    }
    /// The response for `id`, buffering any other frame that arrives first.
    fn reply(&mut self, id: Value) -> Value {
        if let Some(p) = self.seen.iter().position(|v| v["id"] == id) {
            return self.seen.remove(p);
        }
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let v = self
                .rx
                .recv_timeout(left)
                .expect("timed out waiting for a frame");
            if v["id"] == id {
                return v;
            }
            self.seen.push(v);
        }
    }
    fn generate(&mut self, id: u64, step: &str) {
        self.send(
            json!({"jsonrpc":"2.0","id":id,"method":"bridge/invoke","params":{
            "capability":"model.generate","operation":"generate","deadline_ms":100000,"step":step,
            "payload":{"model":"m.x","input_base64":"aGk=","max_output_bytes":16}}}),
        );
    }
    fn invoke(&mut self, id: u64, deadline_ms: u64) {
        self.send(
            json!({"jsonrpc":"2.0","id":id,"method":"bridge/invoke","params":{
            "capability":"decision.evaluate","operation":"evaluate","deadline_ms":deadline_ms,
            "payload":{"task":"model-route/v1","features":{},"options":["a","b"]}}}),
        );
    }
}

fn wait_pids(w: &World) -> (i32, i32) {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(t) = std::fs::read_to_string(w.root.join("pids")) {
            let v: Vec<i32> = t
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if v.len() == 2 {
                return (v[0], v[1]);
            }
        }
        assert!(Instant::now() < end, "adapter never started");
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn hn18_second_frame_cancels_a_blocked_adapter_and_reaps_every_child() {
    let w = world("ignore_cancel");
    let mut b = Bridge::start(&w);
    b.invoke(7, 100_000);
    let (adapter, grandchild) = wait_pids(&w);
    assert!(pid_alive(adapter) && pid_alive(grandchild));
    // Concurrent request on the same session while 7 is blocked.
    b.send(json!({"jsonrpc":"2.0","id":8,"method":"bridge/status","params":{}}));
    assert!(
        b.reply(json!(8))["result"].is_object(),
        "status served during the invocation"
    );
    // Unknown id: refused as such, and it does not poison id 7.
    b.send(json!({"jsonrpc":"2.0","id":9,"method":"bridge/cancel","params":{"id":999}}));
    assert_eq!(b.reply(json!(9))["result"]["state"], "unknown-id");
    b.send(json!({"jsonrpc":"2.0","id":10,"method":"bridge/cancel","params":{"id":7}}));
    assert_eq!(b.reply(json!(10))["result"]["state"], "cancel-requested");
    let r = b.reply(json!(7));
    assert_eq!(r["result"]["state"], "confirmed-terminated", "{r}");
    assert_eq!(r["result"]["cancelled"], true);
    assert_eq!(r["result"]["retried"], false);
    assert_gone(adapter);
    assert_gone(grandchild);
    // A delayed cancel after settlement reports the settled state, not a new cancel.
    b.send(json!({"jsonrpc":"2.0","id":11,"method":"bridge/cancel","params":{"id":7}}));
    let late = b.reply(json!(11));
    assert_eq!(late["result"]["state"], "confirmed-terminated");
    assert!(
        b.child.try_wait().unwrap().is_none(),
        "bridge survives a cancellation"
    );
    drop(b.stdin.take());
    assert!(b.child.wait().unwrap().success());
}

#[test]
fn hn18_client_crash_mid_flight_cancels_and_reaps_children() {
    let w = world("ignore_cancel");
    let mut b = Bridge::start(&w);
    b.invoke(1, 100_000);
    let (adapter, grandchild) = wait_pids(&w);
    drop(b.stdin.take()); // the client vanishes while the adapter is blocked
    assert!(b.child.wait().unwrap().success());
    assert_gone(adapter);
    assert_gone(grandchild);
}

#[test]
fn hn18_in_flight_is_bounded_and_ids_are_unique() {
    let w = world("ignore_cancel");
    let mut b = Bridge::start(&w);
    for id in 1..=4 {
        b.invoke(id, 100_000);
    }
    let (adapter, grandchild) = wait_pids(&w);
    b.invoke(5, 100_000);
    let r = b.reply(json!(5));
    assert_eq!(r["error"]["data"]["code"], "SPX-HPN012", "{r}");
    b.invoke(2, 100_000);
    // Both id 2 requests answer on id 2; the refusal is the one that arrives first.
    let r = b.reply(json!(2));
    assert_eq!(r["error"]["data"]["code"], "SPX-HPN013", "{r}");
    for id in 1..=4 {
        b.send(json!({"jsonrpc":"2.0","id":100 + id,"method":"bridge/cancel","params":{"id":id}}));
    }
    for id in 1..=4 {
        let r = b.reply(json!(id));
        assert_ne!(r["result"]["state"], "completed", "{r}");
    }
    assert_gone(adapter);
    assert_gone(grandchild);
}

#[test]
fn hn18_pipe_floods_in_both_directions_do_not_wedge_the_session() {
    let w = world("flood");
    let mut b = Bridge::start(&w);
    // Hostile adapter floods stdout: quarantined, session continues.
    b.invoke(1, 30_000);
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "refused", "{r}");
    assert_eq!(r["result"]["code"], "SPX-HPC010", "{r}");
    // Client floods: thousands of stray cancels are all answered.
    for i in 0..3000u64 {
        b.send(json!({"jsonrpc":"2.0","id":10_000 + i,"method":"bridge/cancel","params":{"id":i}}));
    }
    let last = b.reply(json!(12_999));
    assert_eq!(last["result"]["state"], "unknown-id");
    b.send(json!({"jsonrpc":"2.0","id":3,"method":"bridge/status","params":{}}));
    assert!(b.reply(json!(3))["result"].is_object());
    // An oversized frame is refused by the bounded reader (SPX-HPA002) and the
    // faulty session is closed (MA-06) instead of buffering the whole line.
    let big = format!(
        "{{\"jsonrpc\":\"2.0\",\"id\":2,\"method\":\"x\",\"params\":{{\"p\":\"{}\"}}}}",
        "A".repeat(2 << 20)
    );
    if let Some(i) = b.stdin.as_mut() {
        let _ = writeln!(i, "{big}").and_then(|()| i.flush());
    }
    let r = b.reply(Value::Null);
    assert_eq!(r["error"]["data"]["code"], "SPX-HPA002", "{r}");
    assert!(!b.child.wait().unwrap().success());
}

#[test]
fn hn18_crash_during_cancellation_is_never_a_current_result_or_a_retry() {
    let w = world("crash_on_invoke");
    let mut b = Bridge::start(&w);
    b.invoke(1, 30_000);
    b.send(json!({"jsonrpc":"2.0","id":2,"method":"bridge/cancel","params":{"id":1}}));
    let r = b.reply(json!(1));
    assert_ne!(r["result"]["state"], "completed", "{r}");
    assert_ne!(r["result"]["status"], "complete", "{r}");
    assert_eq!(r["result"]["retried"], false, "{r}");
    assert!(b.reply(json!(2))["result"]["state"].is_string());
    b.send(json!({"jsonrpc":"2.0","id":3,"method":"bridge/status","params":{}}));
    assert!(b.reply(json!(3))["result"].is_object());
}

#[test]
fn hn18_cancelled_side_effecting_step_is_uncertain_and_never_replayed() {
    let w = world("ignore_cancel");
    let mut b = Bridge::start(&w);
    b.generate(1, "gen-1");
    let (adapter, grandchild) = wait_pids(&w);
    b.send(json!({"jsonrpc":"2.0","id":2,"method":"bridge/cancel","params":{"id":1}}));
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "uncertain-external-effect", "{r}");
    assert_eq!(r["result"]["cancelled"], false);
    assert_gone(adapter);
    assert_gone(grandchild);
    // The same step is refused, with the adapter never started again.
    std::fs::remove_file(w.root.join("pids")).unwrap();
    b.generate(3, "gen-1");
    let r = b.reply(json!(3));
    assert_eq!(r["error"]["data"]["code"], "SPX-HPN015", "{r}");
    assert!(
        !w.root.join("pids").exists(),
        "no duplicate attempt was started"
    );
    // The record is durable: a fresh session still refuses it.
    drop(b.stdin.take());
    b.child.wait().unwrap();
    let mut b2 = Bridge::start(&w);
    b2.generate(1, "gen-1");
    assert_eq!(b2.reply(json!(1))["error"]["data"]["code"], "SPX-HPN015");
    assert!(!w.root.join("pids").exists());
}

#[test]
fn hn18_plain_subprocess_is_never_described_as_isolated() {
    let w = world("");
    let mut b = Bridge::start(&w);
    b.invoke(1, 30_000);
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "completed", "{r}");
    let iso = &r["result"]["isolation"];
    assert_eq!(
        (iso["mode"].as_str(), iso["isolated"].as_bool()),
        (Some("subprocess"), Some(false)),
        "{iso}"
    );
    assert!(iso.get("mechanism").is_none());
}

#[test]
fn hn18_required_isolation_without_a_sandbox_refuses_instead_of_downgrading() {
    let w = world("");
    let cfg = HostConfig {
        backend: IsolationBackend::unavailable(),
        ..HostConfig::default()
    };
    let input = [
        json!({"jsonrpc":"2.0","id":0,"method":"bridge/handshake","params":
            {"protocol": PROTOCOL, "version": 1, "host": {"name":"t","version":"1"}, "capabilities": {}, "command_rewriter": null}}),
        json!({"jsonrpc":"2.0","id":1,"method":"bridge/invoke","params":{"capability":"decision.evaluate","operation":"evaluate",
            "isolation":"required","payload":{"task":"model-route/v1","features":{},"options":["a"]}}}),
    ]
    .map(|v| v.to_string())
    .join("\n");
    let mut out = Vec::new();
    serve_with(
        std::io::Cursor::new(input.into_bytes()),
        &mut out,
        Server::new(&w.env, &w.project).with_host_config(cfg),
    )
    .unwrap();
    let frames: Vec<Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let r = frames.iter().find(|f| f["id"] == 1).unwrap();
    assert_eq!(r["error"]["data"]["code"], "SPX-HPC003", "{r}");
    assert!(
        r["error"]["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("isolation"),
        "{r}"
    );
    assert!(r.get("result").is_none());
}
