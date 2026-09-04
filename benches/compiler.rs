use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use std::path::Path;

fn bench_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler-parse");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
        ("apex-app", "examples/apex-supply-chain/src/app.spx"),
        ("rpg-combat", "examples/rpg-battle-project/src/combat.spx"),
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

fn bench_verify(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler-verify");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
        ("apex-app", "examples/apex-supply-chain/src/app.spx"),
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

fn bench_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler-graph");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("records", "examples/records.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        let program = semaprax::parse(&source, Path::new(path)).unwrap();
        assert!(semaprax::verify::verify(&program).is_empty());
        group.bench_function(id, |b| {
            b.iter(|| {
                let json = semaprax::graph::to_json(&program).unwrap();
                std::hint::black_box(json);
            })
        });
    }
    group.finish();
}

fn bench_format(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler-format");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("banking-ledger", "examples/banking_ledger.spx"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        let program = semaprax::parse(&source, Path::new(path)).unwrap();
        group.bench_function(id, |b| {
            b.iter(|| {
                let canonical = semaprax::format::canonical(&program);
                std::hint::black_box(canonical);
            })
        });
    }
    group.finish();
}

fn bench_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("compiler-check");
    for (id, path) in [
        ("meaning", "examples/meaning.spx"),
        ("math-algorithms", "examples/math_algorithms.spx"),
        ("text-analytics", "examples/text_analytics.spx"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        group.bench_function(id, |b| {
            b.iter(|| {
                let program = semaprax::parse(&source, Path::new(path)).unwrap();
                let diagnostics = semaprax::verify::verify(&program);
                assert!(diagnostics.is_empty());
                std::hint::black_box(program);
            })
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parse, bench_verify, bench_graph, bench_format, bench_check);
criterion_main!(benches);
