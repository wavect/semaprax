//! Issue #102: `std.collections`, `std.mem`, and `std.text` each claim
//! `interpreter`, `native-c11`, and `core-wasm` in `std/packages.json`.
//! `run_examples_and_conformance` already carries dedicated special-case
//! branches for `std.collections` (`run_collections_backend_conformance`)
//! and `std.text` (`assert_text_interpreter_conformance`,
//! `run_text_package_native_conformance`,
//! `run_text_package_wasm_conformance`), but before this test nothing
//! actually invoked those branches outside the un-runnable ~39-package
//! `examples_and_conformance_return_zero_on_interpreter_native_and_wasm`
//! sweep (needs well over 10 GB, see `AGENTS.md`/`CLAUDE.md`). `std.mem` had
//! no special case at all and no dedicated test either, so its generic
//! native/Wasm conformance path was equally unexercised.
//!
//! This test reuses `run_examples_and_conformance`, filtered to these three
//! packages, so all three special-case/generic paths get real coverage
//! standalone.
use super::*;

#[test]
fn collections_mem_text_execute_on_all_three_backends() {
    if cfg!(windows) {
        return;
    }
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| matches!(p.module.as_str(), "std.collections" | "std.mem" | "std.text"))
            .collect(),
    );
}
