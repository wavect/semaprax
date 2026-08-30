# Project Candidate Body Holes v1

Audience: agent client authors and compiler contributors.

Status: authored, unrun; a bounded addition to the
[full graph-operational programme](GRAPH-OPERATIONAL-PROGRAMME.md), not general
incomplete-program compilation or completion of that programme.

`ProjectCandidateDraft` holds an immutable private last-valid
`Arc<ProjectCandidate>` and a bounded map of pending typed body intentions.
There is no placeholder AST, invalid HIR, source edit, disk cache, or hidden
compilation state. A pending hole says that the selected function body remains
to be supplied; it does not pretend that the old body is the intended new body.

## API and lifecycle

All fallible APIs return `Result<_, Vec<Diagnostic>>`:

```rust
ProjectCandidateDraft::open(candidate: Arc<ProjectCandidate>) -> Self
with_body_hole(&self, expected_draft: &str, target: &str, hole_id: &str) -> Self
hole_context(&self, expected_draft: &str, hole_id: &str) -> String
fill_hole(&self, expected_draft: &str, hole_id: &str, expression: &Value) -> Self
complete(&self, expected_draft: &str) -> Arc<ProjectCandidate>
summary(&self, expected_draft: &str) -> &str
```

`draft_digest()` and `to_json()` borrow the digest and canonical report.
`open` starts with no pending holes. Adding or filling a hole returns a new
draft; the original and any siblings remain unchanged. There may be at most
16 unresolved holes, with one hole per target and unique hole IDs. A target
must be an existing explicit, monomorphic, non-main function accepted by the
underlying body-replacement route.

Hole IDs contain 1–128 ASCII letters, digits, `_`, `-`, or `.`. Target IDs are
at most 4,096 bytes. Every operation after open requires the exact current
draft digest. Reusing a hole name in another draft does not reuse its bound
reference. Context returns a deterministic hole handle binding draft digest,
hole ID and target; filling uses the equivalent explicit draft-digest plus
hole-ID selection and does not accept a bearer capability.

`fill_hole` bounds the supplied typed expression before cloning, creates the
existing `replace_function_body` Semantic Change, and invokes the ordinary
Candidate apply path. That path materializes canonical source privately,
reparses and rebuilds the entire Project, revalidates ownership/loans/cleanup,
checks mandatory identity/contract/effect/profile constraints, and preserves
previously admitted bounded core targets. These are compiler admission steps,
not project-test or target-runtime execution. Constructor and admission
failures preserve the original draft and return their existing diagnostics.

A successful fill removes only that hole. Other holes remain unresolved over
the new last-valid revision, and their old draft-bound selectors become stale.
Context for a remaining hole is regenerated against the new last-valid HIR.
Discarding a draft means dropping it; no rollback or source operation occurs.

## Unresolved holes cannot materialize

The draft exposes no candidate, revision, source, diff, evidence or commit
accessor. `complete` is the only candidate-release method and rejects while
any unresolved hole remains. After all fills, it returns the last fully
validated candidate. The resulting candidate still has no filesystem or
publication authority.

Draft reports use `semaprax.project-candidate-draft.v1`, contain only pending
hole summaries and clearly named `last_valid_revision` and
`last_valid_candidate_digest` facts, and always say `materializable: false`.
They never label the incomplete state `candidate_revision` or include
replacement source/source diff/candidate evidence. An empty draft says
`ready_to_complete`; consumers still call `complete` to obtain its candidate.
The previously supplied valid candidate may remain independently held by its
caller, but that object does not represent the incomplete draft.

## Typed context

The additive [Expression Holes v1](PROJECT-CANDIDATE-EXPRESSION-HOLES-V1.md)
extends the same draft with disjoint authored expression selections, including
local lexical scope and selector remapping after fills. Body-only report bytes
and context remain unchanged. The sixteen-hole bound is shared across both kinds.

`hole_context` returns `semaprax.project-candidate-hole-context.v1` containing:

- exact draft, hole, function, source revision/path/module and expected return
  type identity;
- actual parameter names/IDs, type identities and ownership modes in scope;
- existing allowed effects, module permits, and the rule forbidding undeclared
  effects; these facts grant no capability authority;
- existing requires/ensures predicates through the compiler's Graph contract
  renderer;
- stable-ID calls with one local or authenticated import binding, resolved
  parameter/result types and ownership, effect requirements and whether those
  effects fit the selected function's budget;
- complete prior-body LoanPlan and CleanupPlan projections, explicitly marked
  `last_valid_body_not_the_unfilled_hole`;
- the obligations to satisfy return type, contracts, parameter ownership,
  loans/cleanup, effects/capabilities and existing profile/core-target admission.

The accessible-call list describes lexical bindings, not a guarantee of
admissibility for arbitrary arguments. Multiple aliases, unbound declarations,
generic templates and compiler prelude conveniences are not invented as
constructible calls. Recursion, argument typing/ownership and all other
restrictions are checked by fill admission. Calls outside the effect budget
are labeled rather than presented as already permitted.

Prior-body loans, local storage and cleanup vectors are context for replacing
that body. They are not inferred obligations of a nonexistent expression or
proof that an unfilled hole is valid. No new liveness, destruction order,
capability or runtime effect is inferred. Successful fill rebuilds the actual
new body's proof attachments.

The initial constructor surface is the existing scalar `i64`, `i32`, `u8`,
`usize`, `bool`, parameter `place`, stable-ID `call`, `unary`, `binary` and `if`
forms. No nested expression holes, local-binding synthesis, resource
constructors, arbitrary declaration constructors or solver-backed completion
is added. A context can describe a richer function while its chosen filling
expression still must fit the bounded constructor and pass real admission.

The additive [aggregate constructors](PROJECT-AGGREGATE-CONSTRUCTORS-V1.md)
extend fills with stable-ID record and variant values, explicit direct-scalar
generic arguments, and authenticated Option/Result cases. Contexts expose
checked template parameter/field identities and compiler-prelude provenance;
they do not infer generic arguments or claim that a template instance is valid
for this hole. Existing monomorphic descriptor shapes are preserved, but whole
context bytes change when the four compiler-prelude cases are added.

## Bounds, canonicality and diagnostics

Canonical reports sort JSON object keys lexically, preserve array order and
include one terminal LF. The draft digest hashes the exact report bytes with
domain `semaprax.project-candidate-draft.v1` followed by NUL and a u64
little-endian byte length. Hole-handle hashing uses
`semaprax.project-candidate-hole-handle.v1` followed by NUL and the same byte
framing around canonical JSON containing draft, hole and target.

Draft and context reports are capped at 1 MiB. Existing individual compiler
contract/loan/cleanup renderers run under a 16 MiB rendering budget; accessible
call bindings are capped at 1,024. These are wire and per-renderer limits, not
an aggregate heap or proportional-query-cost guarantee. Oversized context
fails instead of silently omitting obligations. Typed input retains the
existing Semantic Change node, depth and byte bounds; Candidate's existing
maximum intention count also limits successful fills.

| Code | Meaning |
| --- | --- |
| `SPX-G230` | Invalid/duplicate/missing hole ID, unavailable target, or invalid compiler-owned context projection. |
| `SPX-G231` | Pending-hole, ID, report, call-inventory, expression-input or renderer capacity exceeded. |
| `SPX-G232` | Stale/invalid draft selector or attempted completion with unresolved holes. |

Ordinary Semantic Change, source, HIR and target admission diagnostics remain
unchanged when a typed fill reaches the existing Candidate route. Draft bytes
are descriptive reports, not an accepted deserialization or authorization
format. They do not add persistence, rebase or publication authority.

## Authored evidence

[Integration evidence](../tests/project_candidate_holes_v1.rs) covers typed
scope/contracts, no incomplete materialization or source/evidence leakage,
multiple pending holes, failed-fill immutability, local/import call selection,
stale and duplicate selectors, exact capacity, cross-root determinism, and
unchanged source files. These tests and compiler/quality gates were not run at
the user's request. Full programme completion and runtime/hosted guarantees
remain unclaimed.
