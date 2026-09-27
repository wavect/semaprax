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
| `iterative-repair-workflow-v1` | repair | **Agent workflow** | Purpose-built for this category (see below): two sequentially-masked defects plus an unrelated sibling function to preserve, and a task statement that narrates a two-round debugging log a solver must read and act on rather than solve in one pass. |

That now accounts for all eleven categories through task content, closing
the scope decision this section previously left open (see "Decision"
immediately below). `agent/orchestrator.py`'s orthogonal execution-path
demonstration through `structured-input-error-handling-v1` — described in
full in the paragraph after the decision note — remains true and
unchanged; it is additional coverage, not the thing that now satisfies the
category.

This mapping is deliberately additive and reversible: no `category` value
changed, no task was deleted or renamed, and a future task can carry its own
`issue_211_category` without touching this table's existing rows.

### Decision: "Agent workflow" needed a dedicated, content-level task

**2026-09-27, maintainer-directed.** This section previously recorded an
open scope-decision request — whether orthogonal coverage through
`agent/orchestrator.py` (running `structured-input-error-handling-v1`
through the full solver path) was sufficient to demonstrate issue #211's
eleventh category, "Agent workflow", or whether a twelfth, content-level
task was required. The maintainer decided: **orthogonal coverage is not
accepted; the benchmark gets a dedicated, content-level task.**
`iterative-repair-workflow-v1` is that task (issue #298): its
public/hidden split genuinely exercises multi-step, iterative agent work
at the content level — a candidate with two interacting defects the
public tests reveal only the first of at a time, hidden tests that check
both the fully corrected behavior and preservation of an unrelated sibling
function, and a task statement (`EQUIVALENCE.md`'s "iterative-repair
narrative" section) that requires reading an earlier step's redacted
output to find the second defect at all. The orthogonal demonstration
below is not removed or narrowed by this decision — it stays in this
corpus exactly as it already was — but it no longer stands as the sole
answer for this category.

`agent/orchestrator.py`'s `evaluate_agent_pair` drives
`structured-input-error-handling-v1::rust` through the full solver path —
prompt construction, budget enforcement, retry accounting, a
transport-produced candidate, transcript-digest binding, and then
`run.py`'s own build/test/leak-check/provenance scoring — and
`agent/tests/test_agent_driver.py::RealToolchainEndToEndTests` exercises
that path end to end against a real `rustc`, including a wrong-candidate
control that passes public but fails hidden through the agent path
specifically (`test_wrong_candidate_from_transport_passes_public_but_fails_hidden`).
A task's `issue_211_category` names its *content* shape;
"Agent workflow" is also, separately, a property some tasks' harness path
can demonstrate, and `structured-input-error-handling-v1` remains this
corpus's example of both at once (content: API evolution; execution path:
Agent workflow) even though it is no longer the category's sole content
representative.

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

## Environment pinning: exact local toolchain identity, and reserved-language provisioning decisions

Issue #298's remaining scope asks this suite to record, per admitted
adapter, "the exact local toolchain identity the runnable adapters now bind
(path + version + digest)", and, for the six reserved external languages,
"the exact provisioning each would need". Neither table below is a pin: a
pin is a byte-identical, network-free artifact the harness refuses to run
without (`runnable_adapter.py`'s `SnapshotError` family already fails
closed on drift from an *admitted descriptor*); what follows is an honest,
dated **observation** of this host, recorded so a future pin has a concrete
starting point instead of a blank page. Recorded 2026-09-27 on one arm64
macOS 26.5.1 (build 25F80, Darwin kernel 25.5.0) host; every value below was
read directly from that host, not copied from a fixture or a comment.

### Admitted adapters: what `runnable_adapter.py`/`runnable_adapter_v2.py` actually bind today

`rust` is admitted by v1; `c`, `python`, `swift`, `java`, and `typescript`
are admitted by v2 (`V2_ADAPTERS` in `runnable_adapter_v2.py`). `semaprax`
and `semaprax-project` are not bound by either module — they invoke the
compiler under test via the caller-supplied `--semaprax` path with no
admission/digest step, because the compiler is the subject being measured,
not an external comparison toolchain — so they carry no row here.

| Adapter | Bound tool(s) | Resolved path | Version observed | SHA-256 (binary) | Trust shape |
| --- | --- | --- | --- | --- | --- |
| `rust` | `rustc` | `/opt/homebrew/Cellar/rust/1.98.0/bin/rustc` (symlinked from `/opt/homebrew/bin/rustc`) | `rustc 1.98.0 (88d9e12ae 2026-08-18) (Homebrew)` | `b0cf136c59e80f0eb7bafbd772f73a412911a55fb99857dcdc1d1bad2423a0ee` | copied toolchain root (`v1._toolchain_digest` hashes the whole `/opt/homebrew/Cellar/rust/1.98.0` root at admission time; not reproduced by hand here — the digest above is only the `rustc` binary itself, a spot check) |
| `rust` (linker) | `cc` | `/usr/bin/cc` | n/a (dispatch stub) | `179301dcb41ea78accc3fa0048a7e6f6710d891945a751a34addd622020c1818` | SIP/root-owned, verified in place |
| `rust` (link editor) | `ld` | `/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/ld` | n/a | `5897b275efd93b201b6df5832dd541262b3f20f290859ba78f2200a6a66ef38b` | SIP/root-owned, verified in place |
| `rust` (SDK) | `MacOSX.sdk` | `/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk` | SDK `26.5`, `SystemVersion.plist` `ProductVersion 26.5.1` / `BuildVersion 25F80` | `SDKSettings.plist` `e5c7c40b8c5dc1a9f99f8b9fa51870f8fe180421225b8201d0c4c826aad11bdc`; `SystemVersion.plist` `d90b1755e5dbb837d2ca1e11083c6e36e6219193a0fcf036d0f7cfe5366e031e` | verified in place |
| `c` | `clang` | `/usr/bin/clang` | `Apple clang version 21.0.0 (clang-2100.1.1.101)` (dispatches to `/Applications/Xcode.app/.../XcodeDefault.xctoolchain/usr/bin`) | `179301dcb41ea78accc3fa0048a7e6f6710d891945a751a34addd622020c1818` | SIP/root-owned, verified in place |
| `python` | `python3` | `/usr/bin/python3` | `Python 3.9.6` | `179301dcb41ea78accc3fa0048a7e6f6710d891945a751a34addd622020c1818` | SIP/root-owned, verified in place — **note**: this host's shell `PATH` resolves an unrelated `python3` at `~/.local/bin/python3` first; the adapter must invoke the absolute `/usr/bin/python3` path from `adapters.json`, never a bare `python3` off `PATH`, or it binds the wrong interpreter |
| `swift` | `swiftc` | `/usr/bin/swiftc` | `swift-driver version: 1.148.6`, `Apple Swift version 6.3.3 (swiftlang-6.3.3.1.3 clang-2100.1.1.101)` | `179301dcb41ea78accc3fa0048a7e6f6710d891945a751a34addd622020c1818` | SIP/root-owned, verified in place |
| `java` | `javac` | `/usr/bin/javac` | `javac 22.0.2` | `d641f84fbed5fcd611d603fe5aa364f152462d7d099e09eeaf36f046be4c3f32` | SIP/root-owned, verified in place |
| `java` | `java` | `/usr/bin/java` | `java 22.0.2 2024-07-16` (HotSpot 64-Bit Server VM, build `22.0.2+9-70`) | `d641f84fbed5fcd611d603fe5aa364f152462d7d099e09eeaf36f046be4c3f32` | SIP/root-owned, verified in place |
| `typescript` | `node` | `/Users/kevin/.nvm/versions/node/v24.3.0/bin/node` | `v24.3.0` | `afa8bdc2d587911bd6ec58d568e15611571f06e3902716354541775556074abf` | copied tool, materialized into a private snapshot before launch |
| `typescript` | `typescript_lib` (`tsc`) | `/Users/kevin/Library/pnpm/global/5/.pnpm/typescript@5.8.3/node_modules/typescript` (dispatch shim at `/Users/kevin/Library/pnpm/tsc`) | `Version 5.8.3` | not spot-checked here (22 MiB package tree; `v2._toolchain_digest`-equivalent copied-root hashing applies at admission time, same as the Rust toolchain root above) | copied root, materialized into a private snapshot before launch |

Two observations worth keeping honest, not silently smoothed over:

- `clang`, `python3`, and `swiftc` are **byte-identical** on this host
  (`179301dc...`) — all three are the same Xcode `xcrun` dispatch stub, not
  three different compiler binaries. `javac`/`java` are likewise
  byte-identical to each other (`d641f84f...`), the JDK launcher stub. This
  matches `runnable_adapter_v2.py`'s own docstring claim that these tools
  are "SIP/root-owned executable" dispatch points rather than the actual
  compiler payloads, and is exactly why v2 verifies path + owner + mode +
  digest and then references the *original* path rather than copying: the
  real compiler lives behind the stub, at a location neither this table nor
  the adapter dereferences further.
- None of this is an authenticated official-release digest the way a
  package registry's signed manifest would be — it is **local-host
  provenance**: the exact path, owner, mode, and byte digest this repository
  actually observed on this one machine, on this one date, no more and no
  less. A different host, a Homebrew upgrade, or an Xcode Command Line Tools
  update changes every value in this table without changing the adapter
  code; `provenance.adapter_version` in a scored result records whichever
  version was actually observed at run time for exactly this reason.

### Reserved external languages: availability/equivalence decision table

None of the six languages below can be provisioned by this worker: each
needs either build-time network access (forbidden by this sandbox's
invariants) or a maintainer decision this worker cannot make unilaterally
(reviewing whether a previously-pinned revision is still the right
comparison subject). No container image was built and no network fetch was
attempted for any row.

| Language | Official toolchain source | Version to pin | Digest mechanism once fetched | Why blocked offline today |
| --- | --- | --- | --- | --- |
| Zero | `github.com/vercel-labs/zerolang`, git checkout at the revision already reserved by `benchmarks/agent-task-comparison-v1/manifest.json` | `vercel-labs/zerolang@eb2ed6c22fe3f6e3152efa0c0d05ffcf1ff4a2c7` (pinned elsewhere in this repo; not yet reviewed as the *current* right comparison subject — see issue #106/#107) | `git verify-commit`/`git rev-parse` against the pinned SHA, then a `sha256:` digest over the built toolchain root exactly as `_toolchain_digest` does for Rust above | `git clone` of that revision needs build-time network access, which this sandbox forbids; separately, issue #106/#107's own review step ("the reserved historical Zero revision is not automatically the appropriate current comparison subject") has not been completed by a maintainer |
| NTNT | not located: no publicly documented official toolchain distribution was found for this benchmark snapshot | unknown until identified | unknown until identified | no source to fetch from at all, offline or online — this is a naming/identification gap, not only a network gap |
| Aver | not located: no publicly documented official toolchain distribution was found for this benchmark snapshot | unknown until identified | unknown until identified | same as NTNT |
| Vera | not located: no publicly documented official toolchain distribution was found for this benchmark snapshot | unknown until identified | unknown until identified | same as NTNT |
| Hale | not located: no publicly documented official toolchain distribution was found for this benchmark snapshot | unknown until identified | unknown until identified | same as NTNT |
| MoonBit | `moonbitlang.com`'s official installer script (a `curl`-piped-to-shell installer, per MoonBit's own published install instructions) | latest stable release at pin time (not yet selected) | the installer's own published release checksum, or a `sha256:` digest over the installed toolchain root exactly as `_toolchain_digest` does for Rust above, once a specific release is pinned | the only official distribution path is a network-fetched installer script; this sandbox forbids build-time network access, and no pre-mirrored offline copy of any specific release exists in this repository today |

For every row above, the concrete next step is the same three-part
maintainer action issue #211/#298 already name: (1) confirm or supply the
official source and an exact pinned revision/release, (2) provision it as a
network-free local checkout, install, or container image outside this
worker's authority, and (3) review the resulting toolchain root's digest the
same way `runnable_adapter.py` already reviews Rust's. None of the three is
performed here.

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
  whatever version is installed on the run host; it does not pin one. The
  "Environment pinning" section above now records exactly what was actually
  observed (path, version, digest) for every admitted adapter on one dated
  host, and an explicit availability/equivalence decision table for the six
  reserved external languages — but recording an observation is not
  provisioning a pin. Building and provisioning pinned, network-free
  per-language toolchain images (issue #211's "Containerized/pinned
  environments") needs infrastructure and a network-access decision this
  repository's invariants forbid making unilaterally — recorded as
  `HUMAN_BLOCKED: container image provisioning` rather than attempted here.
- **A live Agent pilot run and second-host reproduction**: issue #211 also
  asks for "at least one pilot task run across all initial languages and two
  models," and #298 additionally asks for that pilot's scoring inputs to be
  reproduced on an actual second physical/virtual host. Both stay explicitly
  **open** — not attempted, not simulated, and not narrowed — because both
  need authority no bounded implementation worker holds. The exact operator
  actions required, so this stays a checklist rather than a vague blocker:

  1. **Two named model identities** (provider, model, revision — never a
     mutable alias such as `"latest"`, matching `agent/contracts.py`'s
     `ModelIdentity` constructor rule) for the two-model pilot issue #211
     asks for.
  2. **A credential owner**: a human or service account that holds the API
     key(s) for those two models and is authorized to supply them to
     `agent/live_transport.py`'s `api_key` constructor argument (never via
     `os.environ`, per that module's existing refusal-to-construct-without-
     one behavior).
  3. **An explicit spend cap**: a `Budget` (max prompt/completion/total
     tokens, max retries, max cost) the credential owner authorizes in
     advance, wired through `agent/budget.py`'s `BudgetLedger` exactly as it
     already enforces for the replay-transport path today.
  4. **A publication decision**: whether and how the resulting transcripts,
     candidates, and scores are shared, and under what redaction policy
     (`run_agent.py`'s existing literal-redaction mechanism is ready to
     apply one; no policy is chosen here).
  5. **A second physical or virtual host**, provisioned and reachable
     independently of the host this table was recorded on, with its own
     observed toolchain identity recorded the same way the table above
     records this host's — so a reproduction is measured against its own
     honest environment record, not assumed identical to the first host's.

  None of these five is authorized here; recording them is what turns
  "blocked" into an actionable request instead of a closed door. What
  changed in this corpus, and stays true regardless of when the five items
  above are supplied: beyond `run.py`'s toolchain-conformance scoring
  (exercised end to end with three real, non-mocked languages —
  `sequence-digest-v1::semaprax`, `::rust`, `::typescript` — and with
  deterministic mock adapters in
  `tests/documentation/cross_language_benchmark_suite.rs` standing in for
  the six languages with no available toolchain in this sandbox), the
  agent-driver seam described above (`agent/`) means the request/response
  contract, budget enforcement, retry accounting, and transcript-to-
  provenance binding that pilot would need are now implemented and
  exercised too — through `agent/replay_transport.py`, never through
  `agent/live_transport.py`, which stays declared and unexercised for the
  exact reason named above. A human supplying items 1-4 above still only
  needs to wire a working `LiveTransport.complete()` against a real endpoint
  and verify it; the request/response contract, budget accounting, and
  scoring path it plugs into do not need to be invented at that point.
  Recorded as `HUMAN_BLOCKED: model budget, credentials, spend cap, and a
  second host for a live Agent pilot and its cross-host reproduction`.
