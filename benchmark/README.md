# SEMAPRAX Performance Benchmarks

Audience: compiler contributors, performance operators, and maturity reviewers.

This directory is the **performance benchmark suite** — complementary to the
semantic benchmarks in [`benchmarks/`](../benchmarks/) (agent context, task
comparison). It measures *throughput and latency*, not agent productivity.

## What is measured

| Suite | Subject | Metric | Tool |
| --- | --- | --- | --- |
| `compiler` | `parse` → `verify` → `graph` → `format` for single-file `.spx` | ns/op, throughput (bytes/s) | `cargo bench --bench compiler` (criterion) |
| `interpreter` | `semaprax run` (hosted interpreter) for scalar, loop and string examples | ns/op | `cargo bench --bench interpreter` |
| `project` | `check` / `test` / `run` for Project v1/v3/v8 manifests | ns/op | `cargo bench --bench project` |
| `macro` | CLI `check`/`graph`/`context`/`run`/`test`/`build` wall time for every committed `examples/` entry | wall ms, p50/p95 | `benchmark/run.py` or `benchmark/run.sh` |
| `build` | `native` and `web` artifact emission (Clang C11, wasm) where toolchain is present | wall ms | `benchmark/run.py --with-build` |

All benchmarks are **deterministic and offline** — no network, no registry.

## Quick start

```sh
# microbenchmarks (criterion, HTML report in target/criterion/)
cargo bench --bench compiler
cargo bench --bench interpreter
cargo bench --bench project

# all microbenchmarks
cargo bench

# macrobenchmarks (CLI, JSON + markdown)
python3 benchmark/run.py --output benchmark/results/local.json
python3 benchmark/run.py --with-build --output benchmark/results/local-with-build.json

# shell wrapper (requires python3, no hyperfine needed)
./benchmark/run.sh
./benchmark/run.sh --with-build
```

Baseline results for the reference machine (Apple M1, macOS 15, rustc 1.88)
are in [`results/baseline.json`](results/baseline.json) and
[`results/baseline.md`](results/baseline.md). Compare locally with:

```sh
python3 benchmark/run.py --compare benchmark/results/baseline.json --output /tmp/compare.json
```

## Scenarios

[`scenarios.json`](scenarios.json) is the canonical inventory of macro
scenarios. Each entry has an `id`, `kind` (`single`|`project`), `path`,
`command` and `repetitions`. The runner authenticates every listed source
byte before timing and reports `SHA-256` digests for reproducibility.

```sh
cat benchmark/scenarios.json | python3 -m json.tool | head -n 40
```

## Results

Results are written as `benchmark.performance.v1` JSON:

```json
{
  "schema": "benchmark.performance.v1",
  "host": "darwin-arm64",
  "rustc": "1.88.0",
  "commit": "770d2571",
  "timestamp": "2026-09-04T00:00:00Z",
  "scenarios": [
    {"id": "check-meaning", "command": "check", "path": "examples/meaning.spx", "repetitions": 5, "wall_ms": {"p50": 12, "p95": 15}, "digest": "sha256:..."}
  ]
}
```

`benchmark/results/baseline.json` is the exact reference output from the
commit above. `benchmark/results/baseline.md` is its markdown rendering.

## Methodology

See [`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) for measurement
methodology, host disclosure, and non-claims. Key invariants:

- Every benchmark is **cold** (no warm image/cache) unless noted.
- `cargo bench` uses `criterion` with `sample_size=100`, `warm_up_time=3s`.
- Macro `run.py` uses `time.perf_counter()` with 5 repetitions, reports `p50`/`p95`.
- No benchmark writes to `target/` beyond `criterion`'s own output.
- The suite never mutates `examples/` or `benchmarks/` sources.

## CI

The suite is **not** in the blocking `quality` gate (it would flake on
host variance). Run it locally before performance-sensitive changes or via:

```sh
cargo bench --bench compiler -- --quick
python3 benchmark/run.py --quick
```

## Adding a scenario

1. Add the `.spx` or `semaprax.toml` under `examples/` (or `benchmarks/` for
   private fixtures) and ensure `cargo test --test examples` passes.
2. Add an entry to `benchmark/scenarios.json` (keep `id` sorted).
3. Run `python3 benchmark/run.py --output benchmark/results/baseline.json`
   and commit the updated baseline with the feature.

## Non-claims

This suite does not claim hosted, cross-platform, or production performance.
It is local evidence for one host and one build, like `examples/README.md`.
Hosted or multi-engine claims require separate provisioned measurement.
