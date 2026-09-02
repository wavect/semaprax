use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{diagnostic::quote_json, graph, hir, parse, verify, wasm};
use sha2::{Digest, Sha256};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, ValType};

const CALCULATOR: &str = include_str!("../../examples/calculator.spx");
const EXPORT_IDS: &[&str] = &[
    "calculator.add",
    "calculator.divide",
    "calculator.is-negative",
    "calculator.multiply",
    "calculator.not",
    "calculator.subtract",
];

const PACKAGE_INVENTORY: &[&str] = &[
    "app.wasm",
    "index.html",
    "package.json",
    "semaprax.bindings.d.ts",
    "semaprax.bindings.js",
    "semaprax.js",
    "semaprax.scalar-exports.json",
];

static TEMP_ORDINAL: AtomicU64 = AtomicU64::new(0);

fn parsed(source: &str) -> semaprax::ast::Program {
    let program = parse(source, Path::new("scalar-exports-v1.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.is_empty(),
        "verification failed: {}",
        diagnostics
            .iter()
            .map(|item| format!("{}: {}", item.code, item.message))
            .collect::<Vec<_>>()
            .join("; ")
    );
    program
}

fn selected() -> Vec<String> {
    EXPORT_IDS.iter().map(|id| (*id).to_owned()).collect()
}

fn temporary(label: &str) -> PathBuf {
    let ordinal = TEMP_ORDINAL.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-scalar-v1-{label}-{}-{ordinal}",
        std::process::id()
    ))
}

fn raw_symbol(id: &str) -> String {
    let mut result = String::from("spx_scalar_");
    for byte in id.bytes() {
        result.push_str(&format!("{byte:02x}"));
    }
    result
}

fn function_exports(bytes: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for payload in Parser::new(0).parse_all(bytes) {
        if let Payload::ExportSection(section) = payload.unwrap() {
            for export in section {
                let export = export.unwrap();
                if export.kind == ExternalKind::Func {
                    names.push(export.name.to_owned());
                }
            }
        }
    }
    names
}

#[derive(Debug, Eq, PartialEq)]
struct WasmInventory {
    types: Vec<(Vec<ValType>, Vec<ValType>)>,
    imports: Vec<(String, String, u32)>,
    function_types: Vec<u32>,
    exports: Vec<(String, ExternalKind, u32)>,
    has_memory: bool,
    has_table: bool,
    has_global: bool,
    has_start: bool,
}

fn wasm_inventory(bytes: &[u8]) -> WasmInventory {
    let mut inventory = WasmInventory {
        types: Vec::new(),
        imports: Vec::new(),
        function_types: Vec::new(),
        exports: Vec::new(),
        has_memory: false,
        has_table: false,
        has_global: false,
        has_start: false,
    };
    for payload in Parser::new(0).parse_all(bytes) {
        match payload.unwrap() {
            Payload::TypeSection(section) => {
                inventory
                    .types
                    .extend(section.into_iter_err_on_gc_types().map(|ty| {
                        let ty = ty.unwrap();
                        (ty.params().to_vec(), ty.results().to_vec())
                    }))
            }
            Payload::ImportSection(section) => {
                inventory
                    .imports
                    .extend(section.into_imports().map(|import| {
                        let import = import.unwrap();
                        let TypeRef::Func(type_index) = import.ty else {
                            panic!("scalar profile imported a non-function item")
                        };
                        (import.module.to_owned(), import.name.to_owned(), type_index)
                    }));
            }
            Payload::FunctionSection(section) => inventory
                .function_types
                .extend(section.into_iter().map(Result::unwrap)),
            Payload::ExportSection(section) => {
                inventory.exports.extend(section.into_iter().map(|export| {
                    let export = export.unwrap();
                    (export.name.to_owned(), export.kind, export.index)
                }));
            }
            Payload::MemorySection(_) => inventory.has_memory = true,
            Payload::TableSection(_) => inventory.has_table = true,
            Payload::GlobalSection(_) => inventory.has_global = true,
            Payload::StartSection { .. } => inventory.has_start = true,
            _ => {}
        }
    }
    inventory
}

fn package_files(directory: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            assert!(entry.file_type().unwrap().is_file());
            (
                entry.file_name().into_string().unwrap(),
                std::fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn sha256(bytes: &[u8]) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn expected_function_manifest() -> String {
    let facts: &[(&str, &[&str], &str)] = &[
        ("calculator.add", &["i64", "i64"], "i64"),
        ("calculator.divide", &["i64", "i64"], "i64"),
        ("calculator.is-negative", &["i64"], "bool"),
        ("calculator.multiply", &["i64", "i64"], "i64"),
        ("calculator.not", &["bool"], "bool"),
        ("calculator.subtract", &["i64", "i64"], "i64"),
    ];
    facts
        .iter()
        .map(|(id, params, result)| {
            format!(
                "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
                quote_json(id),
                quote_json(&raw_symbol(id)),
                params
                    .iter()
                    .map(|param| quote_json(param))
                    .collect::<Vec<_>>()
                    .join(","),
                quote_json(result),
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn replay_calculator_manifest(directory: &Path) -> Result<(), String> {
    let files = package_files(directory);
    let observed_inventory = files.keys().map(String::as_str).collect::<Vec<_>>();
    if observed_inventory != PACKAGE_INVENTORY {
        return Err(format!(
            "unexpected package inventory: {observed_inventory:?}"
        ));
    }
    let observed = std::str::from_utf8(&files["semaprax.scalar-exports.json"])
        .map_err(|error| error.to_string())?;
    let _: serde_json::Value = serde_json::from_str(observed).map_err(|error| error.to_string())?;
    let program = parsed(CALCULATOR);
    let artifacts = [
        "app.wasm",
        "index.html",
        "package.json",
        "semaprax.bindings.d.ts",
        "semaprax.bindings.js",
        "semaprax.js",
    ];
    let artifact_rows = artifacts
        .iter()
        .map(|path| {
            format!(
                "{{\"path\":{},\"sha256\":{}}}",
                quote_json(path),
                quote_json(&sha256(&files[*path])),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let expected = format!(
        "{{\"schema\":\"semaprax.web.v4\",\"module\":{},\"graph_revision\":{},\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(&program.module),
        quote_json(&graph::revision(&program)),
        artifact_rows,
        expected_function_manifest(),
    );
    if observed != expected {
        return Err("manifest is not the exact independently replayed canonical form".to_owned());
    }
    Ok(())
}

#[test]
fn scalar_profile_package_is_deterministic_exact_and_node_executable() {
    let program = parsed(CALCULATOR);
    let first = temporary("package-a");
    let second = temporary("package-b");
    let mut reversed = selected();
    reversed.reverse();

    wasm::build_web_with_scalar_exports(&program, &first, &reversed).unwrap();
    wasm::build_web_with_scalar_exports(&program, &second, &selected()).unwrap();
    let first_files = package_files(&first);
    assert_eq!(first_files, package_files(&second));
    assert_eq!(
        first_files.keys().map(String::as_str).collect::<Vec<_>>(),
        vec![
            "app.wasm",
            "index.html",
            "package.json",
            "semaprax.bindings.d.ts",
            "semaprax.bindings.js",
            "semaprax.js",
            "semaprax.scalar-exports.json",
        ]
    );

    let expected = EXPORT_IDS
        .iter()
        .map(|id| raw_symbol(id))
        .collect::<Vec<_>>();
    assert_eq!(function_exports(&first_files["app.wasm"]), expected);
    assert!(!function_exports(&first_files["app.wasm"])
        .iter()
        .any(|name| name == "semaprax_main"));

    let manifest: serde_json::Value =
        serde_json::from_slice(&first_files["semaprax.scalar-exports.json"]).unwrap();
    assert_eq!(manifest["schema"], "semaprax.web.v4");
    assert_eq!(manifest["scalar_abi"]["schema"], "semaprax.wasm-scalar.v1");
    assert_eq!(
        manifest["scalar_abi"]["functions"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let artifacts = manifest["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 6);
    for artifact in artifacts {
        let path = artifact["path"].as_str().unwrap();
        assert_eq!(
            artifact["sha256"].as_str().unwrap(),
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(&first_files[path]))
            ),
            "manifest digest mismatch for {path}"
        );
    }
    assert!(
        String::from_utf8_lossy(&first_files["semaprax.bindings.d.ts"])
            .contains("readonly \"calculator.add\"")
    );

    if Command::new("node").arg("--version").output().is_ok() {
        let verifier =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-wasm-scalar-exports.mjs");
        let output = Command::new("node")
            .arg(verifier)
            .arg(&first)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "node failed:\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "scalar-exports-v1-ok"
        );
    }

    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn display_rename_preserves_wasm_and_public_bindings() {
    let original = parsed(CALCULATOR);
    let renamed_source = CALCULATOR
        .replace("fn add(", "fn sum(")
        .replace("add(19, 23)", "sum(19, 23)")
        .replace("fn subtract(", "fn difference(")
        .replace("fn is_negative(", "fn below_zero(");
    let renamed = parsed(&renamed_source);

    let original_wasm = wasm::emit_module_with_scalar_exports(&original, &selected()).unwrap();
    let renamed_wasm = wasm::emit_module_with_scalar_exports(&renamed, &selected()).unwrap();
    assert_eq!(original_wasm, renamed_wasm);
    assert_eq!(
        function_exports(&original_wasm),
        function_exports(&renamed_wasm)
    );

    let first = temporary("rename-a");
    let second = temporary("rename-b");
    wasm::build_web_with_scalar_exports(&original, &first, &selected()).unwrap();
    wasm::build_web_with_scalar_exports(&renamed, &second, &selected()).unwrap();
    for artifact in ["app.wasm", "semaprax.bindings.js", "semaprax.bindings.d.ts"] {
        assert_eq!(
            std::fs::read(first.join(artifact)).unwrap(),
            std::fs::read(second.join(artifact)).unwrap(),
            "display rename changed {artifact}"
        );
    }
    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn existing_destination_is_rejected_without_clobber() {
    let program = parsed(CALCULATOR);
    let output = temporary("no-clobber");
    std::fs::create_dir(&output).unwrap();
    std::fs::write(output.join("foreign.txt"), b"foreign bytes").unwrap();

    let error = wasm::build_web_with_scalar_exports(&program, &output, &selected()).unwrap_err();
    assert_eq!(error.code, "SPX-I307");
    assert_eq!(
        package_files(&output),
        BTreeMap::from([("foreign.txt".to_owned(), b"foreign bytes".to_vec())])
    );
    let _ = std::fs::remove_dir_all(output);
}

#[cfg(unix)]
#[test]
fn destination_symlink_is_rejected_without_writing_through_it() {
    use std::os::unix::fs::symlink;

    let program = parsed(CALCULATOR);
    let root = temporary("symlink-no-clobber");
    let foreign = root.join("foreign");
    let output = root.join("output");
    std::fs::create_dir_all(&foreign).unwrap();
    symlink(&foreign, &output).unwrap();
    let error = wasm::build_web_with_scalar_exports(&program, &output, &selected()).unwrap_err();
    assert_eq!(error.code, "SPX-I307");
    assert!(package_files(&foreign).is_empty());
    assert!(std::fs::symlink_metadata(&output)
        .unwrap()
        .file_type()
        .is_symlink());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn profile_rejects_automatic_aggregate_duplicate_and_hostile_id_admission() {
    let automatic = parse(
        "module scalar.auto; fn generated(value: i64) -> i64 { value } @id(\"app.main\") fn main() -> i64 { 0 }",
        Path::new("scalar-automatic.spx"),
    )
    .unwrap();
    assert!(verify::verify(&automatic)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S103"));
    let resolved = hir::resolve(&automatic).unwrap();
    let automatic_id = resolved.functions[0].id.as_str().to_owned();
    let error = wasm::emit_module_with_scalar_exports(&automatic, &[automatic_id]).unwrap_err();
    assert_eq!(error.code, "SPX-W115");
    assert!(error.message.contains("lowercase [a-z0-9._-]"));

    let aggregate = parsed(
        "module scalar.aggregate; @id(\"value.type\") record Value { @id(\"value.field\") field: i64, } @id(\"scalar.read\") fn read(value: Value) -> i64 { value.field } @id(\"app.main\") fn main() -> i64 { 0 }",
    );
    let error =
        wasm::emit_module_with_scalar_exports(&aggregate, &["scalar.read".to_owned()]).unwrap_err();
    assert_eq!(error.code, "SPX-W115");
    assert!(error
        .message
        .contains("authored resource, record, or variant"));

    let program = parsed(CALCULATOR);
    let error = wasm::emit_module_with_scalar_exports(
        &program,
        &["calculator.add".to_owned(), "calculator.add".to_owned()],
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W115");
    assert!(error.message.contains("more than once"));

    let hostile = parsed(
        "module scalar.hostile; @id(\"Hostile ID!\") fn hostile(value: i64) -> i64 { value } @id(\"app.main\") fn main() -> i64 { 0 }",
    );
    let error =
        wasm::emit_module_with_scalar_exports(&hostile, &["Hostile ID!".to_owned()]).unwrap_err();
    assert_eq!(error.code, "SPX-W115");
    assert!(error.message.contains("lowercase [a-z0-9._-]"));
}

#[test]
fn scalar_profile_normalizes_every_frozen_arithmetic_and_contract_status() {
    let program = parsed(
        r#"module scalar.failures;
@id("app.main") fn main() -> i64 { 0 }
@id("failure.add") fn add(a: i64, b: i64) -> i64 { a + b }
@id("failure.sub") fn sub(a: i64, b: i64) -> i64 { a - b }
@id("failure.mul") fn mul(a: i64, b: i64) -> i64 { a * b }
@id("failure.div") fn div(a: i64, b: i64) -> i64 { a / b }
@id("failure.rem") fn rem(a: i64, b: i64) -> i64 { a % b }
@id("failure.neg") fn neg(value: i64) -> i64 { -value }
@id("failure.requires") fn requires_positive(value: i64) -> i64 requires value > 0 { value }
@id("failure.ensures") fn ensures_positive(value: i64) -> i64 ensures result > 0 { value }
"#,
    );
    let ids = [
        "failure.add",
        "failure.sub",
        "failure.mul",
        "failure.div",
        "failure.rem",
        "failure.neg",
        "failure.requires",
        "failure.ensures",
    ]
    .map(str::to_owned);
    let output = temporary("all-statuses");
    wasm::build_web_with_scalar_exports(&program, &output, &ids).unwrap();
    if Command::new("node").arg("--version").output().is_ok() {
        let script = output.join("verify-all-statuses.mjs");
        std::fs::write(
            &script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instantiateBytes } from "./semaprax.bindings.js";
const runtime = await instantiateBytes(await readFile("./app.wasm"));
const arithmetic = (code) => ({ ok:false, status:{ schema:"semaprax.status.v1", domain_id:"semaprax.arithmetic.v1", code } });
const contract = (code) => ({ ok:false, status:{ schema:"semaprax.status.v1", domain_id:"semaprax.contract.v1", code } });
const min = -(1n << 63n); const max = (1n << 63n) - 1n;
assert.deepEqual(runtime.call("failure.add", max, 1n), arithmetic(1));
assert.deepEqual(runtime.call("failure.sub", min, 1n), arithmetic(2));
assert.deepEqual(runtime.call("failure.mul", max, 2n), arithmetic(3));
assert.deepEqual(runtime.call("failure.div", 1n, 0n), arithmetic(4));
assert.deepEqual(runtime.call("failure.div", min, -1n), arithmetic(5));
assert.deepEqual(runtime.call("failure.rem", 1n, 0n), arithmetic(6));
assert.deepEqual(runtime.call("failure.rem", min, -1n), arithmetic(7));
assert.deepEqual(runtime.call("failure.neg", min), arithmetic(8));
assert.deepEqual(runtime.call("failure.requires", 0n), contract(1));
assert.deepEqual(runtime.call("failure.ensures", 0n), contract(2));
"#,
        )
        .unwrap();
        let result = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "node all-status proof failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn scalar_profile_export_count_boundary_is_exact() {
    let mut source = String::from("module scalar.capacity;\n");
    let mut ids = Vec::new();
    for index in 0..33 {
        let id = format!("capacity.f{index:02}");
        let name = if index == 0 {
            "main".to_owned()
        } else {
            format!("function_{index:02}")
        };
        source.push_str(&format!("@id(\"{id}\") fn {name}() -> i64 {{ {index} }}\n"));
        ids.push(id);
    }
    let program = parsed(&source);
    assert!(wasm::emit_module_with_scalar_exports(&program, &ids[..32]).is_ok());
    let error = wasm::emit_module_with_scalar_exports(&program, &ids).unwrap_err();
    assert_eq!(error.code, "SPX-W116");
}

#[test]
fn hostile_but_admitted_ids_keep_bytewise_order_and_null_prototype_lookup() {
    let program = parsed(
        r#"module scalar.ids;
@id("app.main") fn main() -> i64 { 0 }
@id("__proto__") fn proto(value: i64) -> i64 { value }
@id("10") fn ten(value: i64) -> i64 { value }
@id("2") fn two(value: i64) -> i64 { value }
"#,
    );
    let output = temporary("hostile-ids");
    let ids = ["2", "__proto__", "10"].map(str::to_owned);
    wasm::build_web_with_scalar_exports(&program, &output, &ids).unwrap();
    if Command::new("node").arg("--version").output().is_ok() {
        let script = output.join("verify-hostile-ids.mjs");
        std::fs::write(
            &script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { exportIds, instantiateBytes } from "./semaprax.bindings.js";
assert.deepEqual(exportIds, ["10", "2", "__proto__"]);
const runtime = await instantiateBytes(await readFile("./app.wasm"));
assert.deepEqual(runtime.functions["__proto__"](7n), { ok:true, value:7n });
"#,
        )
        .unwrap();
        let result = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "hostile ID proof failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn selected_wrapper_wasm_has_exact_types_imports_and_exports() {
    let program = parsed(CALCULATOR);
    let bytes = wasm::emit_module_with_scalar_exports(&program, &selected()).unwrap();
    let inventory = wasm_inventory(&bytes);

    assert_eq!(
        inventory.types,
        vec![
            (vec![ValType::I64, ValType::I64], vec![ValType::I64]),
            (vec![ValType::I64], vec![ValType::I64]),
            (vec![ValType::I32], vec![]),
            (vec![ValType::I64], vec![ValType::I32]),
            (vec![ValType::I32], vec![ValType::I32]),
            (vec![], vec![ValType::I64]),
        ]
    );
    assert_eq!(
        inventory.imports,
        [
            ("spx_add", 0),
            ("spx_sub", 0),
            ("spx_mul", 0),
            ("spx_div", 0),
            ("spx_rem", 0),
            ("spx_neg", 1),
            ("spx_contract_fail", 2),
        ]
        .map(|(name, ty)| ("env".to_owned(), name.to_owned(), ty))
    );
    assert_eq!(
        inventory.function_types,
        vec![0, 0, 0, 0, 3, 4, 5, 0, 0, 3, 0, 4, 0]
    );
    assert_eq!(
        inventory.exports,
        EXPORT_IDS
            .iter()
            .enumerate()
            .map(|(ordinal, id)| {
                (
                    raw_symbol(id),
                    ExternalKind::Func,
                    14 + u32::try_from(ordinal).unwrap(),
                )
            })
            .collect::<Vec<_>>()
    );
    assert!(!inventory.has_memory);
    assert!(!inventory.has_table);
    assert!(!inventory.has_global);
    assert!(!inventory.has_start);
}

#[test]
fn canonical_manifest_replay_rejects_structural_and_digest_drift() {
    let program = parsed(CALCULATOR);
    let output = temporary("manifest-replay");
    wasm::build_web_with_scalar_exports(&program, &output, &selected()).unwrap();
    replay_calculator_manifest(&output).unwrap();

    let manifest_path = output.join("semaprax.scalar-exports.json");
    let original = std::fs::read_to_string(&manifest_path).unwrap();
    let cases = [
        original.replacen(
            "{\"schema\":\"semaprax.web.v4\",",
            "{\"schema\":\"semaprax.web.v4\",\"schema\":\"semaprax.web.v4\",",
            1,
        ),
        original.replacen(
            "{\"schema\":\"semaprax.web.v4\",\"module\":\"examples.calculator\",",
            "{\"module\":\"examples.calculator\",\"schema\":\"semaprax.web.v4\",",
            1,
        ),
        original.replacen(
            "{\"schema\":\"semaprax.web.v4\",",
            "{\"schema\":\"semaprax.web.v4\",\"unknown\":0,",
            1,
        ),
        original.replacen("\"capabilities\":[],", "", 1),
    ];
    for mutation in cases {
        assert_ne!(mutation, original);
        std::fs::write(&manifest_path, mutation).unwrap();
        assert!(replay_calculator_manifest(&output).is_err());
    }
    std::fs::write(&manifest_path, &original).unwrap();

    let wasm_path = output.join("app.wasm");
    let original_wasm = std::fs::read(&wasm_path).unwrap();
    let mut changed_wasm = original_wasm.clone();
    *changed_wasm.last_mut().unwrap() ^= 1;
    std::fs::write(&wasm_path, changed_wasm).unwrap();
    assert!(replay_calculator_manifest(&output).is_err());
    std::fs::write(&wasm_path, original_wasm).unwrap();
    replay_calculator_manifest(&output).unwrap();

    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn cli_repeated_exports_succeed_and_malformed_duplicate_or_wrong_target_fail_closed() {
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator.spx");
    let parent = temporary("cli");
    std::fs::create_dir(&parent).unwrap();

    let success_path = parent.join("success");
    let success = Command::new(binary)
        .args(["build", source.to_str().unwrap(), "--target", "web"])
        .args(["--export", "calculator.multiply"])
        .args(["--export", "calculator.add"])
        .args(["--output", success_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        success.status.success(),
        "CLI success failed: {}",
        String::from_utf8_lossy(&success.stderr)
    );
    let success_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(success_path.join("semaprax.scalar-exports.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        success_manifest["scalar_abi"]["functions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["calculator.add", "calculator.multiply"]
    );

    let hyphen_source = parent.join("hyphen.spx");
    std::fs::write(
        &hyphen_source,
        "module tests.hyphen;\n\n@id(\"-x\")\nfn selected() -> i64\n{\n    42\n}\n\n@id(\"app.main\")\nfn main() -> i64\n{\n    selected()\n}\n",
    )
    .unwrap();
    let hyphen_path = parent.join("hyphen");
    let hyphen = Command::new(binary)
        .args([
            "build",
            hyphen_source.to_str().unwrap(),
            "--target",
            "web",
            "--export=-x",
            "--output",
            hyphen_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        hyphen.status.success(),
        "leading-hyphen export failed: {}",
        String::from_utf8_lossy(&hyphen.stderr)
    );
    let hyphen_manifest: serde_json::Value = serde_json::from_slice(
        &std::fs::read(hyphen_path.join("semaprax.scalar-exports.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(
        hyphen_manifest["scalar_abi"]["functions"][0]["stable_id"],
        "-x"
    );

    let malformed = Command::new(binary)
        .args([
            "build",
            source.to_str().unwrap(),
            "--target",
            "web",
            "--export",
        ])
        .output()
        .unwrap();
    assert_eq!(malformed.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&malformed.stderr).contains("requires a value"));

    let duplicate_path = parent.join("duplicate");
    let duplicate = Command::new(binary)
        .args(["build", source.to_str().unwrap(), "--target", "web"])
        .args(["--export", "calculator.add"])
        .args(["--export", "calculator.add"])
        .args(["--output", duplicate_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(duplicate.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("SPX-W115"));
    assert!(!duplicate_path.exists());

    let wrong_target_path = parent.join("wrong-target");
    let wrong_target = Command::new(binary)
        .args(["build", source.to_str().unwrap(), "--target", "native"])
        .args(["--export", "calculator.add"])
        .args(["--output", wrong_target_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(wrong_target.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&wrong_target.stderr)
        .contains("--export is only valid with --target web"));
    assert!(!wrong_target_path.exists());

    let _ = std::fs::remove_dir_all(parent);
}
