//! STB-04 (#547): an I/O worker that cannot be created rolls the started
//! child back through the single owner and returns an ordinary error.

use super::stb02_owner_tests::{sh_prepared, RecSys};
use super::*;
use crate::host::process::{Closed, Proc, Settlement, Sys, WORKERS};
use std::process::Child;
use std::sync::atomic::AtomicI32;
use std::sync::Arc;
use std::thread::JoinHandle;

/// Recording seam that refuses to create worker `fail` (if any) with a
/// synthetic OS-style error and logs each started worker's exit.
struct FailSys {
    rec: Arc<RecSys>,
    fail: Option<usize>,
    pgid: AtomicI32,
}

impl FailSys {
    fn new(fail: Option<usize>) -> Arc<Self> {
        Arc::new(Self {
            rec: Arc::new(RecSys::default()),
            fail,
            pgid: AtomicI32::new(0),
        })
    }
}

impl Sys for FailSys {
    fn signal_group(&self, pgid: i32) -> std::io::Result<()> {
        self.pgid.store(pgid, Ordering::SeqCst);
        self.rec.signal_group(pgid)
    }
    fn kill(&self, child: &mut Child) -> std::io::Result<()> {
        self.rec.kill(child)
    }
    fn wait(&self, child: &mut Child) -> std::io::Result<()> {
        self.rec.wait(child)
    }
    fn spawn_worker(
        &self,
        role: usize,
        name: &'static str,
        f: Box<dyn FnOnce() + Send>,
    ) -> std::io::Result<JoinHandle<()>> {
        if self.fail == Some(role) {
            self.rec
                .events
                .lock()
                .unwrap()
                .push(format!("spawn-fail {role}"));
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "injected thread creation failure",
            ));
        }
        let rec = self.rec.clone();
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                f();
                rec.events.lock().unwrap().push(format!("exit {role}"));
            })
    }
}

fn spawn(fx: &Fx, sys: &Arc<FailSys>) -> std::io::Result<Proc> {
    let seam: Arc<dyn Sys> = sys.clone();
    Proc::spawn_with(&sh_prepared(fx), 1 << 20, 4096, seam)
}

#[test]
fn each_worker_creation_failure_rolls_back_the_started_child() {
    for (k, worker) in WORKERS.iter().enumerate() {
        let fx = fixture();
        let sys = FailSys::new(Some(k));
        let err = spawn(&fx, &sys).err().expect("ordinary error, not a Proc");
        assert_eq!(err.kind(), std::io::ErrorKind::WouldBlock, "worker {k}");
        let msg = err.to_string();
        assert!(msg.contains(worker) && msg.contains("injected"), "{msg}");
        let ev = sys.rec.events();
        // The child was settled exactly once by the owner, after the failure.
        let fail_at = ev.iter().position(|e| *e == format!("spawn-fail {k}"));
        let group_at = ev.iter().position(|e| e == "group");
        assert!(
            fail_at < group_at && fail_at.is_some(),
            "worker {k}: {ev:?}"
        );
        assert_eq!(sys.rec.count("group"), 1, "worker {k}: {ev:?}");
        assert_eq!(sys.rec.count("wait"), 1, "worker {k}: {ev:?}");
        // Every worker that did start reached its terminal state.
        for j in 0..k {
            assert_eq!(sys.rec.count(&format!("exit {j}")), 1, "worker {k}: {ev:?}");
        }
        assert!(!ev.iter().any(|e| e.starts_with(&format!("exit {k}"))));
        assert_gone(sys.pgid.load(Ordering::SeqCst));
    }
}

#[test]
fn full_startup_keeps_all_three_io_roles() {
    let fx = fixture();
    let sys = FailSys::new(None);
    let p = spawn(&fx, &sys).unwrap();
    assert_eq!(p.settlement(), Settlement::Live);
    assert_eq!(p.terminate(Closed::Host("shutdown")), Settlement::Settled);
    drop(p);
    // All three roles ran and finished through the owned cleanup.
    let end = Instant::now() + Duration::from_secs(10);
    while (0..3).any(|j| sys.rec.count(&format!("exit {j}")) == 0) {
        assert!(Instant::now() < end, "{:?}", sys.rec.events());
        std::thread::yield_now();
    }
    assert_eq!(sys.rec.count("group"), 1);
}

#[test]
fn manager_reports_a_start_failure_and_stays_usable() {
    let fx = fixture();
    let m = mgr(|_| {});
    let h = m.prepare(PROJECT, hostile(&fx, "")).unwrap();
    let sys = FailSys::new(Some(1));
    h.set_test_sys(Some(sys.clone()));
    let out = h.invoke(
        &decide("i1"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    match &out {
        Outcome::Unavailable {
            reason,
            request_sent: false,
            fallback_allowed: true,
        } => {
            assert_eq!(reason.code, "SPX-HPC004");
            assert!(reason.message.contains(WORKERS[1]), "{reason}");
        }
        _ => panic!("start failure classification: {out:?}"),
    }
    assert_eq!(h.pid(), None, "no half-constructed process is cached");
    assert_eq!(h.invoke_frames_queued(), 0);
    assert_eq!(h.gate_counts(), (0, 0));
    assert_eq!(h.consecutive_crashes(), 0);
    assert_gone(sys.pgid.load(Ordering::SeqCst));
    // The next independent invocation is healthy.
    h.set_test_sys(None);
    let ok = h.invoke(
        &decide("i2"),
        InvocationClass::SideEffecting,
        &CancelToken::new(),
    );
    assert!(matches!(ok, Outcome::Completed(_)), "{ok:?}");
    assert_eq!(h.invoke_frames_queued(), 1);
    assert_eq!(h.gate_counts(), (0, 0));
    assert_eq!(m.budget_used().jobs, 2);
}
