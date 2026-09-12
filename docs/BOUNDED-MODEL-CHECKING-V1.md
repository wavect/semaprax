# Bounded Model Checking v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors, plus compiler contributors working on
lifecycle/authorization/resource-handle correctness, the Assurance Manifest
v1 obligation join (#129, #183), and the neighboring [Bounded SMT
Discharge v1](SMT-DISCHARGE-V1.md) (#184) and [SMT Proof Certificate
v1](SMT-PROOF-CERTIFICATE-V1.md) (#186) backends this module's `Verified`
outcome joins in the same way theirs do: as an externally supplied
`MethodRecord`, never an automatic `generate()` derivation.

`semaprax.model-checking.v1` (`../src/assurance_manifest/model_checking/`)
is a deterministic, bounded, explicit-state model checker: a generic
`TransitionSystem` trait, a breadth-first exploration engine over it with
hard-required bounds, and two committed protocol projections. It answers
"does every state reachable within these exact bounds satisfy this
invariant" and "is some state satisfying this predicate reachable within
these exact bounds", and it reports incomplete exploration as its own
outcome rather than as a weaker form of "yes". It is proof data, not
permission: it never runs a target, spawns a process, discovers or runs
project tests, writes source, or removes a runtime guard.

## Why this exists

Issue #185 asks for finite-state exploration of Agent steps, authorization
consumption, checkpoints, resource handles, and protocol transitions, with
minimal counterexample traces, explicit bounds whose exhaustion is never
silently read as success, and Assurance Manifest integration. The
`smt_discharge` (#184) audit trail this issue explicitly builds on found a
real bug of exactly the shape this issue warns about: giving a *derived*
value (`result`) the same unconditional range axiom a genuine free
parameter gets made an overflow trap look vacuously proved. Every design
decision below traces back to not repeating that mistake in a different
formalism.

## The engine

[`engine::TransitionSystem`](../src/assurance_manifest/model_checking/engine.rs)
is the one trait a model implements: `initial_states`, `enabled_events`,
`apply`, `is_terminal`, `safety_invariant`. `State` and `Event` must be
`Ord` so the engine deduplicates visited states with a `BTreeSet`/
`BTreeMap` rather than a `HashSet`/`HashMap` — deterministic regardless of
hasher, build, or platform, the same reason `cleanup_plan`'s canonical
vectors are never sorted by an incidental hash order.

[`engine::Bounds`](../src/assurance_manifest/model_checking/engine.rs) has
three required fields and no `Default`: `max_states` and `max_transitions`
are hard caps that stop the search the instant they would be exceeded (they
bound memory and wall time, not search depth); `max_depth` is the classical
bounded-model-checking "k" — a per-branch bound, so a branch that reaches
it stops expanding while shallower sibling branches still finish, and the
minimal-counterexample search a shorter violation would need is never
truncated by a depth limit sized for a different, longer branch.

[`engine::check_safety`](../src/assurance_manifest/model_checking/engine.rs)
runs one bounded, deterministic breadth-first exploration and returns an
[`engine::SafetyOutcome`](../src/assurance_manifest/model_checking/engine.rs):

- `Verified` — the full reachable space closed (the search frontier
  drained to empty; no `Bounds` limit ever fired), no state failed
  `safety_invariant`, and every dead end was a declared terminal state.
  **This is the only outcome a caller may report as `model_checked`.**
- `Violated { trace, invariant }` — some reachable state failed
  `safety_invariant`. BFS guarantees `trace` is the shortest witness this
  exploration's fixed, deterministic order reaches first.
- `DeadState { trace }` — a reachable state has no enabled events and is
  not declared terminal. See "Vacuity defenses" below.
- `EmptyStateSpace` — `initial_states` returned nothing at all.
- `BoundExhausted { limit }` — a `Bounds` field fired before closure;
  `limit` names exactly which one.

[`engine::check_reachable`](../src/assurance_manifest/model_checking/engine.rs)
runs the same traversal searching for a caller-supplied target predicate
instead of a safety violation, returning the mirror-image
[`engine::ReachabilityOutcome`](../src/assurance_manifest/model_checking/engine.rs)
(`Reached` / `NeverReached` / `DeadState` / `EmptyStateSpace` /
`BoundExhausted`) — the bounded-liveness half of the issue's required
outcomes: "is a good state reachable" or "can a bad state never be
escaped" within an exact, reported bound, never claimed beyond it.

## Vacuity defenses

The issue names the exact failure mode to guard against: *"a state space
that is vacuously empty, or a property that holds only because no
reachable state was explored"* — the model-checking analog of #184's
`result`-gets-a-false-range-axiom bug.

Two structural defenses, not merely tests, exist in the engine itself:

1. **`EmptyStateSpace` is checked before anything else runs.** A model
   whose `initial_states` returns nothing can vacuously "satisfy" every
   invariant; the engine refuses to call that `Verified` and reports the
   distinct `EmptyStateSpace` outcome instead.
2. **A reachable dead end must be *declared*, not merely discovered.**
   When `enabled_events` returns nothing for a state, the engine consults
   `is_terminal` before accepting that as closure. An under-implemented
   `enabled_events`/`apply` pair that silently stops generating successors
   after one or two states — the direct model-checking equivalent of an
   axiom that quietly generalizes further than it should — surfaces as
   `DeadState`, explored-state count included, rather than a `Verified`
   report that looks identical to a genuinely closed, correct one.

`assurance_manifest::model_checking::tests` commits both cases as regression
tests over a minimal toy system (not the real models, so the defense is
tested independent of any one model's own correctness):
`vacuous_empty_initial_state_space_is_not_verified` and
`under_implemented_transition_table_is_reported_as_dead_state_not_verified`
— the latter is the direct analog test the owning issue asked for: it
explores exactly one state, has an invariant that trivially holds, and
still must not report `Verified`.

## Model A: agent turn authorization, dispatch, and checkpoint recovery

[`authorization_model`](../src/assurance_manifest/model_checking/authorization_model.rs)
is a ten-phase lifecycle: `Idle -> Authorized -> Dispatched -> {ObservedSuccess,
ObservedFailure, Crashed} -> {Completed, Failed, RecoveringUncertain} ->
RecoveryRefused`. It is a **self-contained projection**, not a hook into
`agent_lifecycle`, `agent_runtime_v2`, or `live_invocation` (all outside
this tranche's file lease): it imports nothing from them. Its vocabulary
mirrors theirs deliberately — `Authorize` mirrors
`agent_lifecycle::authorization::run_authorize_stage` minting a one-use
`Authorized` grant; `Crash`/`RecoverAfterCrash` mirror a journal ending in
`JournalEvent::Intent` and
`agent_runtime_v2::checkpoint::RecoveryDisposition::UncertainIntent`;
`RefuseRedispatch` mirrors `docs/AGENT-OPERATION-CHECKPOINT-V2.md`'s "it
never automatically dispatches that effect again" — without claiming to
execute or even read that code.

Two invariants, checked at every reachable state:

- `dispatch_has_fresh_matching_grant` — a dispatched-or-later state must
  record that the grant it consumed is the one, most recently minted
  grant. "No dispatch without a fresh matching grant."
- `no_uncertain_redispatch` — once a dispatch has crashed (unknown
  outcome), the model must never re-enter `Dispatched` carrying that same
  crashed grant. "No automatic redispatch of uncertain intent."

`Correct` (the real, un-mutated table) is `Verified` within
`Bounds { max_states: 64, max_depth: 16, max_transitions: 128 }`, closing at
exactly 10 states and 9 transitions — both asserted exactly in
`correct_model_explores_every_declared_phase`, a coverage floor that would
fail loudly if a future edit silently stopped generating successors.

`Faulty(Fault)` seeds exactly one illegal transition into the same table:

- `DispatchWithoutAuthorization` adds an illegal `Idle -> Dispatched` edge.
  Caught as a **one-step** `Violated { invariant:
  "dispatch_has_fresh_matching_grant" }`.
- `RedispatchUncertainIntent` adds an illegal `RecoveringUncertain ->
  Dispatched` edge reusing the crashed grant. Caught as a **five-step**
  `Violated { invariant: "no_uncertain_redispatch" }` along the exact path
  `Authorize, Dispatch, Crash, RecoverAfterCrash,
  FaultRedispatchStaleGrant`.

Both exact traces are asserted field-by-field in
`authorization_model::tests`, matching the issue's *"seeded faulty
transition tables produce known minimal counterexamples"* requirement.

## Model B: resource/carrier handle acquire, use, release

[`handle_model`](../src/assurance_manifest/model_checking/handle_model.rs)
is a five-phase lifecycle: `NotAcquired -> Live <-> InInvocation`, with
`Live -> {Released, Abandoned}` as the two terminal discharges. Another
self-contained projection, mirroring `host_ownership::HostOwnerState`'s
`Live`/`InInvocation`/`Dead` naming and the repository's existing "double
release"/"orphaning a live owner" vocabulary
(`docs/ARC-ZONES-V1.md`, `docs/PUBLIC-GENERIC-CARRIER-V1.md`) without
calling any of that code.

Two invariants:

- `discharged_exactly_once` — a handle's `Release`/`Abandon` fires at most
  once ever.
- `no_discharge_while_in_invocation` — a handle is never released or
  abandoned while a call against it is still outstanding.

`Correct` is `Verified`, closing at exactly 5 states and 5 transitions (the
fifth transition, `InInvocation --CompleteInvocation--> Live`, closes a
cycle back to an already-visited state and is counted as explored work
without adding a sixth visited state — see
`correct_model_explores_every_declared_phase`'s comment for why 5 states
and 5 transitions is the correct, not merely convenient, pair of numbers).

`Faulty(Fault)` seeds one illegal transition each:

- `ReleaseWhileInInvocation`: a **three-step** `Violated { invariant:
  "no_discharge_while_in_invocation" }` (`Acquire, BeginInvocation,
  FaultReleaseWhileInInvocation`).
- `DoubleRelease`: a **three-step** `Violated { invariant:
  "discharged_exactly_once" }` (`Acquire, Release, FaultDoubleRelease`).

## Independent replay of a counterexample

Each model's test suite includes
`counterexample_trace_replays_independently_to_the_same_violation`: given
the `Violated` outcome's `trace`, it re-runs the exact recorded event
sequence from a fresh initial state through the model's `apply` function a
second time and confirms it reproduces the identical end state and
re-trips the identical invariant. This proves the reported trace is a real,
reproducible path through the transition function the search already had —
not an artifact of how the search happened to walk its internal indices.

**Exact scope of this replay**: it replays against the *same* `apply`
function the search itself called, not against a separate live executable
fixture (an actual compiled Agent runtime, checkpoint journal, or handle
implementation). The issue lists "replay of counterexample traces against
executable fixtures where possible" as in scope; wiring either committed
model's trace format to a real fixture in `agent_lifecycle`,
`agent_runtime_v2`, or `host_ownership` is explicitly **not implemented**
here — those modules are outside this tranche's file lease, and building
that bridge is future work for whichever tranche owns the corresponding
executable fixture.

## Model identity and drift

[`digest::model_digest`](../src/assurance_manifest/model_checking/digest.rs)
hashes a `ModelDescriptor` (name, version, the exact invariant and
terminal-state labels it declares) together with the `Bounds` an
exploration ran under, using the same domain-separated SHA-256 and
length-prefixing technique
`assurance_manifest::obligation::obligation_id` and `render.rs`'s digests
use, so no field boundary can alias another (see
`length_prefixing_prevents_field_boundary_aliasing`).

**Exact scope of this digest**: it binds a model's *declared* identity and
the bounds it ran under. It does not hash the Rust bytecode of
`enabled_events`/`apply`/`safety_invariant`. A behavioral change to those
functions not accompanied by a `version` bump is **not** caught by this
digest automatically; model authors are responsible for the same
version-bump discipline every other versioned wire schema in this
repository already requires. This is a deliberate, stated non-claim, not
an oversight: building executable-content hashing for arbitrary Rust
functions is out of scope for this tranche.

## Assurance Manifest integration

[`model_checked_record`](../src/assurance_manifest/model_checking/mod.rs)
is the only bridge to `assurance_manifest`: given an `ExploreReport` and a
`ModelDescriptor`, it returns `Some(MethodRecord)` — `class:
AssuranceClass::ModelChecked`, the exact bounds, the model digest, and the
explored state/transition/depth counters — **only** when the outcome is
`Verified`. Every other outcome (`Violated`, `DeadState`,
`EmptyStateSpace`, `BoundExhausted`) returns `None`: this crate's assurance
lattice has no "disproved" or "incomplete" class, and folding any of those
into `assumed`/`attempt_inconclusive` would misrepresent a bounded-but-real
finding as a shrug, exactly as `smt_discharge`'s `Verdict::Sat`/`Refuted`/
`Unknown` are never turned into a method record either.

As with `smt_discharge`, this is a record a caller merges in through
`ExternalRecords` — `assurance_manifest::generate` itself still never
invokes a model checker automatically; see `obligation.rs`'s own
documentation of `ExternalRecords` as the join point for `model_checked`
(and `smt_proved`, `theorem_proved`) records today, ahead of the
"candidate-assurance join" #129 was scoped to build.

`reductions=none (no partial-order reduction implemented)` is recorded
verbatim on every method record's `detail` field: the issue lists partial-
order reduction as in scope "where sound," and this tranche does not
implement it. Every exploration here is a full explicit-state BFS with no
independence-based pruning, so there is no risk of the specific failure
mode the issue warns about — *"partial-order reduction can remove
meaningful authority races if independence is inferred incorrectly"* —
because none is performed.

## Explicitly deferred, not merely unimplemented

- **Executable fixture replay.** See "Independent replay" above.
- **Partial-order reduction.** See "Assurance Manifest integration" above.
- **A third or later model** for retry/failover policies (issue step 7);
  only the two named above are committed.
- **Environment-nondeterminism enumeration beyond what each model's own
  events encode.** Cancellation, handler success/failure/uncertainty, and
  crash boundaries are represented as explicit events in Model A
  (`HostFails`, `Crash`) exactly where the issue asks for them; a general
  facility for declaring arbitrary environment choice points is not built.
- **Hashing model source code for drift detection.** See "Model identity
  and drift" above.
- **Any CLI or MCP surface.** Both models and the engine are library code
  reached from `#[cfg(test)]` and future caller code, not from
  `src/cli/**` (outside this tranche's lease).

## Evidence

Local, fixture-free unit and integration tests — no subprocess, no
filesystem or network access, matching this crate's default `cargo test`
discipline of spawning nothing. Exact counts, commands, and the criterion-
by-criterion mapping to the owning issue's acceptance criteria are in the
implementing session's final report, not duplicated here to avoid the two
drifting apart.
