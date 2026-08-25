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
    encoded
        .chunks_exact(2)
        .map(|pair| (nibble(pair[0]) << 4) | nibble(pair[1]))
        .collect()
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
    for path in [
        "semaprax.bindings.d.ts",
        "semaprax.command.js",
        "package.json",
    ] {
        assert_eq!(
            v4_artifacts.iter().find(|row| row.0 == path).unwrap().1,
            v5_artifacts.iter().find(|row| row.0 == path).unwrap().1,
            "frozen facade artifact changed: {path}"
        );
    }
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
