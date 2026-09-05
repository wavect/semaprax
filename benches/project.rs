//! Project operations, measured as the operations actually are.
//!
//! Three distinct costs are separated, because a regression in one is invisible
//! in the others:
//!
//! - `project-cold-load`: a whole authenticated `check`/`test`/`run` of a
//!   shipped multi-file manifest, exactly what the CLI does per invocation.
//! - `project-retained`: one already authenticated revision, so preparation is
//!   separated from steady-state execution.
//! - `project-frontend-cache`: reanalysis after no change, after one leaf edit,
//!   and after a change to the provider every module consumes, at 1x/2x/4x.
//!
//! Every group verifies its expected outcome before timing, so a benchmark
//! cannot silently measure a failing operation, and nothing here bypasses
//! admission.

#[path = "support/project_fixture.rs"]
mod project_fixture;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use semaprax::project::{
    self, PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectExecutionOptions, ProjectFrontendCache,
    ProjectFrontendSource, ProjectManifest,
};
use std::cell::RefCell;
use std::path::Path;

/// Shipped manifests with more than one source file.
const MANIFESTS: [(&str, &str); 2] = [
    ("calculator-project", "examples/calculator-project"),
    ("apex-supply-chain", "examples/apex-supply-chain"),
];

fn manifest_path(directory: &str) -> std::path::PathBuf {
    Path::new(directory).join("semaprax.toml")
}

/// Source count and byte total of one shipped project: the work unit each
/// cold-load benchmark is parameterised by.
fn manifest_work(directory: &str) -> (usize, u64) {
    let manifest =
        ProjectManifest::parse(&std::fs::read_to_string(manifest_path(directory)).unwrap())
            .unwrap();
    let mut bytes = 0u64;
    for source in manifest.sources() {
        bytes += std::fs::metadata(Path::new(directory).join(source.as_str()))
            .unwrap()
            .len();
    }
    (manifest.sources().len(), bytes)
}

fn bench_project_cold_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("project-cold-load");
    for (id, directory) in MANIFESTS {
        let manifest = manifest_path(directory);
        let (sources, bytes) = manifest_work(directory);
        let parameter = format!("{sources}-sources-{bytes}-bytes");
        group.throughput(Throughput::Bytes(bytes));

        // Expected outcomes first, outside timing.
        project::with_authenticated_project(&manifest, |snapshot| snapshot.check()).unwrap();
        let entry = project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.execute_entry(&ProjectExecutionOptions::default())
        })
        .unwrap();
        assert!(entry.command_succeeded(), "{id} entry must succeed");
        let tests = project::with_authenticated_project(&manifest, |snapshot| {
            snapshot.execute_test(&ProjectExecutionOptions::default())
        })
        .unwrap();
        assert!(tests.command_succeeded(), "{id} tests must pass");

        group.bench_function(BenchmarkId::new(format!("{id}/check"), &parameter), |b| {
            b.iter(|| {
                let checked =
                    project::with_authenticated_project(&manifest, |snapshot| snapshot.check());
                std::hint::black_box(checked).unwrap();
            })
        });
        group.bench_function(BenchmarkId::new(format!("{id}/run"), &parameter), |b| {
            b.iter(|| {
                let execution = project::with_authenticated_project(&manifest, |snapshot| {
                    snapshot.execute_entry(&ProjectExecutionOptions::default())
                });
                std::hint::black_box(execution).unwrap();
            })
        });
        group.bench_function(BenchmarkId::new(format!("{id}/test"), &parameter), |b| {
            b.iter(|| {
                let execution = project::with_authenticated_project(&manifest, |snapshot| {
                    snapshot.execute_test(&ProjectExecutionOptions::default())
                });
                std::hint::black_box(execution).unwrap();
            })
        });
    }
    group.finish();
}

fn bench_project_retained(c: &mut Criterion) {
    let mut group = c.benchmark_group("project-retained");
    for (id, directory) in MANIFESTS {
        let manifest = manifest_path(directory);
        let (sources, bytes) = manifest_work(directory);
        let parameter = format!("{sources}-sources-{bytes}-bytes");
        let revision = project::with_authenticated_project(&manifest, |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap();

        // Steps used is a deterministic work counter for the evaluated closure.
        let execution = revision
            .execute_entry(&ProjectExecutionOptions::default())
            .unwrap();
        assert!(execution.command_succeeded());
        group.throughput(Throughput::Elements(execution.steps_used() as u64));
        group.bench_function(
            BenchmarkId::new(format!("{id}/retained-run"), &parameter),
            |b| {
                b.iter(|| {
                    let execution = revision.execute_entry(&ProjectExecutionOptions::default());
                    std::hint::black_box(execution).unwrap();
                })
            },
        );

        // Preparation and steady-state execution are separate costs: a prepared
        // interpreter resolves its closures once and keeps its worker.
        let ceilings = PreparedProjectInterpreterOptions::default();
        let options = PreparedProjectExecutionOptions::default();
        let cancellation = ProjectExecutionCancellation::new();
        let prepared = revision.prepare_interpreter(ceilings).unwrap();
        let first = prepared.execute_entry(&options, &cancellation).unwrap();
        std::hint::black_box(&first);
        group.bench_function(
            BenchmarkId::new(format!("{id}/prepared-run"), &parameter),
            |b| {
                b.iter(|| {
                    let execution = prepared.execute_entry(&options, &cancellation);
                    std::hint::black_box(execution).unwrap();
                })
            },
        );
        drop(prepared);
        group.bench_function(
            BenchmarkId::new(format!("{id}/prepare-interpreter"), &parameter),
            |b| {
                b.iter(|| {
                    let prepared = revision
                        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
                        .unwrap();
                    std::hint::black_box(&prepared);
                })
            },
        );
    }
    group.finish();
}

fn frontend_sources(sources: &[(String, String)]) -> Vec<ProjectFrontendSource> {
    sources
        .iter()
        .map(|(path, source)| ProjectFrontendSource::new(path, source).unwrap())
        .collect()
}

fn bench_project_frontend_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("project-frontend-cache");
    for scale in project_fixture::SCALES {
        let fixture = project_fixture::generate(scale);
        let manifest = ProjectManifest::parse(&fixture.manifest).unwrap();
        let original = frontend_sources(&fixture.sources);
        let leaf_edited = frontend_sources(&fixture.with_edited_leaf());
        let core_edited = frontend_sources(&fixture.with_edited_core());
        let parameter = format!(
            "{}x-{}-modules-{}-declarations-{}-bytes",
            scale,
            fixture.sources.len(),
            fixture.declarations(),
            fixture.source_bytes()
        );
        group.throughput(Throughput::Bytes(fixture.source_bytes()));

        // The fixture must be admissible before it is timed.
        ProjectFrontendCache::new()
            .build(&manifest, &original)
            .unwrap();

        group.bench_function(BenchmarkId::new("cold", &parameter), |b| {
            b.iter(|| {
                let built = ProjectFrontendCache::new().build(&manifest, &original);
                std::hint::black_box(built).unwrap();
            })
        });

        let warm = RefCell::new(ProjectFrontendCache::new());
        warm.borrow_mut().build(&manifest, &original).unwrap();
        group.bench_function(BenchmarkId::new("rebuild-unchanged", &parameter), |b| {
            b.iter(|| {
                let built = warm.borrow_mut().build(&manifest, &original);
                std::hint::black_box(built).unwrap();
            })
        });

        // Alternating variants keep exactly one module invalidated per build:
        // one leaf and its consumers, or the provider every module consumes.
        for (name, edited) in [
            ("one-leaf-edit", &leaf_edited),
            ("provider-edit", &core_edited),
        ] {
            let cache = RefCell::new(ProjectFrontendCache::new());
            cache.borrow_mut().build(&manifest, &original).unwrap();
            let toggle = RefCell::new(false);
            group.bench_function(BenchmarkId::new(name, &parameter), |b| {
                b.iter(|| {
                    let mut flag = toggle.borrow_mut();
                    *flag = !*flag;
                    let sources = if *flag { edited } else { &original };
                    let built = cache.borrow_mut().build(&manifest, sources);
                    std::hint::black_box(built).unwrap();
                })
            });
        }
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_project_cold_load,
    bench_project_retained,
    bench_project_frontend_cache
);
criterion_main!(benches);
