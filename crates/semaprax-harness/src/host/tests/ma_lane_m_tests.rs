//! MA-01 / MA-02 / MA-04 regressions: launch identity, post-dispatch
//! uncertainty and pre-dispatch cancellation/deadline checks.

use super::*;
use crate::host::isolation::IsolationRequest;
use std::sync::Arc;

fn hostile_with(fx: &Fx, env: &[(&str, &str)], edit: impl FnOnce(&mut Value)) -> LaunchSpec {
    let dir = examples().join("hostile-python");
    let d = descriptor_from(&dir, edit);
    spec_in(&dir, d, fx, env, IsolationRequest::None)
}

fn counter_lines(p: &Path) -> usize {
    std::fs::read_to_string(p).map_or(0, |s| s.lines().count())
}

// ---- MA-01 ----

#[test]
fn changed_launch_identity_is_refused_and_identical_identity_reuses() {
    let fx = fixture();
    let m = mgr(|_| {});
    let base = hostile_with(&fx, &[("HOSTILE_MODE", "")], |_| {});
    let h1 = m.prepare(PROJECT, base.clone()).unwrap();
    // Identical complete identity reuses the same handle.
    let h2 = m.prepare(PROJECT, base.clone()).unwrap();
    assert!(Arc::ptr_eq(&h1, &h2));
    // Warm the process, then try to re-prepare under changed identities.
    let o = h1.invoke(
        &decide("i1"),
        InvocationClass::Decision,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
    let pid = h1.pid();

    let mut iso = base.clone();
    iso.isolation = IsolationRequest::Restricted {
        allow_read: vec![fx.project.clone()],
        allow_write: vec![],
        network: crate::host::isolation::NetworkPolicy::Deny,
    };
    let mut env = base.clone();
    env.forward_env.insert("EXTRA".into(), "1".into());
    let mut rt = base.clone();
    rt.runtime_executable = Some(PathBuf::from("/usr/bin/python3"));
    let mut roots = base.clone();
    roots.project_root = fx.root.clone();
    for (what, spec) in [
        ("isolation request", iso),
        ("forwarded environment", env),
        ("runtime executable", rt),
        ("project root", roots),
    ] {
        let e = m.prepare(PROJECT, spec).err().expect(what);
        assert_eq!(e.code, "SPX-HPC001", "{what}");
        assert!(e.message.contains(what), "{what}: {}", e.message);
    }
    assert_eq!(h1.pid(), pid, "live process is untouched");
    assert_eq!(h1.isolation_mode(), IsolationMode::Subprocess);
}

#[test]
fn changed_grant_is_refused() {
    let fx = fixture();
    let m = mgr(|_| {});
    let base = hostile_with(&fx, &[("HOSTILE_MODE", "")], |_| {});
    m.prepare(PROJECT, base.clone()).unwrap();
    let mut widened = base.clone();
    widened.grant = Grant::issue(
        base.grant.provider_id().to_string(),
        base.grant.descriptor_digest().to_string(),
        base.grant.entry_digest().map(str::to_string),
        None,
        GrantedPermissions {
            read: vec!["project".into()],
            write: vec!["cache".into(), "retention".into()],
            ..Default::default()
        },
    );
    let e = m.prepare(PROJECT, widened).err().expect("refused");
    assert_eq!(e.code, "SPX-HPC001");
    assert!(e.message.contains("grant"), "{}", e.message);
}

// ---- MA-02 ----

#[test]
fn json_rpc_error_after_dispatch_is_uncertain_for_side_effects_only() {
    let fx = fixture();
    let counter = fx.root.join("counter");
    let m = mgr(|_| {});
    let h = m
        .prepare(
            PROJECT,
            hostile_with(
                &fx,
                &[
                    ("HOSTILE_MODE", "error_on_invoke"),
                    ("HOSTILE_COUNTER", counter.to_str().unwrap()),
                ],
                |_| {},
            ),
        )
        .unwrap();
    let c = CancelToken::new();
    let out = h.invoke(&decide("i1"), InvocationClass::SideEffecting, &c);
    assert!(
        matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPC024"),
        "{out:?}"
    );
    assert_eq!(counter_lines(&counter), 1);
    // Safe classes keep their refusal behaviour.
    let out = h.invoke(&decide("i2"), InvocationClass::Decision, &c);
    assert!(
        matches!(&out, Outcome::Refused(d) if d.code == "SPX-HPC024"),
        "{out:?}"
    );
}

#[test]
fn payload_validation_failure_after_dispatch_is_uncertain_for_side_effects() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(
            PROJECT,
            hostile_with(&fx, &[("HOSTILE_MODE", "forbidden_model")], |_| {}),
        )
        .unwrap();
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    assert!(
        matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPA043"),
        "{out:?}"
    );
    assert_eq!(h.invoke_frames_queued(), 1);
}

#[test]
fn pre_dispatch_rejection_stays_refused_with_zero_frames() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(PROJECT, hostile_with(&fx, &[("HOSTILE_MODE", "")], |_| {}))
        .unwrap();
    let mut bad = decide("i1");
    bad.operation = "no-such-operation".into();
    let out = h.invoke(&bad, InvocationClass::SideEffecting, &CancelToken::new());
    assert!(matches!(out, Outcome::Refused(_)), "{out:?}");
    assert_eq!(h.invoke_frames_queued(), 0);
}

// ---- MA-04 ----

#[test]
fn cancelled_invocation_on_warm_process_queues_no_frame() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(PROJECT, hostile_with(&fx, &[("HOSTILE_MODE", "")], |_| {}))
        .unwrap();
    let o = h.invoke(
        &decide("i1"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
    assert_eq!(h.invoke_frames_queued(), 1);
    let c = CancelToken::new();
    c.cancel();
    let o = h.invoke(&decide("i2"), InvocationClass::SideEffecting, &c);
    assert_eq!(o, Outcome::Cancelled);
    assert_eq!(h.invoke_frames_queued(), 1, "no invoke frame after cancel");
    // The slot and job counters were released: a fresh call still runs.
    let o = h.invoke(
        &decide("i3"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
    assert_eq!(m.budget_used().jobs, 3);
}

#[test]
fn deadline_elapsing_during_initialize_queues_no_frame() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(
            PROJECT,
            hostile_with(
                &fx,
                &[
                    ("HOSTILE_MODE", "slow_initialize"),
                    ("HOSTILE_INIT_MS", "700"),
                ],
                |_| {},
            ),
        )
        .unwrap();
    let mut req = decide("i1");
    req.deadline_ms = 250;
    let t = Instant::now();
    let out = h.invoke(&req, InvocationClass::SideEffecting, &CancelToken::new());
    assert!(
        matches!(&out, Outcome::Unavailable { request_sent: false, reason, .. } if reason.code == "SPX-HPC008"),
        "{out:?}"
    );
    assert!(
        t.elapsed() < Duration::from_millis(650),
        "{:?}",
        t.elapsed()
    );
    assert_eq!(h.invoke_frames_queued(), 0);
    assert_eq!(
        h.consecutive_crashes(),
        0,
        "a short deadline is not a crash"
    );
    assert_eq!(h.pid(), None);
}

#[test]
fn request_expiring_while_another_owns_the_start_gate_queues_no_frame() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m
        .prepare(
            PROJECT,
            hostile_with(
                &fx,
                &[
                    ("HOSTILE_MODE", "slow_initialize"),
                    ("HOSTILE_INIT_MS", "600"),
                ],
                |v| v["resources"]["max_concurrency"] = json!(2),
            ),
        )
        .unwrap();
    let h2 = h.clone();
    let owner = std::thread::spawn(move || {
        h2.invoke(
            &decide("owner"),
            InvocationClass::SideEffecting,
            &CancelToken::new(),
        )
    });
    // Let the owner take the start gate before the short-deadline request.
    let end = Instant::now() + Duration::from_secs(5);
    while h.pid().is_none() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(5));
    }
    let mut req = decide("late");
    req.deadline_ms = 150;
    let out = h.invoke(&req, InvocationClass::SideEffecting, &CancelToken::new());
    assert!(
        matches!(
            &out,
            Outcome::Unavailable {
                request_sent: false,
                ..
            }
        ),
        "{out:?}"
    );
    let owner_out = owner.join().unwrap();
    assert!(matches!(owner_out, Outcome::Completed(_)), "{owner_out:?}");
    assert_eq!(h.invoke_frames_queued(), 1, "only the owner dispatched");
}
