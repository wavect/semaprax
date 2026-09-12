# Cross-language Agent benchmark laboratory v1

Status: laboratory shipped (harness, tasks, adapters, provenance,
comparison/regression logic); no timing measurement taken or committed.

Audience: compiler contributors and anyone evaluating SEMAPRAX against
another language's Agent-generation or maintenance outcomes.

Issue #211 asks for a reproducible cross-language Agent benchmark laboratory
comparing SEMAPRAX against Zero, NTNT, Aver, Vera, Hale, MoonBit, Rust, and
TypeScript, with frozen tasks, recorded provenance, hidden tests, and
regression scoring — driven by measured outcomes rather than intuition, and
explicitly not one synthetic leaderboard score. This page is the short
pointer; the suite itself lives in
[`benchmarks/cross-language-v1/`](../benchmarks/cross-language-v1/) and its
full contract in
[`benchmarks/cross-language-v1/docs/METHODOLOGY.md`](../benchmarks/cross-language-v1/docs/METHODOLOGY.md).

## What is built

- **A task equivalence contract.** Every task specifies its input space,
  output shape, the measured region's boundary, and what each language's
  official toolchain invocation and success signal actually are — so a
  cross-language comparison can be checked by a reader, not taken on faith.
  See `tasks/sequence-digest-v1/EQUIVALENCE.md` in the suite for a worked
  example, including a documented case where SEMAPRAX's `run` command and
  Rust's/TypeScript's test runners signal pass/fail through genuinely
  different conventions (stdout value vs. process exit code), and the
  harness's adapter model reads each one honestly instead of forcing a
  shared convention onto all three.
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
- **One real pilot task, three real languages.** `sequence-digest-v1` is
  wired and passing end to end for SEMAPRAX, Rust, and bare-`rustc`
  TypeScript-via-`tsc`+`node` — not mocked. The other six languages on the
  issue's roster are declared in `adapters.json` with an honest
  `blocked_reason` (no located official toolchain, or a toolchain that
  needs a network install this sandbox forbids) and are reported `blocked`,
  never scored as a pass or a fail.
- **Deterministic harness self-tests.** `tests/documentation/cross_language_benchmark_suite.rs`
  exercises the harness against synthetic mock adapters with known
  pass/fail artifacts, independent of any real language toolchain, alongside
  cases for the committed inventory, the leak check, digest drift, and the
  invariant that no timing field ever appears in the result schema.

## What is explicitly not done here, and why

**No timing was measured, and none is committed.** This laboratory was built
on a host running many concurrent build/test lanes at once; any wall-clock
number captured under that contention would be false evidence, not
indicative evidence. The result schema (`benchmark.cross_language.v1`) has
no field to receive one by accident. Issues #85, #130, and #131 own adding a
timing metric once an exclusive quiet host is available — see
`docs/METHODOLOGY.md`'s "What a future quiet-host run must do" section in
the suite for the exact steps.

Also not attempted, and recorded as such rather than silently assumed:
containerized/pinned per-language toolchain images (needs infrastructure and
a network-access decision outside a bounded worker's authority), and a live
multi-model Agent pilot run across all nine languages (needs model API
budget and a publication decision). Both are named `HUMAN_BLOCKED` in the
suite's methodology doc at the exact point they are needed.

## Non-claims

This is local, single-host functional evidence for whichever toolchain
versions happen to be installed on the run host — not a hosted, pinned, or
production benchmarking claim, and not a ranking of SEMAPRAX against any
other language.
