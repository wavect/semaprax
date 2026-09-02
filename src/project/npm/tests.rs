use std::process::Command;

use super::*;

fn manifest() -> ProjectManifest {
    ProjectManifest::parse(
        "schema = \"semaprax.project.v2\"\nname = \"config-validator\"\nversion = \"1.2.3\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.valid\"]\ntests = [\"config.tests\"]\n",
    )
    .unwrap()
}

fn export() -> TextPackageExport {
    TextPackageExport::new(
        "config.valid".into(),
        "spx_text_636f6e6669672e76616c6964".into(),
        vec![TextPackageType::Str, TextPackageType::Bool],
        TextPackageType::Bool,
    )
}

fn runtime_manifest() -> ProjectManifest {
    ProjectManifest::parse(
        "schema = \"semaprax.project.v2\"\nname = \"config-validator\"\nversion = \"1.2.3\"\nprofile = \"useful-text-consumer.v1\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.contains\", \"config.fail\", \"config.len\"]\ntests = [\"config.tests\"]\n",
    )
    .unwrap()
}

fn runtime_package() -> UsefulTextNpmPackage {
    let source = crate::parse(
        "module config.app;\n@id(\"config.contains\") fn contains(value: borrow str, needle: borrow str) -> bool { str_contains(value, needle) }\n@id(\"config.fail\") fn fail(value: borrow str) -> i64 { str_len_bytes(value) / 0 }\n@id(\"config.len\") fn len(value: borrow str) -> i64 { str_len_bytes(value) }\n@id(\"main\") fn main() -> i64 { 0 }\n",
        std::path::Path::new("config-runtime.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let wasm = crate::wasm::emit_resolved_module_with_text_exports(
        &program,
        runtime_manifest().web_exports(),
    )
    .unwrap();
    let exports = derive_exports(&program, runtime_manifest().web_exports()).unwrap();
    render_useful_text_npm_package(&runtime_manifest(), &wasm, &exports).unwrap()
}

fn temporary_directory(label: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("semaprax-{label}-{}-{nonce}", std::process::id()))
}

fn write_package(root: &Path, package: &UsefulTextNpmPackage) {
    std::fs::create_dir(root).unwrap();
    for artifact in package.artifacts() {
        std::fs::write(root.join(artifact.path()), artifact.bytes()).unwrap();
    }
}

#[test]
fn useful_text_npm_inventory_and_metadata_are_exact_and_deterministic() {
    let first = render_useful_text_npm_package(&manifest(), b"\0asm", &[export()]).unwrap();
    let second = render_useful_text_npm_package(&manifest(), b"\0asm", &[export()]).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.artifacts().each_ref().map(|artifact| artifact.path()),
        USEFUL_TEXT_PACKAGE_PATHS
    );
    let package: serde_json::Value =
        serde_json::from_slice(first.artifact("package.json").unwrap()).unwrap();
    assert_eq!(package["name"], "config-validator");
    assert_eq!(package["version"], "1.2.3");
    assert_eq!(package["sideEffects"], false);
    assert_eq!(package["exports"]["./app.wasm"], "./app.wasm");
    assert_eq!(
        package["exports"]["./manifest"],
        "./semaprax.text-exports.json"
    );
    assert_eq!(package["engines"]["node"], ">=22");
    for forbidden in ["private", "scripts", "dependencies", "devDependencies"] {
        assert!(package.get(forbidden).is_none(), "unexpected {forbidden}");
    }
    let bindings = std::str::from_utf8(first.artifact("semaprax.bindings.js").unwrap()).unwrap();
    assert!(bindings.contains("rejectLoneSurrogate"));
    assert!(bindings.contains("used + bytes.byteLength > capacity"));
    assert!(bindings.contains("SemapraxTextError(3"));
    assert!(!bindings.contains("new Uint8Array(e.memory.buffer, base, capacity);\n  let"));
}

#[test]
fn npm_renderer_rejects_v1_unsorted_and_non_text_inputs() {
    let v1 = ProjectManifest::parse(
        "schema = \"semaprax.project.v1\"\nname = \"config-validator\"\nentry = \"config.app\"\nsources = [\"a/app.spx\", \"z/tests.spx\"]\nweb_exports = [\"config.valid\"]\ntests = [\"config.tests\"]\n",
    )
    .unwrap();
    assert_eq!(
        render_useful_text_npm_package(&v1, b"\0asm", &[export()])
            .unwrap_err()
            .code,
        "SPX-W120"
    );
    let scalar_source = crate::parse(
        "module config.app;\n@id(\"config.valid\") fn valid(value: bool) -> bool { value }\n@id(\"main\") fn main() -> i64 { 0 }\n",
        std::path::Path::new("config-v1.spx"),
    )
    .unwrap();
    let scalar_program = crate::hir::resolve(&scalar_source).unwrap();
    let admission = prepare(&v1, &scalar_program, "", "", "", 0).unwrap_err();
    assert_eq!(admission.code, "SPX-W120");
    assert_eq!(
        admission.message,
        "npm facade requires the useful-text-consumer.v1 Project profile"
    );
    let mut later = export();
    later.stable_id = "z.valid".into();
    assert!(render_useful_text_npm_package(&manifest(), b"\0asm", &[later, export()]).is_err());
    let scalar = TextPackageExport::new(
        "config.valid".into(),
        "spx_text_00".into(),
        vec![TextPackageType::Bool],
        TextPackageType::Bool,
    );
    assert!(render_useful_text_npm_package(&manifest(), b"\0asm", &[scalar]).is_err());
}

#[test]
fn carrier_replay_rejects_a_canonical_self_resigned_generated_artifact_mutation() {
    let source = crate::parse(
        "module config.app;\n@id(\"config.valid\") fn valid(value: borrow str, expected: bool) -> bool { str_is_empty(value) == expected }\n@id(\"main\") fn main() -> i64 { 0 }\n",
        std::path::Path::new("config.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let wasm =
        crate::wasm::emit_resolved_module_with_text_exports(&program, &["config.valid".to_owned()])
            .unwrap();
    let mut package = render_useful_text_npm_package(&manifest(), &wasm, &[export()]).unwrap();
    let semantic_recipe = render_semantic_recipe(&program).unwrap();
    let identity = NpmBuildIdentity {
        project_schema: super::super::PROJECT_SCHEMA_V2,
        package: "config-validator",
        version: "1.2.3",
        project_revision: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        workspace_revision:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        project_graph_digest:
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        semantic_recipe: &semantic_recipe,
    };
    let total = package
        .artifacts
        .iter()
        .map(|artifact| artifact.bytes.len())
        .sum();
    let digest = payload_digest(identity, &package);
    let envelope = render_carrier(identity, &package, total, &digest);
    ProjectNpmBuild::inspect_envelope(&envelope, envelope.len()).unwrap();
    let trusted_build = ProjectNpmBuild {
        envelope: envelope.clone(),
        payload_digest: digest.clone(),
        artifact_bytes: total,
        max_bytes: envelope.len(),
        trusted: trusted_binding(identity),
    };
    trusted_build.verify().unwrap();

    let attacker_source = crate::parse(
        "module config.app;\n@id(\"config.valid\") fn valid(value: borrow str, expected: bool) -> bool { !str_is_empty(value) == expected }\n@id(\"main\") fn main() -> i64 { 0 }\n",
        std::path::Path::new("attacker-config.spx"),
    )
    .unwrap();
    let attacker_program = crate::hir::resolve(&attacker_source).unwrap();
    let attacker_wasm = crate::wasm::emit_resolved_module_with_text_exports(
        &attacker_program,
        &["config.valid".to_owned()],
    )
    .unwrap();
    assert_ne!(attacker_wasm, wasm);
    let mut replaced = package.clone();
    replaced
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.path == "app.wasm")
        .unwrap()
        .bytes = attacker_wasm.clone();
    let attacker_sha = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(&attacker_wasm))
    );
    replaced
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.path == "semaprax.text-exports.json")
        .unwrap()
        .bytes =
        render_metadata("config-validator", "1.2.3", &attacker_sha, &[export()]).into_bytes();
    let replaced_total = replaced
        .artifacts
        .iter()
        .map(|artifact| artifact.bytes.len())
        .sum();
    let replaced_digest = payload_digest(identity, &replaced);
    let resigned_wasm = render_carrier(identity, &replaced, replaced_total, &replaced_digest);
    let error = ProjectNpmBuild::inspect_envelope(&resigned_wasm, resigned_wasm.len()).unwrap_err();
    assert!(error.message.contains("semantic recipe replay"));

    // A fully self-consistent alternate recipe is inspectable as compiler
    // output, but cannot replace the context-bound recipe held by the
    // original prepared build or gain publication authority.
    let attacker_recipe = render_semantic_recipe(&attacker_program).unwrap();
    let attacker_identity = NpmBuildIdentity {
        semantic_recipe: &attacker_recipe,
        ..identity
    };
    let attacker_digest = payload_digest(attacker_identity, &replaced);
    let attacker_envelope = render_carrier(
        attacker_identity,
        &replaced,
        replaced_total,
        &attacker_digest,
    );
    ProjectNpmBuild::inspect_envelope(&attacker_envelope, attacker_envelope.len()).unwrap();
    let mut forged = trusted_build.clone();
    forged.envelope = attacker_envelope;
    forged.payload_digest = attacker_digest;
    forged.artifact_bytes = replaced_total;
    forged.max_bytes = forged.envelope.len();
    let error = forged.verify().unwrap_err();
    assert!(error
        .message
        .contains("context-bound trusted Project facts"));

    let package_json = package
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.path == "package.json")
        .unwrap();
    package_json.bytes = b"{\"name\":\"config-validator\",\"version\":\"1.2.3\",\"type\":\"module\",\"scripts\":{\"postinstall\":\"false\"}}\n".to_vec();
    let total = package
        .artifacts
        .iter()
        .map(|artifact| artifact.bytes.len())
        .sum();
    let digest = payload_digest(identity, &package);
    let resigned = render_carrier(identity, &package, total, &digest);
    let error = ProjectNpmBuild::inspect_envelope(&resigned, resigned.len()).unwrap_err();
    assert_eq!(error.code, "SPX-W120");
    assert!(error.message.contains("semantic replay"));
}

#[test]
fn generated_facade_is_bounded_repeatable_and_offline_npm_types_are_strict() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = temporary_directory("npm-runtime-v1");
    write_package(&root, &runtime_package());
    let script = r#"import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import instantiate, { SemapraxTextError } from "./semaprax.bindings.js";
const runtime = instantiate(readFileSync("./app.wasm"));
const f = runtime.functions;
assert.equal(f["config.len"]("repeat"), 6n);
assert.equal(f["config.len"]("repeat"), 6n);
assert.throws(() => f["config.len"](String.fromCharCode(0xd800)), error => error instanceof SemapraxTextError && error.code === 2);
const exact = "a".repeat(65536);
assert.equal(f["config.len"](exact), 65536n);
assert.throws(() => f["config.len"](exact + "a"), error => error instanceof SemapraxTextError && error.code === 1);
assert.throws(() => f["config.contains"]("a".repeat(32768), "b".repeat(32769)), error => error instanceof SemapraxTextError && error.code === 1);
assert.throws(() => f["config.fail"]("must-be-erased"), RangeError);
assert.equal(f["config.len"]("after-failure"), 13n);
const source = readFileSync("./semaprax.bindings.js", "utf8");
assert.match(source, /memory\.fill\(0, 0, used\)/);
assert.match(source, /SemapraxTextError\(3/);
"#;
    let node = Command::new("node")
        .current_dir(&root)
        .args(["--input-type=module", "--eval", script])
        .output()
        .unwrap();
    assert!(
        node.status.success(),
        "generated npm runtime failed:\n{}",
        String::from_utf8_lossy(&node.stderr)
    );

    if Command::new("npm").arg("--version").output().is_ok()
        && Command::new("tsc").arg("--version").output().is_ok()
    {
        let npm_cache = temporary_directory("npm-cache-v1");
        std::fs::create_dir(&npm_cache).unwrap();
        let packed = Command::new("npm")
            .current_dir(&root)
            .env("npm_config_cache", &npm_cache)
            .args(["pack", "--ignore-scripts", "--json"])
            .output()
            .unwrap();
        assert!(
            packed.status.success(),
            "offline npm pack failed:\n{}",
            String::from_utf8_lossy(&packed.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&packed.stdout).unwrap();
        let mut files = report[0]["files"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["path"].as_str().unwrap())
            .collect::<Vec<_>>();
        files.sort_unstable();
        let mut expected = USEFUL_TEXT_PACKAGE_PATHS;
        expected.sort_unstable();
        assert_eq!(files, expected);

        let consumer = root.join("consumer");
        std::fs::create_dir(&consumer).unwrap();
        std::fs::write(
            consumer.join("package.json"),
            "{\"name\":\"consumer\",\"private\":true,\"type\":\"module\"}\n",
        )
        .unwrap();
        let tarball = root.join("config-validator-1.2.3.tgz");
        let installed = Command::new("npm")
            .current_dir(&consumer)
            .env("npm_config_cache", &npm_cache)
            .args([
                "install",
                "--offline",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                tarball.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        assert!(
            installed.status.success(),
            "offline npm install failed:\n{}",
            String::from_utf8_lossy(&installed.stderr)
        );
        std::fs::write(
            consumer.join("consumer.ts"),
            "import instantiate, { exportIds, type UsefulTextRuntime } from \"config-validator\";\nconst bytes = new Uint8Array();\nconst runtime: UsefulTextRuntime = instantiate(bytes);\nconst length: bigint = runtime.functions[\"config.len\"](\"ok\");\nconst contained: boolean = runtime.functions[\"config.contains\"](\"ok\", \"k\");\nvoid [length, contained, exportIds];\n",
        )
        .unwrap();
        let typed = Command::new("tsc")
            .current_dir(&consumer)
            .args([
                "--strict",
                "--noEmit",
                "--target",
                "ES2022",
                "--module",
                "NodeNext",
                "--moduleResolution",
                "NodeNext",
                "consumer.ts",
            ])
            .output()
            .unwrap();
        assert!(
            typed.status.success(),
            "strict TypeScript consumer failed:\n{}",
            String::from_utf8_lossy(&typed.stderr)
        );
        std::fs::remove_dir_all(npm_cache).unwrap();
    }
    std::fs::remove_dir_all(&root).unwrap();
}

#[cfg(unix)]
#[test]
fn handle_relative_publication_never_writes_through_a_substituted_destination() {
    use std::os::unix::fs::symlink;

    let source = crate::parse(
        "module config.app;\n@id(\"config.contains\") fn contains(value: borrow str, needle: borrow str) -> bool { str_contains(value, needle) }\n@id(\"config.fail\") fn fail(value: borrow str) -> i64 { str_len_bytes(value) / 0 }\n@id(\"config.len\") fn len(value: borrow str) -> i64 { str_len_bytes(value) }\n@id(\"main\") fn main() -> i64 { 0 }\n",
        Path::new("publication-race.spx"),
    )
    .unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let package = runtime_package();
    let recipe = render_semantic_recipe(&program).unwrap();
    let identity = NpmBuildIdentity {
        project_schema: super::super::PROJECT_SCHEMA_V2,
        package: "config-validator",
        version: "1.2.3",
        project_revision: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        workspace_revision:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        project_graph_digest:
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
        semantic_recipe: &recipe,
    };
    let total = package.artifacts.iter().map(|item| item.bytes.len()).sum();
    let digest = payload_digest(identity, &package);
    let envelope = render_carrier(identity, &package, total, &digest);
    let build = ProjectNpmBuild {
        envelope,
        payload_digest: digest,
        artifact_bytes: total,
        max_bytes: MAX_PROJECT_NPM_BUILD_BYTES,
        trusted: trusted_binding(identity),
    };
    build.verify().unwrap();

    let root = temporary_directory("npm-handle-relative-race");
    let output = root.join("package");
    let moved = root.join("held-package");
    let foreign = root.join("foreign");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("marker"), b"foreign").unwrap();
    let output_for_hook = output.clone();
    let moved_for_hook = moved.clone();
    let foreign_for_hook = foreign.clone();
    publication::set_test_after_create(Box::new(move || {
        std::fs::rename(&output_for_hook, &moved_for_hook).unwrap();
        symlink(&foreign_for_hook, &output_for_hook).unwrap();
    }));
    let error = build.publish(&output).unwrap_err();
    assert!(
        error.message.contains("identity changed"),
        "{}",
        error.message
    );
    assert_eq!(std::fs::read(foreign.join("marker")).unwrap(), b"foreign");
    assert_eq!(std::fs::read_dir(&foreign).unwrap().count(), 1);
    assert!(std::fs::symlink_metadata(&output)
        .unwrap()
        .file_type()
        .is_symlink());
    std::fs::remove_file(output).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
