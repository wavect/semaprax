# Hot reload benchmark v1

Status: local interpreter measurement contract for HR-07.

Audience: development-loop implementers and performance reviewers.

The shared [acceptance manifest](../benchmarks/hot-reload-v1/manifest.json)
binds exact A/B fixture bytes, the three compared loops, expected A and B
results, required per-loop sample counts, and the run-time compiler subject.
The generated report records the exact Git commit and executable SHA-256, so a
result binds one compiler build without pretending that a committed benchmark
plan can name a future checkout's commit.

Run the checked interpreter loop with an already-built binary:

```sh
python3 benchmarks/hot-reload-v1/run.py --semaprax target/debug/semaprax \
  --samples 11 --warmups 3 --expected-commit "$(git rev-parse HEAD)" --output /tmp/hot-reload-benchmark.json
```

The interpreter sequence starts A, invokes A, saves B, admits and plans B,
acknowledges B activation, invokes B, and stops. It asserts A returns 42 and B
returns 48 through the real JSONL `dev` control stream. Its process remains
alive from A through B. Full restart uses a fresh process after the same B save.
Authenticated warm restart uses the persistent semantic-cache refresh and warm
open commands on that same fixture; it is a separate process restart with
checked-HIR reuse.

The result reports the discarded warmup count, raw values, median and p95 for each loop. It makes no
universal latency claim. It records zero debounce and safe-point wait only for
this synchronous fixture, records plan as the protocol-visible combined
admission/preparation interval, and leaves unavailable per-child peak memory
and separately observable admission timing as `null`. The report never fills
those fields with estimates.

The source-Agent lane is listed as `migration-required`; this benchmark does
not claim an Agent handoff, migration, accounting preservation, provider work,
native/Wasm code swap, or production rollout. See
[Hot Reload Watcher v1](HOT-RELOAD-WATCHER-V1.md) for the bounded control
adapter and [Persistent Semantic Cache v1](PERSISTENT-SEMANTIC-CACHE-V1.md)
for the authenticated restart boundary.

## Cross-layer selector record

The additive [cross-layer manifest](../benchmarks/hot-reload-v1/cross-layer-manifest.json)
does not expand the interpreter benchmark's scope. It names the existing
watcher selectors for B→invalid-C retention and invalid-C→repair, the
source-Agent child selector for A→B journal migration, the retained A→B→C
migration/replay selector, and the prepared-interpreter A→B→C
worker-continuity selector.
These selectors prove different boundaries and must remain separately named in
the record.

`cross_layer.py` runs only selector commands explicitly supplied by the
operator, so it does not trigger Cargo or build a compiler. For each supplied
command it records actual wall-clock samples plus stdout/stderr digests. A
command that is omitted has `unavailable` status; a nonzero command aborts the
run without a report. This prevents a plan or prior test result from becoming a
timing measurement.

Two cells intentionally remain unavailable: the source-Agent JSONL protocol
does not emit an OS process identity that can be compared through activation,
and no native or Wasm source-Agent state replacement implementation exists to
time. The prepared worker's `ThreadId` continuity assertion is an opaque local
library observation. It is not process identity, transport evidence, or a
native-target claim.

## macOS source-attributed roll-up

`macos-cross-layer-evidence.sh` is the macOS-only roll-up command for the
interpreter receipt and every supported cross-layer selector. It builds the
current checkout into a caller-selected private directory below that checkout's
`target/`, with one Cargo job at a time. It then runs the interpreter capture
and each exact test selector serially. The report binds the checkout commit,
`SEMAPRAX_BUILD_COMMIT`, CLI and test-executable digests, Cargo output digests,
selector stdout/stderr digests, and the nonzero exact test counts.

Run it only when a local Cargo slot is available:

```sh
benchmarks/hot-reload-v1/macos-cross-layer-evidence.sh \
  --target-dir "$PWD/target/hr07-macos-evidence" \
  --output /tmp/hot-reload-macos-cross-layer-evidence.json
```

The Stop/resource row runs the existing watcher regression that requests Stop
during admission, clears pending work, retains the active revision, drops the
watcher, and removes its fixture directory. The interpreter rows require the
JSONL `stopped` acknowledgement and clean child exit. These are local test and
protocol observations, not OS-wide process, thread, memory, or production
telemetry.

This command is a reproducible execution path, not committed macOS evidence:
no report checked into this repository claims that it has run. The two
native/Wasm cells remain `unavailable`; a passing local prepared-worker identity
test does not promote either cell to a physical platform claim.
