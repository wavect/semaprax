use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

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
const SELECTED: [&str; 3] = [
    "frame.payload",
    "frame.payload-maybe",
    "frame.payload-result",
];

fn temporary(label: &str) -> PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
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
    let checked = semaprax::check(source, Path::new("frame.spx")).unwrap();
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
    for (name, frame, valid, error) in corpus_frames() {
        let expected = valid.then(|| frame[8..].to_vec());
        let maybe = evaluate_resolved_owned_data(
            &program,
            "frame.payload-maybe",
            &frame,
            DEFAULT_MAX_STEPS,
        )
        .unwrap();
        assert_eq!(maybe.function_id.as_str(), "frame.payload-maybe", "{name}");
        assert_eq!(
            maybe.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::OptionBytes(expected.clone())),
            "{name}"
        );
        assert_eq!(
            maybe.cleanup_events,
            if valid {
                vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            },
            "{name}"
        );

        let result = evaluate_resolved_owned_data(
            &program,
            "frame.payload-result",
            &frame,
            DEFAULT_MAX_STEPS,
        )
        .unwrap();
        assert_eq!(
            result.function_id.as_str(),
            "frame.payload-result",
            "{name}"
        );
        let expected_result = match expected.clone() {
            Some(payload) => Ok(payload),
            None => Err(error),
        };
        assert_eq!(
            result.outcome,
            OwnedDataEvaluationOutcome::Returned(OwnedDataValue::ResultBytesI64(expected_result)),
            "{name}"
        );
        assert_eq!(
            result.cleanup_events,
            if valid {
                vec![OwnedDataCleanupEvent::CopyOutAndSettleBytes]
            } else {
                Vec::new()
            },
            "{name}"
        );

        if let Some(expected) = expected {
            let direct =
                evaluate_resolved_owned_data(&program, "frame.payload", &frame, DEFAULT_MAX_STEPS)
                    .unwrap();
            assert_eq!(direct.function_id.as_str(), "frame.payload", "{name}");
            assert_eq!(
                direct.outcome,
                OwnedDataEvaluationOutcome::Returned(OwnedDataValue::Bytes(expected)),
                "{name}"
            );
            assert_eq!(
                direct.cleanup_events,
                [OwnedDataCleanupEvent::CopyOutAndSettleBytes],
                "{name}"
            );
        }
    }
}

#[test]
fn native_owned_data_provider_runs_the_exact_corpus_at_o0_and_o2() {
    use std::fmt::Write as _;

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
    for symbol in [
        "spx_owned_data_call_spx_frame_dot_payload_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1",
        "spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1",
    ] {
        assert!(artifact.source().contains(symbol));
    }

    let frames = corpus_frames();
    let valid_count = frames.iter().filter(|row| row.2).count();
    let mut declarations = String::new();
    let mut cases = String::new();
    for (index, (name, frame, valid, error)) in frames.iter().enumerate() {
        writeln!(
            declarations,
            "static const uint8_t case_{index}[]={{ {} }};",
            c_bytes(frame)
        )
        .unwrap();
        let pointer = if frame.is_empty() {
            "NULL".to_owned()
        } else {
            format!("case_{index}")
        };
        writeln!(
            cases,
            "if(run_case(context,{pointer},UINT64_C({}),{},INT64_C({}))!=0)return {}; /* {} */",
            frame.len(),
            u8::from(*valid),
            error,
            40 + index,
            name
        )
        .unwrap();
    }
    let probe = format!(
        r#"
{declarations}
static uint32_t drops=UINT32_C(0);
static int copy_drop(spx_context_v1 *context,uint64_t handle,const uint8_t *expected,uint64_t length){{
    uint64_t actual=UINT64_MAX;static uint8_t output[UINT64_C(65528)];
    if(handle==UINT64_C(0)||spx_owned_bytes_len_v1(context,handle,&actual)!=0||actual!=length)return 1;
    if(spx_owned_bytes_copy_v1(context,handle,length==0?NULL:output,length)!=0)return 2;
    if(length!=0&&memcmp(output,expected,(size_t)length)!=0)return 3;
    if(spx_owned_bytes_drop_v1(context,handle)!=0)return 4;
    ++drops;return 0;
}}
static int run_case(spx_context_v1 *context,const uint8_t *frame,uint64_t length,uint8_t valid,int64_t expected_error){{
    uint32_t tag=UINT32_C(99);uint64_t handle=UINT64_C(0);int64_t error=INT64_C(99);
    uint64_t payload_length=valid?length-UINT64_C(8):UINT64_C(0);
    const uint8_t *payload=valid?frame+UINT64_C(8):NULL;
    if(spx_owned_data_call_spx_frame_dot_payload_hyphen_maybe_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 10;
    if(valid){{if(tag!=UINT32_C(1)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 11;}}
    else if(tag!=UINT32_C(0)||handle!=UINT64_C(0)||error!=INT64_C(0))return 12;
    tag=UINT32_C(99);handle=UINT64_C(0);error=INT64_C(99);
    if(spx_owned_data_call_spx_frame_dot_payload_hyphen_result_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 13;
    if(valid){{if(tag!=UINT32_C(0)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 14;}}
    else if(tag!=UINT32_C(1)||handle!=UINT64_C(0)||error!=expected_error)return 15;
    if(valid){{
        tag=UINT32_C(99);handle=UINT64_C(0);error=INT64_C(99);
        if(spx_owned_data_call_spx_frame_dot_payload_v1(context,frame,length,&tag,&handle,&error)!=SPX_OWNED_DATA_SUCCESS)return 16;
        if(tag!=UINT32_C(0)||error!=INT64_C(0)||copy_drop(context,handle,payload,payload_length)!=0)return 17;
    }}
    return context->live_slots==UINT32_C(0)?0:18;
}}
int main(void){{
    uint64_t size=spx_owned_data_context_size_v1();void *storage=malloc((size_t)size);
    if(storage==NULL)return 20;
    if(spx_owned_data_context_init_v1(storage,size)!=SPX_OWNED_DATA_SUCCESS)return 21;
    spx_context_v1 *context=(spx_context_v1*)storage;
    {cases}
    if(drops!=UINT32_C({}))return 22;
    if(context->live_slots!=UINT32_C(0))return 23;
    if(spx_owned_data_context_drop_v1(context)!=SPX_OWNED_DATA_SUCCESS)return 24;
    free(storage);return 0;
}}
"#,
        valid_count * 3
    );

    let root = temporary("native-provider");
    fs::create_dir_all(&root).unwrap();
    for optimization in ["-O0", "-O2"] {
        let source = root.join(format!("provider-{optimization}.c"));
        let executable = root.join(format!("provider-{optimization}"));
        fs::write(&source, format!("{}\n{probe}", artifact.source())).unwrap();
        let compile = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compile.status.success(),
            "{optimization} compile stderr={}",
            String::from_utf8_lossy(&compile.stderr)
        );
        let execute = Command::new(&executable).output().unwrap();
        assert!(
            execute.status.success(),
            "{optimization} status={:?} stdout={} stderr={}",
            execute.status.code(),
            String::from_utf8_lossy(&execute.stdout),
            String::from_utf8_lossy(&execute.stderr)
        );
    }
    fs::remove_dir_all(root).unwrap();
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
    let result = Command::new(binary)
        .args(["build", "--manifest-path"])
        .arg(manifest)
        .args(["--target", target, "-o"])
        .arg(output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "target={target} stdout={} stderr={}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn project_v8_npm_and_rust_routes_run_the_same_corpus_before_and_after_display_rename() {
    let root = temporary("project-v8-routes");
    let before = root.join("before-project");
    let after = root.join("after-project");
    copy_project(&before, false);
    copy_project(&after, true);
    let binary = Path::new(env!("CARGO_BIN_EXE_semaprax"));

    for (label, project) in [("before", &before), ("after", &after)] {
        let npm_consumer = root.join(format!("{label}-web"));
        fs::create_dir_all(&npm_consumer).unwrap();
        fs::write(
            npm_consumer.join("consumer.mjs"),
            include_bytes!("../examples/frame-payload-web/consumer.mjs"),
        )
        .unwrap();
        fs::write(npm_consumer.join("corpus.json"), CORPUS).unwrap();
        build(
            binary,
            &project.join("semaprax.toml"),
            "npm",
            &npm_consumer.join("generated"),
        );
        run_node_consumer(&npm_consumer);

        let rust_consumer = root.join(format!("{label}-rust"));
        fs::create_dir_all(rust_consumer.join("src")).unwrap();
        fs::write(
            rust_consumer.join("Cargo.toml"),
            include_bytes!("../examples/frame-payload-rust/Cargo.toml"),
        )
        .unwrap();
        fs::write(
            rust_consumer.join("src/main.rs"),
            include_bytes!("../examples/frame-payload-rust/src/main.rs"),
        )
        .unwrap();
        fs::write(rust_consumer.join("corpus.json"), CORPUS).unwrap();
        build(
            binary,
            &project.join("semaprax.toml"),
            "rust",
            &rust_consumer.join("generated-sdk"),
        );
        let result = Command::new("cargo")
            .args(["run", "--quiet", "--offline", "--manifest-path"])
            .arg(rust_consumer.join("Cargo.toml"))
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
    }

    let before_api: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("before-web/generated/semaprax.api.json")).unwrap(),
    )
    .unwrap();
    let after_api: serde_json::Value = serde_json::from_slice(
        &fs::read(root.join("after-web/generated/semaprax.api.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(before_api["exports"], after_api["exports"]);
    assert_ne!(
        before_api["project_revision"],
        after_api["project_revision"]
    );
    assert_ne!(
        before_api["workspace_revision"],
        after_api["workspace_revision"]
    );

    fs::remove_dir_all(root).unwrap();
}
