//! Interpreter costs, separated by what each one actually includes.
//!
//! `interpreter::interpret(path, ..)` reads the file, parses it, verifies it,
//! resolves HIR and spawns a dedicated 64 MiB-stack evaluation thread before a
//! single expression is evaluated. That is the honest cost of one cold CLI
//! invocation, so it is named for that. Steady-state evaluator cost is measured
//! separately through the existing prepared-project interpreter, which resolves
//! its closures once and keeps its worker between executions.
//!
//! No benchmark constructs unchecked HIR: the prepared case goes through an
//! ordinary authenticated Project.

#[path = "support/project_fixture.rs"]
mod project_fixture;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::project::{
    self, PreparedProjectExecutionOptions, PreparedProjectInterpreterOptions,
    ProjectExecutionCancellation, ProjectExecutionOptions,
};
use std::path::{Path, PathBuf};

const LOOP_ITERATIONS: u64 = 10_000;

fn bench_interpreter_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter-parse");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
        (
            "apex-supply-chain-app",
            "examples/apex-supply-chain/src/app.spx",
        ),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        let bytes = source.len() as u64;
        group.throughput(Throughput::Bytes(bytes));
        group.bench_function(id, |b| {
            b.iter(|| {
                let program = semaprax::parse(&source, Path::new(path)).unwrap();
                std::hint::black_box(program);
            })
        });
    }
    group.finish();
}

fn bench_interpreter_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter-verify");
    for (id, path) in [
        ("banking-ledger", "examples/banking_ledger.spx"),
        ("text-analytics", "examples/text_analytics.spx"),
        ("order-lifecycle", "examples/order_lifecycle.spx"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        let program = semaprax::parse(&source, Path::new(path)).unwrap();
        group.bench_function(id, |b| {
            b.iter(|| {
                let diagnostics = semaprax::verify::verify(&program);
                std::hint::black_box(diagnostics);
            })
        });
    }
    group.finish();
}

/// A scratch directory owned by this process, removed when it is dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "semaprax-interpreter-bench-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        // Project loading rejects a symlinked ancestor; the platform temporary
        // directory is one on macOS.
        Self(std::fs::canonicalize(&path).unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// One whole cold invocation: read, parse, verify, resolve, spawn the
/// evaluation thread, evaluate. This is end-to-end latency, not evaluator cost.
///
/// Every subject here must be one the interpreter actually admits from its
/// entry point. The first executed run of this group found
/// `examples/text_analytics.spx` under the name `borrowed-text-and-owned-cleanup`:
/// the interpreter rejects it with SPX-F102 `unsupported_callee`, because it
/// passes records through function signatures, so the case had never measured
/// anything and could not. `owned-resource-declarations` replaces it with a
/// program the interpreter does admit; that program *declares* an owned
/// resource, its drop and a postcondition, so ownership analysis and cleanup
/// planning are inside the sample while its evaluation is a constant. When the
/// interpreter admits calling through `own`/`borrow` parameters from an entry
/// point, this case should move to a program that exercises them at runtime.
/// `tests/documentation/benchmark_fixtures.rs` pins every subject's
/// admissibility so an inadmissible one fails a test rather than a timing run.
fn bench_interpreter_cold_end_to_end(c: &mut Criterion) {
    let scratch = Scratch::new("cold");
    let loop_path = scratch.0.join("scalar_loop.spx");
    std::fs::write(
        &loop_path,
        project_fixture::scalar_loop_source(LOOP_ITERATIONS),
    )
    .unwrap();
    let options = InterpreterOptions::new(65_536, 1_000_000).unwrap();

    let mut subjects = vec![(
        "scalar-loop",
        loop_path.clone(),
        "bench.scalar.main",
        LOOP_ITERATIONS,
    )];
    subjects.extend(
        project_fixture::COLD_INTERPRETED_EXAMPLES
            .iter()
            .map(|(id, path, function)| (*id, PathBuf::from(path), *function, 1)),
    );

    let mut group = c.benchmark_group("interpreter-cold-end-to-end");
    for (id, path, function, elements) in subjects {
        // The measured program must produce its expected result before timing.
        interpreter::interpret(&path, function, &[], &options).unwrap();
        let bytes = std::fs::metadata(&path).unwrap().len();
        group.throughput(Throughput::Elements(elements));
        group.bench_function(BenchmarkId::new(id, format!("{bytes}-source-bytes")), |b| {
            b.iter(|| {
                let result = interpreter::interpret(&path, function, &[], &options).unwrap();
                std::hint::black_box(result);
            })
        });
    }
    group.finish();
}

/// Steady-state evaluation only: the project is authenticated, resolved and
/// prepared once, outside the measured loop.
fn bench_interpreter_prepared(c: &mut Criterion) {
    let scratch = Scratch::new("prepared");
    let manifest = project_fixture::scalar_loop_project(LOOP_ITERATIONS).write_to(&scratch.0);
    let revision =
        project::with_authenticated_project(&manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
    let options = PreparedProjectExecutionOptions::default();
    let cancellation = ProjectExecutionCancellation::new();
    let prepared = revision
        .prepare_interpreter(PreparedProjectInterpreterOptions::default())
        .unwrap();
    let first = prepared.execute_entry(&options, &cancellation).unwrap();
    let steps = first.steps_used() as u64;
    let untraced = prepared
        .execute_entry_untraced(options.max_steps, &cancellation)
        .unwrap();
    assert_eq!(untraced.outcome(), first.outcome());
    assert_eq!(untraced.steps_used(), first.steps_used());
    let retained = revision
        .execute_entry(&ProjectExecutionOptions::default())
        .unwrap();
    assert_eq!(retained.steps_used(), first.steps_used());
    match retained.outcome() {
        project::ProjectExecutionOutcome::Returned(value) => assert_eq!(
            first.outcome(),
            &project::ProjectPreparedExecutionOutcome::Returned(*value)
        ),
        other => panic!("scalar benchmark must return successfully: {other:?}"),
    }

    let mut group = c.benchmark_group("interpreter-prepared-evaluator");
    group.throughput(Throughput::Elements(LOOP_ITERATIONS));
    group.bench_function(
        BenchmarkId::new("scalar-loop", format!("{steps}-evaluator-steps")),
        |b| {
            b.iter(|| {
                let execution = prepared.execute_entry(&options, &cancellation);
                std::hint::black_box(execution).unwrap();
            })
        },
    );
    // The no-trace arm emits only outcome/fuel; tracing is a separate cost.
    group.bench_function(
        BenchmarkId::new("scalar-loop-untraced", format!("{steps}-evaluator-steps")),
        |b| {
            b.iter(|| {
                std::hint::black_box(
                    prepared.execute_entry_untraced(options.max_steps, &cancellation),
                )
                .unwrap();
            })
        },
    );
    // Cold arms authenticate and prepare inside each sample. They return the
    // exact same products as their corresponding prepared arms; setup and
    // worker teardown are included rather than shifted outside the timer.
    let cold_untraced = || {
        project::with_authenticated_project(&manifest, |snapshot| {
            let worker =
                snapshot.prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
            worker.execute_entry_untraced(options.max_steps, &cancellation)
        })
    };
    let cold_traced = || {
        project::with_authenticated_project(&manifest, |snapshot| {
            let worker =
                snapshot.prepare_interpreter(PreparedProjectInterpreterOptions::default())?;
            worker.execute_entry(&options, &cancellation)
        })
    };
    assert_eq!(cold_untraced().unwrap(), untraced);
    assert_eq!(cold_traced().unwrap(), first);
    group.bench_function(
        BenchmarkId::new(
            "scalar-loop-cold-untraced",
            format!("{steps}-evaluator-steps"),
        ),
        |b| b.iter(|| std::hint::black_box(cold_untraced()).unwrap()),
    );
    group.bench_function(
        BenchmarkId::new(
            "scalar-loop-cold-traced",
            format!("{steps}-evaluator-steps"),
        ),
        |b| b.iter(|| std::hint::black_box(cold_traced()).unwrap()),
    );
    // Retained execution includes closure re-admission, worker creation and its
    // Project execution report. It is still a whole-operation measurement,
    // not an isolated preparation delta; the prepared result has no envelope.
    group.bench_function(
        BenchmarkId::new("scalar-loop-retained", format!("{steps}-evaluator-steps")),
        |b| {
            b.iter(|| {
                let execution = revision.execute_entry(&ProjectExecutionOptions::default());
                std::hint::black_box(execution).unwrap();
            })
        },
    );
    group.finish();
}

criterion_group!(
    benches,
    bench_interpreter_parse,
    bench_interpreter_verify,
    bench_interpreter_cold_end_to_end,
    bench_interpreter_prepared
);
criterion_main!(benches);
