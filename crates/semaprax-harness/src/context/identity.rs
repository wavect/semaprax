//! Revision identity: project/worktree ids and a content digest of the working
//! tree (uncommitted edits, deletions and renames included), plus a light scan
//! of `.spx` declaration spans used to locate compiler stable identities.
//! mtime and Git HEAD are never consulted.

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{sha256_labeled, sha256_plain};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    "graphify-out",
];
const MAX_FILES: usize = 50_000;

fn err(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPE010", msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub root: PathBuf,
    pub project_id: String,
    pub worktree_id: String,
    /// Digest over sorted `(relative path, content digest)` pairs.
    pub revision: String,
    pub files: BTreeMap<String, String>,
}

/// Common git dir for a worktree checkout (so two worktrees of one repository
/// share a project id), else the root itself.
fn project_identity(root: &Path) -> PathBuf {
    let dot = root.join(".git");
    if dot.is_dir() {
        return dot.canonicalize().unwrap_or(dot);
    }
    if let Ok(text) = std::fs::read_to_string(&dot) {
        if let Some(gd) = text.trim().strip_prefix("gitdir:") {
            let gd = PathBuf::from(gd.trim());
            let gd = if gd.is_absolute() { gd } else { root.join(gd) };
            if let Ok(common) = std::fs::read_to_string(gd.join("commondir")) {
                let c = PathBuf::from(common.trim());
                let c = if c.is_absolute() { c } else { gd.join(c) };
                return c.canonicalize().unwrap_or(c);
            }
            return gd.canonicalize().unwrap_or(gd);
        }
    }
    root.to_path_buf()
}

impl Snapshot {
    pub fn capture(root: &Path) -> HarnessResult<Snapshot> {
        let root = root
            .canonicalize()
            .map_err(|e| err(format!("project root {}: {e}", root.display())))?;
        let mut files = BTreeMap::new();
        walk(&root, &root, &mut files)?;
        let list: Vec<serde_json::Value> = files
            .iter()
            .map(|(p, d)| serde_json::json!([p, d]))
            .collect();
        let revision = crate::json::digest(
            "semaprax.harness-context.revision.v1",
            &serde_json::Value::Array(list),
        );
        let project_id = sha256_labeled(
            "semaprax.harness-context.project.v1",
            project_identity(&root).to_string_lossy().as_bytes(),
        );
        let worktree_id = sha256_labeled(
            "semaprax.harness-context.worktree.v1",
            root.to_string_lossy().as_bytes(),
        );
        Ok(Snapshot {
            root,
            project_id,
            worktree_id,
            revision,
            files,
        })
    }
}

/// Why a captured file cannot be read back as the captured revision.
pub mod unbound {
    pub const MISSING: &str = "deleted-or-renamed";
    pub const UNREADABLE: &str = "unreadable";
    /// Whole-file digest differs from the capture: a same-size edit, any other
    /// edit, or a replacement by a different file kind.
    pub const CHANGED: &str = "source-changed";
}

impl Snapshot {
    /// Read `rel` and prove the bytes are the captured ones: the whole-file digest
    /// must equal the capture's. The returned text is the very buffer that was
    /// hashed, so a span derived from it belongs to the captured revision. A
    /// mutable path is not an immutable snapshot: a later edit can still follow
    /// this read, which is why callers treat the result as a point-in-time proof.
    pub fn read_bound(&self, rel: &str) -> Result<String, &'static str> {
        let Some(want) = self.files.get(rel) else {
            return Err(unbound::MISSING);
        };
        let p = self.root.join(rel);
        if std::fs::symlink_metadata(&p).is_ok_and(|m| !m.is_file()) {
            return Err(unbound::CHANGED);
        }
        let bytes = std::fs::read(&p).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => unbound::MISSING,
            _ => unbound::UNREADABLE,
        })?;
        if sha256_plain(&bytes) != *want {
            return Err(unbound::CHANGED);
        }
        String::from_utf8(bytes).map_err(|_| unbound::UNREADABLE)
    }
}

fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> HarnessResult<()> {
    let rd = std::fs::read_dir(dir).map_err(|e| err(format!("{}: {e}", dir.display())))?;
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let p = e.path();
        let name = e.file_name().to_string_lossy().into_owned();
        let ft = e
            .file_type()
            .map_err(|x| err(format!("{}: {x}", p.display())))?;
        let rel = p
            .strip_prefix(base)
            .unwrap_or(&p)
            .to_string_lossy()
            .replace('\\', "/");
        if ft.is_symlink() {
            let t = std::fs::read_link(&p)
                .map(|t| t.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.insert(rel, sha256_plain(format!("symlink:{t}").as_bytes()));
        } else if ft.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(base, &p, out)?;
            }
        } else if ft.is_file() {
            if out.len() >= MAX_FILES {
                return Err(err(format!("snapshot exceeds {MAX_FILES} files")));
            }
            let bytes = std::fs::read(&p).map_err(|x| err(format!("{}: {x}", p.display())))?;
            out.insert(rel, sha256_plain(&bytes));
        }
    }
    Ok(())
}

/// Digest of lines `start..=end` (1-based) joined with `\n`, no trailing break.
pub fn span_digest(text: &str, start: u64, end: u64) -> Option<String> {
    if start == 0 || end < start {
        return None;
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if end as usize > lines.len() {
        return None;
    }
    let body = lines[(start - 1) as usize..end as usize].join("\n");
    Some(sha256_plain(body.as_bytes()))
}

/// One `.spx` declaration located textually; the compiler confirms the id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpxDecl {
    pub id: String,
    pub name: String,
    pub path: String,
    pub start_line: u64,
    pub end_line: u64,
}

fn id_of(line: &str) -> Option<String> {
    let t = line.trim();
    let rest = t.strip_prefix("@id(\"")?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn name_of(line: &str) -> Option<String> {
    let toks: Vec<&str> = line
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| !t.is_empty())
        .collect();
    toks.windows(2)
        .find(|w| {
            matches!(
                w[0],
                "fn" | "struct" | "enum" | "class" | "trait" | "type" | "const" | "record"
            )
        })
        .map(|w| w[1].to_string())
}

pub fn scan_spx(snap: &Snapshot) -> Vec<SpxDecl> {
    let mut out = Vec::new();
    for rel in snap.files.keys().filter(|p| p.ends_with(".spx")) {
        let Ok(text) = snap.read_bound(rel) else {
            continue; // not the captured bytes: no declaration is claimed from it
        };
        let lines: Vec<&str> = text.split('\n').collect();
        let starts: Vec<usize> = (0..lines.len())
            .filter(|i| id_of(lines[*i]).is_some())
            .collect();
        for (k, &s) in starts.iter().enumerate() {
            let mut e = starts.get(k + 1).copied().unwrap_or(lines.len());
            while e > s + 1 && lines[e - 1].trim().is_empty() {
                e -= 1;
            }
            let name = lines[s + 1..e.min(s + 4)]
                .iter()
                .find_map(|l| name_of(l))
                .unwrap_or_default();
            out.push(SpxDecl {
                id: id_of(lines[s]).unwrap_or_default(),
                name,
                path: rel.clone(),
                start_line: s as u64 + 1,
                end_line: e as u64,
            });
        }
    }
    out
}
