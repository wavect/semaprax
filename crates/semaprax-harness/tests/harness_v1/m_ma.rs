//! MA-01 / MA-02 bridge end-to-end: launch identity and post-dispatch
//! uncertainty against a real hostile adapter (fixture prefix `hp-ma-m`).

use crate::support::{fixture_dir, harness_bin, write};
use semaprax_harness::bridge::negotiate::PROTOCOL;
use semaprax_harness::cli::{run, Environment};
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
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

/// Adopt and trust a copy of the hostile adapter fixed to `mode`; the project
/// selects it for `decision.evaluate`.
fn world(mode: &str) -> World {
    let root = fixture_dir("hp-ma-m").canonicalize().unwrap();
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
        )
        .replace(
            "os.environ.get(\"HOSTILE_COUNTER\")",
            &format!("\"{}\"", root.join("counter").display()),
        )
        .replace(
            "def invoke(mid, req):\n",
            &format!(
                "def invoke(mid, req):\n    if MODE == \"\":\n        open(\"{}\", \"a\").write(\"x\\n\")\n",
                root.join("counter").display()
            ),
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

fn count(w: &World) -> usize {
    std::fs::read_to_string(w.root.join("counter")).map_or(0, |s| s.lines().count())
}

#[test]
fn ma01_required_isolation_never_reuses_the_plain_handle() {
    let w = world("");
    let mut b = Bridge::start(&w);
    b.invoke(1, 30_000);
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "completed", "{r}");
    assert_eq!(count(&w), 1);
    b.send(json!({"jsonrpc":"2.0","id":2,"method":"bridge/invoke","params":{
        "capability":"decision.evaluate","operation":"evaluate","deadline_ms":30000,"isolation":"required",
        "payload":{"task":"model-route/v1","features":{},"options":["a","b"]}}}));
    let r = b.reply(json!(2));
    assert!(r.get("error").is_some(), "must refuse, not reuse: {r}");
    assert_eq!(r["error"]["data"]["code"], "SPX-HPC001", "{r}");
    assert_eq!(count(&w), 1, "no extra provider call");
    // The plain request still reuses its handle.
    b.invoke(3, 30_000);
    assert_eq!(b.reply(json!(3))["result"]["state"], "completed");
    assert_eq!(count(&w), 2);
}

#[test]
fn ma02_adapter_error_after_dispatch_is_uncertain_and_never_replayed() {
    let w = world("error_on_invoke");
    let mut b = Bridge::start(&w);
    b.generate(1, "billing-error-1");
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "uncertain-external-effect", "{r}");
    assert_eq!(count(&w), 1);
    b.generate(2, "billing-error-1");
    let r = b.reply(json!(2));
    assert_eq!(r["error"]["data"]["code"], "SPX-HPN015", "{r}");
    assert_eq!(count(&w), 1, "same session: no second execution");
    drop(b.stdin.take());
    b.child.wait().unwrap();
    let mut b2 = Bridge::start(&w);
    b2.generate(1, "billing-error-1");
    let r = b2.reply(json!(1));
    assert_eq!(r["error"]["data"]["code"], "SPX-HPN015", "{r}");
    assert_eq!(count(&w), 1, "after restart: no second execution");
}
