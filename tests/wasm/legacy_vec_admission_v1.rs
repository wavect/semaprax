//! Legacy Web publication must not succeed with imports its loader lacks.
//! The import-inventory rule is recorded in the scalar export specification.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::wasm;

const ITERATOR_OPERATIONS: &str = include_str!("../../examples/iterator-operations.spx");
const LAZY_ITERATOR_ADAPTERS: &str = include_str!("../../examples/lazy-iterator-adapters.spx");
const VEC_EXAMPLES: [(&str, &str); 2] = [
    ("iterator-operations.spx", ITERATOR_OPERATIONS),
    ("lazy-iterator-adapters.spx", LAZY_ITERATOR_ADAPTERS),
];
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn temporary() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-legacy-web-vec-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

fn assert_inventory(directory: &Path, names: &[&str]) {
    let mut observed = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect::<Vec<_>>();
    observed.sort();
    let mut expected = names.to_vec();
    expected.sort();
    assert_eq!(observed, expected);
}

fn assert_missing(path: &Path) {
    assert_eq!(
        fs::symlink_metadata(path).unwrap_err().kind(),
        std::io::ErrorKind::NotFound
    );
}

fn imports(bytes: &[u8]) -> Vec<(String, String, bool)> {
    let mut found = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(bytes) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                let import = import.unwrap();
                found.push((
                    import.module.to_owned(),
                    import.name.to_owned(),
                    matches!(import.ty, wasmparser::TypeRef::Func(_)),
                ));
            }
        }
    }
    found
}

#[test]
fn vec_imports_reject_before_legacy_web_output_effects() {
    for (name, source) in VEC_EXAMPLES {
        let program = semaprax::check(source, name).unwrap();
        let canonical = semaprax::format::canonical(&program);
        let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
        assert_eq!(
            semaprax::graph::revision(&program),
            semaprax::graph::revision(&reparsed)
        );

        let root = temporary();
        let missing = root.join("missing");
        let error = wasm::build_web(&program, &missing).unwrap_err();
        assert_eq!(error.code, "SPX-W131", "{name}");
        assert!(
            error
                .message
                .contains("env.spx_vec_with_capacity (function)"),
            "{}",
            error.message
        );
        assert_missing(&missing);
        assert_inventory(&root, &[]);

        let existing = root.join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("sentinel"), b"foreign bytes").unwrap();
        let error = wasm::build_web(&program, &existing).unwrap_err();
        assert_eq!(error.code, "SPX-W131", "{name}");
        assert_inventory(&existing, &["sentinel"]);
        assert_eq!(
            fs::read(existing.join("sentinel")).unwrap(),
            b"foreign bytes"
        );

        let input = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join(name);
        for target in ["web", "wasm"] {
            let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
                .arg("build")
                .arg(&input)
                .args(["--target", target, "-o"])
                .arg(&missing)
                .output()
                .unwrap();
            assert!(!result.status.success());
            let stderr = String::from_utf8(result.stderr).unwrap();
            assert!(stderr.contains("SPX-W131"), "{stderr}");
            assert!(stderr.contains("spx_vec_with_capacity"), "{stderr}");
            assert_missing(&missing);

            let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
                .arg("build")
                .arg(&input)
                .args(["--target", target, "--json", "-o"])
                .arg(&missing)
                .output()
                .unwrap();
            assert!(!result.status.success());
            let stdout = String::from_utf8(result.stdout).unwrap();
            let stderr = String::from_utf8(result.stderr).unwrap();
            assert!(!stdout.contains("\"status\":\"built\""), "{stdout}");
            // --json diagnostics are JSON lines on stdout (docs/DEVELOPMENT.md).
            assert!(stdout.contains("\"code\":\"SPX-W131\""), "{stdout}{stderr}");
            assert_missing(&missing);
        }
        assert_inventory(&root, &["existing"]);
        fs::remove_file(existing.join("sentinel")).unwrap();
        fs::remove_dir(&existing).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}

#[test]
fn raw_vec_emission_remains_a_separate_unmodified_route() {
    let program = semaprax::check(ITERATOR_OPERATIONS, "raw-vec.spx").unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    assert!(
        imports(&bytes)
            .iter()
            .any(|(module, name, function)| module == "env"
                && name == "spx_vec_with_capacity"
                && *function),
        "raw Vec import ABI was silently rerouted"
    );
}

#[test]
fn scalar_calculator_still_builds_a_loadable_legacy_web_package() {
    let root = temporary();
    let output = root.join("calculator");
    let input = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator.spx");
    let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("build")
        .arg(&input)
        .args(["--target", "web", "-o"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_inventory(
        &output,
        &[
            "app.wasm",
            "index.html",
            "package.json",
            "semaprax.js",
            "semaprax.manifest.json",
        ],
    );
    let bytes = fs::read(output.join("app.wasm")).unwrap();
    let observed = imports(&bytes);
    assert!(!observed.is_empty());
    assert!(observed
        .iter()
        .all(|(module, _, function)| module == "env" && *function));
    if Command::new("node").arg("--version").output().is_ok() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap().trim(), "42");
    }
    for entry in fs::read_dir(&output).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    fs::remove_dir(&output).unwrap();
    fs::remove_dir(&root).unwrap();
}

#[test]
fn emission_failure_leaves_no_legacy_web_output() {
    let source = include_str!("../../examples/http_app_routing.spx");
    let program = semaprax::check(source, "http_app_routing.spx").unwrap();
    let root = temporary();
    let missing = root.join("missing");
    let error = wasm::build_web(&program, &missing).unwrap_err();
    assert_eq!(error.code, "SPX-W110");
    assert_missing(&missing);
    assert_inventory(&root, &[]);
    fs::remove_dir(&root).unwrap();
}
