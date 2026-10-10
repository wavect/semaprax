//! Additive v31 Catalog source: unchanged 23-case functional oracle, not v30 qualification.
use super::public_example::{compile_native, execute};
use super::*;
use std::collections::BTreeSet;
use std::path::Path;

const EXAMPLE: &str = "examples/catalog-scoped-record-successor";
const SPEC: &[u8] = include_bytes!("../../../../../benchmarks/catalog-tokens-v1/SPEC.md");
const CORPUS: &str =
    include_str!("../../../../../benchmarks/catalog-tokens-v1/acceptance/corpus.json");
const SOURCES: &[(&str, &str)] = &[
    (
        "src/app.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/app.spx"),
    ),
    (
        "src/app.command.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/app.command.spx"),
    ),
    (
        "src/request.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/request.spx"),
    ),
    (
        "src/response.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/response.spx"),
    ),
    (
        "src/model.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/model.spx"),
    ),
    (
        "src/order.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/order.spx"),
    ),
    (
        "src/restock.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/restock.spx"),
    ),
    (
        "src/publication.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/publication.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/tests.spx"),
    ),
];

fn hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}

fn stage(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    std::fs::copy(original.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    for (path, source) in SOURCES {
        let canonical_source = canonical(source);
        assert_eq!(canonical(&canonical_source), canonical_source, "{path}");
        std::fs::write(root.join(path), canonical_source).unwrap();
    }
    // Derive/replay both actual generators from the unchanged bootstrap.
    // No guessed body, source-origin exemption, or half-installed import is used.
    let mut candidates = Vec::new();
    for (path, identity, profile) in [
        (
            "src/request.spx",
            "catalog.request",
            project::JsonCodecProfile::StreamUtf8OwnedRequest {
                max_string_bytes: 16,
            },
        ),
        (
            "src/response.spx",
            "catalog.report",
            project::JsonCodecProfile::CollectionResponse {
                max_string_bytes: 16,
            },
        ),
    ] {
        let generated =
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                project::derive_json_codec_source_with_profile(
                    &snapshot.retain_revision(),
                    path,
                    identity,
                    profile,
                )
            })
            .unwrap();
        assert_eq!(canonical(&generated), generated);
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            project::verify_json_codec_source_with_profile(
                &snapshot.retain_revision(),
                path,
                identity,
                &generated,
                profile,
            )
        })
        .unwrap();
        candidates.push((path, generated));
    }
    for (path, generated) in candidates {
        std::fs::write(root.join(path), generated).unwrap();
    }
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();
}

#[test]
fn typed_catalog_derives_codecs_and_preserves_all_23_functional_cases() {
    let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(corpus["schema"], "catalog.frozen-manual-corpus.v1");
    let digest = format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(SPEC)));
    assert_eq!(corpus["spec_sha256"], digest);
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 23);
    assert_eq!(
        cases
            .iter()
            .map(|case| case["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        23
    );
    let root = crate::standard_library::temporary("catalog-scoped-record-successor");
    stage(&root);
    let native = root.join("catalog-native");
    let c_source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        // Explicit successor route; this assertion must never be relabeled v30.
        assert_eq!(snapshot.manifest().schema(), project::PROJECT_SCHEMA_V31);
        assert_eq!(
            snapshot.manifest().project_profile(),
            project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        );
        let graph = snapshot.semantic_graph();
        super::super::collection_response::assert_scoped_row_reads(
            snapshot.public_api_program(),
            "catalog.output_item",
        );
        assert!(graph.contains("catalog.request.json.utf8.owned.decode"));
        assert!(graph.contains("catalog.report.json.collection-response.encode"));
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert!(tests.command_succeeded());
        assert_eq!(tests.cases().len(), 2);
        snapshot.build_native(&native)?;
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    // The independently frozen corpus carries exact raw bytes and exact all-channel
    // results; never reserialize requests, infer expected output, or skip a case.
    for optimization in ["-O0", "-O2"] {
        let binary = compile_native(&c_source, &root, optimization);
        for case in cases {
            let name = case["name"].as_str().unwrap();
            let output = execute(&binary, &hex(case["input_hex"].as_str().unwrap()));
            assert_eq!(
                output.status.code(),
                Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap()),
                "{optimization}: {name}"
            );
            assert_eq!(
                output.stdout,
                hex(case["stdout_hex"].as_str().unwrap()),
                "{optimization}: {name}"
            );
            assert_eq!(
                output.stderr,
                hex(case["stderr_hex"].as_str().unwrap()),
                "{optimization}: {name}"
            );
        }
    }
    // Also bind the ordinary Project native-build artifact to every original row.
    for case in cases {
        let output = execute(&native, &hex(case["input_hex"].as_str().unwrap()));
        assert_eq!(
            output.status.code(),
            Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap())
        );
        assert_eq!(output.stdout, hex(case["stdout_hex"].as_str().unwrap()));
        assert_eq!(output.stderr, hex(case["stderr_hex"].as_str().unwrap()));
    }
    std::fs::remove_dir_all(root).unwrap();
}
