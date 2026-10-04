use super::*;
use crate::host::isolation::{bwrap_args, macos_profile};

/// Tiny fixture adapter: probes a file read and a loopback connect, reporting
/// each through the decision payload (`a` = succeeded, `b` = blocked).
const PROBE: &str = r#"
import json, os, socket, sys
def send(o):
    sys.stdout.write(json.dumps(o, separators=(",", ":")) + "\n"); sys.stdout.flush()
def probe(kind):
    try:
        if kind == "file":
            open(os.environ["SECRET_PATH"]).read()
        else:
            socket.create_connection(("127.0.0.1", int(os.environ["PROBE_PORT"])), timeout=2).close()
        return "a"
    except Exception:
        return "b"
for raw in sys.stdin:
    m = json.loads(raw)
    meth, mid = m.get("method"), m.get("id")
    if meth == "harness/initialize":
        send({"jsonrpc": "2.0", "id": mid, "result": {"protocol": "semaprax.harness-rpc.v1", "accepted": [{"kind": "decision.evaluate", "version": 1, "operations": ["evaluate"]}]}})
    elif meth == "harness/invoke":
        r = m["params"]
        c = probe(os.environ["PROBE"])
        send({"jsonrpc": "2.0", "id": mid, "result": {"schema": "semaprax.harness-result.v1", "invocation_id": r["invocation_id"], "project": r["project"], "capability": r["capability"], "status": "complete", "payload": {"choice": c, "scores": {c: 1.0}, "abstain": False}, "diagnostics": [], "provenance": {"provider_id": "org.example/probe", "adapter_version": "0.0.1", "upstream_version": None}}})
    elif meth == "harness/shutdown":
        send({"jsonrpc": "2.0", "id": mid, "result": {}}); break
"#;

fn probe_spec(fx: &Fx, probe: &str, port: u16, secret: &Path, iso: IsolationRequest) -> LaunchSpec {
    let dir = fx.root.join("probe");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("adapter.py"), PROBE).unwrap();
    let manifest = json!({
        "schema": "semaprax.harness-provider.v1",
        "provider": {"id": "org.example/probe", "version": "0.1.0"},
        "adapter": {"runtime": "python", "entry": ["adapter.py"], "version": "0.1.0"},
        "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
        "capabilities": [{"kind": "decision.evaluate", "version": 1, "required": true, "operations": ["evaluate"]}],
        "platforms": ["macos-aarch64", "linux-x86_64"],
        "permissions": {"read": ["project"], "write": [], "network": [], "process": [], "secrets": []},
        "resources": {"handshake_timeout_ms": 8000, "invoke_timeout_ms": 8000, "max_frame_bytes": 1048576, "max_concurrency": 1, "idle_shutdown_ms": 60000},
        "cancellation": "cooperative",
        "support": {"license": "Apache-2.0", "isolation": "subprocess", "tested": []}
    });
    std::fs::write(dir.join("harness-provider.json"), manifest.to_string()).unwrap();
    let d = descriptor_from(&dir, |_| {});
    spec_in(
        &dir,
        d,
        fx,
        &[
            ("PROBE", probe),
            ("PROBE_PORT", &port.to_string()),
            ("SECRET_PATH", secret.to_str().unwrap()),
        ],
        iso,
    )
}

fn run(spec: LaunchSpec, backend: IsolationBackend) -> (Outcome, IsolationMode) {
    let m = mgr(|c| c.backend = backend);
    let h = m.prepare(PROJECT, spec).unwrap();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::Decision,
        &CancelToken::new(),
    );
    (out, h.isolation_mode())
}

fn choice(o: &Outcome) -> String {
    let Outcome::Completed(e) = o else {
        panic!("{o:?}")
    };
    e.payload.as_ref().unwrap()["choice"]
        .as_str()
        .unwrap()
        .to_string()
}

fn restricted(fx: &Fx) -> IsolationRequest {
    // The probe dir holds adapter.py; the descriptor dir is added by the host.
    IsolationRequest::Restricted {
        allow_read: vec![fx.project.clone()],
        allow_write: vec![],
        network: NetworkPolicy::Deny,
    }
}

#[test]
fn unsupported_restriction_is_a_tested_refusal_never_a_downgrade() {
    let fx = fixture();
    let s = probe_spec(&fx, "file", 1, &fx.root, restricted(&fx));
    let err = mgr(|c| c.backend = IsolationBackend::unavailable())
        .prepare(PROJECT, s)
        .err()
        .unwrap();
    assert_eq!(err.code, "SPX-HPC003");
    // A configured-but-missing tool is equally refused.
    let s = probe_spec(&fx, "file", 1, &fx.root, restricted(&fx));
    assert_eq!(IsolationBackend::unavailable().mechanism(), None);
    assert_eq!(
        mgr(|c| c.backend = IsolationBackend::unavailable())
            .prepare(PROJECT, s)
            .err()
            .unwrap()
            .code,
        "SPX-HPC003"
    );
}

#[test]
fn plain_subprocess_is_never_labelled_sandboxed() {
    let fx = fixture();
    let secret = fx.root.join("secret.txt");
    std::fs::write(&secret, "TOPSECRET").unwrap();
    let (out, mode) = run(
        probe_spec(&fx, "file", 1, &secret, IsolationRequest::None),
        IsolationBackend::detect(),
    );
    assert_eq!(mode, IsolationMode::Subprocess);
    assert_eq!(choice(&out), "a");
}

#[cfg(target_os = "macos")]
#[test]
fn restricted_mode_blocks_secret_reads_and_loopback_network_on_macos() {
    let fx = fixture();
    let outside = fx.root.join("elsewhere");
    std::fs::create_dir_all(&outside).unwrap();
    let secret = outside.join("secret.txt");
    std::fs::write(&secret, "TOPSECRET").unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let accept = std::thread::spawn(move || {
        let mut n = 0;
        listener.set_nonblocking(true).unwrap();
        let end = Instant::now() + Duration::from_secs(20);
        while Instant::now() < end {
            if listener.accept().is_ok() {
                n += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        n
    });
    let be = IsolationBackend::detect();
    assert_eq!(be.mechanism(), Some("sandbox-exec"));

    // Controls: without restriction both probes succeed, so the blocks below mean something.
    assert_eq!(
        choice(
            &run(
                probe_spec(&fx, "file", port, &secret, IsolationRequest::None),
                be.clone()
            )
            .0
        ),
        "a"
    );
    assert_eq!(
        choice(
            &run(
                probe_spec(&fx, "net", port, &secret, IsolationRequest::None),
                be.clone()
            )
            .0
        ),
        "a"
    );

    let (out, mode) = run(
        probe_spec(&fx, "file", port, &secret, restricted(&fx)),
        be.clone(),
    );
    assert_eq!(
        mode,
        IsolationMode::OsEnforced {
            mechanism: "sandbox-exec"
        }
    );
    assert_eq!(
        choice(&out),
        "b",
        "secret outside allowed roots must be unreadable"
    );
    let (out, _) = run(
        probe_spec(&fx, "net", port, &secret, restricted(&fx)),
        be.clone(),
    );
    assert_eq!(choice(&out), "b", "loopback connect must be blocked");

    // The adapter still works inside its roots: a read inside the project is allowed.
    let inside = fx.project.join("src/lib.rs");
    assert_eq!(
        choice(&run(probe_spec(&fx, "file", port, &inside, restricted(&fx)), be).0),
        "a"
    );
    drop(accept); // detached; it ends on its own deadline
}

#[test]
fn hostile_secret_probe_runs_restricted_and_leaks_nothing_into_the_result() {
    let fx = fixture();
    let secret = fx.root.join("elsewhere-secret.txt");
    std::fs::write(&secret, "TOPSECRET-VALUE").unwrap();
    let be = IsolationBackend::detect();
    if be.mechanism().is_none() {
        return;
    }
    let dir = examples().join("hostile-python");
    let d = descriptor_from(&dir, |_| {});
    let iso = IsolationRequest::Restricted {
        allow_read: vec![examples().join("../sdk")],
        allow_write: vec![],
        network: NetworkPolicy::Deny,
    };
    let s = spec_in(
        &dir,
        d,
        &fx,
        &[
            ("HOSTILE_MODE", "secret_probe"),
            ("SECRET_PATH", secret.to_str().unwrap()),
        ],
        iso,
    );
    let (out, mode) = run(s, be);
    assert!(matches!(mode, IsolationMode::OsEnforced { .. }));
    assert!(!format!("{out:?}").contains("TOPSECRET-VALUE"));
}

#[test]
fn profiles_and_bwrap_arguments_are_generated_deterministically() {
    let (r, w, x) = (
        vec![PathBuf::from("/a/read")],
        vec![PathBuf::from("/a/write")],
        vec![PathBuf::from("/a/exec")],
    );
    let p = macos_profile(&r, &w, &x).unwrap();
    assert!(p.contains("(deny default)") && p.contains("(deny network*)"));
    assert!(
        p.contains("(subpath \"/a/read\")")
            && p.contains("(allow file-write* (subpath \"/a/write\"))")
    );
    assert_eq!(p, macos_profile(&r, &w, &x).unwrap());
    assert!(macos_profile(&[PathBuf::from("/bad\npath")], &[], &[]).is_err());
    let b: Vec<String> = bwrap_args(&r, &w)
        .iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    assert!(b.contains(&"--unshare-net".to_string()) && b.last().map(String::as_str) == Some("--"));
    // Linux planning without bwrap is a refusal; with a (fake) tool path it is labelled bwrap.
    let none = IsolationBackend::for_tests(false, None);
    let req = IsolationRequest::Restricted {
        allow_read: vec![],
        allow_write: vec![],
        network: NetworkPolicy::Deny,
    };
    assert_eq!(
        none.plan(&req, &[], &[], &[]).err().unwrap().code,
        "SPX-HPC003"
    );
    let some = IsolationBackend::for_tests(false, Some("/usr/bin/bwrap"));
    assert_eq!(
        some.plan(&req, &[], &[], &[]).unwrap().0,
        IsolationMode::OsEnforced { mechanism: "bwrap" }
    );
}
