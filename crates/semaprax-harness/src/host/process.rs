//! One adapter process: own process group, bounded stdout framing, a stderr
//! ring that can never block the child, and host-side protocol policing.
//!
//! The stdout reader thread is the only consumer of adapter bytes. It reads
//! one LF-delimited frame at a time (never more than `frame_cap` bytes held),
//! then either delivers it to the single waiter registered for that id or
//! declares a protocol violation, kills the process group and wakes every
//! waiter. Waiters poll with `recv_timeout`, so cancellation never sits behind
//! a blocking read.

use super::launch::Prepared;
use crate::diag::HarnessDiagnostic;
use crate::json::{parse_frame, JsonLimits};
use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
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

struct Shared {
    pid: i32,
    inner: Mutex<Inner>,
    ring: Mutex<Ring>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl Shared {
    fn kill_group(&self) {
        if let Some(pid) = rustix::process::Pid::from_raw(self.pid) {
            let _ = rustix::process::kill_process_group(pid, rustix::process::Signal::KILL);
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
        self.kill_group();
    }
}

pub(crate) struct Proc {
    shared: Arc<Shared>,
    child: Mutex<Option<Child>>,
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

impl Proc {
    pub(crate) fn spawn(p: &Prepared, frame_cap: usize, ring_cap: usize) -> std::io::Result<Proc> {
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
        let shared = Arc::new(Shared {
            pid: child.id() as i32,
            inner: Mutex::new(Inner::default()),
            ring: Mutex::new(Ring {
                buf: VecDeque::new(),
                cap: ring_cap,
                dropped: 0,
            }),
        });
        let (mut stdin, stdout, mut stderr) = (
            child.stdin.take().expect("piped"),
            child.stdout.take().expect("piped"),
            child.stderr.take().expect("piped"),
        );

        let (wtx, wrx) = sync_channel::<Vec<u8>>(8);
        std::thread::spawn(move || {
            while let Ok(mut f) = wrx.recv() {
                f.push(b'\n');
                if stdin.write_all(&f).and_then(|_| stdin.flush()).is_err() {
                    break;
                }
            }
        });

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
            child: Mutex::new(Some(child)),
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

    /// Kill the whole process group, reap the child, wake waiters. Idempotent.
    pub(crate) fn terminate(&self, why: Closed) {
        self.shared.close(why);
        self.shared.kill_group();
        if let Some(mut c) = lock(&self.child).take() {
            let _ = c.wait();
        }
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
        self.terminate(Closed::Host("dropped"));
    }
}
