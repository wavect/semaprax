use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture_v5() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-native-command-project")
}

fn fixture_v4() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-project")
}

fn build(root: &Path) -> project::ProjectNpmBuild {
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        snapshot.build_npm_inline(project::MAX_PROJECT_NPM_BUILD_BYTES)
    })
    .unwrap()
}

fn decode_hex(value: &str) -> Vec<u8> {
    let encoded = value.as_bytes();
    assert_eq!(encoded.len() & 1, 0);
    let nibble = |byte| match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => panic!("carrier hex is not lowercase"),
    };
    let mut decoded = Vec::with_capacity(encoded.len() / 2);
    let mut offset = 0;
    while offset < encoded.len() {
        decoded.push((nibble(encoded[offset]) << 4) | nibble(encoded[offset + 1]));
        offset += 2;
    }
    decoded
}

fn artifacts(value: &serde_json::Value) -> Vec<(&str, Vec<u8>)> {
    value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            (
                row["path"].as_str().unwrap(),
                decode_hex(row["hex"].as_str().unwrap()),
            )
        })
        .collect()
}

fn write_package(root: &Path, build: &project::ProjectNpmBuild) {
    std::fs::create_dir(root).unwrap();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    for (path, bytes) in artifacts(&value) {
        std::fs::write(root.join(path), bytes).unwrap();
    }
}

fn false_after_write_project() -> PathBuf {
    let temporary = std::fs::canonicalize(std::env::temp_dir()).unwrap();
    let root = temporary.join(format!(
        "semaprax-false-after-write-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v5\"\nname = \"false-write\"\nversion = \"0.1.0\"\nprofile = \"useful-data-command.v2\"\nentry = \"false_write.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"false-write.run\"]\ncommand = \"false-write.run\"\ninput = \"stdin-bytes+one-utf8-arg.v1\"\ncapabilities = [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]\ntests = [\"false_write.tests\"]\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/app.spx"),
        "module false_write.app;\n\npermit { process.stdout.write }\n\n@id(\"false-write.run\")\nfn run(input: borrow Slice<u8>, needle: borrow Slice<u8>) -> bool\n    uses { process.stdout.write }\n{\n    let written = stdout_write(input);\n    written == byte_len(input) && false\n}\n\n@id(\"main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module false_write.tests;\n\n@id(\"false-write.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    root
}

#[test]
fn v5_carrier_and_metadata_bind_the_exact_fixed_adapter_contract() {
    let build = build(&fixture_v5());
    build.verify().unwrap();
    project::ProjectNpmBuild::inspect_envelope(build.envelope(), build.max_bytes()).unwrap();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(value["schema"], project::PROJECT_NPM_BUILD_SCHEMA_V4);
    assert_eq!(value["project_schema"], project::PROJECT_SCHEMA_V5);
    let artifacts = artifacts(&value);
    assert_eq!(
        artifacts.iter().map(|(path, _)| *path).collect::<Vec<_>>(),
        [
            "app.wasm",
            "semaprax.js",
            "semaprax.bindings.js",
            "semaprax.bindings.d.ts",
            "semaprax.command.json",
            "semaprax.command.js",
            "package.json",
        ]
    );
    let metadata_bytes = artifacts
        .iter()
        .find(|(path, _)| *path == "semaprax.command.json")
        .unwrap()
        .1
        .as_slice();
    assert!(metadata_bytes.ends_with(b"\n"));
    assert_eq!(
        metadata_bytes.iter().filter(|byte| **byte == b'\n').count(),
        1
    );
    let metadata: serde_json::Value = serde_json::from_slice(metadata_bytes).unwrap();
    assert_eq!(
        metadata,
        serde_json::json!({
            "schema": "semaprax.useful-data-command.v2",
            "package": "spxgrep",
            "version": "0.1.0",
            "command": "spxgrep.contains",
            "input": "stdin-bytes+one-utf8-arg.v1",
            "capabilities": [
                "process.args.read",
                "process.stderr.write",
                "process.stdin.read",
                "process.stdout.write"
            ],
            "stdout_transcript": {
                "policy": "success-only.v1",
                "max_bytes": 65_536,
                "max_writes_per_path": 1
            },
            "result": "bool",
            "exits": {
                "matched": 0,
                "not_matched": 1,
                "adapter_failure": 2
            },
            "wasm": {
                "path": "app.wasm",
                "sha256": metadata["wasm"]["sha256"].clone()
            }
        })
    );
    let wasm_digest = metadata["wasm"]["sha256"].as_str().unwrap();
    assert_eq!(wasm_digest.len(), 64);
    assert!(wasm_digest.bytes().all(|byte| byte.is_ascii_hexdigit()));
}

#[test]
fn v5_reuses_only_the_frozen_v4_facade_surface_and_v3_carrier_stays_valid() {
    let v4 = build(&fixture_v4());
    let v5 = build(&fixture_v5());
    let v4_value: serde_json::Value = serde_json::from_str(v4.envelope()).unwrap();
    let v5_value: serde_json::Value = serde_json::from_str(v5.envelope()).unwrap();
    assert_eq!(v4_value["schema"], project::PROJECT_NPM_BUILD_SCHEMA_V3);
    assert_eq!(v5_value["schema"], project::PROJECT_NPM_BUILD_SCHEMA_V4);
    project::ProjectNpmBuild::inspect_envelope(v4.envelope(), v4.max_bytes()).unwrap();
    let v4_artifacts = artifacts(&v4_value);
    let v5_artifacts = artifacts(&v5_value);
    for path in ["semaprax.bindings.d.ts", "package.json"] {
        assert_eq!(
            v4_artifacts.iter().find(|row| row.0 == path).unwrap().1,
            v5_artifacts.iter().find(|row| row.0 == path).unwrap().1,
            "frozen facade artifact changed: {path}"
        );
    }
    let v4_adapter = &v4_artifacts
        .iter()
        .find(|row| row.0 == "semaprax.command.js")
        .unwrap()
        .1;
    let v5_adapter = &v5_artifacts
        .iter()
        .find(|row| row.0 == "semaprax.command.js")
        .unwrap()
        .1;
    assert_ne!(v4_adapter, v5_adapter);
    let v5_adapter = std::str::from_utf8(v5_adapter).unwrap();
    assert!(
        v5_adapter.contains("if (!matched) { runtime.discardTranscript(); process.exitCode = 1; }")
    );
    assert!(v5_adapter.contains("else { const transcript = runtime.takeTranscript();"));
    assert!(v5_adapter.contains("rejectLoneSurrogate(argv[2]);"));
}

#[test]
fn v5_metadata_policy_tampering_is_rejected() {
    let build = build(&fixture_v5());
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let metadata_hex = value["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "semaprax.command.json")
        .unwrap()["hex"]
        .as_str()
        .unwrap();
    let metadata = String::from_utf8(decode_hex(metadata_hex)).unwrap();
    let hostile = metadata.replace("success-only.v1", "success-only.v0");
    assert_ne!(hostile, metadata);
    let hostile_hex = hostile
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let envelope = build.envelope().replacen(metadata_hex, &hostile_hex, 1);
    assert!(project::ProjectNpmBuild::inspect_envelope(&envelope, build.max_bytes()).is_err());
}

#[test]
fn v5_false_after_write_discards_transcript_and_exits_one() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = false_after_write_project();
    let build = build(&root);
    let package = root.join("package");
    write_package(&package, &build);
    std::fs::write(
        package.join("guest-false.mjs"),
        r#"import fs from "node:fs";
import { instantiate } from "./semaprax.bindings.js";
const runtime = await instantiate(new Uint8Array(fs.readFileSync("./app.wasm")));
const input = new Uint8Array([115, 101, 99, 114, 101, 116]);
const result = runtime.call("false-write.run", input, new Uint8Array());
if (result !== false) throw new Error("false command result changed");
if (runtime.takeTranscript().byteLength !== 0) throw new Error("false command transcript escaped");
process.stdout.write("guest-false-pristine\n");
"#,
    )
    .unwrap();
    let guest = Command::new("node")
        .arg("guest-false.mjs")
        .current_dir(&package)
        .output()
        .unwrap();
    assert!(
        guest.status.success(),
        "{}",
        String::from_utf8_lossy(&guest.stderr)
    );
    assert_eq!(guest.stdout, b"guest-false-pristine\n");
    assert!(guest.stderr.is_empty());

    let mut child = Command::new("node")
        .arg("semaprax.command.js")
        .arg("unused")
        .current_dir(&package)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"secret\0bytes")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn v5_node_adapter_rejects_surrogates_and_maps_stdout_failure_to_exit_two() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-v5-node-hostile-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
    ));
    let build = build(&fixture_v5());
    write_package(&root, &build);

    std::fs::write(
        root.join("surrogate.mjs"),
        "import process from \"node:process\";\nprocess.argv.splice(0, process.argv.length, \"node\", \"semaprax.command.js\", \"\\ud800\");\nawait import(\"./semaprax.command.js\");\n",
    )
    .unwrap();
    let surrogate = Command::new("node")
        .arg("surrogate.mjs")
        .current_dir(&root)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(surrogate.status.code(), Some(2));
    assert!(surrogate.stdout.is_empty());
    assert_eq!(surrogate.stderr, b"spxgrep: command failed\n");

    std::fs::write(
        root.join("broken-stdout.mjs"),
        r#"import process from "node:process";
process.argv.splice(0, process.argv.length, "node", "semaprax.command.js", "");
Object.defineProperty(process.stdout, "write", { value: () => { queueMicrotask(() => process.stdout.emit("error", new Error("injected stdout failure"))); return false; } });
await import("./semaprax.command.js");
"#,
    )
    .unwrap();
    let mut broken = Command::new("node")
        .arg("broken-stdout.mjs")
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    broken.stdin.take().unwrap().write_all(b"matched").unwrap();
    let broken = broken.wait_with_output().unwrap();
    assert_eq!(broken.status.code(), Some(2));
    assert!(broken.stdout.is_empty());
    assert_eq!(broken.stderr, b"spxgrep: command failed\n");

    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn project_v5_native_build_dispatches_the_fixed_command_adapter() {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let executable = std::env::temp_dir().join(format!(
        "semaprax-spxgrep-native-{}-{}{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed),
        suffix,
    ));
    project::with_authenticated_project(&fixture_v5().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        snapshot.build_native(&executable)
    })
    .unwrap();

    let run = |args: &[&str], input: &[u8]| {
        let mut child = Command::new(&executable)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    };

    let input = b"hello\0world\xff";
    let matched = run(&["world"], input);
    assert_eq!(matched.status.code(), Some(0));
    assert_eq!(matched.stdout, input);
    assert!(matched.stderr.is_empty());

    let absent = run(&["absent"], input);
    assert_eq!(absent.status.code(), Some(1));
    assert!(absent.stdout.is_empty());
    assert!(absent.stderr.is_empty());

    let missing = run(&[], b"");
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    assert_eq!(missing.stderr, b"SEMAPRAX native command failed\n");

    std::fs::remove_file(&executable).unwrap();
}
