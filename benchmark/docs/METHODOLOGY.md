# Benchmark Methodology

## Host

Baseline host for `benchmark/results/baseline.json`:

- Model: Apple MacBook Pro 14" (M1, 8-core, 16 GB)
- OS: macOS 15.0 (Sequoia), darwin-arm64
- Rust: `rustc 1.88.0` (stable), `cargo 1.88`
- Clang: AppleClang 16.0.0 (for `native` builds)
- Commit: `770d2571` (main, `semaprax 0.3.0`)
- Date: 2026-09-04

All results are **local, single-host** evidence. They are not hosted,
release, or cross-platform claims.

## Microbenchmarks (`cargo bench`)

- Harness: `criterion 0.5` with `sample_size=100`, `warm_up_time=3s`,
  `measurement_time=5s`.
- Each bench is **cold** per sample (no retained `BuildCache`).
- Throughput is reported as `bytes/s` where input is source bytes;
  latency is `ns/op`.
- HTML reports are in `target/criterion/` and are not committed.

Bench groups:

- `compiler`: `parse` (lexer+parser), `verify` (type/effect/ownership),
  `graph` (semantic graph JSON), `format` (canonical), end-to-end `check`.
- `interpreter`: `hosted_interpreter::run` for scalar, loop and string
  fixtures (`meaning`, `math_algorithms`, `text_analytics`).
- `project`: `project::check` (Phase-A linked HIR), `project::test` and
  `project::run` for Project v1/v3/v8 manifests.

## Macrobenchmarks (`benchmark/run.py`)

- Runner: `python3 benchmark/run.py` invokes `cargo run --locked -p semaprax -- <command> <path>` via `subprocess`.
- Timing: `time.perf_counter()` wall time, `repetitions=5` (or `--quick` → 2).
  Reports `p50` (median) and `p95` (95th percentile) in `wall_ms`.
- Each scenario's source digest (`sha256:…`) is computed from the exact
  committed bytes and stored for reproducibility.
- `check`/`graph`/`context`/`run`/`test` are always measured. `build`
  (`--with-build`) additionally measures `build --target web` and
  `build --target native -o /tmp/...` where the toolchain is present;
  failures are recorded as `skipped` with reason.
- No warm cache is reused: every invocation is a fresh `cargo run` (which
  reuses the already compiled `target/debug/semaprax` binary, so only the
  `semaprax` execution is timed, not `cargo` compilation).

## JSON Schema (`benchmark.performance.v1`)

```json
{
  "schema": "benchmark.performance.v1",
  "host": "darwin-arm64",
  "rustc": "1.88.0",
  "commit": "770d2571",
  "timestamp": "2026-09-04T12:00:00Z",
  "scenarios": [
    {
      "id": "check-meaning",
      "command": "check",
      "path": "examples/meaning.spx",
      "repetitions": 5,
      "wall_ms": {"p50": 12, "p95": 15, "samples": [11,12,12,13,15]},
      "digest": "sha256:42aeae...",
      "status": "ok"
    }
  ]
}
```

- `wall_ms.samples` are the raw wall times in milliseconds.
- `status` is `ok` or `skipped` (with `reason`).
- The file is canonical JSON (sorted keys, 2-space indent, terminal LF).

## Comparison

```sh
python3 benchmark/run.py --compare benchmark/results/baseline.json --output /tmp/compare.json
```

The comparator computes `delta = (local.p50 - baseline.p50)/baseline.p50`
per scenario and reports `regression` if `delta > 0.15` (15% slower) or
`improvement` if `delta < -0.15`. Baseline is not a gate, just a reference.

## Non-claims

- No benchmark is in the blocking `quality` gate. Host variance, thermal
  throttling, and background load make it flaky as a gate.
- Results are **not** normalized for CPU frequency or cross-platform
  comparison.
- `build` benchmarks depend on the host Clang/wasm toolchain and are
  `skipped` where absent.
- Semantic benchmarks in `benchmarks/` (agent context, task comparison)
  remain separate and are not measured here.

## Reproducibility

Every scenario authenticates its `path` bytes before timing:

```sh
sha256sum examples/meaning.spx
cargo run --locked -p semaprax -- check examples/meaning.spx --json | head
```

The `digest` in the JSON must match `sha256sum`. If it does not, the
runner aborts with `digest mismatch`.

## Adding a microbenchmark

Add a function to `benches/compiler.rs` (or `interpreter.rs`, `project.rs`):

```rust
fn bench_parse(c: &mut Criterion) {
    let source = std::fs::read_to_string("examples/meaning.spx").unwrap();
    c.bench_function("parse-meaning", |b| b.iter(|| parse(&source, Path::new("examples/meaning.spx")).unwrap()));
}
```

Register it in `criterion_group!(benches, bench_parse, …)`.

## Adding a macro scenario

Add an entry to `benchmark/scenarios.json` (keep `id` sorted):

```json
{"id": "check-new-example", "kind": "single", "path": "examples/new.spx", "command": "check", "repetitions": 5}
```

Regenerate the baseline:

```sh
python3 benchmark/run.py --output benchmark/results/baseline.json
```

Commit both `scenarios.json` and the updated `baseline.json`.
