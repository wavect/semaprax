# Cross-language benchmark methodology (v1)

This is the general contract every task, adapter, and comparison in
`benchmarks/cross-language-v1/` follows. A task's own `EQUIVALENCE.md` owns
the specific choices for that one task; this file owns the rules that apply
across all of them.

## Why a laboratory, not a leaderboard

Issue #211 asks for empirical evidence to support (or narrow) SEMAPRAX's
architectural claims against other agent-native and conventional languages.
Issue #85 already found methodology defects in the existing performance
suite before this one existed; the fix there — and the rule this suite
follows — is: **never publish a number without the exact conditions that
produced it**, and **never let a failure be scored as an improvement**. A
cross-language comparison adds one more failure mode on top of that: an
unstated difference in what the two languages were actually asked to do.
That is what "equivalence" below exists to close off.

## Equivalence contract (what every task must specify)

Every task's `EQUIVALENCE.md` states, in prose a future reader can check
without trusting the author:

- **Inputs**: exact type, shape, and range, and whether they are literal
  fixtures, generated, or externally supplied. An unstated input space is how
  one language's implementation ends up handling a case the other's never
  sees.
- **Outputs**: exact type and shape, and how success is judged (an exact
  equality, a tolerance, a property).
- **Boundary of the measured region**: what is inside the comparison
  (typically: the pure logic under test, invoked by the language's own test
  runner) and what is outside it (typically: process startup, compiler
  invocation, test-harness overhead). Two languages compared on unequal
  boundaries — one measured warm-process, one measured cold-start — are not
  comparable even if every other input matches.
- **Allowed optimizations**: what idiomatic freedom each language keeps
  (control-flow shape, standard-library use) and what would substitute a
  different problem (delegating the actual algorithm to a library call that
  does the work the task exists to measure).
- **Official toolchain and success signal per language**: the exact,
  version-recorded invocation `adapters.json` declares, and how that
  language's own convention signals pass/fail — not a convention imposed on
  it. `sequence-digest-v1/EQUIVALENCE.md` is a worked example: SEMAPRAX's
  `run` reports outcome through printed stdout, not process exit status,
  because its exit code reports whether *interpretation completed*, not
  whether the program's own assertions passed; Rust's and TypeScript's
  official invocations use exit status directly. The harness's per-adapter
  `success` predicate (`adapters.json`) reads each one honestly instead of
  forcing a shared convention that would misrepresent one of them.

An unspecified equivalence is how a benchmark quietly becomes marketing —
issue #211 calls this out directly ("Explicitly out of scope: ... Comparing
projects on features they explicitly do not claim without labeling the
mismatch"). Every task in this suite must be checkable against this
contract by a reader who was not the one who wrote it.

## Provenance binding

Every scored task/language pair records, in the result document
(`benchmark.cross_language.v1`):

- **`provenance.public_digest`** / **`hidden_digest`** / **`digest`**: a
  `sha256:`-prefixed digest over every file's relative path and bytes in the
  public directory, the hidden overlay, and their combination
  (`run.py::digest_tree`). Editing, adding, or removing one file changes the
  identity; reordering files during traversal does not, because the digest
  sorts by relative path before hashing.
- **`provenance.adapter_version`**: the first line of the adapter's declared
  `version_command` output, observed on the run host — never a literal
  string in the adapter manifest, so a run cannot claim a toolchain version
  it did not actually invoke.
- **`revision`**: the exact commit the repository was at when the run
  happened, and whether the working tree was dirty (`git_revision`).
- **`host`**: platform, OS release, logical CPU count, and the Python
  interpreter version the harness itself ran under. Deliberately excludes
  `load_average` and any other timing-adjacent signal in this schema
  version — see "No timing" below.

An `expected_digest` may be declared per task/language pair in `tasks.json`.
If the run host's digest does not match it, the pair's status is
`drifted`: no build or test step runs at all, and the pair contributes no
comparable pass/fail either. This mirrors `benchmarks/performance-v1`'s rule
that a digest mismatch fails closed before any measurement, not after.

## Scoring and hidden-test isolation

Each task/language pair runs in two phases, each in its own scratch
directory:

1. **Public phase**: only the task's `public/<language>/` files are copied
   into a scratch directory, then the adapter's declared build step (if any)
   and run step execute there. This is exactly what a solver working the
   public task would see and be scored against for the "does it build and
   pass its own tests" signal.
2. **Hidden phase**: a *second*, separate scratch directory receives the
   public files first, then the task's `hidden/<language>/` files are
   overlaid on top (a file at the same relative path replaces the public
   one; a new relative path is added). The same build/run steps execute
   there.

The harness asserts a **leak check** after the public phase: the public
scratch directory's file set must share no hidden-only path with the task's
`hidden/` directory. A hidden test module is only ever assembled into a tree
the public build step never sees, which is the actual property "hidden
tests" is supposed to guarantee — not merely that the file lives in a
differently-named directory in the repository.

A pair's overall `status` is:

- **`ok`**: build (if any) succeeded, the public run step's success
  predicate was satisfied, the leak check found nothing, and the hidden run
  step's success predicate was also satisfied.
- **`failed`**: any of the above did not hold. The `reason` field names
  which phase (`build`/`run`, public or hidden) and the adapter's own
  failure detail.
- **`blocked`**: the adapter is declared in `adapters.json` with
  `"implemented": false`, or the task declares no implementation for that
  language. Never counted as a pass or a fail; a blocked pair is not "the
  language failed," it is "this snapshot never asked the language."
- **`drifted`**: the pair's provenance digest did not match a declared
  `expected_digest`. No comparable outcome; see above.

## Comparison and regression logic

`--compare <baseline.json>` scores a completed run against a prior one:

- A pair present in the local run but absent from the baseline is reported
  `no baseline`, not silently skipped.
- A pair whose local or baseline status is anything other than `ok` is
  reported `incomparable` with both statuses named. A regression can never
  be measured against, or hidden behind, a failure — the same rule
  `benchmarks/performance-v1` enforces for wall-clock scenarios, applied
  here to pass/fail outcomes instead.
- Both `ok`: `unchanged (both ok)`. There is currently no scored numeric
  axis beyond pass/fail (see "No timing" below), so a regression in this
  schema version means "a pair that used to build and pass no longer does,"
  which is itself a real and useful signal — it catches toolchain drift and
  fixture drift, independent of any timing claim.

## No timing (and why this is not a gap being papered over)

`benchmark.cross_language.v1` has **no wall-clock field at all** in this
version, and `run.py` never calls a timer. This is a deliberate schema
choice, not an oversight to be quietly worked around:

- This benchmark laboratory was built on a host running roughly eight to ten
  concurrent build/test lanes at once. Any wall-clock number captured here
  would measure host contention, not the language, the compiler, or the
  runtime being compared. A committed number under those conditions would
  read as evidence later and would be false.
- Issues **#85**, **#130**, and **#131** are the measurement-side work,
  deliberately held for an exclusive quiet host. This suite's job is to make
  sure the laboratory — tasks, adapters, provenance, scoring, comparison —
  is ready the moment a quiet host is available, not to pre-empt that work
  with contended numbers.
- Every other piece of "reproducible cross-language benchmarking" this
  issue asks for — frozen task equivalence, pinned/observed toolchain
  identity, hidden-test isolation, fail-closed drift detection, pass/fail
  regression scoring — does not need wall-clock time and is fully built and
  exercised here.

### What a future quiet-host run must do to produce real numbers

1. Confirm the host is exclusive (no concurrent build/test lane; issues
   #130/#131 own defining "quiet" precisely and the acceptance check for it).
2. Add a `wall_ms`-shaped field to `benchmark.cross_language.v2` (a new
   schema version, not a silent extension of v1) with the same discipline
   `benchmarks/performance-v1` already uses: an untimed verification run
   before any timed sample, N repetitions, only a *successful* run
   publishes a comparable sample, and the host's load average recorded
   beside the sample so an idle claim is auditable rather than declared.
3. Re-run `run.py` (or its v2 successor) on that quiet host and commit the
   result under `results/` with the observed host, revision, and dirty flag
   — never on a modified working tree, mirroring
   `benchmarks/performance-v1/results/baseline.json`'s existing rule.
4. Only then does a comparison across languages carry a timing claim, and
   only for the exact toolchain versions and task equivalence recorded
   beside it.

## What the harness cannot yet check

Recorded honestly rather than silently assumed:

- **Delegated-algorithm detection**: `EQUIVALENCE.md`'s "no substituting a
  stdlib fold for the algorithm" rule is enforced by author discipline and
  code review, not mechanically. A future task with a less trivially
  reviewable algorithm should add a structural check (e.g., a source-pattern
  scan) rather than rely on this alone.
- **Containerized/pinned toolchains**: `provenance.adapter_version` records
  whatever version is installed on the run host; it does not pin one.
  Building and provisioning pinned, network-free per-language toolchain
  images (issue #211's "Containerized/pinned environments") needs
  infrastructure and a network-access decision this repository's
  invariants forbid making unilaterally — recorded as
  `HUMAN_BLOCKED: container image provisioning` rather than attempted here.
- **A live Agent pilot run**: issue #211 also asks for "at least one pilot
  task run across all initial languages and two models." That needs a model
  API budget, credentials, and a publication decision, none of which a
  bounded implementation worker holds — recorded as
  `HUMAN_BLOCKED: model budget and credentials for a live Agent pilot`.
  What is built here is the harness that pilot would run through, exercised
  end to end with three real, non-mocked languages
  (`sequence-digest-v1::semaprax`, `::rust`, `::typescript`) and with
  deterministic mock adapters in this suite's own test module
  (`tests/documentation/cross_language_benchmark_suite.rs`) standing in for
  the six languages with no available toolchain in this sandbox.
