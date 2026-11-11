//! Fresh packet acquisition only; outer invocation guards retain controllers.
use super::{held_4a6eb, load_current, INVOCATION_PACKET_READS};
use std::path::Path;

pub(in super::super) fn run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &Path, length: usize, sha: &str| held_4a6eb(path, length, sha);
    for &(path, length, sha) in INVOCATION_PACKET_READS {
        read(&root.join(path), length, sha).unwrap();
    }
    // Acquire the same physical successor child, policy and page as composition.
    load_current().unwrap();
}
