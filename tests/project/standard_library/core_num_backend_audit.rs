//! Issue #102: `std.core`, `std.num`, `std.num.overflow`, `std.random`,
//! `std.time`, and `std.test` each claim `interpreter`, `native-c11`, and
//! `core-wasm` in `std/packages.json`, but none of them has ever had a
//! dedicated test that exercises `run_examples_and_conformance` for its
//! module the way `std.format`, `std.log`, `std.io`, and the other
//! `*_execute_on_all_three_backends` tests do. Absent such a test, the only
//! thing that would ever run these six packages' conformance (tests.spx)
//! closures on native C11 and Core Wasm is the single ~39-package
//! `examples_and_conformance_return_zero_on_interpreter_native_and_wasm`
//! sweep, which this host cannot run (see `AGENTS.md`/`CLAUDE.md`: it needs
//! well over 10 GB). So today their per-backend claim is unverified in
//! practice, exactly like `std.db`/`std.jobs` were before
//! `db_jobs_backend_audit.rs`.
//!
//! This test reuses the same `run_examples_and_conformance` machinery,
//! filtered to these six packages, so it is cheap enough to run standalone.
use super::*;

#[test]
fn core_num_random_time_test_execute_on_all_three_backends() {
    if cfg!(windows) {
        return;
    }
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| {
                matches!(
                    p.module.as_str(),
                    "std.core" | "std.num" | "std.num.overflow" | "std.random" | "std.time" | "std.test"
                )
            })
            .collect(),
    );
}
