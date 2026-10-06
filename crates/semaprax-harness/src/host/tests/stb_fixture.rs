//! Shared real-child fixture for the STB stability regressions: a small
//! python adapter whose barriers and releases are files the test controls.

use super::*;

/// Environment knobs (all optional):
/// `STB_INIT_BARRIER` – write own pid here when `harness/initialize` arrives.
/// `STB_INIT_RELEASE` – withhold the initialize reply until this file exists.
/// `STB_INVOKE_LOG`   – append one line per `harness/invoke` frame received.
/// `STB_INVOKE_MODE`  – `hang` never answers an invocation.
/// `STB_CLOSE_STDIN`  – after the initialize reply close fd 0 (stdout stays
///                      open), write own pid here, then wait for `STB_EXIT`.
/// `STB_ENVELOPE`     – malformed outer envelope around a valid inner reply:
///                      `init-version`, `invoke-no-version` or `invoke-bad-error`.
pub(crate) const STB_ADAPTER: &str = r#"#!/usr/bin/env python3
import json, os, sys, time
OUT = sys.stdout.buffer
ENV = os.environ.get


def send(obj):
    env = ENV("STB_ENVELOPE", "")
    if env == "init-version" and "protocol" in (obj.get("result") or {}):
        obj["jsonrpc"] = "1.0"
    if env == "invoke-no-version" and "schema" in (obj.get("result") or {}):
        del obj["jsonrpc"]
    if env == "invoke-bad-error" and "schema" in (obj.get("result") or {}):
        obj = {"jsonrpc": "2.0", "id": obj["id"], "error": {"message": "no code"}}
    OUT.write(json.dumps(obj, separators=(",", ":"), sort_keys=True).encode() + b"\n")
    OUT.flush()


def wait_file(path):
    end = time.time() + 120
    while not os.path.exists(path) and time.time() < end:
        time.sleep(0.01)


def touch(path, text):
    with open(path + ".tmp", "w") as fh:
        fh.write(text)
    os.rename(path + ".tmp", path)


def init_reply(mid, msg):
    ops = {"context.repository": ["orient", "search", "skeleton", "references"],
           "command.view": ["view"], "decision.evaluate": ["evaluate"]}
    acc = [{"kind": c["kind"], "version": c["version"], "operations": ops.get(c["kind"], [])}
           for c in msg["params"].get("offered", [])]
    return {"jsonrpc": "2.0", "id": mid, "result": {
        "protocol": "semaprax.harness-rpc.v1", "accepted": acc}}


def invoke_reply(mid, req):
    opts = (req.get("payload") or {}).get("options") or ["a"]
    return {"jsonrpc": "2.0", "id": mid, "result": {
        "schema": "semaprax.harness-result.v1", "invocation_id": req["invocation_id"],
        "project": req["project"], "capability": req["capability"], "status": "complete",
        "payload": {"choice": opts[0], "scores": {opts[0]: 1.0}, "abstain": False},
        "diagnostics": [],
        "provenance": {"provider_id": "org.example/hostile", "adapter_version": "0.0.1",
                       "upstream_version": "none"}}}


def main():
    for raw in sys.stdin.buffer:
        if not raw.strip():
            continue
        msg = json.loads(raw)
        method, mid = msg.get("method"), msg.get("id")
        if method == "harness/initialize":
            if ENV("STB_INIT_BARRIER"):
                touch(ENV("STB_INIT_BARRIER"), str(os.getpid()))
            if ENV("STB_INIT_RELEASE"):
                wait_file(ENV("STB_INIT_RELEASE"))
            send(init_reply(mid, msg))
            if ENV("STB_CLOSE_STDIN"):
                os.close(0)
                touch(ENV("STB_CLOSE_STDIN"), str(os.getpid()))
                wait_file(ENV("STB_EXIT"))
                return
        elif method == "harness/invoke":
            if ENV("STB_INVOKE_LOG"):
                with open(ENV("STB_INVOKE_LOG"), "a") as fh:
                    fh.write(msg["params"]["invocation_id"] + "\n")
            if ENV("STB_INVOKE_MODE") == "hang":
                while True:
                    time.sleep(1)
            send(invoke_reply(mid, msg["params"]))
        elif method == "harness/shutdown":
            send({"jsonrpc": "2.0", "id": mid, "result": {}})
            return


if __name__ == "__main__":
    main()
"#;

/// Spec for the STB adapter in a fresh fixture directory, with the hostile
/// descriptor's identity and the given resource overrides and environment.
pub(crate) fn stb_spec(fx: &Fx, env: &[(&str, &str)], edit: impl FnOnce(&mut Value)) -> LaunchSpec {
    let dir = fx.root.join("stb");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        examples().join("hostile-python/harness-provider.json"),
        dir.join("harness-provider.json"),
    )
    .unwrap();
    std::fs::write(dir.join("adapter.py"), STB_ADAPTER).unwrap();
    let d = descriptor_from(&dir, edit);
    spec_in(&dir, d, fx, env, IsolationRequest::None)
}

/// Poll (bounded) until `path` exists, then return its contents.
pub(crate) fn await_file(path: &Path) -> String {
    let end = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(s) = std::fs::read_to_string(path) {
            return s;
        }
        assert!(Instant::now() < end, "{} never appeared", path.display());
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn lines(path: &Path) -> usize {
    std::fs::read_to_string(path).map_or(0, |s| s.lines().count())
}

/// Run `f` on a thread and require it to finish within `limit` (an outer
/// watchdog independent of the code under test).
pub(crate) fn within<T: Send + 'static>(
    limit: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f());
    });
    rx.recv_timeout(limit)
        .unwrap_or_else(|_| panic!("watchdog: not finished within {limit:?}"))
}
