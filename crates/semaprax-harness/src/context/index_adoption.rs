//! Generic opt-in index adoption (HN-10), shared by index-backed providers.
//!
//! A provider that finds a user-owned index describes it as an [`IndexDescriptor`]
//! (provider, version, schema, canonical root and worktree, configuration digest,
//! indexed input digests, coverage, ownership mode). [`verify`] binds that
//! descriptor to the current working tree; only a clean verification lets the
//! index answer, read-only or as an immutable copied snapshot. A Git revision
//! never proves freshness: every indexed input is compared by content digest.
//! [`GenerationStore`] gives Semaprax-owned caches a single-flight refresh lock
//! and an atomic generation swap, so a concurrent reader sees a complete old or a
//! complete new generation. Diagnostics: `SPX-HPF001..006` (see [`Mismatch`]).

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::digest;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

pub const DESCRIPTOR_SCHEMA: &str = "semaprax.harness-index-adoption.v1";

/// Who may write the adopted index: nobody (read-only reuse) or only Semaprax
/// (an immutable copy made after validation). A user index is never overwritten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ownership {
    ReadOnly,
    CopiedSnapshot,
}

impl Ownership {
    pub fn as_str(self) -> &'static str {
        match self {
            Ownership::ReadOnly => "read-only",
            Ownership::CopiedSnapshot => "copied-snapshot",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "read-only" => Some(Ownership::ReadOnly),
            "copied-snapshot" => Some(Ownership::CopiedSnapshot),
            _ => None,
        }
    }
}

/// What actually happened, reported with the work done (never a bare "cache hit").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    ReusedUserIndex,
    CopiedValidatedIndex,
    IncrementalRefresh,
    Rebuilt,
    Incompatible,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Outcome::ReusedUserIndex => "reused-user-index",
            Outcome::CopiedValidatedIndex => "copied-validated-index",
            Outcome::IncrementalRefresh => "incremental-refresh",
            Outcome::Rebuilt => "rebuilt",
            Outcome::Incompatible => "incompatible",
        }
    }
}

/// Deterministic work counters; wall time is measured by callers, apart from construction.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Work {
    pub files_verified: u64,
    pub bytes_hashed: u64,
    pub bytes_copied: u64,
    pub files_indexed: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexDescriptor {
    pub provider: String,
    pub upstream_version: String,
    pub index_schema: String,
    /// Canonical (realpath) source root the index was built for.
    pub source_root: String,
    /// Worktree identity; two worktrees of one HEAD may differ in uncommitted content.
    pub worktree_id: String,
    /// Digest of parser/extractor identity and indexing configuration.
    pub config_digest: String,
    /// Project-relative path -> `sha256:` content digest of every indexed input.
    pub inputs: BTreeMap<String, String>,
    pub coverage_languages: Vec<String>,
    pub ownership: Ownership,
}

impl IndexDescriptor {
    pub fn inputs_digest(&self) -> String {
        digest(
            "semaprax.harness-index-adoption.inputs.v1",
            &json!(self.inputs),
        )
    }

    pub fn to_json(&self) -> Value {
        json!({"schema": DESCRIPTOR_SCHEMA, "provider": self.provider, "upstream_version": self.upstream_version,
               "index_schema": self.index_schema, "source_root": self.source_root, "worktree_id": self.worktree_id,
               "config_digest": self.config_digest, "inputs_digest": self.inputs_digest(),
               "coverage": {"languages": self.coverage_languages, "files": self.inputs.len()},
               "ownership": self.ownership.as_str()})
    }
}

/// The host's current view: what an index must agree with to be current evidence.
#[derive(Clone, Debug)]
pub struct Expected {
    pub provider: String,
    pub upstream_version: String,
    pub index_schema: String,
    pub source_root: String,
    pub worktree_id: String,
    pub config_digest: String,
    /// Indexable files now present and permitted: path -> content digest.
    pub tree: BTreeMap<String, String>,
    /// Paths the privacy policy excludes (ignored, secret, outside scope): never servable.
    pub excluded: BTreeSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mismatch {
    pub code: &'static str,
    pub message: String,
}

fn m(code: &'static str, message: String) -> Mismatch {
    Mismatch { code, message }
}

/// Verifies compatibility and source binding. Empty vector means the index may serve queries.
/// 001 provider/version/schema, 002 another root or worktree, 003 changed parser/config,
/// 004 private or excluded path in the index, 005 stale content (same size or not), 006 file missing from index.
pub fn verify(d: &IndexDescriptor, e: &Expected) -> Vec<Mismatch> {
    let mut out = Vec::new();
    if d.provider != e.provider
        || d.upstream_version != e.upstream_version
        || d.index_schema != e.index_schema
    {
        out.push(m(
            "SPX-HPF001",
            format!(
                "index is {} {} {}, expected {} {} {}",
                d.provider,
                d.upstream_version,
                d.index_schema,
                e.provider,
                e.upstream_version,
                e.index_schema
            ),
        ));
    }
    if d.source_root != e.source_root || d.worktree_id != e.worktree_id {
        out.push(m(
            "SPX-HPF002",
            format!(
                "index was built for {} ({}), this is {} ({})",
                d.source_root, d.worktree_id, e.source_root, e.worktree_id
            ),
        ));
    }
    if d.config_digest != e.config_digest {
        out.push(m(
            "SPX-HPF003",
            "parser or indexing configuration changed since the index was built".into(),
        ));
    }
    for (p, got) in &d.inputs {
        if e.excluded.contains(p) {
            out.push(m(
                "SPX-HPF004",
                format!("indexed path {p} is excluded by policy"),
            ));
        } else if let Some(want) = e.tree.get(p) {
            if want != got {
                out.push(m(
                    "SPX-HPF005",
                    format!("indexed path {p} differs from the working tree"),
                ));
            }
        } else {
            out.push(m(
                "SPX-HPF005",
                format!("indexed path {p} is absent from the working tree"),
            ));
        }
    }
    for p in e.tree.keys() {
        if !d.inputs.contains_key(p) {
            out.push(m(
                "SPX-HPF006",
                format!("working-tree file {p} is not in the index"),
            ));
        }
    }
    out
}

/// Maps the first mismatch to a diagnostic for callers that refuse instead of falling back.
pub fn first_diagnostic(mismatches: &[Mismatch]) -> Option<HarnessDiagnostic> {
    mismatches
        .first()
        .map(|x| HarnessDiagnostic::new(x.code, x.message.clone()))
}

// ---- single-flight refresh and atomic generations ------------------------------------

/// Semaprax-owned generations under `<root>/gen/<n>`, the live one named by `<root>/CURRENT`.
/// Readers follow CURRENT; a build happens in `gen/<n>.partial` under [`RefreshLock`] and becomes
/// visible only by renaming the directory and then CURRENT.
pub struct GenerationStore {
    root: PathBuf,
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

impl GenerationStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn gens(&self) -> PathBuf {
        self.root.join("gen")
    }

    /// Directory of the live generation, if a complete one is published.
    pub fn current(&self) -> Option<(u64, PathBuf)> {
        let name = std::fs::read_to_string(self.root.join("CURRENT")).ok()?;
        let n: u64 = name.trim().strip_prefix('g')?.parse().ok()?;
        let dir = self.gens().join(format!("g{n}"));
        dir.is_dir().then_some((n, dir))
    }

    /// Single-flight: at most one holder; a dead holder's lock is reclaimed. `Err` if the wait expires.
    pub fn lock(&self, wait: std::time::Duration) -> HarnessResult<RefreshLock> {
        std::fs::create_dir_all(&self.root).map_err(io)?;
        let lock = self.root.join("refresh.lock");
        let start = std::time::Instant::now();
        loop {
            match std::fs::create_dir(&lock) {
                Ok(()) => {
                    let _ = std::fs::write(lock.join("owner"), std::process::id().to_string());
                    return Ok(RefreshLock { dir: lock });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let pid = std::fs::read_to_string(lock.join("owner"))
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok());
                    if pid.is_some_and(|p| !pid_alive(p)) {
                        let _ = std::fs::remove_dir_all(&lock);
                        continue;
                    }
                    if start.elapsed() > wait {
                        return Err(HarnessDiagnostic::new(
                            "SPX-HPF010",
                            "timed out waiting for the index refresh lock",
                        ));
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                Err(e) => return Err(io(e)),
            }
        }
    }

    /// Starts the next generation in a fresh `.partial` directory. Hold the lock.
    pub fn begin(&self) -> HarnessResult<(u64, PathBuf)> {
        std::fs::create_dir_all(self.gens()).map_err(io)?;
        let next = std::fs::read_dir(self.gens())
            .map_err(io)?
            .flatten()
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_prefix('g')?
                    .split('.')
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
            .max()
            .unwrap_or(0)
            + 1;
        let dir = self.gens().join(format!("g{next}.partial"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(io)?;
        Ok((next, dir))
    }

    /// Makes generation `n` live atomically (directory rename, then CURRENT rename) and prunes all but
    /// the previous one, which an in-flight reader may still be using.
    pub fn publish(&self, n: u64) -> HarnessResult<PathBuf> {
        let fin = self.gens().join(format!("g{n}"));
        std::fs::rename(self.gens().join(format!("g{n}.partial")), &fin).map_err(io)?;
        let tmp = self.root.join(format!(
            "CURRENT.{}.{}.tmp",
            std::process::id(),
            TMP_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&tmp, format!("g{n}\n")).map_err(io)?;
        std::fs::rename(&tmp, self.root.join("CURRENT")).map_err(io)?;
        for e in std::fs::read_dir(self.gens()).map_err(io)?.flatten() {
            let old = e
                .file_name()
                .to_str()
                .and_then(|s| s.strip_prefix('g')?.split('.').next()?.parse::<u64>().ok());
            if old.is_some_and(|o| o + 1 < n) {
                let _ = std::fs::remove_dir_all(e.path());
            }
        }
        Ok(fin)
    }
}

pub struct RefreshLock {
    dir: PathBuf,
}

impl Drop for RefreshLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn io(e: std::io::Error) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPF011", format!("index generation I/O: {e}"))
}

fn pid_alive(pid: i32) -> bool {
    rustix::process::Pid::from_raw(pid)
        .is_some_and(|p| rustix::process::test_kill_process(p).is_ok())
}

/// Copies `from` into `<dest_root>/<digest>` read-only once; a present identical snapshot is reused
/// (returns bytes copied: 0). The source is never modified.
pub fn copy_snapshot(
    from: &Path,
    dest_root: &Path,
    snapshot_digest: &str,
) -> HarnessResult<(PathBuf, u64)> {
    let name = snapshot_digest.trim_start_matches("sha256:");
    let dest = dest_root.join(name);
    if dest.join(".complete").is_file() {
        return Ok((dest, 0));
    }
    let tmp = dest_root.join(format!(
        "{name}.partial-{}-{}",
        std::process::id(),
        TMP_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    let bytes = copy_dir(from, &tmp).map_err(io)?;
    std::fs::write(tmp.join(".complete"), b"").map_err(io)?;
    if std::fs::rename(&tmp, &dest).is_err() {
        let _ = std::fs::remove_dir_all(&tmp); // a concurrent copier won
    }
    Ok((dest, bytes))
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<u64> {
    std::fs::create_dir_all(to)?;
    let mut n = 0;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let (p, q) = (e.path(), to.join(e.file_name()));
        if e.file_type()?.is_dir() {
            n += copy_dir(&p, &q)?;
        } else {
            n += std::fs::copy(&p, &q)?;
            let mut perm = std::fs::metadata(&q)?.permissions();
            perm.set_readonly(true);
            std::fs::set_permissions(&q, perm)?;
        }
    }
    Ok(n)
}
