//! Exact selected Rust API-index admission shared by linked preparation and
//! consumption. A Linux evidence run selects only its supplied regular files;
//! the consumer must replay those same target facts before reconstructing the
//! held Project revision used by generated M3.

use semaprax_rust_api_index::RustApiIndex;
use std::{env, fs, path::PathBuf};

pub(crate) const LINUX_X86_64_TARGET: &str = "x86_64-unknown-linux-gnu";

fn selected_index(name: &str, built_in: &[u8]) -> Vec<u8> {
    let Some(directory) = env::var_os("RI13_RUST_API_INDEX_DIR") else {
        return built_in.to_vec();
    };
    let directory = PathBuf::from(directory);
    assert!(
        directory.is_absolute(),
        "RI13 Rust API index directory must be absolute"
    );
    let path = directory.join(name);
    let metadata = fs::symlink_metadata(&path).expect("selected RI13 Rust API index metadata");
    assert!(
        metadata.file_type().is_file(),
        "selected RI13 Rust API index must be a regular file"
    );
    fs::read(path).expect("selected RI13 Rust API index")
}

pub(crate) fn admit_selected_index(name: &str, built_in: &[u8]) -> RustApiIndex {
    let index = RustApiIndex::admit_extractor_output(&selected_index(name, built_in))
        .expect("admitted selected RI13 Rust API index");
    if env::var_os("RI13_RUST_API_INDEX_DIR").is_some() {
        assert_eq!(
            index.target(),
            LINUX_X86_64_TARGET,
            "selected RI13 Rust API index target"
        );
    }
    index
}
