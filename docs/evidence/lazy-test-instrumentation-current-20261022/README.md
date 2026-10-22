# Current instrumentation source custody

The CI diagnostic preflight had three source-audit errors after allocator backing
moved and forecast instrumentation acquired module/step attribution. The audit
now authenticates the whole current root and the allocator's explicit shared
fragment, then restores the exact published lazy-v1 source before applying every
original inverse. All four original archives, SHA pins and inverse lists remain
unchanged. Every original Python case and old callback/window assertion remains.

Additional controls compare the actual current allocator callbacks and complete
observation windows directly to their originals, and independently reject root,
backing and foreign-fragment drift.

`callback-captures.json` is local private component evidence from Rust 1.97.1,
not hosted or complete-compiler qualification. Five corpora execute the complete
real old and current forecast modules: regular records, ring truncation, oversized
paths, overflow sentinels and incomplete passes. All original component/authority
values, the four original global values, identities, modes, paths, counters and
ring order agree. Two formerly reserved global slots now contain the winner
sentinel and retained-wins flag. The current reporter intentionally withholds
incomplete legacy totals; the audit requires that stronger behavior.

The two suffixes only read real private state after each operation; no producer
body or type is replaced. The portable driver and replay script reconstruct the
exact old module from the unchanged original inverse table and authenticate both
whole producer prefixes against the recorded source hashes.

Local gate: all 12 original Python cases and three new regressions pass (15 total).

To replay on a device with sufficient free RAM, use Rust 1.97.1 and a matching
built `serde_json` dependency. Supply an explicit scratch directory on the external
SSD; no Cargo source, shared target or network access is needed:

```sh
python3 docs/evidence/lazy-test-instrumentation-current-20261022/replay-captures.py \
  --rustc /absolute/path/to/rustc \
  --serde-json /absolute/path/to/libserde_json.rlib \
  --deps /absolute/path/to/deps \
  --scratch /Volumes/Semaprax/tmp/instrumentation-callback-replay
```
