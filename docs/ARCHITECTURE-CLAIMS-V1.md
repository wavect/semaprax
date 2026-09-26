# Architecture Claims v1

Status: initial slice. Two operators, `forbid_reaches` and
`protocol_realizers_bound` (issue #297, see
[`protocol_realizers_bound`](#protocol_realizers_bound-issue-297)), are implemented,
locally tested, and derive their results directly from checked facts. The
remaining operators [issue #205](https://github.com/wavect/semaprax/issues/205)
lists (unique/bounded writers, effect/capability attribution, dependency
direction, deployment facts, authorization mint/consume), CLI/MCP wiring,
candidate-review integration, and Assurance Manifest recording are **not**
implemented and are not claimed as done anywhere in this document.

Audience: compiler contributors and reviewers of checked-fact-derived
documentation and tooling.

An architecture claim asks a question about the checked program, not about a
diagram someone drew. For one exact [`ProjectRevision`](../src/project/mod.rs),
the evaluator returns `held`, `violated`, or `unevaluable`, plus a small
witness. It derives call edges from checked HIR on each evaluation, so a
source change cannot leave a stale hand-authored graph looking valid. This is
the first bounded slice of [issue #205](https://github.com/wavect/semaprax/issues/205).

## What is implemented

Only `forbid_reaches(from, to)` exists today. It asks whether `from` can
reach `to` through static calls in the exact revision's entry, public-API,
and test HIR programs. It is not a general effect or authorization checker.

The implementation is owned by [`src/architecture_claims.rs`](../src/architecture_claims.rs)
and exported as `semaprax::architecture_claims`:

```rust
pub const ARCHITECTURE_CLAIM_SET_RESULT_SCHEMA: &str =
    "semaprax.architecture-claim-set-result.v1";

pub struct ArchitectureClaim { /* opaque */ }
pub struct ArchitectureClaimSet { /* opaque */ }
pub struct ArchitectureClaimSetResult { /* opaque */ }
```

`ArchitectureClaim::forbid_reaches(id, from, to)` validates the claim id and
the two declaration references and rejects a reflexive claim (`from == to`)
at construction. `ArchitectureClaimSet::new(claims)` validates a bounded,
duplicate-id-free collection. `ArchitectureClaimSet::evaluate(&revision)`
builds the call graph once from `revision`'s checked HIR and evaluates every
claim against it, returning one `ArchitectureClaimSetResult` whose `to_json`
is a compact, recursively key-sorted, size-capped, newline-terminated
document bound to `revision.project_revision()`.

## No caller-authored edges, no competing graph

The call graph is derived, once per evaluation, directly from the exact
`ResolvedProgram`s a `ProjectRevision` already retains — the same
`ResolvedFunction`/`ResolvedFunctionTemplate` bodies
[`declaration_consumers`](UNIVERSAL-SEMANTIC-QUERY-V1.md) walks for its own,
differently-shaped, "who calls this declaration" fact. Nothing in this module
accepts a caller-supplied edge list, and nothing here copies facts into a
second index that could silently drift from the checked HIR: every claim
result changes only when the bound `project_revision` changes.

Edge extraction is an iterative (never recursive — a recursive walk over
deeply nested expressions has previously stack-overflowed elsewhere in this
compiler on a default-stack debug build) walk of every `ResolvedExprKind`
variant with **no wildcard arm**. A future HIR expression kind that can carry
a nested call must be judged in that match explicitly; the build fails
instead of the walker silently treating an unrecognized shape as "no edge."

Three expression kinds carry meaning for this operator:

- `Call { callee, .. }` is a statically known call to another declaration in
  this revision's programs. It becomes a fully traversed graph edge.
- `NativeRustImportCall` names a statically known target id, but that target
  is foreign code with no retained HIR body in these three programs.
  Reaching it is a known fact (it can be the witness target `to`), but what
  it does next is not, so it is never treated as a safe dead end while a
  claim's status is still undecided.
- `Invoke { callable, .. }` calls a *computed* callable value (dynamic
  dispatch over a function reference or closure value). Its concrete target
  is never statically known here, so any node performing one taints the
  whole claim `unevaluable` unless a definite violation was already found
  elsewhere in the reachable closure.

`HostCommandCall` is deliberately **not** modeled as a call-graph edge in
this slice: it is a closed, enumerable host primitive with no further
reachability into program code by construction (the capability boundary
model already separates host effects from program call structure).
Attributing host effects is the distinct, unimplemented `require_all_effects_attributed`
claim kind the issue lists, not `forbid_reaches`.

## Deterministic minimal witness

Adjacency is built into `BTreeMap`/`BTreeSet`, so neighbor expansion order is
fixed by declaration id alone, independent of HIR traversal or hash-map
iteration order. Reachability is a breadth-first search over that fixed
order, so:

- a `violated` result's `path` is a shortest static-call path from `from` to
  `to`, and is the same path across repeated evaluation of equivalent input
  even when more than one shortest path exists (a diamond graph is exercised
  directly in the unit tests);
- an `unevaluable` result's `frontier` lists every node in the *reachable*
  closure that carries a dynamic-invoke or unresolved-external-target fact,
  in sorted stable-id order.

A definite `violated` witness always takes priority over an unrelated
`unevaluable` branch elsewhere in the same closure: a reachable dynamic
dispatch that has nothing to do with the concrete counterexample already
found does not downgrade a real violation into "not sure."

## Fail-closed capacity, not a silent pass

The per-declaration HIR walk is bounded by `MAX_ARCHITECTURE_CLAIM_HIR_WALK`
(65,536 expression nodes) and the graph reachability search by
`MAX_ARCHITECTURE_CLAIM_GRAPH_WALK` (65,536 visited declarations). Exceeding
either is a hard `SPX-AC602` error from `evaluate`, never a `held` result
reported under a truncated search. A focused unit test evaluates a claim
under an artificially tiny bound to prove this fails closed rather than
returning a shortened, falsely confident verdict.

## Result shape

The rendered result schema is `semaprax.architecture-claim-set-result.v1`.
Its exact top-level keys are `claims`, `limits`, `nonclaims`,
`project_revision`, `schema`. Each entry in `claims` (sorted by `claim_id`)
carries a fixed field set regardless of status: `claim_id`, `operator`,
`from`, `to`, `status` (`"held" | "violated" | "unevaluable"`), `path`
(populated only when `violated`), and `frontier` (populated only when
`unevaluable`). `nonclaims` states plainly that this is a derived read-only
projection with no source execution or publication authority, that absence
of a static edge is not proof against reflection or dynamic behavior outside
the admitted profile, and that only static direct-call and native-import
edges are modeled.

For `protocol_realizers_bound`, each claim entry instead carries `claim_id`,
`operator`, `protocol`, `protocol_name`, `attests` (always
`"via_targets_are_checked_call_graph_nodes_not_ordering"`), `via`, `reason`, `authority` (always `"none"`), and `status`. Existing
`forbid_reaches` entries and the top-level keys and `nonclaims` are unchanged.

## `protocol_realizers_bound` (issue #297)

`ArchitectureClaim::protocol_realizers_bound(id, protocol)` names one
declared `.spx` `session protocol` by its persistent `@id`
([Session/protocol types v1](SESSION-PROTOCOL-TYPES-V1.md#declared-session-protocols-issue-297)).
It attests **only** that every `via` target of that declaration is a checked
function node of the evaluated revision's direct call graph. It does **not**
attest message order, call order, that any caller follows the protocol, or
that a realizer is ever called, and it grants no capability, effect, or
execution authority.

The evaluator reparses only the revision's authenticated sources that contain
the `session protocol` keyword pair, and only when the set contains this
operator. It runs the same `SPX-K1xx` source checks the single-file verifier
applies, then checks each `via` edge against the node set of the same call
graph `forbid_reaches` walks. The caller supplies only the protocol identity.

- `held`: at least one `via`, and every `via` target is a checked node.
- No `violated` outcome is reachable from built source, and none is
  reported. A build refuses a `via` naming no function of the declaring
  module (`SPX-K104`), project modules are never pruned, and bundled-dependency
  pruning keeps every `via` target, so every `via` target of a built revision
  is a checked node -- even one nothing calls or exports
  (`an_uncalled_unexported_via_target_is_still_a_checked_node`). A target
  absent from the evaluated graph would mean an inconsistent revision and is
  refused (`SPX-AC601`), never reported as a claim outcome.
- `unevaluable`: `reason: "no_via_binding"` when the declaration binds no
  `via`; `reason: "duplicate_session_protocol_identity"` when two retained
  sources declare the same identity. Only claims naming that identity are
  affected; the rest of the set still evaluates.
- Refused (`SPX-AC601`): the protocol identity is not declared in the
  revision, a retained declaration fails its source checks, or a `via`
  target is absent from the evaluated graph.

The result is bound to `project_revision` like every other claim. Project
Assurance Manifest v1 records a held claim as an `architecture_law`
obligation with locator `architecture:protocol_realizers_bound:<claim-id>`
keyed to the protocol's `@id`; its method detail states the same realizer-only
scope. There is no CLI flag for this operator yet.

## Why staleness is loud here, not silent

The rendered result is bound to `revision.project_revision()`. An
integration test in the `tests/workspace` harness
([`tests/workspace/architecture_claims.rs`](../tests/workspace/architecture_claims.rs))
proves this is not merely asserted: it evaluates the identical claim over
two revisions of the same source module that differ only in whether one
function calls another, and shows the result flips from `held` to
`violated` with the expected witness path, while the two revisions' bound
`project_revision` digests are never equal. A claim result cannot be
mistaken for the same fact re-read twice after the underlying checked
behavior changed.

## What is not implemented

- The other operators #205 lists: `require only`, `require exactly_one`,
  `require all effects attributed`, `bound calls/effects`, layer/dependency
  direction, package/service/resource access, Agent authorization
  mint/consume constraints, and deployment/topology facts.
- CLI and MCP surfaces. `ArchitectureClaimSet`/`ArchitectureClaim` are Rust
  library types only; no `semaprax` subcommand or MCP method calls them yet.
- Candidate-review integration (new/resolved violations shown before
  apply/publication) and Assurance Manifest v1 recording of claim results and
  assumptions.
- Deployment/topology facts; this slice only sees the three HIR programs a
  `ProjectRevision` retains, not `DeploymentRoot` or capability-manifest
  facts.

None of the above is asserted as done by this document, the module's own
doc comment, or its tests.
