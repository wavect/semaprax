//! Executable gate for the standard library slice under `std/`.
//!
//! Every package below `std/` is an ordinary Project v1 whose entry module is
//! its examples module and whose single test module is its conformance suite.
//! This module proves, for each package, that the library sources are
//! canonical, that every public declaration carries a `std.`-prefixed stable
//! identity and is exercised by the conformance module, that examples and
//! conformance return `0` on the interpreter, native C11 at O0 and O2, and
//! Core Wasm under Node, and that the committed human and agent catalogs are
//! exactly what the sources generate. [Standard Library v1] owns the contract.
//!
//! [Standard Library v1]: ../../docs/STANDARD-LIBRARY-V1.md

use std::path::{Path, PathBuf};
use std::process::Command;

use semaprax::{codegen, format, parse, project, verify};

const PACKAGES: &str = "std/packages.json";
const AGENT_CATALOG: &str = "std/catalog.json";
const HUMAN_CATALOG: &str = "docs/STANDARD-LIBRARY-CATALOG.md";
const CATALOG_SCHEMA: &str = "semaprax.standard-library-catalog.v1";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn temporary(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-standard-library-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

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
        let program = parse(&source, &path).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(
            format::canonical(&program),
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
            assert!(
                function.effects.is_empty(),
                "{}: `{}` declares effects, which the core tier forbids",
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
        assert!(
            library
                .program
                .functions
                .iter()
                .any(|function| examples.contains(&format!("@id(\"{}\")", function.stable_id))),
            "{}: examples module imports nothing from the library",
            library.path.display()
        );
        assert!(
            library.program.types.is_empty() && library.program.permits.is_empty(),
            "{}: the core-tier slice admits functions only",
            library.path.display()
        );
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

#[test]
fn examples_and_conformance_return_zero_on_interpreter_native_and_wasm() {
    let scratch = temporary("lanes");
    for package in packages() {
        let manifest = root()
            .join("std")
            .join(&package.directory)
            .join("semaprax.toml");
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
            let wasm = snapshot.test_wasm_module()?;
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
const imports = {{env:{{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{{throw new Error();}}}}}};
const linked = await WebAssembly.instantiate(bytes, imports);
assert.equal(linked.instance.exports.semaprax_main(), 0n);
"#,
                    wasm_path.file_name().unwrap().to_string_lossy()
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

fn render_catalogs() -> (String, String) {
    let mut human = String::new();
    human.push_str("# Standard library catalog\n\n");
    human.push_str(
        "Status: generated from `std/` by `tests/project.rs::standard_library`; edit the sources, then regenerate with `cargo test --locked -p semaprax --test project -- --ignored standard_library::regenerate_catalogs`.\n\n",
    );
    human.push_str("Audience: agents and humans choosing a standard-library declaration.\n\n");
    human.push_str(
        "Every declaration below is verified, canonical, and executed by its package's conformance module on the interpreter, native C11, and Core Wasm lanes. [Standard Library v1](STANDARD-LIBRARY-V1.md) owns the contract; `std/catalog.json` is the same catalog for tools.\n",
    );
    let mut modules = Vec::new();
    for package in packages() {
        let (library, _, _) = package_sources(&package);
        human.push_str(&format!(
            "\n## `{}`\n\nPackage `std/{}`, tier `{}`, status {}. Targets: {}.\n",
            package.module,
            package.directory,
            package.tier,
            package.status,
            package
                .targets
                .iter()
                .map(|target| format!("`{target}`"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let mut declarations = Vec::new();
        for function in &library.program.functions {
            let head = declaration_head(&library.source, &function.stable_id);
            human.push_str(&format!(
                "\n### `{}`\n\n```semaprax\n{}\n```\n",
                function.stable_id,
                head.join("\n")
            ));
            declarations.push(serde_json::json!({
                "id": function.stable_id,
                "kind": "function",
                "name": function.name,
                "head": head,
                "effects": function.effects,
                "requires": function.requires.len(),
                "ensures": function.ensures.len(),
            }));
        }
        modules.push(serde_json::json!({
            "module": package.module,
            "package": format!("std/{}", package.directory),
            "tier": package.tier,
            "targets": package.targets,
            "status": package.status,
            "declarations": declarations,
        }));
    }
    let agent = serde_json::json!({
        "schema": CATALOG_SCHEMA,
        "modules": modules,
    });
    (
        human,
        format!("{}\n", serde_json::to_string_pretty(&agent).unwrap()),
    )
}

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
    // A library module has no `main`, so a standalone check reports the
    // executable-module diagnostic; the package route is the supported one.
    for package in packages() {
        let (library, _, _) = package_sources(&package);
        let diagnostics = verify::verify(&library.program);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T105"),
            "{}: expected the standalone executable-module diagnostic, found {diagnostics:?}",
            library.path.display()
        );
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code == "SPX-T105"),
            "{}: library module has diagnostics beyond the standalone entry rule: {diagnostics:?}",
            library.path.display()
        );
    }
}
