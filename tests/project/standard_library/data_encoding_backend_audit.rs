//! Issue #102: `std.data.csv`, `std.data.json`, `std.data.json.digits`,
//! `std.data.json.doc`, `std.data.json.token`, `std.encoding`, `std.url`, and
//! `std.path` each claim `interpreter`, `native-c11`, and `core-wasm` in
//! `std/packages.json`, but before this test none of them had a dedicated
//! test exercising `run_examples_and_conformance` for its module the way
//! `std.format`, `std.log`, `std.io`, and the sibling
//! `*_backend_audit.rs`/`*_execute_on_all_three_backends` tests already do.
//! Absent such a test, only the single ~39-package
//! `examples_and_conformance_return_zero_on_interpreter_native_and_wasm`
//! sweep would ever run their conformance (tests.spx) closures on native C11
//! and Core Wasm, and that sweep needs well over 10 GB and cannot run on
//! this host (see `AGENTS.md`/`CLAUDE.md`).
//!
//! This test reuses the same `run_examples_and_conformance` machinery,
//! filtered to these eight packages, so it is cheap enough to run
//! standalone, exactly like `core_num_backend_audit.rs` and
//! `db_jobs_backend_audit.rs`.
use super::*;

#[test]
fn data_encoding_url_path_execute_on_all_three_backends() {
    if cfg!(windows) {
        return;
    }
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| {
                matches!(
                    p.module.as_str(),
                    "std.data.csv"
                        | "std.data.json"
                        | "std.data.json.digits"
                        | "std.data.json.doc"
                        | "std.data.json.token"
                        | "std.encoding"
                        | "std.url"
                        | "std.path"
                )
            })
            .collect(),
    );
}
