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
    let entries = [
        project::JsonCodecPlanEntry {
            source_path: "src/request.spx".to_owned(),
            record_id: "catalog.request".to_owned(),
            profile: project::JsonCodecProfile::StreamUtf8OwnedRequest {
                max_string_bytes: 16,
            },
        },
        project::JsonCodecPlanEntry {
            source_path: "src/response.spx".to_owned(),
            record_id: "catalog.report".to_owned(),
            profile: project::JsonCodecProfile::CollectionResponse {
                max_string_bytes: 16,
            },
        },
    ];
    let bundle = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let bundle = project::derive_json_codec_plan(&revision, &entries)?;
        project::verify_json_codec_plan(&revision, &entries, &bundle)?;
        assert_eq!(bundle.entries.len(), 2);
        assert_eq!(
            bundle.authored_sources.len(),
            bundle.candidate_sources.len()
        );
        Ok(bundle)
    })
    .unwrap();
    for entry in &bundle.entries {
        let generated = bundle
            .candidate_sources
            .iter()
            .find(|source| source.path == entry.source_path)
            .unwrap();
        assert_eq!(canonical(&generated.source), generated.source);
    }
    // Only the complete, independently replayed candidate becomes staged input.
    // The checked candidate carries the manifest's bundled standard dependency
    // sources and their transitive closure too (`candidate.sources`, since the
    // paired-bundle derivation).
    // They are resolved from the toolchain, not staged project files: each must
    // be byte-identical to its authored counterpart, and the set is exact.
    let mut bundled = BTreeSet::new();
    for source in &bundle.candidate_sources {
        if source.path.starts_with("dependencies/") {
            let authored = bundle
                .authored_sources
                .iter()
                .find(|authored| authored.path == source.path)
                .unwrap();
            assert_eq!(authored.source, source.source, "{}", source.path);
            assert_eq!(authored.sha256, source.sha256, "{}", source.path);
            assert!(!root.join(&source.path).exists(), "{}", source.path);
            bundled.insert(source.path.as_str());
        } else {
            std::fs::write(root.join(&source.path), &source.source).unwrap();
        }
    }
    assert_eq!(
        bundled,
        BTreeSet::from([
            "dependencies/std.data.json.digits/0.1.0/digits.spx",
            "dependencies/std.data.json.doc/0.1.0/doc.spx",
            "dependencies/std.data.json.query/0.1.0/query.spx",
            "dependencies/std.data.json.scan/0.1.0/scan.spx",
            "dependencies/std.data.json.token/0.1.0/token.spx",
            "dependencies/std.data.json.utf8/0.1.0/utf8.spx",
            "dependencies/std.data.json.write/0.1.0/write.spx",
            "dependencies/std.data.json/0.1.0/json.spx",
            "dependencies/std.io/0.1.0/io.spx",
            "dependencies/std.text/0.1.0/text.spx",
        ])
    );
    // The original ordinary Input→Decode tail now composes. Preserve its
    // independent projected-owner refusal against the same generated schemas.
    let original = include_str!("catalog-scoped-record-command-original.txt");
    std::fs::write(root.join("src/app.spx"), canonical(original)).unwrap();
    let errors =
        project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    {
        let code = "SPX-O118";
        assert!(
            errors.iter().any(|error| error.code == code),
            "{code}: {errors:?}"
        );
    }
    assert!(
        !errors
            .iter()
            .any(|error| matches!(error.code, "SPX-T258" | "SPX-T216")),
        "{errors:?}"
    );
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
        assert!(graph.contains("core.vec.str-at"));
        assert!(!graph.contains("\"catalog.tag\""));
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
    let adopted_order = std::fs::read_to_string(root.join("src/order.spx")).unwrap();
    // Isolate only byte-for adoption; retain the current row traversal and
    // substitute the literal pre-adoption helper from the independent archive.
    let frozen_order = include_str!("../../../../../docs/evidence/application-traversal-plan-adoption/order_before_traversal.txt");
    let marker = "@id(\"catalog.identifier\")";
    let next = "@id(\"catalog.valid_sorted_tags\")";
    let old_helper =
        &frozen_order[frozen_order.find(marker).unwrap()..frozen_order.find(next).unwrap()];
    let begin = adopted_order.find(marker).unwrap();
    let end = adopted_order.find(next).unwrap();
    let helper_only_order = format!(
        "{}{}{}",
        &adopted_order[..begin],
        old_helper,
        &adopted_order[end..]
    );
    std::fs::write(root.join("src/order.spx"), helper_only_order).unwrap();
    let helper_only_c =
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            codegen::emit_hir_c_with_stdin_stream_collection_records(
                snapshot.public_api_program(),
                "catalog.command",
            )
            .map_err(|error| vec![error])
        })
        .unwrap();
    std::fs::write(root.join("src/order.spx"), include_str!("../../../../../docs/evidence/application-traversal-plan-adoption/order_before_traversal.txt")).unwrap();
    let indexed_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    std::fs::write(root.join("src/order.spx"), adopted_order).unwrap();
    // Differential source retains the original ordinary normalized-input join,
    // while retaining the admitted owned-record command destructuring. The old
    // receive/decide workaround remains independently executed below.
    let workaround = std::fs::read_to_string(root.join("src/app.spx")).unwrap();
    let original_receive = "fn receive(bytes: own Bytes, length: usize) -> Decode\n{\n    decode(byte_range(bytes_as_slice(bytes), 0usize, length))\n}";
    assert!(workaround.contains(original_receive));
    let old_command = "let normalized = normalize();\n    match own normalized { StreamInput::Error { code: _code, offset: _offset, field: _field } => reject(), StreamInput::Ready { bytes, length } => { let outcome = receive(bytes, length); decide(outcome) }, }";
    assert!(workaround.contains(old_command));
    let joined = workaround.replace(original_receive,
        "fn receive() -> Decode uses { process.stdin.read } {let normalized=normalize();match own normalized {StreamInput::Error{code,offset,field}=>Decode::Error{code:code,offset:offset,field:field},StreamInput::Ready{bytes,length}=>decode(byte_range(bytes_as_slice(bytes),0usize,length)),}}")
        .replace(old_command,"decide(receive())");
    std::fs::write(root.join("src/app.spx"), canonical(&joined)).unwrap();
    let joined_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    std::fs::write(root.join("src/app.spx"), workaround).unwrap();
    super::string_slot_adoption::catalog(&root);
    let wrapper_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot.semantic_graph().contains("catalog.tag"));
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    // All five sources execute every frozen raw-byte/all-channel row unchanged.
    for (route, source) in [
        ("slot-views", &c_source),
        ("original-indexed-validation", &indexed_c),
        ("original-indexed-helper-only", &helper_only_c),
        ("joined", &joined_c),
        ("tag-wrapper", &wrapper_c),
    ] {
        for optimization in ["-O0", "-O2"] {
            let binary = compile_native(source, &root, optimization);
            for case in cases {
                let name = case["name"].as_str().unwrap();
                let output = execute(&binary, &hex(case["input_hex"].as_str().unwrap()));
                assert_eq!(
                    output.status.code(),
                    Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap()),
                    "{route}: {optimization}: {name}"
                );
                assert_eq!(
                    output.stdout,
                    hex(case["stdout_hex"].as_str().unwrap()),
                    "{route}: {optimization}: {name}"
                );
                assert_eq!(
                    output.stderr,
                    hex(case["stderr_hex"].as_str().unwrap()),
                    "{route}: {optimization}: {name}"
                );
            }
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

#[path = "catalog_sort_adoption.rs"]
mod catalog_sort_adoption;

#[test]
fn typed_vec_constructor_retains_exact_explicit_chain_application_control() {
    let root = crate::standard_library::temporary("catalog-vec-constructor-control");
    stage(&root);
    let original = include_str!("vec_constructor_adoption/catalog_tests_explicit_chain.txt");
    let adopted = SOURCES
        .iter()
        .find(|(path, _)| *path == "src/tests.spx")
        .unwrap()
        .1;
    assert!(!original.contains("vec_of<"));
    assert!(adopted.contains("vec_of<InputItem>"));
    super::vec_constructor_adoption::assert_application_control(&root, adopted, original);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn identifier_byte_for_keeps_string_slot_and_record_field_callers() {
    let root = crate::standard_library::temporary("catalog-byte-for-callers");
    stage(&root);
    let original_tests = std::fs::read_to_string(root.join("src/tests.spx")).unwrap();
    let adopted_order = std::fs::read_to_string(root.join("src/order.spx")).unwrap();
    let frozen = include_str!("../../../../../docs/evidence/application-traversal-plan-adoption/order_before_traversal.txt");
    let marker = "@id(\"catalog.identifier\")";
    let next = "@id(\"catalog.valid_sorted_tags\")";
    let begin = adopted_order.find(marker).unwrap();
    let end = adopted_order.find(next).unwrap();
    let old_helper = &frozen[frozen.find(marker).unwrap()..frozen.find(next).unwrap()];
    let manual = format!(
        "{}{}{}",
        &adopted_order[..begin],
        old_helper,
        &adopted_order[end..]
    );
    for (name, literal, expected) in [
        ("empty", "\"\"", false),
        ("single", "\"A\"", true),
        ("classes", "\"AZaz09_-\"", true),
        ("sixteen", "\"0123456789ABCDEF\"", true),
        ("seventeen", "\"0123456789ABCDEFG\"", false),
        ("first", "\"@ab\"", false),
        ("middle", "\"a@b\"", false),
        ("last", "\"ab@\"", false),
        ("nul", "\"a\\u{0}b\"", false),
        ("utf8", "\"é\"", false),
    ] {
        let imports = "use function @id(\"catalog.valid_sorted_tags\") from catalog.order as valid_tags;\nuse function @id(\"catalog.valid_sorted_items\") from catalog.order as valid_items;\n";
        let source = original_tests.replacen(
            "@id(\"catalog.tests.main\")",
            &format!("{imports}\n@id(\"catalog.tests.main\")"),
            1,
        );
        let probe = format!(
            r#"
@id("catalog.tests.byte_for_callers")
fn test_byte_for_callers()->i64 {{
    let tags=vec_push<string>(vec_with_capacity<string>(1usize),{literal});
    let items=vec_push<InputItem>(vec_with_capacity<InputItem>(1usize),InputItem{{id:{literal},department:1,priority:1,stock:1,topup:1,mark:1u8}});
    let slot=valid_tags(tags);
    let field=valid_items(items);
    if slot=={expected} && field=={expected} {{0}}else{{1}}
}}
"#
        );
        std::fs::write(root.join("src/tests.spx"), canonical(&(source + &probe))).unwrap();
        for (route, order) in [("byte-for", &adopted_order), ("indexed-helper", &manual)] {
            std::fs::write(root.join("src/order.spx"), order).unwrap();
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                assert_eq!(snapshot.manifest().schema(), project::PROJECT_SCHEMA_V31);
                let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
                assert!(tests.command_succeeded(), "{name}/{route}");
                assert_eq!(tests.cases().len(), 3, "{name}/{route}");
                Ok(())
            })
            .unwrap();
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
