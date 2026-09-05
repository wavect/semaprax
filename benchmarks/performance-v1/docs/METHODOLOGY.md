# Benchmark Methodology

## Host

The host is not declared here; it is **recorded by the runner from the machine
that actually ran**, into the result document's `host` object: platform tag
(`darwin-arm64`, `linux-x86_64`, …), system, release, machine, logical CPU
count, load average at the start, and the `rustc`, `cargo` and `clang` versions
it observed. A Linux run therefore never identifies itself as macOS.

No baseline is committed. `results/baseline.json` records
`"recorded": false` with an empty scenario list, and
[`results/baseline.md`](../results/baseline.md) explains how to record a real
one. A host string with no measurements behind it is provenance without
evidence, so it is not written down.

All results are **local, single-host** evidence for one binary. They are not
hosted, release, or cross-platform claims.

## Microbenchmarks (`cargo bench`)

- Harness: `criterion 0.5` with `sample_size=100`, `warm_up_time=3s`,
  `measurement_time=5s`.
- Throughput is reported as `bytes/s` where the input is source bytes, or as
  evaluator steps/elements where the subject is execution; latency is `ns/op`.
- Each group name says what the sample includes: `cold` groups authenticate and
  analyse per sample, `retained` and `prepared` groups do that once in setup.
- Every group verifies its expected outcome before timing, and each benchmark id
  carries its work unit (source count, byte total, declarations, or evaluator
  steps).
- HTML reports are in `target/criterion/` and are not committed.

Bench groups:

- `compiler`: `parse` (lexer+parser), `verify` (type/effect/ownership),
  `graph` (semantic graph JSON), `format` (canonical), end-to-end `check`, over
  single-file examples.
- `interpreter`:
  - `interpreter-parse`, `interpreter-verify`: front-end phases over committed
    examples.
  - `interpreter-cold-end-to-end`: `interpreter::interpret(path, …)` — read,
    parse, verify, resolve, spawn the 64 MiB-stack evaluation thread, evaluate.
    This is one cold invocation's latency, not evaluator cost. Cases: a
    generated scalar loop, `text_analytics` (borrowed text and bytes with owned
    string cleanup), `math_algorithms`.
  - `interpreter-prepared-evaluator`: the same scalar loop as an authenticated
    Project, executed through `ProjectRevision::prepare_interpreter` — closures
    resolved once, worker retained — beside the retained-but-unprepared
    revision. The difference is the preparation the prepared case avoids. No
    benchmark constructs unchecked HIR.
- `project`:
  - `project-cold-load`: `check`, `run` and `test` through
    `project::with_authenticated_project` for the shipped `calculator-project`
    and `apex-supply-chain` manifests — the whole per-invocation CLI cost.
  - `project-retained`: one retained revision: steady-state `execute_entry`,
    prepared-interpreter execution, and interpreter preparation itself.
  - `project-frontend-cache`: `ProjectFrontendCache` reanalysis at 1x/2x/4x of a
    generated multi-module fixture — cold, unchanged rebuild, one leaf edited,
    and the provider every module consumes edited.

The generated fixture is ordinary canonical `.spx` source with an ordinary
`semaprax.project.v1` manifest. `tests/documentation/benchmark_fixtures.rs`
checks, runs and tests it, and pins the invalidation shape, so fixture validity
is covered without running a Criterion campaign in every pull request.

## Macrobenchmarks (`benchmarks/performance-v1/run.py`)

### What is timed

- The runner selects the measured compiler **before** any timing: either the
  binary given with `--semaprax PATH`, or the result of one
  `cargo build --locked -p semaprax --bin semaprax` whose own duration is
  recorded separately as `subject.build_ms`. Every sample is then one direct
  execution of that binary. No Cargo process, and no Cargo freshness check, is
  inside a sample.
- The result records what was measured: `subject.binary`, its `sha256` digest,
  the build `profile` (`debug`, `release` or `provided`), the reported
  `--version`, the `commit`, and whether the working tree was `dirty`.
- Timing is `time.perf_counter()` wall time over `repetitions` (default 5, or 2
  with `--quick`), reported as `p50`/`p95` with the raw `samples` and the
  `completed_samples` count.
- Digesting inputs, the expected-outcome verification run, and build-destination
  allocation all happen outside the timed region.

### Subject identity and drift

- A scenario's subject is its whole authenticated input closure, not one path. A
  project manifest does not bind its own sources, so the manifest and every
  source it declares are digested; `subject.digest` is one digest over that
  ordered list. Changing any included source changes the recorded identity.
- A scenario may declare `expected_digest`. A mismatch is reported as `drifted`
  and nothing is timed.
- The closure is digested again after the samples. If any byte changed during
  measurement the record becomes `drifted` and publishes no `wall_ms`.

### Outcomes

Every scenario declares an expected outcome (`"expect": "success"` by default,
`"failure"` where the diagnostic path is the subject). One untimed verification
run establishes the outcome before any sample is taken. The status is then one
of:

- `ok` — the expected outcome, with `wall_ms` and a completed sample count.
- `failed` — a wrong exit status or a timeout. It publishes **no** `wall_ms`;
  any observed times are kept under `observed_ms` for diagnosis only. Failures
  and drift make the runner exit non-zero.
- `skipped` — genuinely unsupported: the path is absent, or a tool the scenario
  declares in `requires` (for example `clang`) is not installed.
- `drifted` — the inputs did not match, or did not stay, what was measured.

### Builds

`build` scenarios run only with `--with-build`. Each repetition receives its own
freshly created temporary parent directory and writes to a not-yet-existing
`out` inside it, because publication requires a fresh destination. The runner
removes only the parent it created. The committed inventory contains web
(single-file and project) and native project builds; the native one declares
`requires: ["clang"]` and is skipped where Clang is absent.

## JSON Schema (`benchmark.performance.v2`)

```json
{
  "schema": "benchmark.performance.v2",
  "timestamp": "2026-09-05T18:00:00Z",
  "host": {"platform": "linux-x86_64", "system": "Linux", "release": "6.8.0",
           "machine": "x86_64", "cpu_count": 16, "python": "3.12.3",
           "rustc": "rustc 1.88.0 (…)", "cargo": "cargo 1.88.0 (…)",
           "clang": "clang version 18.1.3", "load_average": [0.2, 0.3, 0.4]},
  "subject": {"binary": "/…/target/debug/semaprax", "digest": "sha256:…",
              "profile": "debug", "build_ms": 41234.0,
              "version": "semaprax 0.3.0", "commit": "…", "dirty": false},
  "timing": {"clock": "time.perf_counter",
             "measures": "one direct execution of the selected semaprax binary",
             "excludes": "cargo startup, compiler build, input digesting, and the untimed verification run",
             "timeout_seconds": 120},
  "summary": {"ok": 22, "failed": 0, "skipped": 3, "drifted": 0},
  "scenarios": [
    {
      "id": "check-meaning",
      "command": "check",
      "kind": "single",
      "path": "examples/meaning.spx",
      "args": [],
      "expect": "success",
      "repetitions": 5,
      "completed_samples": 5,
      "subject": {"digest": "sha256:…",
                  "inputs": [{"path": "examples/meaning.spx", "digest": "sha256:42aeae…"}]},
      "verification": {"status": "ok", "reason": ""},
      "status": "ok",
      "wall_ms": {"p50": 12, "p95": 15, "samples": [11, 12, 12, 13, 15]}
    }
  ]
}
```

The file is canonical JSON (sorted keys, 2-space indent, terminal LF).
`--markdown PATH` renders the same document as a table.

## Comparison

```sh
python3 benchmarks/performance-v1/run.py \
  --compare benchmarks/performance-v1/results/baseline.json --output /tmp/compare.json
```

Only compatible successful pairs are scored: both sides `ok`, the same command
and arguments, the same subject digest, and both carrying timing. Anything else
is reported as `incomparable` with a reason, so a fast failure can never be
scored as an improvement. For a scored pair the comparator computes
`delta = (local.p50 - baseline.p50)/baseline.p50` and reports `regression` above
`+15%` or `improvement` below `-15%`. Comparing against a baseline that holds no
recorded measurement is an error, not an empty comparison. Wall-time thresholds
stay advisory across heterogeneous hosts; the runner's exit status reflects
result accounting (failures and drift), not speed.

## Non-claims

- No benchmark is in the blocking `quality` gate. Host variance, thermal
  throttling, and background load make it flaky as a gate.
- Results are **not** normalized for CPU frequency or cross-platform
  comparison. A `debug` subject and a `release` subject are different subjects.
- `build` scenarios depend on the host Clang/wasm toolchain and are `skipped`
  where a declared tool is absent.
- Semantic benchmarks in `benchmarks/` (agent context, task comparison)
  remain separate and are not measured here.

## Reproducibility

Every scenario authenticates its whole input closure before timing and again
afterwards:

```sh
sha256sum examples/meaning.spx
python3 benchmarks/performance-v1/run.py --dry-run --output /tmp/plan.json
```

The plan reports each scenario's `subject.inputs` with their digests and the
`subject.digest` over them. Those digests must match `sha256sum`. A recorded
`expected_digest` that does not match, or any input that changes during the run,
fails the scenario closed as `drifted`.

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

Add an entry to `benchmarks/performance-v1/scenarios.json`:

```json
{"id": "check-new-example", "kind": "single", "path": "examples/new.spx", "command": "check", "repetitions": 5}
```

Optional keys: `"args"`, `"expect": "failure"` where the diagnostic path is the
subject, `"requires": ["clang"]` for an external tool, and `"expected_digest"`
to pin the subject bytes. Confirm the entry resolves and identify its subject:

```sh
python3 benchmarks/performance-v1/run.py --dry-run --output /tmp/plan.json
python3 benchmarks/performance-v1/run.py --only check-new-example \
  --semaprax target/debug/semaprax --output /tmp/one.json
```

Regenerating `results/baseline.json` is a separate act: do it on an idle host
from a clean checkout, and commit `baseline.json` with its `--markdown`
rendering. Numbers from a loaded shared machine are not a baseline.
