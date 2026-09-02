//! Actual generated owned npm facades under test-only engine/import faults.
//! These authored gates are not executed as part of this implementation.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../project_owned_failure_fsm_v1/baseline.rs"]
mod baseline;

use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, with_authenticated_project,
    ProjectNpmBuild, PublicApiSubject, MAX_PROJECT_NPM_BUILD_BYTES,
    PUBLIC_OWNED_DATA_PROJECT_SCHEMA, PUBLIC_OWNED_UTF8_PROJECT_SCHEMA,
};

const FACT: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const ARTIFACTS: [&str; 6] = [
    "app.wasm",
    "semaprax.js",
    "semaprax.bindings.js",
    "semaprax.bindings.d.ts",
    "semaprax.api.json",
    "package.json",
];
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-owned-fsm-{label}-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&path).unwrap();
    path.canonicalize().unwrap()
}

fn source(result: &str, value: &str, declarations: &str, extra: &str) -> String {
    include_str!("../project_owned_failure_fsm_v1/source.spx")
        .replace("__DECLARATIONS__", declarations)
        .replace("__RESULT__", result)
        .replace("__VALUE__", value)
        .replace("__EXTRA__", extra)
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    build.verify().unwrap();
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    let rows = envelope["artifacts"].as_array().unwrap();
    assert_eq!(rows.len(), ARTIFACTS.len());
    rows.iter()
        .zip(ARTIFACTS)
        .map(|(row, expected)| {
            assert_eq!(row["path"], expected);
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (expected.to_owned(), bytes)
        })
        .collect()
}

fn remove_exact(directory: &Path, names: &[&str]) {
    let entries = fs::read_dir(directory)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(entries.len(), names.len());
    for entry in &entries {
        let name = entry.file_name();
        assert!(names.contains(&name.to_str().unwrap()));
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        assert!(metadata.is_file() && !metadata.file_type().is_symlink());
    }
    for name in names {
        fs::remove_file(directory.join(name)).unwrap();
    }
    fs::remove_dir(directory).unwrap();
}

fn consume(
    build: &ProjectNpmBuild,
    label: &str,
    config: serde_json::Value,
    expected_wasm: Option<&[u8]>,
    descriptor: &[u8],
    descriptor_digest: &str,
) {
    let rows = artifacts(build);
    assert_eq!(
        rows[2].1,
        b"export { instantiate, exportIds, wasmSha256, default } from \"./semaprax.js\";\n"
    );
    if let Some(expected) = expected_wasm {
        // Current direct-lowering consistency, not a historical Wasm KAT.
        // The production diff and existing legacy KATs own byte preservation.
        assert_eq!(rows[0].1, expected);
    }
    // Renderer routing is driven by the real descriptor/result vocabulary,
    // never by invoking a private legacy fragment in isolation.
    let runtime = std::str::from_utf8(&rows[1].1).unwrap();
    assert!(runtime.contains("function snapshotArguments("));
    baseline::assert_artifacts(&rows, &config, descriptor, descriptor_digest);
    let directory = directory(label);
    for (name, bytes) in rows {
        fs::write(directory.join(name), bytes).unwrap();
    }
    fs::write(
        directory.join("probe.mjs"),
        include_str!("../project_owned_failure_fsm_v1/probe.mjs"),
    )
    .unwrap();
    fs::write(
        directory.join("utf8.mjs"),
        include_str!("../project_owned_failure_fsm_v1/utf8.mjs"),
    )
    .unwrap();
    fs::write(
        directory.join("identity.mjs"),
        include_str!("../project_owned_failure_fsm_v1/identity.mjs"),
    )
    .unwrap();
    fs::write(
        directory.join("result.mjs"),
        include_str!("../project_owned_failure_fsm_v1/result.mjs"),
    )
    .unwrap();
    fs::write(
        directory.join("finalization.mjs"),
        include_str!("../project_owned_failure_fsm_v1/finalization.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .arg(config.to_string())
        .current_dir(&directory)
        .output()
        .expect("Node is required for the owned npm failure-state gate");
    assert!(
        output.status.success(),
        "{label}: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "owned-failure-fsm-ok"
    );
    let mut names = ARTIFACTS.to_vec();
    names.push("probe.mjs");
    names.push("utf8.mjs");
    names.push("identity.mjs");
    names.push("result.mjs");
    names.push("finalization.mjs");
    remove_exact(&directory, &names);
}

#[test]
fn v8_and_v10_real_direct_variant_and_mixed_packages_fail_stop() {
    for (schema, prefix) in [
        (PUBLIC_OWNED_DATA_PROJECT_SCHEMA, "v8"),
        (PUBLIC_OWNED_UTF8_PROJECT_SCHEMA, "v10"),
    ] {
        for family in ["direct", "variant", "mixed"] {
            let utf8 = prefix == "v10" && family == "mixed";
            let (result, value) = if family == "variant" {
                (
                    "Result<Bytes, i64>",
                    "Result<Bytes, i64>::Ok { value: staged }",
                )
            } else {
                ("Bytes", "staged")
            };
            let mut selected = vec!["case.before".to_owned(), "case.copy".to_owned()];
            let extra = if family == "variant" {
                selected.extend(["case.err".into(), "case.none".into()]);
                "@id(\"case.err\") fn err() -> Result<Bytes, i64> { Result<Bytes, i64>::Err { error: -7 } }\n@id(\"case.none\") fn none() -> Option<Bytes> { Option<Bytes>::None {} }"
            } else if family == "mixed" {
                selected.push("case.flag".into());
                if utf8 {
                    selected.push("case.text".into());
                    "@id(\"case.flag\") fn flag(value: bool) -> bool { value }\n@id(\"case.text\") fn text() -> string { \"\\u{feff}A\\u{0}λ\" }"
                } else {
                    "@id(\"case.flag\") fn flag(value: bool) -> bool { value }"
                }
            } else {
                ""
            };
            selected.push("case.utf8".into());
            let text = source(result, value, "", extra);
            let checked = semaprax::check(&text, "failure-fsm.spx").unwrap();
            let canonical = semaprax::format::canonical(&checked);
            let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
            assert_eq!(
                semaprax::graph::revision(&checked),
                semaprax::graph::revision(&reparsed)
            );
            let program = semaprax::hir::resolve(&checked).unwrap();
            let descriptor = derive_public_api_descriptor(
                &program,
                &selected,
                PublicApiSubject {
                    project_schema: schema,
                    project_revision: FACT,
                    workspace_revision: FACT,
                    project_graph_digest: FACT,
                },
            )
            .unwrap();
            let wasm =
                semaprax::wasm::emit_resolved_module_with_owned_data_exports(&program, &descriptor)
                    .unwrap();
            let build = prepare_owned_data_npm_build(
                &program,
                &descriptor,
                "owned-fsm",
                "0.1.0",
                MAX_PROJECT_NPM_BUILD_BYTES,
            )
            .unwrap();
            consume(
                &build,
                &format!("{prefix}-{family}"),
                serde_json::json!({"family":family,"utf8":utf8,"schema":prefix}),
                Some(&wasm),
                &descriptor.canonical_bytes(),
                &descriptor.digest(),
            );
        }
    }
}

#[test]
fn v9_real_project_record_package_fails_stop_before_publication() {
    let root = directory("v9-project");
    fs::create_dir(root.join("src")).unwrap();
    let declarations = "@id(\"case.packet\") record Packet { @id(\"case.payload\") payload: Bytes, @id(\"case.kind\") kind: i64, @id(\"case.valid\") valid: bool, @id(\"case.size\") size: usize, }";
    let app = source(
        "Packet",
        "Packet { payload: staged, kind: 7, valid: true, size: byte_len(input) }",
        declarations,
        "",
    );
    for (path, text) in [
        ("src/app.spx", app.as_str()),
        (
            "src/tests.spx",
            "module case.tests; @id(\"case.tests.main\") fn main() -> i64 { 0 }",
        ),
    ] {
        let parsed = semaprax::parse(text, Path::new(path)).unwrap();
        fs::write(root.join(path), semaprax::format::canonical(&parsed)).unwrap();
    }
    let manifest = "schema = \"semaprax.project.v9\"\nname = \"owned-fsm\"\nversion = \"0.1.0\"\nprofile = \"flat-owned-record-api.v1\"\nentry = \"case.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"case.before\", \"case.copy\", \"case.utf8\"]\ntests = [\"case.tests\"]\n";
    fs::write(root.join("semaprax.toml"), manifest).unwrap();
    with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let descriptor = snapshot.flat_owned_record_api_descriptor()?;
        let fields = descriptor.exports()[0].fields();
        let config = serde_json::json!({"family":"record","utf8":false,"schema":"v9","fields":fields.iter().map(|field| field.host_name()).collect::<Vec<_>>()});
        let build = snapshot.build_npm_inline(MAX_PROJECT_NPM_BUILD_BYTES)?;
        consume(&build, "v9-record", config, None, &descriptor.canonical_bytes(), &descriptor.digest());
        Ok(())
    }).unwrap();
    remove_exact(&root.join("src"), &["app.spx", "tests.spx"]);
    remove_exact(&root, &["semaprax.toml"]);
}
