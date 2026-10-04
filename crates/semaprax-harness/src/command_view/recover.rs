//! Bounded raw recovery from host retention: never re-executes a command.

use super::policy::Policy;
use super::retention::{valid_handle, Retention, StreamName};
use crate::cli::Environment;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use std::path::Path;

/// Largest single recovery read.
pub const MAX_RECOVER: u64 = 4 << 20;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recovered {
    pub text: String,
    pub bytes: u64,
    pub stream_total: u64,
    pub next_offset: Option<u64>,
}

/// Read `limit` bytes of one retained stream from `offset`.
pub fn recover(
    env: &Environment,
    project: &Path,
    handle: &str,
    stream: StreamName,
    offset: u64,
    limit: u64,
) -> HarnessResult<Recovered> {
    let bad = |m: &str| HarnessDiagnostic::new("SPX-HPH031", m.to_string());
    let root = env
        .cwd
        .join(project)
        .canonicalize()
        .map_err(|e| bad(&format!("project path: {e}")))?;
    let pid = sha256_plain(root.to_string_lossy().as_bytes());
    recover_by_id(env, &pid, handle, stream, offset, limit)
}

/// Like `recover`, for a project that no longer exists on disk (a scratch tree).
pub fn recover_by_id(
    env: &Environment,
    pid: &str,
    handle: &str,
    stream: StreamName,
    offset: u64,
    limit: u64,
) -> HarnessResult<Recovered> {
    let bad = |m: &str| HarnessDiagnostic::new("SPX-HPH031", m.to_string());
    let policy = Policy::load(env)?;
    let rp = policy
        .retention
        .ok_or_else(|| bad("retention is not enabled by policy"))?;
    let home = env
        .harness_home
        .as_ref()
        .ok_or_else(|| bad("no harness home"))?;
    if !valid_handle(handle) {
        return Err(HarnessDiagnostic::new(
            "SPX-HPH040",
            "recovery handle must look like `cv-<24 hex>`",
        ));
    }
    let r = Retention::open(home, pid, &rp)?;
    let (bytes, total) = r.read(handle, stream, offset, limit.min(MAX_RECOVER))?;
    let (text, _) = super::guard::decode(&bytes);
    let next = offset + bytes.len() as u64;
    Ok(Recovered {
        text,
        bytes: bytes.len() as u64,
        stream_total: total,
        next_offset: (next < total).then_some(next),
    })
}
