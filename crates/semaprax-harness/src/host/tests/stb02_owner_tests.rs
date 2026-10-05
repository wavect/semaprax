//! STB-02 (#545): one owner per process generation signals the group and
//! reaps the leader; after the reap no path may signal the numeric id again.

use super::*;
use crate::diag::HarnessDiagnostic;
use crate::host::launch::Prepared;
use crate::host::process::{Closed, Delivery, Proc, RealSys, Settlement, Sys};
use std::process::Child;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Barrier, Mutex};

/// Recording seam over the real OS. After the leader is reaped the group id
/// is treated as reused by an unrelated process: any later group signal is
/// recorded as `group-after-reap` and never reaches the OS.
#[derive(Default)]
pub(crate) struct RecSys {
    pub events: Mutex<Vec<String>>,
    reaped: AtomicBool,
    /// Fail this many `wait` calls first (without reaping).
    pub fail_waits: Mutex<u32>,
    /// Fail every group signal (simulated `EPERM`).
    pub fail_group: AtomicBool,
}

impl RecSys {
    fn log(&self, e: &str) {
        self.events.lock().unwrap().push(e.to_string());
    }
    pub fn events(&self) -> Vec<String> {
        self.events.lock().unwrap().clone()
    }
    pub fn count(&self, e: &str) -> usize {
        self.events().iter().filter(|x| *x == e).count()
    }
}

impl Sys for RecSys {
    fn signal_group(&self, pgid: i32) -> std::io::Result<()> {
        if self.reaped.load(Ordering::SeqCst) {
            self.log("group-after-reap");
            return Ok(());
        }
        if self.fail_group.load(Ordering::SeqCst) {
            self.log("group-err");
            return Err(std::io::Error::from_raw_os_error(1));
        }
        self.log("group");
        RealSys.signal_group(pgid)
    }
    fn kill(&self, child: &mut Child) -> std::io::Result<()> {
        self.log("kill");
        child.kill()
    }
    fn wait(&self, child: &mut Child) -> std::io::Result<()> {
        {
            let mut n = self.fail_waits.lock().unwrap();
            if *n > 0 {
                *n -= 1;
                self.log("wait-err");
                return Err(std::io::Error::other("injected wait failure"));
            }
        }
        child.wait()?;
        self.reaped.store(true, Ordering::SeqCst);
        self.log("wait");
        Ok(())
    }
}

/// `/bin/sh` that starts a grandchild in its own group, records both pids
/// and stays alive until killed.
pub(crate) fn sh_prepared(fx: &Fx) -> Prepared {
    let pids = fx.root.join("owner-pids");
    Prepared {
        program: PathBuf::from("/bin/sh"),
        args: vec![
            "-c".into(),
            format!(
                "sleep 300 & echo $$ $! > {p}.tmp && mv {p}.tmp {p}; wait",
                p = pids.display()
            )
            .into(),
        ],
        env: [("PATH".to_string(), "/bin:/usr/bin".to_string())].into(),
        cwd: fx.root.clone(),
        mode: IsolationMode::Subprocess,
    }
}

pub(crate) fn owner_pids(fx: &Fx) -> (i32, i32) {
    let s = super::stb_fixture::await_file(&fx.root.join("owner-pids"));
    let v: Vec<i32> = s.split_whitespace().map(|x| x.parse().unwrap()).collect();
    (v[0], v[1])
}

fn spawn(fx: &Fx, sys: &Arc<RecSys>) -> Arc<Proc> {
    let sys: Arc<dyn Sys> = sys.clone();
    Arc::new(Proc::spawn_with(&sh_prepared(fx), 1 << 20, 4096, sys).unwrap())
}

const ONE_SETTLEMENT: [&str; 3] = ["group", "kill", "wait"];

#[test]
fn terminate_twice_then_final_drop_signals_once() {
    let fx = fixture();
    let sys = Arc::new(RecSys::default());
    let a = spawn(&fx, &sys);
    let b = a.clone();
    let (leader, grandchild) = owner_pids(&fx);
    assert_eq!(a.settlement(), Settlement::Live);
    assert_eq!(a.terminate(Closed::Host("cancel")), Settlement::Settled);
    assert_eq!(b.terminate(Closed::Host("failed")), Settlement::Settled);
    drop(a);
    drop(b); // final reference: Drop runs terminate once more
    assert_eq!(sys.events(), ONE_SETTLEMENT, "signal/wait ordering");
    assert_eq!(sys.count("group-after-reap"), 0);
    assert_gone(leader);
    assert_gone(grandchild);
}

#[test]
fn violation_and_cancel_race_share_one_owner_and_late_callers_never_signal() {
    let fx = fixture();
    let sys = Arc::new(RecSys::default());
    let p = spawn(&fx, &sys);
    let (leader, grandchild) = owner_pids(&fx);
    let rx = p.request("harness/invoke", json!({})).unwrap();
    let gate = Arc::new(Barrier::new(2));
    let viol = HarnessDiagnostic::new("SPX-HPC009", "injected violation");
    let t = {
        let (p, gate, viol) = (p.clone(), gate.clone(), viol.clone());
        std::thread::spawn(move || {
            gate.wait();
            p.inject_violation(viol);
        })
    };
    gate.wait();
    p.terminate(Closed::Host("cancel"));
    t.join().unwrap();
    assert_eq!(sys.events(), ONE_SETTLEMENT);
    // The first authoritative reason is retained and the waiter is released.
    let first = p.closed().unwrap();
    assert!(
        first == Closed::Host("cancel") || first == Closed::Violation(viol.clone()),
        "{first:?}"
    );
    match rx.recv_timeout(Duration::from_secs(5)).unwrap() {
        Delivery::Closed(c) => assert_eq!(c, first),
        _ => panic!("pending waiter must get a terminal close"),
    }
    // Late callbacks of this (now obsolete) generation: zero new signals,
    // even though the simulated id now belongs to someone else.
    p.inject_violation(HarnessDiagnostic::new("SPX-HPC009", "late reader"));
    p.terminate(Closed::Host("deadline"));
    assert_eq!(p.closed().unwrap(), first, "first failure is sticky");
    drop(p);
    assert_eq!(sys.events(), ONE_SETTLEMENT);
    assert_eq!(sys.count("group-after-reap"), 0);
    assert_gone(leader);
    assert_gone(grandchild);
}

#[test]
fn failed_wait_is_incomplete_and_retry_keeps_single_ownership() {
    let fx = fixture();
    let sys = Arc::new(RecSys::default());
    *sys.fail_waits.lock().unwrap() = 1;
    let p = spawn(&fx, &sys);
    let (leader, _) = owner_pids(&fx);
    let s = p.terminate(Closed::Host("cancel"));
    assert!(
        matches!(&s, Settlement::Incomplete(w) if w.contains("reap")),
        "an ignored wait error is not settlement: {s:?}"
    );
    // The leader is still unreaped and owned, so an explicit retry may
    // signal again; after it reaps, nothing does.
    assert_eq!(p.terminate(Closed::Host("cancel")), Settlement::Settled);
    drop(p);
    assert_eq!(
        sys.events(),
        ["group", "kill", "wait-err", "group", "kill", "wait"]
    );
    assert_gone(leader);
}

#[test]
fn failed_group_signal_is_incomplete_and_retires_authority_after_reap() {
    let fx = fixture();
    let sys = Arc::new(RecSys::default());
    sys.fail_group.store(true, Ordering::SeqCst);
    let p = spawn(&fx, &sys);
    let (leader, grandchild) = owner_pids(&fx);
    let s = p.terminate(Closed::Host("shutdown"));
    assert!(
        matches!(&s, Settlement::Incomplete(w) if w.contains("process-group")),
        "{s:?}"
    );
    // Reaped leader: no later call may signal the numeric id again.
    sys.fail_group.store(false, Ordering::SeqCst);
    p.terminate(Closed::Host("shutdown"));
    drop(p);
    assert_eq!(sys.events(), ["group-err", "kill", "wait"]);
    assert_gone(leader);
    // The seam really withheld the group signal: the grandchild survived.
    // Test cleanup only: it is still our own descendant, killed by pid.
    assert!(pid_alive(grandchild));
    let _ = rustix::process::kill_process(
        rustix::process::Pid::from_raw(grandchild).unwrap(),
        rustix::process::Signal::KILL,
    );
}

#[test]
fn real_owner_settles_the_child_and_its_group() {
    let fx = fixture();
    let p = Proc::spawn(&sh_prepared(&fx), 1 << 20, 4096).unwrap();
    let (leader, grandchild) = owner_pids(&fx);
    assert!(pid_alive(leader) && pid_alive(grandchild));
    assert_eq!(p.terminate(Closed::Host("shutdown")), Settlement::Settled);
    assert_gone(leader);
    assert_gone(grandchild);
}
