//! End-to-end source binding for the typed-record ShiftSim successor.
use super::public_example::{compile_native, execute};
use super::*;
use std::path::Path;

const EXAMPLE: &str = "examples/shiftsim-typed-record-successor";
const REQUEST_SCHEMA: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/request.spx");
const RESPONSE_SCHEMA: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/response.spx");
const APP_STUB: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/app.spx");
const APP_COMMAND: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/app.command.spx");
const MODEL: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/model.spx");
const ORDER: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/order.spx");
const SCHEDULE: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/schedule.spx");
const TESTS: &str =
    include_str!("../../../../../examples/shiftsim-typed-record-successor/src/tests.spx");
const CORPUS: &str =
    include_str!("../../../../../benchmarks/event-sim-tokens-v1/acceptance/corpus.json");
const SAMPLE_EXPECTED: &[u8] = br#"{"assignments":[{"id":"P0","server":"S0","arrival":0,"start":0,"finish":1,"wait":0,"late":false},{"id":"P1","server":"S1","arrival":0,"start":0,"finish":1,"wait":0,"late":false}],"metrics":{"patients":2,"total_wait":0,"max_wait":0,"late":0,"busy_time":2,"makespan":1,"peak_queue":2,"utilization_ppm":1000000}}"#;

fn manifest() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(EXAMPLE)
            .join("semaprax.toml"),
    )
    .unwrap()
}

fn write_example(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::create_dir_all(root.join("fixtures")).unwrap();
    std::fs::write(root.join("semaprax.toml"), manifest()).unwrap();
    for (path, source) in [
        ("src/request.spx", REQUEST_SCHEMA),
        ("src/response.spx", RESPONSE_SCHEMA),
        ("src/app.spx", APP_STUB),
        ("src/app.command.spx", APP_COMMAND),
        ("src/model.spx", MODEL),
        ("src/order.spx", ORDER),
        ("src/schedule.spx", SCHEDULE),
        ("src/tests.spx", TESTS),
    ] {
        std::fs::write(root.join(path), source).unwrap();
    }
}

fn assert_one_diagnostic_line(output: &std::process::Output) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.ends_with(b"\n"));
    let text = std::str::from_utf8(&output.stderr).expect("diagnostic is UTF-8");
    assert_eq!(text.lines().count(), 1);
    assert!(!text.trim().is_empty());
}

fn escaped_ascii(value: &str) -> String {
    let mut output = String::from("\"");
    for byte in value.bytes() {
        output.push_str(&format!("\\u{byte:04x}"));
    }
    output.push('\"');
    output
}

fn request_wire(value: &serde_json::Value, field: Option<&str>, escaped: bool) -> String {
    match value {
        serde_json::Value::Object(object) => {
            let entries = object
                .iter()
                .map(|(key, value)| {
                    let encoded_key = if escaped {
                        escaped_ascii(key)
                    } else {
                        serde_json::to_string(key).unwrap()
                    };
                    format!("{encoded_key}:{}", request_wire(value, Some(key), escaped))
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{{{entries}}}")
        }
        serde_json::Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(|value| request_wire(value, field, escaped))
                .collect::<Vec<_>>()
                .join(",")
        ),
        serde_json::Value::String(value) if escaped && matches!(field, Some("id" | "servers")) => {
            escaped_ascii(value)
        }
        _ => serde_json::to_string(value).unwrap(),
    }
}

fn expected_wire(value: &serde_json::Value) -> String {
    let assignments = value["assignments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            format!(
                "{{\"id\":{},\"server\":{},\"arrival\":{},\"start\":{},\"finish\":{},\"wait\":{},\"late\":{}}}",
                serde_json::to_string(item["id"].as_str().unwrap()).unwrap(),
                serde_json::to_string(item["server"].as_str().unwrap()).unwrap(),
                item["arrival"], item["start"], item["finish"], item["wait"], item["late"]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let metrics = &value["metrics"];
    format!(
        "{{\"assignments\":[{assignments}],\"metrics\":{{\"patients\":{},\"total_wait\":{},\"max_wait\":{},\"late\":{},\"busy_time\":{},\"makespan\":{},\"peak_queue\":{},\"utilization_ppm\":{}}}}}\n",
        metrics["patients"],
        metrics["total_wait"],
        metrics["max_wait"],
        metrics["late"],
        metrics["busy_time"],
        metrics["makespan"],
        metrics["peak_queue"],
        metrics["utilization_ppm"]
    )
}

#[test]
fn unfactored_schedule_retains_original_source_and_factored_admission() {
    let original = include_str!("shiftsim_schedule_unfactored.txt");
    let root = crate::standard_library::temporary("shiftsim-original-factored-admission");
    write_example(&root);
    super::string_slot_adoption::shiftsim(&root);
    std::fs::write(root.join("src/schedule.spx"), original).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot.semantic_graph().contains("shiftsim.schedule.run"));
        semaprax::hir::validate(snapshot.public_api_program()).map_err(|error| vec![error])
    })
    .expect("factored replay must admit the unchanged original schedule");
    assert_eq!(
        std::fs::read_to_string(root.join("src/schedule.spx")).unwrap(),
        original
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn original_copy_state_replacement_calls_keep_both_t281_refusals() {
    let root = crate::standard_library::temporary("shiftsim-copy-replace-refusal");
    write_example(&root);
    super::string_slot_adoption::shiftsim(&root);
    let mut original = super::string_slot_adoption::shiftsim_schedule().to_owned();
    for name in ["PatientState", "WorkerState"] {
        let admitted = format!("vec_set<{name}>(");
        assert_eq!(original.matches(&admitted).count(), 1);
        original = original.replace(&admitted, &format!("vec_replace<{name}>("));
    }
    std::fs::write(root.join("src/schedule.spx"), original).unwrap();
    let entered = std::cell::Cell::new(false);
    let errors = project::with_authenticated_project(&root.join("semaprax.toml"), |_| {
        entered.set(true);
        Ok(())
    })
    .expect_err("original Copy-state vec_replace calls must remain closed");
    assert!(
        !entered.get(),
        "invalid Copy operations cannot authenticate a snapshot"
    );
    assert_eq!(errors.len(), 2, "{errors:?}");
    assert!(
        errors.iter().all(|error| error.code == "SPX-T281"),
        "{errors:?}"
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_record_shiftsim_derives_both_codecs_and_runs_retained_corpus() {
    let root = crate::standard_library::temporary("shiftsim-typed-record-successor");
    write_example(&root);

    let entries = [
        project::JsonCodecPlanEntry {
            source_path: "src/request.spx".to_owned(),
            record_id: "shiftsim.request".to_owned(),
            profile: project::JsonCodecProfile::StreamOwnedRequest,
        },
        project::JsonCodecPlanEntry {
            source_path: "src/response.spx".to_owned(),
            record_id: "shiftsim.report".to_owned(),
            profile: project::JsonCodecProfile::CollectionResponse {
                max_string_bytes: 16,
            },
        },
    ];
    let bundle = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let bundle = project::derive_json_codec_plan(&revision, &entries)?;
        project::verify_json_codec_plan(&revision, &entries, &bundle)?;
        Ok(bundle)
    })
    .unwrap();
    for entry in &bundle.entries {
        let source = bundle
            .candidate_sources
            .iter()
            .find(|s| s.path == entry.source_path)
            .unwrap();
        std::fs::write(root.join(&source.path), &source.source).unwrap();
    }
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();

    let native = root.join("shiftsim-native");
    let c_source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(snapshot.manifest().schema(), project::PROJECT_SCHEMA_V31);
        // The Project semantic graph carries cross-module edges and declared
        // types only; builtin call identities live in the checked HIR.
        super::assert_calls_string_slot_view(snapshot.public_api_program());
        assert!(!snapshot.semantic_graph().contains("\"shiftsim.server\""));
        assert_eq!(
            snapshot.manifest().project_profile(),
            project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        );
        assert!(snapshot
            .semantic_graph()
            .contains("shiftsim.request.json.owned.decode"));
        assert!(snapshot
            .semantic_graph()
            .contains("shiftsim.report.json.collection-response.encode"));
        assert!(snapshot
            .execute_entry(&project::ProjectExecutionOptions::default())?
            .command_succeeded());
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert!(tests.command_succeeded());
        assert_eq!(tests.cases().len(), 2);
        snapshot.build_native(&native)?;
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "shiftsim.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();

    // Complete pre-traversal typed source remains an independent full-corpus arm.
    std::fs::write(root.join("src/schedule.spx"), include_str!("../../../../../docs/evidence/application-traversal-plan-adoption/schedule_before_traversal.txt")).unwrap();
    let indexed_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "shiftsim.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    std::fs::write(root.join("src/schedule.spx"), SCHEDULE).unwrap();

    let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(
        corpus["valid"].as_array().unwrap().len() + corpus["invalid"].as_array().unwrap().len(),
        15
    );
    super::string_slot_adoption::shiftsim(&root);
    let wrapper_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot.semantic_graph().contains("shiftsim.server"));
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "shiftsim.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    // The original independent branch expressions remain an executable
    // differential, in addition to the separately retained wrapper adoption.
    let original = include_str!("shiftsim_schedule_unfactored.txt");
    std::fs::write(root.join("src/schedule.spx"), original).unwrap();
    let original_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "shiftsim.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("src/schedule.spx")).unwrap(),
        original
    );
    for source in [&c_source, &indexed_c, &wrapper_c, &original_c] {
        for optimization in ["-O0", "-O2"] {
            let binary = compile_native(source, &root, optimization);
            for case in corpus["valid"].as_array().unwrap() {
                let name = case["name"].as_str().unwrap();
                let escaped = case["request_encoding"] == "unicode-escaped-keys-and-identifiers-v1";
                let mut input =
                    vec![b' '; case["leading_whitespace_bytes"].as_u64().unwrap_or(0) as usize];
                input.extend_from_slice(request_wire(&case["input"], None, escaped).as_bytes());
                let output = execute(&binary, &input);
                assert_eq!(
                    output.status.code(),
                    Some(0),
                    "{name}: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    output.stdout,
                    expected_wire(&case["expected"]).as_bytes(),
                    "{name}"
                );
                assert!(output.stderr.is_empty(), "{name}");
            }
            for identifier in ["", "ABCDEFGHIJKLMNOPQ", "é", "has space", "NUL\0"] {
                for slot in ["server", "patient"] {
                    let mut request = serde_json::json!({
                        "servers": ["S0"],
                        "patients": [{"id":"P0","arrival":0,"service":1,"priority":0,"deadline":1}]
                    });
                    if slot == "server" {
                        request["servers"][0] = serde_json::json!(identifier);
                    } else {
                        request["patients"][0]["id"] = serde_json::json!(identifier);
                    }
                    let output = execute(&binary, request_wire(&request, None, false).as_bytes());
                    assert_one_diagnostic_line(&output);
                }
            }
            let too_many_servers = serde_json::json!({
                "servers": (0..9).map(|i| format!("S{i}")).collect::<Vec<_>>(),
                "patients": []
            });
            let too_many_patients = serde_json::json!({
                "servers": ["S0"],
                "patients": (0..257).map(|i| serde_json::json!({
                    "id": format!("P{i}"),"arrival":0,"service":1,"priority":0,"deadline":1
                })).collect::<Vec<_>>()
            });
            for request in [too_many_servers, too_many_patients] {
                let output = execute(&binary, request_wire(&request, None, false).as_bytes());
                assert_one_diagnostic_line(&output);
            }
            for case in corpus["invalid"].as_array().unwrap() {
                let name = case["name"].as_str().unwrap();
                let input = request_wire(&case["input"], None, false);
                let output = execute(&binary, input.as_bytes());
                assert_eq!(output.status.code(), Some(2), "{name}");
                assert_one_diagnostic_line(&output);
            }
        }
    }

    let sample = std::fs::read(root.join("fixtures/request.json")).unwrap();
    let output = execute(&native, &sample);
    assert_eq!(output.status.code(), Some(0));
    let mut sample_expected = SAMPLE_EXPECTED.to_vec();
    sample_expected.push(b'\n');
    assert_eq!(output.stdout, sample_expected);
    assert!(output.stderr.is_empty());
    let malformed = std::fs::read(root.join("fixtures/malformed.json")).unwrap();
    let output = execute(&native, &malformed);
    assert_one_diagnostic_line(&output);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn typed_vec_constructor_retains_exact_explicit_chain_application_control() {
    let root = crate::standard_library::temporary("shiftsim-vec-constructor-control");
    write_example(&root);
    let original = include_str!("vec_constructor_adoption/shiftsim_tests_explicit_chain.txt");
    let adopted = TESTS;
    assert!(!original.contains("vec_of<"));
    assert!(adopted.contains("vec_of<string>"));
    super::vec_constructor_adoption::assert_application_control(&root, adopted, original);
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "shiftsim_id_validation.rs"]
mod shiftsim_id_validation;
