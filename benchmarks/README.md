# Benchmarks

This directory holds all SEMAPRAX benchmarks. `benches/` (sibling to this
directory, at repository root) holds the Rust `cargo bench` harness;
`benchmarks/` holds data-driven benchmark suites.

## Layout

| Path | Kind | Tool | Description |
| --- | --- | --- | --- |
| [`benches/`](../benches/) | Rust microbenchmarks | `cargo bench` (criterion) | `parse`/`verify`/`graph`/`format` and interpreter throughput |
| [`benchmarks/performance-v1/`](./performance-v1/) | Performance macrobenchmarks | `benchmarks/performance-v1/run.py` | CLI wall time for every `examples/` entry (`check`/`graph`/`run`/`context`/`test`/`build`) |
| [`benchmarks/hot-reload-v1/`](./hot-reload-v1/) | Interpreter development-loop benchmark | `benchmarks/hot-reload-v1/run.py` | Digest-pinned save-to-ack activation versus full and authenticated warm restart |
| [`benchmarks/agent-context-v1/`](./agent-context-v1/) | Semantic benchmark | `semaprax context` | Bounded context recall (corpus + maintenance fixture) |
| [`benchmarks/agent-task-comparison-v1/`](./agent-task-comparison-v1/) | Agent productivity benchmark | `scripts/agent-task-comparison.py` | Paired `graph-operational` vs `source-first` trials |
| [`benchmarks/cross-language-v1/`](./cross-language-v1/) | Cross-language Agent benchmark laboratory | `benchmarks/cross-language-v1/run.py` | Equivalence-specified tasks scored per language (SEMAPRAX, Rust, TypeScript wired; six more declared and blocked), pass/fail regression only — no timing (see [`docs/CROSS-LANGUAGE-BENCHMARK-V1.md`](../docs/CROSS-LANGUAGE-BENCHMARK-V1.md)) |
| [`benchmarks/bend2-law-v1/`](./bend2-law-v1/) | Bend 2 law benchmark | `benchmarks/bend2-law-v1/run.py` | Pinned Bend ordinary/verdict and separate SEMAPRAX paths, with explicit law-gaming controls and raw timing samples; unprovisioned cells remain unavailable (see [`docs/BEND2-LAW-BENCHMARK-V1.md`](../docs/BEND2-LAW-BENCHMARK-V1.md)) |

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

# Interpreter save-to-ack versus restart loops (already-built binary)
python3 benchmarks/hot-reload-v1/run.py --semaprax target/debug/semaprax --output /tmp/hot-reload-benchmark.json

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
- For a cross-language task: add a `tasks/<id>/EQUIVALENCE.md`,
  `public/<language>/` and `hidden/<language>/` trees, and an entry in
  `benchmarks/cross-language-v1/tasks.json`; wiring a new language needs an
  entry in `adapters.json` with a real, pinned, officially documented
  toolchain invocation. See `benchmarks/cross-language-v1/docs/METHODOLOGY.md`.

## Non-claims

All results are local, single-host evidence. See
[`benchmarks/performance-v1/docs/METHODOLOGY.md`](./performance-v1/docs/METHODOLOGY.md)
for host disclosure and methodology.
