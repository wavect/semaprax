//! Legacy Web publication must not succeed with missing String imports.
//! These authored regressions do not execute as part of this implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{hir, wasm};

const DIRECT: &str = r#"module web.direct;
@id("app.main") fn main() -> i64 { string_len("literal") }
"#;
const INSTANCE_ONLY: &str = r#"module web.instance;
@id("app.helper") fn helper<T>(value: T) -> T { let text = "instance-only"; value }
@id("app.main") fn main() -> i64 { helper<i64>(42) }
"#;
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn temporary() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-legacy-web-string-{}-{}",
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

fn remove_file(path: &Path) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    fs::remove_file(path).unwrap();
}

#[test]
fn direct_and_materialized_strings_reject_before_legacy_output_effects() {
    for source in [DIRECT, INSTANCE_ONLY] {
        let program = semaprax::check(source, "legacy-web.spx").unwrap();
        let canonical = semaprax::format::canonical(&program);
        let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
        assert_eq!(
            semaprax::graph::revision(&program),
            semaprax::graph::revision(&reparsed)
        );
        let resolved = hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        if source == INSTANCE_ONLY {
            assert_eq!(resolved.functions.len(), 1);
            assert_eq!(resolved.function_instances.len(), 1);
            assert!(matches!(
                resolved.functions[0].return_type,
                hir::ResolvedType::I64
            ));
        }

        let root = temporary();
        let missing = root.join("missing");
        let error = wasm::build_web(&program, &missing).unwrap_err();
        assert_eq!(error.code, "SPX-W116");
        assert_eq!(
            fs::symlink_metadata(&missing).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_inventory(&root, &[]);

        let existing = root.join("existing");
        fs::create_dir(&existing).unwrap();
        fs::write(existing.join("sentinel"), b"foreign bytes").unwrap();
        let error = wasm::build_web(&program, &existing).unwrap_err();
        assert_eq!(error.code, "SPX-W116");
        assert_inventory(&existing, &["sentinel"]);
        assert_eq!(
            fs::read(existing.join("sentinel")).unwrap(),
            b"foreign bytes"
        );

        let input = root.join("input.spx");
        fs::write(&input, &canonical).unwrap();
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
            assert!(stderr.contains("SPX-W116"), "{stderr}");
            assert!(stderr.contains("--profile internal-strings-v1"), "{stderr}");
            assert!(stderr.contains("admitted scalar exports"), "{stderr}");
            assert_eq!(
                fs::symlink_metadata(&missing).unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );
        }
        assert_inventory(&root, &["existing", "input.spx"]);
        remove_file(&input);
        remove_file(&existing.join("sentinel"));
        fs::remove_dir(&existing).unwrap();
        fs::remove_dir(&root).unwrap();
    }
}

#[test]
fn raw_string_emission_remains_a_separate_unmodified_route() {
    let program = semaprax::check(DIRECT, "raw-string.spx").unwrap();
    let bytes = wasm::emit_module(&program).unwrap();
    let mut found = false;
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                let import = import.unwrap();
                found |= import.module == "env" && import.name == "spx_string_new";
            }
        }
    }
    assert!(found, "raw String import ABI was silently rerouted");
}
