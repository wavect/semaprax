//! Authenticated project snapshot: a digest of the manifest and every source
//! file, plus the worktree identity. Every pipeline step is bound to one
//! snapshot revision; a changed tree is a stale revision (`SPX-HPD005`).

use crate::contract::ProjectBinding;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{digest, sha256_plain};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MANIFEST: &str = "semaprax.toml";
const MAX_FILES: usize = 4096;
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const SKIP_DIRS: &[&str] = &["target", "node_modules"];

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPD004", msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    /// Canonical project root (contains `semaprax.toml`).
    pub root: PathBuf,
    pub project_id: String,
    pub worktree: String,
    /// Digest of manifest + every `.spx` file path/content.
    pub revision: String,
    pub files: BTreeMap<String, String>,
}

impl Snapshot {
    /// Capture `project` (a directory holding `semaprax.toml`).
    pub fn capture(project: &Path) -> HarnessResult<Snapshot> {
        let root = project
            .canonicalize()
            .map_err(|e| bad(format!("project {}: {e}", project.display())))?;
        if !root.join(MANIFEST).is_file() {
            return Err(bad(format!("{} has no {MANIFEST}", root.display())));
        }
        let mut files = BTreeMap::new();
        walk(&root, &root, &mut files)?;
        let revision = digest(
            "semaprax.harness-snapshot.v1",
            &json!({"files": files.iter().collect::<BTreeMap<_, _>>()}),
        );
        let root_text = root.to_string_lossy().into_owned();
        let project_id = digest("semaprax.harness-project.v1", &json!(root_text));
        let worktree_root = worktree_of(&root);
        let worktree = digest(
            "semaprax.harness-worktree.v1",
            &json!(worktree_root.to_string_lossy()),
        );
        Ok(Snapshot {
            root,
            project_id,
            worktree,
            revision,
            files,
        })
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST)
    }

    pub fn binding(&self) -> ProjectBinding {
        ProjectBinding {
            id: self.project_id.clone(),
            worktree: self.worktree.clone(),
            revision: self.revision.clone(),
        }
    }

    /// Recapture and require the identical revision (stale refusal otherwise).
    pub fn verify_current(&self) -> HarnessResult<()> {
        let now = Snapshot::capture(&self.root)?;
        if now.revision != self.revision {
            let changed: Vec<&String> = now
                .files
                .keys()
                .chain(self.files.keys())
                .filter(|k| now.files.get(*k) != self.files.get(*k))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            return Err(HarnessDiagnostic::new(
                "SPX-HPD005",
                format!(
                    "stale revision: the project changed since snapshot {} (changed: {}); start a new lineage",
                    self.revision,
                    changed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
                ),
            ));
        }
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        json!({"project": self.project_id, "worktree": self.worktree, "revision": self.revision,
               "files": self.files.len()})
    }
}

/// Nearest ancestor holding `.git`, else the project root.
fn worktree_of(root: &Path) -> PathBuf {
    let mut cur = Some(root);
    while let Some(dir) = cur {
        if dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    root.to_path_buf()
}

fn walk(base: &Path, dir: &Path, out: &mut BTreeMap<String, String>) -> HarnessResult<()> {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| bad(format!("{}: {e}", dir.display())))?
        .collect::<Result<_, _>>()
        .map_err(|e| bad(format!("{}: {e}", dir.display())))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path).map_err(|e| bad(format!("{name}: {e}")))?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            walk(base, &path, out)?;
        } else if meta.is_file()
            && (name.ends_with(".spx") || path.parent() == Some(base) && name == MANIFEST)
        {
            if meta.len() > MAX_FILE_BYTES || out.len() >= MAX_FILES {
                return Err(bad("project exceeds the snapshot file bounds"));
            }
            let bytes = std::fs::read(&path).map_err(|e| bad(format!("{name}: {e}")))?;
            let rel = path
                .strip_prefix(base)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            out.insert(rel, sha256_plain(&bytes));
        }
    }
    Ok(())
}
