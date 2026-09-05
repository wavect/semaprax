//! The benchmark project fixture must be a real, admissible project.
//!
//! `benches/project.rs` and `benches/interpreter.rs` measure Project check,
//! test, run, frontend reanalysis and prepared evaluation over a generated
//! multi-module fixture. A benchmark that silently measured a rejected project,
//! or an "edit" that invalidated everything, would report meaningless numbers
//! and nobody would notice: Criterion timing does not run in a pull request.
//! These cases pin fixture validity and the invalidation shape cheaply, without
//! running a timing campaign.

#[path = "../../benches/support/project_fixture.rs"]
mod project_fixture;

use semaprax::project::{
    self, PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectExecutionOptions, ProjectFrontendCache,
    ProjectFrontendSource, ProjectManifest,
};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static SERIAL: AtomicUsize = AtomicUsize::new(0);

fn scratch() -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "{}{}-{}",
        "spx-benchmark-fixture-",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    // Project loading rejects a symlinked ancestor, and the platform temporary
    // directory is one on macOS.
    std::fs::canonicalize(&path).unwrap()
}

fn frontend_sources(sources: &[(String, String)]) -> Vec<ProjectFrontendSource> {
    sources
        .iter()
        .map(|(path, source)| ProjectFrontendSource::new(path, source).unwrap())
        .collect()
}

fn build(
    cache: &mut ProjectFrontendCache,
    manifest: &ProjectManifest,
    sources: &[ProjectFrontendSource],
) -> Value {
    let built = cache.build(manifest, sources).unwrap();
    serde_json::from_str(built.to_json()).unwrap()
}

#[test]
fn every_generated_scale_is_an_admissible_project() {
    for scale in project_fixture::SCALES {
        let fixture = project_fixture::generate(scale);
        assert_eq!(fixture.sources.len(), fixture.leaves() + 3);
        let directory = scratch();
        let manifest = fixture.write_to(&directory);

        project::with_authenticated_project(&manifest, |snapshot| snapshot.check())
            .unwrap_or_else(|diagnostics| panic!("scale {scale} must check: {diagnostics:?}"));
        let entry = project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.execute_entry(&ProjectExecutionOptions::default())
        })
        .unwrap();
        assert!(
            entry.command_succeeded(),
            "scale {scale} entry must succeed"
        );
        let tests = project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.execute_test(&ProjectExecutionOptions::default())
        })
        .unwrap();
        assert!(tests.command_succeeded(), "scale {scale} tests must pass");
        std::fs::remove_dir_all(&directory).unwrap();
    }
}

#[test]
fn the_scalar_loop_fixture_is_an_admissible_project_and_a_standalone_module() {
    let fixture = project_fixture::scalar_loop_project(64);
    let directory = scratch();
    let manifest = fixture.write_to(&directory);
    let execution = project::with_authenticated_project(&manifest, |snapshot| {
        snapshot.execute_entry(&ProjectExecutionOptions::default())
    })
    .unwrap();
    assert!(execution.command_succeeded());
    assert!(execution.steps_used() > 0, "the loop must evaluate steps");

    // The prepared-evaluator benchmark measures this seam: closures resolved
    // once, then executed repeatedly without re-reading or re-verifying source.
    let revision =
        project::with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let options = PreparedProjectExecutionOptions::default();
    let cancellation = ProjectExecutionCancellation::new();
    let first = prepared.execute_entry(&options, &cancellation).unwrap();
    let second = prepared.execute_entry(&options, &cancellation).unwrap();
    assert_eq!(first.steps_used(), second.steps_used());
    assert_eq!(first.outcome(), second.outcome());

    // The same source is interpreted directly by the cold end-to-end benchmark.
    let standalone = directory.join("standalone.spx");
    std::fs::write(&standalone, project_fixture::scalar_loop_source(64)).unwrap();
    let options = semaprax::interpreter::InterpreterOptions::new(65_536, 1_000_000).unwrap();
    semaprax::interpreter::interpret(&standalone, "bench.scalar.main", &[], &options).unwrap();
    std::fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn a_controlled_edit_invalidates_exactly_its_consumers() {
    let fixture = project_fixture::generate(2);
    let manifest = ProjectManifest::parse(&fixture.manifest).unwrap();
    let original = frontend_sources(&fixture.sources);
    let mut cache = ProjectFrontendCache::new();

    let cold = build(&mut cache, &manifest, &original);
    assert_eq!(
        cold["work"]["modules_parsed"].as_u64().unwrap(),
        fixture.sources.len() as u64,
        "a cold build parses every module"
    );

    let unchanged = build(&mut cache, &manifest, &original);
    assert_eq!(
        unchanged["work"]["modules_parsed"].as_u64().unwrap(),
        0,
        "an unchanged rebuild reuses every module"
    );
    assert!(unchanged["invalidated_sources"]
        .as_array()
        .unwrap()
        .is_empty());

    let leaf = build(
        &mut cache,
        &manifest,
        &frontend_sources(&fixture.with_edited_leaf()),
    );
    let invalidated = leaf["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert!(invalidated.contains(&"src/leaf_0.spx".to_owned()));
    assert!(
        !invalidated.contains(&"src/core.spx".to_owned()),
        "editing a leaf must not invalidate its provider: {invalidated:?}"
    );
    assert!(
        !invalidated.contains(&"src/leaf_1.spx".to_owned()),
        "editing one leaf must not invalidate a sibling: {invalidated:?}"
    );
    assert!(
        invalidated.len() < fixture.sources.len(),
        "a leaf edit must reanalyse less than the whole project: {invalidated:?}"
    );

    // Return to the original bytes, then change the provider every module uses.
    build(&mut cache, &manifest, &original);
    let core = build(
        &mut cache,
        &manifest,
        &frontend_sources(&fixture.with_edited_core()),
    );
    let invalidated = core["invalidated_sources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        invalidated.len(),
        fixture.sources.len(),
        "a provider edit reaches every consumer: {invalidated:?}"
    );
}
