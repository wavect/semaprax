//! Shared test support: unique fixture directories and the built host binary.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

/// Fresh empty directory `<tmp>/<prefix>-<pid>-<serial>`; each module uses its
/// own literal prefix.
pub fn fixture_dir(prefix: &str) -> PathBuf {
    let n = SERIAL.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("{prefix}-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create fixture dir");
    dir
}

pub fn write(dir: &Path, rel: &str, contents: &str) -> PathBuf {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(&path, contents).expect("write fixture");
    path
}

/// Path of the `semaprax-harness` binary built for this test run.
pub fn harness_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_semaprax-harness"))
}

/// Required absolute path from the environment for provisioned tests; panics
/// with an actionable message instead of silently skipping.
pub fn required_tool(var: &str) -> PathBuf {
    let value = std::env::var_os(var).unwrap_or_else(|| panic!("provisioned test requires {var}=<absolute path>"));
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{var} must be absolute");
    path
}

/// Repository root (two levels above this crate).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().expect("repo root")
}
