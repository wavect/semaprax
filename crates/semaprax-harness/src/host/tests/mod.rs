//! Real-adapter tests: every case drives a python stdio adapter from
//! `packages/semaprax-harness-adapters/examples` (or a tiny fixture adapter
//! written here) through the host. `Grant::issue` is crate-private, so these
//! live inside the crate.

mod hostile;
mod isolation_tests;
mod lifecycle_tests;
mod ma_lane_m_tests;
mod model_tests;
mod net;

use super::grant::{Grant, GrantedPermissions};
use super::*;
use crate::contract::{CapabilityKind, CapabilityRef, Descriptor, ProjectBinding, RequestEnvelope};
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

static N: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct Fx {
    pub root: PathBuf,
    pub project: PathBuf,
    pub cache: PathBuf,
    pub retention: PathBuf,
}

/// Fresh canonical fixture tree `<tmp>/hp-hp03-<pid>-<n>/{project,cache,retention}`.
pub(crate) fn fixture() -> Fx {
    let n = N.fetch_add(1, Ordering::SeqCst);
    let root = std::env::temp_dir().join(format!("hp-hp03-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for d in ["project/src", "cache", "retention"] {
        std::fs::create_dir_all(root.join(d)).unwrap();
    }
    let root = root.canonicalize().unwrap();
    std::fs::write(
        root.join("project/src/lib.rs"),
        "pub fn alpha_symbol() {}\n",
    )
    .unwrap();
    Fx {
        project: root.join("project"),
        cache: root.join("cache"),
        retention: root.join("retention"),
        root,
    }
}

/// `python3` resolved once through `/usr/bin/which`, then passed explicitly.
pub(crate) fn python() -> PathBuf {
    static P: OnceLock<PathBuf> = OnceLock::new();
    P.get_or_init(|| {
        let out = std::process::Command::new("/usr/bin/which")
            .arg("python3")
            .output()
            .expect("which");
        let p = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        assert!(p.is_absolute(), "python3 not found");
        p
    })
    .clone()
}

pub(crate) fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/semaprax-harness-adapters/examples")
        .canonicalize()
        .unwrap()
}

pub(crate) fn descriptor_from(dir: &Path, edit: impl FnOnce(&mut Value)) -> Descriptor {
    let mut v: Value =
        serde_json::from_slice(&std::fs::read(dir.join("harness-provider.json")).unwrap()).unwrap();
    edit(&mut v);
    Descriptor::parse(v.to_string().as_bytes()).unwrap()
}

/// Spec for an adapter directory containing `adapter.py`.
pub(crate) fn spec_in(
    dir: &Path,
    d: Descriptor,
    fx: &Fx,
    env: &[(&str, &str)],
    isolation: IsolationRequest,
) -> LaunchSpec {
    let entry = sha256_plain(&std::fs::read(dir.join(&d.entry[0])).unwrap());
    let perms = GrantedPermissions {
        read: vec!["project".into()],
        write: vec!["cache".into()],
        ..Default::default()
    };
    let grant = Grant::issue(
        d.provider_id.clone(),
        d.digest().to_string(),
        Some(entry),
        None,
        perms,
    );
    LaunchSpec {
        descriptor: d,
        descriptor_dir: dir.to_path_buf(),
        runtime_executable: Some(python()),
        upstream_executable: None,
        grant,
        project_root: fx.project.clone(),
        cache_dir: fx.cache.clone(),
        retention_dir: fx.retention.clone(),
        isolation,
        forward_env: env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

pub(crate) fn hostile(fx: &Fx, mode: &str) -> LaunchSpec {
    let dir = examples().join("hostile-python");
    let d = descriptor_from(&dir, |_| {});
    let pidfile = fx.root.join("pids");
    spec_in(
        &dir,
        d,
        fx,
        &[
            ("HOSTILE_MODE", mode),
            ("HOSTILE_PIDFILE", pidfile.to_str().unwrap()),
        ],
        IsolationRequest::None,
    )
}

/// Source-index example copied into the fixture (it emits `sha256:<hex>`
/// digests, the form the HP-01 validator requires).
pub(crate) fn source_index(fx: &Fx) -> LaunchSpec {
    let src = examples().join("source-index-python");
    let dir = fx.root.join("examples/source-index-python");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(fx.root.join("sdk/python")).unwrap();
    std::fs::copy(
        examples().join("../sdk/python/semaprax_harness_adapter.py"),
        fx.root.join("sdk/python/semaprax_harness_adapter.py"),
    )
    .unwrap();
    for f in ["adapter.py", "harness-provider.json", "index.py"] {
        std::fs::copy(src.join(f), dir.join(f)).unwrap();
    }
    spec_in(
        &dir,
        descriptor_from(&dir, |_| {}),
        fx,
        &[],
        IsolationRequest::None,
    )
}

pub(crate) const PROJECT: &str = "p1";

pub(crate) fn request(kind: CapabilityKind, op: &str, payload: Value, id: &str) -> RequestEnvelope {
    RequestEnvelope {
        invocation_id: id.into(),
        project: ProjectBinding {
            id: PROJECT.into(),
            worktree: "w1".into(),
            revision: "r1".into(),
        },
        lock_digest: "sha256:0000000000000000000000000000000000000000000000000000000000000001"
            .into(),
        capability: CapabilityRef { kind, version: 1 },
        operation: op.into(),
        deadline_ms: 30_000,
        max_result_bytes: 1 << 20,
        remaining_calls: 8,
        lineage: vec![],
        payload,
    }
}

pub(crate) fn decide(id: &str) -> RequestEnvelope {
    request(
        CapabilityKind::DecisionEvaluate,
        "evaluate",
        json!({"task": "model-route/v1", "features": {}, "options": ["a", "b"]}),
        id,
    )
}

pub(crate) fn search(id: &str, q: &str) -> RequestEnvelope {
    request(
        CapabilityKind::ContextRepository,
        "search",
        json!({"query": q}),
        id,
    )
}

pub(crate) fn mgr(edit: impl FnOnce(&mut HostConfig)) -> AdapterManager {
    let mut c = HostConfig::default();
    edit(&mut c);
    AdapterManager::new(c)
}

pub(crate) fn pid_alive(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

/// Poll until `pid` is gone (grandchildren are reaped asynchronously by init).
pub(crate) fn assert_gone(pid: i32) {
    let end = Instant::now() + Duration::from_secs(5);
    while pid_alive(pid) && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!pid_alive(pid), "pid {pid} survived settlement");
}

/// `(adapter pid, grandchild pid)` written by hostile `ignore_cancel`.
pub(crate) fn wait_pids(fx: &Fx) -> (i32, i32) {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(s) = std::fs::read_to_string(fx.root.join("pids")) {
            let v: Vec<i32> = s
                .split_whitespace()
                .filter_map(|x| x.parse().ok())
                .collect();
            if v.len() == 2 {
                return (v[0], v[1]);
            }
        }
        assert!(Instant::now() < end, "hostile pidfile never appeared");
        std::thread::sleep(Duration::from_millis(20));
    }
}
