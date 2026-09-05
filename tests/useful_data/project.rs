#[cfg(not(windows))]
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{ProjectExecutionOptions, ProjectExecutionOutcome};
use semaprax::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};
use semaprax::{codegen, project};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/binary-frame-project")
}

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn copy_fixture(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "semaprax-binary-frame-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    for relative in [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/frame.spx",
        "src/tests.spx",
    ] {
        std::fs::copy(fixture().join(relative), root.join(relative)).unwrap();
    }
    root.canonicalize().unwrap()
}

#[test]
fn project_v3_links_exact_data_roots_and_native_o0_o2_agree() {
    let generated =
        project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            assert!(snapshot.manifest().is_v3());
            assert_eq!(
                snapshot.manifest().profile(),
                Some(project::PROJECT_PROFILE_USEFUL_DATA_V1)
            );
            for selected in snapshot.manifest().web_exports() {
                assert!(snapshot
                    .retain_revision()
                    .public_api_program()
                    .functions
                    .iter()
                    .any(|function| function.id.as_str() == selected));
                assert!(snapshot.semantic_graph().contains(selected));
            }
            let test = snapshot.execute_test(&ProjectExecutionOptions::default())?;
            assert_eq!(test.outcome(), &ProjectExecutionOutcome::Returned(0));
            assert!(test.command_succeeded());
            let execution: serde_json::Value = serde_json::from_str(test.envelope()).unwrap();
            assert_eq!(execution["project_schema"], project::PROJECT_SCHEMA_V3);
            project::verify_execution_envelope(test.envelope()).unwrap();
            let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
            assert_eq!(graph["project_schema"], project::PROJECT_SCHEMA_V3);
            let context: serde_json::Value = serde_json::from_str(&snapshot.semantic_context(
                WorkspaceAnalysisTargetKind::Declaration,
                "binary-frame.length",
                WorkspaceContextOptions::default(),
            )?)
            .unwrap();
            assert_eq!(context["project_schema"], project::PROJECT_SCHEMA_V3);
            let impact: serde_json::Value = serde_json::from_str(&snapshot.semantic_impact(
                WorkspaceAnalysisTargetKind::Declaration,
                "binary-frame.length",
                WorkspaceImpactOptions::default(),
            )?)
            .unwrap();
            assert_eq!(impact["project_schema"], project::PROJECT_SCHEMA_V3);
            let test_wasm = snapshot.test_wasm_module()?;
            assert!(test_wasm.starts_with(b"\0asm"));
            codegen::emit_hir_c(snapshot.retain_revision().public_api_program())
                .map_err(|error| vec![error])
        })
        .unwrap();

    if !command_available("clang") {
        return;
    }
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(64), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    const uint8_t payload[] = {{UINT8_C(83),UINT8_C(80),UINT8_C(88),UINT8_C(1),UINT8_C(255),UINT8_C(0),UINT8_C(255)}};
    const uint8_t moved_payload[] = {{UINT8_C(255),UINT8_C(83),UINT8_C(80),UINT8_C(255),UINT8_C(88),UINT8_C(1),UINT8_C(0)}};
    spx_slice_u8_v1 frame = {{payload, UINT64_C(7)}};
    spx_slice_u8_v1 moved_frame = {{moved_payload, UINT64_C(7)}};
    spx_slice_u8_v1 empty_frame = {{NULL, UINT64_C(0)}};
    uint64_t length = UINT64_C(0), checksum = UINT64_C(0), combined = UINT64_C(0);
    bool magic = false;
    if ({length_fn}(&context, frame, &length) != SPX_STATUS_SUCCESS || length != UINT64_C(7)) return 11;
    if ({magic_fn}(&context, frame, &magic) != SPX_STATUS_SUCCESS || !magic) return 12;
    if ({checksum_fn}(&context, frame, &checksum) != SPX_STATUS_SUCCESS || checksum != UINT64_C(2)) return 13;
    checksum = UINT64_C(0);
    if ({checksum_fn}(&context, moved_frame, &checksum) != SPX_STATUS_SUCCESS || checksum != UINT64_C(2)) return 15;
    checksum = UINT64_C(99);
    if ({checksum_fn}(&context, empty_frame, &checksum) != SPX_STATUS_SUCCESS || checksum != UINT64_C(0)) return 16;
    if ({combined_fn}(&context, frame, frame, &combined) != SPX_STATUS_SUCCESS || combined != UINT64_C(14)) return 14;
    return 0;
}}
"#,
        length_fn = symbol("binary-frame.length"),
        magic_fn = symbol("binary-frame.has-magic"),
        checksum_fn = symbol("binary-frame.checksum"),
        combined_fn = symbol("binary-frame.combine-length"),
    );
    for optimization in ["-O0", "-O2"] {
        let stem = format!(
            "semaprax-binary-frame-native-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let ran = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(executable);
        assert!(ran.status.success(), "native exit {:?}", ran.status.code());
    }
}

#[cfg(not(windows))]
#[test]
fn data_package_is_digest_authenticated_strict_and_installed_compiler_free() {
    let root = copy_fixture("npm");
    let output = root.join("package");
    let inline = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap();
    inline.verify().unwrap();
    project::ProjectNpmBuild::inspect_envelope(inline.envelope(), inline.max_bytes()).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm(&output)
    })
    .unwrap();
    let inventory = std::fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        inventory,
        BTreeSet::from([
            "app.wasm".to_owned(),
            "package.json".to_owned(),
            "semaprax.bindings.d.ts".to_owned(),
            "semaprax.bindings.js".to_owned(),
            "semaprax.data-exports.json".to_owned(),
            "semaprax.js".to_owned(),
        ])
    );

    if command_available("node") {
        std::fs::write(
            root.join("run.mjs"),
            r#"import assert from "node:assert/strict";
import fs from "node:fs";
import { instantiate, SemapraxDataError } from "./package/semaprax.bindings.js";
const original = new Uint8Array(fs.readFileSync("./package/app.wasm"));
const pending = instantiate(original);
original.fill(0);
const runtime = await pending;
const frame = Uint8Array.from([83,80,88,1,255,0,255]);
const movedFrame = Uint8Array.from([255,83,80,255,88,1,0]);
assert.equal(runtime.functions["binary-frame.length"](frame), 7n);
assert.equal(runtime.functions["binary-frame.has-magic"](frame), true);
assert.equal(runtime.functions["binary-frame.checksum"](frame), 2n);
assert.equal(runtime.functions["binary-frame.checksum"](movedFrame), 2n);
assert.equal(runtime.functions["binary-frame.checksum"](new Uint8Array()), 0n);
assert.equal(runtime.functions["binary-frame.combine-length"](frame, frame), 14n);
assert.equal(runtime.functions["binary-frame.length"](new Uint8Array(65536)), 65536n);
let boundary;
assert.throws(() => runtime.functions["binary-frame.combine-length"](new Uint8Array(32768), new Uint8Array(32769)), error => { boundary = error; return error instanceof SemapraxDataError; });
assert.equal(boundary.code, 11); assert.equal(boundary.domain, "semaprax.data-adapter.v1");
assert.equal(runtime.functions["binary-frame.length"](frame), 7n);
for (const invalid of [new Uint16Array(1), new DataView(new ArrayBuffer(1)), [1], new (class extends Uint8Array {})(1)]) {
  assert.throws(() => runtime.functions["binary-frame.length"](invalid), TypeError);
}
let proxyTraps = 0;
const proxied = new Proxy(new Uint8Array(1), { getPrototypeOf() { proxyTraps++; throw new Error("prototype trap"); } });
assert.throws(() => runtime.functions["binary-frame.length"](proxied), TypeError);
assert.equal(proxyTraps, 0);
if (typeof SharedArrayBuffer === "function") assert.throws(() => runtime.functions["binary-frame.length"](new Uint8Array(new SharedArrayBuffer(1))), TypeError);
const hostile = new Uint8Array([83,80,88,1]);
Object.defineProperties(hostile, {
  [Symbol.iterator]: { get() { throw new Error("iterator observed"); } },
  buffer: { get() { throw new Error("buffer property observed"); } },
  length: { get() { throw new Error("length property observed"); } },
});
assert.equal(runtime.functions["binary-frame.has-magic"](hostile), true);
assert.equal(runtime.functions["binary-frame.combine-length"](hostile, hostile), 8n);
const offset = new Uint8Array(Uint8Array.from([0,83,80,88,1]).buffer, 1, 4);
assert.equal(runtime.functions["binary-frame.has-magic"](offset), true);
const tampered = new Uint8Array(fs.readFileSync("./package/app.wasm")); tampered[8] ^= 1;
await assert.rejects(instantiate(tampered));
const detached = new Uint8Array([1]); structuredClone(detached, {transfer:[detached.buffer]});
await assert.rejects(instantiate(detached), TypeError);
const detachedArg = new Uint8Array([1]); structuredClone(detachedArg, {transfer:[detachedArg.buffer]});
assert.throws(() => runtime.functions["binary-frame.length"](detachedArg), TypeError);
"#,
        )
        .unwrap();
        let ran = Command::new("node")
            .arg("run.mjs")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            ran.status.success(),
            "{}",
            String::from_utf8_lossy(&ran.stderr)
        );
    }

    if command_available("tsc") {
        std::fs::write(
            root.join("package.json"),
            "{\"name\":\"binary-frame-local-check\",\"private\":true,\"type\":\"module\"}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("tsconfig.json"),
            "{\"compilerOptions\":{\"strict\":true,\"noEmit\":true,\"module\":\"NodeNext\",\"moduleResolution\":\"NodeNext\",\"target\":\"ES2022\",\"lib\":[\"ES2022\",\"DOM\"]},\"files\":[\"index.ts\"]}\n",
        )
        .unwrap();
        std::fs::write(
            root.join("index.ts"),
            "import { instantiate } from \"./package/semaprax.bindings.js\";\ndeclare const wasm: Uint8Array;\ndeclare const frame: Uint8Array;\nconst runtime = await instantiate(wasm);\nconst length: bigint = runtime.functions[\"binary-frame.length\"](frame);\nconst magic: boolean = runtime.functions[\"binary-frame.has-magic\"](frame);\nvoid length; void magic;\n",
        )
        .unwrap();
        let checked = Command::new("tsc")
            .args(["-p", "tsconfig.json"])
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            checked.status.success(),
            "{}{}",
            String::from_utf8_lossy(&checked.stdout),
            String::from_utf8_lossy(&checked.stderr),
        );
    }

    if command_available("npm") {
        let npm_cache = root.join("npm-cache");
        let packed = Command::new("npm")
            .args(["pack", "--offline", "--ignore-scripts", "--json"])
            .env("npm_config_cache", &npm_cache)
            .current_dir(&output)
            .output()
            .unwrap();
        assert!(
            packed.status.success(),
            "{}",
            String::from_utf8_lossy(&packed.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&packed.stdout).unwrap();
        let tarball = output.join(report[0]["filename"].as_str().unwrap());
        let consumer = root.join("consumer");
        std::fs::create_dir(&consumer).unwrap();
        std::fs::write(
            consumer.join("package.json"),
            "{\"name\":\"binary-frame-consumer\",\"private\":true,\"type\":\"module\"}\n",
        )
        .unwrap();
        let installed = Command::new("npm")
            .args([
                "install",
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
            ])
            .arg(&tarball)
            .env("npm_config_cache", &npm_cache)
            .current_dir(&consumer)
            .output()
            .unwrap();
        assert!(
            installed.status.success(),
            "{}",
            String::from_utf8_lossy(&installed.stderr)
        );
        let installed_package = consumer.join("node_modules/binary-frame");
        let installed_inventory = std::fs::read_dir(&installed_package)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(installed_inventory, inventory);

        if command_available("node") {
            std::fs::write(
                consumer.join("run.mjs"),
                r#"import assert from "node:assert/strict";
import fs from "node:fs";
import { instantiate } from "binary-frame";
const wasmUrl = new URL(import.meta.resolve("binary-frame/app.wasm"));
const bytes = Uint8Array.from(fs.readFileSync(wasmUrl));
const runtime = await instantiate(bytes);
const frame = Uint8Array.from([83,80,88,1,255,0,255]);
const movedFrame = Uint8Array.from([255,83,80,255,88,1,0]);
assert.equal(runtime.functions["binary-frame.length"](frame), 7n);
assert.equal(runtime.functions["binary-frame.has-magic"](frame), true);
assert.equal(runtime.functions["binary-frame.checksum"](frame), 2n);
assert.equal(runtime.functions["binary-frame.checksum"](movedFrame), 2n);
assert.equal(runtime.functions["binary-frame.checksum"](new Uint8Array()), 0n);
"#,
            )
            .unwrap();
            let ran = Command::new("node")
                .arg("run.mjs")
                .current_dir(&consumer)
                .output()
                .unwrap();
            assert!(
                ran.status.success(),
                "{}",
                String::from_utf8_lossy(&ran.stderr)
            );
        }

        if command_available("tsc") {
            std::fs::write(
                consumer.join("tsconfig.json"),
                "{\"compilerOptions\":{\"strict\":true,\"noEmit\":true,\"module\":\"NodeNext\",\"moduleResolution\":\"NodeNext\",\"target\":\"ES2022\",\"lib\":[\"ES2022\",\"DOM\"]},\"files\":[\"index.ts\"]}\n",
            )
            .unwrap();
            std::fs::write(
                consumer.join("index.ts"),
                "import { instantiate } from \"binary-frame\";\ndeclare const wasm: Uint8Array;\ndeclare const frame: Uint8Array;\nconst runtime = await instantiate(wasm);\nconst length: bigint = runtime.functions[\"binary-frame.length\"](frame);\nconst magic: boolean = runtime.functions[\"binary-frame.has-magic\"](frame);\nvoid length; void magic;\n",
            )
            .unwrap();
            let checked = Command::new("tsc")
                .args(["-p", "tsconfig.json"])
                .current_dir(&consumer)
                .output()
                .unwrap();
            assert!(
                checked.status.success(),
                "{}",
                String::from_utf8_lossy(&checked.stderr)
            );
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[cfg(windows)]
#[test]
fn data_carrier_replays_but_publication_fails_closed_without_windows_authority() {
    let root = copy_fixture("windows-publication");
    let output = root.join("package");
    let inline = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap();
    inline.verify().unwrap();
    project::ProjectNpmBuild::inspect_envelope(inline.envelope(), inline.max_bytes()).unwrap();
    let diagnostics =
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            snapshot.build_npm(&output)
        })
        .unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-W120");
    assert!(diagnostics[0].message.contains("handle-relative Windows"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn stable_id_exports_survive_a_display_name_change() {
    let baseline =
        project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
            snapshot
                .build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
                .map(|build| (snapshot.project_revision().to_owned(), build))
        })
        .unwrap();
    let root = copy_fixture("rename");
    let frame_path = root.join("src/frame.spx");
    let source = std::fs::read_to_string(&frame_path).unwrap();
    assert_eq!(source.matches("fn frame_length").count(), 1);
    std::fs::write(
        &frame_path,
        source.replace("fn frame_length", "fn measured_length"),
    )
    .unwrap();
    let renamed = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot
            .retain_revision()
            .public_api_program()
            .functions
            .iter()
            .any(|function| function.id.as_str() == "binary-frame.length"));
        snapshot
            .build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
            .map(|build| (snapshot.project_revision().to_owned(), build))
    })
    .unwrap();
    assert_ne!(baseline.0, renamed.0);
    assert_ne!(baseline.1.envelope(), renamed.1.envelope());
    baseline.1.verify().unwrap();
    renamed.1.verify().unwrap();
    let _ = std::fs::remove_dir_all(root);
}
