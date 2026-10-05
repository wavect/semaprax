//! STB-03 (#546): a failed adapter stdin write wakes every pending waiter
//! through the first-failure close and settles the process through its
//! single owner, without granting replay to side effects.

use super::stb02_owner_tests::{owner_pids, sh_prepared, RecSys};
use super::stb_fixture::*;
use super::*;
use crate::host::process::{Closed, Delivery, Proc, Settlement, Sys};
use std::io::Write;
use std::process::{Child, ChildStdin};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
enum Fault {
    /// Pass this many bytes to the real pipe, then fail the write.
    AfterPrefix(usize),
    /// Every write succeeds; flush fails.
    Flush,
}

/// Recording OS seam whose stdin sink waits for a test release, then fails.
struct FaultSys {
    rec: RecSys,
    fault: Fault,
    hold: Mutex<Option<Receiver<()>>>,
}

struct FaultyStdin {
    inner: ChildStdin,
    fault: Fault,
    written: usize,
    hold: Option<Receiver<()>>,
    sys: Arc<FaultSys>,
}

impl Write for FaultyStdin {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if let Some(h) = self.hold.take() {
            let _ = h.recv();
        }
        if let Fault::AfterPrefix(n) = self.fault {
            if self.written >= n {
                self.sys.rec.events.lock().unwrap().push("write-err".into());
                return Err(std::io::ErrorKind::BrokenPipe.into());
            }
            let take = buf.len().min(n - self.written);
            let k = self.inner.write(&buf[..take])?;
            self.written += k;
            return Ok(k);
        }
        self.inner.write(buf)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if let Fault::Flush = self.fault {
            self.sys.rec.events.lock().unwrap().push("flush-err".into());
            return Err(std::io::Error::other("injected flush failure"));
        }
        self.inner.flush()
    }
}

/// The writer worker drops its sink only after its failure handling (close
/// and settle) returned, so this event marks the worker's terminal state.
impl Drop for FaultyStdin {
    fn drop(&mut self) {
        self.sys
            .rec
            .events
            .lock()
            .unwrap()
            .push("sink-dropped".into());
    }
}

fn await_event(sys: &FaultSys, e: &str) {
    let end = Instant::now() + Duration::from_secs(10);
    while sys.rec.count(e) == 0 {
        assert!(Instant::now() < end, "event {e} never happened");
        std::thread::yield_now();
    }
}

/// `Sys` is implemented on the shared handle so the sink can log into it.
struct Seam(Arc<FaultSys>);

impl Sys for Seam {
    fn signal_group(&self, pgid: i32) -> std::io::Result<()> {
        self.0.rec.signal_group(pgid)
    }
    fn kill(&self, child: &mut Child) -> std::io::Result<()> {
        self.0.rec.kill(child)
    }
    fn wait(&self, child: &mut Child) -> std::io::Result<()> {
        self.0.rec.wait(child)
    }
    fn stdin(&self, s: ChildStdin) -> Box<dyn Write + Send> {
        Box::new(FaultyStdin {
            inner: s,
            fault: self.0.fault,
            written: 0,
            hold: self.0.hold.lock().unwrap().take(),
            sys: self.0.clone(),
        })
    }
}

fn spawn_faulty(fx: &Fx, fault: Fault) -> (Arc<Proc>, Arc<FaultSys>, Sender<()>) {
    let (go, hold) = channel();
    let sys = Arc::new(FaultSys {
        rec: RecSys::default(),
        fault,
        hold: Mutex::new(Some(hold)),
    });
    let seam: Arc<dyn Sys> = Arc::new(Seam(sys.clone()));
    let p = Proc::spawn_with(&sh_prepared(fx), 1 << 20, 4096, seam).unwrap();
    (Arc::new(p), sys, go)
}

fn transport(c: &Closed) -> bool {
    matches!(c, Closed::Transport(d) if d.code == "SPX-HPC007" && d.message.contains("stdin write failed"))
}

/// All registered waiters (frames already queued) settle on one failure.
fn waiters_all_settle(fault: Fault, failure: &str) {
    let fx = fixture();
    let (p, sys, go) = spawn_faulty(&fx, fault);
    let (leader, grandchild) = owner_pids(&fx);
    let rxs: Vec<_> = (0..3)
        .map(|_| p.request("harness/invoke", json!({})).unwrap())
        .collect();
    go.send(()).unwrap();
    for rx in rxs {
        match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
            Delivery::Closed(c) => assert!(transport(&c), "{c:?}"),
            _ => panic!("waiter must receive the transport failure"),
        }
    }
    assert!(transport(&p.closed().unwrap()));
    // Process settled by the single owner after the observed write failure.
    await_event(&sys, "sink-dropped");
    assert_eq!(p.settlement(), Settlement::Settled);
    assert_eq!(
        sys.rec.events(),
        [failure, "group", "kill", "wait", "sink-dropped"]
    );
    // A later request is a genuine pre-enqueue refusal: nothing is sent.
    let late = p.request("harness/invoke", json!({}));
    assert!(matches!(&late, Err(c) if transport(c)));
    drop(p);
    assert_eq!(sys.rec.count("group"), 1);
    assert_gone(leader);
    assert_gone(grandchild);
}

#[test]
fn failure_after_a_partial_frame_wakes_every_waiter() {
    waiters_all_settle(Fault::AfterPrefix(7), "write-err");
}

#[test]
fn flush_failure_wakes_every_waiter() {
    waiters_all_settle(Fault::Flush, "flush-err");
}

#[test]
fn earlier_cancel_reason_survives_a_later_write_failure() {
    let fx = fixture();
    let (p, sys, go) = spawn_faulty(&fx, Fault::AfterPrefix(0));
    let rx = p.request("harness/invoke", json!({})).unwrap();
    p.terminate(Closed::Host("cancel"));
    go.send(()).unwrap();
    match rx.recv_timeout(Duration::from_secs(10)).unwrap() {
        Delivery::Closed(c) => assert_eq!(c, Closed::Host("cancel")),
        _ => panic!("terminal close expected"),
    }
    // The writer's own close and settle ran (and found both already done).
    await_event(&sys, "sink-dropped");
    assert_eq!(p.closed().unwrap(), Closed::Host("cancel"));
    assert_eq!(
        sys.rec.events(),
        ["group", "kill", "wait", "write-err", "sink-dropped"]
    );
}

#[test]
fn half_closed_adapter_fails_a_queued_side_effect_promptly_and_uncertainly() {
    let fx = fixture();
    let closed = fx.root.join("stdin-closed");
    let exit = fx.root.join("exit");
    let m = mgr(|_| {});
    let spec = stb_spec(
        &fx,
        &[
            ("STB_CLOSE_STDIN", closed.to_str().unwrap()),
            ("STB_EXIT", exit.to_str().unwrap()),
        ],
        |v| {
            v["resources"]["invoke_timeout_ms"] = json!(30_000);
        },
    );
    let h = m.prepare(PROJECT, spec).unwrap();
    // Initialize through a call that fails pre-dispatch (unaccepted op)...
    let warm = h.invoke(
        &request(
            CapabilityKind::CommandView,
            "wrap",
            json!({"form": "wrapper", "argv": ["ls"]}),
            "w",
        ),
        InvocationClass::SafeRead,
        &CancelToken::new(),
    );
    assert!(
        matches!(&warm, Outcome::Refused(d) if d.code == "SPX-HPC020"),
        "{warm:?}"
    );
    // ...then wait until the adapter has closed its stdin (stdout stays open).
    let pid: i32 = await_file(&closed).trim().parse().unwrap();
    let h2 = h.clone();
    let out = within(Duration::from_secs(10), move || {
        h2.invoke(
            &decide("i1"),
            InvocationClass::SideEffecting,
            &CancelToken::new(),
        )
    });
    assert!(
        matches!(&out, Outcome::Uncertain(d) if d.code == "SPX-HPC007" && d.message.contains("stdin write failed")),
        "transport failure, not EOF or timeout; never replayable: {out:?}"
    );
    assert_eq!(h.invoke_frames_queued(), 1);
    assert_gone(pid);
    assert_eq!(h.pid(), None, "no half-closed adapter stays available");
    assert!(matches!(h.state(), AdapterState::Unavailable(_)));
    let _ = std::fs::write(&exit, "x");
}
