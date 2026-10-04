//! Immutable content-addressed snapshot store under the harness home
//! (`<home>/artifacts/<hex>/{inventory.json,files/..}`), HN-19.
//!
//! Publication extracts into a private temp directory, validates the result
//! against its inventory and renames it into place, so a partial extraction is
//! never addressable. Activation re-validates before anything is loaded.

use super::d;
use super::inventory::{self, Bounds, Inventory, ScanRules};
use crate::diag::HarnessResult;
use crate::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SERIAL: AtomicUsize = AtomicUsize::new(0);
const FILES: &str = "files";
const INVENTORY_FILE: &str = "inventory.json";

#[derive(Clone, Debug)]
pub struct Snapshot {
    /// v2 digest (`sha256:<hex>`) the snapshot is addressed by.
    pub digest: String,
    /// Directory holding the extracted files (read-only by convention).
    pub files_dir: PathBuf,
    pub inventory: Inventory,
}

fn io(what: &str, e: std::io::Error) -> crate::diag::HarnessDiagnostic {
    d("SPX-HPM035", format!("{what}: {e}"))
}

fn hex_of(digest: &str) -> HarnessResult<&str> {
    digest
        .strip_prefix("sha256:")
        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| d("SPX-HPM035", format!("`{digest}` is not a snapshot digest")))
}

pub fn snapshot_dir(store: &Path, digest: &str) -> HarnessResult<PathBuf> {
    Ok(store.join(hex_of(digest)?))
}

/// Extract `src` into the store (idempotent) and return the validated snapshot.
pub fn publish(
    store: &Path,
    src: &Path,
    rules: &ScanRules,
    bounds: &Bounds,
) -> HarnessResult<Snapshot> {
    std::fs::create_dir_all(store).map_err(|e| io("cannot create the snapshot store", e))?;
    let tmp = store.join(format!(
        ".tmp-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    let result = extract(&tmp, src, rules, bounds);
    let inv = match result {
        Ok(i) => i,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err(e);
        }
    };
    let digest = inv.digest();
    let final_dir = snapshot_dir(store, &digest)?;
    if final_dir.exists() {
        let _ = std::fs::remove_dir_all(&tmp);
        return open(store, &digest);
    }
    if let Err(e) = std::fs::rename(&tmp, &final_dir) {
        let _ = std::fs::remove_dir_all(&tmp);
        // A concurrent publisher of the same digest won the race: use its snapshot.
        if final_dir.exists() {
            return open(store, &digest);
        }
        return Err(io("cannot publish the snapshot", e));
    }
    open(store, &digest)
}

fn extract(tmp: &Path, src: &Path, rules: &ScanRules, bounds: &Bounds) -> HarnessResult<Inventory> {
    let files = tmp.join(FILES);
    std::fs::create_dir_all(&files).map_err(|e| io("cannot create snapshot staging", e))?;
    let inv = inventory::walk(src, rules, bounds, |f| {
        let to = files.join(&f.rel);
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p).map_err(|e| io("cannot stage a file", e))?;
        }
        std::fs::write(&to, &f.bytes).map_err(|e| io("cannot stage a file", e))
    })?;
    let doc = format!("{}\n", json::canonical(&inv.to_json()));
    std::fs::write(tmp.join(INVENTORY_FILE), doc)
        .map_err(|e| io("cannot stage the inventory", e))?;
    Ok(inv)
}

/// Open and fully validate a stored snapshot: inventory digest equals the
/// address, every file hashes to its entry, and no extra file exists.
pub fn open(store: &Path, digest: &str) -> HarnessResult<Snapshot> {
    let dir = snapshot_dir(store, digest)?;
    let corrupt = |m: String| d("SPX-HPM035", format!("snapshot {digest} is not valid: {m}"));
    let bytes = std::fs::read(dir.join(INVENTORY_FILE))
        .map_err(|e| corrupt(format!("inventory unreadable: {e}")))?;
    let v = json::parse_strict(&bytes, &json::JsonLimits::frame(8 << 20))
        .map_err(|e| corrupt(e.message))?;
    let inv = Inventory::from_json(&v)?;
    if inv.digest() != digest {
        return Err(corrupt("inventory does not hash to its address".into()));
    }
    let files_dir = dir.join(FILES);
    let seen = inventory::walk(
        &files_dir,
        &ScanRules {
            classify: inventory::classify_adapter,
            exclude_dir_names: vec![],
            exclude_file_names: vec![],
            exclude_suffixes: vec![],
            exclude_paths: vec![],
        },
        &Bounds {
            max_files: usize::MAX,
            max_total_bytes: u64::MAX,
            max_file_bytes: u64::MAX,
            max_depth: 64,
        },
        |_| Ok(()),
    )
    .map_err(|e| corrupt(e.message))?;
    // Same paths and bytes; kinds come from the inventory, not the walk.
    let key = |i: &Inventory| {
        i.entries()
            .iter()
            .map(|e| (e.path.clone(), e.bytes, e.sha256.clone()))
            .collect::<Vec<_>>()
    };
    if key(&seen) != key(&inv) {
        return Err(corrupt(
            "files differ from the inventory (missing, extra or modified)".into(),
        ));
    }
    Ok(Snapshot {
        digest: digest.to_string(),
        files_dir,
        inventory: inv,
    })
}
