# Rooted record String views v1

Audience: coding agents, compiler contributors and reviewers.

Status: focused source, replay and backend gates passed on macOS arm64.
The response emitter and Catalog publication use the rooted views.
Repository-wide qualification and matched agent trials remain pending.

## Meaning and admission

`string_as_str(value.inner.label)` may borrow an existing String leaf from a
named, live owned or borrowed record root. Every projection is an ordinary
record field; every traversed monomorphic, invariant-free record and field has
an explicit stable identity. The complete root must already fit the ordinary
String-record or nested collection-record runtime. Its current target and
Project profile must independently admit the carrier. This adds no record
shape, public ABI, capability, provider permission or generic-template shape.

The result is the existing nonescaping `borrow str`. The operation neither
copies the String nor transfers or clears any owning field. The exact fused
composition `str_as_bytes(string_as_str(value.inner.label))` produces the
existing read-only Slice view of the same field. Direct `str_as_bytes` on a
String remains ill-typed. Named unprojected views keep their prior rules.
Temporary constructors, ordinary call-result roots, variant projections,
implicit or stale field identities, unavailable owners and wrong leaf types
remain refused. There is no stored, returned, mutable or escaping borrow.

The same rooted view may be formed inside a loop. This admission applies to
the view operand, not to general nested-record construction, mutation, moving
or ordinary String-copy expressions in loops. No other loop profile widens.

## Lifetime and authority

Source verification derives the complete root and field-name path from the
authored expression. Independent HIR verification resolves the actual live
binding and authenticates each stable field ID, declaration owner, field
position, declared type and explicit identity. A cached result type or path
cannot substitute for this proof. Source errors retain SPX-T266 for an invalid
storage expression and SPX-T265 for a move overlapping a live view; malformed
retained HIR fails with SPX-H006.

BorrowPlace retains the full `ValueId` and field-ID path. A local view, aliases
and reborrows protect that exact path until its last use. Parent or overlapping
field movement is forbidden during the loan. A distinct sibling remains
independent. An inline borrowed argument protects its root through the ordinary
left-to-right argument staging and grouped call commit. A borrowed record
parameter retains the caller's synchronous loan and cannot mint ownership.
It needs no fabricated local owning-root loan attachment.

Existing SharedLoanPlan rebuilding derives the same path and end edges;
cleanup inventory and canonical cleanup plans retain the sole String owner.
Result publication follows ordinary postconditions and non-result cleanup.
Failure during later argument evaluation or inside the callee leaves the
first selected status intact and settles the original owner exactly once.

The existing depth 64, field-visit 4096, owned-leaf 256, loan, byte, UTF-8,
allocation, fuel and builder bounds remain unchanged. Old v27/v29/v30 carrier
restrictions remain unchanged: a Vec-bearing nested root still requires its
separately selected v31 route. No codec, generated name or origin gains an
exemption.

## Representations and replay

Native C11 reads the authenticated canonical String leaf slot or borrowed
parameter alias, and forms the existing length-aware `spx_str_v1`. It never
loads an inert record shell's owning field or invokes ordinary String cloning.
Frozen terminated profiles retain their existing semantics and caps.

Core Wasm reads the existing String carrier at the authenticated field path
and uses the ordinary read-only arena view boundary. It does not allocate,
clone, renew or settle an owner. The opaque standalone internal-String route
retains its existing refusal unless its ordinary authenticated conversion
route is available.

The interpreter follows the field path by reference before constructing its
abstract immutable borrowed representation. It bypasses `clone_value` for the
String leaf and incurs no logical String materialization. Its internal Arc
representation is not evidence of physical allocation equivalence.

Graph v73 versions the projected String BorrowPlace and fused Slice provenance.
For the latter, `owned_string` denotes the terminal owning String storage;
the root may be its enclosing owned or borrowed record and the full path is
retained. It grants no move permission. The marker
`semaprax.projected-string-views.v1` has `authority:false`. Programs without
these views preserve earlier graph schemas and bytes. Retained HIR, cache,
source and loan replay remain mandatory; older graph and evidence routes fail
closed rather than interpreting the new path meaning.

## Executable obligations

The five HIR and two interpreter tests passed for canonical source, exact
graph/cache replay, full loan paths, field/root/origin forgery, temporary
roots, active-loan movement, grouped arguments, and the existing String
materialization limits. The shared-loan source suite passed all 14 cases,
including overlapping assignment and movement after the view's last use.
Both `owned_data::projected_string_views` cases passed on the interpreter,
native O0/O2 and strict Core Wasm. They exercised NUL and Unicode, repeated
borrowed reads, physical clone refusal, and first-status and owner settlement
on success and failure.

The response emitter forms views directly from authenticated String fields.
Catalog's `publish` borrows `parts.body` and `parts.trailer` for its two
fragment calls. The unchanged Catalog23 corpus passes on the typed v31
command. Its owning Project test also checks that both field loans are rooted
in the original `ReportParts` parameter and have recorded end edges.

The repository-wide quality run stopped on the existing rustfmt drift in
`src/source_reflows/ci_inputs/successor_fronts.rs`. Fresh matched live trials
remain required to measure authored tokens, turns and accepted-task cost.
