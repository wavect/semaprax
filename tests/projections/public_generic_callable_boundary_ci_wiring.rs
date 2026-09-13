//! Issue #140: the `public-generic-ownership-milestone` CI job wires the
//! metadata-consumer grammar corpus (`public_generic_consumers`) and the
//! separation/grammar library modules, but audited against the pinned
//! workflow at this issue's baseline it selected NONE of the actual
//! callable-boundary corpus: neither adapter harness
//! (`tests/public_generic_native_adapter_v1.rs`,
//! `tests/public_generic_wasm_adapter_v1.rs` -- real generated Rust/C11/
//! C++17/TypeScript *calling* consumers executed end to end against a
//! compiled native provider or a real Wasm module, the cross-engine
//! settlement differential corpus, and the shared hostile/malformed-input
//! manifest) nor this harness's own sibling module
//! `public_generic_descriptor_carrier_hostile_replay` (the independent
//! malformed-descriptor/carrier/binding matrix, issue #173). All three are
//! real, passing, non-trivial local suites -- confirmed by running each
//! explicitly (29 passed/3 ignored, 18 passed, 6 passed respectively at
//! this issue's baseline) -- executed by zero hosted run. This is exactly
//! the same defect class `execution_matrix::every_owning_module_with_tests_has_a_ci_selector`
//! (`tests/project/standard_library/execution_matrix.rs`) closes for the
//! standard library: a real, green, local test suite with no CI selector
//! is invisible to every hosted gate, so a regression in it is invisible
//! too.
//!
//! Unlike that guard, this one does not enumerate every individual test
//! function -- the callable-boundary corpus spans two dedicated top-level
//! harness binaries (issue #140's own `AGENTS.md`-mandated module-per-file
//! layout, not a single `mod`-list harness) where the natural CI unit is
//! the WHOLE binary, matching how `tests/public_generic_native_adapter_v1/run_all_four_callers.sh`
//! already invokes each (`cargo test --locked --test "$native_bin"`/
//! `"$wasm_bin"` with no per-test filter). It instead requires the three
//! literal invocation strings below -- each already a distinctive,
//! never-otherwise-occurring substring of `.github/workflows/ci.yml`
//! (confirmed empirically: zero hits for any of the three harness/module
//! names anywhere in the pinned workflow before this issue) -- and, as a
//! sanity check on the guard itself, that every file it names is real and
//! still contains at least one `#[test]`.
//!
//! `.github/workflows/ci.yml` is coordinator-owned for this issue (per the
//! implementing worker's own assignment) rather than edited directly here,
//! so this guard is EXPECTED TO FAIL until the coordinator applies exactly
//! the delta named in each assertion's own message -- restated once, in
//! full, right here:
//!
//! ```yaml
//!       - name: Callable-boundary corpus: generated consumers, settlement, and hostile replay
//!         shell: bash
//!         run: |
//!           set -euo pipefail
//!           cargo test --locked -p semaprax --test public_generic_native_adapter_v1
//!           cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1
//!           cargo test --locked -p semaprax --test projections public_generic_descriptor_carrier_hostile_replay
//! ```
//!
//! placed as a new step in the `public-generic-ownership-milestone` job,
//! after its existing "Four-language metadata consumers and hostile
//! replay" step (`.github/workflows/ci.yml`) -- the job already resolves
//! and exports the `clang`/`clang++`/`node` toolchains this corpus needs
//! in its own "Resolve the consumer toolchains this host really has" step,
//! so no new toolchain provisioning is required.

use std::fs;
use std::path::Path;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn ci_workflow() -> String {
    fs::read_to_string(root().join(".github/workflows/ci.yml"))
        .expect("read the pinned CI workflow")
}

fn public_generic_job(ci: &str) -> &str {
    let start = ci
        .find("  public-generic-ownership-milestone:")
        .expect("public-generic-ownership-milestone job exists");
    let end = start
        + ci[start..]
            .find("\n  std-library-depth:")
            .expect("public-generic-ownership-milestone job has a following job");
    &ci[start..end]
}

/// One row: the harness/module identity, the file(s) that must contain a
/// real `#[test]` (so this guard cannot demand a selector for a harness
/// that does not actually exist or has gone empty), and the exact,
/// currently-absent invocation string the milestone job must contain.
struct Row {
    description: &'static str,
    test_files: &'static [&'static str],
    required_ci_invocation: &'static str,
}

const ROWS: &[Row] = &[
    Row {
        description: "native adapter harness (Rust/C11/C++17 calling consumers, \
                       cross-engine settlement differential, shared hostile corpus)",
        test_files: &[
            "tests/public_generic_native_adapter_v1/c_calling_consumer.rs",
            "tests/public_generic_native_adapter_v1/cxx_calling_consumer.rs",
            "tests/public_generic_native_adapter_v1/rust_calling_consumer.rs",
            "tests/public_generic_native_adapter_v1/settlement_corpus.rs",
            "tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs",
        ],
        required_ci_invocation: "cargo test --locked -p semaprax --test public_generic_native_adapter_v1",
    },
    Row {
        description: "Core-Wasm adapter harness (TypeScript calling consumer against a real \
                       compiled Wasm export, shared hostile corpus)",
        test_files: &[
            "tests/public_generic_wasm_adapter_v1/compiled_reference_endpoint.rs",
            "tests/public_generic_wasm_adapter_v1/shared_hostile_corpus.rs",
            "tests/public_generic_wasm_adapter_v1/typescript_calling_consumer.rs",
        ],
        required_ci_invocation: "cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1",
    },
    Row {
        description: "independent malformed descriptor/carrier/binding replay (issue #173)",
        test_files: &["tests/projections/public_generic_descriptor_carrier_hostile_replay.rs"],
        required_ci_invocation:
            "cargo test --locked -p semaprax --test projections public_generic_descriptor_carrier_hostile_replay",
    },
];

#[test]
fn callable_boundary_corpus_has_a_real_test_in_every_named_file() {
    let mut missing = Vec::new();
    for row in ROWS {
        for file in row.test_files {
            let source = fs::read_to_string(root().join(file))
                .unwrap_or_else(|error| panic!("read {file}: {error}"));
            if !source.contains("#[test]") {
                missing.push(format!("{file} (row: {})", row.description));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "these files no longer contain any `#[test]`, so the corresponding CI-wiring \
         requirement below is demanding a selector for a harness that no longer has real \
         tests -- update this guard's ROWS to match: {missing:?}"
    );
}

#[test]
fn public_generic_ownership_milestone_job_selects_the_full_callable_boundary_corpus() {
    let workflow = ci_workflow();
    let ci = public_generic_job(&workflow);
    let mut missing = Vec::new();
    for row in ROWS {
        if !ci.contains(row.required_ci_invocation) {
            missing.push(format!(
                "{}\n    -> add: {}",
                row.description, row.required_ci_invocation
            ));
        }
    }
    assert!(
        missing.is_empty(),
        "`.github/workflows/ci.yml`'s `public-generic-ownership-milestone` job does not select \
         the real, passing, local callable-boundary corpus below, so no hosted run ever executes \
         it (issue #140) -- add each missing invocation as its own step in that job (the job \
         already resolves clang/clang++/node in its \"Resolve the consumer toolchains this host \
         really has\" step, so no new toolchain provisioning is needed):\n{}",
        missing.join("\n")
    );
}

#[test]
fn public_generic_milestone_preflights_the_pinned_consumer_toolchains() {
    let workflow = ci_workflow();
    let ci = public_generic_job(&workflow);
    for required in [
        "Install the repository-pinned TypeScript compiler (Unix)",
        "Install the repository-pinned TypeScript compiler (Windows)",
        "if: runner.os != 'Windows'",
        "if: runner.os == 'Windows'",
        "npm ci --ignore-scripts",
        "SPX_PG_TSC",
        "xcrun --find ar",
        "clang -### -x c /dev/null -o /dev/null",
        "call \"%SPX_PG_TSC%\" --version || exit /b 1",
        "Version 5.8.3",
        "rustc --version --verbose",
        "cargo --version",
        "runner_arch",
        "clangxx_path",
        "ar_path",
        "node_path",
        "tsc_path",
        "node -p 'process.arch'",
    ] {
        assert!(
            ci.contains(required),
            "public-generic-ownership-milestone preflight lost required identity/provisioning marker: {required}"
        );
    }
}
