//! Two independently selected versions of the same API execute in one SDK.

use super::*;
use crate::public_sdk::indexed_multiple::prepare_indexed_scalars;

const FIRST: &[u8] = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
const SECOND: &[u8] = b"pub fn add(left:i64,right:i64)->i64{left+right-20}\n";

fn source() -> String {
    SOURCE.replace(
        "failure status \"host.math.v1\";",
        "failure status \"host.math.v1\";\n    @id(\"host.add2\")\n    import rust selected fn second_add from \"type::add\" effects { host.math } failure status \"host.math.v1\";",
    ).replace("host_add(left, right) + right", "host_add(left, right) + second_add(left, right)")
}

fn selections_options() -> NativeRustSdkOptions {
    let mut selected = options();
    selected.imports.push("host.add2".into());
    selected
}

struct Fixture {
    index: RustApiIndex,
    source: Vec<u8>,
    alias: String,
}

impl Fixture {
    fn new(source: &[u8], rustc: &str, alias: &str, version: &str) -> Self {
        let mut envelope: Value = serde_json::from_slice(include_bytes!(
            "../../../semaprax-rust-api-index/fixtures/local-api-fixture-v2-envelope.json"
        ))
        .unwrap();
        let index = &mut envelope["index"];
        index["package"]["name"] = "fixture_math".into();
        index["package"]["version"] = version.into();
        index["package"]["source_sha256"] = raw_digest(source).into();
        index["package"]["renamed_from"] = if alias == "fixture_math" {
            Value::Null
        } else {
            alias.into()
        };
        index["target"] = target_triple().unwrap().into();
        index["stable_rustc_version"] = rustc.into();
        let mut item = index["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["path"] == "local_api_fixture::cfg_selected")
            .unwrap()
            .clone();
        item["path"] = format!("{alias}::add").into();
        item["signature"] = "fn add(left: i64, right: i64) -> i64".into();
        index["items"] = serde_json::json!([item]);
        index["types"] = serde_json::json!([]);
        let mut bytes = serde_json::to_vec(&envelope).unwrap();
        bytes.push(b'\n');
        Self {
            index: RustApiIndex::admit_extractor_output(&bytes).unwrap(),
            source: source.to_vec(),
            alias: alias.into(),
        }
    }

    fn selection<'a>(&'a self, id: &'a str) -> IndexedScalarSelection<'a> {
        IndexedScalarSelection {
            import_id: id,
            index_bytes: self.index.canonical_json().as_bytes(),
            package_source_bytes: &self.source,
            package: SelectedPackage {
                cargo_alias: &self.alias,
                name: &self.index.package().name,
                version: &self.index.package().version,
                source_sha256: &self.index.package().source_sha256,
                target: self.index.target(),
                feature_digest: self.index.feature_digest(),
                stable_rustc_version: self.index.stable_rustc_version(),
            },
        }
    }
}

#[test]
fn indexed_multiple_replays_exact_selections_and_preserves_roundtrip_identity() {
    let first = Fixture::new(FIRST, "rustc 1.98.0", "fixture_math", "0.0.1");
    let second = Fixture::new(SECOND, "rustc 1.98.0", "type", "0.0.2");
    let selections = [first.selection("host.add"), second.selection("host.add2")];
    let path = Path::new("indexed-multiple.spx");
    let source = source();
    let (program, hir, plans, sources) =
        prepare_indexed_scalars(&source, path, &selections_options(), &selections).unwrap();
    assert_eq!(plans.len(), 2);
    assert_ne!(plans[0].physical_symbol, plans[1].physical_symbol);
    assert_eq!(plans[0].package_version, "0.0.1");
    assert_eq!(plans[1].package_version, "0.0.2");
    assert_eq!(
        sources,
        [
            std::str::from_utf8(FIRST).unwrap(),
            std::str::from_utf8(SECOND).unwrap()
        ]
    );
    assert_eq!(hir.interfaces[0].imports.len(), 2);
    let canonical = semaprax::format::canonical(&program);
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert!(graph.contains(&plans[0].index_digest));
    assert!(graph.contains(&plans[1].index_digest));
    let mut reversed_options = selections_options();
    reversed_options.imports.reverse();
    let reversed = [selections[1], selections[0]];
    let (rebound, _, reordered_plans, reordered_sources) =
        prepare_indexed_scalars(&canonical, path, &reversed_options, &reversed).unwrap();
    assert_eq!(reordered_plans, plans);
    assert_eq!(reordered_sources, sources);
    assert_eq!(semaprax::graph::to_json(&rebound).unwrap(), graph);

    let missing = prepare_indexed_scalars(&source, path, &selections_options(), &selections[..1])
        .unwrap_err();
    assert!(!missing.is_empty());
    let duplicate = [selections[0], selections[0]];
    assert!(prepare_indexed_scalars(&source, path, &selections_options(), &duplicate).is_err());
    let mut changed = selections;
    changed[1].package_source_bytes = FIRST;
    let drift =
        prepare_indexed_scalars(&source, path, &selections_options(), &changed).unwrap_err();
    assert_eq!(drift[0].code, "SPX-B142");
    assert_eq!(drift[0].path.as_deref(), Some("indexed-multiple.spx"));
    assert!(drift[0].span.is_some());
    for field in ["version", "alias", "feature", "compiler", "target"] {
        let mut changed = selections;
        match field {
            "version" => changed[1].package.version = "0.0.1",
            "alias" => changed[1].package.cargo_alias = "other",
            "feature" => {
                changed[1].package.feature_digest =
                    "sha256:0000000000000000000000000000000000000000000000000000000000000000"
            }
            "compiler" => changed[1].package.stable_rustc_version = "rustc 1.97.0",
            "target" => changed[1].package.target = "x86_64-unknown-other",
            _ => unreachable!(),
        }
        let drift =
            prepare_indexed_scalars(&source, path, &selections_options(), &changed).unwrap_err();
        assert_eq!(drift[0].code, "SPX-B142", "{field}");
        assert!(drift[0].span.is_some(), "{field}");
    }
    let conflicting = Fixture::new(SECOND, "rustc 1.98.0", "fixture_math", "0.0.2");
    let collision = [selections[0], conflicting.selection("host.add2")];
    let collision_source = source.replace("type::add", "fixture_math::add");
    let drift = prepare_indexed_scalars(&collision_source, path, &selections_options(), &collision)
        .unwrap_err();
    assert_eq!(drift[0].code, "SPX-B142");
    let stale_path = source.replace("type::add", "type::missing");
    let drift =
        prepare_indexed_scalars(&stale_path, path, &selections_options(), &selections).unwrap_err();
    assert_eq!(drift[0].code, "SPX-B141");
}

#[test]
fn indexed_multiple_sdk_executes_two_versions_keyword_alias_and_negative_control() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC for physical RI-04 test");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG for physical RI-04 test");
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    assert!(actual_version.status.success());
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let first = Fixture::new(FIRST, actual_version, "fixture_math", "0.0.1");
    let second = Fixture::new(SECOND, actual_version, "type", "0.0.2");
    let selections = [first.selection("host.add"), second.selection("host.add2")];
    let source = source();
    let path = Path::new("indexed-multiple.spx");
    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-ri04-indexed-multiple-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir(&root).unwrap();
    let output = root.join("sdk");
    reset_build_observer();
    build_indexed_scalars_native_rust_sdk(
        &source,
        path,
        selections_options(),
        &selections,
        &output,
    )
    .unwrap_or_else(|error| {
        panic!(
            "indexed multiple build failed: {error:?}; stage: {:?}",
            test_build_snapshot()
        )
    });
    let lib = std::fs::read_to_string(output.join("src/lib.rs")).unwrap();
    assert!(lib.contains("mod r#type{"));
    assert!(lib.contains("=fixture_math::add"));
    assert!(lib.contains("=r#type::add"));
    let (_, _, plans, _) =
        prepare_indexed_scalars(&source, path, &selections_options(), &selections).unwrap();
    for plan in &plans {
        assert!(lib.contains(&format!("fn {}(", plan.physical_symbol)));
        assert!(lib.contains(&plan.index_digest));
    }
    let manifest = std::fs::read_to_string(output.join("semaprax.native-rust-sdk.json")).unwrap();
    assert!(manifest.contains(&raw_digest(lib.as_bytes())));
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &output), 0);

    let reversed_output = root.join("reversed");
    reset_build_observer();
    build_indexed_scalars_native_rust_sdk(
        &source,
        path,
        selections_options(),
        &[selections[1], selections[0]],
        &reversed_output,
    )
    .unwrap();
    for file in [
        "src/lib.rs",
        "native/descriptor.json",
        "semaprax.native-rust-sdk.json",
    ] {
        assert_eq!(
            std::fs::read(output.join(file)).unwrap(),
            std::fs::read(reversed_output.join(file)).unwrap(),
            "{file}"
        );
    }
    assert_eq!(
        run_published_sdk(&rustc, &clang, &root, &reversed_output),
        0
    );

    let flipped = Fixture::new(
        b"pub fn add(left:i64,right:i64)->i64{left+right-19}\n",
        actual_version,
        "type",
        "0.0.2",
    );
    let flipped_output = root.join("flipped");
    reset_build_observer();
    build_indexed_scalars_native_rust_sdk(
        &source,
        path,
        selections_options(),
        &[selections[0], flipped.selection("host.add2")],
        &flipped_output,
    )
    .unwrap();
    assert_eq!(
        run_published_sdk(&rustc, &clang, &root, &flipped_output),
        12
    );

    let wrong = Fixture::new(
        b"pub fn add(_left:i64,_right:i64)->bool{true}\n",
        actual_version,
        "type",
        "0.0.2",
    );
    let wrong_output = root.join("wrong");
    reset_build_observer();
    assert!(build_indexed_scalars_native_rust_sdk(
        &source,
        path,
        selections_options(),
        &[selections[0], wrong.selection("host.add2")],
        &wrong_output
    )
    .is_err());
    assert!(!wrong_output.exists());
    let stale_output = root.join("stale");
    let mut stale = selections;
    stale[1].package_source_bytes = FIRST;
    reset_build_observer();
    let error = build_indexed_scalars_native_rust_sdk(
        &source,
        path,
        selections_options(),
        &stale,
        &stale_output,
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-B142");
    assert!(!stale_output.exists());
    std::fs::remove_dir_all(root).unwrap();
}
