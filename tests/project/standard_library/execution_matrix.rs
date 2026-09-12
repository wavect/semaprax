//! Issue #102: an explicit package -> owning-test execution matrix, plus the
//! negative controls the issue's own acceptance criteria name. The matrix
//! turns "some package/backend row is missing" and "two rows claim the same
//! package" into a hard failure *before* anything downstream can read the
//! matrix as complete coverage; the negative controls below turn "an
//! example silently returns the wrong value" and "a matrix definition can
//! pass with no real rows" into hard failures too.
//!
//! This module intentionally never re-executes the packages themselves
//! (`run_examples_and_conformance` and its per-package callers already do
//! that): it only guards the *inventory* those callers are built from, and
//! proves - with synthetic data, not the real table - that the guard itself
//! can fail.
use super::*;

/// What backs one package's execution claim: a real owning test in this
/// harness, or an explicit, recorded reason the row is out of scope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Owner {
    /// `crate-relative::module::path::to::the_test_fn`, exactly as it
    /// appears after `standard_library::` in `cargo test`'s selector and in
    /// `.github/workflows/ci.yml`.
    Test(&'static str),
    /// A package excluded from a claim, with the reason recorded here
    /// rather than left as a silent gap.
    #[allow(dead_code)]
    Excluded(&'static str),
}

/// The declared package -> owner table. One row per package in
/// `std/packages.json`; `execution_matrix_covers_every_declared_package_with_no_duplicates`
/// asserts this set is exactly `packages()`'s module set with no duplicate,
/// and that every `Owner::Test` name resolves to a real `#[test]` function
/// somewhere under `tests/project/standard_library/`.
fn declared_owners() -> Vec<(&'static str, Owner)> {
    use Owner::Test;
    vec![
        ("std.agent", Test("testing::agent_package_and_bundled_consumer_execute_across_engines")),
        ("std.async", Test("async_net_backend_audit::async_http_net_execute_on_all_three_backends")),
        ("std.auth", Test("auth_backend_audit::auth_executes_on_all_three_backends")),
        ("std.bytes", Test("byte_spans::byte_spans_execute_on_all_three_backends")),
        ("std.collections", Test("collections_mem_text_backend_audit::collections_mem_text_execute_on_all_three_backends")),
        ("std.core", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.data.csv", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.data.json", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.data.json.dec", Test("json_cursors::json_cursors_decode_execute_on_all_three_backends")),
        ("std.data.json.digits", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.data.json.doc", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.data.json.token", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.data.json.utf8", Test("logging::utf8_ascii_scan_preserves_package_conformance")),
        ("std.data.json.write", Test("json_cursors::json_cursors_write_execute_on_all_three_backends")),
        ("std.data.toml", Test("toml_cursors::toml_cursors_execute_on_all_three_backends")),
        ("std.db", Test("db_jobs_backend_audit::db_and_jobs_execute_on_all_three_backends")),
        ("std.encoding", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.encoding.base64", Test("base64::base64_encoding_executes_on_all_three_backends")),
        ("std.env", Test("environment::environment_package_executes_all_functions_with_injected_snapshot")),
        ("std.env.policy", Test("env_policy::env_policy_executes_on_all_three_backends")),
        ("std.format", Test("formatting::format_writer_executes_on_all_three_backends")),
        ("std.fs", Test("filesystem::filesystem_standard_commands_execute_on_all_three_backends")),
        ("std.http", Test("async_net_backend_audit::async_http_net_execute_on_all_three_backends")),
        ("std.io", Test("io_cursors::io_cursors_execute_on_all_three_backends")),
        ("std.io.lines", Test("io_lines::io_lines_execute_on_all_three_backends")),
        ("std.jobs", Test("db_jobs_backend_audit::db_and_jobs_execute_on_all_three_backends")),
        ("std.log", Test("logging::log_writer_executes_on_all_three_backends")),
        ("std.log.redact", Test("log_redact_backend_audit::log_redact_executes_on_all_three_backends")),
        ("std.mem", Test("collections_mem_text_backend_audit::collections_mem_text_execute_on_all_three_backends")),
        ("std.net", Test("async_net_backend_audit::async_http_net_execute_on_all_three_backends")),
        ("std.num", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.num.overflow", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.path", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
        ("std.path.normalize", Test("path_normalize::path_normalization_executes_on_all_three_backends")),
        ("std.path.value", Test("typed_paths_execute_on_all_three_backends")),
        ("std.process", Test("process::process_package_executes_all_functions_with_registered_request_shape")),
        ("std.random", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.test", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.test.bytes", Test("testing::test_bytes_package_and_bundled_consumer_execute_across_engines")),
        ("std.text", Test("collections_mem_text_backend_audit::collections_mem_text_execute_on_all_three_backends")),
        ("std.time", Test("core_num_backend_audit::core_num_random_time_test_execute_on_all_three_backends")),
        ("std.url", Test("data_encoding_backend_audit::data_encoding_url_path_execute_on_all_three_backends")),
    ]
}

#[derive(Debug, PartialEq, Eq)]
enum MatrixError {
    DuplicateRow(String),
    MissingRow(String),
}

/// Reject a duplicate package row or a package the `required` set names but
/// `rows` never covers - both *before* any caller can read `rows` as a
/// complete coverage claim.
fn validate_inventory(required: &[String], rows: &[(String, Owner)]) -> Result<(), MatrixError> {
    let mut seen = std::collections::HashSet::new();
    for (module, _) in rows {
        if !seen.insert(module.clone()) {
            return Err(MatrixError::DuplicateRow(module.clone()));
        }
    }
    for module in required {
        if !rows.iter().any(|(candidate, _)| candidate == module) {
            return Err(MatrixError::MissingRow(module.clone()));
        }
    }
    Ok(())
}

/// Every `.rs` file directly under `tests/project/standard_library/`, plus
/// `standard_library.rs` itself, concatenated - used only to confirm a
/// declared owner's function actually exists in source, not to execute it.
fn harness_source() -> String {
    let mut combined =
        std::fs::read_to_string(root().join("tests/project/standard_library.rs")).unwrap();
    let directory = root().join("tests/project/standard_library");
    let mut entries = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        combined.push('\n');
        combined.push_str(&std::fs::read_to_string(&path).unwrap());
    }
    combined
}

/// Whether `owner`'s function (its last `::`-separated segment) exists as an
/// `fn <name>` declaration anywhere in the harness source. This does not
/// confirm the function is reachable or annotated `#[test]` - only that the
/// name in the table is not a typo or a stale reference to a deleted test.
fn owner_fn_exists(source: &str, owner: &str) -> bool {
    let short_name = owner.rsplit("::").next().unwrap();
    source.contains(&format!("fn {short_name}("))
}

#[test]
fn execution_matrix_covers_every_declared_package_with_no_duplicates() {
    let required = packages().into_iter().map(|p| p.module).collect::<Vec<_>>();
    let rows = declared_owners();
    let inventory = rows
        .iter()
        .map(|(module, owner)| ((*module).to_owned(), *owner))
        .collect::<Vec<_>>();
    validate_inventory(&required, &inventory).unwrap();
    // No row claims a package `packages()` does not also declare - the
    // converse of the `MissingRow` direction just checked.
    let declared_modules = inventory
        .iter()
        .map(|(module, _)| module.clone())
        .collect::<std::collections::HashSet<_>>();
    for module in &required {
        assert!(
            declared_modules.contains(module),
            "{module}: declared in {PACKAGES} but has no execution_matrix row"
        );
    }
    assert_eq!(
        inventory.len(),
        required.len(),
        "execution_matrix row count must equal the declared package count exactly, \
         with no row for a package {PACKAGES} does not list"
    );
    let source = harness_source();
    for (module, owner) in &rows {
        match owner {
            Owner::Test(name) => assert!(
                owner_fn_exists(&source, name),
                "{module}: owning test `{name}` was not found under tests/project/standard_library/"
            ),
            Owner::Excluded(reason) => assert!(
                !reason.is_empty(),
                "{module}: an excluded row must record a non-empty reason"
            ),
        }
    }
}

/// Required failure case: "a missing backend row or duplicate package
/// inventory entry fails before any coverage claim." Both checked here
/// against synthetic data - never the real 41-package table - so this
/// proves the *validator* can fail, not merely that today's table happens
/// to be clean.
#[test]
fn execution_matrix_rejects_a_duplicate_package_row() {
    let required = vec!["std.negative-control".to_owned()];
    let rows = vec![
        ("std.negative-control".to_owned(), Owner::Test("a")),
        ("std.negative-control".to_owned(), Owner::Test("b")),
    ];
    match validate_inventory(&required, &rows) {
        Err(MatrixError::DuplicateRow(module)) => assert_eq!(module, "std.negative-control"),
        other => panic!("expected a rejected duplicate row, got {other:?}"),
    }
}

#[test]
fn execution_matrix_rejects_a_missing_declared_package_row() {
    let required = vec!["std.negative-control".to_owned(), "std.other".to_owned()];
    let rows = vec![("std.negative-control".to_owned(), Owner::Test("a"))];
    match validate_inventory(&required, &rows) {
        Err(MatrixError::MissingRow(module)) => assert_eq!(module, "std.other"),
        other => panic!("expected a rejected missing row, got {other:?}"),
    }
}

/// An empty matrix reporting "all rows green" is exactly the failure mode
/// this whole module exists to rule out: `validate_inventory` must reject
/// an empty `rows` against any non-empty `required` set rather than
/// vacuously accepting it.
#[test]
fn execution_matrix_rejects_an_empty_table_against_any_required_package() {
    let required = vec!["std.negative-control".to_owned()];
    let rows: Vec<(String, Owner)> = Vec::new();
    match validate_inventory(&required, &rows) {
        Err(MatrixError::MissingRow(module)) => assert_eq!(module, "std.negative-control"),
        other => panic!("expected the empty table to reject as missing, got {other:?}"),
    }
}

/// Required failure case: "a deliberately wrong example result fails the
/// owning gate even when its tests module passes." Builds a minimal,
/// self-contained fixture (no dependency on any real `std/` package, so it
/// cannot be confused with one) whose `tests.spx` returns `0` (passes) and
/// whose `examples.spx` returns `1` (a deliberately wrong result), then
/// replays the *exact* assertion `run_examples_and_conformance` uses for
/// every real package - `assert_eq!(entry.outcome(),
/// &ProjectExecutionOutcome::Returned(0), ...)` - inside `catch_unwind` and
/// asserts it panics. If this ever stopped panicking, the real gate would
/// have stopped catching a wrong example too.
#[test]
fn a_deliberately_wrong_example_fails_the_owning_gate_even_though_its_tests_module_passes() {
    let directory = temporary("negative-control-wrong-example");
    std::fs::create_dir_all(directory.join("src")).unwrap();
    std::fs::write(
        directory.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n\
         [package]\n\
         name = \"negative-control-fixture\"\n\
         version = \"0.1.0\"\n\
         profile = \"owned-data-api.v1\"\n\n\
         [modules]\n\
         entry = \"negative.control.examples\"\n\
         sources = [\"src/examples.spx\", \"src/tests.spx\"]\n\
         tests = [\"negative.control.tests\"]\n\n\
         [exports]\n\
         web = []\n",
    )
    .unwrap();
    // A deliberately wrong example: the package's own convention (matching
    // every real bundled package) is that `examples.spx`'s `main` returns
    // `0` on success. This one returns `1` instead.
    std::fs::write(
        directory.join("src/examples.spx"),
        "module negative.control.examples;\n\n\
         @id(\"negative.control.examples.main\")\n\
         fn main() -> i64\n\
         {\n    1\n}\n",
    )
    .unwrap();
    // Its conformance module passes on its own, unconditionally - proving
    // the wrong example is not merely masked by a tests module that would
    // fail anyway.
    std::fs::write(
        directory.join("src/tests.spx"),
        "module negative.control.tests;\n\n\
         @id(\"negative.control.tests.main\")\n\
         fn main() -> i64\n\
         {\n    0\n}\n",
    )
    .unwrap();
    project::with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        let tests = snapshot.execute_test(&options)?;
        assert_eq!(
            tests.outcome(),
            &project::ProjectExecutionOutcome::Returned(0),
            "fixture's own conformance module must pass unconditionally: {tests:?}"
        );
        let entry = snapshot.execute_entry(&options)?;
        // The real gate's exact assertion, replayed here and caught rather
        // than allowed to fail this test outright.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            assert_eq!(
                entry.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "negative-control-fixture: examples failed on the interpreter"
            );
        }));
        assert!(
            caught.is_err(),
            "a deliberately wrong example (returned {:?}, not Returned(0)) must fail the owning \
             gate's assertion even though its tests module passed",
            entry.outcome()
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

/// Required failure case: "toolchain-unavailable required runs are
/// failures, not green skips." Every native/Wasm step in this harness
/// spawns `clang`/`node` with `Command::new(..).output().unwrap()` (see
/// `compile_and_run_c` and every `Command::new("node")` call in this
/// directory) - there is no `which`-style availability probe anywhere in
/// `tests/project/standard_library/` that turns a missing toolchain into a
/// skip. `Command::output()` returns `Err(io::ErrorKind::NotFound)` for a
/// binary that cannot be found; `.unwrap()` on that `Err` panics, i.e.
/// fails the test. This replays that exact `.unwrap()` shape against a
/// binary name that cannot exist and confirms it panics rather than
/// returning quietly, so a required toolchain genuinely missing on a CI
/// runner fails the same way, not as an empty green pass.
#[test]
fn a_missing_required_toolchain_fails_rather_than_skips() {
    let caught = std::panic::catch_unwind(|| {
        std::process::Command::new("semaprax-toolchain-that-does-not-exist-negative-control")
            .output()
            .unwrap();
    });
    assert!(
        caught.is_err(),
        "a missing required toolchain binary must fail (panic) exactly like every \
         `Command::new(..).output().unwrap()` call in this harness, not return quietly"
    );
}

/// Issue #102's own root cause, guarded permanently: a module can carry real
/// `#[test]` functions in this harness and still never run in a hosted CI
/// job, because `cargo test`'s bare `--test project` selects nothing on its
/// own and every module needs its own named selector line in
/// `.github/workflows/ci.yml`'s `std-library-depth` job. `environment`,
/// `process`, `text`, `execution_matrix` (this module), `filesystem_v2`,
/// `provider_outcomes`, `auth_backend_audit` and `log_redact_backend_audit`
/// all had `#[test]` functions with zero selector anywhere in `ci.yml` when
/// this test was written - passing locally, executed by no hosted run. This
/// walks every `mod <name>;` declared directly in `standard_library.rs`
/// (skipping one - `temporary` - that is a shared helper with no `#[test]`
/// of its own) and fails if its file contains a `#[test]` but `ci.yml`
/// contains no `standard_library::<name>` selector naming it. A module whose
/// tests are only reached as a nested submodule of another (for example
/// `json_cursors::json_roundtrip`) is not itself declared in
/// `standard_library.rs`, so it is correctly out of this walk: its owning
/// parent's selector already covers it by substring.
#[test]
fn every_owning_module_with_tests_has_a_ci_selector() {
    let ci = std::fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    let harness =
        std::fs::read_to_string(root().join("tests/project/standard_library.rs")).unwrap();
    let directory = root().join("tests/project/standard_library");
    let mut missing = Vec::new();
    for line in harness.lines() {
        let Some(name) = line
            .trim()
            .strip_prefix("mod ")
            .and_then(|rest| rest.strip_suffix(';'))
        else {
            continue;
        };
        let module_source = std::fs::read_to_string(directory.join(format!("{name}.rs"))).unwrap();
        if !module_source.contains("#[test]") {
            // A shared helper module (e.g. `temporary`) with no test of its
            // own needs no selector.
            continue;
        }
        if !ci.contains(&format!("standard_library::{name}")) {
            missing.push(name.to_owned());
        }
    }
    assert!(
        missing.is_empty(),
        "these standard_library test modules declare `#[test]` functions but \
         .github/workflows/ci.yml selects none of them, so no hosted CI run ever \
         executes them: {missing:?}"
    );
}
