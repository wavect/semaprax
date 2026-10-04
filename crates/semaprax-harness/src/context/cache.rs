//! Revision-safe result cache for external provider responses. Lives under the
//! host-provided cache root, never in the project. The key binds the content
//! digest of the working tree, the provider/extractor identity, configuration
//! and permission scope, the query and the output contract. Authority is not a
//! cache property: the broker rechecks it before every read, hits included.

use super::external::{ExternalQuery, ExternalResponse, ProviderIdentity};
use super::identity::Snapshot;
use crate::json::digest;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const OUTPUT_CONTRACT: &str = "semaprax.harness-context.v1/byte-v1";
const ENTRY_SCHEMA: &str = "semaprax.harness-context-cache.v1";

#[derive(Clone, Copy, Debug)]
pub struct CacheConfig {
    pub max_entries: usize,
    pub max_bytes: u64,
    pub ttl_secs: u64,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 64,
            max_bytes: 16 * 1024 * 1024,
            ttl_secs: 3600,
        }
    }
}

pub type Clock = Box<dyn Fn() -> u64 + Send + Sync>;

pub fn system_clock() -> Clock {
    Box::new(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    })
}

pub struct ResultCache {
    root: PathBuf,
    cfg: CacheConfig,
    clock: Clock,
}

/// Key plus the metadata needed for purge and supersession.
#[derive(Clone, Debug)]
pub struct CacheKey {
    pub key: String,
    pub worktree_id: String,
    pub provider_id: String,
    /// Same worktree + provider + query: a newer revision supersedes older entries.
    pub group: String,
}

impl CacheKey {
    pub fn new(snap: &Snapshot, identity: &ProviderIdentity, q: &ExternalQuery) -> Self {
        let query = json!({"op": q.op, "payload": q.payload});
        let key = digest(
            "semaprax.harness-context.cache-key.v1",
            &json!({"project": snap.project_id, "worktree": snap.worktree_id, "revision": snap.revision,
                    "provider": identity.to_json(), "query": query, "output": OUTPUT_CONTRACT}),
        );
        let group = digest(
            "semaprax.harness-context.cache-group.v1",
            &json!({"worktree": snap.worktree_id, "provider": identity.provider_id, "query": query}),
        );
        Self {
            key,
            worktree_id: snap.worktree_id.clone(),
            provider_id: identity.provider_id.clone(),
            group,
        }
    }
}

impl ResultCache {
    pub fn new(root: PathBuf, cfg: CacheConfig, clock: Clock) -> Self {
        Self { root, cfg, clock }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path(&self, key: &str) -> PathBuf {
        self.root
            .join(format!("{}.json", key.trim_start_matches("sha256:")))
    }

    fn read(&self, p: &Path) -> Option<Value> {
        let v: Value = serde_json::from_slice(&std::fs::read(p).ok()?).ok()?;
        (v["schema"] == ENTRY_SCHEMA).then_some(v)
    }

    pub fn get(&self, k: &CacheKey) -> Option<ExternalResponse> {
        let p = self.path(&k.key);
        let v = self.read(&p)?;
        let age = (self.clock)().saturating_sub(v["created"].as_u64()?);
        if v["key"] != k.key.as_str() || age > self.cfg.ttl_secs {
            let _ = std::fs::remove_file(&p);
            return None;
        }
        ExternalResponse::from_json(&v["response"])
    }

    pub fn put(&self, k: &CacheKey, resp: &ExternalResponse) {
        if std::fs::create_dir_all(&self.root).is_err() {
            return;
        }
        for (p, v) in self.entries() {
            if v["group"] == k.group.as_str() && v["key"] != k.key.as_str() {
                let _ = std::fs::remove_file(p);
            }
        }
        let doc = json!({"schema": ENTRY_SCHEMA, "key": k.key, "created": (self.clock)(), "worktree_id": k.worktree_id,
                         "provider_id": k.provider_id, "group": k.group, "response": resp.to_json()});
        // Unique per write: two threads or processes must never share a temp file, or one entry's bytes
        // could be renamed into another's slot.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let tmp = self.root.join(format!(
            ".tmp-{}-{}",
            std::process::id(),
            SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        if std::fs::write(&tmp, doc.to_string()).is_ok()
            && std::fs::rename(&tmp, self.path(&k.key)).is_err()
        {
            let _ = std::fs::remove_file(&tmp);
        }
        self.evict();
    }

    fn entries(&self) -> Vec<(PathBuf, Value)> {
        let Ok(rd) = std::fs::read_dir(&self.root) else {
            return vec![];
        };
        let mut out: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| self.read(&p).map(|v| (p, v)))
            .collect();
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Drop expired entries, then oldest-first until count and size bounds hold.
    pub fn evict(&self) {
        let now = (self.clock)();
        let mut live = Vec::new();
        for (p, v) in self.entries() {
            let created = v["created"].as_u64().unwrap_or(0);
            if now.saturating_sub(created) > self.cfg.ttl_secs {
                let _ = std::fs::remove_file(&p);
            } else {
                let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                live.push((created, p, size));
            }
        }
        live.sort();
        let mut total: u64 = live.iter().map(|e| e.2).sum();
        let mut count = live.len();
        for (_, p, size) in live {
            if count <= self.cfg.max_entries && total <= self.cfg.max_bytes {
                break;
            }
            let _ = std::fs::remove_file(p);
            count -= 1;
            total = total.saturating_sub(size);
        }
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn purge_where(&self, f: impl Fn(&Value) -> bool) -> usize {
        let mut n = 0;
        for (p, v) in self.entries() {
            if f(&v) && std::fs::remove_file(p).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// Explicit purge of everything.
    pub fn purge_all(&self) -> usize {
        self.purge_where(|_| true)
    }

    /// Revocation purge: every entry a provider ever produced.
    pub fn purge_provider(&self, provider_id: &str) -> usize {
        self.purge_where(|v| v["provider_id"] == provider_id)
    }

    pub fn purge_worktree(&self, worktree_id: &str) -> usize {
        self.purge_where(|v| v["worktree_id"] == worktree_id)
    }
}
