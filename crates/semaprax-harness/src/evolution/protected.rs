//! Content digests of protected material (official skill snapshots, the
//! application repository, graders), asserted equal before and after a run.

use super::w;
use crate::diag::HarnessResult;
use crate::json::{sha256_labeled, sha256_plain};
use std::path::Path;

const SKIP_DIRS: &[&str] = &[".git", "target", "node_modules"];
const MAX_FILES: usize = 50_000;

/// Digest of a file, or of a directory tree (sorted relative path + content;
/// symlinks hash their target text; VCS and build directories skipped).
pub fn digest_path(p: &Path) -> HarnessResult<String> {
    let md = std::fs::symlink_metadata(p)
        .map_err(|e| w("SPX-HPW001", format!("protected path {}: {e}", p.display())))?;
    if md.is_file() {
        let b = std::fs::read(p)
            .map_err(|e| w("SPX-HPW001", format!("protected path {}: {e}", p.display())))?;
        return Ok(sha256_plain(&b));
    }
    let mut lines = Vec::new();
    walk(p, p, &mut lines)?;
    lines.sort();
    Ok(sha256_labeled(
        "semaprax.evolution-protected.v1",
        lines.join("\n").as_bytes(),
    ))
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> HarnessResult<()> {
    let rd = std::fs::read_dir(dir).map_err(|e| {
        w(
            "SPX-HPW001",
            format!("protected path {}: {e}", dir.display()),
        )
    })?;
    for entry in rd.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let md = std::fs::symlink_metadata(&path)
            .map_err(|e| w("SPX-HPW001", format!("{}: {e}", path.display())))?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        if md.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                walk(root, &path, out)?;
            }
        } else if md.file_type().is_symlink() {
            let t = std::fs::read_link(&path).unwrap_or_default();
            out.push(format!("{rel}\0link:{}", t.display()));
        } else {
            let b = std::fs::read(&path)
                .map_err(|e| w("SPX-HPW001", format!("{}: {e}", path.display())))?;
            out.push(format!("{rel}\0{}", sha256_plain(&b)));
        }
        if out.len() > MAX_FILES {
            return Err(w("SPX-HPW001", "protected tree exceeds 50000 files"));
        }
    }
    Ok(())
}

/// True when either path contains (or equals) the other.
pub fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}
