# Benchmarks

This directory holds all SEMAPRAX benchmarks. `benches/` (sibling to this
directory, at repository root) holds the Rust `cargo bench` harness;
`benchmarks/` holds data-driven benchmark suites.

## Layout

| Path | Kind | Tool | Description |
| --- | --- | --- | --- |
| [`benches/`](../benches/) | Rust microbenchmarks | `cargo bench` (criterion) | `parse`/`verify`/`graph`/`format` and interpreter throughput |
| [`benchmarks/performance-v1/`](./performance-v1/) | Performance macrobenchmarks | `benchmarks/performance-v1/run.py` | CLI wall time for every `examples/` entry (`check`/`graph`/`run`/`context`/`test`/`build`) |
| [`benchmarks/agent-context-v1/`](./agent-context-v1/) | Semantic benchmark | `semaprax context` | Bounded context recall (corpus + maintenance fixture) |
| [`benchmarks/agent-task-comparison-v1/`](./agent-task-comparison-v1/) | Agent productivity benchmark | `scripts/agent-task-comparison.py` | Paired `graph-operational` vs `source-first` trials |

## Quick start

```sh
# Rust microbenchmarks
cargo bench --bench compiler
cargo bench --bench interpreter
cargo bench --bench project
cargo bench  # all

# Performance macrobenchmarks
python3 benchmarks/performance-v1/run.py --output benchmarks/performance-v1/results/local.json
./benchmarks/performance-v1/run.sh
./benchmarks/performance-v1/run.sh --with-build

# Semantic benchmarks
cat benchmarks/agent-context-v1/corpus.tsv
python3 scripts/agent-task-comparison.py plan --manifest benchmarks/agent-task-comparison-v1/manifest.json --output /tmp/plan.json
```

## Consolidation

Prior to `4f835caa`, performance benchmarks lived in `benchmark/` (singular)
at the repository root. They have been consolidated into
`benchmarks/performance-v1/` for consistency with the versioned
`agent-*` suites. The singular `benchmark/` path no longer exists; update
scripts to `benchmarks/performance-v1/`.

`benches/` remains separate by Rust convention (`cargo bench` expects
`benches/*.rs` at the repository root). It is not moved into `benchmarks/`.

## Adding a benchmark

- For Rust microbenchmarks: edit `benches/*.rs` and add a `criterion_group!`.
- For performance macros: edit `benchmarks/performance-v1/scenarios.json` and
  regenerate the baseline (`python3 benchmarks/performance-v1/run.py --output benchmarks/performance-v1/results/baseline.json`).
- For semantic tasks: see `benchmarks/agent-task-comparison-v1/README` (if present) or
  `docs/AGENT-TASK-COMPARISON-V1.md`.

## Non-claims

All results are local, single-host evidence. See
[`benchmarks/performance-v1/docs/METHODOLOGY.md`](./performance-v1/docs/METHODOLOGY.md)
for host disclosure and methodology.
