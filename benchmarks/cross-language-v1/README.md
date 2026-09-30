# Cross-language benchmark laboratory (v1)

This suite is the **laboratory**, not a result: the harness, the task and
adapter inventories, the provenance binding, and the comparison/regression
logic that issue #211 asks for.

**What is, and is not, "Agent" here.** `run.py` (this directory's original
harness) scores a fixed, human-written source tree through installed
toolchain commands; that alone does not independently authenticate their origin. It has no model, no provider, no sampling parameters, and
no budget anywhere in it — it is a cross-language *toolchain-conformance and
scoring* harness, not something that has ever run a model. `agent/` (added
alongside it) is the seam an Agent-driven run would go through: an explicit
solver-request/response contract, a deterministic offline replay transport
that exercises that whole path end to end with no credentials and no
network, and a declared-but-inert live-provider transport that has never
been executed against a real endpoint in this repository. **No result in
this directory or in `results/` was ever produced by a real model.** See
[`agent/README.md`](agent/README.md) for that seam's design and non-claims,
and [Non-claims](#non-claims) below for the rest (timing, single-host
evidence, and the six unimplemented languages). See
[`docs/METHODOLOGY.md`](docs/METHODOLOGY.md) for the full equivalence
contract, provenance model, and what a future quiet-host or credentialed run
must do to produce real numbers.

## Current official comparison support

The supported independently runnable set for issue #322 is **TypeScript only**:
official Node.js 22.12.0 Darwin arm64 plus TypeScript 5.8.3, on the fixed macOS
26.5.1 / 25F80 v3 host profile. All other 13 adapters are explicitly **not
supported for independent official comparisons**, even where local-fixture
execution exists. The retained corrected TypeScript 54/54 result belongs to
its original revision/host, not every subsequent checkout. See the
[complete supported-set decision](../../docs/CROSS-LANGUAGE-RUNNABLE-ADAPTER-V3.md#9-supported-runnable-set-issue-322).
The 13-task x 14-adapter denominator remains **182 rows**, including all 169
not-supported slots and their reasons. This scope does not withdraw compiler
features or remove legacy fixture workflows, and it makes no model claim.

## Layout

| Path | Role |
| --- | --- |
| `run.py` | The toolchain-conformance harness: resolves tasks/adapters, builds and tests each task/language pair against a fixed, human-written source tree, records provenance, scores comparisons. No model involved. |
| `agent/` | The agent-driver seam: explicit model/sampling/budget contracts, a deterministic offline replay transport, a declared-but-inert live transport, and an orchestrator that scores a transport-produced candidate through `run.py`'s own build/test/leak-check/provenance machinery. It retains complete, digest-bound transcript/candidate evidence and supports only explicit literal-redaction projections; candidate paths must exactly match their declaration. See `agent/README.md`. |
| `reproduction_capsule.py` | An offline, input-only capsule builder/verifier. It binds the exact supplied task and adapter inventory bytes plus every declared public tree, hidden tree, equivalence contract, and adapter row. It invokes no toolchain or model; an `inputs_match` verification is explicitly not a benchmark result. It requires descriptor-relative no-follow traversal and returns unavailable rather than falling back to pathname traversal on hosts without it. |
| `supported_scope.py` | Read-only supported-set projection using the pinned v3 source/correction gates: all 182 rows, explicit exclusions, no runtime probe or execution. |
| `tasks.json` | Task inventory (schema `benchmark.cross_language.tasks.v1`). Each task declares a `split` — `development` (the frozen original pilot) or `held_out` (issue #106's contamination-protected extension; see that task's `EQUIVALENCE.md`) — and both its pre-existing five-value `category` and an additive `issue_211_category` naming which of issue #211's eleven task categories it demonstrates; see `docs/METHODOLOGY.md`'s "Taxonomy mapping" section for the full table and reasoning, including why `category` itself is never rewritten |
| `adapters.json` | Per-language adapter inventory (schema `benchmark.cross_language.adapters.v1`): official toolchain invocation, version probe, success signal |
| `tasks/<task-id>/EQUIVALENCE.md` | That task's fairness contract: inputs, outputs, measured boundary, allowed optimizations |
| `tasks/<task-id>/public/<language>/` | The source tree a solver (human or Agent) would author against |
| `tasks/<task-id>/hidden/<language>/` | Files that overlay (same relative path replaces, new paths add) the public tree for scoring only; never copied into the public build step |
| `results/` | Where a real run's output JSON goes; empty in this commit (see Non-claims) |

## Quick start

```sh
# Complete official support inventory; no runtime probe, execution or score.
python3 benchmarks/cross-language-v1/supported_scope.py

# Plan only: resolve every task/language pair, run nothing.
python3 benchmarks/cross-language-v1/run.py --dry-run --output /tmp/plan.json

# Legacy local-fixture workflow, NOT independent official-toolchain admission.
# Score implemented adapters against the committed tasks.
python3 benchmarks/cross-language-v1/run.py \
  --semaprax target/debug/semaprax \
  --output /tmp/result.json

# Restrict to one task or one language.
python3 benchmarks/cross-language-v1/run.py --semaprax target/debug/semaprax \
  --only sequence-digest-v1 --language rust --output /tmp/rust-only.json

# Compare a run against a prior recorded result (pass/fail regression only;
# there is no timing field to compare in this schema version).
python3 benchmarks/cross-language-v1/run.py --semaprax target/debug/semaprax \
  --output /tmp/local.json --compare benchmarks/cross-language-v1/results/prior.json

# Bind the declared scoring inputs without invoking an adapter, toolchain, or model.
python3 benchmarks/cross-language-v1/reproduction_capsule.py create \
  --root "$PWD" \
  --tasks benchmarks/cross-language-v1/tasks.json \
  --adapters benchmarks/cross-language-v1/adapters.json \
  --output /tmp/cross-language-inputs.json

# On another checkout, verify that exactly those inputs still match. This does
# not run the benchmark; `inputs_match` means only that the scoring inputs match.
python3 benchmarks/cross-language-v1/reproduction_capsule.py verify \
  --root "$PWD" \
  --tasks benchmarks/cross-language-v1/tasks.json \
  --adapters benchmarks/cross-language-v1/adapters.json \
  --output /tmp/cross-language-inputs.json
```

## Languages

`adapters.json` retains 14 adapters: `semaprax`, `semaprax-project`, `rust`,
`typescript`, `c`, `python`, `swift`, `java`, `zero`, `ntnt`, `aver`, `vera`,
`hale`, and `moonbit`. Eight have `implemented: true`; the final six retain
their original `blocked_reason`. These flags describe the frozen fixture
inventory, not official admission. Missing task ports likewise remain visible
as `declared: false` slots rather than disappearing from the denominator.

The v1 Rust and v2 C/Python/Swift/Java/TypeScript paths remain local fixtures.
Only the separately authenticated, corrected v3 TypeScript route is supported
for independent official conformance. The other 13 adapters are explicitly
excluded from that set; a future admission needs its own official artifact
provenance, source/equivalence review and actual bounded scorer/hostile controls.
Neither a fixture pass nor a scope-inventory report satisfies those gates.

## Non-claims

- **No Agent-driven result in this directory was ever produced by a real
  model.** `agent/`'s replay transport is a deterministic, hand-authored
  script standing in for a model response, the same way
  `tests/documentation/cross_language_benchmark_suite.rs`'s `MockLanguage`
  stands in for a real language toolchain to pin the harness's own logic —
  never a recorded real-model transcript. `agent/`'s live transport is
  declared and has never been executed against a real endpoint: it refuses
  to construct without explicit credentials (never an environment-variable
  fallback) and refuses to run even when credentials are supplied, because
  this repository has never verified its wire mapping against a real
  response. A live, credentialed, two-model pilot stays
  `HUMAN_BLOCKED: model budget and credentials`, exactly as before — this
  seam makes that pilot's eventual code path testable today, it does not
  perform it. See `agent/README.md`.
- **No timing was measured to produce this suite, and none is committed.**
  This host runs many concurrent build lanes at once; a wall-clock number
  measured here would record contention, not the compiler or the language
  runtime. The result schema (`benchmark.cross_language.v1`) has no field to
  receive one by accident — see `docs/METHODOLOGY.md`. Issues #85, #130, and
  #131 own adding a timing metric once an exclusive quiet host is available.
- **A reproduction capsule is not a run receipt, environment lock, or result.**
  It is a deterministic fingerprint of the scoring inputs supplied to it. It
  deliberately has neither a model/provider interface nor an adapter/toolchain
  invocation path, and it records `execution: not_attempted` even when all
  inputs match. Existing agent replay and result/provenance documents retain
  their separate schemas; this capsule does not reinterpret them.
- The original pilot task (`sequence-digest-v1`, `split: development`) is
  real, small, and deliberately narrow (see its `EQUIVALENCE.md`). It is
  evidence that the harness works end to end for three real languages, not a
  claim that SEMAPRAX outperforms or underperforms Rust or TypeScript at
  anything. It stays frozen; issue #106's held-out extension
  (`bounded-counter-repair-v1`, `split: held_out`) is added alongside it, not
  in place of it — see `tasks/bounded-counter-repair-v1/EQUIVALENCE.md`'s
  "Held-out discipline" section for what that split declaration commits to.
  `concurrent-delta-merge-v1` is a further held-out addition purpose-built
  for issue #211's "concurrent change" category (`docs/METHODOLOGY.md`'s
  "Taxonomy mapping" section); it is evidence the harness's leak check and
  provenance binding hold for a newly-authored task, not a performance claim
  either.
- Legacy `run.py` and v1/v2 local-fixture results are not independently
  authenticated official-toolchain evidence. The separately pinned v3
  TypeScript profile has the retained local evidence described above; it does
  not promote the other adapters or another host. `supported_scope.py` emits
  `execution: not_attempted` and `runtime_availability: not_probed`, not a new
  conformance receipt. Quiet-host timing and model comparisons remain separate
  (see `docs/METHODOLOGY.md`).
