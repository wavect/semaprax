# Workflow Profiling v1

Status: **experimental, opt-in benchmark instrumentation**.

Audience: compiler contributors and performance operators.

Workflow profiling times selected compiler, Project, and prepared-interpreter
stages within bounds. It helps local performance investigations; it is neither
a language feature, semantic profile, nor authorization mechanism.

## Enabling the observer

The `unstable-workflow-profiling` Cargo feature is deliberately absent from the
default feature set. A normal build therefore contains no active workflow
observer and no profiling overhead at the instrumentation sites. The
`workflow_observer` benchmark also declares this feature as a required feature;
it must be enabled explicitly when building or running that target.

The feature exposes `semaprax::workflow_profile::capture`, `Observation`, and
the closed `Stage` inventory to the benchmark host. Instrumentation is compiled
at the selected sites only when the feature is enabled. The current stage
inventory is:

`parse`, `canonicalize`, `resolve`, `hir_validate`, `workspace_preflight`,
`project_link`, `graph_render`, `analysis_index`, `target_admission`,
`image_derivation`, `interpreter_preparation`, and `prepared_execution`.

Changing that inventory is a schema change for
`benchmark.workflow-observation.v1`; consumers fail closed on stage drift.

## Observation semantics

`capture` measures one synchronous operation with `std::time::Instant` and
stores its session in thread-local state. `Observation.total_ns` is elapsed wall
time for the whole closure. Each stage reports its call count, inclusive wall
time, and self wall time. Inclusive time includes nested instrumented stages;
self time subtracts nested instrumented time on the same thread. The sum of
self times is therefore useful for the closed instrumentation sites, but it is
not exhaustive CPU attribution.

The observer follows the thread on which `capture` runs. Work performed by a
worker thread is not traced as a separate interior: when the caller waits for a
worker, that wait is part of the caller's enclosing span. A capture started on a
child thread has an independent thread-local session and does not appear in the
parent's observation. Scheduling, blocking, preemption, and other host effects
remain in the wall-clock values.

Captures cannot be nested. A nested call returns `CaptureBusy` without running
its closure. The session is cleaned up on unwinding, so a panic does not leave
observation state active for later work. These refusal and unwind paths are
observability behavior; they do not turn a failed operation into a successful
measurement.

## Bounds and completeness

The enabled observer accepts at most 64 simultaneously open span frames and
1,000,000 span entries per capture. When either bound is reached, the live
operation continues, but additional spans are omitted and the observation is
marked `complete: false`. A malformed span drop also marks the observation
incomplete. The benchmark host asserts completeness and the campaign validator
rejects any incomplete sample, so truncated data cannot be summarized as a
normal observation.

The limits are an evidence bound, not a limit on compiler or interpreter
behavior. `complete: true` means that the bounded observer recorded all spans
that were opened at its instrumented sites for that capture and closed its
stack consistently. It does not mean that all work, all threads, or all CPU
activity was measured.

## Product equality and authority boundary

Timing is collected around ordinary operations and does not define their
results. The workflow observer compares the products of cold and warm frontend
arms, and compares traced and untraced execution outcomes; it also retains a
plain uncaptured timing for comparison. Those equality checks are the benchmark
fixture's guards against changing the product while observing it. They are not
proof that two arbitrary implementations are equivalent.

Observations carry no compiler, validation, execution, filesystem, workspace,
commit, or publication authority. They do not enter canonical source,
canonical formatting, semantic graph JSON, Wasm bytes, diagnostics, project
revisions, execution traces, or other generated artifacts. Recording or
serializing an observation cannot admit a target, authorize a transaction,
change an `ACTIVE` generation, or promote evidence. The observer is a benchmark
host facility and must not be used as a substitute for the ordinary locks,
checks, or commit boundaries.

## Campaign protocol and non-claims

`benchmarks/performance-v1/observe-workflows.py` launches the already-built
observer independently for each campaign, records the observer binary digest,
source inventory digest, commit, dirty-tree state, profile, host facts, and
configuration, and fails closed on source or binary drift. It requires a quiet
host before and after each campaign, bounds campaigns to 1–5 and samples to
1–20, rejects malformed or incomplete documents, and emits
`benchmark.workflow-campaign.v1`. A campaign is `quiet_observed` only after all
requested campaigns pass these checks; `nonquiet`, `failed`, and `drifted`
results are diagnostic evidence and are not successful measurements.

An observation or campaign does not claim:

- a hosted, cross-platform, physical-device, or production result;
- a speedup, regression, capacity limit, or ranking for real deployments;
- stable absolute performance across hosts, kernels, toolchains, profiles, or
  builds;
- exhaustive CPU, wall-clock, allocation, I/O, or worker-thread attribution;
- that an instrumented timing equals the performance of an uninstrumented
  release build; or
- that a quiet host removes all scheduler, thermal, cache, or background-work
  variance.

Use the values to compare like-for-like observations with the recorded subject,
fixture, profile, and host metadata. Product equality is a separate invariant
from timing equality, and neither one upgrades local benchmark evidence into a
semantic, hosted, or production claim.

## PLAN comparison diagnostic census

The same explicit feature also exposes `workflow_profile::plan_comparison`.
`Capture::begin(Binding)` enables a separate, current-thread, fixed-storage
census; `finish` must follow compiler owner teardown. It never enables an old
comparison arm, changes admission, or changes any payment, work request, result,
cap, diagnostic, or publication. The existing timing capture is independent.
This is **instrumented production-algorithm** data when the library is built
without `cfg(test)`, and **instrumented library-test** data otherwise. Neither
is default-shipping evidence or a wall-time/allocation performance result.
The fixed TLS storage and instrumentation frames are outside the compiler's
allocation ledger; this overhead is not a saving and is not priced as compiler
work. Recording does not allocate Rust heap storage. Formatting the report is a
host operation after teardown.

The closed route records actual reached prefixes: owner admission, successful
left/right prepare payments and completed resets, prepared pairs P, comparison
rounds N, pair-scope/match payments, STEP (including its EMPTY bank), LOAD,
exhausted segments, TAIL, segment heads and tail-Some yields, left/right
exhaustion, terminal comparisons, and invocation success/end/unwind. Per-site
byte totals include only successful admitted banks, never future iterations.
Each detailed row carries the actual compiled invoice, copied function span,
payer and selected-route classification. Refused prefixes remain distinct from
successful invocations; counters do not authenticate a budget or source.

Storage retains 128 detailed invocation rows plus checked aggregate counters.
Exceeding that bound marks `detail_complete: false` while aggregate counting
continues. Nesting beyond 32 also loses detail. Checked overflow, reentrant
storage access, or inconsistent guard closure invalidates `counts_complete`;
a live invocation refuses `finish`. Sessions and guards are thread-bound and
capture generations cannot wrap. Function IDs retain full byte length, SHA256,
and a bounded byte prefix, which can end within UTF-8 and is never a full-string
claim. No function/payer pointers survive teardown.

`Binding` is explicitly caller-labelled fixture/source metadata, with optional
caller revision bytes. It carries no validation authority. The two existing
native string-vector fixtures emit this diagnostic only with the feature,
after their original generator returns or unwinds; they retain all original
inputs, options, C fault/allocation assertions and panic payloads. Their label
marks revision unknown: the external round manifest must bind the actual source
HEAD and instrumented binary. No old shipping baseline, application admission,
or complete old/current net is established by a census alone.

The same `Capture` also returns an independent fixed `paid_phases` bank for
cleanup canonical rebuilding/frontier validation, structural replay, factored construction and
comparison, HIR/PLAN path construction, and final comparison. This bank never
uses comparator rows, serials, event counters, or their nesting stack. Each
phase has an explicit payer (`ActiveBuilder`, `Unobserved`, `Oracle`, `Selected`,
or `Unknown`) and lexical origin (`Shipping` or `TestCrosscheck`). Ordinary
test-only crosschecks can debit the active builder; their true payer is retained
while their children inherit `TestCrosscheck`. That label never exempts a check
or changes its account. Production-library captures contain no such test hook.

`CanonicalRebuild` deliberately encloses frontier current/finish/advance, full
plan rebuilding, and canonical comparison; it is not a narrower constructor-only
measurement. Phase scope exit is not semantic success. The bank records balanced entries and
ends, normal exits, unwind, ordinary sticky-refusal prefixes, unknown intervals,
and checked sums of known same-ledger debits. Values are inclusive: nested
phase totals must not be added. Restoring even the same outer ledger invalidates
an enclosing snapshot through the existing epoch; missing or foreign accounts
remain unknown rather than zero. Phase overflow or inconsistent closure is
reported separately from comparator counters; a live phase refuses `finish`.
No pointers survive teardown. Instrumentation and its fixed TLS bookkeeping
remain absent in a default build, and supply no allocator/RSS measurement,
forecast discount, application admission, fee credit, or validation authority.
The feature-only native Normalizer owner uses the exact six application and six
standard-library sources through public semantic-workspace initialization at
its unchanged default 64MiB cap. It reports an actual refusal without relabelling
that refusal as successful application qualification.

The #905 suffix selector appends `EmptySelectorAdmitted` and
`EmptySelectorSkipped` to the existing closed comparator-site sequence; earlier
site indices and meanings remain unchanged. The former records only an actually
admitted `size_of::<bool>()` bank, the latter the ensuing structural empty
branch with zero admitted bytes. The old `IdentityEmptyProbe` remains the
original full-PROBE empty-branch event for independently compiled old callers;
it is not reused for the cheaper selector. Nonempty shipping runs still report
their full retained `IdentityProbeAdmitted` bank. These additive counts retain
the same fixed capture, overflow, partial-prefix and source-binding rules and
supply no admission or whole-net conclusion by themselves.

The #907 diagnostic successor appends final independent-comparison admissions at
site indices52–55: `FinalFlatAdmitted`, `FinalSharedAdmitted`,
`FinalStepAdmitted`, and `FinalTailAdmitted`. All previous52 indices and meanings
remain unchanged. Counts and admitted bytes record successful actual banks,
including exhaustion visits; they do not relabel these as PLAN-sort packets.
The separate100-row paid-phase bank and its10×5×2 ordering remain unchanged.
Matched old/current arms must retain this same additive observer API and hooks.


## Path-family request census v1

The feature-only resolved-core report additionally carries three fixed rows per
attempt: residual, ordinary path-state replay, and selected path-state replay.
The additive diagnostic line schema is `semaprax.path-family-census.v1`.
Existing attempt ordinals, operation/stage labels, exclusive debit buckets and
first-refusal ownership remain unchanged. Family debit bytes independently
partition the same attempt's consumed bytes; family totals do not replace the
existing operation attribution. Selected scope includes its caller receiving
fees before body entry, which can therefore refuse with zero body entries.

Rows count actual entered/ended/error/refused/unwound scopes, full successful
terminal arities, Empty/One/Many incoming observations, admitted Box formations,
held transfers, equal duplicates, promotions and tree insertion requests.
Request counts and requested/admitted bytes describe real ledger requests;
they are **not physical allocator counts or B-tree node allocations**.
Known descendant ledgers without a separate attempt are recorded in the nearest
owning attempt with explicit descendant subsets. Raw descendant request bytes
must not be added to the parent's eventual settlement debit. Separate attempt
rows must not be summed as disjoint application work.

Tags bind only telemetry to the actual live ledger, capture generation and
checked ledger-install serial; no pointer survives teardown. Missing/stale
owners, unbalanced scopes, reentrant access or checked overflow make evidence
incomplete. Counter updates never choose a compiler result, acquire authority,
change invoices or limits, or allocate dynamic storage. The fixed observer
storage/layout report includes these rows. Formatting happens after teardown.
The six unchanged application inputs still require exact plain/observed product
equality and complete final-attempt receipts before any application attribution;
fixture allocator metrics alone supply no application-family allocation census.
