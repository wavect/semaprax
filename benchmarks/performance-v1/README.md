# SEMAPRAX Performance Benchmarks

Audience: compiler contributors, performance operators, and maturity reviewers.

This directory is the **performance benchmark suite** — complementary to the
semantic benchmarks in [`benchmarks/`](../) (agent context, task
comparison). It measures *throughput and latency*, not agent productivity.

## What is measured

| Suite | Subject | Metric | Tool |
| --- | --- | --- | --- |
| `compiler` | `parse` → `verify` → `graph` → `format` for single-file `.spx` | ns/op, throughput (bytes/s) | `cargo bench --bench compiler` (criterion) |
| `interpreter` | `semaprax run` (hosted interpreter) for scalar, loop and string examples | ns/op | `cargo bench --bench interpreter` |
| `project` | `check` / `test` / `run` for Project v1/v3/v8 manifests | ns/op | `cargo bench --bench project` |
| `macro` | one direct execution of a selected `semaprax` binary: `check`/`graph`/`context`/`run`/`test`/`build` over committed `examples/` entries | wall ms, p50/p95 | `benchmarks/performance-v1/run.py` or `benchmarks/performance-v1/run.sh` |
| `build` | `native` and `web` artifact emission (Clang C11, wasm) where toolchain is present | wall ms | `benchmarks/performance-v1/run.py --with-build` |

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
python3 benchmarks/performance-v1/run.py --output benchmarks/performance-v1/results/local.json
python3 benchmarks/performance-v1/run.py --with-build --output benchmarks/performance-v1/results/local-with-build.json

# shell wrapper (requires python3, no hyperfine needed)
./benchmarks/performance-v1/run.sh
./benchmarks/performance-v1/run.sh --with-build

# resolve the inventory without timing anything
python3 benchmarks/performance-v1/run.py --dry-run --output /tmp/plan.json

# measure an already built binary (nothing Cargo does is inside a sample)
python3 benchmarks/performance-v1/run.py --semaprax target/debug/semaprax \
  --only check-meaning --output /tmp/one.json
```

**No baseline is committed.** [`results/baseline.json`](results/baseline.json)
records `"recorded": false` with an empty scenario list and
[`results/baseline.md`](results/baseline.md) says how to record one on an idle
host. Comparing against it is an error rather than an empty comparison:

```sh
python3 benchmarks/performance-v1/run.py --compare benchmarks/performance-v1/results/baseline.json --output /tmp/compare.json
```

Only compatible successful pairs are ever scored — same command, same arguments,
same subject digest, both `ok` — so a fast failure cannot be reported as an
improvement.

## Scenarios

[`scenarios.json`](scenarios.json) is the canonical inventory of macro
scenarios. Each entry has an `id`, `kind` (`single`|`project`), `path`,
`command` and `repetitions`, and may declare `args`, an expected outcome
(`"expect": "failure"` where the diagnostic path is the subject), external tools
it needs (`"requires": ["clang"]`), and an `expected_digest`. The runner digests
each scenario's whole input closure — for a project, the manifest *and* every
source it declares — before timing and again afterwards; drift fails the
scenario closed.

```sh
cat benchmarks/performance-v1/scenarios.json | python3 -m json.tool | head -n 40
```

## Results

Results are written as `benchmark.performance.v2` JSON. The document records the
host it actually ran on, the binary it measured (path, digest, profile, version,
commit, dirty flag), what the timing includes, a status summary, and one record
per scenario:

```json
{
  "schema": "benchmark.performance.v2",
  "host": {"platform": "linux-x86_64", "cpu_count": 16, "rustc": "rustc 1.88.0 (…)"},
  "subject": {"binary": "target/debug/semaprax", "digest": "sha256:…", "profile": "debug",
              "commit": "…", "dirty": false},
  "summary": {"ok": 22, "failed": 0, "skipped": 3, "drifted": 0},
  "scenarios": [
    {"id": "check-meaning", "command": "check", "status": "ok", "expect": "success",
     "repetitions": 5, "completed_samples": 5,
     "wall_ms": {"p50": 12, "p95": 15, "samples": [11, 12, 12, 13, 15]},
     "subject": {"digest": "sha256:…", "inputs": [{"path": "examples/meaning.spx", "digest": "sha256:…"}]}}
  ]
}
```

`status` is `ok`, `failed`, `skipped` (a declared tool or path is genuinely
absent) or `drifted`. Only `ok` records carry `wall_ms`; a failure keeps its
observed times under `observed_ms` for diagnosis and cannot be compared.
Failures and drift make the runner exit non-zero. `--markdown PATH` renders the
same document as a table.

## Methodology

See [`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) for measurement
methodology, host disclosure, and non-claims. Key invariants:

- The measured binary is built or supplied **before** timing; no Cargo work is
  inside a sample.
- `cargo bench` uses `criterion` with `sample_size=100`, `warm_up_time=3s`.
- Macro `run.py` uses `time.perf_counter()` with 5 repetitions, reports `p50`/`p95`.
- Each scenario's expected outcome is established by an untimed verification run.
- No benchmark writes to `target/` beyond `criterion`'s own output.
- The suite never mutates `examples/` or `benchmarks/` sources, and removes only
  the temporary directories it creates itself.

## CI

The suite is **not** in the blocking `quality` gate (it would flake on
host variance). Run it locally before performance-sensitive changes or via:

```sh
cargo bench --bench compiler -- --quick
python3 benchmarks/performance-v1/run.py --quick --output /tmp/quick.json
```

## Adding a scenario

1. Add the `.spx` or `semaprax.toml` under `examples/` (or `benchmarks/` for
   private fixtures) and ensure `cargo test --test examples` passes.
2. Add an entry to `benchmarks/performance-v1/scenarios.json`.
3. Confirm it resolves and measures:
   `python3 benchmarks/performance-v1/run.py --dry-run --output /tmp/plan.json`
   then `--only <id> --semaprax target/debug/semaprax --output /tmp/one.json`.
4. Recording a baseline is a separate act, on an idle host from a clean
   checkout. Do not commit numbers measured on a loaded shared machine.

## Non-claims

This suite does not claim hosted, cross-platform, or production performance.
It is local evidence for one host and one build, like `examples/README.md`.
Hosted or multi-engine claims require separate provisioned measurement.
