# Cross-language Agent benchmark laboratory v1

Status: laboratory shipped (harness, tasks, adapters, provenance,
comparison/regression logic); no timing measurement taken or committed.

Audience: compiler contributors and anyone evaluating SEMAPRAX against
another language's Agent-generation or maintenance outcomes.

Issue #211 asks for a reproducible cross-language Agent benchmark laboratory
comparing SEMAPRAX against Zero, NTNT, Aver, Vera, Hale, MoonBit, Rust, and
TypeScript, with frozen tasks, recorded provenance, hidden tests, and
regression scoring — driven by measured outcomes rather than intuition, and
explicitly not one synthetic leaderboard score. This page is a short pointer;
the suite lives in
[`benchmarks/cross-language-v1/`](../benchmarks/cross-language-v1/) and its
full contract in
[`benchmarks/cross-language-v1/docs/METHODOLOGY.md`](../benchmarks/cross-language-v1/docs/METHODOLOGY.md).

## What is built

- **A task equivalence contract.** Each task fixes input, output, measured
  boundary, official invocation, and success signal so comparisons are
  inspectable rather than asserted. The sequence-digest example shows why the
  adapter preserves SEMAPRAX stdout and Rust/TypeScript exit-code conventions
  instead of inventing one shared signal.
- **Provenance binding.** Every scored task/language pair records a
  `sha256:` digest over its public and hidden source trees, the toolchain
  version actually observed on the run host, and the exact repository
  revision — mirroring `benchmarks/performance-v1`'s existing rule that a
  digest mismatch fails closed, before any measurement, not after.
- **Hidden-test isolation with an enforced leak check.** A task's hidden
  files are only ever overlaid into a second scratch directory, assembled
  after the public build step already ran in a separate one; the harness
  asserts no hidden-only path ever reached the public scratch tree.
- **Pass/fail comparison and regression logic.** `--compare` scores a run
  against a prior one: a pair that regresses from `ok` to `failed` is
  reported, a pair whose baseline or local status is not `ok` is reported
  `incomparable` rather than silently scored, exactly as
  `benchmarks/performance-v1` already does for wall-clock scenarios.
- **One real pilot task, three real languages, plus a held-out extension
  (issue #106).** `sequence-digest-v1` (`split: development`) is wired and
  passing end to end for SEMAPRAX, Rust, and bare-`rustc`
  TypeScript-via-`tsc`+`node` — not mocked; it stays frozen. Alongside it,
  `bounded-counter-repair-v1` (`split: held_out`, `category: repair`) adds a
  realistic off-by-one repair task beyond signature migration/greenfield
  work — a saturating counter whose classic "clamp only the final summed
  delta" bug is invisible to its public tests and caught only by its hidden
  overlay — also wired and passing end to end for all three languages. Every
  task now declares an explicit `split`; `tasks/<task-id>/EQUIVALENCE.md`'s
  "Held-out discipline" section states what that declaration commits a
  held-out task to. The other six languages on the issue's roster are
  declared in `adapters.json` with an honest `blocked_reason` (no located
  official toolchain, or a toolchain that needs a network install this
  sandbox forbids) and are reported `blocked`, never scored as a pass or a
  fail; the reserved Zero lane's reason now names the exact pinned revision
  `benchmarks/agent-task-comparison-v1/manifest.json` already reserves
  (`vercel-labs/zerolang@eb2ed6c2...`), rather than restating "not located"
  for a toolchain this repository has, in fact, already identified and
  pinned but cannot fetch without build-time network access.
- **Deterministic harness self-tests.** `tests/documentation/cross_language_benchmark_suite.rs`
  exercises the harness against synthetic mock adapters with known
  pass/fail artifacts, independent of any real language toolchain, alongside
  cases for the committed inventory, the leak check, digest drift, and the
  invariant that no timing field ever appears in the result schema.

## Task category coverage (issue #211's eleven categories)

Issue #211 names eleven task categories the corpus should cover. Every task
carries an additive `issue_211_category` field in `tasks.json` naming which
one it demonstrates (its own five-value `category` field predates that exact
wording and stays unchanged — see `docs/METHODOLOGY.md`'s "Taxonomy mapping"
section for the full reasoning per task):

| Category (issue #211) | Task(s) |
| --- | --- |
| Greenfield | `sequence-digest-v1` |
| Feature change | `clean-install-calculator-v1` |
| Cross-file refactor | `module-import-refactor-v1` |
| API evolution | `structured-input-error-handling-v1` |
| Security fix | `cold-chain-release-gate-v1` |
| Ownership change | `owned-byte-sentinel-balance-v1` |
| Requirement preservation | `bounded-counter-repair-v1`, `booking-window-conflict-v1` |
| Agent workflow | `iterative-repair-workflow-v1` |
| Concurrent change | `stable-dispatch-order-v1`, `concurrent-delta-merge-v1` |
| Failure recovery | `telemetry-overflow-diagnosis-v1` |
| Context-limited maintenance | `stale-edit-preservation-v1` |

All eleven categories are now covered by task content. "Agent workflow" was
previously demonstrated only orthogonally, by running
`structured-input-error-handling-v1` through `agent/orchestrator.py`'s full
solver path instead of through a dedicated fixture; that resolution was put
to a maintainer for sign-off and, on 2026-09-27, was decided against —
orthogonal coverage is not accepted, so this corpus gained a thirteenth,
content-level task built specifically to exercise multi-step, iterative
agent work: `iterative-repair-workflow-v1` has two sequentially-masked
defects (its own public tests reveal only the first at a time), hidden
tests that check both the fully corrected behavior and preservation of an
unrelated sibling function, and a task statement that requires reading an
earlier step's redacted output to find the second defect at all. See
`docs/METHODOLOGY.md`'s "Decision: 'Agent workflow' needed a dedicated,
content-level task" section for the full record; `agent/orchestrator.py`'s
orthogonal demonstration through `structured-input-error-handling-v1`
remains in this corpus as additional coverage, not the sole answer.

## Environment pinning

`docs/METHODOLOGY.md`'s "Environment pinning" section records, for every
adapter `runnable_adapter.py`/`runnable_adapter_v2.py` actually admits
(`rust`, `c`, `python`, `swift`, `java`, `typescript`), the exact path,
version, and SHA-256 digest this repository observed on one dated local
host — and, for the six reserved external languages (Zero, NTNT, Aver, Vera,
Hale, MoonBit), an explicit availability/equivalence decision table: official
toolchain source, version to pin, digest mechanism, and why each is blocked
without a network fetch this sandbox forbids. None of this is a pin — it is
a dated observation recorded so a future pin starts from a concrete state
rather than nothing.

## What is explicitly not done here, and why

**No timing was measured or committed.** This laboratory was built on a host
with concurrent build/test lanes, so a wall-clock value would be false evidence.
`benchmark.cross_language.v1` has no timing field. Issues #85, #130, and #131
own a metric once an exclusive quiet host exists — see
`docs/METHODOLOGY.md`'s "What a future quiet-host run must do" section in
the suite for the exact steps.

Also not attempted, and recorded as such rather than silently assumed:
containerized/pinned per-language toolchain images (needs infrastructure and
a network-access decision outside a bounded worker's authority), and a live
two-model Agent pilot plus its second-host reproduction (needs two named
model identities, a credential owner, an authorized spend cap, a publication
decision, and an independently provisioned second host — none of which a
bounded worker holds or may supply). Both stay open, itemized as an exact
operator checklist rather than a vague blocker, in `docs/METHODOLOGY.md`'s
"What the harness cannot yet check" section at the `HUMAN_BLOCKED` entries
naming exactly where they are needed.

## Non-claims

This is local, single-host functional evidence for whichever toolchain
versions happen to be installed on the run host — not a hosted, pinned, or
production benchmarking claim, and not a ranking of SEMAPRAX against any
other language.
