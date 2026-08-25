use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, project};
use sha2::{Digest, Sha256};
use wasmparser::{ExternalKind, Parser, Payload};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project")
}

fn temporary(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "semaprax-project-backend-v1-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ))
}

fn copy_fixture(label: &str) -> PathBuf {
    let destination = temporary(label);
    std::fs::create_dir_all(destination.join("src")).unwrap();
    for path in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(fixture_root().join(path), destination.join(path)).unwrap();
    }
    destination.canonicalize().unwrap()
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

fn raw_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_scalar_");
    for byte in id.bytes() {
        write!(symbol, "{byte:02x}").unwrap();
    }
    symbol
}

fn quote_json(value: &str) -> String {
    serde_json::to_string(value).unwrap()
}

fn expected_project_function_rows() -> Vec<String> {
    [
        ("calculator.add", &["i64", "i64"][..], "i64"),
        ("calculator.divide", &["i64", "i64"][..], "i64"),
        ("calculator.is-negative", &["i64"][..], "bool"),
        ("calculator.multiply", &["i64", "i64"][..], "i64"),
        ("calculator.not", &["bool"][..], "bool"),
        ("calculator.subtract", &["i64", "i64"][..], "i64"),
    ]
    .iter()
    .map(|(id, parameters, result)| {
        format!(
            "{{\"stable_id\":{},\"wasm_export\":{},\"parameters\":[{}],\"result\":{}}}",
            quote_json(id),
            quote_json(&raw_symbol(id)),
            parameters
                .iter()
                .map(|parameter| quote_json(parameter))
                .collect::<Vec<_>>()
                .join(","),
            quote_json(result),
        )
    })
    .collect()
}

fn expected_project_functions() -> String {
    expected_project_function_rows().join(",")
}

fn replay_project_manifest(
    directory: &Path,
    project_revision: &str,
    workspace_revision: &str,
    project_graph_digest: &str,
) -> Result<(), String> {
    let files = package_files(directory);
    let inventory = files.keys().map(String::as_str).collect::<Vec<_>>();
    let expected_inventory = [
        "app.wasm",
        "index.html",
        "package.json",
        "semaprax.bindings.d.ts",
        "semaprax.bindings.js",
        "semaprax.js",
        "semaprax.scalar-exports.json",
    ];
    if inventory != expected_inventory {
        return Err(format!(
            "unexpected project package inventory: {inventory:?}"
        ));
    }
    let observed = std::str::from_utf8(&files["semaprax.scalar-exports.json"])
        .map_err(|error| error.to_string())?;
    let _: serde_json::Value = serde_json::from_str(observed).map_err(|error| error.to_string())?;
    let artifact_rows = [
        "app.wasm",
        "index.html",
        "package.json",
        "semaprax.bindings.d.ts",
        "semaprax.bindings.js",
        "semaprax.js",
    ]
    .iter()
    .map(|path| {
        format!(
            "{{\"path\":{},\"sha256\":\"{:x}\"}}",
            quote_json(path),
            semaprax::digest_hex::LowerHex(Sha256::digest(&files[*path])),
        )
    })
    .collect::<Vec<_>>()
    .join(",");
    let expected = format!(
        "{{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",\"project\":\"calculator\",\"project_revision\":{},\"workspace_revision\":{},\"project_graph_digest\":{},\"entry_module\":\"calculator.app\",\"capabilities\":[],\"artifacts\":[{}],\"scalar_abi\":{{\"schema\":\"semaprax.wasm-scalar.v1\",\"functions\":[{}]}}}}\n",
        quote_json(project_revision),
        quote_json(workspace_revision),
        quote_json(project_graph_digest),
        artifact_rows,
        expected_project_functions(),
    );
    if observed != expected {
        return Err("project manifest is not the exact independently replayed form".to_owned());
    }
    Ok(())
}

fn exports(bytes: &[u8]) -> Vec<String> {
    Parser::new(0)
        .parse_all(bytes)
        .filter_map(|payload| match payload.unwrap() {
            Payload::ExportSection(section) => Some(
                section
                    .into_iter()
                    .filter_map(|export| {
                        let export = export.unwrap();
                        (export.kind == ExternalKind::Func).then(|| export.name.to_owned())
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
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
    let _ = std::fs::remove_file(c_path);
}

fn run_result(path: &Path, expected: &str) {
    let output = Command::new(path).output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), expected);
    assert!(output.stderr.is_empty());
}

#[test]
fn linked_project_runs_entry_and_test_closures_at_native_o0_o2() {
    let root = temporary("native");
    std::fs::create_dir(&root).unwrap();
    project::with_authenticated_project(&fixture_root().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let entry = snapshot.execute_entry(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            entry.outcome(),
            &project::ProjectExecutionOutcome::Returned(42)
        );
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            tests.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        let o0 = root.join("calculator-o0");
        let o2 = root.join("calculator-o2");
        compile_c(&c, &o0, "-O0");
        compile_c(&c, &o2, "-O2");
        run_result(&o0, "42");
        run_result(&o2, "42");

        let test_c = codegen::emit_hir_c(snapshot.test_program()).map_err(|error| vec![error])?;
        let test_o0 = root.join("calculator-test-o0");
        let test_o2 = root.join("calculator-test-o2");
        compile_c(&test_c, &test_o0, "-O0");
        compile_c(&test_c, &test_o2, "-O2");
        run_result(&test_o0, "0");
        run_result(&test_o2, "0");
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn project_web_package_is_deterministic_exact_and_node_executable() {
    let first = temporary("web-a");
    let second = temporary("web-b");
    project::with_authenticated_project(&fixture_root().join("semaprax.toml"), |snapshot| {
        snapshot.build_web(&first)?;
        snapshot.build_web(&second)?;
        assert_eq!(package_files(&first), package_files(&second));
        let files = package_files(&first);
        assert_eq!(files.len(), 7);
        assert_eq!(
            exports(&files["app.wasm"]),
            [
                "calculator.add",
                "calculator.divide",
                "calculator.is-negative",
                "calculator.multiply",
                "calculator.not",
                "calculator.subtract",
            ]
            .map(raw_symbol)
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&files["semaprax.scalar-exports.json"]).unwrap();
        assert_eq!(manifest["schema"], "semaprax.web-project.v1");
        assert_eq!(manifest["project_schema"], "semaprax.project.v1");
        assert_eq!(manifest["project"], "calculator");
        assert_eq!(manifest["project_revision"], snapshot.project_revision());
        assert_eq!(manifest["workspace_revision"], snapshot.workspace_revision());
        let semantic_graph: serde_json::Value =
            serde_json::from_str(snapshot.semantic_graph()).unwrap();
        assert_eq!(
            manifest["project_graph_digest"],
            semantic_graph["graph_digest"]
        );
        assert_eq!(manifest["entry_module"], "calculator.app");
        for artifact in manifest["artifacts"].as_array().unwrap() {
            let path = artifact["path"].as_str().unwrap();
            assert_eq!(
                artifact["sha256"].as_str().unwrap(),
                format!(
                    "{:x}",
                    semaprax::digest_hex::LowerHex(Sha256::digest(&files[path]))
                )
            );
        }
        replay_project_manifest(
            &first,
            snapshot.project_revision(),
            snapshot.workspace_revision(),
            semantic_graph["graph_digest"].as_str().unwrap(),
        )
        .unwrap();

        let manifest_path = first.join("semaprax.scalar-exports.json");
        let original_manifest = std::fs::read_to_string(&manifest_path).unwrap();
        let artifact_rows = ["app.wasm", "index.html"]
            .map(|path| {
                format!(
                    "{{\"path\":{},\"sha256\":\"{:x}\"}}",
                    quote_json(path),
                    semaprax::digest_hex::LowerHex(Sha256::digest(&files[path])),
                )
            });
        let function_rows = expected_project_function_rows();
        let mutations = [
            original_manifest.replacen(
                "{\"schema\":\"semaprax.web-project.v1\",",
                "{\"schema\":\"semaprax.web-project.v1\",\"schema\":\"semaprax.web-project.v1\",",
                1,
            ),
            original_manifest.replacen(
                "{\"schema\":\"semaprax.web-project.v1\",\"project_schema\":\"semaprax.project.v1\",",
                "{\"project_schema\":\"semaprax.project.v1\",\"schema\":\"semaprax.web-project.v1\",",
                1,
            ),
            original_manifest.replacen("\"capabilities\":[],", "", 1),
            original_manifest.replacen(
                "\"entry_module\":\"calculator.app\",",
                "\"entry_module\":\"calculator.app\",\"unknown\":0,",
                1,
            ),
            original_manifest.replacen(
                &format!("{},{}", artifact_rows[0], artifact_rows[1]),
                &format!("{},{}", artifact_rows[1], artifact_rows[0]),
                1,
            ),
            original_manifest.replacen(
                &format!("{},{}", function_rows[0], function_rows[1]),
                &format!("{},{}", function_rows[1], function_rows[0]),
                1,
            ),
        ];
        for mutation in mutations {
            assert_ne!(mutation, original_manifest);
            std::fs::write(&manifest_path, mutation).unwrap();
            assert!(replay_project_manifest(
                &first,
                snapshot.project_revision(),
                snapshot.workspace_revision(),
                semantic_graph["graph_digest"].as_str().unwrap(),
            )
            .is_err());
        }
        std::fs::write(&manifest_path, &original_manifest).unwrap();
        let wasm_path = first.join("app.wasm");
        let original_wasm = std::fs::read(&wasm_path).unwrap();
        let mut changed_wasm = original_wasm.clone();
        *changed_wasm.last_mut().unwrap() ^= 1;
        std::fs::write(&wasm_path, changed_wasm).unwrap();
        assert!(replay_project_manifest(
            &first,
            snapshot.project_revision(),
            snapshot.workspace_revision(),
            semantic_graph["graph_digest"].as_str().unwrap(),
        )
        .is_err());
        std::fs::write(&wasm_path, original_wasm).unwrap();
        replay_project_manifest(
            &first,
            snapshot.project_revision(),
            snapshot.workspace_revision(),
            semantic_graph["graph_digest"].as_str().unwrap(),
        )
        .unwrap();

        let script = first.join("verify-project.mjs");
        std::fs::write(
            &script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { instantiateBytes } from "./semaprax.bindings.js";
const bytes = await readFile("./app.wasm");
const runtime = await instantiateBytes(bytes);
assert.deepEqual(runtime.call("calculator.add", 19n, 23n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.subtract", 84n, 42n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.multiply", 6n, 7n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.divide", 84n, 2n), {ok:true,value:42n});
assert.deepEqual(runtime.call("calculator.is-negative", -1n), {ok:true,value:true});
assert.deepEqual(runtime.call("calculator.not", true), {ok:true,value:false});
assert.deepEqual(runtime.call("calculator.add", (1n << 63n) - 1n, 1n), {ok:false,status:{schema:"semaprax.status.v1",domain_id:"semaprax.arithmetic.v1",code:1}});
assert.deepEqual(runtime.call("calculator.divide", 1n, 0n), {ok:false,status:{schema:"semaprax.status.v1",domain_id:"semaprax.contract.v1",code:1}});
const changed = Buffer.from(bytes); changed[changed.length - 1] ^= 1;
await assert.rejects(instantiateBytes(changed), /authentication failed/);
"#,
        )
        .unwrap();
        let node = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&first)
            .output()
            .unwrap();
        assert!(
            node.status.success(),
            "Node project package failed: {}",
            String::from_utf8_lossy(&node.stderr)
        );

        let test_wasm = snapshot.test_wasm_module()?;
        let test_script = first.join("verify-test.mjs");
        std::fs::write(first.join("test.wasm"), test_wasm).unwrap();
        std::fs::write(
            &test_script,
            r#"import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
const bytes = await readFile("./test.wasm");
const checked = (operation) => (a, b) => { const value = operation(a, b); if (value < -(1n<<63n) || value > (1n<<63n)-1n) throw new RangeError(); return value; };
const imports = {env:{spx_add:checked((a,b)=>a+b),spx_sub:checked((a,b)=>a-b),spx_mul:checked((a,b)=>a*b),spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:(a)=>-a,spx_contract_fail:()=>{throw new Error();}}};
const linked = await WebAssembly.instantiate(bytes, imports);
assert.equal(linked.instance.exports.semaprax_main(), 0n);
"#,
        )
        .unwrap();
        let node = Command::new("node")
            .arg(test_script.file_name().unwrap())
            .current_dir(&first)
            .output()
            .unwrap();
        assert!(
            node.status.success(),
            "Node project test closure failed: {}",
            String::from_utf8_lossy(&node.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(first);
    let _ = std::fs::remove_dir_all(second);
}

#[test]
fn stable_id_display_rename_preserves_web_api_and_native_behavior() {
    let renamed = copy_fixture("renamed");
    let core_path = renamed.join("src/core.spx");
    let core = std::fs::read_to_string(&core_path).unwrap();
    std::fs::write(&core_path, core.replace("fn add(", "fn sum(")).unwrap();
    let original_output = temporary("rename-original");
    let renamed_output = temporary("rename-changed");
    project::with_authenticated_project(&fixture_root().join("semaprax.toml"), |snapshot| {
        snapshot.build_web(&original_output)
    })
    .unwrap();
    project::with_authenticated_project(&renamed.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        snapshot.build_web(&renamed_output)?;
        let native = renamed.join("renamed-native");
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        compile_c(&c, &native, "-O2");
        run_result(&native, "42");
        Ok(())
    })
    .unwrap();
    for artifact in ["app.wasm", "semaprax.bindings.js", "semaprax.bindings.d.ts"] {
        assert_eq!(
            std::fs::read(original_output.join(artifact)).unwrap(),
            std::fs::read(renamed_output.join(artifact)).unwrap(),
            "display rename changed {artifact}"
        );
    }
    let _ = std::fs::remove_dir_all(original_output);
    let _ = std::fs::remove_dir_all(renamed_output);
    let _ = std::fs::remove_dir_all(renamed);
}
