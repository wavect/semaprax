//! Issue #102: `std.async`, `std.http`, and `std.net` each claim
//! `interpreter`, `native-c11`, and `core-wasm` in `std/packages.json`, but
//! before this test none of them had a dedicated test exercising
//! `run_examples_and_conformance` for its module the way `std.format`,
//! `std.log`, and the sibling `*_backend_audit.rs` tests already do. Absent
//! such a test, only the single ~39-package
//! `examples_and_conformance_return_zero_on_interpreter_native_and_wasm`
//! sweep would ever run their conformance (tests.spx) closures on native C11
//! and Core Wasm, and that sweep needs well over 10 GB and cannot run on
//! this host (see `AGENTS.md`/`CLAUDE.md`).
//!
//! None of the three declares a `required` hosted capability in its
//! manifest (unlike `std.env`/`std.fs`/`std.process`), so they take the
//! generic `run_examples_and_conformance` path rather than an
//! injected-provider special case.
use super::*;

#[test]
fn async_http_net_execute_on_all_three_backends() {
    if cfg!(windows) {
        return;
    }
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| matches!(p.module.as_str(), "std.async" | "std.http" | "std.net"))
            .collect(),
    );
}
