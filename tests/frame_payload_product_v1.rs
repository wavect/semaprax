#[path = "support/full_toolchain.rs"]
mod full_toolchain;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "support/native_rust_cargo.rs"]
mod native_rust_cargo;

#[path = "support/native_rust_target.rs"]
mod native_rust_target;

use semaprax::hir;
use semaprax::interpreter::{
    evaluate_resolved_owned_data, OwnedDataCleanupEvent, OwnedDataEvaluationOutcome,
    OwnedDataValue, DEFAULT_MAX_STEPS,
};
use semaprax::project::{
    derive_public_api_descriptor, prepare_owned_data_npm_build, ProjectNpmBuild,
    PublicApiResultType, PublicApiSubject, PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const FRAME_SOURCE: &str = include_str!("../examples/frame-payload-project/src/frame.spx");
const MANIFEST: &str = include_str!("../examples/frame-payload-project/semaprax.toml");
const CORPUS: &[u8] = include_bytes!("../examples/frame-payload-project/corpus.json");

#[path = "frame_payload_product_v1/adversarial.rs"]
mod adversarial;
#[path = "frame_payload_product_v1/adversarial_consumers.rs"]
mod adversarial_consumers;
#[path = "frame_payload_product_v1/backend_equivalence.rs"]
mod backend_equivalence;
#[path = "frame_payload_product_v1/consumer_acceptance.rs"]
mod consumer_acceptance;
#[path = "frame_payload_product_v1/raw_wasm.rs"]
mod raw_wasm;
#[path = "frame_payload_product_v1/subject_binding.rs"]
mod subject_binding;
use backend_equivalence::{assert_interpreter_corpus, assert_native_corpus};
const SELECTED: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];

fn temporary(label: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().canonicalize().unwrap().join(format!(
        "semaprax-frame-payload-{label}-{}-{id}",
        std::process::id()
    ))
}

fn subject() -> PublicApiSubject<'static> {
    PublicApiSubject {
        project_schema: PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
        project_revision: "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        workspace_revision:
            "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        project_graph_digest:
            "sha256:3333333333333333333333333333333333333333333333333333333333333333",
    }
}

fn resolve(source: &str) -> hir::ResolvedProgram {
    let source = format!("{source}\n@id(\"frame.fixture.main\")\nfn main() -> i64 {{ 0 }}\n");
    let checked = semaprax::check(&source, Path::new("frame.spx")).unwrap();
    hir::resolve(&checked).unwrap()
}

fn artifacts(build: &ProjectNpmBuild) -> Vec<(String, Vec<u8>)> {
    let envelope: serde_json::Value = serde_json::from_str(build.envelope()).unwrap();
    envelope["artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let hex = row["hex"].as_str().unwrap();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
                .collect();
            (row["path"].as_str().unwrap().to_owned(), bytes)
        })
        .collect()
}

fn write_package(build: &ProjectNpmBuild, directory: &Path) {
    fs::create_dir_all(directory).unwrap();
    for (path, bytes) in artifacts(build) {
        fs::write(directory.join(path), bytes).unwrap();
    }
}

fn run_node_consumer(root: &Path) {
    let output = Command::new("node")
        .arg("consumer.mjs")
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "frame-payload-web-v1-ok"
    );
}

#[test]
fn canonical_product_corpus_and_manifest_are_exact() {
    let typescript = include_str!("../examples/frame-payload-web/consumer.ts");
    assert!(typescript.contains("new Uint8Array(await response.arrayBuffer())"));
    assert!(typescript.contains("await instantiate(wasm)"));
    assert!(!typescript.contains("instantiate(new URL("));
    assert_eq!(
        CORPUS,
        include_bytes!("../examples/frame-payload-web/corpus.json")
    );
    assert_eq!(
        CORPUS,
        include_bytes!("../examples/frame-payload-rust/corpus.json")
    );
    let corpus: serde_json::Value = serde_json::from_slice(CORPUS).unwrap();
    let rows = corpus["cases"].as_array().unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "empty",
            "short",
            "bad-magic",
            "zero",
            "text",
            "nul",
            "invalid-utf8",
            "max-65528",
            "mismatch",
        ]
    );
    assert_eq!(corpus["maximum_frame_bytes"], 65_536);
    assert_eq!(rows[7]["payload_length"], 65_528);
    assert_eq!(
        MANIFEST,
        "schema = \"semaprax.project.v8\"\nname = \"frame-payload\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"frame_payload.app\"\nsources = [\"src/app.spx\", \"src/frame.spx\", \"src/tests.spx\"]\nweb_exports = [\"frame.payload\", \"frame.payload-maybe\", \"frame.payload-result\"]\ntests = [\"frame_payload.tests\"]\n"
    );
    assert!(FRAME_SOURCE.contains("first == 83u8 && second == 80u8"));
    assert!(FRAME_SOURCE.contains("fourth == 49u8"));
    assert!(FRAME_SOURCE.contains("Result<Bytes, i64>::Err { error: 1 }"));
    assert!(FRAME_SOURCE.contains("Result<Bytes, i64>::Err { error: 2 }"));
    assert!(FRAME_SOURCE.contains("Result<Bytes, i64>::Err { error: 3 }"));
}

#[test]
fn descriptor_npm_corpus_and_display_rename_replay_without_project_routing() {
    let selected = SELECTED.map(str::to_owned);
    let before = resolve(FRAME_SOURCE);
    let renamed_source = FRAME_SOURCE.replace("fn payload_result(", "fn decoded_payload_result(");
    assert_ne!(renamed_source, FRAME_SOURCE);
    let after = resolve(&renamed_source);
    let before_descriptor = derive_public_api_descriptor(&before, &selected, subject()).unwrap();
    let after_descriptor = derive_public_api_descriptor(&after, &selected, subject()).unwrap();
    assert_eq!(
        before_descriptor.canonical_bytes(),
        after_descriptor.canonical_bytes()
    );
    assert_eq!(before_descriptor.digest(), after_descriptor.digest());
    let result = before_descriptor
        .exports()
        .iter()
        .find(|row| row.stable_id().as_str() == "frame.payload-result")
        .unwrap();
    assert_eq!(result.typescript_name(), "frame.payload-result");
    assert_eq!(
        result.rust_method_name(),
        "spx_frame_dot_payload_hyphen_result"
    );
    assert_eq!(result.result(), PublicApiResultType::ResultOwnedBytesI64);

    let before_build = prepare_owned_data_npm_build(
        &before,
        &before_descriptor,
        "frame-payload",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    let after_build = prepare_owned_data_npm_build(
        &after,
        &after_descriptor,
        "frame-payload",
        "0.1.0",
        40 * 1024 * 1024,
    )
    .unwrap();
    before_build.verify().unwrap();
    after_build.verify().unwrap();
    for build in [&before_build, &after_build] {
        let declarations = artifacts(build)
            .into_iter()
            .find(|(path, _)| path == "semaprax.bindings.d.ts")
            .map(|(_, bytes)| String::from_utf8(bytes).unwrap())
            .unwrap();
        assert!(
            declarations.contains("readonly \"frame.payload\": (arg0: Uint8Array) => Uint8Array;")
        );
        assert!(declarations
            .contains("readonly \"frame.payload-maybe\": (arg0: Uint8Array) => OptionalBytes;"));
        assert!(declarations.contains(
            "readonly \"frame.payload-result\": (arg0: Uint8Array) => SemapraxResult<Uint8Array, bigint>;"
        ));
    }

    let root = temporary("npm");
    for (name, build) in [("before", &before_build), ("after", &after_build)] {
        let consumer = root.join(name);
        write_package(build, &consumer.join("generated"));
        fs::write(
            consumer.join("consumer.mjs"),
            include_bytes!("../examples/frame-payload-web/consumer.mjs"),
        )
        .unwrap();
        fs::write(consumer.join("corpus.json"), CORPUS).unwrap();
        consumer_acceptance::write_web_support(&consumer);
        run_node_consumer(&consumer);
    }
    fs::remove_dir_all(root).unwrap();
}

fn decode_hex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn corpus_frames() -> Vec<(String, Vec<u8>, bool, i64)> {
    let corpus: serde_json::Value = serde_json::from_slice(CORPUS).unwrap();
    corpus["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            let frame = if row["kind"] == "hex" {
                decode_hex(row["frame_hex"].as_str().unwrap())
            } else {
                assert_eq!(row["kind"], "generated-index-mod-256");
                let length = row["payload_length"].as_u64().unwrap() as usize;
                let payload = (0..length).map(|index| index as u8).collect::<Vec<_>>();
                let mut frame = Vec::with_capacity(length + 8);
                frame.extend_from_slice(b"SPX1");
                frame.extend_from_slice(&(length as u32).to_be_bytes());
                frame.extend_from_slice(&payload);
                frame
            };
            (
                row["name"].as_str().unwrap().to_owned(),
                frame,
                row["valid"].as_bool().unwrap(),
                row["error"].as_i64().unwrap_or(0),
            )
        })
        .collect()
}

fn c_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "UINT8_C(0)".to_owned();
    }
    bytes
        .iter()
        .map(|byte| format!("UINT8_C({byte})"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn reference_interpreter_runs_the_exact_corpus_with_status_identity_and_cleanup() {
    let program = resolve(FRAME_SOURCE);
    assert_interpreter_corpus(&program);
}

#[test]
fn native_owned_data_provider_runs_the_exact_corpus_at_o0_and_o2() {
    let selected = SELECTED.map(str::to_owned);
    let program = resolve(FRAME_SOURCE);
    let descriptor = derive_public_api_descriptor(&program, &selected, subject()).unwrap();
    let artifact = semaprax::codegen::emit_native_owned_data_provider(
        &program,
        &selected,
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    assert_native_corpus(artifact.source(), "native-provider");
}

fn copy_project(destination: &Path, renamed: bool) {
    fs::create_dir_all(destination.join("src")).unwrap();
    fs::write(destination.join("semaprax.toml"), MANIFEST).unwrap();
    fs::write(
        destination.join("src/app.spx"),
        include_bytes!("../examples/frame-payload-project/src/app.spx"),
    )
    .unwrap();
    let frame = if renamed {
        FRAME_SOURCE.replace("fn payload_result(", "fn decoded_payload_result(")
    } else {
        FRAME_SOURCE.to_owned()
    };
    fs::write(destination.join("src/frame.spx"), frame).unwrap();
    fs::write(
        destination.join("src/tests.spx"),
        include_bytes!("../examples/frame-payload-project/src/tests.spx"),
    )
    .unwrap();
}

fn build(binary: &Path, manifest: &Path, target: &str, output: &Path) {
    let mut command = Command::new(binary);
    command
        .args(["build", "--manifest-path"])
        .arg(manifest)
        .args(["--target", target, "-o"])
        .arg(output);
    if target == "rust" {
        let clang = configured_tool("CLANG", &["/usr/bin/clang"]);
        let archiver = if cfg!(target_os = "macos") {
            configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/libtool"])
        } else {
            configured_tool("SEMAPRAX_ARCHIVER", &["/usr/bin/ar", "/bin/ar"])
        };
        command
            .env("CLANG", clang)
            .env("SEMAPRAX_ARCHIVER", archiver);
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "target={target} stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn configured_tool(variable: &str, candidates: &[&str]) -> PathBuf {
    if let Some(configured) = std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute() && path.is_file())
    {
        #[cfg(windows)]
        if variable == "SEMAPRAX_ARCHIVER" {
            return configured;
        }
        if let Ok(canonical) = configured.canonicalize() {
            return canonical;
        }
    }
    candidates
        .iter()
        .map(PathBuf::from)
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_absolute() && path.is_file())
        .unwrap_or_else(|| panic!("{variable} must name an installed absolute tool"))
}

#[test]
fn project_v8_npm_and_rust_routes_run_the_same_corpus_before_and_after_display_rename() {
    let root = temporary("project-v8-routes");
    let before = root.join("before-project");
    let after = root.join("after-project");
    copy_project(&before, false);
    copy_project(&after, true);
    let binary = Path::new(full_toolchain::binary());
    let mut bound_subjects = Vec::new();

    for (label, project) in [("before", &before), ("after", &after)] {
        let npm_consumer = root.join(format!("{label}-web"));
        fs::create_dir_all(&npm_consumer).unwrap();
        fs::write(
            npm_consumer.join("consumer.mjs"),
            include_bytes!("../examples/frame-payload-web/consumer.mjs"),
        )
        .unwrap();
        fs::write(npm_consumer.join("corpus.json"), CORPUS).unwrap();
        consumer_acceptance::write_web_support(&npm_consumer);
        build(
            binary,
            &project.join("semaprax.toml"),
            "npm",
            &npm_consumer.join("generated"),
        );
        run_node_consumer(&npm_consumer);
        adversarial_consumers::run_node(&npm_consumer);

        let rust_consumer = root.join(format!("{label}-rust"));
        let rust_sdk = root.join(format!("{label}-generated-sdk"));
        fs::create_dir_all(rust_consumer.join("src")).unwrap();
        let rust_manifest = include_str!("../examples/frame-payload-rust/Cargo.toml").replace(
            "../frame-payload-generated-sdk",
            &format!("../{label}-generated-sdk"),
        );
        fs::write(rust_consumer.join("Cargo.toml"), &rust_manifest).unwrap();
        fs::write(
            rust_consumer.join("src/main.rs"),
            include_bytes!("../examples/frame-payload-rust/src/main.rs"),
        )
        .unwrap();
        fs::write(rust_consumer.join("corpus.json"), CORPUS).unwrap();
        let lock = include_bytes!("../examples/frame-payload-rust/Cargo.lock");
        fs::write(rust_consumer.join("Cargo.lock"), lock).unwrap();
        build(binary, &project.join("semaprax.toml"), "rust", &rust_sdk);
        bound_subjects.push(subject_binding::verify_product(
            project,
            &npm_consumer.join("generated"),
            &rust_sdk,
        ));
        let cargo_target = native_rust_target::CargoTarget::new();
        let result = native_rust_cargo::cargo_command()
            .args(["run", "--quiet", "--locked", "--offline", "--manifest-path"])
            .arg(rust_consumer.join("Cargo.toml"))
            .current_dir(&rust_consumer)
            .env("CARGO_TARGET_DIR", cargo_target.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&result.stdout).trim(),
            "frame-payload-rust-v1-ok"
        );
        assert_eq!(
            fs::read(rust_consumer.join("Cargo.lock")).unwrap(),
            lock.as_slice()
        );
        adversarial_consumers::run_rust(
            &root.join(format!("{label}-rust-adversarial")),
            &rust_manifest,
            lock,
            cargo_target.path(),
        );
    }

    let before_api: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("before-web/generated/semaprax.api.json")).unwrap(),
    )
    .unwrap();
    let after_api: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("after-web/generated/semaprax.api.json")).unwrap(),
    )
    .unwrap();
    let before_descriptor: serde_json::Value =
        serde_json::from_str(before_api["descriptor"].as_str().unwrap()).unwrap();
    let after_descriptor: serde_json::Value =
        serde_json::from_str(after_api["descriptor"].as_str().unwrap()).unwrap();
    assert_eq!(before_descriptor["exports"], after_descriptor["exports"]);
    assert_ne!(
        before_descriptor["project_revision"],
        after_descriptor["project_revision"]
    );
    assert_ne!(
        before_descriptor["workspace_revision"],
        after_descriptor["workspace_revision"]
    );
    subject_binding::verify_display_rename(&bound_subjects[0], &bound_subjects[1]);

    fs::remove_dir_all(root).unwrap();
}
