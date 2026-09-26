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

## Taxonomy mapping: issue #211's eleven task categories

Issue #211 names eleven task categories the corpus should cover: greenfield,
feature change, cross-file refactor, API evolution, security fix, ownership
change, requirement preservation, Agent workflow, concurrent change, failure
recovery, and context-limited maintenance. This corpus's `tasks.json` grew its
own five-value `category` field (`greenfield`, `repair`, `validation`,
`diagnosis`, `onboarding`) before that exact eleven-item wording existed, and
`category` is read by `run.py`'s and `agent/orchestrator.py`'s own result
records (`tests/documentation/cross_language_benchmark_suite.rs` pins
`bounded-counter-repair-v1`'s `category` to the literal string `"repair"`),
so it stays exactly as committed rather than being rewritten to chase a
label change.

Each task instead carries an additive `issue_211_category` field naming
which of the eleven categories it demonstrates, decided by what the task's
own `EQUIVALENCE.md` actually measures rather than by relabeling in place:

| Task | `category` | `issue_211_category` | Why |
| --- | --- | --- | --- |
| `sequence-digest-v1` | greenfield | **greenfield** | New pure computation, no existing code to change. |
| `module-import-refactor-v1` | greenfield | **cross-file refactor** | The measured skill is importing a helper from a separate module, not the arithmetic itself. |
| `owned-byte-sentinel-balance-v1` | greenfield | **ownership change** | Consumes an owned buffer; the ownership axis is SEMAPRAX-specific and has no analogue in the other five categories. |
| `stable-dispatch-order-v1` | greenfield | **concurrent change** | Models simultaneously-arriving jobs that must be ordered deterministically, preserving arrival order under equal priority — the same "several updates land at once, order must still be well-defined" property `concurrent-delta-merge-v1` (below) measures with arithmetic instead of ordering. |
| `concurrent-delta-merge-v1` | greenfield | **concurrent change** | Purpose-built for this category (see below): merges two independently-arriving deltas against one shared bound, catching the bug of letting one delta's clamp affect the other. |
| `bounded-counter-repair-v1` | repair | **requirement preservation** | The stated requirement — clamp after every step, not just the final sum — is exactly what a plausible repair silently drops. |
| `booking-window-conflict-v1` | repair | **requirement preservation** | The half-open, exclusive-end requirement ("adjacent handoffs are not overlaps") is what a plausible repair (loosening `<` to `<=`) violates. |
| `stale-edit-preservation-v1` | repair | **context-limited maintenance** | Measures whether a targeted fix leaves an unrelated, already-correct piece of work untouched — the failure mode of an agent that cannot hold the whole file in view and "cleans up" what it does not need to touch. |
| `structured-input-error-handling-v1` | validation | **API evolution** | The subject is a versioned record envelope; classifying an unsupported version against a supported one is a compatibility-boundary question, not a pure-arithmetic one. |
| `cold-chain-release-gate-v1` | validation | **security fix** | The realistic wrong candidate joins two safety predicates with `\|\|` instead of `&&` — a fail-open defect in a release/safety gate, the canonical shape of a security bug that lets unsafe data through. |
| `telemetry-overflow-diagnosis-v1` | diagnosis | **failure recovery** | The task is precisely about recovering from an arithmetic-overflow failure (saturate) instead of panicking, silently leaving the declared range, or raising a fault, across three runtimes that each fail differently for the same root cause. |
| `clean-install-calculator-v1` | onboarding | **feature change** | Its own `EQUIVALENCE.md` states the distinguishing skill directly: add one new operation to an existing, tool-generated scaffold without disturbing any of it — ordinary feature addition to an existing project, not greenfield authoring. |

That accounts for ten of the eleven categories through task content. The
eleventh, **Agent workflow**, is not a content shape a static public/hidden
fixture can express on its own — it is a claim about *how* a task is solved
and scored, not what the task's logic does. This corpus demonstrates it
orthogonally, through the seam `agent/` already implements rather than
through a twelfth task family: `agent/orchestrator.py`'s
`evaluate_agent_pair` drives `structured-input-error-handling-v1::rust`
through the full solver path — prompt construction, budget enforcement,
retry accounting, a transport-produced candidate, transcript-digest binding,
and then `run.py`'s own build/test/leak-check/provenance scoring — and
`agent/tests/test_agent_driver.py::RealToolchainEndToEndTests` exercises
that path end to end against a real `rustc`, including a wrong-candidate
control that passes public but fails hidden through the agent path
specifically (`test_wrong_candidate_from_transport_passes_public_but_fails_hidden`).
A task's `issue_211_category` therefore names its *content* shape;
"Agent workflow" is a property of the harness path a task is run through,
and `structured-input-error-handling-v1` is this corpus's example of both at
once (content: API evolution; execution path: Agent workflow).

This mapping is deliberately additive and reversible: no `category` value
changed, no task was deleted or renamed, and a future task can carry its own
`issue_211_category` without touching this table's existing rows.

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

Before scoring, each implemented task/language pair must have a public
directory and a non-empty hidden overlay. The overlay must add at least one
regular file or replace a public file with different bytes. Replacing files
at the same relative paths is supported; a new filename is not required.
This is the same structural rule the committed-inventory test enforces,
now also applied to caller-supplied task trees at evaluation time.

Missing, non-directory, empty, or byte-identical hidden overlays produce
`failed` with a reason before any version probe, build, or run. The agent
scorer performs this preflight before calling its transport, so it records
no usage, transcript, candidate, or scoring evidence for such a refusal.
Fixture admission precedes digest checks and transport budget/retry outcomes;
valid fixtures retain those existing checks. A refused pair has no `public`,
`hidden`, `leak_check`, or `provenance` result because none was produced.
Observed inspection errors also fail closed without emitting file contents.

`--dry-run` uses the same check for implemented, declared pairs and exits
nonzero for an invalid overlay. Its plan schema is unchanged: `exists`
continues to mean that both directories exist, even when an existing overlay
is empty or changes no bytes. Refusal details are written to standard error.

This is a structural prerequisite, not proof of additional test coverage:
different bytes can still test the same behavior. Task-specific wrong-candidate
controls remain necessary. The preflight assumes operator-controlled fixture
trees that stay unchanged during evaluation; it does not provide filesystem
sandboxing, race-free snapshots, or secrecy from arbitrary host processes.

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

## Agent driver seam

`run.py` (the harness this whole document otherwise describes) scores a
fixed, human-written source tree — it has no model, provider, sampling, or
budget concept, and no code path in it invokes a model. `agent/` (a sibling
directory) adds the seam a real Agent-driven run would go through, without
modifying `run.py`:

- **`agent/contracts.py`** defines the request a solver must be given —
  `ModelIdentity` (provider, model, revision; a mutable alias such as
  `"latest"` or `"@main"` is rejected at construction, same rule
  `adapters.json` already follows for toolchain selectors), `SamplingParams`
  (temperature, top_p, an explicit `seed`), and `Budget` (max prompt/
  completion/total tokens, max retries, max cost). Every one of these is a
  required constructor argument; none has an environment-variable or other
  ambient fallback (`AGENTS.md`: "Capabilities are explicit... no ambient...
  secret, key... authority" — read here as applying to model authority, not
  only filesystem/network authority).
- **`agent/budget.py`**'s `BudgetLedger` charges usage against a `Budget` and
  raises `BudgetExceededError` or `RetriesExhaustedError` the instant a
  ceiling would be crossed, leaving its own recorded usage exactly as it was
  before the rejected charge. `agent/orchestrator.py` catches either and
  writes a terminal record (`status: "budget_exceeded"` or
  `"retries_exhausted"`) with **no** `public`/`hidden` key — the same
  discipline `run.py` already uses for `blocked` and `drifted` (see "Scoring
  and hidden-test isolation" above): a pair that never reached a build/run
  step is recorded as one, not silently absorbed into `failed`.
- **`agent/replay_transport.py`** is a deterministic, offline
  `SolverTransport`: it reads one committed JSON fixture
  (`benchmark.cross_language.agent.replay_fixture.v1`) and replays its
  scripted attempt sequence verbatim — no network call, no clock, no RNG.
  Given the same fixture and request, two runs on any host produce
  byte-identical usage and transcript digests
  (`tests/test_agent_driver.py::ReplayTransportTests.test_replay_is_byte_for_byte_deterministic`).
  A fixture is a hand-authored script standing in for a model response —
  the offline analogue of `tests/documentation/cross_language_benchmark_suite.rs`'s
  `MockLanguage` — never a recorded real-model transcript; every committed
  fixture says so in its own `_non_claim` field.
- **`agent/live_transport.py`** is a real-provider `SolverTransport`.
  Declared, never exercised in this repository: it refuses to construct
  without an explicit, non-empty `api_key` argument (never `os.environ`),
  and refuses to `complete()` even when a key is supplied, because no HTTP
  request/response mapping in it has ever been verified against a real
  provider response here (no network access, no credentials in this
  environment, and none acquired to build this seam). Shipping a "working"
  HTTP call that has never actually been exercised would itself be the
  untested-capability pattern this document's audit calls out; refusing is
  the honest alternative until a human supplies credentials, a
  network-egress decision, and reviews the wire mapping against a real
  response.
- **`agent/orchestrator.py`**'s `evaluate_agent_pair` is the agent-driven
  analogue of `run.py::evaluate_pair`: it calls the transport, and — only if
  a candidate was actually produced within budget — writes the candidate's
  files into the same two-phase (`public` then `hidden`) scratch-tree
  scoring, by *importing* `run.py`'s own `digest_tree`, `stage`,
  `relative_files`, and `copy_tree` (via `agent/_harness.py`) rather than
  reimplementing them. The leak check and provenance computation are
  therefore the same code, not a second copy that could drift from it. The
  provenance block this path records binds the task's own digest, the
  adapter's observed toolchain version, the model identity, the sampling
  seed, the exact prompt's digest, and the transcript's digest together —
  the "Transcript capture bound to the run's provenance" requirement.
  It also retains each ordered transcript entry and each exact
  transport-produced candidate artifact, each with a content digest. An
  operator may give `run_agent.py` an explicit versioned literal-redaction
  policy to create a publishable projection; the values selected for
  redaction are never emitted, their replacements carry the selected value's
  digest, and every projected item still commits to its original digest. This
  is deliberate redaction, not an ambient secret scan. Candidate paths must
  equal the declared candidate-path set exactly; a traversal/root path,
  non-text payload, or attempted scaffold/test replacement fails before any
  scratch write.

This closes the "no LLM anywhere" gap at the level that is actually
testable without credentials: the full driver path (prompt construction,
budget enforcement, retry accounting, transcript capture, scoring, leak
check) executes end to end, offline, deterministically, against a real
committed task and a real `rustc` toolchain
(`tests/test_agent_driver.py::RealToolchainEndToEndTests`). It does **not**
mean a live Agent has been benchmarked: see `agent/README.md`'s Non-claims,
and the unchanged `HUMAN_BLOCKED: model budget and credentials` entry below.

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
  bounded implementation worker holds — still recorded as
  `HUMAN_BLOCKED: model budget and credentials for a live Agent pilot`. What
  changed is what "the harness that pilot would run through" now includes:
  beyond `run.py`'s toolchain-conformance scoring (exercised end to end with
  three real, non-mocked languages —
  `sequence-digest-v1::semaprax`, `::rust`, `::typescript` — and with
  deterministic mock adapters in
  `tests/documentation/cross_language_benchmark_suite.rs` standing in for
  the six languages with no available toolchain in this sandbox), the
  agent-driver seam described above (`agent/`) means the request/response
  contract, budget enforcement, retry accounting, and transcript-to-
  provenance binding that pilot would need are now implemented and
  exercised too — through `agent/replay_transport.py`, never through
  `agent/live_transport.py`, which stays declared and unexercised for the
  exact reason named above. A human supplying credentials still only needs
  to wire a working `LiveTransport.complete()` against a real endpoint and
  verify it; the request/response contract, budget accounting, and scoring
  path it plugs into do not need to be invented at that point.
