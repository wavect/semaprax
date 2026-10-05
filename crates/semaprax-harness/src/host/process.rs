//! One adapter process: own process group, bounded stdout framing, a stderr
//! ring that can never block the child, and host-side protocol policing.
//!
//! The stdout reader thread is the only consumer of adapter bytes. It reads
//! one LF-delimited frame at a time (never more than `frame_cap` bytes held),
//! then either delivers it to the single waiter registered for that id or
//! declares a protocol violation, kills the process group and wakes every
//! waiter. Waiters poll with `recv_timeout`, so cancellation never sits behind
//! a blocking read.
//!
//! Signal and reap authority has exactly one owner per process generation
//! (`Shared::settle`): termination, violation, cancellation, timeout and drop
//! all serialize through it. The numeric process-group id is signalled only
//! while the group leader is still unreaped (so the id cannot have been
//! reused); once the leader is reaped that authority is retired for good.

use super::launch::Prepared;
use crate::diag::HarnessDiagnostic;
use crate::json::{parse_frame, JsonLimits};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};

/// Why a process stopped delivering frames.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Closed {
    /// Stdout reached EOF without host action (exit or crash).
    Exited,
    /// The host terminated it (`deadline`, `cancel`, `shutdown`, ...).
    Host(&'static str),
    /// Protocol violation; the adapter must be quarantined.
    Violation(HarnessDiagnostic),
    /// Writing to the adapter's stdin failed. A frame may have been partly
    /// transmitted, so pending requests stay "sent".
    Transport(HarnessDiagnostic),
}

pub(crate) enum Delivery {
    Result(Value),
    Error(String),
    Closed(Closed),
}

#[derive(Default)]
struct Inner {
    pending: HashMap<u64, SyncSender<Delivery>>,
    closed: Option<Closed>,
}

struct Ring {
    buf: VecDeque<u8>,
    cap: usize,
    dropped: u64,
}

/// The OS boundary of the process owner. Production uses [`RealSys`]; tests
/// substitute a recording seam to observe signal/wait ordering.
pub(crate) trait Sys: Send + Sync {
    /// `SIGKILL` the whole process group. A group with no members is `Ok`.
    fn signal_group(&self, pgid: i32) -> std::io::Result<()>;
    /// `SIGKILL` the (unreaped) group leader itself.
    fn kill(&self, child: &mut Child) -> std::io::Result<()> {
        child.kill()
    }
    /// Reap the group leader.
    fn wait(&self, child: &mut Child) -> std::io::Result<()> {
        child.wait().map(drop)
    }
    /// The byte sink the stdin writer uses.
    fn stdin(&self, s: ChildStdin) -> Box<dyn Write + Send> {
        Box::new(s)
    }
}

pub(crate) struct RealSys;

impl Sys for RealSys {
    fn signal_group(&self, pgid: i32) -> std::io::Result<()> {
        let Some(pid) = rustix::process::Pid::from_raw(pgid) else {
            return Err(std::io::Error::other("invalid process-group id"));
        };
        match rustix::process::kill_process_group(pid, rustix::process::Signal::KILL) {
            Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
            // macOS answers EPERM for a group whose members are all exiting
            // or zombies (it signals every live permitted member and succeeds
            // otherwise). The group id is the unreaped leader's pid, so a
            // non-reaping `waitid` confirms that case within a short bound.
            Err(rustix::io::Errno::PERM) if leader_exits(pid) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

/// Whether the (unreaped, owned) leader has exited, or does so within a
/// short bound, without reaping it.
fn leader_exits(pid: rustix::process::Pid) -> bool {
    use rustix::process::{waitid, WaitId, WaitIdOptions};
    let opts = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT | WaitIdOptions::NOHANG;
    let end = std::time::Instant::now() + std::time::Duration::from_millis(250);
    loop {
        if matches!(waitid(WaitId::Pid(pid), opts), Ok(Some(_))) {
            return true;
        }
        if std::time::Instant::now() >= end {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

/// Physical settlement of one process generation.
enum Settle {
    /// Owned and unreaped: the group id is still ours to signal.
    Live(Child),
    /// Leader reaped after a confirmed group signal; no authority remains.
    Settled,
    /// Cleanup did not complete. With `Some(child)` the leader is still
    /// unreaped and owned, so a later settle may retry; with `None` it was
    /// reaped but the group signal failed, and nothing may be signalled again.
    Incomplete { child: Option<Child>, why: String },
}

/// Observable cleanup outcome (tests and audit).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Settlement {
    Live,
    Settled,
    Incomplete(String),
}

struct Shared {
    pid: i32,
    inner: Mutex<Inner>,
    ring: Mutex<Ring>,
    owner: Mutex<Settle>,
    sys: Arc<dyn Sys>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    /// Kill the group and reap the leader, once. Concurrent callers serialize
    /// on the owner lock: the first does the work, later ones observe the
    /// settled state and send no signal. Lock order: `owner` is never held
    /// while taking `inner` or `ring`.
    fn settle(&self) {
        let mut o = lock(&self.owner);
        let mut child = match std::mem::replace(&mut *o, Settle::Settled) {
            Settle::Live(c) | Settle::Incomplete { child: Some(c), .. } => c,
            done => {
                *o = done;
                return;
            }
        };
        // The leader is unreaped here, so the group id cannot be reused yet.
        let group = self.sys.signal_group(self.pid);
        let leader = self.sys.kill(&mut child);
        if let (Err(g), Err(k)) = (&group, &leader) {
            // Nothing proves the leader was signalled: waiting could block
            // forever. Keep ownership so an explicit retry stays possible.
            *o = Settle::Incomplete {
                why: format!("cannot signal adapter process: group: {g}; leader: {k}"),
                child: Some(child),
            };
            return;
        }
        if let Err(e) = self.sys.wait(&mut child) {
            *o = Settle::Incomplete {
                why: format!("cannot reap adapter process: {e}"),
                child: Some(child),
            };
            return;
        }
        // Reaped: the numeric id is no longer ours. Retire it either way.
        *o = match group {
            Ok(()) => Settle::Settled,
            Err(e) => Settle::Incomplete {
                why: format!("process-group signal failed: {e}"),
                child: None,
            },
        };
    }

    fn settlement(&self) -> Settlement {
        match &*lock(&self.owner) {
            Settle::Live(_) => Settlement::Live,
            Settle::Settled => Settlement::Settled,
            Settle::Incomplete { why, .. } => Settlement::Incomplete(why.clone()),
        }
    }

    /// Record why the process is done (first reason wins) and wake waiters.
    fn close(&self, why: Closed) {
        let pending: Vec<_> = {
            let mut g = lock(&self.inner);
            if g.closed.is_none() {
                g.closed = Some(why);
            }
            g.pending.drain().collect()
        };
        let reason = lock(&self.inner).closed.clone().expect("set above");
        for (_, tx) in pending {
            let _ = tx.try_send(Delivery::Closed(reason.clone()));
        }
    }

    fn violate(&self, d: HarnessDiagnostic) {
        self.close(Closed::Violation(d));
        self.settle();
    }
}

pub(crate) struct Proc {
    shared: Arc<Shared>,
    writer: SyncSender<Vec<u8>>,
    next_id: std::sync::atomic::AtomicU64,
}

fn violation(code: &'static str, msg: String) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Bounded line read. `Ok(None)` is clean EOF; an unterminated tail is dropped.
fn read_frame<R: BufRead>(r: &mut R, cap: usize) -> Result<Option<Vec<u8>>, HarnessDiagnostic> {
    let mut line = Vec::new();
    loop {
        let buf = r
            .fill_buf()
            .map_err(|e| violation("SPX-HPC007", format!("adapter stdout read failed: {e}")))?;
        if buf.is_empty() {
            return Ok(None);
        }
        let (take, found) = match buf.iter().position(|b| *b == b'\n') {
            Some(i) => (i, true),
            None => (buf.len(), false),
        };
        if line.len() + take > cap {
            return Err(violation(
                "SPX-HPC012",
                format!("adapter frame exceeds the {cap}-byte limit"),
            ));
        }
        line.extend_from_slice(&buf[..take]);
        r.consume(take + usize::from(found));
        if found {
            return Ok(Some(line));
        }
    }
}

fn dispatch(shared: &Shared, frame: &[u8], cap: usize) -> Result<(), HarnessDiagnostic> {
    let v = parse_frame(frame, &JsonLimits::frame(cap)).map_err(|d| {
        violation(
            "SPX-HPC009",
            format!(
                "unparseable frame on adapter stdout ({}): {}",
                d.code, d.message
            ),
        )
    })?;
    let obj = v
        .as_object()
        .ok_or_else(|| violation("SPX-HPC009", "adapter frame is not a JSON object".into()))?;
    if let Some(m) = obj.get("method") {
        let m: String = m.as_str().unwrap_or("?").chars().take(64).collect();
        return Err(violation(
            "SPX-HPC011",
            format!("adapter initiated `{m}`; adapters may not send requests or notifications"),
        ));
    }
    let id = obj.get("id").and_then(Value::as_u64).ok_or_else(|| {
        violation(
            "SPX-HPC009",
            "adapter response has no host-issued integer id".into(),
        )
    })?;
    let delivery = match (obj.get("result"), obj.get("error")) {
        (Some(r), None) => Delivery::Result(r.clone()),
        (None, Some(e)) => Delivery::Error(
            e.get("message")
                .and_then(Value::as_str)
                .unwrap_or("error")
                .chars()
                .take(200)
                .collect(),
        ),
        _ => {
            return Err(violation(
                "SPX-HPC009",
                "response must carry exactly one of result/error".into(),
            ))
        }
    };
    let tx = lock(&shared.inner).pending.remove(&id);
    match tx {
        Some(tx) => {
            let _ = tx.try_send(delivery);
            Ok(())
        }
        None => Err(violation(
            "SPX-HPC010",
            format!("adapter answered unknown or already-answered id {id}"),
        )),
    }
}

/// The stdin writer worker. A terminal write or flush failure is reported
/// through the shared first-failure close (waking every pending waiter) and
/// the single process owner; queued frames are then dropped with their
/// callers already released.
fn write_frames(mut w: Box<dyn Write + Send>, rx: Receiver<Vec<u8>>, shared: &Shared) {
    while let Ok(mut f) = rx.recv() {
        f.push(b'\n');
        if let Err(e) = w.write_all(&f).and_then(|_| w.flush()) {
            let msg: String = format!("adapter stdin write failed: {e}")
                .chars()
                .take(200)
                .collect();
            shared.close(Closed::Transport(violation("SPX-HPC007", msg)));
            shared.settle();
            return;
        }
    }
}

impl Proc {
    pub(crate) fn spawn(p: &Prepared, frame_cap: usize, ring_cap: usize) -> std::io::Result<Proc> {
        Self::spawn_with(p, frame_cap, ring_cap, Arc::new(RealSys))
    }

    pub(crate) fn spawn_with(
        p: &Prepared,
        frame_cap: usize,
        ring_cap: usize,
        sys: Arc<dyn Sys>,
    ) -> std::io::Result<Proc> {
        let mut cmd = Command::new(&p.program);
        cmd.args(&p.args)
            .env_clear()
            .envs(&p.env)
            .current_dir(&p.cwd)
            .process_group(0);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn()?;
        let (stdin, stdout, stderr) = (
            child.stdin.take().expect("piped"),
            child.stdout.take().expect("piped"),
            child.stderr.take().expect("piped"),
        );
        let mut stderr = stderr;
        let stdin = sys.stdin(stdin);
        let shared = Arc::new(Shared {
            pid: child.id() as i32,
            inner: Mutex::new(Inner::default()),
            ring: Mutex::new(Ring {
                buf: VecDeque::new(),
                cap: ring_cap,
                dropped: 0,
            }),
            owner: Mutex::new(Settle::Live(child)),
            sys,
        });

        let (wtx, wrx) = sync_channel::<Vec<u8>>(8);
        let s = shared.clone();
        std::thread::spawn(move || write_frames(stdin, wrx, &s));

        let s = shared.clone();
        std::thread::spawn(move || {
            let mut r = BufReader::with_capacity(8192, stdout);
            loop {
                match read_frame(&mut r, frame_cap) {
                    Ok(Some(frame)) => {
                        if let Err(d) = dispatch(&s, &frame, frame_cap) {
                            s.violate(d);
                            return;
                        }
                    }
                    Ok(None) => return s.close(Closed::Exited),
                    Err(d) if d.code == "SPX-HPC007" => return s.close(Closed::Exited),
                    Err(d) => return s.violate(d),
                }
            }
        });

        let s = shared.clone();
        std::thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = stderr.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                let mut g = lock(&s.ring);
                g.buf.extend(&chunk[..n]);
                while g.buf.len() > g.cap {
                    g.buf.pop_front();
                    g.dropped += 1;
                }
            }
        });

        Ok(Proc {
            shared,
            writer: wtx,
            next_id: std::sync::atomic::AtomicU64::new(1),
        })
    }

    pub(crate) fn closed(&self) -> Option<Closed> {
        lock(&self.shared.inner).closed.clone()
    }

    /// Register a waiter and queue the request frame. `Err` means the process
    /// is already closed (or its write queue is wedged) and nothing was sent.
    pub(crate) fn request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Receiver<Delivery>, Closed> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let (tx, rx) = sync_channel(1);
        {
            let mut g = lock(&self.shared.inner);
            if let Some(c) = &g.closed {
                return Err(c.clone());
            }
            g.pending.insert(id, tx);
        }
        let frame = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
            .to_string()
            .into_bytes();
        if self.writer.try_send(frame).is_err() {
            lock(&self.shared.inner).pending.remove(&id);
            return Err(self.closed().unwrap_or(Closed::Exited));
        }
        Ok(rx)
    }

    /// Fire-and-forget notification (`harness/cancel`).
    pub(crate) fn notify(&self, method: &str, params: Value) {
        let frame = json!({"jsonrpc": "2.0", "method": method, "params": params})
            .to_string()
            .into_bytes();
        let _ = self.writer.try_send(frame);
    }

    /// Wake waiters (first reason wins), then kill the whole process group
    /// and reap the leader through the single owner. Idempotent: once the
    /// leader is reaped no further signal is sent.
    /// Returns the physical cleanup state; `Incomplete` is never reported as
    /// settled.
    pub(crate) fn terminate(&self, why: Closed) -> Settlement {
        self.shared.close(why);
        self.shared.settle();
        self.shared.settlement()
    }

    #[cfg(test)]
    pub(crate) fn settlement(&self) -> Settlement {
        self.shared.settlement()
    }

    /// Test seam: the reader's protocol-violation cleanup path.
    #[cfg(test)]
    pub(crate) fn inject_violation(&self, d: HarnessDiagnostic) {
        self.shared.violate(d);
    }

    /// Captured stderr tail and the number of bytes dropped from the ring.
    pub(crate) fn stderr_tail(&self) -> (String, u64) {
        let g = lock(&self.shared.ring);
        (
            String::from_utf8_lossy(&g.buf.iter().copied().collect::<Vec<_>>()).into_owned(),
            g.dropped,
        )
    }

    pub(crate) fn pid(&self) -> i32 {
        self.shared.pid
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        let _ = self.terminate(Closed::Host("dropped"));
    }
}
