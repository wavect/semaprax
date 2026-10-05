//! MF-01 (explicit close, evict and re-prepare) and MF-03 (crash accounting
//! once per process generation).

use super::*;
use crate::host::isolation::NetworkPolicy;
use std::sync::Arc;

fn ok(m: &AdapterManager, h: &Arc<AdapterHandle>, id: &str) -> bool {
    let _ = m;
    matches!(
        h.invoke(&decide(id), InvocationClass::SafeRead, &CancelToken::new()),
        Outcome::Completed(_)
    )
}

fn env_changed(fx: &Fx) -> LaunchSpec {
    let mut s = hostile(fx, "");
    s.forward_env.insert("MF01_EXTRA".into(), "1".into());
    s
}

#[test]
fn shutdown_then_prepare_same_identity_yields_a_fresh_usable_handle() {
    let fx = fixture();
    let m = mgr(|_| {});
    let a = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &a, "i1"));
    a.shutdown();
    assert!(a.is_closed());
    let b = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    assert!(ok(&m, &b, "i2"));
    // The old Arc stays permanently closed.
    assert!(a.is_closed());
    let out = a.invoke(
        &decide("i3"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Outcome::Refused(d) if d.code == "SPX-HPC021"),
        "{out:?}"
    );
    assert_eq!(a.pid(), None);
    assert!(Arc::ptr_eq(
        &m.handle(PROJECT, "org.example/hostile").unwrap(),
        &b
    ));
}

#[test]
fn prepared_but_never_started_handle_is_replaced_after_close_with_changed_identity() {
    let fx = fixture();
    let m = mgr(|_| {});
    let a = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert_eq!(a.pid(), None);
    // Changed identity while bound is refused and names the real recovery API.
    let e = m.prepare(PROJECT, env_changed(&fx)).err().unwrap();
    assert_eq!(e.code, "SPX-HPC001");
    assert!(e.message.contains("forwarded environment"), "{e}");
    assert!(e.message.contains("AdapterManager::reprepare"), "{e}");
    a.shutdown();
    let b = m.prepare(PROJECT, env_changed(&fx)).unwrap();
    assert!(!Arc::ptr_eq(&a, &b) && a.is_closed() && !b.is_closed());
    assert!(ok(&m, &b, "i1"));
}

#[test]
fn changed_identity_while_active_is_rejected_including_isolation_and_env() {
    let fx = fixture();
    let m = mgr(|_| {});
    let a = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &a, "i1"));
    let mut iso = hostile(&fx, "");
    iso.isolation = IsolationRequest::Restricted {
        allow_read: vec![],
        allow_write: vec![],
        network: NetworkPolicy::Deny,
    };
    for (spec, what) in [
        (iso, "isolation request"),
        (env_changed(&fx), "forwarded environment"),
    ] {
        let e = m.prepare(PROJECT, spec).err().unwrap();
        assert_eq!(e.code, "SPX-HPC001");
        assert!(e.message.contains(what), "{e}");
    }
    assert!(
        !a.is_closed() && ok(&m, &a, "i2"),
        "rejection must not disturb it"
    );
}

#[test]
fn reprepare_validates_first_then_replaces_an_active_handle() {
    let fx = fixture();
    let m = mgr(|_| {});
    let a = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &a, "i1"));
    let pid = a.pid().unwrap();
    // An invalid replacement leaves the active handle untouched.
    let mut bad = env_changed(&fx);
    bad.grant = Grant::issue(
        bad.descriptor.provider_id.clone(),
        "sha256:bad".into(),
        None,
        None,
        GrantedPermissions::default(),
    );
    assert!(m.reprepare(PROJECT, bad).is_err());
    assert!(!a.is_closed() && a.pid() == Some(pid));
    let b = m.reprepare(PROJECT, env_changed(&fx)).unwrap();
    assert!(a.is_closed() && !b.is_closed());
    assert_gone(pid);
    assert!(ok(&m, &b, "i2"));
    // close_and_evict drops the entry; prepare builds a new one afterwards.
    assert!(m.close_and_evict(PROJECT, "org.example/hostile"));
    assert!(b.is_closed());
    assert!(m.handle(PROJECT, "org.example/hostile").is_none());
    assert!(!m.close_and_evict(PROJECT, "org.example/hostile"));
    let c = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &c, "i3"));
}

#[test]
fn quarantine_is_not_cleared_by_prepare_only_by_explicit_recovery() {
    let fx = fixture();
    let m = mgr(|_| {});
    let a = m.prepare(PROJECT, hostile(&fx, "flood")).unwrap();
    let out = a.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(out, Outcome::Quarantined(_)));
    a.shutdown();
    assert!(!a.is_closed(), "quarantine survives shutdown");
    let same = m.prepare(PROJECT, hostile(&fx, "flood")).unwrap();
    assert!(Arc::ptr_eq(&a, &same));
    let b = m.reprepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &b, "i2"));
}

#[test]
fn concurrent_reprepare_never_leaves_two_active_owners() {
    let fx = fixture();
    let m = Arc::new(mgr(|_| {}));
    let a = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    assert!(ok(&m, &a, "i0"));
    let specs: Vec<_> = (0..4).map(|_| hostile(&fx, "")).collect();
    let ts: Vec<_> = specs
        .into_iter()
        .map(|s| {
            let m = m.clone();
            std::thread::spawn(move || m.reprepare(PROJECT, s).unwrap())
        })
        .collect();
    let hs: Vec<_> = ts.into_iter().map(|t| t.join().unwrap()).collect();
    let cur = m.handle(PROJECT, "org.example/hostile").unwrap();
    assert!(a.is_closed());
    for h in &hs {
        assert!(h.is_closed() || Arc::ptr_eq(h, &cur));
    }
    assert!(!cur.is_closed());
    assert!(ok(&m, &cur, "i1"));
    for h in &hs {
        if !Arc::ptr_eq(h, &cur) {
            assert_eq!(h.pid(), None, "a displaced handle owns no process");
        }
    }
    assert!(a
        .invoke(
            &decide("i2"),
            InvocationClass::SafeRead,
            &CancelToken::new()
        )
        .is_refused_closed());
}

trait RefusedClosed {
    fn is_refused_closed(&self) -> bool;
}
impl RefusedClosed for Outcome {
    fn is_refused_closed(&self) -> bool {
        matches!(self, Outcome::Refused(d) if d.code == "SPX-HPC021")
    }
}

// ---- MF-03 ----

const CRASH_ADAPTER: &str = r#"#!/usr/bin/env python3
import json, os, sys
OUT = sys.stdout.buffer
n = 0
for raw in sys.stdin.buffer:
    if not raw.strip():
        continue
    msg = json.loads(raw)
    m, mid = msg.get("method"), msg.get("id")
    if m == "harness/initialize":
        ops = {"context.repository": ["orient", "search", "skeleton", "references"],
               "command.view": ["view"], "decision.evaluate": ["evaluate"]}
        acc = [{"kind": c["kind"], "version": c["version"], "operations": ops.get(c["kind"], [])}
               for c in msg["params"].get("offered", [])]
        OUT.write(json.dumps({"jsonrpc": "2.0", "id": mid, "result": {
            "protocol": "semaprax.harness-rpc.v1", "accepted": acc}}).encode() + b"\n")
        OUT.flush()
    elif m == "harness/invoke":
        n += 1
        if n == 3:
            with open(os.environ["MF03_BARRIER"], "a") as fh:
                fh.write("exit\n")
            os._exit(3)
"#;

fn crash_spec(fx: &Fx) -> LaunchSpec {
    let dir = fx.root.join("mf03");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(
        examples().join("hostile-python/harness-provider.json"),
        dir.join("harness-provider.json"),
    )
    .unwrap();
    std::fs::write(dir.join("adapter.py"), CRASH_ADAPTER).unwrap();
    let d = descriptor_from(&dir, |v| {
        v["resources"]["max_concurrency"] = json!(3);
        v["resources"]["invoke_timeout_ms"] = json!(10_000);
    });
    let barrier = fx.root.join("barrier");
    spec_in(
        &dir,
        d,
        fx,
        &[("MF03_BARRIER", barrier.to_str().unwrap())],
        IsolationRequest::None,
    )
}

/// One process generation: three concurrent side-effecting invocations that
/// all go unanswered when the adapter exits.
fn crash_generation(h: &Arc<AdapterHandle>, tag: &str) {
    let ts: Vec<_> = (0..3)
        .map(|i| {
            let h = h.clone();
            let id = format!("{tag}-{i}");
            std::thread::spawn(move || {
                h.invoke(
                    &decide(&id),
                    InvocationClass::SideEffecting,
                    &CancelToken::new(),
                )
            })
        })
        .collect();
    for t in ts {
        let out = t.join().unwrap();
        assert!(
            matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPC007"),
            "side effects stay non-replayable: {out:?}"
        );
    }
}

#[test]
fn three_pending_waiters_on_one_exit_charge_one_crash_per_generation() {
    let fx = fixture();
    let m = mgr(|c| c.crash_threshold = 3);
    let h = m.prepare(PROJECT, crash_spec(&fx)).unwrap();
    crash_generation(&h, "g1");
    assert_eq!(h.invoke_frames_queued(), 3);
    let barrier = std::fs::read_to_string(fx.root.join("barrier")).unwrap();
    assert_eq!(barrier.lines().count(), 1);
    assert_eq!(h.consecutive_crashes(), 1, "one exit is one failure");
    assert!(
        matches!(h.state(), AdapterState::Unavailable(_)),
        "{:?}",
        h.state()
    );
    // Distinct failing generations still reach the threshold of three.
    crash_generation(&h, "g2");
    assert_eq!(h.consecutive_crashes(), 2);
    assert!(matches!(h.state(), AdapterState::Unavailable(_)));
    crash_generation(&h, "g3");
    assert_eq!(h.consecutive_crashes(), 3);
    assert!(matches!(h.state(), AdapterState::Quarantined(d) if d.code == "SPX-HPC016"));
}
