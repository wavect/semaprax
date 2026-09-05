use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use semaprax::interpreter::{self, InterpreterOptions};
use std::path::Path;

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

fn bench_interpreter_loop(c: &mut Criterion) {
    const ITERATIONS: u64 = 10_000;
    let source = format!(
        r#"module bench.interpreter_loop;
@id("app.main")
fn main() -> i64 {{
    let mut acc = 0;
    let mut i = 0;
    while i < {ITERATIONS} {{
        acc = (acc + i * 3) % 1000003;
        i = i + 1;
        i < {ITERATIONS}
    }}
    acc
}}
"#
    );
    let path = std::env::temp_dir().join(format!(
        "semaprax-interpreter-bench-{}.spx",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let options = InterpreterOptions::new(65_536, 1_000_000).unwrap();
    let mut group = c.benchmark_group("interpreter-runtime");
    group.throughput(Throughput::Elements(ITERATIONS));
    group.bench_function("indexed-environment-loop", |b| {
        b.iter(|| {
            let result = interpreter::interpret(&path, "app.main", &[], &options).unwrap();
            std::hint::black_box(result);
        })
    });
    group.finish();
    std::fs::remove_file(path).unwrap();
}

criterion_group!(
    benches,
    bench_interpreter_parse,
    bench_interpreter_verify,
    bench_interpreter_loop
);
criterion_main!(benches);
