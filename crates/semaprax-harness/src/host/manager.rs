//! Adapter manager: one handle per `(project id, provider id)`, lazy start,
//! bounded concurrency and queue, deadlines, cancellation, idle shutdown, a
//! crash circuit breaker and the retry boundary.

use super::budget::{BudgetLedger, HostBudget};
use super::isolation::{IsolationBackend, IsolationMode};
use super::launch::LaunchSpec;
use super::lifecycle::{AdapterState, CancelToken, InvocationClass, Outcome};
use super::process::{Closed, Delivery, Proc};
use super::rpc;
use crate::contract::{
    negotiate, ActiveCapability, CancellationMode, HostSupport, ProjectBinding, RequestEnvelope,
    ResultEnvelope,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::canonical;
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Host-wide knobs. Descriptor bounds can only tighten these.
#[derive(Clone, Debug)]
pub struct HostConfig {
    pub backend: IsolationBackend,
    pub budget: HostBudget,
    pub host_max_concurrency: u32,
    pub max_queue: usize,
    /// Consecutive crashes/hangs before the adapter is quarantined.
    pub crash_threshold: u32,
    pub cancel_grace_ms: u64,
    pub shutdown_grace_ms: u64,
    pub stderr_ring_bytes: usize,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            backend: IsolationBackend::detect(),
            budget: HostBudget::default(),
            host_max_concurrency: 4,
            max_queue: 8,
            crash_threshold: 3,
            cancel_grace_ms: 500,
            shutdown_grace_ms: 1000,
            stderr_ring_bytes: 64 * 1024,
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Frame cap: descriptor `max_frame_bytes`, never above the 4 MiB host cap.
fn frame_cap(spec: &LaunchSpec) -> usize {
    spec.descriptor
        .resources
        .max_frame_bytes
        .min(4 * 1024 * 1024)
}

#[derive(Default)]
struct Gate {
    in_flight: u32,
    waiting: usize,
    closing: bool,
    /// The one absolute shutdown deadline. No business invocation is
    /// dispatched once it has passed.
    close_by: Option<Instant>,
}

struct Core {
    state: AdapterState,
    proc: Option<Arc<Proc>>,
    accepted: Vec<ActiveCapability>,
    crashes: u32,
    last_used: Instant,
    mode: IsolationMode,
    last_stderr: (String, u64),
}

pub struct AdapterHandle {
    project_id: String,
    spec: LaunchSpec,
    config: Arc<HostConfig>,
    ledger: Arc<Mutex<BudgetLedger>>,
    offered: Vec<ActiveCapability>,
    limit: u32,
    gate: Mutex<Gate>,
    cv: Condvar,
    start: Mutex<()>,
    /// Shutdown-owned stop condition. Startup observes it while waiting for
    /// the start gate and the handshake; it is never a caller's token.
    halt: CancelToken,
    core: Mutex<Core>,
    /// `harness/invoke` frames queued to an adapter (test and audit probe).
    invoke_frames: AtomicU64,
}

enum Wait {
    Got(Delivery),
    Timeout,
    Cancelled,
    /// The handle began shutting down (only for waits that observe `halt`).
    Halted,
}

fn wait(rx: &Receiver<Delivery>, until: Instant, cancel: &CancelToken) -> Wait {
    wait_or_halt(rx, until, cancel, None)
}

fn wait_or_halt(
    rx: &Receiver<Delivery>,
    until: Instant,
    cancel: &CancelToken,
    halt: Option<&CancelToken>,
) -> Wait {
    loop {
        if cancel.is_cancelled() {
            return Wait::Cancelled;
        }
        if halt.is_some_and(CancelToken::is_cancelled) {
            return Wait::Halted;
        }
        let now = Instant::now();
        if now >= until {
            return Wait::Timeout;
        }
        match rx.recv_timeout((until - now).min(Duration::from_millis(10))) {
            Ok(d) => return Wait::Got(d),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => return Wait::Got(Delivery::Closed(Closed::Exited)),
        }
    }
}

impl AdapterHandle {
    pub fn state(&self) -> AdapterState {
        lock(&self.core).state.clone()
    }
    pub fn provider_id(&self) -> &str {
        &self.spec.descriptor.provider_id
    }
    pub fn project_id(&self) -> &str {
        &self.project_id
    }
    /// `Subprocess` unless an OS-enforced restriction was applied.
    pub fn isolation_mode(&self) -> IsolationMode {
        lock(&self.core).mode
    }
    pub fn last_used(&self) -> Instant {
        lock(&self.core).last_used
    }
    pub fn consecutive_crashes(&self) -> u32 {
        lock(&self.core).crashes
    }
    /// Bounded stderr tail of the current or last process and dropped bytes.
    pub fn stderr_tail(&self) -> (String, u64) {
        let c = lock(&self.core);
        c.proc
            .as_ref()
            .map_or_else(|| c.last_stderr.clone(), |p| p.stderr_tail())
    }
    /// Contract versions of `kind` negotiated for this handle: the adapter's
    /// accepted set once it has initialized, else what the host offered.
    pub fn negotiated_versions(&self, kind: crate::contract::CapabilityKind) -> Vec<u32> {
        let c = lock(&self.core);
        let from = if c.accepted.is_empty() {
            &self.offered
        } else {
            &c.accepted
        };
        from.iter()
            .filter(|a| a.kind == kind)
            .map(|a| a.version)
            .collect()
    }
    /// Number of `harness/invoke` frames this handle has queued so far.
    pub fn invoke_frames_queued(&self) -> u64 {
        self.invoke_frames.load(Ordering::SeqCst)
    }
    /// Pid of the live adapter process (also its process-group id).
    pub fn pid(&self) -> Option<i32> {
        lock(&self.core).proc.as_ref().map(|p| p.pid())
    }

    /// Write a live (non-terminal) state. A handle that is draining or closed
    /// is never revived by a late startup, cancel or deadline path.
    fn set_state(&self, s: AdapterState) {
        let mut c = lock(&self.core);
        if !matches!(c.state, AdapterState::Draining | AdapterState::Closed) {
            c.state = s;
        }
    }

    /// Refusal for work that met a closing handle before anything was sent.
    fn closed_refusal(msg: &'static str) -> Outcome {
        Outcome::Refused(diag("SPX-HPC021", msg))
    }

    /// Whether the shutdown deadline has passed (no dispatch after it).
    fn past_close_by(&self) -> bool {
        lock(&self.gate)
            .close_by
            .is_some_and(|d| Instant::now() >= d)
    }

    #[cfg(test)]
    pub(crate) fn gate_counts(&self) -> (u32, usize) {
        let g = lock(&self.gate);
        (g.in_flight, g.waiting)
    }

    /// Terminate and forget the process, keeping its stderr tail.
    fn drop_proc(&self, proc: &Arc<Proc>, why: Closed) {
        proc.terminate(why);
        let mut c = lock(&self.core);
        if c.proc.as_ref().is_some_and(|p| Arc::ptr_eq(p, proc)) {
            c.last_stderr = proc.stderr_tail();
            c.proc = None;
        }
    }

    fn quarantine(&self, proc: &Arc<Proc>, d: HarnessDiagnostic) {
        self.drop_proc(proc, Closed::Violation(d.clone()));
        lock(&self.core).state = AdapterState::Quarantined(d);
    }

    /// A crash/hang: count it; open the breaker at the threshold.
    ///
    /// Charged once per process generation: only the caller that still finds
    /// `proc` installed in `core` (checked and cleared under the same lock)
    /// counts it. Every other waiter of the same exit, and any delayed waiter
    /// of an older generation, finds a different or absent process and leaves
    /// the shared state alone.
    fn record_failure(&self, proc: &Arc<Proc>, d: HarnessDiagnostic) {
        proc.terminate(Closed::Host("failed"));
        let mut c = lock(&self.core);
        if !c.proc.as_ref().is_some_and(|p| Arc::ptr_eq(p, proc)) {
            return;
        }
        c.last_stderr = proc.stderr_tail();
        c.proc = None;
        c.crashes += 1;
        if c.crashes < self.config.crash_threshold
            && matches!(c.state, AdapterState::Draining | AdapterState::Closed)
        {
            return;
        }
        c.state = if c.crashes >= self.config.crash_threshold {
            AdapterState::Quarantined(diag(
                "SPX-HPC016",
                format!(
                    "crash circuit breaker open after {} consecutive failures; last: {}",
                    c.crashes, d.message
                ),
            ))
        } else {
            AdapterState::Unavailable(d)
        };
    }

    /// Pre-dispatch refusal: nothing was written to the adapter, so the
    /// caller may retry or fall back.
    fn pre_dispatch_stop(until: Instant, cancel: &CancelToken) -> Option<Outcome> {
        if cancel.is_cancelled() {
            Some(Outcome::Cancelled)
        } else if Instant::now() >= until {
            Some(Outcome::Unavailable {
                reason: diag(
                    "SPX-HPC008",
                    "invocation deadline elapsed before dispatch; nothing was sent",
                ),
                request_sent: false,
                fallback_allowed: true,
            })
        } else {
            None
        }
    }

    fn try_start(&self) -> Option<MutexGuard<'_, ()>> {
        match self.start.try_lock() {
            Ok(g) => Some(g),
            Err(std::sync::TryLockError::Poisoned(p)) => Some(p.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) => None,
        }
    }

    /// Wait for start ownership, honouring cancellation, the deadline and
    /// handle shutdown (a queued waiter never outlives a closing handle).
    // Outcome is the intentionally rich terminal value returned verbatim to callers.
    #[allow(clippy::result_large_err)]
    fn lock_start(
        &self,
        until: Instant,
        cancel: &CancelToken,
    ) -> Result<MutexGuard<'_, ()>, Outcome> {
        loop {
            if self.halt.is_cancelled() {
                return Err(Self::closed_refusal("adapter handle is closing"));
            }
            if let Some(g) = self.try_start() {
                return Ok(g);
            }
            if let Some(o) = Self::pre_dispatch_stop(until, cancel) {
                return Err(o);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Start ownership for shutdown, bounded by its absolute deadline.
    fn lock_start_by(&self, deadline: Instant) -> Option<MutexGuard<'_, ()>> {
        loop {
            if let Some(g) = self.try_start() {
                return Some(g);
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    // Outcome is the intentionally rich terminal value returned verbatim to callers.
    #[allow(clippy::result_large_err)]
    fn ensure_running(
        &self,
        project: &ProjectBinding,
        cancel: &CancelToken,
        deadline: Instant,
    ) -> Result<Arc<Proc>, Outcome> {
        let _start = self.lock_start(deadline, cancel)?;
        if let Some(o) = Self::pre_dispatch_stop(deadline, cancel) {
            return Err(o);
        }
        if self.halt.is_cancelled() {
            return Err(Self::closed_refusal("adapter handle is closing"));
        }
        {
            let c = lock(&self.core);
            match &c.state {
                AdapterState::Quarantined(d) => return Err(Outcome::Quarantined(d.clone())),
                AdapterState::Closed | AdapterState::Draining => {
                    return Err(Outcome::Refused(diag(
                        "SPX-HPC021",
                        "adapter handle is closed",
                    )))
                }
                _ => {}
            }
            if let Some(p) = &c.proc {
                if p.closed().is_none() {
                    return Ok(p.clone());
                }
            }
        }
        let stale = lock(&self.core).proc.clone();
        if let Some(p) = stale {
            self.drop_proc(&p, Closed::Host("restart"));
        }
        let prepared = self
            .spec
            .prepare(&self.config.backend)
            .map_err(Outcome::Refused)?;
        let proc = Arc::new(
            Proc::spawn(
                &prepared,
                frame_cap(&self.spec),
                self.config.stderr_ring_bytes,
            )
            .map_err(|e| Outcome::Unavailable {
                reason: diag("SPX-HPC004", format!("cannot start adapter: {e}")),
                request_sent: false,
                fallback_allowed: true,
            })?,
        );
        {
            // Publish the new generation only while the handle is open; the
            // halt check and the publish share the core lock that shutdown
            // reads the current generation under.
            let mut c = lock(&self.core);
            if self.halt.is_cancelled() {
                drop(c);
                proc.terminate(Closed::Host("shutdown"));
                return Err(Self::closed_refusal(
                    "adapter handle closed during startup; nothing was sent",
                ));
            }
            c.mode = prepared.mode;
            c.proc = Some(proc.clone());
        }
        let unavailable = |d: HarnessDiagnostic| Outcome::Unavailable {
            reason: d,
            request_sent: false,
            fallback_allowed: true,
        };
        let rx = proc
            .request(
                "harness/initialize",
                rpc::initialize_params(&self.spec.descriptor, &self.offered, project),
            )
            .map_err(|_| unavailable(diag("SPX-HPC007", "adapter exited before the handshake")))?;
        let hs_until = Instant::now()
            + Duration::from_millis(self.spec.descriptor.resources.handshake_timeout_ms);
        // The handshake is bounded by its own cap and the invocation deadline.
        let until = hs_until.min(deadline);
        let got = wait_or_halt(&rx, until, cancel, Some(&self.halt));
        // Shutdown owns the outcome of an interrupted startup: a late reply,
        // a host stop or the halt itself all end in the same closed refusal.
        if self.halt.is_cancelled()
            && !matches!(got, Wait::Got(Delivery::Closed(Closed::Violation(_))))
        {
            self.drop_proc(&proc, Closed::Host("shutdown"));
            return Err(Self::closed_refusal(
                "adapter handle closed during startup; nothing was sent",
            ));
        }
        match got {
            Wait::Got(Delivery::Result(v)) => match rpc::parse_initialize(&v, &self.offered) {
                Ok(acc) => {
                    let mut c = lock(&self.core);
                    // Commit only into an open handle (rechecked under the lock
                    // shutdown publishes `Draining` under).
                    if self.halt.is_cancelled()
                        || matches!(c.state, AdapterState::Draining | AdapterState::Closed)
                    {
                        drop(c);
                        self.drop_proc(&proc, Closed::Host("shutdown"));
                        return Err(Self::closed_refusal(
                            "adapter handle closed during startup; nothing was sent",
                        ));
                    }
                    c.accepted = acc;
                    c.state = AdapterState::Negotiated;
                    Ok(proc)
                }
                Err(d) => {
                    self.quarantine(&proc, d.clone());
                    Err(Outcome::Quarantined(d))
                }
            },
            Wait::Got(Delivery::Closed(Closed::Violation(d))) => {
                self.quarantine(&proc, d.clone());
                Err(Outcome::Quarantined(d))
            }
            Wait::Got(Delivery::Error(m)) => {
                let d = diag("SPX-HPC006", format!("adapter declined initialize: {m}"));
                self.record_failure(&proc, d.clone());
                Err(unavailable(d))
            }
            Wait::Got(Delivery::Closed(_)) => {
                let d = diag("SPX-HPC007", "adapter exited during the handshake");
                self.record_failure(&proc, d.clone());
                Err(unavailable(d))
            }
            Wait::Timeout if deadline < hs_until => {
                // The invocation ran out of time, not the adapter: no crash is
                // counted and nothing was dispatched.
                self.drop_proc(&proc, Closed::Host("deadline"));
                self.set_state(AdapterState::Prepared);
                Err(Self::pre_dispatch_stop(deadline, &CancelToken::new())
                    .expect("deadline elapsed"))
            }
            Wait::Timeout => {
                let d = diag(
                    "SPX-HPC005",
                    format!(
                        "handshake exceeded {} ms; adapter killed",
                        self.spec.descriptor.resources.handshake_timeout_ms
                    ),
                );
                self.record_failure(&proc, d.clone());
                Err(unavailable(d))
            }
            Wait::Cancelled => {
                self.drop_proc(&proc, Closed::Host("cancel"));
                self.set_state(AdapterState::Prepared);
                Err(Outcome::Cancelled)
            }
            Wait::Halted => unreachable!("handled above"),
        }
    }

    /// Take an in-flight slot, queueing (bounded) until `until` or cancel.
    // Outcome is the intentionally rich terminal value returned verbatim to callers.
    #[allow(clippy::result_large_err)]
    fn admit(&self, until: Instant, cancel: &CancelToken) -> Result<(), Outcome> {
        let mut g = lock(&self.gate);
        if g.closing {
            return Err(Outcome::Refused(diag(
                "SPX-HPC021",
                "adapter handle is closing",
            )));
        }
        if let Some(o) = Self::pre_dispatch_stop(until, cancel) {
            return Err(o);
        }
        if g.in_flight >= self.limit {
            if g.waiting >= self.config.max_queue {
                return Err(Outcome::Refused(diag(
                    "SPX-HPC017",
                    "adapter queue is full",
                )));
            }
            g.waiting += 1;
            while g.in_flight >= self.limit {
                if g.closing {
                    g.waiting -= 1;
                    return Err(Self::closed_refusal("adapter handle is closing"));
                }
                let now = Instant::now();
                if cancel.is_cancelled() || now >= until {
                    g.waiting -= 1;
                    return Err(if cancel.is_cancelled() {
                        Outcome::Cancelled
                    } else {
                        Outcome::Unavailable {
                            reason: diag("SPX-HPC017", "timed out waiting in the adapter queue"),
                            request_sent: false,
                            fallback_allowed: true,
                        }
                    });
                }
                g = self
                    .cv
                    .wait_timeout(g, (until - now).min(Duration::from_millis(10)))
                    .unwrap_or_else(|p| p.into_inner())
                    .0;
            }
            g.waiting -= 1;
        }
        g.in_flight += 1;
        Ok(())
    }

    fn release(&self) {
        lock(&self.gate).in_flight -= 1;
        self.cv.notify_all();
    }

    /// Run one invocation. Never retries; the caller owns fallback and may use
    /// it only as `Outcome` permits.
    pub fn invoke(
        &self,
        req: &RequestEnvelope,
        class: InvocationClass,
        cancel: &CancelToken,
    ) -> Outcome {
        if req.project.id != self.project_id {
            return Outcome::Refused(diag(
                "SPX-HPC020",
                "request is bound to a different project than this adapter handle",
            ));
        }
        if let Err(d) = req.validate() {
            return Outcome::Refused(d);
        }
        let res = &self.spec.descriptor.resources;
        let started = Instant::now();
        let until = started + Duration::from_millis(req.deadline_ms.min(res.invoke_timeout_ms));
        if let AdapterState::Quarantined(d) = self.state() {
            return Outcome::Quarantined(d);
        }
        if let Err(d) = lock(&self.ledger).start_job(&self.config.budget) {
            return Outcome::Refused(d);
        }
        let out = match self.admit(until, cancel) {
            Err(o) => o,
            Ok(()) => {
                let o = self.run(req, class, cancel, until);
                self.release();
                o
            }
        };
        let bytes = match &out {
            Outcome::Completed(r) => canonical(&r.to_json()).len() as u64,
            _ => 0,
        };
        lock(&self.ledger).finish_job(started.elapsed().as_millis() as u64, bytes);
        lock(&self.core).last_used = Instant::now();
        out
    }

    fn run(
        &self,
        req: &RequestEnvelope,
        class: InvocationClass,
        cancel: &CancelToken,
        until: Instant,
    ) -> Outcome {
        let proc = match self.ensure_running(&req.project, cancel, until) {
            Ok(p) => p,
            Err(o) => return o,
        };
        let accepted = lock(&self.core).accepted.iter().any(|a| {
            a.kind == req.capability.kind
                && a.version == req.capability.version
                && a.operations.contains(&req.operation)
        });
        if !accepted {
            return Outcome::Refused(diag(
                "SPX-HPC020",
                format!(
                    "adapter did not accept {} v{} operation `{}`",
                    req.capability.kind.as_str(),
                    req.capability.version,
                    req.operation
                ),
            ));
        }
        {
            let mut c = lock(&self.core);
            if c.state == AdapterState::Negotiated {
                c.state = AdapterState::Active;
            }
        }
        // Last host check before the side-effect boundary.
        if let Some(o) = Self::pre_dispatch_stop(until, cancel) {
            return o;
        }
        if self.past_close_by() {
            return Self::closed_refusal(
                "adapter handle shutdown deadline passed before dispatch; nothing was sent",
            );
        }
        self.invoke_frames.fetch_add(1, Ordering::SeqCst);
        let rx = match proc.request("harness/invoke", req.to_json()) {
            Ok(rx) => rx,
            Err(c) => return self.closed_outcome(&proc, c, class, false),
        };
        let mut got = wait(&rx, until, cancel);
        if matches!(got, Wait::Cancelled) {
            proc.notify(
                "harness/cancel",
                json!({"invocation_id": req.invocation_id}),
            );
            if self.spec.descriptor.cancellation == CancellationMode::Cooperative {
                let grace = (Instant::now() + Duration::from_millis(self.config.cancel_grace_ms))
                    .min(until);
                if let Wait::Got(d) = wait(&rx, grace, &CancelToken::new()) {
                    got = Wait::Got(d);
                }
            }
            if matches!(got, Wait::Cancelled) {
                self.drop_proc(&proc, Closed::Host("cancel"));
                self.set_state(AdapterState::Prepared);
                return if class.may_fall_back() {
                    Outcome::Cancelled
                } else {
                    Outcome::Uncertain(diag(
                        "SPX-HPC018",
                        "side-effecting invocation cancelled after it was sent; outcome unknown",
                    ))
                };
            }
        }
        match got {
            Wait::Got(Delivery::Result(v)) => self.accept_result(&proc, req, class, &v),
            // The request was dispatched: an adapter error does not prove it
            // never ran, so side-effecting work stays non-replayable.
            Wait::Got(Delivery::Error(m)) => {
                let d = diag("SPX-HPC024", format!("adapter returned an error: {m}"));
                Self::post_dispatch_refusal(class, d)
            }
            Wait::Got(Delivery::Closed(c)) => self.closed_outcome(&proc, c, class, true),
            Wait::Timeout => {
                let d = diag(
                    "SPX-HPC008",
                    format!(
                        "invocation `{}` exceeded its deadline; process group killed",
                        req.invocation_id
                    ),
                );
                self.record_failure(&proc, d.clone());
                self.fail(class, true, d)
            }
            Wait::Cancelled | Wait::Halted => unreachable!("handled above"),
        }
    }

    fn accept_result(
        &self,
        proc: &Arc<Proc>,
        req: &RequestEnvelope,
        class: InvocationClass,
        v: &serde_json::Value,
    ) -> Outcome {
        match ResultEnvelope::parse_for(req, canonical(v).as_bytes()) {
            Ok(env) => {
                lock(&self.core).crashes = 0;
                Outcome::Completed(env)
            }
            Err(d) => {
                // Binding/shape breaches (spoofing, authority members, strict-JSON
                // refusals) quarantine; payload-level refusals discard only the result.
                let n: u32 = d.code.trim_start_matches("SPX-HPA").parse().unwrap_or(0);
                if (1..=9).contains(&n) || matches!(n, 31 | 32 | 33 | 36 | 37) {
                    let q = diag(
                        "SPX-HPC015",
                        format!(
                            "result violates its envelope binding ({}): {}",
                            d.code, d.message
                        ),
                    );
                    self.quarantine(proc, q.clone());
                    Outcome::Quarantined(q)
                } else {
                    Self::post_dispatch_refusal(class, d)
                }
            }
        }
    }

    /// A refusal after `harness/invoke` was written. Only safe classes may
    /// treat it as a plain refusal; a side-effecting step may have executed.
    fn post_dispatch_refusal(class: InvocationClass, d: HarnessDiagnostic) -> Outcome {
        if class.may_fall_back() {
            Outcome::Refused(d)
        } else {
            Outcome::Uncertain(d)
        }
    }

    fn closed_outcome(
        &self,
        proc: &Arc<Proc>,
        c: Closed,
        class: InvocationClass,
        sent: bool,
    ) -> Outcome {
        match c {
            Closed::Violation(d) => {
                self.quarantine(proc, d.clone());
                Outcome::Quarantined(d)
            }
            Closed::Exited => {
                let d = diag("SPX-HPC007", "adapter exited without answering");
                self.record_failure(proc, d.clone());
                self.fail(class, sent, d)
            }
            Closed::Host(why) => {
                let d = diag(
                    "SPX-HPC007",
                    format!("adapter was stopped by the host ({why}) before answering"),
                );
                self.fail(class, sent, d)
            }
        }
    }

    /// The retry boundary: after a request was written with no response, only
    /// safe classes may fall back; side-effecting work is `Uncertain`.
    fn fail(&self, class: InvocationClass, sent: bool, reason: HarnessDiagnostic) -> Outcome {
        if sent && !class.may_fall_back() {
            Outcome::Uncertain(reason)
        } else {
            Outcome::Unavailable {
                reason,
                request_sent: sent,
                fallback_allowed: true,
            }
        }
    }

    /// Graceful stop of one process: `harness/shutdown` with a reply wait
    /// bounded by `until`, then the group is killed regardless so no
    /// grandchild survives. Past `until` it goes straight to the kill.
    fn stop(&self, proc: &Arc<Proc>, until: Instant) {
        if proc.closed().is_none() && Instant::now() < until {
            if let Ok(rx) = proc.request("harness/shutdown", json!({})) {
                let _ = rx.recv_timeout(until.saturating_duration_since(Instant::now()));
            }
        }
        self.drop_proc(proc, Closed::Host("shutdown"));
    }

    /// True once `shutdown` has completed on a handle that was not
    /// quarantined. A closed handle never serves another invocation.
    pub fn is_closed(&self) -> bool {
        self.state() == AdapterState::Closed
    }

    /// Drain in-flight work, stop the adapter, close the handle.
    ///
    /// One absolute deadline (`shutdown_grace_ms` from the first call) bounds
    /// the drain, the wait for start ownership and the graceful
    /// `harness/shutdown` reply. A startup in progress observes the halt and
    /// abandons its handshake without dispatching anything. After the
    /// deadline only the forced cleanup remains: a process-group `SIGKILL`
    /// and reaping the killed child, bounded by the OS rather than by a
    /// promise of hard real-time termination.
    ///
    /// The handle stays closed permanently; `AdapterManager::prepare` replaces
    /// a closed entry with a fresh handle and `AdapterManager::close_and_evict`
    /// / `reprepare` do so explicitly.
    pub fn shutdown(&self) {
        let deadline = {
            let mut g = lock(&self.gate);
            g.closing = true;
            let d = Instant::now() + Duration::from_millis(self.config.shutdown_grace_ms);
            *g.close_by.get_or_insert(d)
        };
        self.halt.cancel();
        self.cv.notify_all();
        {
            let mut c = lock(&self.core);
            if !matches!(c.state, AdapterState::Quarantined(_) | AdapterState::Closed) {
                c.state = AdapterState::Draining;
            }
        }
        let mut g = lock(&self.gate);
        while g.in_flight > 0 && Instant::now() < deadline {
            g = self
                .cv
                .wait_timeout(g, Duration::from_millis(10))
                .unwrap_or_else(|p| p.into_inner())
                .0;
        }
        drop(g);
        // Bounded: a startup owner that still holds the gate is stopped below
        // through the generation it published, never waited on past `deadline`.
        let start = self.lock_start_by(deadline);
        // The current generation, read after the halt: no startup can publish
        // a newer one now, so nothing captured earlier can be stale.
        let current = lock(&self.core).proc.clone();
        if let Some(p) = current {
            let until = if start.is_some() {
                deadline
            } else {
                Instant::now()
            };
            self.stop(&p, until);
        }
        drop(start);
        let mut c = lock(&self.core);
        if !matches!(c.state, AdapterState::Quarantined(_)) {
            c.state = AdapterState::Closed;
        }
    }

    /// Stop the adapter if it has been idle for `idle_shutdown_ms` as of `now`
    /// (injected, so tests need no sleeping). Returns whether it stopped; the
    /// next invocation restarts lazily.
    pub fn reap_idle(&self, now: Instant) -> bool {
        let idle = Duration::from_millis(self.spec.descriptor.resources.idle_shutdown_ms);
        let _start = lock(&self.start);
        if lock(&self.gate).in_flight > 0 {
            return false;
        }
        let proc = {
            let c = lock(&self.core);
            if now.saturating_duration_since(c.last_used) < idle {
                return false;
            }
            c.proc.clone()
        };
        let Some(p) = proc else { return false };
        self.stop(
            &p,
            Instant::now() + Duration::from_millis(self.config.shutdown_grace_ms),
        );
        let mut c = lock(&self.core);
        if matches!(c.state, AdapterState::Negotiated | AdapterState::Active) {
            c.state = AdapterState::Prepared;
        }
        true
    }
}

/// Registry of handles keyed by `(project id, provider id)`; each project gets
/// its own adapter process. Budgets are shared across the manager.
pub struct AdapterManager {
    config: Arc<HostConfig>,
    ledger: Arc<Mutex<BudgetLedger>>,
    handles: Mutex<BTreeMap<(String, String), Arc<AdapterHandle>>>,
}

impl AdapterManager {
    pub fn new(config: HostConfig) -> Self {
        Self {
            config: Arc::new(config),
            ledger: Arc::default(),
            handles: Mutex::default(),
        }
    }

    /// Same-key identity check for an entry that is still bound.
    fn same_identity(
        h: &Arc<AdapterHandle>,
        spec: &LaunchSpec,
    ) -> HarnessResult<Arc<AdapterHandle>> {
        if h.spec.descriptor.digest() != spec.descriptor.digest() {
            return Err(diag(
                "SPX-HPC001",
                "a different descriptor is already bound to this (project, provider)",
            ));
        }
        if let Some(what) = h.spec.launch_difference(spec) {
            return Err(diag(
                "SPX-HPC001",
                format!(
                    "this (project, provider) is already bound to a handle with a different launch identity ({what}); \
                     close it first with AdapterHandle::shutdown (a closed entry is then replaced by prepare) \
                     or AdapterManager::reprepare"
                ),
            ));
        }
        Ok(h.clone())
    }

    /// Validate the launch fully and build an unregistered `Prepared` handle.
    fn build(&self, project_id: &str, spec: LaunchSpec) -> HarnessResult<Arc<AdapterHandle>> {
        let prepared = spec.prepare(&self.config.backend)?;
        let negotiation = negotiate(&spec.descriptor, &HostSupport::first_wave())?;
        if negotiation.active.is_empty() {
            return Err(diag(
                "SPX-HPC023",
                "no declared capability is negotiable with this host",
            ));
        }
        let limit = spec
            .descriptor
            .resources
            .max_concurrency
            .min(self.config.host_max_concurrency)
            .max(1);
        Ok(Arc::new(AdapterHandle {
            project_id: project_id.to_string(),
            config: self.config.clone(),
            ledger: self.ledger.clone(),
            offered: negotiation.active,
            limit,
            gate: Mutex::default(),
            cv: Condvar::new(),
            start: Mutex::new(()),
            halt: CancelToken::new(),
            invoke_frames: AtomicU64::new(0),
            core: Mutex::new(Core {
                state: AdapterState::Prepared,
                proc: None,
                accepted: Vec::new(),
                crashes: 0,
                last_used: Instant::now(),
                mode: prepared.mode,
                last_stderr: (String::new(), 0),
            }),
            spec,
        }))
    }

    /// Verify the launch (grant, digests, isolation) and register the handle
    /// in state `Prepared`. Nothing is started until the first invocation. A
    /// second call for the same key returns the existing handle if the full
    /// launch identity is unchanged. An entry whose handle has been closed
    /// (`AdapterHandle::shutdown` completed) is replaced by a fresh handle; a
    /// quarantined entry is never replaced here (use `close_and_evict` or
    /// `reprepare`).
    pub fn prepare(&self, project_id: &str, spec: LaunchSpec) -> HarnessResult<Arc<AdapterHandle>> {
        let key = (project_id.to_string(), spec.descriptor.provider_id.clone());
        let mut handles = lock(&self.handles);
        if let Some(h) = handles.get(&key) {
            if !h.is_closed() {
                return Self::same_identity(h, &spec);
            }
        }
        let h = self.build(project_id, spec)?;
        handles.insert(key, h.clone());
        Ok(h)
    }

    /// Explicitly close the handle bound to the key and drop it from the
    /// registry; the old `Arc` stays permanently closed. Returns whether an
    /// entry existed. The slow shutdown runs without the registry lock; the
    /// entry stays registered (and so blocks a second owner) until the old
    /// process is stopped. This is also the explicit recovery path for a
    /// quarantined adapter.
    pub fn close_and_evict(&self, project_id: &str, provider_id: &str) -> bool {
        let key = (project_id.to_string(), provider_id.to_string());
        let Some(old) = lock(&self.handles).get(&key).cloned() else {
            return false;
        };
        old.shutdown();
        let mut handles = lock(&self.handles);
        if handles.get(&key).is_some_and(|h| Arc::ptr_eq(h, &old)) {
            handles.remove(&key);
        }
        true
    }

    /// Validate `spec` in full, then close and replace the entry for its key
    /// (whatever its identity or state, including quarantine) with a fresh
    /// `Prepared` handle. A failed validation leaves the existing handle
    /// untouched. If a concurrent caller already installed a replacement, that
    /// handle is returned when its identity matches `spec` and refused
    /// otherwise, so two active owners can never coexist.
    pub fn reprepare(
        &self,
        project_id: &str,
        spec: LaunchSpec,
    ) -> HarnessResult<Arc<AdapterHandle>> {
        let key = (project_id.to_string(), spec.descriptor.provider_id.clone());
        let fresh = self.build(project_id, spec)?;
        let old = lock(&self.handles).get(&key).cloned();
        if let Some(o) = &old {
            o.shutdown();
        }
        let mut handles = lock(&self.handles);
        if let Some(cur) = handles.get(&key) {
            let replaced = old.as_ref().is_some_and(|o| Arc::ptr_eq(o, cur));
            if !replaced && !cur.is_closed() {
                return Self::same_identity(cur, &fresh.spec);
            }
        }
        handles.insert(key, fresh.clone());
        Ok(fresh)
    }

    pub fn handle(&self, project_id: &str, provider_id: &str) -> Option<Arc<AdapterHandle>> {
        lock(&self.handles)
            .get(&(project_id.to_string(), provider_id.to_string()))
            .cloned()
    }

    /// Reap every idle adapter as of `now`; returns the keys stopped.
    pub fn reap_idle(&self, now: Instant) -> Vec<(String, String)> {
        let all: Vec<_> = lock(&self.handles)
            .iter()
            .map(|(k, h)| (k.clone(), h.clone()))
            .collect();
        all.into_iter()
            .filter(|(_, h)| h.reap_idle(now))
            .map(|(k, _)| k)
            .collect()
    }

    pub fn shutdown_all(&self) {
        let all: Vec<_> = lock(&self.handles).values().cloned().collect();
        for h in all {
            h.shutdown();
        }
    }

    pub fn budget_used(&self) -> BudgetLedger {
        lock(&self.ledger).clone()
    }
}

impl Drop for AdapterManager {
    fn drop(&mut self) {
        self.shutdown_all();
    }
}
