# Source-checked typed-hole fill suggestions v1

Status: implementation and regression cases authored, unrun.

Audience: semantic agent clients and compiler contributors.

`ProjectCandidateDraft::hole_fill_suggestions(expected_draft, hole_id)` derives
bounded expressions from the selected hole's existing compiler context and
tries each through ordinary `fill_hole`. It returns only proposals for which
complete source replay succeeds. Every attempt starts from the same original
immutable draft; the temporary result is dropped. No proposed fill is selected
for the user, retained in a session registry or published.

This supplies concrete checked proposals beyond the lexical call inventory.
It does not solve the intended program or prove a runtime contract. An admitted
expression can have different behavior from the old expression.

## Proposal grammar and order

Existing context owns lexical scope, stable callable identities, expected type
and effect budget for body, body-expression and contract-expression holes.
Neither that context nor the four compact navigation facets change shape.
The finite search grammar is:

1. A `place` for each lexical binding whose exact type identity matches the
   expected type, in existing scope order.
2. A direct `call` for each accessible monomorphic callable with the exact
   expected return type and effects within the hole's budget, in existing call
   order. Arguments are same-type lexical places, enumerated in parameter order
   over the scope's ordered choices.

The enclosing function is excluded as a direct callee. No literal defaults,
source fragments, nested calls, builtin calls, projections, aggregates or
placeholder arguments are invented. Explicit fill retains its broader grammar.
Names are not ranked as semantically preferable; same-shaped nominal types
with different identities are not interchangeable.

Type and effect matching are prefilters. Scope is not a liveness proof, and
matching a parameter type does not prove that a value can be moved twice,
borrowed here or used after consumption. Normal fill owns source, type,
ownership, loan, cleanup, contract-form and target-profile admission, including
remapping the other pending holes. No admission rule is widened for discovery.

## Report and use

The closed schema is `semaprax.project-hole-fill-suggestions.v1`:

| Field | Meaning |
| --- | --- |
| `draft_revision`, `hole_id` | Exact original draft and pending hole |
| `context_revision` | Digest of the full hole context, including its final LF |
| `last_valid_revision`, `expected_type_id` | Existing source revision and checked type identity |
| `considered`, `rejected` | Actual fill attempts and ordinary rejected attempts |
| `search_exhausted` | Whether the finite grammar above was exhausted within the limit |
| `suggestions` | Ordered accepted proposals, each with `expression` and `preview_draft_revision` |
| `validation` | `ordinary_fill_source_replay` |
| `tests` | `not_run` |
| `source_authority`, `draft_retained` | Both false |
| `nonclaims` | Fixed exclusions of intent correctness, runtime contract proof, complete expression search and liveness inference |

The context digest uses the existing `semaprax.project-hole-context.v1` domain,
NUL, the payload's u64 little-endian byte length and exact context bytes. It
agrees with `hole_summary` for the same draft/hole. It is provenance, not
authority to reuse a report against changed source.

`preview_draft_revision` identifies a temporary result for correlation and
independent replay, not a registered draft handle. To choose a suggestion,
submit its exact expression through ordinary `hole/fill` with the original
draft/hole selectors; normal replay occurs again. Other holes remain pending,
and completion and publication retain separate boundaries.

`considered` equals `rejected` plus the suggestion count. All ordinary fill
failures count, including conservative admission and capacity failures. Empty
suggestions do not prove that no valid fill exists. `search_exhausted: true`
applies only to the defined finite grammar. Successful preview does not prove
runtime preconditions, postconditions, termination, tests or desired behavior.

## Bounds and authority

At most 32 fill attempts run across all place and call proposals. The next
proposal can be inspected without filling it to distinguish exhaustion from a
limited prefix. Cartesian combinations are enumerated incrementally; the
complete product is never allocated or counted. Scope is bounded to 16,384
entries, calls to 1,024, and each signature to 64 parameters. Type-indexed scope
options avoid complete-scope scans for each combination. Existing 1 MiB context
and source/constructor bounds remain. The complete report is bounded to 64 KiB
and fails rather than truncating expressions, successful proposals or fields.

Existing stale draft/hole diagnostics retain their owners. Malformed compiler
context uses `SPX-G230`; metadata and report capacity use `SPX-G231`. Per-proposal
fill failures are counted as rejected, never promoted to admitted suggestions.
The engine invokes no interpreter, generated target, external tool, network or
source publication. The attempt limit bounds replay count, not total CPU,
heap, stack or latency. Small output does not establish reduced compiler work
or model-token savings.

## Protocol and evidence

Candidate-enabled v5 sessions expose `hole/fill-suggestions` with exactly
`image_revision`, `draft_revision` and `hole_id`. Requests cannot select a wider
grammar, compiler profile or execution policy. The pure handler takes a detached
immutable draft through the existing parallel-read allowlist. The complete live
request or batch remains inside source authentication, without registry mutation.

Selected discovery bundles the closed report and finite place/call grammar.
TypeScript, Python and Rust clients and MCP use the same method/schema selection.
These shapes do not replace ordinary fill validation.

`tests/project/hole_fill_suggestions.rs` and
`tests/image_v5/hole_fill_suggestions.rs` author replay, parent retention, source
preservation, bounds, stale selection, ownership and transport boundaries.
They have not been executed. Runtime contracts, actual client execution,
representative tasks and measured improvements remain outstanding; no
completion row is promoted.
