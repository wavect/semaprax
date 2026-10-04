//! Launch rig shared by every suite: a throwaway harness home, the descriptor
//! adopted and trusted through the real profile verbs, and a real
//! [`AdapterManager`]. Nothing global is read or written.

use super::report::{fail_with, Fail};
use crate::cli::Environment;
use crate::contract::{
    CapabilityKind, CapabilityRef, Descriptor, ProjectBinding, RequestEnvelope, ResultEnvelope,
    Runtime,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::grant::{Grant, GrantedPermissions};
use crate::host::{
    AdapterHandle, AdapterManager, AdapterState, CancelToken, HostConfig, InvocationClass,
    IsolationRequest, LaunchSpec, NetworkPolicy, Outcome,
};
use crate::profile::adopt::{adopt, AdoptOptions};
use crate::profile::installations::{CurrentDigests, Inspected, LocalState};
use crate::profile::trust::{grant_for, requested_as_granted, TrustRecord};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const PROJECT_ID: &str = "conformance-project";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

pub fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// What is under test and how to launch it.
#[derive(Clone, Debug)]
pub struct Target {
    pub descriptor: Descriptor,
    pub path: PathBuf,
    pub runtime: Option<PathBuf>,
    pub upstream: Option<PathBuf>,
    pub forward_env: BTreeMap<String, String>,
    pub restricted: bool,
    pub tmp: PathBuf,
}

impl Target {
    pub fn new(
        path: &Path,
        runtime: Option<PathBuf>,
        upstream: Option<PathBuf>,
        forward_env: BTreeMap<String, String>,
        restricted: bool,
        tmp: PathBuf,
    ) -> HarnessResult<Self> {
        let path = path.canonicalize().map_err(|e| {
            diag(
                "SPX-HPP002",
                format!("cannot read descriptor {}: {e}", path.display()),
            )
        })?;
        let bytes = std::fs::read(&path)
            .map_err(|e| diag("SPX-HPP002", format!("cannot read descriptor: {e}")))?;
        let descriptor = Descriptor::parse(&bytes)?;
        if matches!(descriptor.runtime, Runtime::Node | Runtime::Python) {
            match &runtime {
                Some(r) if r.is_absolute() && r.is_file() => {}
                _ => {
                    return Err(diag(
                        "SPX-HPP002",
                        format!(
                            "a {} adapter needs `--runtime <absolute path of the interpreter>` or an adopted runtime (`semaprax harness setup`); PATH is never searched",
                            descriptor.runtime.as_str()
                        ),
                    ))
                }
            }
        }
        Ok(Self {
            descriptor,
            path,
            runtime,
            upstream,
            forward_env,
            restricted,
            tmp,
        })
    }

    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new("/"))
    }

    /// Operations the descriptor declares for `kind` at v1.
    pub fn ops(&self, kind: CapabilityKind) -> Vec<String> {
        declared_ops(&self.descriptor, kind)
    }
}

pub fn declared_ops(d: &Descriptor, kind: CapabilityKind) -> Vec<String> {
    d.capabilities
        .iter()
        .filter(|c| c.kind == Some(kind) && c.version == 1)
        .flat_map(|c| c.operations.clone())
        .collect()
}

/// Per-rig knobs.
type CfgEdit = Box<dyn FnOnce(&mut HostConfig)>;
type DescriptorEdit = Box<dyn Fn(&mut Value)>;

#[derive(Default)]
pub struct Setup {
    pub files: Vec<(String, Vec<u8>)>,
    pub env: Vec<(String, String)>,
    pub isolation: Option<IsolationRequest>,
    pub cfg: Option<CfgEdit>,
    /// Edit the descriptor JSON; the variant gets an in-memory grant.
    pub edit: Option<DescriptorEdit>,
}

impl Setup {
    pub fn files(mut self, files: &[(&str, &str)]) -> Self {
        self.files = files
            .iter()
            .map(|(p, c)| (p.to_string(), c.as_bytes().to_vec()))
            .collect();
        self
    }
    pub fn env(mut self, k: &str, v: &str) -> Self {
        self.env.push((k.into(), v.into()));
        self
    }
}

pub struct Rig {
    pub work: PathBuf,
    pub project: PathBuf,
    pub descriptor: Descriptor,
    pub handle: Arc<AdapterHandle>,
    pub mgr: AdapterManager,
    pub revision: u32,
    pub grant: Grant,
    pub upstream_version: Option<String>,
    pub requested_isolation: &'static str,
    seq: u32,
}

/// Grant stripped of network, process and secret classes: a conformance run
/// never grants ambient authority, whatever the descriptor requests.
fn strip_ambient(g: &Grant) -> Grant {
    let p = g.permissions();
    Grant::issue(
        g.provider_id().to_string(),
        g.descriptor_digest().to_string(),
        g.entry_digest().map(str::to_string),
        g.upstream_digest().map(str::to_string),
        GrantedPermissions {
            read: p.read.clone(),
            write: p.write.clone(),
            ..Default::default()
        },
    )
}

/// In-memory trust for an edited descriptor (never persisted).
pub fn variant_grant(insp: &Inspected, d: &Descriptor) -> HarnessResult<Grant> {
    let cur = CurrentDigests {
        descriptor_digest: d.digest().to_string(),
        requested: d.permissions.clone(),
        requires_upstream: false,
        upstream_digest: None,
        ..insp.current.clone()
    };
    let mut state = LocalState::default();
    state.trust.insert(
        d.provider_id.clone(),
        TrustRecord {
            descriptor_digest: cur.descriptor_digest.clone(),
            entry_digest: cur.entry_digest.clone(),
            upstream_digest: cur.upstream_digest.clone(),
            granted: requested_as_granted(&cur.requested),
        },
    );
    grant_for(&state, &d.provider_id, &cur)
}

impl Rig {
    pub fn open(t: &Target, files: &[(&str, &str)]) -> HarnessResult<Rig> {
        Self::open_with(t, Setup::default().files(files))
    }

    pub fn open_with(t: &Target, setup: Setup) -> HarnessResult<Rig> {
        let n = SERIAL.fetch_add(1, Ordering::SeqCst);
        let tmp = t.tmp.canonicalize().unwrap_or_else(|_| t.tmp.clone());
        let work = tmp.join(format!("hp-conformance-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&work);
        let io = |e: std::io::Error| diag("SPX-HPP003", format!("rig setup: {e}"));
        for d in ["home", "project", "cache", "retention"] {
            std::fs::create_dir_all(work.join(d)).map_err(io)?;
        }
        let work = work.canonicalize().map_err(io)?;
        let project = work.join("project");
        for (rel, bytes) in &setup.files {
            let p = project.join(rel);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).map_err(io)?;
            }
            std::fs::write(&p, bytes).map_err(io)?;
        }
        let built = Self::launch(t, setup, &work, &project);
        if built.is_err() {
            let _ = std::fs::remove_dir_all(&work);
        }
        built
    }

    fn launch(t: &Target, setup: Setup, work: &Path, project: &Path) -> HarnessResult<Rig> {
        let env = Environment {
            harness_home: Some(work.join("home")),
            compiler: None,
            cwd: work.to_path_buf(),
            vars: BTreeMap::new(),
        };
        let id = t.descriptor.provider_id.clone();
        let report = adopt(
            &env,
            &t.path,
            &AdoptOptions {
                upstream: t.upstream.clone(),
                project: project.to_path_buf(),
                allow_project_local: false,
            },
        )?;
        let trusted = crate::profile::cli_trust(std::slice::from_ref(&id), &env);
        // An upstream that is just the adapter's own bundled code (`local:`
        // package, no identity probe) is bound by the entry digest and is
        // trusted by the real verb; the in-memory fallback below only covers
        // an older trust store that still refuses it (SPX-HPB033).
        let bundled = t
            .descriptor
            .upstream
            .as_ref()
            .is_some_and(|u| u.package.starts_with("local:") && u.identity_probe.is_empty());
        if trusted.code != 0 && !(bundled && trusted.stderr.contains("SPX-HPB033")) {
            return Err(diag(
                "SPX-HPP003",
                format!("trust refused: {}", trusted.stderr.trim()),
            ));
        }
        let state = LocalState::load(&env)?;
        let inst = state
            .installations
            .get(&id)
            .ok_or_else(|| diag("SPX-HPP003", "adopted installation missing"))?;
        let insp = inst.inspect()?;
        let mut descriptor = insp.descriptor.clone();
        let mut grant = if trusted.code == 0 {
            grant_for(&state, &id, &insp.current)?
        } else {
            variant_grant(&insp, &descriptor)?
        };
        if let Some(edit) = &setup.edit {
            let mut v = descriptor.to_json();
            edit(&mut v);
            descriptor = Descriptor::parse(v.to_string().as_bytes())?;
            grant = variant_grant(&insp, &descriptor)?;
        }
        let grant = strip_ambient(&grant);
        let isolation = setup.isolation.clone().unwrap_or_else(|| {
            if t.restricted {
                IsolationRequest::Restricted {
                    allow_read: vec![project.to_path_buf()],
                    allow_write: vec![],
                    network: NetworkPolicy::Deny,
                }
            } else {
                IsolationRequest::None
            }
        });
        let requested_isolation = match isolation {
            IsolationRequest::None => "none",
            IsolationRequest::Restricted { .. } => "restricted",
        };
        let mut forward_env = t.forward_env.clone();
        forward_env.extend(setup.env.iter().cloned());
        let spec = LaunchSpec {
            descriptor: descriptor.clone(),
            descriptor_dir: t.dir().to_path_buf(),
            runtime_executable: t.runtime.clone(),
            upstream_executable: insp.upstream_path.clone(),
            grant: grant.clone(),
            project_root: project.to_path_buf(),
            cache_dir: work.join("cache"),
            retention_dir: work.join("retention"),
            isolation,
            forward_env,
        };
        let mut cfg = HostConfig::default();
        if let Some(f) = setup.cfg {
            f(&mut cfg);
        }
        let mgr = AdapterManager::new(cfg);
        let handle = mgr.prepare(PROJECT_ID, spec)?;
        Ok(Rig {
            work: work.to_path_buf(),
            project: project.to_path_buf(),
            descriptor,
            handle,
            mgr,
            revision: 1,
            grant,
            upstream_version: report.installation.upstream.and_then(|u| u.version),
            requested_isolation,
            seq: 0,
        })
    }

    pub fn write(&self, rel: &str, contents: &str) {
        let p = self.project.join(rel);
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(p, contents);
    }

    /// New snapshot revision for the next request (after a project edit).
    pub fn bump(&mut self) {
        self.revision += 1;
    }

    pub fn request(&mut self, kind: CapabilityKind, op: &str, payload: Value) -> RequestEnvelope {
        self.seq += 1;
        RequestEnvelope {
            invocation_id: format!("inv-{:06}", self.seq),
            project: ProjectBinding {
                id: PROJECT_ID.into(),
                worktree: "conformance-worktree".into(),
                revision: format!("rev-{}", self.revision),
            },
            lock_digest: format!("sha256:{}", "0".repeat(63) + "1"),
            capability: CapabilityRef { kind, version: 1 },
            operation: op.into(),
            deadline_ms: 30_000,
            max_result_bytes: 1 << 20,
            remaining_calls: 8,
            lineage: vec![],
            payload,
        }
    }

    pub fn run(&mut self, kind: CapabilityKind, op: &str, payload: Value) -> Outcome {
        let req = self.request(kind, op, payload);
        self.handle
            .invoke(&req, class_of(kind), &CancelToken::new())
    }

    pub fn isolation_name(&self) -> String {
        match self.handle.isolation_mode() {
            crate::host::IsolationMode::Subprocess => "subprocess".to_string(),
            crate::host::IsolationMode::OsEnforced { mechanism } => {
                format!("os-enforced:{mechanism}")
            }
        }
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        self.mgr.shutdown_all();
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

pub fn class_of(kind: CapabilityKind) -> InvocationClass {
    if kind == CapabilityKind::DecisionEvaluate {
        InvocationClass::Decision
    } else {
        InvocationClass::SafeRead
    }
}

/// Structured description of a non-completed outcome (no temp paths).
pub fn describe(o: &Outcome) -> Value {
    match o {
        Outcome::Completed(r) => json!({"outcome": "completed", "status": r.status.as_str()}),
        Outcome::Refused(d) => json!({"outcome": "refused", "code": d.code, "message": d.message}),
        Outcome::Quarantined(d) => {
            json!({"outcome": "quarantined", "code": d.code, "message": d.message})
        }
        Outcome::Unavailable {
            reason,
            request_sent,
            fallback_allowed,
        } => json!({
            "outcome": "unavailable", "code": reason.code, "message": reason.message,
            "request_sent": request_sent, "fallback_allowed": fallback_allowed}),
        Outcome::Cancelled => json!({"outcome": "cancelled"}),
        Outcome::Uncertain(d) => {
            json!({"outcome": "uncertain", "code": d.code, "message": d.message})
        }
    }
}

/// A completed result, or a failure naming what the host did instead.
pub fn completed(o: Outcome) -> Result<ResultEnvelope, Fail> {
    match o {
        Outcome::Completed(r) => Ok(r),
        other => Err(fail_with(
            "the host did not accept the adapter's result",
            describe(&other),
        )),
    }
}

/// Completed with a usable payload (status complete or partial).
pub fn usable(o: Outcome) -> Result<(ResultEnvelope, Value), Fail> {
    let r = completed(o)?;
    match (r.status.as_str(), r.payload.clone()) {
        ("complete" | "partial" | "stale", Some(p)) => Ok((r, p)),
        (s, _) => Err(fail_with(
            format!("adapter answered with status `{s}`"),
            json!({"status": s, "diagnostics": r.diagnostics}),
        )),
    }
}

/// Cooperative cancellation: cancel once the adapter is running. A conformant
/// adapter answers (a result) within the host's cancel grace; one that ignores
/// `harness/cancel` is group-killed by the host and fails here.
pub fn cancellation(
    rig: &mut Rig,
    kind: CapabilityKind,
    op: &str,
    payload: Value,
) -> Result<Value, Fail> {
    let req = rig.request(kind, op, payload);
    let token = CancelToken::new();
    let (t2, h2) = (token.clone(), rig.handle.clone());
    let done = Arc::new(AtomicBool::new(false));
    let d2 = done.clone();
    let waiter = std::thread::spawn(move || {
        let end = Instant::now() + Duration::from_secs(15);
        while !d2.load(Ordering::SeqCst) && Instant::now() < end {
            if h2.state() == AdapterState::Active {
                std::thread::sleep(Duration::from_millis(20));
                t2.cancel();
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    });
    let out = rig.handle.invoke(&req, class_of(kind), &token);
    done.store(true, Ordering::SeqCst);
    let _ = waiter.join();
    match out {
        Outcome::Completed(r) => {
            Ok(json!({"answered_within_grace": true, "status": r.status.as_str()}))
        }
        Outcome::Cancelled => Err(fail_with(
            "adapter ignored harness/cancel; the host had to kill its process group",
            json!({"outcome": "cancelled", "adapter_pid_gone": rig.handle.pid().is_none()}),
        )),
        other => Err(fail_with(
            "cancellation run did not complete cleanly",
            describe(&other),
        )),
    }
}

/// Minimal valid request for `kind`: `(operation, payload)`.
pub fn minimal(t: &Target, kind: CapabilityKind) -> Option<(String, Value)> {
    let ops = t.ops(kind);
    let has = |o: &str| ops.iter().any(|x| x == o);
    match kind {
        CapabilityKind::ContextRepository if has("search") => {
            Some(("search".into(), json!({"query": "conformance"})))
        }
        CapabilityKind::ContextRepository if has("references") => {
            Some(("references".into(), json!({"symbol": "conformance"})))
        }
        CapabilityKind::ContextRepository if has("orient") => {
            Some(("orient".into(), json!({"max_items": 4})))
        }
        CapabilityKind::CommandView if has("view") => Some((
            "view".into(),
            json!({"form": "post-execution", "argv": ["true"], "stdout": "ok\n", "stderr": ""}),
        )),
        CapabilityKind::DecisionEvaluate if has("evaluate") => Some((
            "evaluate".into(),
            json!({"task": "model-route/v1", "features": {"complexity": 0.7}, "options": ["cheap", "strong"]}),
        )),
        CapabilityKind::SkillCatalog if has("list") => Some(("list".into(), json!({"limit": 4}))),
        _ => None,
    }
}
