# Rooted record String views v1

Status: source implementation and owning regressions authored for OPT-728;
execution, public qualification and application adoption are pending.

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

The following regression sources are staged, not executed:

- `--lib hir::projected_string_view::tests::` (4): canonical source, exact
  graph/cache replay, full loan paths, field/root/origin forgeries, temporary
  roots, active-loan movement, inline grouped arguments and frozen profiles.
- `--lib interpreter::projected_string_view::tests::` (1): repeated nested
  borrowed views at the existing logical String-allocation and byte ceilings,
  with room only for the two authored literals.
- `--test owned_data projected_string_views::` (2): identical source on the
  interpreter, native O0/O2 and strict Core Wasm; nested owned/borrowed roots,
  NUL and Unicode, loops, aliasing, owning transfer after last use, physical
  clone refusal, and repeated settlement on success, later-argument failure,
  callee failure and range failure. Existing host stale-carrier and ownership
  validation remain enabled.

The ordinary full quality profile remains required for product qualification.
The response generator retains its named borrow-match workaround until these
gates qualify. Catalog23 and other original application requirements and
strong TypeScript baselines must remain unchanged. The observed 773-byte
workaround increase is an authored-source observation; it proves no token,
turn, latency, billing or accepted-task-cost improvement. Fresh matched live
campaigns own those claims separately.
