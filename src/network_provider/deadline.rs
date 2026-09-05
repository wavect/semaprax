//! Aggregate operation deadlines for the real-socket provider.
//!
//! Three different bounds appear in Bounded Language Network I/O v1 and they
//! are deliberately distinct:
//!
//! - a **per-syscall timeout** is what a socket option such as `SO_RCVTIMEO`
//!   enforces. One blocking call returns after it, and a caller that issues
//!   several calls spends the timeout several times over;
//! - an **operation deadline** is the monotonic instant by which one whole
//!   provider operation — name resolution, every candidate address, the TLS
//!   handshake, every partial write, and every retried read — must finish. It
//!   is what this module implements, and it is the bound a host selects;
//! - an **invocation budget** is the evaluator's own accounting: dense
//!   handles, chunk capacities, and the cumulative byte total for one
//!   invocation. It bounds how much a program may transfer, not how long a
//!   transfer may take.
//!
//! The deadline is monotonic. It is derived from an injected [`MonotonicClock`]
//! so budget-boundary regressions can advance time explicitly instead of
//! sleeping, and the shipped [`SystemClock`] reads `Instant`, which no wall
//! clock adjustment can move backwards.
//!
//! This module promises a *bound on waiting*, not forced cancellation. A
//! blocking syscall that a platform will not interrupt is bounded by the
//! remaining slice handed to it; nothing here claims the authority to abort a
//! call the host cannot abort.

use std::fmt::Debug;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The largest aggregate deadline any caller may select. A host that asks for
/// more is clamped to this fixed safe maximum rather than refused, so no
/// configuration path can produce an unbounded operation.
pub const MAX_OPERATION_DEADLINE: Duration = Duration::from_secs(30);

/// The aggregate deadline a provider built without an explicit policy uses.
pub const DEFAULT_OPERATION_DEADLINE: Duration = MAX_OPERATION_DEADLINE;

/// The smallest slice handed to a blocking syscall. `std` rejects a zero
/// socket timeout as "no timeout", so a deadline that is nearly, but not
/// quite, exhausted still yields a genuinely bounded call.
pub const MIN_SYSCALL_SLICE: Duration = Duration::from_millis(1);

/// A monotonic clock seam.
///
/// Implementations report nanoseconds elapsed since an arbitrary fixed origin
/// of their own choosing. The value must never decrease.
pub trait MonotonicClock: Debug + Send + Sync {
    /// Nanoseconds since this clock's origin.
    fn elapsed_nanos(&self) -> u128;
}

/// The shipped clock, reading `std::time::Instant`.
#[derive(Clone, Debug)]
pub struct SystemClock {
    origin: Instant,
}

impl Default for SystemClock {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl SystemClock {
    /// Create a clock whose origin is now.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl MonotonicClock for SystemClock {
    fn elapsed_nanos(&self) -> u128 {
        self.origin.elapsed().as_nanos()
    }
}

/// A clock a test advances by hand, so a budget boundary is exercised without
/// waiting for it.
#[derive(Debug, Default)]
pub struct ScriptedClock {
    nanos: AtomicU64,
}

impl ScriptedClock {
    /// Create a clock reading zero.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Move the clock forward. Time never moves backwards.
    pub fn advance(&self, step: Duration) {
        let step = u64::try_from(step.as_nanos()).unwrap_or(u64::MAX);
        self.nanos.fetch_add(step, Ordering::SeqCst);
    }
}

impl MonotonicClock for ScriptedClock {
    fn elapsed_nanos(&self) -> u128 {
        u128::from(self.nanos.load(Ordering::SeqCst))
    }
}

/// One caller-selected aggregate deadline policy.
///
/// The policy is chosen once by the host that constructs the provider. Every
/// provider operation starts a fresh [`Deadline`] from it; the policy itself
/// never mutates, so one slow operation cannot shorten the next.
#[derive(Clone, Debug)]
pub struct DeadlinePolicy {
    budget: Duration,
    clock: Arc<dyn MonotonicClock>,
}

impl Default for DeadlinePolicy {
    fn default() -> Self {
        Self::new(DEFAULT_OPERATION_DEADLINE)
    }
}

impl DeadlinePolicy {
    /// Select an aggregate operation budget, clamped to
    /// [`MAX_OPERATION_DEADLINE`].
    #[must_use]
    pub fn new(budget: Duration) -> Self {
        Self::with_clock(budget, Arc::new(SystemClock::new()))
    }

    /// Select an aggregate operation budget over an explicit clock.
    #[must_use]
    pub fn with_clock(budget: Duration, clock: Arc<dyn MonotonicClock>) -> Self {
        Self {
            budget: budget.min(MAX_OPERATION_DEADLINE),
            clock,
        }
    }

    /// The clamped budget every operation receives.
    #[must_use]
    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// Start one operation's deadline.
    #[must_use]
    pub fn start(&self) -> Deadline {
        Deadline {
            clock: Arc::clone(&self.clock),
            expires_at_nanos: self.clock.elapsed_nanos() + self.budget.as_nanos(),
        }
    }
}

/// One in-flight aggregate deadline.
///
/// Every sub-operation asks for the *remaining* duration. Nothing restarts the
/// full budget, so several failing candidate addresses, several partial
/// writes, or several interrupted reads all draw down the same total.
#[derive(Clone, Debug)]
pub struct Deadline {
    clock: Arc<dyn MonotonicClock>,
    expires_at_nanos: u128,
}

impl Deadline {
    /// The duration left, or `None` once the deadline has passed.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        let now = self.clock.elapsed_nanos();
        let left = self.expires_at_nanos.checked_sub(now)?;
        if left == 0 {
            return None;
        }
        Some(Duration::from_nanos(
            u64::try_from(left).unwrap_or(u64::MAX),
        ))
    }

    /// Whether the deadline has passed.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.remaining().is_none()
    }

    /// The remaining duration as a per-syscall timeout, never zero, or `None`
    /// once the aggregate deadline is spent.
    #[must_use]
    pub fn slice(&self) -> Option<Duration> {
        Some(self.remaining()?.max(MIN_SYSCALL_SLICE))
    }

    /// The remaining duration further capped by a caller-chosen bound, such as
    /// a program's own `net_wait` timeout.
    #[must_use]
    pub fn slice_capped(&self, cap: Duration) -> Option<Duration> {
        Some(self.remaining()?.min(cap).max(MIN_SYSCALL_SLICE))
    }
}

/// The `std::io::Error` a spent deadline produces. It is deliberately
/// `TimedOut`, so callers classify an exhausted aggregate exactly as they
/// classify an exhausted per-syscall timeout.
pub(crate) fn deadline_expired() -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "aggregate network operation deadline expired",
    )
}

/// A borrowed socket that re-derives its per-syscall timeout from an aggregate
/// deadline before *every* read and write.
///
/// This is what makes a multi-syscall sub-operation — a TLS handshake, a
/// partial write, a record that arrives one byte at a time — honour the
/// operation deadline rather than paying a fresh timeout per syscall.
pub(crate) struct DeadlineSocket<'a> {
    socket: &'a mut TcpStream,
    deadline: &'a Deadline,
}

impl<'a> DeadlineSocket<'a> {
    pub(crate) fn new(socket: &'a mut TcpStream, deadline: &'a Deadline) -> Self {
        Self { socket, deadline }
    }
}

impl Read for DeadlineSocket<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let slice = self.deadline.slice().ok_or_else(deadline_expired)?;
        self.socket.set_read_timeout(Some(slice))?;
        self.socket.read(buffer)
    }
}

impl Write for DeadlineSocket<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let slice = self.deadline.slice().ok_or_else(deadline_expired)?;
        self.socket.set_write_timeout(Some(slice))?;
        self.socket.write(bytes)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let slice = self.deadline.slice().ok_or_else(deadline_expired)?;
        self.socket.set_write_timeout(Some(slice))?;
        self.socket.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_caller_budget_is_clamped_to_the_fixed_maximum() {
        let policy = DeadlinePolicy::new(Duration::from_secs(3_600));
        assert_eq!(policy.budget(), MAX_OPERATION_DEADLINE);
        let policy = DeadlinePolicy::new(Duration::from_millis(250));
        assert_eq!(policy.budget(), Duration::from_millis(250));
        assert_eq!(
            DeadlinePolicy::default().budget(),
            DEFAULT_OPERATION_DEADLINE
        );
    }

    #[test]
    fn remaining_shrinks_and_never_restarts() {
        let clock = Arc::new(ScriptedClock::new());
        let policy = DeadlinePolicy::with_clock(Duration::from_millis(100), clock.clone());
        let deadline = policy.start();
        assert_eq!(deadline.remaining(), Some(Duration::from_millis(100)));
        clock.advance(Duration::from_millis(60));
        assert_eq!(deadline.remaining(), Some(Duration::from_millis(40)));
        clock.advance(Duration::from_millis(30));
        assert_eq!(deadline.remaining(), Some(Duration::from_millis(10)));
        clock.advance(Duration::from_millis(10));
        assert_eq!(deadline.remaining(), None);
        assert!(deadline.expired());
        assert_eq!(deadline.slice(), None);
        // A later operation gets its own full budget; the spent one does not
        // recover.
        assert_eq!(policy.start().remaining(), Some(Duration::from_millis(100)));
        assert!(deadline.expired());
    }

    #[test]
    fn a_syscall_slice_is_never_zero_and_respects_a_caller_cap() {
        let clock = Arc::new(ScriptedClock::new());
        let policy = DeadlinePolicy::with_clock(Duration::from_millis(10), clock.clone());
        let deadline = policy.start();
        assert_eq!(
            deadline.slice_capped(Duration::from_millis(5)),
            Some(Duration::from_millis(5))
        );
        assert_eq!(
            deadline.slice_capped(Duration::from_secs(30)),
            Some(Duration::from_millis(10))
        );
        clock.advance(Duration::from_micros(9_999));
        assert_eq!(deadline.slice(), Some(MIN_SYSCALL_SLICE));
        assert_eq!(
            deadline.slice_capped(Duration::ZERO),
            Some(MIN_SYSCALL_SLICE)
        );
        clock.advance(Duration::from_micros(1));
        assert_eq!(deadline.slice(), None);
        assert_eq!(deadline.slice_capped(Duration::from_secs(1)), None);
    }

    #[test]
    fn an_expired_deadline_refuses_socket_io_without_a_syscall() {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = std::thread::spawn(move || listener.accept().unwrap());
        let mut socket = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let peer = accepted.join().unwrap();
        let clock = Arc::new(ScriptedClock::new());
        let deadline = DeadlinePolicy::with_clock(Duration::from_millis(5), clock.clone()).start();
        clock.advance(Duration::from_millis(5));
        let mut bounded = DeadlineSocket::new(&mut socket, &deadline);
        let write = bounded.write(b"ping").unwrap_err();
        assert_eq!(write.kind(), std::io::ErrorKind::TimedOut);
        let read = bounded.read(&mut [0u8; 4]).unwrap_err();
        assert_eq!(read.kind(), std::io::ErrorKind::TimedOut);
        drop(peer);
    }
}
