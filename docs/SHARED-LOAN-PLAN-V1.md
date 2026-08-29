# Shared Loan Plan v1

Status: bounded compiler proof contract with local executable evidence.

Audience: language users, tool authors, compiler contributors, and reviewers.

Shared Loan Plan v1 is the target-neutral proof artifact for the exact
synchronous immutable borrows already admitted by verified HIR. It gives each
loan an exact owner place, provenance, and path-sensitive lifetime so source
verification and HIR replay can reject an overlapping move, transfer, or
assignment while that loan is live. The plan is compiler-owned data: it
creates no runtime reference, cleanup action, capability, or authority.

This is the required foundation for later nested owned-aggregate borrowing.
It does not itself admit that feature.

## Closed admission

Version 1 attaches proof only to non-escaping shared immutable loans whose
ultimate verified root is move- or mutation-capable `own` storage within one
resolved function. A program whose candidates are all borrow-rooted
requires no ownership-conflict attachment and preserves its legacy Graph
schema. The borrowed value is used synchronously and cannot be returned,
stored in an aggregate, captured, exported, sent to another task, or cross a
function boundary except through an already admitted synchronous borrow
parameter whose result contains no borrow.

Every attached loan has a dense resolved-function-local identity
`0..loan_count` and an authenticated expression `site` recorded separately
from its control-flow start. Identities are assigned in canonical verified-HIR
discovery order, are not stable across source revisions, and are never
addresses, runtime handles, capabilities, or cleanup flags. A plan contains at
most **256 loans**.

## Owner places and provenance

Each loan names the exact verified owner `Place`: its resolved root value and
the canonical projection vector derived from HIR. The currently admitted
own-root loans are unprojected; projected borrowing from owned aggregate
fields remains closed. A root loan records no parent. A reborrow records the
exact live parent loan and preserves the ultimate owner place and provenance
chain. Parent identities must precede their children, every chain must be
acyclic, and a child cannot start before or end after its parent.

Version 1 accepts multiple simultaneous immutable loans. Equal-root sibling
loans and reborrows remain separate loans with separate uses and endpoints;
different owner roots remain independent. The overlap replay compares complete
places defensively, including prefix/descendant relationships, even though v1
does not yet admit projected owned-field loans. While any overlapping loan is
live, the verifier rejects assignment, unique transfer, `match own`, or move of
the owner place or an overlapping prefix/descendant. Shared
loans never make an unavailable owner available and never alter Cleanup
Inventory or CleanupPlan liveness.

Mutable loans, ownership through a borrowed/shared boundary, and inferred
projection into the still-closed nested owned aggregate profiles are not
admitted.

## Last use and path edges

A loan's `site` identifies the authenticated borrow expression, while its
`start` identifies the canonical Before program point where that loan becomes
live. These coincide for direct views and borrowed calls; per-arm `match
borrow` loans start at the arm or guard entry below their shared match site.
The lifetime ends after the last use on each reachable control-flow path, not
merely at the closing source brace. Branches can therefore produce several
termination edges for one loan. The plan identifies every edge on which a loan
stops being live and records
the destination program point as an end summary. A path with no later use ends
the loan at the earliest authenticated edge that preserves all observable
left-to-right and lazy-control-flow behavior.

`Try` and `TryOption` operand completion has two authenticated successors:
normal unwrapping and immediate residual return from the enclosing contract,
body, or postcondition root. A loan used later remains live only on the normal
successor and terminates on the residual-return edge.

The canonical `semaprax.loan-plan.v1` carrier records declaration-ordered
loans with their site, cause, origin, parent, start, end program points, and
exact terminating CFG-edge identities. Its `endpoints` vector contains the
canonical Before/After program points and records incoming/outgoing may-live
unions plus starts and kills; its `edges` vector carries the exact
path-specific live vector. Loan vectors use dense loan-identity order. Graph
projection preserves them exactly and must not sort, deduplicate, infer, or
repair them; execution backends consume no runtime loan carrier.

The plan contains at most **4,096 program points** and **4,096 CFG edges**.
Each canonical build, including the rebuild used for independent replay, has a
deterministic **1,000,000-work-unit** ceiling over examined HIR nodes,
control-flow edges, loan relations, and overlap checks. Exceeding any bound
fails closed before Graph projection or backend admission; the implementation
may not truncate, merge, sort, or repair the plan.

## Independent replay

Validated HIR does not trust an attached Shared Loan Plan. Independent replay
rebuilds the canonical plan from typed HIR and its verified control flow, then
requires exact agreement. It rejects, at minimum:

- sparse, duplicate, reordered, or out-of-range loan identities;
- a forged owner root, projection, provenance chain, or parent relation;
- a missing use, premature or extended endpoint, or an endpoint on the wrong
  path;
- a child outside its parent's live range;
- a move, assignment, or unique transfer overlapping a live loan;
- a missing or extra loan, endpoint, or edge.

The plan is descriptive proof data. Neither a serialized plan nor its digest
authorizes source changes, execution, memory access, or publication.

## Graph v23 and compatibility

A program carrying a nonempty authenticated Shared Loan Plan v1 selects
`semaprax.graph.v23`. Graph v23 projects dense loan identity, exact owner place,
parent provenance, and canonical path edges. Graph consumers must reject
v23 until they explicitly implement it; they must not silently read it as an
older schema.

Programs that require no Shared Loan Plan preserve their legacy Graph version,
bytes, Cleanup Inventory, and CleanupPlan schema and meaning. Shared Loan Plan
v1 does not widen CleanupPlan v6, Graph v22, the interpreter value model,
native C11 layout, Core-Wasm layout, or any public ABI.

Semantic Workspace v1 rejects a module that simultaneously requires the
owned-variant Graph v22 base schema and a nonempty Shared Loan Plan. Graph v23
must not mask an unsupported v22 base contract at either the source-schema or
change-view boundary; a later combined schema requires its own specification
and evidence.

## Executable evidence

The local evidence gate owns:

- canonical dense identities and deterministic plan/Graph v23 fixtures;
- exact unprojected owner roots, direct loans, parent reborrows, and multiple
  equal-root shared loans;
- straight-line and branch-specific last-use endpoints, including a move that
  becomes legal only after every overlapping path has ended, plus the distinct
  normal and residual-return successors of `TryOption`;
- rejection of premature moves and transfers plus hostile mutations across
  loan, endpoint, edge, ordering, and omission surfaces;
- the exact 256-loan boundary and 257-loan rejection, plus proof that a large
  loan-free function does not acquire the plan's CFG limits;
- deterministic resolved-HIR fixtures at exactly 4,096 program points and
  exactly 4,096 CFG edges, independent canonical rebuild at both boundaries,
  hostile boundary-carrier reorder rejection, and isolated first-overflow
  diagnostics (the first representable point overflow is 4,098 because every
  expression contributes paired Before/After points);
- one real 256-loan canonical fixture whose disconnected typed Boolean roots
  are composed from independently measured production-planner charge deltas
  to consume exactly 1,000,000 work units, plus exact replay, a limit-minus-one
  rejection on the final unit, and a production-limit overflow that stops at
  unit 1,000,001; and
- preservation of Graph v22 selection with no loan carrier when no plan is
  needed, while the existing cleanup-plan gates remain the cleanup authority.

The dedicated exact-boundary fixtures are authored in the current source tree
but were not executed by this implementation audit. Their source presence is
not local-green or hosted evidence; promotion remains contingent on executing
the focused gate at the exact claimed commit.

Interpreter, native, and Wasm regression gates must continue to prove that
admitted programs retain identical observable behavior, but they consume no
runtime loan object and establish no borrowed ABI. Evidence described here is
local repository evidence only; hosted promotion is explicitly unclaimed.

## Non-claims

Shared Loan Plan v1 does not claim mutable borrowing, general lifetime
inference, escaping borrows, borrow-valued results, closures, async or
concurrent loans, cross-file or cross-task lifetimes, regions, arenas,
retain/release, ARC, raw pointers, foreign ownership, or a public/native/Wasm/
Component ABI. It does not admit nested owned aggregate borrowing, generalize
`match borrow`, execute cleanup, prove process memory safety outside verified
HIR, or complete either ownership row in the completion matrix.
