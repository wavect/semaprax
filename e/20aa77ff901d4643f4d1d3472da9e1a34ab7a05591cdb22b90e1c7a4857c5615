//! Fresh packet acquisition only; outer invocation guards retain controllers.
use super::{compiled_successor_v19, held_4a6eb, invocation_load, INVOCATION_PACKET_READS};
use std::path::Path;

pub(in super::super) fn run() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &Path, length: usize, sha: &str| held_4a6eb(path, length, sha);
    for &(path, length, sha) in INVOCATION_PACKET_READS {
        read(&root.join(path), length, sha).unwrap();
    }
    let (_, page) = invocation_load(&read).unwrap();
    compiled_successor_v19::authenticate20_using(&page, &read).unwrap();
    compiled_successor_v19::authenticate_using(&page, &read).unwrap();
}
