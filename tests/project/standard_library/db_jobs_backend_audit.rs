//! Issue #102: a package advertising a backend must actually exercise its
//! conformance on that backend. `std.db` (#190) and `std.jobs` (#192) landed
//! shortly before this audit; `std.jobs` had only been executed on the
//! interpreter by its author, and the CLI-only narrow path this session used
//! to spot-check the rest of the catalog (`semaprax run`, `semaprax build
//! --target native`, `semaprax build --target wasm`) never compiles or runs
//! either package's conformance module (`src/tests.spx`) on native or Wasm:
//! the project-level `build` command only ever emits the entry (examples)
//! closure, never the test closure (see `ProjectSnapshot::build_native`,
//! which lowers `self.entry_program` regardless of profile, and
//! `ProjectSnapshot::build_web`, documented as building "the authenticated
//! project entry closure"). `ProjectSnapshot::test_wasm_module` and the
//! direct `codegen::emit_hir_c(test_program)` call are the only routes that
//! exist for conformance-on-native/Wasm, and today they are reachable only
//! from this in-repo harness, not from any CLI subcommand.
//!
//! This test reuses exactly the machinery
//! `examples_and_conformance_return_zero_on_interpreter_native_and_wasm`
//! uses for every package, filtered down to these two, so it is cheap enough
//! to run standalone without the full ~39-package sweep that needs well over
//! 10 GB on this host (see `AGENTS.md`/`CLAUDE.md`). It is the sibling of
//! the existing `typed_paths_execute_on_all_three_backends` narrow check.
use super::*;

#[test]
fn db_and_jobs_execute_on_all_three_backends() {
    if cfg!(windows) {
        return;
    }
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| matches!(p.module.as_str(), "std.db" | "std.jobs"))
            .collect(),
    );
}
