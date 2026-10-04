//! Lifecycle states, invocation classes, cancellation and invocation outcomes.

use crate::contract::ResultEnvelope;
use crate::diag::HarnessDiagnostic;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// `prepared -> negotiated -> active -> draining -> closed`, plus
/// `unavailable` (restartable failure) and `quarantined` (terminal for the
/// session). An idle shutdown returns to `Prepared`: the next call restarts
/// lazily. `Closed` is final.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterState {
    Prepared,
    Negotiated,
    Active,
    Draining,
    Closed,
    Unavailable(HarnessDiagnostic),
    Quarantined(HarnessDiagnostic),
}

impl AdapterState {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Negotiated => "negotiated",
            Self::Active => "active",
            Self::Draining => "draining",
            Self::Closed => "closed",
            Self::Unavailable(_) => "unavailable",
            Self::Quarantined(_) => "quarantined",
        }
    }
}

/// Retry boundary. Only `SafeRead` and `Decision` may fall back or retry after
/// a crash with no response received; `SideEffecting` never does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvocationClass {
    SafeRead,
    Decision,
    SideEffecting,
}

impl InvocationClass {
    pub fn may_fall_back(self) -> bool {
        !matches!(self, Self::SideEffecting)
    }
}

/// Cooperative cancellation flag; cancellation is a request, never proof the
/// adapter stopped (the host kills the process group after a grace period).
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Result of one invocation. `Completed` carries untrusted data that already
/// passed envelope binding and payload validation.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Validated result envelope (any status).
    Completed(ResultEnvelope),
    /// The adapter answered but its result was refused; payload discarded.
    Refused(HarnessDiagnostic),
    /// The adapter breached the protocol or binding; process group killed and
    /// the adapter is quarantined. The requested authority was not exercised.
    Quarantined(HarnessDiagnostic),
    /// No usable response. A caller may fall back only when `fallback_allowed`
    /// (safe class, or the request was never written).
    Unavailable {
        reason: HarnessDiagnostic,
        request_sent: bool,
        fallback_allowed: bool,
    },
    /// Cancelled before any response (safe class).
    Cancelled,
    /// A side-effecting request was sent and its outcome is unknown. Never
    /// retried automatically.
    Uncertain(HarnessDiagnostic),
}
