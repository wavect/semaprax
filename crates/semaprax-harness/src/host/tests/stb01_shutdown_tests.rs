//! STB-01 (#544): shutdown is bounded by its own deadline even while a
//! startup owns the start gate, and a closing handle is never revived.

use super::stb_fixture::*;
use super::*;
use std::sync::Arc;

const GRACE_MS: u64 = 200;
/// Far below the 30 s handshake/invocation allowance the fixtures use.
const WATCHDOG: Duration = Duration::from_secs(10);

fn long_allowance(v: &mut Value) {
    v["resources"]["handshake_timeout_ms"] = json!(30_000);
    v["resources"]["invoke_timeout_ms"] = json!(30_000);
    v["resources"]["max_concurrency"] = json!(2);
}

struct Paths {
    barrier: PathBuf,
    release: PathBuf,
    log: PathBuf,
}

fn paths(fx: &Fx) -> Paths {
    Paths {
        barrier: fx.root.join("init-barrier"),
        release: fx.root.join("init-release"),
        log: fx.root.join("invoke-log"),
    }
}

fn withheld_init(fx: &Fx, p: &Paths) -> LaunchSpec {
    stb_spec(
        fx,
        &[
            ("STB_INIT_BARRIER", p.barrier.to_str().unwrap()),
            ("STB_INIT_RELEASE", p.release.to_str().unwrap()),
            ("STB_INVOKE_LOG", p.log.to_str().unwrap()),
        ],
        long_allowance,
    )
}

fn spawn_invoke(
    h: &Arc<AdapterHandle>,
    id: &str,
    class: InvocationClass,
) -> std::thread::JoinHandle<Outcome> {
    let (h, id) = (h.clone(), id.to_string());
    std::thread::spawn(move || h.invoke(&decide(&id), class, &CancelToken::new()))
}

fn closed_refusal(o: &Outcome) -> bool {
    matches!(o, Outcome::Refused(d) if d.code == "SPX-HPC021")
}

fn shutdown_bounded(h: &Arc<AdapterHandle>) -> Duration {
    let h = h.clone();
    within(WATCHDOG, move || {
        let t = Instant::now();
        h.shutdown();
        t.elapsed()
    })
}

#[test]
fn withheld_initialize_cannot_extend_shutdown_or_revive_the_handle() {
    let fx = fixture();
    let p = paths(&fx);
    let m = mgr(|c| c.shutdown_grace_ms = GRACE_MS);
    let h = m.prepare(PROJECT, withheld_init(&fx, &p)).unwrap();
    let t = spawn_invoke(&h, "i1", InvocationClass::SideEffecting);
    // Post-spawn, pre-handshake: the adapter holds the initialize reply.
    let pid: i32 = await_file(&p.barrier).trim().parse().unwrap();
    let took = shutdown_bounded(&h);
    assert!(took < WATCHDOG, "shutdown took {took:?}");
    // Release the valid initialize reply only after the grace has expired.
    std::fs::write(&p.release, "go").unwrap();
    let out = within(WATCHDOG, move || t.join().unwrap());
    assert!(closed_refusal(&out), "{out:?}");
    assert_gone(pid);
    assert_eq!(h.state(), AdapterState::Closed);
    assert_eq!(h.pid(), None);
    assert_eq!(h.gate_counts(), (0, 0), "gate released exactly once");
    // A closed handle stays closed and never dispatches business work.
    let again = h.invoke(
        &decide("i2"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(closed_refusal(&again), "{again:?}");
    assert_eq!(h.invoke_frames_queued(), 0);
    assert_eq!(
        lines(&p.log),
        0,
        "no harness/invoke frame reached the adapter"
    );
    // Repeated shutdown is immediate and harmless.
    assert!(shutdown_bounded(&h) < WATCHDOG);
    assert_eq!(h.state(), AdapterState::Closed);
    // The explicit reprepare path still yields a usable handle.
    let _ = std::fs::remove_file(&p.barrier);
    let b = m.reprepare(PROJECT, withheld_init(&fx, &p)).unwrap();
    let ok = b.invoke(
        &decide("i3"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(ok, Outcome::Completed(_)), "{ok:?}");
    assert_eq!(lines(&p.log), 1);
    assert!(h.is_closed() && !b.is_closed());
}

#[test]
fn waiter_queued_behind_the_start_gate_is_released_by_shutdown() {
    let fx = fixture();
    let p = paths(&fx);
    let m = mgr(|c| c.shutdown_grace_ms = GRACE_MS);
    let h = m.prepare(PROJECT, withheld_init(&fx, &p)).unwrap();
    let owner = spawn_invoke(&h, "i1", InvocationClass::Decision);
    let pid: i32 = await_file(&p.barrier).trim().parse().unwrap();
    // A second admitted invocation waits for the start gate the first owns.
    let queued = spawn_invoke(&h, "i2", InvocationClass::SideEffecting);
    let end = Instant::now() + WATCHDOG;
    while h.gate_counts().0 < 2 {
        assert!(Instant::now() < end, "second invocation never admitted");
        std::thread::yield_now();
    }
    assert!(shutdown_bounded(&h) < WATCHDOG);
    std::fs::write(&p.release, "go").unwrap();
    for t in [owner, queued] {
        let out = within(WATCHDOG, move || t.join().unwrap());
        assert!(closed_refusal(&out), "{out:?}");
    }
    assert_gone(pid);
    assert_eq!(h.gate_counts(), (0, 0));
    assert_eq!(h.invoke_frames_queued(), 0);
    assert_eq!(lines(&p.log), 0);
    assert!(h.is_closed());
}

#[test]
fn waiter_queued_in_the_admission_gate_is_refused_by_shutdown() {
    let fx = fixture();
    let p = paths(&fx);
    let m = mgr(|c| c.shutdown_grace_ms = GRACE_MS);
    let spec = stb_spec(
        &fx,
        &[
            ("STB_INIT_BARRIER", p.barrier.to_str().unwrap()),
            ("STB_INIT_RELEASE", p.release.to_str().unwrap()),
            ("STB_INVOKE_LOG", p.log.to_str().unwrap()),
        ],
        |v| {
            long_allowance(v);
            v["resources"]["max_concurrency"] = json!(1);
        },
    );
    let h = m.prepare(PROJECT, spec).unwrap();
    let owner = spawn_invoke(&h, "i1", InvocationClass::Decision);
    await_file(&p.barrier);
    let queued = spawn_invoke(&h, "i2", InvocationClass::SideEffecting);
    let end = Instant::now() + WATCHDOG;
    while h.gate_counts().1 < 1 {
        assert!(Instant::now() < end, "second invocation never queued");
        std::thread::yield_now();
    }
    assert!(shutdown_bounded(&h) < WATCHDOG);
    for t in [owner, queued] {
        let out = within(WATCHDOG, move || t.join().unwrap());
        assert!(closed_refusal(&out), "{out:?}");
    }
    assert_eq!(h.gate_counts(), (0, 0));
    assert_eq!(h.invoke_frames_queued(), 0);
}

#[test]
fn dispatched_invocation_is_bounded_by_the_grace_and_stays_uncertain() {
    let fx = fixture();
    let p = paths(&fx);
    let m = mgr(|c| c.shutdown_grace_ms = GRACE_MS);
    let spec = stb_spec(
        &fx,
        &[
            ("STB_INIT_BARRIER", p.barrier.to_str().unwrap()),
            ("STB_INVOKE_LOG", p.log.to_str().unwrap()),
            ("STB_INVOKE_MODE", "hang"),
        ],
        long_allowance,
    );
    let h = m.prepare(PROJECT, spec).unwrap();
    let t = spawn_invoke(&h, "i1", InvocationClass::SideEffecting);
    let pid: i32 = await_file(&p.barrier).trim().parse().unwrap();
    let end = Instant::now() + WATCHDOG;
    while lines(&p.log) < 1 {
        assert!(Instant::now() < end, "invocation never reached the adapter");
        std::thread::sleep(Duration::from_millis(5));
    }
    let took = shutdown_bounded(&h);
    assert!(
        took >= Duration::from_millis(GRACE_MS),
        "grace honoured: {took:?}"
    );
    let out = within(WATCHDOG, move || t.join().unwrap());
    assert!(
        matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPC007"),
        "dispatched side effect stays non-replayable: {out:?}"
    );
    assert_gone(pid);
    assert_eq!(h.invoke_frames_queued(), 1);
    assert_eq!(lines(&p.log), 1);
    assert_eq!(h.gate_counts(), (0, 0));
    assert!(h.is_closed());
}

#[test]
fn shutdown_before_any_spawn_and_after_graceful_use() {
    let fx = fixture();
    let p = paths(&fx);
    let m = mgr(|c| c.shutdown_grace_ms = GRACE_MS);
    let spec = stb_spec(
        &fx,
        &[("STB_INVOKE_LOG", p.log.to_str().unwrap())],
        long_allowance,
    );
    // Pre-spawn: nothing was ever started.
    let a = m.prepare(PROJECT, spec.clone()).unwrap();
    assert!(shutdown_bounded(&a) < WATCHDOG);
    let out = a.invoke(
        &decide("i0"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(closed_refusal(&out), "{out:?}");
    assert_eq!((a.pid(), a.invoke_frames_queued()), (None, 0));
    // Normal graceful completion, then shutdown of the live generation.
    let b = m.prepare(PROJECT, spec).unwrap();
    let ok = b.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(ok, Outcome::Completed(_)), "{ok:?}");
    let pid = b.pid().unwrap();
    assert!(shutdown_bounded(&b) < WATCHDOG);
    assert_gone(pid);
    assert!(b.is_closed() && b.pid().is_none());
    assert_eq!(lines(&p.log), 1);
}
