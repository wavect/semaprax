use criterion::{criterion_group, criterion_main, Criterion};
use std::path::Path;

fn bench_project_parse(c: &mut Criterion) {
    let mut group = c.benchmark_group("project-parse");
    for (id, path) in [
        ("calculator-project-app", "examples/calculator-project/src/app.spx"),
        ("apex-supply-chain-demand", "examples/apex-supply-chain/src/demand.spx"),
        ("analytics-pipeline-core", "examples/analytics-pipeline-project/src/core.spx"),
    ] {
        let source = std::fs::read_to_string(path).unwrap();
        group.bench_function(id, |b| {
            b.iter(|| {
                let program = semaprax::parse(&source, Path::new(path)).unwrap();
                std::hint::black_box(program);
            })
        });
    }
    group.finish();
}

fn bench_project_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("project-graph");
    for (id, path) in [
        ("calculator", "examples/calculator.spx"),
        ("apex-demand", "examples/apex-supply-chain/src/demand.spx"),
        ("rpg-stats", "examples/rpg-battle-project/src/stats.spx"),
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

criterion_group!(benches, bench_project_parse, bench_project_graph);
criterion_main!(benches);
