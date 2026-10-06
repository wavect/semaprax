//! Persistent update state `<home>/updates/state.json` (`semaprax.updates-state.v1`).
//! Records policy, per-source active/previous/pending/rejected revisions and a
//! bounded notice. It never holds user settings, skill modes or local skills;
//! those live elsewhere and are untouched by update, rollback and revoke.

use super::d;
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "semaprax.updates-state.v1";
/// Last-known-good revisions retained per source.
pub const KEEP_PREVIOUS: usize = 5;

pub fn state_path(home: &Path) -> PathBuf {
    home.join("updates").join("state.json")
}

pub fn lock_path(home: &Path) -> PathBuf {
    home.join("updates").join("state.lock")
}

/// Longest an update-state transaction waits for another writer.
pub const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Exclusive ownership of the update state for one read-modify-write. It is an
/// advisory `flock` on `updates/state.lock`: the kernel releases it when this
/// guard drops or the process exits (including a crash), so a stale lock file
/// never blocks anyone. The file itself is never deleted.
#[derive(Debug)]
pub struct StateLock {
    _file: Option<std::fs::File>,
}

/// Take the state lock, waiting at most [`LOCK_WAIT`] (`SPX-HPU017` when busy).
pub fn lock(home: &Path) -> HarnessResult<StateLock> {
    lock_within(home, LOCK_WAIT)
}

#[cfg(unix)]
pub fn lock_within(home: &Path, wait: std::time::Duration) -> HarnessResult<StateLock> {
    use rustix::fs::{flock, FlockOperation};
    let path = lock_path(home);
    let io =
        |e: &dyn std::fmt::Display| d("SPX-HPU017", format!("cannot lock {}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| io(&e))?;
    }
    let f = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| io(&e))?;
    let start = std::time::Instant::now();
    loop {
        match flock(&f, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(StateLock { _file: Some(f) }),
            Err(e) if e == rustix::io::Errno::WOULDBLOCK => {
                if start.elapsed() >= wait {
                    return Err(d(
                        "SPX-HPU017",
                        format!(
                            "update state is busy: another update operation holds {}",
                            path.display()
                        ),
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => return Err(io(&e)),
        }
    }
}

#[cfg(not(unix))]
pub fn lock_within(_: &Path, _: std::time::Duration) -> HarnessResult<StateLock> {
    Ok(StateLock { _file: None })
}

pub fn store_dir(home: &Path) -> PathBuf {
    home.join("artifacts")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Skill,
    Adapter,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Skill => "skill",
            Kind::Adapter => "adapter",
        }
    }
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "skill" => Some(Kind::Skill),
            "adapter" => Some(Kind::Adapter),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    /// The user approved routine upstream checks once (onboarding).
    pub approved: bool,
    /// Compatible content-only updates may activate without review.
    pub auto_content: bool,
    pub ttl_secs: u64,
    pub timeout_ms: u64,
    /// Explicit absolute `gh` executable, if configured.
    pub gh: Option<String>,
}

impl Default for Policy {
    fn default() -> Self {
        Self {
            approved: false,
            auto_content: false,
            ttl_secs: 86_400,
            timeout_ms: 15_000,
            gh: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileRec {
    pub path: String,
    pub upstream_path: String,
    pub git_blob_sha: String,
    /// Plain `sha256:<hex>`.
    pub sha256: String,
    pub bytes: u64,
    pub kind: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revision {
    pub commit: String,
    pub tag: Option<String>,
    pub version: String,
    /// Artifact-v2 digest; the snapshot address.
    pub digest: String,
    pub repo: String,
    pub license: Option<String>,
    pub license_sha256: Option<String>,
    /// Declared identity (skill `name`, adapter `provider.id`).
    pub identity: String,
    /// Requested capabilities, sorted (`tool:..`, `hook:declared`, `network:..`).
    pub requested: Vec<String>,
    pub files: Vec<FileRec>,
    pub activated_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending {
    pub rev: Revision,
    pub reasons: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rejected {
    pub commit: String,
    pub version: String,
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct Source {
    pub id: String,
    pub kind: Kind,
    pub repo: String,
    pub subpath: Option<String>,
    pub channel: String,
    pub head_branch: String,
    /// `(path, upstream_path)` pairs; empty means the whole subpath tree.
    pub files: Vec<(String, String)>,
    pub active: Option<Revision>,
    pub previous: Vec<Revision>,
    pub pending: Option<Pending>,
    pub rejected: Option<Rejected>,
    /// Commit rolled back from; never re-applied automatically.
    pub held: Option<String>,
    pub revoked: Vec<String>,
    /// Active revision is revoked and no safe fallback exists.
    pub unavailable: bool,
    pub resolved_head: Option<(String, String)>,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub policy: Policy,
    pub last_check: u64,
    pub notice: Option<String>,
    pub sources: BTreeMap<String, Source>,
}

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or("").to_string()
}
fn os(v: &Value, k: &str) -> Option<String> {
    v[k].as_str().map(String::from)
}
fn strs(v: &Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

impl Revision {
    pub fn to_json(&self) -> Value {
        json!({
            "commit": self.commit, "tag": self.tag, "version": self.version, "digest": self.digest,
            "repo": self.repo, "license": self.license, "license_sha256": self.license_sha256,
            "identity": self.identity, "requested": self.requested, "activated_at": self.activated_at,
            "files": self.files.iter().map(|f| json!({
                "path": f.path, "upstream_path": f.upstream_path, "git_blob_sha": f.git_blob_sha,
                "sha256": f.sha256, "bytes": f.bytes, "kind": f.kind})).collect::<Vec<_>>(),
        })
    }
    fn from_json(v: &Value) -> Revision {
        Revision {
            commit: s(v, "commit"),
            tag: os(v, "tag"),
            version: s(v, "version"),
            digest: s(v, "digest"),
            repo: s(v, "repo"),
            license: os(v, "license"),
            license_sha256: os(v, "license_sha256"),
            identity: s(v, "identity"),
            requested: strs(&v["requested"]),
            activated_at: v["activated_at"].as_u64().unwrap_or(0),
            files: v["files"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|f| FileRec {
                    path: s(f, "path"),
                    upstream_path: s(f, "upstream_path"),
                    git_blob_sha: s(f, "git_blob_sha"),
                    sha256: s(f, "sha256"),
                    bytes: f["bytes"].as_u64().unwrap_or(0),
                    kind: s(f, "kind"),
                })
                .collect(),
        }
    }
}

impl Source {
    pub fn to_json(&self) -> Value {
        json!({
            "id": self.id, "kind": self.kind.as_str(), "repo": self.repo, "subpath": self.subpath,
            "channel": self.channel, "head_branch": self.head_branch,
            "files": self.files.iter().map(|(a, b)| json!({"path": a, "upstream": b})).collect::<Vec<_>>(),
            "active": self.active.as_ref().map(Revision::to_json),
            "previous": self.previous.iter().map(Revision::to_json).collect::<Vec<_>>(),
            "pending": self.pending.as_ref().map(|p| json!({"rev": p.rev.to_json(), "reasons": p.reasons})),
            "rejected": self.rejected.as_ref().map(|r| json!({
                "commit": r.commit, "version": r.version, "code": r.code, "message": r.message})),
            "held": self.held, "revoked": self.revoked, "unavailable": self.unavailable,
            "resolved_head": self.resolved_head.as_ref().map(|(b, c)| json!({"branch": b, "commit": c})),
        })
    }

    fn from_json(v: &Value) -> HarnessResult<Source> {
        let kind = Kind::parse(&s(v, "kind"))
            .ok_or_else(|| d("SPX-HPU013", "update state has an unknown source kind"))?;
        Ok(Source {
            id: s(v, "id"),
            kind,
            repo: s(v, "repo"),
            subpath: os(v, "subpath"),
            channel: s(v, "channel"),
            head_branch: s(v, "head_branch"),
            files: v["files"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|p| (s(p, "path"), s(p, "upstream")))
                .collect(),
            active: v["active"]
                .is_object()
                .then(|| Revision::from_json(&v["active"])),
            previous: v["previous"]
                .as_array()
                .into_iter()
                .flatten()
                .map(Revision::from_json)
                .collect(),
            pending: v["pending"].is_object().then(|| Pending {
                rev: Revision::from_json(&v["pending"]["rev"]),
                reasons: strs(&v["pending"]["reasons"]),
            }),
            rejected: v["rejected"].is_object().then(|| Rejected {
                commit: s(&v["rejected"], "commit"),
                version: s(&v["rejected"], "version"),
                code: s(&v["rejected"], "code"),
                message: s(&v["rejected"], "message"),
            }),
            held: os(v, "held"),
            revoked: strs(&v["revoked"]),
            unavailable: v["unavailable"].as_bool().unwrap_or(false),
            resolved_head: v["resolved_head"].is_object().then(|| {
                (
                    s(&v["resolved_head"], "branch"),
                    s(&v["resolved_head"], "commit"),
                )
            }),
        })
    }
}

impl State {
    pub fn to_json(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "policy": {"approved": self.policy.approved, "auto_content": self.policy.auto_content,
                "ttl_secs": self.policy.ttl_secs, "timeout_ms": self.policy.timeout_ms, "gh": self.policy.gh},
            "last_check": self.last_check, "notice": self.notice,
            "sources": self.sources.iter().map(|(k, v)| (k.clone(), v.to_json())).collect::<serde_json::Map<_, _>>(),
        })
    }

    pub fn load(home: &Path) -> HarnessResult<State> {
        let path = state_path(home);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
            Err(e) => {
                return Err(d(
                    "SPX-HPU013",
                    format!("cannot read {}: {e}", path.display()),
                ))
            }
        };
        let bad = |m: String| d("SPX-HPU013", format!("{} is corrupt: {m}", path.display()));
        let v = json::parse_strict(&bytes, &json::JsonLimits::frame(16 << 20))
            .map_err(|e| bad(e.message))?;
        if v["schema"] != SCHEMA {
            return Err(bad("unknown schema".into()));
        }
        let p = &v["policy"];
        let dflt = Policy::default();
        let mut st = State {
            policy: Policy {
                approved: p["approved"].as_bool().unwrap_or(false),
                auto_content: p["auto_content"].as_bool().unwrap_or(false),
                ttl_secs: p["ttl_secs"].as_u64().unwrap_or(dflt.ttl_secs),
                timeout_ms: p["timeout_ms"].as_u64().unwrap_or(dflt.timeout_ms),
                gh: os(p, "gh"),
            },
            last_check: v["last_check"].as_u64().unwrap_or(0),
            notice: os(&v, "notice"),
            sources: BTreeMap::new(),
        };
        for (k, sv) in v["sources"].as_object().into_iter().flatten() {
            let src = Source::from_json(sv)?;
            st.sources.insert(k.clone(), src);
        }
        Ok(st)
    }

    pub fn save(&self, home: &Path) -> HarnessResult<()> {
        let doc = format!("{}\n", json::canonical(&self.to_json()));
        crate::profile::installations::write_atomic(&state_path(home), doc.as_bytes())
    }
}
