# Deterministic Scoped Task Model v1

- Status: Locally evidenced hidden proof model; no runtime, scheduler, language
  syntax, compiler, or backend wiring exists or is claimed
- Version: 0.1
- Audience: language, compiler, runtime, and conformance-test implementers;
  agents auditing structured-concurrency semantics before any implementation
  exists

## Summary

This document fixes the bounded target-neutral model that a future
structured-concurrency implementation must preserve. The repository contains
`src/scoped_tasks.rs`, a deterministic proof-data model of scoped task
execution: a bounded task DAG inside one strict scope tree, sequential
scheduling in canonical stable-id order, sticky cancellation propagation,
children-before-parents cleanup on scope exit in reverse completion order,
first-failure stickiness with sibling draining, and closed per-task
`Sendable`/`Shareable` annotations.

The module deliberately contains no threads, no async runtime, no scheduler
integration, no language syntax, no parser/HIR/Graph/backend changes, and no
`Sendable` checking of real programs. Like the callable-v3 settlement model,
everything it produces is evidence of what a conforming implementation MUST do,
never authority to execute anything. A modeled task body is a closed scripted
outcome (`Succeed`, `Fail(Semantic)`, or `Fail(Physical(nonzero))`); nothing is
run and no work is performed.

The key rule is:

> Within one scope tree, tasks can never outlive their scope: scheduling,
> cancellation, and cleanup are fully determined by canonical stable-id order,
> cancellation marks descendants before any sibling starts new work, children
> finalize before parents in reverse completion order, and the first drained
> failure wins over everything that happens later — including later
> cancellation.

## Relationship to existing contracts

This model extends, but does not replace, existing contracts:

- [RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) defines target-neutral
  semantic cleanup order and sticky failure selection for single-function
  cleanup plans; this model lifts the same ordering discipline to a task DAG.
- [RFC 0004](RFC-0004-NATIVE-CALL-SETTLEMENT.md) defines recovery settlement
  for native owned calls; its nonzero-physical-failure convention and its
  evidence-not-authority framing are reused here.
- Cleanup-plan vectors remain canonical execution order; this model never
  repairs, sorts, or substitutes them.
- The completion-matrix "Structured concurrency" row remains far from complete;
  actors/reducers, synchronization, real schedulers, and verified concurrency
  on implemented backends stay open exactly as before.

## Normative goals

The terms **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative.

For an admitted model:

1. Every scope except the root **MUST** have exactly one parent and exactly one
   declared join from that parent; joining anything else **MUST** be rejected.
2. Task dependencies **MUST** form an acyclic bounded graph whose prerequisite
   scopes lie on the dependent's own scope lineage; a cross-branch dependency
   **MUST** be rejected as an escaping reference.
3. Scheduling **MUST** be sequential and deterministic: among ready tasks, the
   canonical smallest stable identity starts first.
4. Cancelling a scope **MUST** mark every descendant scope and **MUST**
   materialize those cancellations — announcing each cancelled scope and
   cancelling each pending task — before starting any other work.
5. An already-started task **MUST** drain to its scripted outcome even after
   cancellation; only pending tasks become cancelled.
6. Scope exit **MUST** finalize started tasks in exact reverse completion
   order, then exit deepest-first so children always finalize and exit before
   parents.
7. The first failure observed in a subtree **MUST** win stickily: independent
   siblings still drain to completion, dependents of failed prerequisites are
   abandoned as cancelled, and a drained failure reported at scope exit beats a
   late cancellation mark.
8. Every structural ambiguity — duplicate identities, unknown endpoints, self
   or duplicate dependencies, cycles, missing/double/orphan joins, zero
   physical failure codes, and bound or work-budget violations — **MUST** fail
   closed at construction or at the offending operation.

## Bounded structure

`semaprax.scoped-tasks-model.v1` enforces:

| Quantity | Maximum |
| --- | ---: |
| Scopes per model | 4,096 |
| Tasks per model | 4,096 |
| Dependency edges per model | 65,536 |
| Validation work units (`tasks * (edges + 1) + scopes`) | 1,000,000 |

Scope identities form a strict containment tree with exactly one root. Task
identities are globally unique. Joins are not free-form edges: the constructor
requires exactly one `waiter = parent(target)` join per non-root scope, rejects
a join targeting the root or naming any non-parent waiter as an orphan join,
and rejects duplicates as double joins. These checks make "tasks cannot outlive
their scope" structural rather than advisory.

## Closed state vocabulary

Task states are exactly `Pending -> Started -> Completed | Failed`, with
`Cancelled` reachable from `Pending`. A started task never becomes cancelled;
it drains to its declared outcome. Failure kinds are `Semantic` and
`Physical(u32)` where zero is rejected against the settlement convention that
zero means success. Per-task lifetime annotations are the closed pairs
`Sendable`/`NotSendable` and `Shareable`/`NotShareable`; they are recorded
projections of declared intent and imply no analysis of any real program.

Observable events are closed too: `started`, `completed`, `failed`,
`cancelled`, `scope_cancelled`, `finalized`, and `scope_exited` (carrying
`Success`, `Failed { task, failure }`, or `Cancelled`). Step precedence is:
drain the running task, announce newly cancelled scopes, cancel affected
pending tasks, start the smallest ready task, abandon permanently blocked
tasks, finalize in reverse completion order, then exit finished scopes
deepest-first with ties resolved to the canonical smaller identity.

## Deterministic traces and digests

Every run projects a canonical JSON trace bound to the model fingerprint and
the terminal root outcome:

```json
{"schema":"semaprax.scoped-tasks-trace.v1","model_fingerprint":"…","events":[…],"root_outcome":…,"first_failure":…}
```

Model and trace projections use separately domain-separated SHA-256
fingerprints (`semaprax.scoped-tasks-model-fingerprint.v1` and
`semaprax.scoped-tasks-trace-fingerprint.v1`) over length-prefixed bytes, so
identical logical models built in different input orders are byte-identical and
cross-domain digest confusion fails. These projections are test evidence, not a
wire format.

## Required executable evidence

`tests/cleanup_backends/scoped_tasks_model.rs` plus seven focused module units currently
cover:

- pinned known-answer trace digests for four canonical scenarios:
  - join-all: `c2c1ac40d3ce622bd1ac07984a88978dba13c79be2a568b9680943ccb07dbb91`
  - cancellation mid-scope:
    `98a5bf2f423a7a4d82f5edf2fb9f5374821e478b988871ef5a3635329c2d256b`
  - failure drain with abandoned dependent:
    `b51cf73d42c73a97bc71a1e54b791896d053cdd5bcf4e61bfac6c2de545c6f6c`
  - nested scopes with children-before-parents cleanup:
    `051da66037c3a17b8e58fda8f12902dcdc53f198e3c7ed1d464c65d239def03d`
- exact event sequences for stable-id scheduling, reverse-completion
  finalization, cancellation-before-new-work, running-work draining under
  cancellation, sticky failure beating late cancellation, and dependent
  abandonment;
- hostile construction rejections: escaping sibling-scope dependency, double
  join, orphan join, duplicate/self/cyclic dependencies, duplicate identities,
  multiple or missing roots, scope cycles, zero physical failure code, and
  scope/task/bound/work-budget violations;
- hostile run operations: unknown-scope cancellation, post-completion
  cancellation, effect-free repeated cancellation, premature `finish`, and
  quiescent stepping after completion;
- determinism under full input permutation (identical fingerprints, JSON,
  event vectors, and digests);
- byte-pinned canonical trace projection and JSON-validity parsing of both
  projections; and
- domain separation between model fingerprints and trace digests, including
  divergence between empty and complete traces.

These cases prove the bounded deterministic model only.

## Explicit nonclaims

Deterministic Scoped Task Model v1 adds no language syntax, no parser, HIR,
Graph, verifier, formatter, CLI, compiler-backend, or Wasm change, and no
runtime: no threads, fibers, executors, scheduler integration, timers, I/O, or
real concurrent execution of any kind. It executes no user code — task bodies
are closed scripted outcomes. It performs no `Sendable`/`Shareable` checking
of real programs, no aliasing or data-rules analysis, no deadlock or liveness
proof beyond the modeled drain barrier, and no timing, preemption, fairness, or
parallelism claim: the schedule is sequential by construction. It grants no
authority, mutates no source, claims no actors/reducers or synchronization
primitives, and satisfies none of the remaining completion-gate requirements
for the Structured concurrency row. No feature is "implemented" beyond this
proof data and its executable evidence.

## Current status

The model, its module units, and the integration suite are locally evidenced
under the standard quality gates. There is no public surface change, no
diagnostic family, and no backend behavior change. Any future runtime or
language integration must preserve the scheduling, cancellation, cleanup,
stickiness, and serialization rules above and must not cite this document as
evidence for execution semantics it does not implement.
