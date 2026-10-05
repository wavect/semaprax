//! Bridge lifecycle extension (HN-18): `bridge/invoke` runs one provider
//! invocation through the adapter host while the session keeps reading frames;
//! `bridge/cancel {id}` reaches it by JSON-RPC id. Cancellation reuses the host
//! (`CancelToken`, group kill, reap, retry boundary); nothing here stops a
//! process itself. Specification: `docs/HARNESS-BRIDGE-V1.md`.

use crate::cli::Environment;
use crate::contract::{
    CapabilityKind, CapabilityRef, ProjectBinding, RequestEnvelope, ResultStatus,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::{
    AdapterManager, CancelToken, HostConfig, InvocationClass, IsolationMode, IsolationRequest,
    LaunchSpec, NetworkPolicy, Outcome,
};
use crate::profile::resolve::BindingState;
use crate::workflow::journal::Journal;
use serde_json::{json, Value};
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

/// Concurrent in-flight invocations per session; further requests are refused.
pub const MAX_IN_FLIGHT: usize = 4;
/// Settled ids remembered so a delayed `bridge/cancel` gets the real answer.
const SETTLED_KEPT: usize = 256;

fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// What the host can say about a cancellation. `confirmed-terminated` only when
/// the owned process group is gone (or the request was never written);
/// `uncertain-external-effect` when a side-effecting request was sent and its
/// effect cannot be known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CancelState {
    CancelRequested,
    ConfirmedTerminated,
    UncertainExternalEffect,
}

impl CancelState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CancelRequested => "cancel-requested",
            Self::ConfirmedTerminated => "confirmed-terminated",
            Self::UncertainExternalEffect => "uncertain-external-effect",
        }
    }
}

#[derive(Default)]
struct Inner {
    live: BTreeMap<String, CancelToken>,
    /// Final state per settled id: a cancel state or `completed`.
    settled: BTreeMap<String, &'static str>,
    order: VecDeque<String>,
}

/// In-flight table: bounded, keyed by the canonical JSON-RPC id.
#[derive(Default)]
pub struct Registry(Mutex<Inner>);

impl Registry {
    pub fn begin(&self, key: &str, token: &CancelToken) -> HarnessResult<()> {
        let mut g = lock(&self.0);
        if g.live.contains_key(key) {
            return Err(diag(
                "SPX-HPN013",
                format!("request id {key} is already in flight"),
            ));
        }
        if g.live.len() >= MAX_IN_FLIGHT {
            return Err(diag(
                "SPX-HPN012",
                format!(
                    "{MAX_IN_FLIGHT} invocations are already in flight; retry after one settles"
                ),
            ));
        }
        g.settled.remove(key);
        g.live.insert(key.to_string(), token.clone());
        Ok(())
    }

    pub fn settle(&self, key: &str, state: &'static str) {
        let mut g = lock(&self.0);
        g.live.remove(key);
        g.settled.insert(key.to_string(), state);
        g.order.retain(|k| k != key);
        g.order.push_back(key.to_string());
        while g.order.len() > SETTLED_KEPT {
            if let Some(old) = g.order.pop_front() {
                g.settled.remove(&old);
            }
        }
    }

    /// Answer `bridge/cancel {id}`. A live id is only *requested*: the invoke's
    /// own response carries the final state. Unknown ids are never remembered,
    /// so a stray cancel cannot poison a later request that reuses the id.
    pub fn cancel(&self, key: &str) -> Value {
        let g = lock(&self.0);
        if let Some(t) = g.live.get(key) {
            t.cancel();
            return json!({"id": key, "state": CancelState::CancelRequested.as_str(), "cancelled": false});
        }
        match g.settled.get(key) {
            Some(&s) => json!({"id": key, "state": s, "cancelled": s == "confirmed-terminated"}),
            None => json!({"id": key, "state": "unknown-id", "cancelled": false}),
        }
    }

    /// Session end or client crash: request cancellation of everything live.
    pub fn cancel_all(&self) {
        for t in lock(&self.0).live.values() {
            t.cancel();
        }
    }

    pub fn live(&self) -> usize {
        lock(&self.0).live.len()
    }
}

/// Provider invocation for one bridge session.
pub struct Invoker {
    env: Environment,
    project: PathBuf,
    manager: AdapterManager,
    counter: AtomicU64,
    journal: Mutex<Option<Journal>>,
    pub registry: Registry,
}

fn class_of(kind: CapabilityKind) -> InvocationClass {
    match kind {
        // Possibly billed generation: never retried or replayed after a send.
        CapabilityKind::ModelGenerate => InvocationClass::SideEffecting,
        CapabilityKind::DecisionEvaluate => InvocationClass::Decision,
        _ => InvocationClass::SafeRead,
    }
}

fn isolation_json(mode: IsolationMode) -> Value {
    match mode {
        IsolationMode::Subprocess => {
            json!({"mode": "subprocess", "isolated": false, "note": "plain subprocess; no OS enforcement"})
        }
        IsolationMode::OsEnforced { mechanism } => {
            json!({"mode": "os-enforced", "isolated": true, "mechanism": mechanism})
        }
    }
}

impl Invoker {
    pub fn new(env: &Environment, project: &Path, config: HostConfig) -> Self {
        Self {
            env: env.clone(),
            project: project.to_path_buf(),
            manager: AdapterManager::new(config),
            counter: AtomicU64::new(0),
            journal: Mutex::new(None),
            registry: Registry::default(),
        }
    }

    fn project_id(&self) -> String {
        let p = self.project.canonicalize().unwrap_or(self.project.clone());
        crate::skills::cli_defaults::project_id(&p)
    }

    /// Run `bridge/invoke`. The returned value is the response `result`; a
    /// refusal is an error frame. `token` is the one `bridge/cancel` trips.
    pub fn run(&self, params: &Value, token: &CancelToken) -> HarnessResult<Value> {
        let obj = params
            .as_object()
            .ok_or_else(|| diag("SPX-HPN005", "params must be an object"))?;
        if let Some(k) = obj.keys().find(|k| {
            !matches!(
                k.as_str(),
                "capability" | "operation" | "payload" | "deadline_ms" | "step" | "isolation"
            )
        }) {
            return Err(diag("SPX-HPN005", format!("unknown parameter `{k}`")));
        }
        let kind = obj
            .get("capability")
            .and_then(Value::as_str)
            .and_then(CapabilityKind::parse)
            .ok_or_else(|| diag("SPX-HPN005", "`capability` must name a capability kind"))?;
        let operation = obj
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| diag("SPX-HPN005", "`operation` must be a string"))?
            .to_string();
        let payload = obj.get("payload").cloned().unwrap_or_else(|| json!({}));
        let deadline_ms = obj
            .get("deadline_ms")
            .and_then(Value::as_u64)
            .unwrap_or(30_000);
        let required_isolation = match obj.get("isolation").and_then(Value::as_str) {
            None => false,
            Some("required") => true,
            Some(o) => {
                return Err(diag(
                    "SPX-HPN005",
                    format!("`isolation` must be `required`, not `{o}`"),
                ))
            }
        };
        let class = class_of(kind);

        let res = crate::profile::resolve_project(&self.env, &self.project)?;
        let selected = res
            .profile
            .binding(kind)
            .is_some_and(|b| b.state == BindingState::Selected);
        let launch = res.launches.get(&kind).filter(|_| selected).ok_or_else(|| {
            diag(
                "SPX-HPN014",
                format!("no trusted provider is selected for {}; the builtin fallback has no bridge invocation", kind.as_str()),
            )
        })?;
        crate::profile::check_grant_current(&self.env, &launch.grant)?;
        let home = self.env.harness_home.clone().ok_or_else(|| {
            diag(
                "SPX-HPN014",
                "no harness home: cannot place the provider cache",
            )
        })?;
        let desc = &launch.descriptor;
        let runtime_executable = crate::profile::runtime::pick(
            desc.runtime,
            None,
            launch.runtime.as_deref(),
            None,
            &self.env,
        );
        let project_id = self.project_id();
        let canon = self.project.canonicalize().unwrap_or(self.project.clone());
        let tag = format!("{}-bridge", desc.provider_id.replace('/', "_"));
        let isolation = if required_isolation {
            IsolationRequest::Restricted {
                allow_read: vec![canon.clone()],
                allow_write: vec![],
                network: NetworkPolicy::Deny,
            }
        } else {
            IsolationRequest::None
        };
        let spec = LaunchSpec {
            descriptor: desc.clone(),
            descriptor_dir: launch
                .descriptor_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            runtime_executable,
            upstream_executable: launch.upstream_path.clone(),
            grant: launch.grant.clone(),
            project_root: canon,
            cache_dir: home.join("cache").join("adapters").join(&tag),
            retention_dir: home.join("retention").join(&tag),
            isolation,
            forward_env: Default::default(),
        };
        // A required-isolation request the host cannot enforce is refused here
        // (SPX-HPC003); it is never downgraded to a plain subprocess.
        let handle = self.manager.prepare(&project_id, spec)?;
        let step = obj
            .get("step")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{}:{}", kind.as_str(), operation));
        if class == InvocationClass::SideEffecting {
            self.journal_begin(&home, &project_id, &step)?;
        }
        let n = self.counter.fetch_add(1, Ordering::SeqCst) + 1;
        let req = RequestEnvelope {
            invocation_id: format!("inv-bridge-{n:06}"),
            project: ProjectBinding {
                id: project_id.clone(),
                worktree: project_id,
                revision: "bridge".into(),
            },
            lock_digest: res.profile.lock_digest(),
            capability: CapabilityRef { kind, version: 1 },
            operation,
            deadline_ms: deadline_ms.clamp(1, 600_000),
            max_result_bytes: 1 << 20,
            remaining_calls: 8,
            lineage: vec![],
            payload,
        };
        let out = handle.invoke(&req, class, token);
        let cancelled = token.is_cancelled();
        let iso = isolation_json(handle.isolation_mode());
        self.settle_journal(&step, class, &out, cancelled);
        Ok(describe(&out, class, cancelled, iso))
    }

    fn journal_begin(&self, home: &Path, project_id: &str, step: &str) -> HarnessResult<()> {
        let mut g = lock(&self.journal);
        if g.is_none() {
            *g = Some(Journal::open(
                &home.join("cache").join("bridge"),
                project_id,
            )?);
        }
        let j = g.as_mut().expect("journal opened");
        if !j.may_run(step) {
            return Err(diag(
                "SPX-HPN015",
                format!("step `{step}` already ran or has an uncertain external effect; it is not retried"),
            ));
        }
        j.append(step, "begin", json!({}))
    }

    fn settle_journal(&self, step: &str, class: InvocationClass, out: &Outcome, cancelled: bool) {
        if class != InvocationClass::SideEffecting {
            return;
        }
        let mut g = lock(&self.journal);
        let Some(j) = g.as_mut() else { return };
        let _ = match out {
            Outcome::Completed(_) if !cancelled => j.append(step, "done", json!({})),
            Outcome::Refused(d) if !cancelled => j.append(step, "refused", json!({"code": d.code})),
            // Cancelled before the request was written: provably never ran.
            Outcome::Cancelled => j.append(step, "cancelled", json!({})),
            _ => j.record_uncertain(
                step,
                if cancelled {
                    "cancel"
                } else {
                    "outcome-unknown"
                },
            ),
        };
    }

    /// Stop adapters idle past their `idle_shutdown_ms` as of `now` (MA-10). The
    /// serving session drives this from its maintenance tick; the host keeps
    /// in-flight work alive and restarts a reaped adapter lazily.
    pub(crate) fn reap_idle(&self, now: std::time::Instant) -> usize {
        self.manager.reap_idle(now).len()
    }
}

/// Result frame body. A result that arrives after cancellation was requested
/// is discarded, never returned as current.
fn describe(out: &Outcome, class: InvocationClass, cancelled: bool, iso: Value) -> Value {
    let side = class == InvocationClass::SideEffecting;
    if cancelled {
        let (state, discarded) = match out {
            Outcome::Uncertain(_) => (CancelState::UncertainExternalEffect, false),
            Outcome::Completed(_) => (
                if side {
                    CancelState::UncertainExternalEffect
                } else {
                    CancelState::ConfirmedTerminated
                },
                true,
            ),
            Outcome::Unavailable {
                request_sent: true, ..
            } if side => (CancelState::UncertainExternalEffect, false),
            _ => (CancelState::ConfirmedTerminated, false),
        };
        return json!({"state": state.as_str(), "cancelled": state == CancelState::ConfirmedTerminated,
            "result_discarded": discarded, "retried": false, "isolation": iso});
    }
    match out {
        Outcome::Completed(r) => json!({"state": "completed", "status": r.status.as_str(),
            "result": r.to_json(), "complete": r.status == ResultStatus::Complete, "isolation": iso}),
        Outcome::Cancelled => {
            json!({"state": CancelState::ConfirmedTerminated.as_str(), "cancelled": true, "isolation": iso})
        }
        Outcome::Uncertain(d) => {
            json!({"state": CancelState::UncertainExternalEffect.as_str(), "cancelled": false,
            "code": d.code, "message": d.message, "retried": false, "isolation": iso})
        }
        Outcome::Refused(d) | Outcome::Quarantined(d) => {
            json!({"state": "refused", "code": d.code, "message": d.message, "isolation": iso})
        }
        Outcome::Unavailable {
            reason,
            request_sent,
            ..
        } => json!({"state": "unavailable", "code": reason.code,
            "message": reason.message, "request_sent": request_sent, "retried": false, "isolation": iso}),
    }
}

/// Final state recorded in the registry for a response.
pub fn settled_state(v: &Value) -> &'static str {
    match v["state"].as_str() {
        Some("confirmed-terminated") => "confirmed-terminated",
        Some("uncertain-external-effect") => "uncertain-external-effect",
        _ => "completed",
    }
}
