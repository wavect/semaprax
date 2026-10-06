//! DV-06 (#566): a protocol violation the stdout reader records while no
//! request is pending quarantines the handle; the next invocation neither
//! restarts the adapter nor writes another `harness/invoke` frame.

use super::stb_fixture::*;
use super::*;

const WATCHDOG: Duration = Duration::from_secs(20);

struct Idle {
    start_log: PathBuf,
    invoke_log: PathBuf,
    release: PathBuf,
}

fn idle(fx: &Fx) -> Idle {
    Idle {
        start_log: fx.root.join("starts"),
        invoke_log: fx.root.join("invokes"),
        release: fx.root.join("idle-release"),
    }
}

fn spec(fx: &Fx, i: &Idle, frame: &str) -> LaunchSpec {
    stb_spec(
        fx,
        &[
            ("STB_START_LOG", i.start_log.to_str().unwrap()),
            ("STB_INVOKE_LOG", i.invoke_log.to_str().unwrap()),
            ("STB_IDLE_RELEASE", i.release.to_str().unwrap()),
            ("STB_IDLE_FRAME", frame),
        ],
        |v| {
            v["resources"]["handshake_timeout_ms"] = json!(30_000);
            v["resources"]["invoke_timeout_ms"] = json!(30_000);
        },
    )
}

fn idle_violation_quarantines(frame: &str, code: &str) {
    let fx = fixture();
    let i = idle(&fx);
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, spec(&fx, &i, frame)).unwrap();
    let first = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(first, Outcome::Completed(_)), "{first:?}");
    let pid = h.pid().expect("first generation");
    assert_eq!((lines(&i.start_log), lines(&i.invoke_log)), (1, 1));

    // Only now, with the first reply consumed and nothing pending, does the
    // adapter emit its bad frame. The reader kills that generation.
    std::fs::write(&i.release, "go").unwrap();
    assert_gone(pid);

    let h2 = h.clone();
    let second = within(WATCHDOG, move || {
        h2.invoke(
            &decide("i2"),
            InvocationClass::SafeRead,
            &CancelToken::new(),
        )
    });
    match &second {
        Outcome::Quarantined(d) => assert_eq!(d.code, code, "{d:?}"),
        other => panic!("expected quarantine, got {other:?}"),
    }
    assert!(matches!(h.state(), AdapterState::Quarantined(_)));
    assert_eq!(lines(&i.start_log), 1, "no additional adapter start");
    assert_eq!(
        lines(&i.invoke_log),
        1,
        "no additional harness/invoke frame"
    );
    assert_eq!(h.pid(), None);

    // Stays quarantined for the session.
    let third = h.invoke(
        &decide("i3"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(third, Outcome::Quarantined(_)), "{third:?}");
    assert_eq!(lines(&i.start_log), 1);
    h.shutdown();
}

#[test]
fn idle_unknown_id_response_quarantines_without_restart() {
    idle_violation_quarantines("unknown-id", "SPX-HPC010");
}

#[test]
fn idle_malformed_json_quarantines_without_restart() {
    idle_violation_quarantines("malformed", "SPX-HPC009");
}

#[test]
fn state_reports_quarantine_before_the_next_invocation() {
    let fx = fixture();
    let i = idle(&fx);
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, spec(&fx, &i, "unknown-id")).unwrap();
    let first = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(first, Outcome::Completed(_)), "{first:?}");
    let pid = h.pid().unwrap();
    std::fs::write(&i.release, "go").unwrap();
    assert_gone(pid);
    assert!(matches!(h.state(), AdapterState::Quarantined(_)));
    assert_eq!(lines(&i.start_log), 1);
    h.shutdown();
}

#[test]
fn an_idle_reaped_adapter_still_restarts() {
    let fx = fixture();
    let i = idle(&fx);
    let m = mgr(|_| {});
    // No idle release file is ever written: the adapter would block, so use
    // the plain fixture without the idle knobs.
    let s = stb_spec(
        &fx,
        &[("STB_START_LOG", i.start_log.to_str().unwrap())],
        |_| {},
    );
    let h = m.prepare(PROJECT, s).unwrap();
    let o = h.invoke(
        &decide("i1"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
    assert!(h.reap_idle(Instant::now() + Duration::from_secs(3600)));
    let o = h.invoke(
        &decide("i2"),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(matches!(o, Outcome::Completed(_)), "{o:?}");
    assert_eq!(lines(&i.start_log), 2, "reaped adapter restarts lazily");
    assert!(!matches!(h.state(), AdapterState::Quarantined(_)));
    h.shutdown();
}
