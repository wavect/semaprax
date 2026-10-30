use std::process::Command;

use semaprax::{codegen, project, wasm};

#[cfg(unix)]
use super::filesystem_v3;
use super::{
    assert_text_interpreter_conformance, compile_c, decimal, environment, filesystem,
    filesystem_v2, formatting, interpreter_i64, io_lines, json_cursors, logging, path_normalize,
    pattern_conformance, process, root, run_and_capture_i64, run_collections_backend_conformance,
    run_text_package_native_conformance, run_text_package_wasm_conformance, temporary, testing,
    wasm_host, PackageMetadata,
};

// The CLI compiles on one explicit 16 MiB worker (`cli_driver::CLI_STACK_BYTES`)
// because the admitted compiler depth exceeds a default thread stack; the
// library entry points leave that to their caller. These lanes call the same
// entry points directly, so give them the CLI's stack instead of the 2 MiB
// libtest default. Measured: unoptimized Core Wasm planning of the std.pattern
// tests closure overflows 2 MiB and completes in 4 MiB.
const COMPILER_STACK_BYTES: usize = 16 * 1024 * 1024;

pub(super) fn run_examples_and_conformance(selected: Vec<PackageMetadata>) {
    std::thread::Builder::new()
        .name("standard-library-conformance".to_owned())
        .stack_size(COMPILER_STACK_BYTES)
        .spawn(move || run_examples_and_conformance_on_compiler_stack(selected))
        .unwrap()
        .join()
        .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
}

fn run_examples_and_conformance_on_compiler_stack(selected: Vec<PackageMetadata>) {
    assert!(!selected.is_empty());
    let scratch = temporary("lanes");
    for package in selected {
        if package.module == "std.int.decimal" {
            decimal::run_conformance();
            continue;
        }
        if package.module == "std.fs" {
            filesystem::run_conformance();
            filesystem_v2::run_conformance();
            #[cfg(unix)]
            filesystem_v3::run_conformance();
            continue;
        }
        if package.module == "std.env" {
            environment::run_conformance();
            continue;
        }
        if testing::run_if_supported(&package) {
            continue;
        }
        if package.module == "std.process" {
            process::run_conformance();
            continue;
        }
        let manifest = root()
            .join("std")
            .join(&package.directory)
            .join("semaprax.toml");
        let manifests =
            pattern_conformance::conformance_manifests(&scratch, &manifest, &package.module);
        for manifest in manifests {
            project::with_authenticated_project(&manifest, |snapshot| {
                snapshot.check()?;
                let options = project::ProjectExecutionOptions::default();
                let entry = snapshot.execute_entry(&options)?;
                let interpreter_examples_value = interpreter_i64(
                    entry.outcome(),
                    &format!("{}: examples failed on the interpreter", package.directory),
                );
                assert_eq!(
                    interpreter_examples_value, 0,
                    "{}: examples did not report success on the interpreter",
                    package.directory
                );
                let tests = snapshot.execute_test(&options)?;
                let interpreter_tests_value = interpreter_i64(
                    tests.outcome(),
                    &format!(
                        "{}: conformance failed on the interpreter",
                        package.directory
                    ),
                );
                assert_eq!(
                    interpreter_tests_value, 0,
                    "{}: conformance did not report success on the interpreter",
                    package.directory
                );
                if package.module == "std.collections" {
                    run_collections_backend_conformance(snapshot, &scratch)?;
                    return Ok(());
                }
                for (role, program) in [
                    ("examples", snapshot.entry_program()),
                    ("tests", snapshot.test_program()),
                ] {
                    // Issue #102: compare the native backend's *actual* computed
                    // value against the interpreter's, rather than each backend
                    // independently asserting it returned the sentinel `0`.
                    // Three backends each self-reporting success proves nothing
                    // about equivalence between them; a real cross-backend
                    // comparison must read one backend's computed value and
                    // check it against another's.
                    let expected = if role == "examples" {
                        interpreter_examples_value
                    } else {
                        interpreter_tests_value
                    };
                    let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
                    for optimization in ["-O0", "-O2"] {
                        let binary = scratch.join(format!(
                            "{}-{role}{}",
                            package.directory,
                            optimization.to_lowercase()
                        ));
                        compile_c(&c, &binary, optimization);
                        let native_value = run_and_capture_i64(&binary);
                        assert_eq!(
                            native_value, expected,
                            "{}: native {role} ({optimization}) returned {native_value}, the \
                         interpreter returned {expected} for the same closure — backends \
                         disagree",
                            package.directory
                        );
                    }
                }
                if package.module == "std.text" {
                    assert_text_interpreter_conformance(snapshot);
                    run_text_package_native_conformance(snapshot, &scratch)?;
                    run_text_package_wasm_conformance(snapshot, &scratch)?;
                    return Ok(());
                }
                // Issue #102: a package's Wasm claim must cover BOTH its
                // examples (entry) and conformance (tests) closures, not only
                // conformance. The reported gap was `byte_range` used solely in
                // `std/bytes`'s examples module while this loop only ever built
                // the tests module for Wasm, so that use never ran on Core Wasm
                // despite the package's Wasm claim. The tuned narrow live-Bytes
                // bounds below were derived from the conformance closure's own
                // allocation count; the examples closure runs against the same
                // generous default every untuned package's tests already use,
                // since its allocation profile is not independently tuned here.
                for (role, module_bytes) in [
                    ("tests", snapshot.test_wasm_module()?),
                    (
                        "examples",
                        wasm::emit_resolved_module(snapshot.entry_program())
                            .map_err(|error| vec![error])?,
                    ),
                ] {
                    let cursor_case = json_cursors::is_cursor_case(&manifest);
                    // Each fixture must balance its declared live Bytes bound.
                    let arena = package.module == "std.pattern"
                        || role == "tests"
                            && (cursor_case
                                || matches!(
                                    package.module.as_str(),
                                    "std.data.json.dec"
                                        | "std.data.csv"
                                        | "std.encoding.base64"
                                        | "std.io"
                                        | "std.io.lines"
                                        | "std.path.value"
                                        | "std.path.normalize"
                                        | "std.pattern"
                                        | "std.log.redact"
                                        | "std.email"
                                        | "std.webhook"
                                        | "std.tracing"
                                        | "std.metrics"
                                        | "std.http"
                                )
                                || (package.module == "std.format"
                                    && formatting::uses_byte_arena(&manifest))
                                || (package.module == "std.log"
                                    && logging::uses_byte_writes(&manifest)));
                    if role == "tests" || package.module == "std.pattern" {
                        for name in ["spx_bytes_zeroed", "spx_bytes_set"] {
                            let present = module_bytes
                                .windows(name.len())
                                .any(|w| w == name.as_bytes());
                            // Individual typed-Path observation cases allocate via copy
                            // without importing the buffer-writing operations.
                            if package.module != "std.path.value" && !cursor_case {
                                // NDJSON's boundary fixtures construct two zeroed
                                // buffers but never write bytes. Core Wasm still
                                // imports owned-buffer operations as one fixed
                                // five-entry family whenever any member, including
                                // `bytes_zeroed`, is called, so the never-called
                                // `spx_bytes_set` is imported too. Require every
                                // member as an exact `env` import entry, so a fused
                                // `spx_bytes_set5` prefix cannot satisfy the plain
                                // `spx_bytes_set` name.
                                let expected = if package.module == "std.data.ndjson" {
                                    for member in [
                                        "spx_bytes_zeroed",
                                        "spx_bytes_set",
                                        "spx_bytes_set5",
                                        "spx_bytes_set1_or5",
                                        "spx_bytes_set1_or6_or48",
                                    ] {
                                        let mut entry = b"\x03env".to_vec();
                                        entry.push(u8::try_from(member.len()).unwrap());
                                        entry.extend_from_slice(member.as_bytes());
                                        assert!(
                                            module_bytes
                                                .windows(entry.len())
                                                .any(|w| w == entry.as_slice()),
                                            "{}: owned-buffer import family lacks `{member}`",
                                            package.directory
                                        );
                                    }
                                    true
                                } else {
                                    arena
                                };
                                assert_eq!(
                                    present, expected,
                                    "{}: `{name}` import",
                                    package.directory
                                );
                            }
                        }
                    }
                    let live_entry_bound = if package.module == "std.pattern" {
                        pattern_conformance::live_bound(
                            &manifest,
                            role,
                            if role == "tests" {
                                snapshot.test_program()
                            } else {
                                snapshot.entry_program()
                            },
                        )
                    } else if role == "examples" {
                        4096
                    } else if package.module == "std.log" {
                        logging::live_byte_bound(&manifest)
                    } else if package.module == "std.io.lines" {
                        io_lines::live_byte_bound(&manifest)
                    } else if package.module == "std.path.normalize" {
                        path_normalize::live_byte_bound(&manifest)
                    } else if cursor_case
                        || matches!(
                            package.module.as_str(),
                            "std.format" | "std.data.csv" | "std.data.ndjson"
                        )
                    {
                        2
                    } else if package.module == "std.path.value"
                        || package.module == "std.encoding.base64"
                    {
                        3
                    } else if arena {
                        1
                    } else {
                        4096
                    };
                    // Issue #102: the value Core Wasm must reproduce is the
                    // interpreter's actual computed value for this same role,
                    // not an independent `0` sentinel.
                    let expected_value = if role == "examples" {
                        interpreter_examples_value
                    } else {
                        interpreter_tests_value
                    };
                    let wasm_path = scratch.join(format!("{}-{role}.wasm", package.directory));
                    std::fs::write(&wasm_path, module_bytes).unwrap();
                    let script = scratch.join(format!("{}-{role}.mjs", package.directory));
                    std::fs::write(
                        &script,
                        wasm_host::wasm_conformance_js(
                            &wasm_path.file_name().unwrap().to_string_lossy(),
                            live_entry_bound,
                            expected_value,
                            (package.module == "std.pattern").then_some(live_entry_bound),
                        ),
                    )
                    .unwrap();
                    let node = Command::new("node")
                        .arg(script.file_name().unwrap())
                        .current_dir(&scratch)
                        .output()
                        .unwrap();
                    assert!(
                        node.status.success(),
                        "{}: Node {role} conformance closure failed: {}",
                        package.directory,
                        String::from_utf8_lossy(&node.stderr)
                    );
                    if package.module == "std.pattern" {
                        pattern_conformance::assert_adjacent_refusal(
                            &scratch,
                            &wasm_path,
                            live_entry_bound,
                            expected_value,
                        );
                    }
                }
                Ok(())
            })
            .unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(scratch);
}
