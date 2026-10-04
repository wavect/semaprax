# Interpreter hot-reload benchmark v1

This suite measures three local development loops against one digest-pinned
calculator Project: a running interpreter's save-to-ack activation, a full
fresh-process interpreter restart, and an authenticated semantic-cache restart.
It is an interpreter-only suite. The source-Agent lane remains
`migration-required` in the shared acceptance manifest and produces no Agent
journey, state, budget, deadline, or effect claim.

Build the selected compiler once, outside samples, then run:

```sh
python3 benchmarks/hot-reload-v1/run.py \
  --semaprax target/debug/semaprax --samples 11 \
  --output /tmp/hot-reload-benchmark.json
```

The runner creates a private temporary Project for every sample. It invokes A
(42), writes exact B source (48), requires `candidate_admitted`, requires the
`activated` acknowledgement, invokes B (48), then stops. The live loop keeps
one `semaprax dev … --jsonl --interpreter` child for A through B, so it measures
an actual save-to-ack activation rather than a process replacement.

`full-restart` starts a fresh interpreter process after B is saved. The
authenticated warm loop initializes a private cache store, persists A, saves B,
then runs `semantic-cache-refresh` followed by `semantic-cache-warm-open` on
B's successor entry. It is deliberately reported as restart-with-reuse, not
hot reload.

Each result includes every raw sample, sample count, median, p95, binary digest,
Git commit, host facts, the acceptance-manifest digest, and all fixture digests.
`peak_rss_bytes` is `null` until a portable per-child measurement exists.
The v1 control protocol exposes plan and activate acknowledgements but no
internal admission timer: the report records candidate preparation as the full
plan round trip and leaves `source_admission_check_ms` unavailable. The fixture
has no outstanding invocation, so safe-point wait is zero. Those limitations
are carried in each live record instead of being estimated.

Validate the committed contract without starting the compiler or timing a host:

```sh
python3 benchmarks/hot-reload-v1/test_run.py
python3 benchmarks/hot-reload-v1/run.py --dry-run --output /tmp/hot-reload-plan.json
```
