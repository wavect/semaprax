//! Output sinks for the bridge session (DV-05).
//!
//! A response frame is delivered through a [`Sink`]. Two policies exist:
//!
//! * [`Direct`] writes on the calling thread under one mutex. It serves any
//!   embedded `Write`: the session cannot end while that writer is blocked
//!   (it cannot be cancelled portably), so a blocked write is only *observed*
//!   (`stalled`) and the session reacts by cancelling live adapter work and
//!   refusing new admissions.
//! * [`Pumped`] hands frames to one detached writer thread through a small
//!   bounded queue. A producer waits at most the stall allowance for room, so
//!   a consumer that stops reading turns into `TimedOut` and the session can
//!   end even though the writer thread stays blocked in the kernel. Used for
//!   the installed stdio entry point, where process exit reclaims that thread.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Frames the pump may hold before producers wait. Admission permits already
/// bound response workers, so this only absorbs inline replies.
const PUMP_CAPACITY: usize = 8;

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn stalled_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "bridge output stalled: the client stopped reading responses",
    )
}

pub(crate) trait Sink: Sync {
    /// Deliver one frame (without its LF).
    fn put(&self, line: &str) -> io::Result<()>;
    /// A write has been blocked longer than the stall allowance.
    fn stalled(&self) -> bool;
    /// Flush everything accepted so far, bounded by the stall allowance.
    fn finish(&self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) struct Direct<W> {
    out: Mutex<W>,
    since: Mutex<Option<Instant>>,
    stall: Duration,
}

impl<W: Write> Direct<W> {
    pub(crate) fn new(out: W, stall: Duration) -> Self {
        Self {
            out: Mutex::new(out),
            since: Mutex::new(None),
            stall,
        }
    }
}

impl<W: Write + Send> Sink for Direct<W> {
    fn put(&self, line: &str) -> io::Result<()> {
        let mut o = lock(&self.out);
        *lock(&self.since) = Some(Instant::now());
        let r = writeln!(o, "{line}").and_then(|()| o.flush());
        *lock(&self.since) = None;
        r
    }

    fn stalled(&self) -> bool {
        lock(&self.since).is_some_and(|t| t.elapsed() >= self.stall)
    }
}

#[derive(Default)]
struct PumpState {
    queue: VecDeque<String>,
    writing_since: Option<Instant>,
    failed: Option<io::ErrorKind>,
}

pub(crate) struct Pumped {
    state: Mutex<PumpState>,
    changed: Condvar,
    stall: Duration,
}

impl Pumped {
    pub(crate) fn spawn<W: Write + Send + 'static>(mut out: W, stall: Duration) -> Arc<Self> {
        let p = Arc::new(Self {
            state: Mutex::default(),
            changed: Condvar::new(),
            stall,
        });
        let me = p.clone();
        std::thread::spawn(move || loop {
            let line = {
                let mut g = lock(&me.state);
                loop {
                    if let Some(l) = g.queue.front().cloned() {
                        g.writing_since = Some(Instant::now());
                        break l;
                    }
                    if Arc::strong_count(&me) == 1 {
                        return;
                    }
                    g = me
                        .changed
                        .wait_timeout(g, Duration::from_millis(100))
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
            };
            let r = writeln!(out, "{line}").and_then(|()| out.flush());
            let mut g = lock(&me.state);
            g.writing_since = None;
            g.queue.pop_front();
            if let Err(e) = r {
                g.failed = Some(e.kind());
                g.queue.clear();
            }
            drop(g);
            me.changed.notify_all();
        });
        p
    }

    fn wait<'a>(
        &self,
        g: MutexGuard<'a, PumpState>,
        until: Instant,
    ) -> Option<MutexGuard<'a, PumpState>> {
        let left = until.checked_duration_since(Instant::now())?;
        Some(
            self.changed
                .wait_timeout(g, left)
                .unwrap_or_else(|e| e.into_inner())
                .0,
        )
    }
}

impl Sink for Arc<Pumped> {
    fn put(&self, line: &str) -> io::Result<()> {
        let until = Instant::now() + self.stall;
        let mut g = lock(&self.state);
        loop {
            if let Some(k) = g.failed {
                return Err(k.into());
            }
            if g.queue.len() < PUMP_CAPACITY {
                g.queue.push_back(line.to_string());
                drop(g);
                self.changed.notify_all();
                return Ok(());
            }
            g = self.wait(g, until).ok_or_else(stalled_error)?;
        }
    }

    fn stalled(&self) -> bool {
        lock(&self.state)
            .writing_since
            .is_some_and(|t| t.elapsed() >= self.stall)
    }

    fn finish(&self) -> io::Result<()> {
        let until = Instant::now() + self.stall;
        let mut g = lock(&self.state);
        loop {
            if let Some(k) = g.failed {
                return Err(k.into());
            }
            if g.queue.is_empty() {
                return Ok(());
            }
            g = self.wait(g, until).ok_or_else(stalled_error)?;
        }
    }
}
