use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project;
use wasmparser::{Parser, Payload};

static SERIAL: AtomicU64 = AtomicU64::new(0);

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/spxgrep-project")
}

fn build() -> project::ProjectNpmBuild {
    project::with_authenticated_project(&fixture().join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert!(snapshot.manifest().is_v4());
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
    let mut bytes = Vec::with_capacity(encoded.len() / 2);
    let mut offset = 0;
    while offset < encoded.len() {
        bytes.push((nibble(encoded[offset]) << 4) | nibble(encoded[offset + 1]));
        offset += 2;
    }
    bytes
}

#[test]
fn command_carrier_is_exact_replayable_and_adds_no_host_io_import() {
    let build = build();
    build.verify().unwrap();
    project::ProjectNpmBuild::inspect_envelope(build.envelope(), build.max_bytes()).unwrap();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    assert_eq!(value["schema"], project::PROJECT_NPM_BUILD_SCHEMA_V3);
    let artifacts = value["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 7);
    assert_eq!(
        artifacts
            .iter()
            .map(|row| row["path"].as_str().unwrap())
            .collect::<Vec<_>>(),
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
    let wasm = decode_hex(artifacts[0]["hex"].as_str().unwrap());
    let mut imports = Vec::new();
    let mut exports = BTreeSet::new();
    for payload in Parser::new(0).parse_all(&wasm) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    exports.insert(export.unwrap().name.to_owned());
                }
            }
            _ => {}
        }
    }
    assert!(imports.iter().all(|(module, name)| {
        module == "env"
            && !name.contains("stdout")
            && !name.contains("wasi")
            && !name.contains("path")
            && !name.contains("network")
            && !name.contains("child")
    }));
    for expected in [
        "memory",
        "__spx_stdout_length_v1",
        "__spx_stdout_base_v1",
        "__spx_stdout_capacity_v1",
    ] {
        assert!(exports.contains(expected), "missing {expected}");
    }
    let mut tampered = value;
    let hex = tampered["artifacts"][5]["hex"].as_str().unwrap().to_owned();
    let replacement = if hex.starts_with('0') { "1" } else { "0" };
    tampered["artifacts"][5]["hex"] =
        serde_json::Value::String(format!("{replacement}{}", &hex[1..]));
    let envelope = serde_json::to_string(&tampered).unwrap();
    assert!(project::ProjectNpmBuild::inspect_envelope(&envelope, build.max_bytes()).is_err());
}

#[test]
fn compiler_free_command_matches_flushes_once_and_enforces_the_combined_bound() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let build = build();
    let value: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-spxgrep-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    for artifact in value["artifacts"].as_array().unwrap() {
        std::fs::write(
            root.join(artifact["path"].as_str().unwrap()),
            decode_hex(artifact["hex"].as_str().unwrap()),
        )
        .unwrap();
    }
    let run = |needle: &str, input: &[u8]| {
        let mut child = Command::new("node")
            .arg("semaprax.command.js")
            .arg(needle)
            .current_dir(&root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(input).unwrap();
        child.wait_with_output().unwrap()
    };
    let input = b"alpha\nneedle here\nomega\n";
    let matched = run("needle", input);
    assert_eq!(matched.status.code(), Some(0));
    assert_eq!(matched.stdout, input);
    assert!(matched.stderr.is_empty());

    let absent = run("missing", input);
    assert_eq!(absent.status.code(), Some(1));
    assert!(absent.stdout.is_empty());
    assert!(absent.stderr.is_empty());

    let empty = run("", input);
    assert_eq!(empty.status.code(), Some(0));
    assert_eq!(empty.stdout, input);

    let binary = [0, 0xff, 0xf0, 0x28, 0x8c, 0x28, b'x', 0];
    let binary_match = run("x", &binary);
    assert_eq!(binary_match.status.code(), Some(0));
    assert_eq!(binary_match.stdout, binary);

    let exact = run("", &vec![b'x'; 65_536]);
    assert_eq!(exact.status.code(), Some(0));
    assert_eq!(exact.stdout.len(), 65_536);
    let over = run("x", &vec![b'x'; 65_536]);
    assert_eq!(over.status.code(), Some(2));
    assert!(over.stdout.is_empty());
    assert_eq!(over.stderr, b"spxgrep: command failed\n");

    for args in [Vec::<&str>::new(), vec!["x", "extra"]] {
        let usage = Command::new("node")
            .arg("semaprax.command.js")
            .args(args)
            .current_dir(&root)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(usage.status.code(), Some(2));
        assert!(usage.stdout.is_empty());
        assert_eq!(usage.stderr, b"spxgrep: command failed\n");
    }

    std::fs::write(
        root.join("broken-stdout.mjs"),
        r#"import process from "node:process";
Object.defineProperty(process.stdout, "write", { value: (_bytes, callback) => { queueMicrotask(() => callback(new Error("injected stdout failure"))); return false; } });
await import("./semaprax.command.js");
"#,
    )
    .unwrap();
    let mut broken = Command::new("node")
        .arg("broken-stdout.mjs")
        .arg("")
        .current_dir(&root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    broken
        .stdin
        .take()
        .unwrap()
        .write_all(&vec![b'x'; 65_536])
        .unwrap();
    let broken = broken.wait_with_output().unwrap();
    assert_eq!(broken.status.code(), Some(2));
    assert_eq!(broken.stderr, b"spxgrep: command failed\n");

    let runtime_path = root.join("semaprax.js");
    let runtime = std::fs::read_to_string(&runtime_path).unwrap();
    let hostile = runtime.replace(
        "settle() { if (entries.size !== 0)",
        "settle() { throw new Error(\"injected settlement failure\"); if (entries.size !== 0)",
    );
    assert_ne!(hostile, runtime);
    std::fs::write(runtime_path, hostile).unwrap();
    std::fs::write(
        root.join("settlement.mjs"),
        r#"import fs from "node:fs";
let captured = null;
const instantiateRaw = WebAssembly.instantiate;
WebAssembly.instantiate = async (...args) => { const linked = await instantiateRaw(...args); captured = linked.instance; return linked; };
import { instantiate } from "./semaprax.bindings.js";
const runtime = await instantiate(new Uint8Array(fs.readFileSync("./app.wasm")));
let failed = false; try { runtime.call("spxgrep.contains", new TextEncoder().encode("needle"), new TextEncoder().encode("needle")); } catch { failed = true; }
if (!failed) throw new Error("settlement failure was hidden");
let escaped = false; try { runtime.takeTranscript(); escaped = true; } catch {}
if (escaped) throw new Error("failed transcript escaped");
if (captured === null) throw new Error("raw instance was not observed");
const rawTranscript = new Uint8Array(captured.exports.memory.buffer, 131072, 65536);
if (rawTranscript.some(byte => byte !== 0)) throw new Error("failed transcript remained in raw memory");
process.stdout.write("settlement-fail-stop\n");
"#,
    )
    .unwrap();
    let settlement = Command::new("node")
        .arg("settlement.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        settlement.status.success(),
        "{}",
        String::from_utf8_lossy(&settlement.stderr)
    );
    assert_eq!(settlement.stdout, b"settlement-fail-stop\n");
    std::fs::remove_dir_all(root).unwrap();
}
