//! Physical admission evidence for the later owned npm profiles. These tests
//! consume actual generated packages; Node absence is not passing evidence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, with_authenticated_project,
    ProjectNpmBuild, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
    PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FACT: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";

fn directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-profile-inputs-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

fn consume_package(build: &ProjectNpmBuild, label: &str, profile: serde_json::Value) {
    build.verify().unwrap();
    let directory = directory(label);
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let artifacts = envelope["artifacts"].as_array().unwrap();
    assert_eq!(artifacts.len(), 6);
    for artifact in artifacts {
        let path = artifact["path"].as_str().unwrap();
        assert!(matches!(
            path,
            "app.wasm"
                | "semaprax.js"
                | "semaprax.bindings.js"
                | "semaprax.bindings.d.ts"
                | "semaprax.api.json"
                | "package.json"
        ));
        let hex = artifact["hex"].as_str().unwrap();
        let bytes = (0..hex.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
            .collect::<Vec<_>>();
        if path == "semaprax.js" {
            let runtime = std::str::from_utf8(&bytes).unwrap();
            assert!(runtime.contains("function snapshotArguments("));
            assert!(!runtime.contains("arrayBufferSlice"));
            assert!(runtime.contains("reflectApply(typedTag,value,[])!==\"Uint8Array\""));
        }
        fs::write(directory.join(path), bytes).unwrap();
    }
    fs::write(
        directory.join("admission.mjs"),
        include_str!("fixtures/owned_data_input_admission_v8.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("admission.mjs")
        .arg(profile.to_string())
        .current_dir(&directory)
        .output()
        .expect("Node is required for the owned-profile input-admission gate");
    assert!(
        output.status.success(),
        "{label}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "owned-input-admission-v8-ok"
    );
}

#[test]
fn v10_direct_variant_and_utf8_facades_admit_before_payload_allocation() {
    for (label, result, value, mixed) in [
        ("v10-direct", "Bytes", "bytes_copy(input)", false),
        (
            "v10-option",
            "Option<Bytes>",
            "Option<Bytes>::Some { value: bytes_copy(input) }",
            false,
        ),
        (
            "v10-result",
            "Result<Bytes, i64>",
            "Result<Bytes, i64>::Ok { value: bytes_copy(input) }",
            false,
        ),
        ("v10-utf8-mixed", "Bytes", "bytes_copy(input)", true),
    ] {
        let extra = if mixed {
            r#"@id("probe.greeting") fn greeting() -> string { "hello\u{0}世界" }
@id("probe.mixed") fn mixed(input: borrow Slice<u8>, text: borrow str, value: i64, flag: bool) -> Bytes { if flag { bytes_copy(input) } else { bytes_copy(str_as_bytes(text)) } }"#
        } else {
            ""
        };
        let source = format!("module probe.input;\n@id(\"probe.bytes\") fn copy(input: borrow Slice<u8>, text: borrow str, other: borrow Slice<u8>) -> {result} {{ {value} }}\n{extra}\n@id(\"probe.main\") fn main() -> i64 {{ 0 }}\n");
        let program =
            semaprax::hir::resolve(&semaprax::check(&source, "input.spx").unwrap()).unwrap();
        let mut selected = vec!["probe.bytes".to_owned()];
        if mixed {
            selected.extend(["probe.greeting".to_owned(), "probe.mixed".to_owned()]);
        }
        let descriptor = derive_public_api_descriptor(
            &program,
            &selected,
            PublicApiSubject {
                project_schema: PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
                project_revision: FACT,
                workspace_revision: FACT,
                project_graph_digest: FACT,
            },
        )
        .unwrap();
        let build = prepare_owned_data_npm_build(
            &program,
            &descriptor,
            "profile-inputs",
            "0.1.0",
            MAX_PROJECT_NPM_BUILD_BYTES,
        )
        .unwrap();
        consume_package(
            &build,
            label,
            serde_json::json!({"requireMixed":mixed,"utf8":mixed,"moduleBoundaries":true}),
        );
    }
}

#[test]
fn v9_record_facade_admits_before_copies_and_preserves_frozen_fields() {
    let directory = directory("v9-project");
    fs::create_dir(directory.join("src")).unwrap();
    let manifest = "schema = \"semaprax.project.v9\"\nname = \"profile-inputs\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"probe.input\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"probe.bytes\", \"probe.mixed\"]\ntests = [\"probe.tests\"]\n";
    fs::write(directory.join("semaprax.toml"), manifest).unwrap();
    let source = r#"module probe.input;
@id("probe.packet") record Packet {
    @id("probe.payload") payload: Bytes,
    @id("probe.kind") kind: i64,
    @id("probe.valid") valid: bool,
    @id("probe.size") size: usize,
}
@id("probe.bytes")
fn copy(input: borrow Slice<u8>, text: borrow str, other: borrow Slice<u8>) -> Packet {
    Packet { payload: bytes_copy(input), kind: 7, valid: true, size: byte_len(input) }
}
@id("probe.mixed")
fn mixed(input: borrow Slice<u8>, text: borrow str, value: i64, flag: bool) -> Packet {
    if flag {
        Packet { payload: bytes_copy(input), kind: 7, valid: flag, size: byte_len(input) }
    } else {
        Packet { payload: bytes_copy(str_as_bytes(text)), kind: 7, valid: flag, size: byte_len(str_as_bytes(text)) }
    }
}
@id("probe.main") fn main() -> i64 { 0 }
"#;
    for (path, source) in [
        ("src/app.spx", source),
        (
            "src/tests.spx",
            "module probe.tests;\n@id(\"probe.tests.main\") fn main() -> i64 { 0 }\n",
        ),
    ] {
        let source =
            semaprax::format::canonical(&semaprax::parse(source, Path::new(path)).unwrap());
        fs::write(directory.join(path), source).unwrap();
    }
    with_authenticated_project(&directory.join("semaprax.toml"), |snapshot| {
        let descriptor = snapshot.flat_owned_record_api_descriptor()?;
        let fields = descriptor.exports()[0].fields();
        assert_eq!(fields.len(), 4);
        let profile = serde_json::json!({
            "requireMixed":true,
            "moduleBoundaries":true,
            "recordFields":{
                "payload":fields[0].host_name(),
                "kind":fields[1].host_name(),
                "valid":fields[2].host_name(),
                "size":fields[3].host_name(),
            }
        });
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        consume_package(&build, "v9-record", profile);
        Ok(())
    })
    .unwrap();
}
