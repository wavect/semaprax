//! Held Project sources bind selected imports before linking and publication.

use super::*;
use crate::public_sdk::indexed_project::prepare_project_bindings;
use semaprax::project::{ProjectFrontendCache, ProjectFrontendSource, ProjectManifest};

const MANIFEST: &str = "schema = \"semaprax.project.v1\"\nname = \"indexed\"\nentry = \"interop.fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"interop.add\"]\ntests = [\"interop.tests\"]\n";

fn canonical(source: &str, path: &str) -> String {
    semaprax::format::canonical(&semaprax::parse(source, Path::new(path)).unwrap())
}

#[test]
fn indexed_project_rebinds_graph_and_executes_authenticated_package() {
    let rustc = std::env::var("RUSTC").expect("configure absolute RUSTC");
    let clang = std::env::var("CLANG").expect("configure absolute CLANG");
    let actual_version = Command::new(&rustc).arg("--version").output().unwrap();
    let actual_version = std::str::from_utf8(&actual_version.stdout).unwrap().trim();
    let crate_source = b"pub fn add(left:i64,right:i64)->i64{left+right}\n";
    let index = index_for(crate_source, actual_version);
    let replay = RustApiIndex::replay(&index).unwrap();
    let package_digest = raw_digest(crate_source);
    let source = canonical(SOURCE, "src/app.spx");
    let tests = canonical(
        "module interop.tests; @id(\"interop.tests.main\") fn main() -> i64 { 0 }",
        "src/tests.spx",
    );
    let selected = IndexedProjectScalarSelection {
        source_path: "src/app.spx",
        source: &source,
        selection: IndexedScalarSelection {
            import_id: "host.add",
            index_bytes: &index,
            package_source_bytes: crate_source,
            package: SelectedPackage {
                cargo_alias: "fixture_math",
                name: "fixture_math",
                version: "0.0.1",
                source_sha256: &package_digest,
                target: target_triple().unwrap(),
                feature_digest: replay.feature_digest(),
                stable_rustc_version: actual_version,
            },
        },
    };
    let bindings = prepare_project_bindings(&[selected]).unwrap();
    let sources = [
        ProjectFrontendSource::new("src/app.spx", &source).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    let manifest = ProjectManifest::parse(MANIFEST).unwrap();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    assert!(
        cache.build(&manifest, &sources).is_err(),
        "ordinary Project loading must not infer index authority"
    );
    let first = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .unwrap();
    let graph: Value = serde_json::from_str(first.revision().semantic_graph()).unwrap();
    assert_eq!(graph["schema"], "semaprax.project-semantic-graph.v5");
    let imports = &graph["indexed_rust_imports"]["imports"];
    assert_eq!(imports[0]["id"], "host.add");
    assert_eq!(imports[0]["rust_path"], "fixture_math::add");
    assert_eq!(imports[0]["selected_index_digest"], replay.digest());
    assert_eq!(imports[0]["effects"], serde_json::json!(["host.math"]));
    assert_eq!(imports[0]["failure_domain"], "host.math.v1");
    let first_graph = first.revision().semantic_graph().to_owned();
    let repeated = cache
        .build_indexed_rust(&manifest, &sources, &bindings)
        .unwrap();
    assert_eq!(repeated.revision().semantic_graph(), first_graph);
    let mut drifted = bindings.clone();
    drifted[0].index_digest =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    let rebound = cache
        .build_indexed_rust(&manifest, &sources, &drifted)
        .unwrap();
    let work: Value = serde_json::from_str(rebound.to_json()).unwrap();
    assert_eq!(work["manifest_context_reset"], true);
    assert_ne!(rebound.revision().semantic_graph(), first_graph);
    drifted[0].signature = "fn add(left: bool, right: i64) -> i64".into();
    assert!(cache
        .build_indexed_rust(&manifest, &sources, &drifted)
        .is_err());
    let changed_sources = [
        ProjectFrontendSource::new("src/app.spx", &(source.clone() + "\n")).unwrap(),
        ProjectFrontendSource::new("src/tests.spx", &tests).unwrap(),
    ];
    assert_eq!(
        cache
            .build_indexed_rust(&manifest, &changed_sources, &bindings)
            .err()
            .unwrap()[0]
            .code,
        "SPX-B142"
    );

    let root = std::fs::canonicalize(std::env::temp_dir())
        .unwrap()
        .join(format!(
            "semaprax-ri04-indexed-project-{}",
            std::process::id()
        ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(root.join("src/app.spx"), &source).unwrap();
    std::fs::write(root.join("src/tests.spx"), &tests).unwrap();
    let output = root.join("sdk");
    reset_build_observer();
    let bundle =
        build_indexed_project_native_rust_sdk(&root.join("semaprax.toml"), &[selected], &output)
            .unwrap_or_else(|error| {
                panic!(
                    "indexed Project failed: {error:?}, stage {:?}",
                    test_build_snapshot()
                )
            });
    assert_eq!(bundle.output_directory(), output);
    assert_eq!(run_published_sdk(&rustc, &clang, &root, &output), 0);
    assert_eq!(
        std::fs::read_to_string(root.join("src/app.spx")).unwrap(),
        source
    );

    // An independently rebuilt package with changed behavior must fail the
    // exact same native consumer assertion, not succeed on graph metadata.
    let flipped = b"pub fn add(left:i64,right:i64)->i64{left+right+1}\n";
    let flipped_index = index_for(flipped, actual_version);
    let flipped_digest = raw_digest(flipped);
    let flipped_selection = IndexedProjectScalarSelection {
        selection: IndexedScalarSelection {
            index_bytes: &flipped_index,
            package_source_bytes: flipped,
            package: SelectedPackage {
                source_sha256: &flipped_digest,
                ..selected.selection.package
            },
            ..selected.selection
        },
        ..selected
    };
    let flipped_output = root.join("flipped");
    reset_build_observer();
    build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[flipped_selection],
        &flipped_output,
    )
    .unwrap();
    assert_eq!(
        run_published_sdk(&rustc, &clang, &root, &flipped_output),
        12
    );
    std::fs::write(
        root.join("src/app.spx"),
        source.replace("+ right", "+ left"),
    )
    .unwrap();
    let refused_output = root.join("stale");
    let errors = build_indexed_project_native_rust_sdk(
        &root.join("semaprax.toml"),
        &[selected],
        &refused_output,
    )
    .unwrap_err();
    assert_eq!(errors[0].code, "SPX-B142");
    assert!(!refused_output.exists());
    std::fs::remove_dir_all(root).unwrap();
}
