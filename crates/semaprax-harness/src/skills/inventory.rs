//! Artifact inventory/digest v2 (HN-19): identity over the normalized relative
//! path, file kind and exact bytes of every admitted file. Used for skill
//! bundles and for adapter-owned helper closures. The digest is evidence of
//! identity, never permission. See `docs/HARNESS-ARTIFACT-IDENTITY-V1.md`.

use super::d;
use crate::diag::HarnessResult;
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const INVENTORY_SCHEMA: &str = "semaprax.artifact-inventory.v2";
/// Identity label of the v2 digest; the pre-v2 skill digest is `legacy-v1`.
pub const IDENTITY_V2: &str = "artifact-v2";
pub const IDENTITY_LEGACY: &str = "legacy-v1";
/// Prefix that marks a v2 closure digest where a plain `sha256:` entry digest
/// was recorded before (trust records, grants).
pub const V2_PREFIX: &str = "artifact-v2:";

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileKind {
    /// `SKILL.md`, manifests and other passive prose.
    PassiveText,
    /// `references/` and `assets/` payloads.
    ReferenceAsset,
    /// `scripts/` content; never executed by the catalog.
    ExecutableScript,
    /// Adapter entry, descriptor and helper modules.
    AdapterCode,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PassiveText => "passive-text",
            Self::ReferenceAsset => "reference-asset",
            Self::ExecutableScript => "executable-script",
            Self::AdapterCode => "adapter-code",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::PassiveText,
            Self::ReferenceAsset,
            Self::ExecutableScript,
            Self::AdapterCode,
        ]
        .into_iter()
        .find(|k| k.as_str() == s)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InventoryEntry {
    /// Normalized relative path, `/` separated.
    pub path: String,
    pub kind: FileKind,
    pub bytes: u64,
    /// Plain `sha256:<hex>` of the exact file bytes.
    pub sha256: String,
}

/// Canonical (path-sorted) set of entries.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Inventory {
    entries: Vec<InventoryEntry>,
}

/// Which files a scan admits, and what kind each one is.
#[derive(Clone, Debug)]
pub struct ScanRules {
    pub classify: fn(&str) -> FileKind,
    /// Directory names skipped wherever they occur (caches, VCS metadata).
    pub exclude_dir_names: Vec<String>,
    /// File names skipped wherever they occur.
    pub exclude_file_names: Vec<String>,
    pub exclude_suffixes: Vec<String>,
    /// Relative paths (`dir/` prefix or exact file) from explicit closure rules.
    pub exclude_paths: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub max_files: usize,
    pub max_total_bytes: u64,
    pub max_file_bytes: u64,
    pub max_depth: usize,
}

impl Bounds {
    pub const SKILL: Bounds = Bounds {
        max_files: 256,
        max_total_bytes: 16 << 20,
        max_file_bytes: 4 << 20,
        max_depth: 8,
    };
    pub const ADAPTER: Bounds = Bounds {
        max_files: 2048,
        max_total_bytes: 64 << 20,
        max_file_bytes: 16 << 20,
        max_depth: 12,
    };
}

pub fn classify_skill(rel: &str) -> FileKind {
    if rel.starts_with("scripts/") {
        FileKind::ExecutableScript
    } else if rel.starts_with("references/") || rel.starts_with("assets/") {
        FileKind::ReferenceAsset
    } else {
        FileKind::PassiveText
    }
}

pub fn classify_adapter(_rel: &str) -> FileKind {
    FileKind::AdapterCode
}

const CACHE_DIRS: &[&str] = &[".git", "__pycache__", ".pytest_cache", ".mypy_cache"];
const CACHE_FILES: &[&str] = &[".DS_Store"];

impl ScanRules {
    pub fn skill() -> Self {
        Self {
            classify: classify_skill,
            exclude_dir_names: vec![".git".into()],
            exclude_file_names: CACHE_FILES.iter().map(|s| s.to_string()).collect(),
            exclude_suffixes: Vec::new(),
            exclude_paths: Vec::new(),
        }
    }
    pub fn adapter(extra_excludes: Vec<String>) -> Self {
        Self {
            classify: classify_adapter,
            exclude_dir_names: CACHE_DIRS.iter().map(|s| s.to_string()).collect(),
            exclude_file_names: CACHE_FILES.iter().map(|s| s.to_string()).collect(),
            exclude_suffixes: vec![".pyc".into(), ".pyo".into()],
            exclude_paths: extra_excludes,
        }
    }
    fn skips(&self, rel: &str, name: &str, is_dir: bool) -> bool {
        if is_dir && self.exclude_dir_names.iter().any(|n| n == name) {
            return true;
        }
        if !is_dir
            && (self.exclude_file_names.iter().any(|n| n == name)
                || self
                    .exclude_suffixes
                    .iter()
                    .any(|s| name.ends_with(s.as_str())))
        {
            return true;
        }
        self.exclude_paths
            .iter()
            .any(|p| match p.strip_suffix('/') {
                Some(dir) => rel == dir || rel.starts_with(p.as_str()),
                None => rel == p,
            })
    }
}

fn unsafe_path(msg: String) -> crate::diag::HarnessDiagnostic {
    d("SPX-HPM034", msg)
}

/// A normalized relative path: no root, empty, `.`/`..` segment, backslash or
/// control character.
pub fn normalized_rel_ok(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 512
        && !p.starts_with('/')
        && !p.chars().any(|c| c == '\\' || c.is_control())
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

impl Inventory {
    /// Build from entries in any order. Duplicate or case-colliding paths are
    /// refused (`SPX-HPM034`).
    pub fn new(mut entries: Vec<InventoryEntry>) -> HarnessResult<Self> {
        entries.sort_by(|a, b| a.path.cmp(&b.path));
        let mut folded = BTreeMap::new();
        for e in &entries {
            if !normalized_rel_ok(&e.path) {
                return Err(unsafe_path(format!(
                    "`{}` is not a normalized relative path",
                    e.path
                )));
            }
            if let Some(prev) = folded.insert(e.path.to_lowercase(), e.path.clone()) {
                return Err(unsafe_path(format!(
                    "paths `{prev}` and `{}` collide after case folding",
                    e.path
                )));
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[InventoryEntry] {
        &self.entries
    }
    pub fn get(&self, path: &str) -> Option<&InventoryEntry> {
        self.entries
            .binary_search_by(|e| e.path.as_str().cmp(path))
            .ok()
            .map(|i| &self.entries[i])
    }

    pub fn to_json(&self) -> Value {
        json!({"schema": INVENTORY_SCHEMA, "entries": self.entries.iter().map(|e| json!({
            "path": e.path, "kind": e.kind.as_str(), "bytes": e.bytes, "sha256": e.sha256,
        })).collect::<Vec<_>>()})
    }

    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        let bad = || d("SPX-HPM035", "malformed artifact inventory document");
        let o = v.as_object().ok_or_else(bad)?;
        if o.get("schema").and_then(Value::as_str) != Some(INVENTORY_SCHEMA) || o.len() != 2 {
            return Err(bad());
        }
        let mut out = Vec::new();
        for e in o.get("entries").and_then(Value::as_array).ok_or_else(bad)? {
            let e = e.as_object().ok_or_else(bad)?;
            out.push(InventoryEntry {
                path: e
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(bad)?
                    .into(),
                kind: e
                    .get("kind")
                    .and_then(Value::as_str)
                    .and_then(FileKind::parse)
                    .ok_or_else(bad)?,
                bytes: e.get("bytes").and_then(Value::as_u64).ok_or_else(bad)?,
                sha256: e
                    .get("sha256")
                    .and_then(Value::as_str)
                    .ok_or_else(bad)?
                    .into(),
            });
        }
        Self::new(out)
    }

    /// Canonical v2 digest (`sha256:<hex>`, domain `semaprax.artifact-inventory.v2`).
    pub fn digest(&self) -> String {
        json::digest(INVENTORY_SCHEMA, &self.to_json())
    }

    pub fn total_bytes(&self) -> u64 {
        self.entries.iter().map(|e| e.bytes).sum()
    }
}

/// One admitted file as read during a walk.
pub struct WalkedFile {
    pub rel: String,
    pub bytes: Vec<u8>,
}

#[cfg(unix)]
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    a.dev() == b.dev() && a.ino() == b.ino()
}
#[cfg(not(unix))]
fn same_file(_: &std::fs::Metadata, _: &std::fs::Metadata) -> bool {
    true
}

#[cfg(unix)]
fn link_count(m: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    m.nlink()
}
#[cfg(not(unix))]
fn link_count(_: &std::fs::Metadata) -> u64 {
    1
}

fn read_regular(
    path: &Path,
    shown: &str,
    meta: &std::fs::Metadata,
    b: &Bounds,
) -> HarnessResult<Vec<u8>> {
    use std::io::Read;
    if meta.len() > b.max_file_bytes {
        return Err(unsafe_path(format!(
            "`{shown}` exceeds {} bytes",
            b.max_file_bytes
        )));
    }
    let f = std::fs::File::open(path)
        .map_err(|e| unsafe_path(format!("cannot read `{shown}`: {e}")))?;
    let after = f
        .metadata()
        .map_err(|e| unsafe_path(format!("cannot stat `{shown}`: {e}")))?;
    if !after.is_file() || !same_file(meta, &after) {
        return Err(unsafe_path(format!("`{shown}` changed while being read")));
    }
    let mut buf = Vec::with_capacity(meta.len() as usize);
    f.take(b.max_file_bytes.saturating_add(1))
        .read_to_end(&mut buf)
        .map_err(|e| unsafe_path(format!("cannot read `{shown}`: {e}")))?;
    if buf.len() as u64 > b.max_file_bytes {
        return Err(unsafe_path(format!(
            "`{shown}` exceeds {} bytes",
            b.max_file_bytes
        )));
    }
    Ok(buf)
}

/// Walk `root`, refusing symlinks, hardlinks, special files, non-UTF-8 or
/// unnormalizable names, and bound violations. Calls `sink` for every admitted
/// file in sorted order and returns the inventory of what it saw.
pub fn walk(
    root: &Path,
    rules: &ScanRules,
    bounds: &Bounds,
    mut sink: impl FnMut(&WalkedFile) -> HarnessResult<()>,
) -> HarnessResult<Inventory> {
    let rm = std::fs::symlink_metadata(root)
        .map_err(|e| unsafe_path(format!("cannot read {}: {e}", root.display())))?;
    if !rm.is_dir() {
        return Err(unsafe_path(format!(
            "{} is not a directory (symlinks are refused)",
            root.display()
        )));
    }
    let mut entries = Vec::new();
    let mut total = 0u64;
    let mut pending = vec![(root.to_path_buf(), String::new(), 0usize)];
    while let Some((dir, prefix, depth)) = pending.pop() {
        if depth > bounds.max_depth {
            return Err(unsafe_path(format!(
                "`{prefix}` nests deeper than {}",
                bounds.max_depth
            )));
        }
        let mut names: Vec<(String, std::path::PathBuf)> = Vec::new();
        for e in std::fs::read_dir(&dir)
            .map_err(|e| unsafe_path(format!("cannot list {}: {e}", dir.display())))?
        {
            let e = e.map_err(|e| unsafe_path(format!("cannot list {}: {e}", dir.display())))?;
            let name = e
                .file_name()
                .into_string()
                .map_err(|_| unsafe_path(format!("non-UTF-8 file name under `{prefix}`")))?;
            names.push((name, e.path()));
        }
        names.sort();
        for (name, path) in names {
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let meta = std::fs::symlink_metadata(&path)
                .map_err(|e| unsafe_path(format!("cannot stat `{rel}`: {e}")))?;
            let ft = meta.file_type();
            if ft.is_symlink() {
                if rules.skips(&rel, &name, ft.is_dir()) {
                    continue;
                }
                return Err(unsafe_path(format!(
                    "`{rel}` is a symlink; symlinks are refused"
                )));
            }
            if ft.is_dir() {
                if !rules.skips(&rel, &name, true) {
                    pending.push((path, rel, depth + 1));
                }
                continue;
            }
            if rules.skips(&rel, &name, false) {
                continue;
            }
            if !ft.is_file() {
                return Err(unsafe_path(format!("`{rel}` is not a regular file")));
            }
            if !normalized_rel_ok(&rel) {
                return Err(unsafe_path(format!(
                    "`{rel}` is not a normalized relative path"
                )));
            }
            if link_count(&meta) > 1 {
                return Err(unsafe_path(format!(
                    "`{rel}` is a hardlink alias (link count > 1)"
                )));
            }
            if entries.len() >= bounds.max_files {
                return Err(unsafe_path(format!("more than {} files", bounds.max_files)));
            }
            let bytes = read_regular(&path, &rel, &meta, bounds)?;
            total = total.saturating_add(bytes.len() as u64);
            if total > bounds.max_total_bytes {
                return Err(unsafe_path(format!(
                    "more than {} bytes in total",
                    bounds.max_total_bytes
                )));
            }
            entries.push(InventoryEntry {
                kind: (rules.classify)(&rel),
                bytes: bytes.len() as u64,
                sha256: json::sha256_plain(&bytes),
                path: rel.clone(),
            });
            sink(&WalkedFile { rel, bytes })?;
        }
    }
    Inventory::new(entries)
}

/// Inventory of a directory without keeping any bytes.
pub fn scan(root: &Path, rules: &ScanRules, bounds: &Bounds) -> HarnessResult<Inventory> {
    walk(root, rules, bounds, |_| Ok(()))
}

/// `artifact-v2:sha256:<hex>`: the v2 closure digest as recorded in trust and
/// grants.
pub fn labelled(digest: &str) -> String {
    format!("{V2_PREFIX}{digest}")
}

pub fn is_v2_label(s: &str) -> bool {
    s.starts_with(V2_PREFIX)
}

/// Optional explicit closure rules of an adapter directory.
pub const CLOSURE_FILE: &str = "harness-closure.json";
pub const CLOSURE_SCHEMA: &str = "semaprax.harness-closure.v1";

/// Closure rules: `{"schema": ..., "exclude": ["test/", "EVIDENCE.md"]}`. The
/// file itself is part of the closure, so changing the rules changes identity.
pub fn closure_rules(dir: &Path, protected: &[&str]) -> HarnessResult<ScanRules> {
    let p = dir.join(CLOSURE_FILE);
    let mut exclude = Vec::new();
    if std::fs::symlink_metadata(&p).is_ok() {
        let bytes = std::fs::read(&p)
            .map_err(|e| d("SPX-HPM034", format!("cannot read {CLOSURE_FILE}: {e}")))?;
        let v = json::parse_strict(&bytes, &json::JsonLimits::frame(64 * 1024))
            .map_err(|e| d("SPX-HPM034", format!("{CLOSURE_FILE}: {}", e.message)))?;
        let o = v.as_object().filter(|o| {
            o.len() == 2 && o.get("schema").and_then(Value::as_str) == Some(CLOSURE_SCHEMA)
        });
        let o = o.ok_or_else(|| {
            d(
                "SPX-HPM034",
                format!("{CLOSURE_FILE} must be {{schema: {CLOSURE_SCHEMA}, exclude: [..]}}"),
            )
        })?;
        for x in o
            .get("exclude")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let s = x
                .as_str()
                .filter(|s| normalized_rel_ok(s.trim_end_matches('/')))
                .ok_or_else(|| {
                    d(
                        "SPX-HPM034",
                        format!("{CLOSURE_FILE}: `exclude` holds normalized relative paths"),
                    )
                })?;
            let covers = |q: &str| match s.strip_suffix('/') {
                Some(dir) => q == dir || q.starts_with(s),
                None => q == s,
            };
            if protected.iter().any(|q| covers(q)) || covers(CLOSURE_FILE) {
                return Err(d("SPX-HPM034", format!("{CLOSURE_FILE} cannot exclude `{s}`: it is the entry, descriptor or the rules themselves")));
            }
            exclude.push(s.to_string());
        }
    }
    Ok(ScanRules::adapter(exclude))
}

/// Read one inventoried file from `root`, refusing a symlink on any component
/// and a file that is not a regular, single-link file.
pub fn read_file(root: &Path, rel: &str, max_bytes: u64) -> HarnessResult<Vec<u8>> {
    if !normalized_rel_ok(rel) {
        return Err(unsafe_path(format!(
            "`{rel}` is not a normalized relative path"
        )));
    }
    let mut cur = root.to_path_buf();
    let parts: Vec<&str> = rel.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        cur.push(part);
        let m = std::fs::symlink_metadata(&cur)
            .map_err(|e| unsafe_path(format!("cannot stat `{rel}`: {e}")))?;
        if m.file_type().is_symlink() {
            return Err(unsafe_path(format!("`{rel}` passes through a symlink")));
        }
        if i + 1 == parts.len() {
            if !m.is_file() || link_count(&m) > 1 {
                return Err(unsafe_path(format!(
                    "`{rel}` is not a regular single-link file"
                )));
            }
            let b = Bounds {
                max_files: 1,
                max_total_bytes: max_bytes,
                max_file_bytes: max_bytes,
                max_depth: 0,
            };
            return read_regular(&cur, rel, &m, &b);
        } else if !m.is_dir() {
            return Err(unsafe_path(format!(
                "`{rel}` passes through a non-directory"
            )));
        }
    }
    unreachable!("normalized paths are non-empty")
}

/// Closure inventory of an adapter directory: every file under `dir` except
/// caches and the explicit exclusions of `harness-closure.json`. The entry
/// and the rules file can never be excluded.
pub fn adapter_closure(
    dir: &Path,
    entry_rel: &str,
    protected: &[&str],
) -> HarnessResult<Inventory> {
    let mut all = vec![entry_rel];
    all.extend_from_slice(protected);
    let rules = closure_rules(dir, &all)?;
    walk(dir, &rules, &Bounds::ADAPTER, |_| Ok(()))
}

/// [`adapter_closure`] as the `artifact-v2:sha256:..` label stored in trust
/// records and grants.
pub fn adapter_closure_label(
    dir: &Path,
    entry_rel: &str,
    protected: &[&str],
) -> HarnessResult<String> {
    adapter_closure(dir, entry_rel, protected).map(|i| labelled(&i.digest()))
}
