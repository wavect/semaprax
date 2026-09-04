use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::path::Path;

fn bench_interpreter_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter-parse");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
        ("apex-supply-chain-app", "examples/apex-supply-chain/src/app.spx"),
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

criterion_group!(benches, bench_interpreter_parse, bench_interpreter_verify);
criterion_main!(benches);
