//! Standard-library identity, catalog and executable conformance gates.

use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::{codegen, format, project, verify, wasm};

use super::owned_bounded_vec_dependencies::{compile_and_run_c, run_internal_wasm};

const PACKAGES: &str = "std/packages.json";
const AGENT_CATALOG: &str = "std/catalog.json";
const HUMAN_CATALOG: &str = "docs/STANDARD-LIBRARY-CATALOG.md";
const CATALOG_SCHEMA: &str = "semaprax.standard-library-catalog.v1";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[path = "standard_library/environment.rs"]
mod environment;
#[path = "standard_library/formatting.rs"]
mod formatting;
#[path = "standard_library/logging.rs"]
mod logging;
#[path = "standard_library/process.rs"]
mod process;

#[path = "standard_library/temporary.rs"]
mod temporary;
use temporary::temporary;

#[derive(Clone, Debug)]
struct PackageMetadata {
    directory: String,
    module: String,
    tier: String,
    targets: Vec<String>,
    status: String,
}

fn packages() -> Vec<PackageMetadata> {
    let text = std::fs::read_to_string(root().join(PACKAGES)).unwrap();
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["schema"], "semaprax.standard-library-packages.v1");
    let packages = value["packages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|package| PackageMetadata {
            directory: package["directory"].as_str().unwrap().to_owned(),
            module: package["module"].as_str().unwrap().to_owned(),
            tier: package["tier"].as_str().unwrap().to_owned(),
            targets: package["targets"]
                .as_array()
                .unwrap()
                .iter()
                .map(|target| target.as_str().unwrap().to_owned())
                .collect(),
            status: package["status"].as_str().unwrap().to_owned(),
        })
        .collect::<Vec<_>>();
    assert!(!packages.is_empty(), "{PACKAGES} lists no packages");
    let mut directories = std::fs::read_dir(root().join("std"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("semaprax.toml").is_file())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    directories.sort();
    let mut listed = packages
        .iter()
        .map(|package| package.directory.clone())
        .collect::<Vec<_>>();
    assert_eq!(
        listed, directories,
        "{PACKAGES} must list exactly the package directories under std/ in sorted order"
    );
    listed.dedup();
    assert_eq!(
        listed.len(),
        packages.len(),
        "{PACKAGES} lists a directory twice"
    );
    for package in &packages {
        assert!(
            matches!(
                package.tier.as_str(),
                "core"
                    | "alloc"
                    | "portable"
                    | "hosted"
                    | "browser"
                    | "embedded"
                    | "agent"
                    | "test"
            ),
            "{}: unknown portability tier `{}`",
            package.directory,
            package.tier
        );
        assert!(
            matches!(package.status.as_str(), "partial" | "implemented"),
            "{}: unknown status `{}`",
            package.directory,
            package.status
        );
        assert!(
            package.module.starts_with("std."),
            "{}: module `{}` is outside the std namespace",
            package.directory,
            package.module
        );
    }
    packages
}

struct LibrarySource {
    path: PathBuf,
    program: semaprax::ast::Program,
    source: String,
}

/// The library module of a package: the one listed source that is neither
/// the entry (examples) module nor the test (conformance) module.
fn package_sources(package: &PackageMetadata) -> (LibrarySource, String, String) {
    let package_root = root().join("std").join(&package.directory);
    let manifest = std::fs::read_to_string(package_root.join("semaprax.toml")).unwrap();
    let field = |name: &str| -> String {
        let line = manifest
            .lines()
            .find(|line| line.starts_with(&format!("{name} = ")))
            .unwrap_or_else(|| panic!("{}: manifest lacks `{name}`", package.directory));
        line.split_once(" = ").unwrap().1.to_owned()
    };
    let unquote = |value: &str| value.trim_matches('"').to_owned();
    let entry = unquote(&field("entry"));
    let tests = field("tests");
    let tests = unquote(tests.trim_start_matches('[').trim_end_matches(']'));
    let sources = field("sources");
    let sources = sources
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|item| unquote(item.trim()))
        .collect::<Vec<_>>();
    let mut library = Vec::new();
    for relative in &sources {
        let path = package_root.join(relative);
        let source = std::fs::read_to_string(&path).unwrap();
        let (program, comments) =
            semaprax::parse_with_comments(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::comments::canonical_with_comments(&program, &comments),
            source,
            "{} is not canonical",
            path.display()
        );
        if program.module != entry && program.module != tests {
            library.push(LibrarySource {
                path,
                program,
                source,
            });
        }
    }
    assert_eq!(
        library.len(),
        1,
        "{}: a standard-library package holds exactly one library module beside its examples and conformance modules",
        package.directory
    );
    let library = library.pop().unwrap();
    assert_eq!(
        library.program.module, package.module,
        "{}: library module name disagrees with {PACKAGES}",
        package.directory
    );
    (library, entry, tests)
}

fn required_consumer_profile(package: &PackageMetadata) -> String {
    std::fs::read_to_string(
        root()
            .join("std")
            .join(&package.directory)
            .join("semaprax.toml"),
    )
    .unwrap()
    .lines()
    .find_map(|line| {
        line.strip_prefix("profile = \"")
            .and_then(|value| value.strip_suffix('"'))
    })
    .unwrap_or("scalar")
    .to_owned()
}

#[test]
fn every_public_declaration_has_a_std_identity_contracts_examples_and_conformance() {
    for package in packages() {
        let (library, entry, tests) = package_sources(&package);
        assert_eq!(entry, format!("{}.examples", package.module));
        assert_eq!(tests, format!("{}.tests", package.module));
        let package_root = root().join("std").join(&package.directory);
        let conformance = std::fs::read_to_string(package_root.join("src/tests.spx")).unwrap();
        let examples = std::fs::read_to_string(package_root.join("src/examples.spx")).unwrap();
        assert!(
            !library.program.functions.is_empty(),
            "{}: library module declares no functions",
            library.path.display()
        );
        for function in &library.program.functions {
            let prefix = format!("{}.", package.module);
            assert!(
                function.explicit_id && function.stable_id.starts_with(&prefix),
                "{}: `{}` needs an explicit @id below `{prefix}`",
                library.path.display(),
                function.name
            );
            let expected_effects: Vec<String> =
                match (package.module.as_str(), function.stable_id.as_str()) {
                    ("std.fs", "std.fs.read" | "std.fs.metadata" | "std.fs.list") => {
                        vec!["fs.read".into()]
                    }
                    (
                        "std.fs",
                        "std.fs.write-new"
                        | "std.fs.write-atomic"
                        | "std.fs.create-dir"
                        | "std.fs.remove",
                    ) => vec!["fs.write".into()],
                    ("std.env", _) => vec!["process.environment.read".into()],
                    ("std.process", "std.process.run") => vec!["process.execute".into()],
                    _ => vec![],
                };
            assert_eq!(
                function.effects,
                expected_effects,
                "{}: `{}` declares an unexpected effect inventory",
                library.path.display(),
                function.name
            );
            let import = format!(
                "use function @id(\"{}\") from {} as ",
                function.stable_id, package.module
            );
            assert!(
                conformance.contains(&import),
                "{}: conformance module does not import `{}`",
                library.path.display(),
                function.stable_id
            );
            assert_ne!(function.name, "main");
        }
        for declaration in &library.program.types {
            assert!(
                declaration.explicit_id
                    && declaration
                        .stable_id
                        .starts_with(&format!("{}.", package.module))
            );
            assert!(conformance.contains(&format!(
                "use type @id(\"{}\") from {} as ",
                declaration.stable_id, package.module
            )));
        }
        assert!(
            library
                .program
                .functions
                .iter()
                .any(|function| examples.contains(&format!("@id(\"{}\")", function.stable_id))),
            "{}: examples module imports nothing from the library",
            library.path.display()
        );
        let expected_permits: Vec<String> = if package.module == "std.fs" {
            assert_eq!(package.tier, "hosted");
            assert_eq!(required_consumer_profile(&package), "filesystem-io.v2");
            vec!["fs.read".into(), "fs.write".into()]
        } else if package.module == "std.env" {
            assert_eq!(package.tier, "hosted");
            assert_eq!(required_consumer_profile(&package), "environment-io.v1");
            vec!["process.environment.read".into()]
        } else if package.module == "std.process" {
            assert_eq!(package.tier, "hosted");
            assert_eq!(required_consumer_profile(&package), "process-io.v1");
            vec!["process.execute".into()]
        } else {
            vec![]
        };
        assert_eq!(
            library.program.permits,
            expected_permits,
            "{}: library declarations must carry exactly their admitted capabilities",
            library.path.display()
        );
    }
}

#[test]
fn collections_is_the_exact_alloc_tier_transparent_vec_surface() {
    let package = packages()
        .into_iter()
        .find(|package| package.module == "std.collections")
        .expect("std.collections package metadata");
    assert_eq!(package.directory, "collections");
    assert_eq!(package.tier, "alloc");
    assert_eq!(package.status, "partial");
    assert_eq!(package.targets, ["interpreter", "native-c11", "core-wasm"]);

    let (library, entry, tests) = package_sources(&package);
    assert_eq!(entry, "std.collections.examples");
    assert_eq!(tests, "std.collections.tests");
    assert_eq!(library.program.functions.len(), 8);
    let identities = library
        .program
        .functions
        .iter()
        .map(|function| function.stable_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        [
            "std.collections.vec.with-capacity",
            "std.collections.vec.push",
            "std.collections.vec.len",
            "std.collections.vec.capacity",
            "std.collections.vec.get",
            "std.collections.vec.reserve-exact",
            "std.collections.vec.set",
            "std.collections.vec.clear",
        ]
    );
    assert!(library
        .program
        .functions
        .iter()
        .all(|function| function.explicit_id
            && function.type_parameters.len() == 1
            && function.effects.is_empty()));

    let package_root = root().join("std/collections");
    let manifest = std::fs::read_to_string(package_root.join("semaprax.toml")).unwrap();
    assert!(manifest.contains("schema = \"semaprax.project.v8\""));
    assert!(manifest.contains("profile = \"owned-data-api.v1\""));
    assert!(manifest.contains("web_exports = []"));
    let conformance = std::fs::read_to_string(package_root.join("src/tests.spx")).unwrap();
    for scalar in ["i64", "i32", "u8", "usize", "char", "f32", "f64", "bool"] {
        for operation in [
            "with_capacity",
            "push",
            "len",
            "capacity",
            "get",
            "reserve_exact",
            "set",
            "clear",
        ] {
            assert!(
                conformance.contains(&format!("{operation}<{scalar}>")),
                "std.collections conformance does not instantiate {operation}<{scalar}>"
            );
        }
    }
}

#[test]
fn collections_project_check_test_run_without_public_exports() {
    let manifest = root().join("std/collections/semaprax.toml");
    let scratch = temporary("collections-lanes");
    project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        // The historical accessor retains the internal entry-plus-export
        // closure even when there are no exports; it is not itself a public
        // descriptor or target. Pin that internal anchor, then prove below
        // that no public descriptor exists for this package.
        assert_eq!(
            snapshot.public_api_program().entrypoint.as_str(),
            "std.collections.examples.main"
        );
        let descriptor = snapshot
            .public_api_descriptor()
            .expect_err("a no-export package has no public API descriptor");
        assert!(descriptor
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-J105"));
        run_collections_backend_conformance(snapshot, &scratch)?;
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

fn run_collections_backend_conformance(
    snapshot: &project::ProjectRevision,
    scratch: &Path,
) -> Result<(), Vec<semaprax::diagnostic::Diagnostic>> {
    for program in [snapshot.entry_program(), snapshot.test_program()] {
        let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
        assert!(!c.contains("memcpy(result, source"));
        for optimization in ["-O0", "-O2"] {
            compile_and_run_c(&c, scratch, optimization, "0");
        }
        let core = wasm::emit_resolved_module(program).map_err(|error| vec![error])?;
        run_internal_wasm(&core, scratch, 0);
    }
    Ok(())
}

#[test]
fn collections_no_export_manifest_rejects_unauthenticated_package_sources() {
    let source_root = root().join("std/collections");
    for (source, needle, replacement) in [
        (
            "collections.spx",
            "std.collections.vec.get",
            "std.collections.vec.get-lookalike",
        ),
        (
            "examples.spx",
            "std.collections.examples.main",
            "std.collections.examples.lookalike",
        ),
        (
            "tests.spx",
            "std.collections.tests.main",
            "std.collections.tests.lookalike",
        ),
    ] {
        let scratch = temporary(&format!("collections-hostile-{source}"));
        std::fs::create_dir_all(scratch.join("src")).unwrap();
        std::fs::copy(
            source_root.join("semaprax.toml"),
            scratch.join("semaprax.toml"),
        )
        .unwrap();
        for candidate in ["collections.spx", "examples.spx", "tests.spx"] {
            let mut contents =
                std::fs::read_to_string(source_root.join("src").join(candidate)).unwrap();
            if candidate == source {
                contents = contents.replacen(needle, replacement, 1);
            }
            std::fs::write(scratch.join("src").join(candidate), contents).unwrap();
        }
        let errors =
            project::with_authenticated_project(&scratch.join("semaprax.toml"), |_| Ok(()))
                .unwrap_err();
        assert!(errors.iter().any(|diagnostic| {
            diagnostic.code == "SPX-J100"
                && diagnostic
                    .message
                    .contains("authenticated wrapper, example, and conformance sources")
        }));
        let _ = std::fs::remove_dir_all(scratch);
    }
}

fn compile_c(source: &str, output: &Path, optimization: &str) {
    let c_path = output.with_extension("c");
    std::fs::write(&c_path, source).unwrap();
    let result = Command::new("clang")
        .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "clang {optimization} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn run_returns_zero(path: &Path) {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success(), "{} failed", path.display());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "0",
        "{} did not report success",
        path.display()
    );
}

fn run_text_package_wasm_conformance(
    snapshot: &mut project::ProjectSnapshot,
    scratch: &Path,
) -> Result<(), Vec<semaprax::diagnostic::Diagnostic>> {
    let output = scratch.join("text-npm");
    snapshot.build_npm(&output)?;
    let script = scratch.join("text-conformance.mjs");
    std::fs::write(
        &script,
        r#"import assert from "node:assert/strict";
import fs from "node:fs";
import { instantiate } from "./text-npm/semaprax.bindings.js";
const runtime = instantiate(fs.readFileSync("./text-npm/app.wasm"));
assert.equal(runtime.functions["std.text.byte_len"]("a\0é世界"), 10n);
assert.equal(runtime.functions["std.text.byte_len"](""), 0n);
assert.equal(runtime.functions["std.text.is_empty"](""), true);
assert.equal(runtime.functions["std.text.is_empty"]("é"), false);
assert.equal(runtime.functions["std.text.starts_with"]("a\0é世界", "a\0"), true);
assert.equal(runtime.functions["std.text.starts_with"]("a\0", "a\0é世界"), false);
assert.equal(runtime.functions["std.text.contains"]("a\0é世界", "é世"), true);
assert.equal(runtime.functions["std.text.contains"]("a\0é世界", "界a"), false);
assert.equal(runtime.functions["std.text.equals"]("a\0é世界", "a\0é世界"), true);
assert.equal(runtime.functions["std.text.equals"]("a\0é世界", "a\0é世"), false);
assert.equal(runtime.functions["std.text.equals"]("", ""), true);
"#,
    )
    .unwrap();
    let node = Command::new("node")
        .arg(script.file_name().unwrap())
        .current_dir(scratch)
        .output()
        .unwrap();
    assert!(
        node.status.success(),
        "std.text: Node conformance failed: {}",
        String::from_utf8_lossy(&node.stderr)
    );
    Ok(())
}

fn assert_text_interpreter_conformance(snapshot: &project::ProjectSnapshot) {
    use project::{
        PublicApiArgument as Arg, PublicApiEvaluationOutcome as Outcome, PublicApiValue,
    };

    let evaluate = |id: &str, arguments: &[Arg<'_>]| {
        snapshot
            .evaluate_text_api_v1(id, arguments, 1_000)
            .unwrap()
            .outcome
    };
    assert_eq!(
        evaluate("std.text.byte_len", &[Arg::BorrowStr("a\0é世界")]),
        Outcome::Returned(PublicApiValue::I64(10))
    );
    assert_eq!(
        evaluate("std.text.is_empty", &[Arg::BorrowStr("")]),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.starts_with",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0")],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.contains",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("é世")],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.equals",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0é世界"),],
        ),
        Outcome::Returned(PublicApiValue::Bool(true))
    );
    assert_eq!(
        evaluate(
            "std.text.equals",
            &[Arg::BorrowStr("a\0é世界"), Arg::BorrowStr("a\0é世")],
        ),
        Outcome::Returned(PublicApiValue::Bool(false))
    );
}

fn native_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

fn run_text_package_native_conformance(
    snapshot: &project::ProjectSnapshot,
    scratch: &Path,
) -> Result<(), Vec<semaprax::diagnostic::Diagnostic>> {
    let generated = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
    let source = format!(
        "#define SPX_NO_ENTRY_WRAPPER 1\n{generated}\nint main(void) {{\n\
         const uint8_t value_bytes[] = {{0x61,0x00,0xc3,0xa9,0xe4,0xb8,0x96,0xe7,0x95,0x8c}};\n\
         const uint8_t prefix_bytes[] = {{0x61,0x00}};\n\
         const uint8_t middle_bytes[] = {{0xc3,0xa9,0xe4,0xb8,0x96}};\n\
         const uint8_t absent_bytes[] = {{0xe7,0x95,0x8c,0x61}};\n\
         spx_str_v1 value = {{value_bytes, UINT64_C(10)}};\n\
         spx_str_v1 prefix = {{prefix_bytes, UINT64_C(2)}};\n\
         spx_str_v1 middle = {{middle_bytes, UINT64_C(5)}};\n\
         spx_str_v1 absent = {{absent_bytes, UINT64_C(4)}};\n\
         spx_str_v1 empty = {{NULL, UINT64_C(0)}};\n\
         struct spx_status_entry entries[UINT32_C(1)];\n\
         struct spx_context context = {{0}};\n\
         if (!spx_context_init(&context, UINT64_C(1), entries, UINT32_C(1), NULL, NULL, NULL)) return 10;\n\
         int64_t length = -1; bool result = false;\n\
         if ({byte_len}(&context, value, &length) != SPX_STATUS_SUCCESS || length != INT64_C(10)) return 11;\n\
         if ({is_empty}(&context, empty, &result) != SPX_STATUS_SUCCESS || !result) return 12;\n\
         if ({starts_with}(&context, value, prefix, &result) != SPX_STATUS_SUCCESS || !result) return 13;\n\
         if ({contains}(&context, value, middle, &result) != SPX_STATUS_SUCCESS || !result) return 14;\n\
         if ({contains}(&context, value, absent, &result) != SPX_STATUS_SUCCESS || result) return 15;\n\
         if ({equals}(&context, value, value, &result) != SPX_STATUS_SUCCESS || !result) return 16;\n\
         if ({equals}(&context, value, absent, &result) != SPX_STATUS_SUCCESS || result) return 17;\n\
         if ({equals}(&context, empty, empty, &result) != SPX_STATUS_SUCCESS || !result) return 18;\n\
         return 0;\n\
         }}\n",
        byte_len = native_symbol("std.text.byte_len"),
        is_empty = native_symbol("std.text.is_empty"),
        starts_with = native_symbol("std.text.starts_with"),
        contains = native_symbol("std.text.contains"),
        equals = native_symbol("std.text.equals"),
    );
    for optimization in ["-O0", "-O2"] {
        let binary = scratch.join(format!("text-native-{}", optimization.to_lowercase()));
        compile_c(&source, &binary, optimization);
        let executed = Command::new(&binary).output().unwrap();
        assert!(
            executed.status.success(),
            "std.text native conformance {optimization} failed with {:?}",
            executed.status.code()
        );
    }
    Ok(())
}

#[test]
fn examples_and_conformance_return_zero_on_interpreter_native_and_wasm() {
    run_examples_and_conformance(packages());
}

#[test]
fn io_cursors_execute_on_all_three_backends() {
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| p.module == "std.io")
            .collect(),
    );
}

#[test]
fn typed_paths_execute_on_all_three_backends() {
    run_examples_and_conformance(
        packages()
            .into_iter()
            .filter(|p| p.module == "std.path.value")
            .collect(),
    );
}

fn run_examples_and_conformance(selected: Vec<PackageMetadata>) {
    assert!(!selected.is_empty());
    let scratch = temporary("lanes");
    for package in selected {
        if package.module == "std.fs" {
            filesystem::run_conformance();
            filesystem_v2::run_conformance();
            continue;
        }
        if package.module == "std.env" {
            environment::run_conformance();
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
        let manifests = logging::conformance_manifests(&scratch, &manifest, &package.module);
        for manifest in manifests {
            project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.check()?;
            let options = project::ProjectExecutionOptions::default();
            let entry = snapshot.execute_entry(&options)?;
            assert_eq!(
                entry.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "{}: examples failed on the interpreter",
                package.directory
            );
            let tests = snapshot.execute_test(&options)?;
            assert_eq!(
                tests.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "{}: conformance failed on the interpreter",
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
                let c = codegen::emit_hir_c(program).map_err(|error| vec![error])?;
                for optimization in ["-O0", "-O2"] {
                    let binary = scratch.join(format!(
                        "{}-{role}{}",
                        package.directory,
                        optimization.to_lowercase()
                    ));
                    compile_c(&c, &binary, optimization);
                    run_returns_zero(&binary);
                }
            }
            if package.module == "std.text" {
                assert_text_interpreter_conformance(snapshot);
                run_text_package_native_conformance(snapshot, &scratch)?;
                run_text_package_wasm_conformance(snapshot, &scratch)?;
                return Ok(());
            }
            let wasm = snapshot.test_wasm_module()?;
            let cursor_case = json_cursors::is_cursor_case(&manifest);
            // Each fixture must balance its declared live Bytes bound.
            let arena = cursor_case || matches!(package.module.as_str(), "std.data.json.dec" | "std.io" | "std.path.value") || (package.module == "std.format" && formatting::uses_byte_arena(&manifest)) || (package.module == "std.log" && logging::uses_byte_writes(&manifest));
            for name in ["spx_bytes_zeroed", "spx_bytes_set"] {
                let present = wasm.windows(name.len()).any(|w| w == name.as_bytes());
                // Individual typed-Path observation cases allocate via copy
                // without importing the buffer-writing operations.
                if package.module != "std.path.value" && !cursor_case {
                    assert_eq!(present, arena, "{}: `{name}` import", package.directory);
                }
            }
            let live_entry_bound = if package.module == "std.log" { logging::live_byte_bound(&manifest) } else if cursor_case || package.module == "std.format" { 2 } else if package.module == "std.path.value" { 3 } else if arena { 1 } else { 4096 };
            let wasm_path = scratch.join(format!("{}-tests.wasm", package.directory));
            std::fs::write(&wasm_path, wasm).unwrap();
            let script = scratch.join(format!("{}-tests.mjs", package.directory));
            std::fs::write(
                &script,
                format!(
                    r#"import assert from "node:assert/strict";
import {{ readFile }} from "node:fs/promises";
const bytes = await readFile("./{}");
const checked = (operation) => (a, b) => {{ const value = operation(a, b); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; }};
const entries = new Map(); let next = 1; const boxes = new Map(); let nextBox = 1n; let linked;
const decode = carrier => {{ const word = BigInt.asUintN(64, carrier), length = Number(word & 0xffffffffn), root = Number((word >> 32n) & 0xffffffffn); return {{ word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff }}; }};
const read = decoded => {{ if (decoded.tagged) {{ const value = entries.get(decoded.token); if (!(value instanceof Uint8Array) || value.length !== decoded.length) throw new Error("stale byte token"); return value; }} const memory = new Uint8Array((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (decoded.root > memory.length - decoded.length) throw new Error("byte range"); return memory.slice(decoded.root, decoded.root + decoded.length); }};
const allocate = bytes => {{ if (entries.size >= {}) throw new Error("owned Bytes live entry limit exceeded"); const token = next++, owned = new Uint8Array(bytes); entries.set(token, owned); return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(owned.length)); }};
const boxKey = value => {{ if (typeof value !== "bigint" || value === 0n) throw new Error("invalid Box carrier"); return value.toString(); }};
const readBox = (value, tag) => {{ const entry = boxes.get(boxKey(value)); if (!entry || entry.tag !== tag) throw new Error("stale or mistyped Box"); return entry; }};
const imports = {{env:{{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{{throw new Error();}},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{{ const b = read(decode(c)), u = BigInt.asUintN(64, i); return u >= BigInt(b.length) ? -1 : b[Number(u)]; }},spx_bytes_drop:c=>{{ const d = decode(c); read(d); entries.delete(d.token); }},spx_bytes_as_slice:c=>{{ const d = decode(c); read(d); return BigInt.asIntN(64, d.word); }},spx_bytes_zeroed:count=>{{ if (typeof count !== "bigint" || count < 0n || count > 65536n) throw new Error("owned byte buffer capacity invariant"); return allocate(new Uint8Array(Number(count))); }},spx_bytes_set:(c,i,v)=>{{ const d = decode(c), b = read(d); if (typeof i !== "bigint" || i < 0n || i >= BigInt(b.length) || !Number.isInteger(v) || v < 0 || v > 255) throw new Error("owned byte buffer element invariant"); b[Number(i)] = v; return BigInt.asIntN(64, d.word); }},spx_box_new:(tag,bits)=>{{ if (boxes.size >= 4096) return 0n; const token=nextBox++; boxes.set(boxKey(token),{{tag,bits}}); return token; }},spx_box_get:(value,tag)=>readBox(value,tag).bits,spx_box_into_inner:(value,tag)=>{{ const entry=readBox(value,tag); boxes.delete(boxKey(value)); return entry.bits; }},spx_box_drop:value=>{{ if (!boxes.delete(boxKey(value))) throw new Error("double Box drop"); }}}}}};
linked = await WebAssembly.instantiate(bytes, imports);
// Re-entry observes an owned buffer that outlived one call as a live entry.
for (let r = 0; r < 4; ++r) {{ assert.equal(linked.instance.exports.semaprax_main(), 0n); assert.equal(entries.size, 0); assert.equal(boxes.size, 0); }}
"#,
                    wasm_path.file_name().unwrap().to_string_lossy(),
                    live_entry_bound
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
                "{}: Node conformance closure failed: {}",
                package.directory,
                String::from_utf8_lossy(&node.stderr)
            );
            Ok(())
        })
        .unwrap();
        }
    }
    let _ = std::fs::remove_dir_all(scratch);
}

/// Every source-portable standard-library module stands alone when vendored.
/// `std.collections` and `std.mem` use compiler-authenticated transparent generic wrappers.
#[test]
fn every_library_module_is_self_contained_when_vendored() {
    let scratch = temporary("vendored");
    for package in packages() {
        if matches!(package.module.as_str(), "std.collections" | "std.mem") {
            assert_eq!(package.tier, "alloc");
            continue;
        }
        let (library, entry, tests) = package_sources(&package);
        let package_root = root().join("std").join(&package.directory);
        let project = scratch.join(&package.directory);
        std::fs::create_dir_all(project.join("src")).unwrap();
        std::fs::write(project.join("src/vendored.spx"), &library.source).unwrap();
        for file in ["examples.spx", "tests.spx"] {
            std::fs::copy(
                package_root.join("src").join(file),
                project.join("src").join(file),
            )
            .unwrap();
        }
        // Keep the package's own schema, profile, exports, and test module;
        // only the source inventory and name change.
        let manifest = std::fs::read_to_string(package_root.join("semaprax.toml"))
            .unwrap()
            .lines()
            .map(|line| {
                if line.starts_with("sources = ") {
                    "sources = [\"src/examples.spx\", \"src/tests.spx\", \"src/vendored.spx\"]"
                        .to_owned()
                } else if line.starts_with("name = ") {
                    format!("name = \"vendored-{}\"", package.directory)
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert!(manifest.contains(&format!("entry = \"{entry}\"")));
        assert!(manifest.contains(&format!("tests = [\"{tests}\"]")));
        std::fs::write(project.join("semaprax.toml"), manifest).unwrap();
        project::with_authenticated_project(&project.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            let options = project::ProjectExecutionOptions::default();
            for (role, execution) in [
                ("examples", snapshot.execute_entry(&options)?),
                ("conformance", snapshot.execute_test(&options)?),
            ] {
                assert_eq!(
                    execution.outcome(),
                    &project::ProjectExecutionOutcome::Returned(0),
                    "{}: vendored {role} failed",
                    package.directory
                );
            }
            Ok(())
        })
        .unwrap();
    }
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_multiple_bundled_std_packages() {
    let scratch = temporary("manifest-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"std-consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.sign\"]\n\n[dependencies]\nstd.core = \"~0.1.0\"\nstd.encoding = \"=0.1.0\"\nstd.num = \"^0.1.0\"\nstd.random = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/app.spx"),
        "module consumer.app;\nuse function @id(\"std.encoding.decode_hex_byte\") from std.encoding as decode_hex_byte;\nuse function @id(\"std.num.sign\") from std.num as sign;\nuse function @id(\"std.random.sample_below\") from std.random as sample_below;\n\n@id(\"consumer.sign\")\nfn classify(value: i64) -> i64\n{\n    sign(value)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if classify(-4) == -1 && classify(0) == 0 && classify(8) == 1 && decode_hex_byte(52u8, 97u8) == 74 && sample_below(1, 10) == 7 { 0 } else { 1 }\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\nuse function @id(\"std.core.compare\") from std.core as compare;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    if compare(7, 3) == 1 { 0 } else { 1 }\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.core/0.1.0/core.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.num/0.1.0/num.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.encoding/0.1.0/encoding.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.random/0.1.0/random.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_bundled_std_test_and_time() {
    let scratch = temporary("manifest-time-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"time-consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.time\"\nsources = [\"src/tests.spx\", \"src/time.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.remaining\"]\n\n[dependencies]\nstd.test = \"=0.1.0\"\nstd.time = \"~0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/time.spx"),
        "module consumer.time;\nuse function @id(\"std.test.failure_unless\") from std.test as failure_unless;\nuse function @id(\"std.time.remaining_milliseconds\") from std.time as remaining_milliseconds;\n\n@id(\"consumer.remaining\")\nfn remaining(now: i64, deadline: i64) -> i64\n    requires now >= 0 && deadline >= 0\n{\n    remaining_milliseconds(now, deadline)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    failure_unless(remaining(250, 1000) == 750)\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.time/0.1.0/time.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.test/0.1.0/test.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_bundled_std_csv_toml_and_path() {
    let scratch = temporary("manifest-csv-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"csv-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.csv\"\nsources = [\"src/csv.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.csv-fields\"]\n\n[dependencies]\nstd.data.csv = \"^0.1.0\"\nstd.data.toml = \"~0.1.0\"\nstd.path = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/csv.spx"),
        "module consumer.csv;\nuse function @id(\"std.data.csv.field_count\") from std.data.csv as field_count;\nuse function @id(\"std.data.toml.assignment_index\") from std.data.toml as assignment_index;\nuse function @id(\"std.path.segment_count\") from std.path as segment_count;\n\n@id(\"consumer.csv-fields\")\nfn csv_fields(record: borrow Slice<u8>) -> usize\n{\n    field_count(record)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let record = [97u8, 44u8, 98u8];\n    let line = [97u8, 61u8, 49u8];\n    let path = [97u8, 47u8, 98u8];\n    if csv_fields(array_as_slice(record)) == 2usize && assignment_index(array_as_slice(line)) == 1 && segment_count(array_as_slice(path)) == 2usize { 0 } else { 1 }\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.csv/0.1.0/csv.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.toml/0.1.0/toml.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.path/0.1.0/path.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_transitive_std_url_encoding() {
    let scratch = temporary("manifest-url-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"url-consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.url\"\nsources = [\"src/tests.spx\", \"src/url.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.percent-byte\"]\n\n[dependencies]\nstd.url = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/url.spx"),
        "module consumer.url;\nuse function @id(\"std.url.decode_percent_triplet\") from std.url as decode_percent_triplet;\n\n@id(\"consumer.percent-byte\")\nfn percent_byte(marker: u8, high: u8, low: u8) -> i64\n{\n    decode_percent_triplet(marker, high, low)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    if percent_byte(37u8, 50u8, 70u8) == 47 { 0 } else { 1 }\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        let workspace = snapshot.workspace_manifest();
        assert!(workspace.contains("dependencies/std.url/0.1.0/url.spx"));
        assert!(workspace.contains("dependencies/std.encoding/0.1.0/encoding.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_bundled_std_data_json() {
    let scratch = temporary("manifest-json-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.json\"\nsources = [\"src/json.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.json-string\"]\n\n[dependencies]\nstd.data.json = \"^0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/json.spx"),
        "module consumer.json;\nuse function @id(\"std.data.json.is_string\") from std.data.json as is_string;\n\n@id(\"consumer.json-string\")\nfn json_string(input: borrow Slice<u8>) -> bool\n{\n    is_string(input)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let quoted = [34u8, 111u8, 107u8, 34u8];\n    let truncated = [34u8, 111u8, 107u8];\n    if json_string(array_as_slice(quoted)) && !json_string(array_as_slice(truncated)) { 0 } else { 1 }\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json/0.1.0/json.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// The two largest byte-data packages are each about 4.7 KiB, and the consumer
/// imports twenty-four of their functions. While an importing module was
/// charged for each imported contract and body as if it were resolved
/// structure, that closure exhausted the Workspace Semantic Graph pre-bound
/// and reported `SPX-G171`.
#[test]
fn package_manifest_links_two_large_sibling_packages() {
    let scratch = temporary("manifest-two-large-siblings");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"sibling-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.probe\"]\n\n[dependencies]\nstd.bytes = \"=0.1.0\"\nstd.data.json = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\nuse function @id(\"std.bytes.equals\") from std.bytes as equals;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    let left = [1u8, 2u8];\n    let right = [1u8, 2u8];\n    if equals(array_as_slice(left), array_as_slice(right)) { 0 } else { 1 }\n}\n",
    )
    .unwrap();
    let mut app = String::from("module consumer.app;\n");
    for name in [
        "at_in",
        "at_is",
        "code_unit",
        "escape_end",
        "escape_kind",
        "failure",
        "failure_offset",
        "hex_at",
        "is_failure",
        "is_string",
        "skip_whitespace",
        "string_end",
    ] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.{name}\") from std.data.json as {name};\n"
        ));
    }
    for name in [
        "byte_to_i64",
        "count",
        "ends_with",
        "equals",
        "get_or",
        "index_of",
        "is_ascii",
        "read_u16_be",
        "read_u16_le",
        "read_u32_be",
        "read_u32_le",
        "starts_with",
    ] {
        app.push_str(&format!(
            "use function @id(\"std.bytes.{name}\") from std.bytes as {name};\n"
        ));
    }
    app.push_str("\n@id(\"consumer.probe\")\nfn probe(view: borrow Slice<u8>) -> bool\n{\n    is_ascii(view) && is_string(view) && at_is(view, 0usize, 34u8) && at_in(view, 1usize, 32u8, 255u8) && !is_failure(view, string_end(view, 0usize))\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let quoted = [34u8, 111u8, 107u8, 34u8];\n    let view = array_as_slice(quoted);\n    let a = skip_whitespace(view, 0usize) + failure(view, 0usize) + failure_offset(view, 0usize, 0usize) + escape_end(view, 0usize) + count(view, 34u8);\n    let b = hex_at(view, 1usize) + code_unit(view, 1usize) + escape_kind(view, 0usize) + get_or(view, 0usize, 0) + byte_to_i64(34u8);\n    let c = index_of(view, 34u8) + read_u16_be(view, 0usize) + read_u16_le(view, 0usize) + read_u32_be(view, 0usize) + read_u32_le(view, 0usize);\n    let d = equals(view, view) && starts_with(view, view) && ends_with(view, view);\n    if probe(view) && d && a > 0usize && b >= -8 && c >= 0 { 0 } else { 1 }\n}\n");
    std::fs::write(scratch.join("src/app.spx"), app).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.bytes/0.1.0/bytes.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json/0.1.0/json.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// The JSON slice is authored as sibling packages rather than one library
/// module because a single `std/` module large enough to hold the scanner,
/// the number and literal tokens, UTF-8 validation, and the writer exhausts
/// the Workspace Semantic Graph pre-bound. This proves the split composes:
/// one consumer links the string-token scanner and the number/literal token
/// package and reads both kinds of scalar token.
#[test]
fn package_manifest_links_json_scanner_and_token_siblings() {
    let scratch = temporary("manifest-json-token-siblings");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-token-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.scalar\"]\n\n[dependencies]\nstd.data.json = \"=0.1.0\"\nstd.data.json.token = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\nuse function @id(\"std.data.json.token.is_number\") from std.data.json.token as is_number;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    let number = [48u8];\n    if is_number(array_as_slice(number)) { 0 } else { 1 }\n}\n",
    )
    .unwrap();
    let mut app = String::from("module consumer.app;\n");
    for name in ["is_failure", "is_string", "skip_whitespace", "string_end"] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.{name}\") from std.data.json as {name};\n"
        ));
    }
    for name in [
        "i64_or",
        "is_literal",
        "is_number",
        "literal_end",
        "number_end",
    ] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.token.{name}\") from std.data.json.token as {name};\n"
        ));
    }
    app.push_str("\n@id(\"consumer.scalar\")\nfn scalar(view: borrow Slice<u8>) -> bool\n{\n    is_string(view) || is_number(view) || is_literal(view)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let quoted = [34u8, 111u8, 107u8, 34u8];\n    let number = [45u8, 49u8, 50u8, 46u8, 53u8];\n    let word = [116u8, 114u8, 117u8, 101u8];\n    let counted = [52u8, 50u8];\n    let text = array_as_slice(quoted);\n    let digits = array_as_slice(number);\n    let literal = array_as_slice(word);\n    let integer = array_as_slice(counted);\n    if scalar(text) && scalar(digits) && scalar(literal) && string_end(text, 0usize) == 4usize && number_end(digits, 0usize) == 5usize && literal_end(literal, 0usize) == 4usize && i64_or(integer, 0usize, 0) == 42 && skip_whitespace(text, 0usize) == 0usize && !is_failure(text, string_end(text, 0usize)) { 0 } else { 1 }\n}\n");
    std::fs::write(scratch.join("src/app.spx"), app).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json/0.1.0/json.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.token/0.1.0/token.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// The writer half of the same split: the byte-view string encoder and the
/// scalar number and literal encoder are separate packages, and one consumer
/// links both to render a quoted string, an exact `i64`, a `usize` count, and
/// a literal without any value passing through an `f64`.
#[test]
fn package_manifest_links_json_writer_siblings() {
    let scratch = temporary("manifest-json-writer-siblings");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-writer-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.encoded-len\"]\n\n[dependencies]\nstd.data.json.digits = \"=0.1.0\"\nstd.data.json.write = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\nuse function @id(\"std.data.json.digits.i64_len\") from std.data.json.digits as i64_len;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    if i64_len(-9223372036854775808) == 20 { 0 } else { 1 }\n}\n",
    )
    .unwrap();
    let mut app = String::from("module consumer.app;\n");
    for name in ["i64_byte", "i64_len", "literal_byte"] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.digits.{name}\") from std.data.json.digits as {name};\n"
        ));
    }
    for name in [
        "escape_len",
        "quoted_byte",
        "quoted_len",
        "usize_byte",
        "usize_len",
    ] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.write.{name}\") from std.data.json.write as {name};\n"
        ));
    }
    app.push_str("\n@id(\"consumer.encoded-len\")\nfn encoded_len(view: borrow Slice<u8>) -> usize\n{\n    quoted_len(view)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let label = [107u8, 34u8];\n    let view = array_as_slice(label);\n    let quoted = encoded_len(view) == 5usize && quoted_byte(view, 0usize) == 34 && quoted_byte(view, 2usize) == 92 && escape_len(34u8) == 2usize;\n    let rendered = usize_len(1234usize) == 4usize && usize_byte(1234usize, 3usize) == 52 && i64_len(-42) == 3 && i64_byte(-42, 0) == 45 && literal_byte(1, 0) == 116;\n    if quoted && rendered { 0 } else { 1 }\n}\n");
    std::fs::write(scratch.join("src/app.spx"), app).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.digits/0.1.0/digits.spx"));
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.write/0.1.0/write.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// The structural document layer is its own sibling: a consumer links only
/// `std.data.json.doc` and validates a whole JSON document, including the
/// nesting-depth bound, mismatched closers, and trailing bytes.
#[test]
fn package_manifest_links_the_json_document_layer() {
    let scratch = temporary("manifest-json-document");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-document-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.valid\"]\n\n[dependencies]\nstd.data.json.doc = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\nuse function @id(\"std.data.json.doc.is_document\") from std.data.json.doc as is_document;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    let empty = [123u8, 125u8];\n    if is_document(array_as_slice(empty)) { 0 } else { 1 }\n}\n",
    )
    .unwrap();
    let mut app = String::from("module consumer.app;\n");
    for name in ["document_end", "is_document", "whole_end"] {
        app.push_str(&format!(
            "use function @id(\"std.data.json.doc.{name}\") from std.data.json.doc as {name};\n"
        ));
    }
    app.push_str("\n@id(\"consumer.valid\")\nfn valid(view: borrow Slice<u8>) -> bool\n{\n    is_document(view)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let object = [123u8, 34u8, 97u8, 34u8, 58u8, 91u8, 49u8, 93u8, 125u8];\n    let trailing = [123u8, 125u8, 32u8, 120u8];\n    let crossed = [91u8, 49u8, 125u8];\n    let view = array_as_slice(object);\n    let extra = array_as_slice(trailing);\n    let mismatched = array_as_slice(crossed);\n    let accepted = valid(view) && document_end(view, 0usize, 32usize) == 9usize && whole_end(view, 32usize) == byte_len(view);\n    let rejected = !valid(extra) && whole_end(extra, 32usize) - byte_len(extra) - 1usize == 3usize && !valid(mismatched) && document_end(mismatched, 0usize, 32usize) - byte_len(mismatched) - 1usize == 2usize;\n    let bounded = document_end(view, 0usize, 1usize) - byte_len(view) - 1usize == 5usize;\n    if accepted && rejected && bounded { 0 } else { 1 }\n}\n");
    std::fs::write(scratch.join("src/app.spx"), app).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert_eq!(
            snapshot.execute_test(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.doc/0.1.0/doc.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

/// The decoded-string layer is consumable as a bundled dependency. `SPX-W115`
/// keeps the owned buffer out of the consumer's entry program, so the
/// buffer-backed comparison runs from its conformance module instead.
#[test]
fn package_manifest_links_the_json_decoder() {
    let scratch = temporary("manifest-json-decoder");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    for (name, text) in [
        ("semaprax.toml", "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-decoder-consumer\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.matches\"]\n\n[dependencies]\nstd.data.json.dec = \"=0.1.0\"\n"),
        ("src/app.spx", "module consumer.app;\nuse function @id(\"std.data.json.dec.decoded_len\") from std.data.json.dec as decoded_len;\nuse function @id(\"std.data.json.dec.decoded_size\") from std.data.json.dec as decoded_size;\nuse function @id(\"std.data.json.dec.emit_at\") from std.data.json.dec as emit_at;\nuse function @id(\"std.data.json.dec.slice_eq\") from std.data.json.dec as slice_eq;\n\n@id(\"consumer.matches\")\nfn matches(left: borrow Slice<u8>, right: borrow Slice<u8>) -> bool\n{\n    slice_eq(left, right)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    let raw = [34u8, 97u8, 92u8, 110u8, 98u8, 34u8];\n    let source = array_as_slice(raw);\n    let lone = [34u8, 92u8, 117u8, 68u8, 56u8, 51u8, 68u8, 34u8];\n    let broken = array_as_slice(lone);\n    let sized = decoded_size(source) == 3usize && decoded_len(source, 0usize) == 3usize;\n    let streamed = emit_at(source, 1usize, 0usize) == 97 && emit_at(source, 2usize, 0usize) == 10;\n    let rejected = decoded_size(broken) - byte_len(broken) - 1usize == 1usize;\n    if sized && streamed && rejected { 0 } else { 1 }\n}\n"),
        ("src/tests.spx", "module consumer.tests;\nuse function @id(\"std.data.json.dec.decoded_eq\") from std.data.json.dec as decoded_eq;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    let raw = [34u8, 92u8, 117u8, 68u8, 56u8, 51u8, 68u8, 92u8, 117u8, 68u8, 69u8, 48u8, 48u8, 34u8];\n    let source = array_as_slice(raw);\n    let want = [240u8, 159u8, 152u8, 128u8];\n    let expected = array_as_slice(want);\n    if decoded_eq(source, 0usize, expected) { 0 } else { 1 }\n}\n"),
    ] {
        std::fs::write(scratch.join(name), text).unwrap();
    }
    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        let zero = &project::ProjectExecutionOutcome::Returned(0);
        assert_eq!(snapshot.execute_entry(&options)?.outcome(), zero);
        assert_eq!(snapshot.execute_test(&options)?.outcome(), zero);
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.data.json.dec/0.1.0/dec.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn package_manifest_links_borrowed_text_from_std_text() {
    let scratch = temporary("manifest-text-dependency");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"text-consumer\"\nversion = \"0.1.0\"\nprofile = \"useful-text-consumer.v1\"\n\n[modules]\nentry = \"consumer.text\"\nsources = [\"src/tests.spx\", \"src/text.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.empty\"]\n\n[dependencies]\nstd.text = \"=0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/text.spx"),
        "module consumer.text;\nuse function @id(\"std.text.is_empty\") from std.text as text_is_empty;\n\n@id(\"consumer.empty\")\nfn empty(value: borrow str) -> bool\n{\n    text_is_empty(value)\n}\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .evaluate_text_api_v1(
                    "consumer.empty",
                    &[project::PublicApiArgument::BorrowStr("")],
                    1_000,
                )?
                .outcome,
            project::PublicApiEvaluationOutcome::Returned(project::PublicApiValue::Bool(true))
        );
        assert!(snapshot
            .workspace_manifest()
            .contains("dependencies/std.text/0.1.0/text.spx"));
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn bundled_standard_dependency_range_mismatch_fails_closed() {
    let scratch = temporary("manifest-dependency-range");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(
        scratch.join("semaprax.toml"),
        "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"std-consumer\"\nversion = \"0.1.0\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.main\"]\n\n[dependencies]\nstd.core = \"=0.2.0\"\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/tests.spx"),
        "module consumer.tests;\n\n@id(\"consumer.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.join("src/app.spx"),
        "module consumer.app;\n\n@id(\"consumer.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();

    let diagnostics =
        project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
            snapshot.check()
        })
        .unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "SPX-J121", "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].message,
        "dependency `std.core` range `=0.2.0` does not admit bundled version 0.1.0"
    );
    let _ = std::fs::remove_dir_all(scratch);
}

/// The declaration head of one function in canonical source: the `fn` line
/// followed by its `uses`, `requires`, and `ensures` lines, exactly as an
/// agent would write them.
fn declaration_head(source: &str, stable_id: &str) -> Vec<String> {
    let marker = format!("@id(\"{stable_id}\")");
    let mut lines = source.lines().skip_while(|line| *line != marker);
    assert_eq!(lines.next(), Some(marker.as_str()));
    lines
        .take_while(|line| *line != "{")
        .map(str::to_owned)
        .collect()
}

#[path = "standard_library/catalog.rs"]
mod catalog;
use catalog::render_catalogs;

#[test]
fn committed_catalogs_match_the_sources() {
    let (human, agent) = render_catalogs();
    let regenerate = "regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`";
    assert_eq!(
        std::fs::read_to_string(root().join(HUMAN_CATALOG)).unwrap_or_default(),
        human,
        "{HUMAN_CATALOG} is stale; {regenerate}"
    );
    assert_eq!(
        std::fs::read_to_string(root().join(AGENT_CATALOG)).unwrap_or_default(),
        agent,
        "{AGENT_CATALOG} is stale; {regenerate}"
    );
}

#[test]
#[ignore = "writes the generated catalogs; run explicitly after changing std/"]
fn regenerate_catalogs() {
    let (human, agent) = render_catalogs();
    std::fs::write(root().join(HUMAN_CATALOG), human).unwrap();
    std::fs::write(root().join(AGENT_CATALOG), agent).unwrap();
}

#[test]
fn library_modules_verify_inside_their_packages_only() {
    // A library module either has no `main` or imports another package, so a
    // standalone check reports the corresponding workspace-only diagnostic;
    // the owning package route above is the supported verification surface.
    for package in packages() {
        let (library, _, _) = package_sources(&package);
        let diagnostics = verify::verify(&library.program);
        let expected = if library.program.module_uses.is_empty() {
            "SPX-T105"
        } else {
            "SPX-G172"
        };
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == expected),
            "{}: expected standalone diagnostic {expected}, found {diagnostics:?}",
            library.path.display()
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == expected),
            "{}: library module has diagnostics beyond {expected}: {diagnostics:?}",
            library.path.display()
        );
    }
}

#[path = "standard_library/io_cursors.rs"]
mod io_cursors;

#[path = "standard_library/typed_paths.rs"]
mod typed_paths;

#[path = "standard_library/filesystem.rs"]
mod filesystem;
#[path = "standard_library/filesystem_v2.rs"]
mod filesystem_v2;

#[path = "standard_library/json_cursors.rs"]
mod json_cursors;
