//! Lane T (MA-05, MA-06, MA-10): bridge transport-error cleanup, bounded frame
//! reading for both stdio servers, and the idle reaper driven from a live
//! session (fixture prefix `hp-tma`).

use crate::support::{fixture_dir, harness_bin, write};
use semaprax_harness::bridge::mcp::{self, McpOptions};
use semaprax_harness::bridge::negotiate::PROTOCOL;
use semaprax_harness::bridge::rpc::{serve_with, Server};
use semaprax_harness::cli::{run, Environment};
use semaprax_harness::profile::resolve::current_platform;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

const ID: &str = "org.example/hostile";
const CAP: usize = 1 << 20;

fn python() -> PathBuf {
    std::env::var_os("HARNESS_PYTHON")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|d| d.join("python3"))
                .find(|p| p.is_file())
        })
        .expect("python3 not found: the adapter-dependent bridge cells cannot run here")
}

fn pid_alive(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

fn wait_gone(pid: i32, within: Duration) -> Duration {
    let t = Instant::now();
    while pid_alive(pid) && t.elapsed() < within {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(!pid_alive(pid), "pid {pid} survived {within:?}");
    t.elapsed()
}

fn s(a: &[&str]) -> Vec<String> {
    a.iter().map(|x| x.to_string()).collect()
}

struct World {
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    env: Environment,
}

impl World {
    fn pids(&self) -> Option<(i32, i32)> {
        let t = std::fs::read_to_string(self.root.join("pids")).ok()?;
        let v: Vec<i32> = t
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect();
        (v.len() == 2).then(|| (v[0], v[1]))
    }
    fn wait_pids(&self) -> (i32, i32) {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(p) = self.pids() {
                return p;
            }
            assert!(Instant::now() < end, "adapter never started");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    /// Adapter process ids in start order (one line per adapter start).
    fn starts(&self) -> Vec<i32> {
        std::fs::read_to_string(self.root.join("starts"))
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|x| x.parse().ok())
            .collect()
    }
    fn wait_starts(&self, n: usize) -> Vec<i32> {
        let end = Instant::now() + Duration::from_secs(20);
        loop {
            let v = self.starts();
            if v.len() >= n {
                return v;
            }
            assert!(Instant::now() < end, "adapter start {n} never recorded");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Trusted copy of the hostile adapter in `mode` with a declared idle shutdown;
/// every adapter start appends its pid to `starts`.
fn world(mode: &str, idle_ms: u64) -> World {
    let root = fixture_dir("hp-tma").canonicalize().unwrap();
    let ex = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/semaprax-harness-adapters/examples/hostile-python");
    let src = std::fs::read_to_string(ex.join("adapter.py"))
        .unwrap()
        .replace("os.environ.get(\"HOSTILE_MODE\", \"\")", &format!("\"{mode}\""))
        .replace(
            "os.environ.get(\"HOSTILE_PIDFILE\")",
            &format!("\"{}\"", root.join("pids").display()),
        )
        .replace(
            "PROTOCOL = \"semaprax.harness-rpc.v1\"",
            &format!(
                "PROTOCOL = \"semaprax.harness-rpc.v1\"\nopen(\"{}\", \"a\").write(str(os.getpid()) + \"\\n\")",
                root.join("starts").display()
            ),
        );
    write(&root, "adapter/adapter.py", &src);
    let mut d: Value =
        serde_json::from_slice(&std::fs::read(ex.join("harness-provider.json")).unwrap()).unwrap();
    d["platforms"]
        .as_array_mut()
        .unwrap()
        .push(json!(current_platform()));
    d["resources"]["invoke_timeout_ms"] = json!(120_000);
    d["resources"]["handshake_timeout_ms"] = json!(10_000);
    d["resources"]["idle_shutdown_ms"] = json!(idle_ms);
    write(&root, "adapter/harness-provider.json", &d.to_string());
    let home = root.join("home");
    let project = root.join("project");
    std::fs::create_dir_all(&home).unwrap();
    write(&project, "src/lib.rs", "pub fn a() {}\n");
    write(
        &project,
        "semaprax.harness.toml",
        &format!("schema = \"semaprax.harness-config.v1\"\n[capability.\"decision.evaluate\"]\nmode = \"auto\"\nprovider = \"{ID}\"\n"),
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

fn handshake() -> String {
    json!({"jsonrpc":"2.0","id":0,"method":"bridge/handshake","params":
        {"protocol": PROTOCOL, "version": 1, "host": {"name":"t","version":"1"}, "capabilities": {}, "command_rewriter": null}})
    .to_string()
}

fn invoke(id: u64) -> String {
    json!({"jsonrpc":"2.0","id":id,"method":"bridge/invoke","params":{
        "capability":"decision.evaluate","operation":"evaluate","deadline_ms":100_000,
        "payload":{"task":"model-route/v1","features":{},"options":["a","b"]}}})
    .to_string()
}

fn status(id: u64) -> String {
    json!({"jsonrpc":"2.0","id":id,"method":"bridge/status","params":{}}).to_string()
}

// ---------------------------------------------------------------- real process

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
            .stderr(Stdio::null())
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
        b.send_raw(&handshake());
        assert_eq!(b.reply(json!(0))["result"]["lifecycle"]["invoke"], true);
        b
    }
    fn send_raw(&mut self, line: &str) {
        let i = self.stdin.as_mut().unwrap();
        writeln!(i, "{line}").unwrap();
        i.flush().unwrap();
    }
    fn reply(&mut self, id: Value) -> Value {
        if let Some(p) = self.seen.iter().position(|v| v["id"] == id) {
            return self.seen.remove(p);
        }
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let v = self
                .rx
                .recv_timeout(end.saturating_duration_since(Instant::now()))
                .expect("timed out waiting for a frame");
            if v["id"] == id {
                return v;
            }
            self.seen.push(v);
        }
    }
    fn wait_exit(&mut self, within: Duration) -> std::process::ExitStatus {
        let end = Instant::now() + within;
        loop {
            if let Some(st) = self.child.try_wait().unwrap() {
                return st;
            }
            assert!(
                Instant::now() < end,
                "bridge did not exit within {within:?}"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[test]
fn ma05_invalid_utf8_with_a_live_invocation_cancels_and_reaps_promptly() {
    let w = world("ignore_cancel", 60_000);
    let mut b = Bridge::start(&w);
    b.send_raw(&invoke(1));
    let (adapter, grandchild) = w.wait_pids();
    assert!(pid_alive(adapter) && pid_alive(grandchild));
    let t = Instant::now();
    {
        let i = b.stdin.as_mut().unwrap();
        i.write_all(&[0xff, 0xfe, b'\n']).unwrap();
        i.flush().unwrap();
    }
    let st = b.wait_exit(Duration::from_secs(20));
    assert!(!st.success(), "a transport error is not a success: {st:?}");
    wait_gone(adapter, Duration::from_secs(8));
    wait_gone(grandchild, Duration::from_secs(8));
    // Far below the 100 s invocation deadline.
    assert!(t.elapsed() < Duration::from_secs(20), "{:?}", t.elapsed());
}

#[test]
fn ma06_oversized_unterminated_frame_closes_the_session_and_cleans_up() {
    let w = world("ignore_cancel", 60_000);
    let mut b = Bridge::start(&w);
    b.send_raw(&invoke(1));
    let (adapter, grandchild) = w.wait_pids();
    // 4 MiB with no newline: refused once the cap is crossed.
    let mut stdin = b.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        let chunk = vec![b'A'; 64 * 1024];
        for _ in 0..64 {
            if stdin.write_all(&chunk).is_err() {
                break;
            }
        }
    });
    let st = b.wait_exit(Duration::from_secs(20));
    assert!(!st.success());
    let _ = writer.join();
    // The refusal frame names the stable size diagnostic.
    let mut codes = vec![];
    while let Ok(v) = b.rx.recv_timeout(Duration::from_millis(500)) {
        codes.push(v["error"]["data"]["code"].clone());
    }
    assert!(codes.contains(&json!("SPX-HPA002")), "{codes:?}");
    wait_gone(adapter, Duration::from_secs(8));
    wait_gone(grandchild, Duration::from_secs(8));
}

#[test]
fn ma10_quiet_open_session_reaps_the_idle_adapter_then_restarts_lazily() {
    let w = world("", 300);
    let mut b = Bridge::start(&w);
    b.send_raw(&invoke(1));
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "completed", "{r}");
    let done = Instant::now();
    let first = w.wait_starts(1)[0];
    assert!(pid_alive(first));
    // stdin stays open and silent; the session's own tick reaps the adapter.
    let reaped = wait_gone(first, Duration::from_secs(10));
    let latency = done.elapsed();
    assert!(b.child.try_wait().unwrap().is_none(), "bridge stays up");
    assert!(
        latency >= Duration::from_millis(250),
        "reaped before the idle interval: {latency:?}"
    );
    // Next request restarts and renegotiates through the ordinary host path.
    let t = Instant::now();
    b.send_raw(&invoke(2));
    let r = b.reply(json!(2));
    assert_eq!(r["result"]["state"], "completed", "{r}");
    let restart = t.elapsed();
    let starts = w.wait_starts(2);
    assert_eq!(starts.len(), 2, "exactly one restart: {starts:?}");
    assert_ne!(starts[0], starts[1]);
    eprintln!(
        "MA-10 fixture: idle_shutdown_ms=300 tick=250ms; retained adapter after last use {latency:?} (poll {reaped:?}); \
         retained processes while quiet: 1 -> 0; lazy restart request latency {restart:?}"
    );
    // EOF stops the tick and the session; the second adapter goes too.
    drop(b.stdin.take());
    assert!(b.wait_exit(Duration::from_secs(20)).success());
    wait_gone(starts[1], Duration::from_secs(8));
}

#[test]
fn ma10_live_invocation_across_the_idle_interval_is_not_reaped() {
    let w = world("ignore_cancel", 200);
    let mut b = Bridge::start(&w);
    b.send_raw(&invoke(1));
    let (adapter, grandchild) = w.wait_pids();
    std::thread::sleep(Duration::from_millis(1500)); // several idle intervals and ticks
    assert!(
        pid_alive(adapter) && pid_alive(grandchild),
        "live work was reaped as idle"
    );
    b.send_raw(
        &json!({"jsonrpc":"2.0","id":2,"method":"bridge/cancel","params":{"id":1}}).to_string(),
    );
    assert_eq!(b.reply(json!(2))["result"]["state"], "cancel-requested");
    let r = b.reply(json!(1));
    assert_eq!(r["result"]["state"], "confirmed-terminated", "{r}");
    wait_gone(adapter, Duration::from_secs(8));
    wait_gone(grandchild, Duration::from_secs(8));
    drop(b.stdin.take());
    assert!(b.wait_exit(Duration::from_secs(20)).success());
}

// ------------------------------------------------------------ in-process (MA-05)

type Wait = Box<dyn Fn() -> bool + Send>;

/// Delivers stages in order: each waits for its condition, then yields its
/// bytes (or its injected error).
struct Scripted {
    stages: std::collections::VecDeque<(Wait, io::Result<Vec<u8>>)>,
    cur: io::Cursor<Vec<u8>>,
}

impl Scripted {
    fn new(stages: Vec<(Wait, io::Result<Vec<u8>>)>) -> BufReader<Self> {
        BufReader::new(Self {
            stages: stages.into(),
            cur: io::Cursor::new(vec![]),
        })
    }
}

fn lines(f: &[String]) -> io::Result<Vec<u8>> {
    Ok(format!("{}\n", f.join("\n")).into_bytes())
}

fn now() -> Wait {
    Box::new(|| true)
}

impl Read for Scripted {
    fn read(&mut self, o: &mut [u8]) -> io::Result<usize> {
        loop {
            let n = self.cur.read(o)?;
            if n > 0 {
                return Ok(n);
            }
            let Some((until, bytes)) = self.stages.pop_front() else {
                return Ok(0);
            };
            let end = Instant::now() + Duration::from_secs(30);
            while !until() && Instant::now() < end {
                std::thread::sleep(Duration::from_millis(10));
            }
            self.cur = io::Cursor::new(bytes?);
        }
    }
}

struct FailOn<F: Fn(&[u8]) -> bool> {
    when: F,
    sink: Vec<u8>,
}
impl<F: Fn(&[u8]) -> bool> Write for FailOn<F> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        if (self.when)(b) {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected client output failure",
            ));
        }
        self.sink.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn pids_present(root: PathBuf) -> Wait {
    Box::new(move || {
        std::fs::read_to_string(root.join("pids"))
            .map(|t| t.split_whitespace().count() == 2)
            .unwrap_or(false)
    })
}

fn pids_of(w: &World) -> (i32, i32) {
    w.pids().expect("pids recorded")
}

fn adapter_dead(root: PathBuf) -> Wait {
    Box::new(move || {
        let t = std::fs::read_to_string(root.join("pids")).unwrap_or_default();
        t.split_whitespace()
            .next()
            .and_then(|x| x.parse::<i32>().ok())
            .is_some_and(|p| !pid_alive(p))
    })
}

#[test]
fn ma05_injected_read_error_cancels_live_work_and_preserves_the_error() {
    let w = world("ignore_cancel", 60_000);
    let reader = Scripted::new(vec![
        (now(), lines(&[handshake(), invoke(1)])),
        (
            pids_present(w.root.clone()),
            Err(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "injected read failure",
            )),
        ),
    ]);
    let t = Instant::now();
    let e = serve_with(reader, io::sink(), Server::new(&w.env, &w.project)).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::ConnectionReset);
    assert!(e.to_string().contains("injected read failure"), "{e}");
    let (a, g) = pids_of(&w);
    // serve_with only returns after scoped workers joined: children are settled.
    wait_gone(a, Duration::from_secs(8));
    wait_gone(g, Duration::from_secs(8));
    assert!(t.elapsed() < Duration::from_secs(25), "{:?}", t.elapsed());
}

#[test]
fn ma05_failing_inline_response_write_cancels_live_work_and_preserves_the_error() {
    let w = world("ignore_cancel", 60_000);
    // The handshake answer succeeds; the status answer cannot be delivered.
    let out = FailOn {
        when: |b: &[u8]| String::from_utf8_lossy(b).contains("\"id\":2,"),
        sink: vec![],
    };
    let reader = Scripted::new(vec![
        (now(), lines(&[handshake(), invoke(1)])),
        (pids_present(w.root.clone()), lines(&[status(2)])),
    ]);
    let e = serve_with(reader, out, Server::new(&w.env, &w.project)).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::BrokenPipe);
    assert!(
        e.to_string().contains("injected client output failure"),
        "{e}"
    );
    let (a, g) = pids_of(&w);
    wait_gone(a, Duration::from_secs(8));
    wait_gone(g, Duration::from_secs(8));
}

#[test]
fn ma05_failing_worker_response_write_ends_the_session() {
    let w = world("ignore_cancel", 60_000);
    let cancel =
        json!({"jsonrpc":"2.0","id":10,"method":"bridge/cancel","params":{"id":1}}).to_string();
    // The cancel settles invocation 1; its worker response (id 1) cannot be
    // written. The next frame must find the session over, not serve it.
    let out = FailOn {
        when: |b: &[u8]| String::from_utf8_lossy(b).contains("\"id\":1,"),
        sink: vec![],
    };
    let reader = Scripted::new(vec![
        (now(), lines(&[handshake(), invoke(1)])),
        (pids_present(w.root.clone()), lines(&[cancel])),
        (adapter_dead(w.root.clone()), Ok(vec![])),
        (
            Box::new(|| {
                std::thread::sleep(Duration::from_millis(500));
                true
            }),
            lines(&[status(20)]),
        ),
    ]);
    let e = serve_with(reader, out, Server::new(&w.env, &w.project)).unwrap_err();
    assert_eq!(e.kind(), io::ErrorKind::BrokenPipe, "{e}");
    let (a, g) = pids_of(&w);
    wait_gone(a, Duration::from_secs(8));
    wait_gone(g, Duration::from_secs(8));
}

// ------------------------------------------------------------ in-process (MA-06)

/// Endless non-newline bytes, counting what the server pulled.
struct Endless(std::sync::Arc<std::sync::atomic::AtomicUsize>);
impl Read for Endless {
    fn read(&mut self, o: &mut [u8]) -> io::Result<usize> {
        let n = o.len().min(4096);
        o[..n].fill(b'A');
        self.0.fetch_add(n, std::sync::atomic::Ordering::SeqCst);
        Ok(n)
    }
}

fn plain_env() -> Environment {
    let cwd = fixture_dir("hp-tma");
    Environment {
        harness_home: None,
        compiler: None,
        cwd,
        vars: BTreeMap::new(),
    }
}

/// A one-line JSON frame of exactly `len` bytes.
fn padded(method: &str, len: usize) -> String {
    let head =
        format!("{{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"{method}\",\"params\":{{\"p\":\"");
    let tail = "\"}}";
    let pad = len - head.len() - tail.len();
    format!("{head}{}{tail}", "A".repeat(pad))
}

fn rpc_frames(input: Vec<u8>) -> (io::Result<()>, Vec<Value>) {
    let env = plain_env();
    let mut out = Vec::new();
    let r = serve_with(
        BufReader::with_capacity(8192, io::Cursor::new(input)),
        &mut out,
        Server::new(&env, &env.cwd.clone()),
    );
    (
        r,
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
    )
}

fn mcp_frames(input: Vec<u8>) -> (io::Result<()>, Vec<Value>) {
    let env = plain_env();
    let mut out = Vec::new();
    let r = mcp::serve(
        BufReader::with_capacity(8192, io::Cursor::new(input)),
        &mut out,
        &env,
        &env.cwd.clone(),
        McpOptions {
            session: None,
            host_skills_dir: None,
            log: None,
        },
    );
    (
        r,
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect(),
    )
}

fn code(f: &Value) -> Option<&str> {
    f["error"]["data"]["code"].as_str()
}

#[test]
fn ma06_rpc_boundary_frames_coalescing_eof_and_utf8() {
    // Exactly at the limit: parsed (and refused as pre-handshake, not as oversize).
    let (r, f) = rpc_frames(format!("{}\n", padded("bridge/status", CAP)).into_bytes());
    r.unwrap();
    assert_eq!(code(&f[0]), Some("SPX-HPN004"), "{f:?}");
    // One byte over: the size diagnostic, then the session closes.
    let (r, f) =
        rpc_frames(format!("{}\n{}\n", padded("bridge/status", CAP + 1), status(9)).into_bytes());
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert_eq!(code(&f[0]), Some("SPX-HPA002"), "{f:?}");
    assert_eq!(f.len(), 1, "no frame after the oversize one is served");
    // Unterminated tail at EOF is a frame; multiple frames in one chunk.
    let input = format!("{}\n\n{}\n{}", status(1), status(2), status(3));
    let (r, f) = rpc_frames(input.into_bytes());
    r.unwrap();
    assert_eq!(
        f.iter().map(|v| v["id"].clone()).collect::<Vec<_>>(),
        vec![json!(1), json!(2), json!(3)]
    );
    // Clean EOF with nothing sent.
    let (r, f) = rpc_frames(vec![]);
    r.unwrap();
    assert!(f.is_empty());
    // Invalid UTF-8 ends the session with the I/O error.
    let (r, _) = rpc_frames(vec![0xff, 0xfe, b'\n']);
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn ma06_mcp_boundary_frames_coalescing_eof_and_utf8() {
    let (r, f) = mcp_frames(format!("{}\n", padded("ping", CAP)).into_bytes());
    r.unwrap();
    assert_eq!(f[0]["result"], json!({}), "{f:?}");
    let (r, f) =
        mcp_frames(format!("{}\n{}\n", padded("ping", CAP + 1), padded("ping", 80)).into_bytes());
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert_eq!(code(&f[0]), Some("SPX-HPA002"), "{f:?}");
    assert_eq!(f.len(), 1);
    let ping = |i: u64| json!({"jsonrpc":"2.0","id":i,"method":"ping"}).to_string();
    let (r, f) = mcp_frames(format!("{}\n{}\n\n{}", ping(1), ping(2), ping(3)).into_bytes());
    r.unwrap();
    assert_eq!(f.len(), 3);
    let (r, f) = mcp_frames(vec![]);
    r.unwrap();
    assert!(f.is_empty());
    let (r, _) = mcp_frames(vec![b'{', 0xc3, 0x28, b'\n']);
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
}

#[test]
fn ma06_unterminated_flood_is_cut_off_near_the_cap_in_both_servers() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let env = plain_env();
    let budget = CAP + 1 + 2 * 8192;
    let n = Arc::new(AtomicUsize::new(0));
    let r = serve_with(
        BufReader::with_capacity(8192, Endless(n.clone())),
        io::sink(),
        Server::new(&env, &env.cwd.clone()),
    );
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(
        n.load(Ordering::SeqCst) <= budget,
        "rpc pulled {}",
        n.load(Ordering::SeqCst)
    );
    let n = Arc::new(AtomicUsize::new(0));
    let r = mcp::serve(
        BufReader::with_capacity(8192, Endless(n.clone())),
        io::sink(),
        &env,
        &env.cwd.clone(),
        McpOptions {
            session: None,
            host_skills_dir: None,
            log: None,
        },
    );
    assert_eq!(r.unwrap_err().kind(), io::ErrorKind::InvalidData);
    assert!(
        n.load(Ordering::SeqCst) <= budget,
        "mcp pulled {}",
        n.load(Ordering::SeqCst)
    );
}
