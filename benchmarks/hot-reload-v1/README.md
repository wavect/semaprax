# Interpreter hot-reload benchmark v1

This suite measures three local development loops against one digest-pinned
calculator Project: a running interpreter's save-to-ack activation, a full
fresh-process interpreter restart, and an authenticated semantic-cache restart.
It is an interpreter-only suite. The source-Agent lane remains
`migration-required` in the shared acceptance manifest and produces no Agent
journey, state, budget, deadline, or effect claim.

Build the selected compiler once with `SEMAPRAX_BUILD_COMMIT` set to the checkout's exact commit, outside samples, then run:

```sh
benchmarks/hot-reload-v1/macos-pilot.sh \
  --semaprax /absolute/path/to/semaprax \
  --output /tmp/hot-reload-benchmark.json
```

The Mac pilot refuses a non-macOS host and requires the CLI's embedded commit to
match the current checkout before it runs. The runner creates a private temporary
Project for every sample. Its scenario matrix records a cold small A→B session,
a warm A→B→A cycle in one interpreter child, an identical-byte save returning
`unchanged`, a core-plus-test edit that refuses an incompatible callable closure
and retains A, and an invalid-edit rejection followed by B repair. Each
compatible edit requires `candidate_admitted`, `activated`, and the expected
invocation result. The failed-edit case requires
`candidate_rejected`, invokes the retained A result, then repairs to B. Every
live scenario ends with the JSONL `stopped` reply and a zero child exit.

`full-restart` starts a fresh interpreter process after B is saved. The
authenticated warm loop initializes a private cache store, persists A, saves B,
then runs `semantic-cache-refresh` followed by `semantic-cache-warm-open` on
B's successor entry. It is deliberately reported as restart-with-reuse, not
hot reload.

Each result includes every raw sample, measured and discarded warmup counts, median, p95, binary digest,
Git commit, host facts, the acceptance-manifest digest, and all fixture digests.
`peak_rss_bytes` is `null` until a portable per-child measurement exists.
The v1 control protocol exposes plan and activate acknowledgements but no
internal admission or candidate-preparation timers. Each phase therefore keeps
the measured source-write duration, save-to-plan response, plan-control round
trip, and activation-control round trip separate, while retaining
`source_admission_check_ms` and `candidate_preparation_ms` as unavailable. The
fixture has no outstanding invocation, so safe-point wait is exactly zero.
Those limitations are carried in each live record instead of being estimated.

Validate the committed contract without starting the compiler or timing a host:

```sh
python3 benchmarks/hot-reload-v1/test_run.py
python3 benchmarks/hot-reload-v1/test_macos_cross_layer_evidence.py
python3 benchmarks/hot-reload-v1/run.py --dry-run --output /tmp/hot-reload-plan.json
```

## Cross-layer acceptance record

`cross-layer-manifest.json` maps the existing HR-01, HR-02, HR-04, and
source-Agent selectors into one HR-07 evidence record. It includes watcher
selectors for B→invalid-C retention and invalid-C→repair, source-Agent A→B,
retained A→B→C handoff and replay, and the prepared-worker A→B→C identity
selector. The last of these is an opaque in-process worker observation; it is
not a process identity.

The cross-layer runner never builds a selector. Pass an already-built exact
selector command for each cell that should be timed. It records raw elapsed
values, summary values, and stdout/stderr digests only after every invocation
returns zero. Omitted cells remain `unavailable`; the committed manifest keeps
native-process identity and native/Wasm state swap explicitly unavailable.

```sh
python3 benchmarks/hot-reload-v1/cross_layer.py --dry-run \
  --output /tmp/hot-reload-cross-layer-plan.json

# The argv is JSON so paths and exact test filters stay unambiguous.
python3 benchmarks/hot-reload-v1/cross_layer.py --samples 5 \
  --selector-command '{"id":"source-agent-a-b","argv":["/abs/path/to/cli_help_surface_v1","source_agent_hot_reload::full_dev_source_agent_migrates_real_journal_a_to_b_with_local_opencode_stub","--exact"]}' \
  --output /tmp/hot-reload-cross-layer.json
```

The example does not stand in for a committed measurement. A supplied command
must start with an absolute executable file. The report records the repository
head, the resolved selector path, and the SHA-256 of that executable for every
sample. It refuses if those bytes change during the sample. This binds the
observed selector bytes but does not establish platform support, production
rollout, or that the executable was built from the reported checkout.

## Compact cross-layer capture

On macOS, capture the current-head interpreter receipt and the exact native/Wasm
limitations in one JSON file. The wrapper requires an already-built CLI whose
`version --json` commit equals the checkout's `HEAD`; it does not invoke Cargo.

```sh
benchmarks/hot-reload-v1/macos-cross-layer-capture.sh \
  --semaprax "$SEMAPRAX_BIN" \
  --output /tmp/hot-reload-cross-layer-capture.json
```

The capture stores the benchmark receipt SHA-256, CLI binary digest, CLI version
and commit, and the runner stdout/stderr SHA-256 values. `native-process-identity`
and `native-or-wasm-state-swap` remain `unavailable` with the committed manifest
requirements. It does not turn either limitation into a measured selector.

## macOS source-attributed cross-layer roll-up

For one current-checkout macOS run that builds the CLI and each owned test
harness itself, use the bounded roll-up command when a Cargo slot is available:

```sh
benchmarks/hot-reload-v1/macos-cross-layer-evidence.sh \
  --target-dir "$PWD/target/hr07-macos-evidence" \
  --output /tmp/hot-reload-macos-cross-layer-evidence.json
```

It permits only a private target directory below this checkout's `target/` and
uses one Cargo job. The runner builds the current checkout with
`SEMAPRAX_BUILD_COMMIT` set to `HEAD`, captures the interpreter receipt, then
runs every supported test selector exactly once in manifest order. Each row
records source-build and output digests plus the parsed nonzero Rust test count.
The Stop/resource selector proves the watcher clears pending work and releases
its fixture; the interpreter receipt proves the JSONL Stop acknowledgement and
child exit. This is a local, serial test observation. It does not establish
OS-wide resource telemetry, source-Agent process identity, native/Wasm swap,
or production support.

The command is documented for reproducible local execution only. No committed
report currently says it has been executed on macOS.

The roll-up now executes exact existing watcher, prepared-worker, and
source-Agent regressions for source races, stale plans, path escape hints,
manifest reauthentication, event overflow, inventory bounds, safe-point
activation, duplicate activation, post-pivot acknowledgement loss, journal
acknowledgement loss, and migration claim/reservation faults. Every selector
must report exactly one passed test, and each report row binds its executable
digest and source-build record. The report includes a platform/lane table:
macOS lanes are measured by this runner; Linux and Windows remain unavailable
from this macOS-only command.

The macOS roll-up also runs three exact VS Code adapter regressions for an
oversized response, unexpected child exit, and Stop during an unacknowledged
activation. These use a scripted child and establish editor protocol handling,
not a physical CLI process-death or core shutdown journey. A hot-reload-specific
unknown effect outcome is exercised by the source-Agent journal fault selector:
the deterministic effect adapter runs once, its `effect_observed` acknowledgement
is lost before journal write, the retained tail remains an unresolved intent,
and successor C is refused without another provider or effect dispatch. A
successful run also does not establish native/Wasm swapping.
