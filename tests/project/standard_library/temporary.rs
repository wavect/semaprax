use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

pub(super) fn temporary(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-standard-library-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    // The Project loader authenticates directory ancestry and rejects a
    // symlinked temp root such as macOS `/var`, so hand it the real path.
    path.canonicalize().unwrap()
}
