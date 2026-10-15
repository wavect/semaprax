# Scoped Record Row Loan v1

Status: source implementation batch for OPT #775; executable qualification is
pending. No owning gate has been executed for this tranche and no performance
or allocation result is claimed.
Audience: coding agents, compiler contributors and reviewers.

This additive contract extends
[Scoped Record Vec Traversal v2](SCOPED-RECORD-VEC-TRAVERSAL-V2.md). Programs
without a row loan keep their exact source, HIR, loan plan, cleanup plan,
graph schema, cache payload layout and native/Wasm bytes. Public capability,
command profiles and the public owned-data ABI are unchanged.

## Source shape

```
@id("app.weight") fn weight(row: borrow Row) -> i64 { row.score + 1 }

for borrow row in rows {
    total = total + weight(row);
    0
}
```

Inside `for borrow row in rows`, the bare row name is admitted as a value in
exactly one position: a direct argument of an ordinary call whose callee is a
monomorphic source function (local, or imported with `use function`) and whose
parameter at that position is declared `borrow R` for the row's exact record
`R`. `R` must be a flat owned-leaf record (at least one String or Bytes field);
Copy-record rows keep their by-value parameter contract and stay refused.

Everything else stays `SPX-T284`, including `let kept = row`, `own R`
parameters, generic, method, intrinsic and function-value targets, entry points
and `yields` functions, rows inside closures, and a helper whose body could
store, return, capture, forward, view, rebind or otherwise escape the row. The
existing escape message is unchanged for every pre-existing refusal; a helper
that fails the body proof reports "record traversal row can only be lent to a
helper that reads Copy scalar fields of its borrowed parameter".

## Helper proof

The helper is the single authored, effect-free function, compiled once. Its
result must be one of the eight Value Copy scalars listed below; owned, borrowed
view and nominal results are outside v1. Contracts and checked scalar failures
retain their ordinary behavior. Its lent parameter
is independently proven, from its own body, `requires` and `ensures`, to occur
only as `parameter.field` reads of Copy scalar fields (`i64`, `i32`, `u8`,
`usize`, `char`, `f32`, `f64`, `bool`) outside any closure. A scalar result
type alone is never sufficient. String/Bytes field views, whole-parameter uses,
`match borrow`, captures, assignments and passing the parameter onward
(including recursive forwarding) refuse. The source verifier checks this when
the helper body is visible; complete HIR validation re-proves it for every
callee, including imported helpers, from the current linked HIR. Names,
digests and cached flags carry no proof authority. A lent helper may not also
be referenced as a function value.

## Descriptor

HIR represents the argument as `ResolvedExprKind::RowLoan`, an explicit
private descriptor with additive owning version `semaprax.row-loan.v1`:

| Field | Meaning |
| --- | --- |
| `version` | `V1` only. |
| `element` | Exact nominal flat owned-leaf record. |
| `traversal` | The lowered `for borrow` region whose freeze protects the call. |
| `bound` | That region's immutable length snapshot. |
| `callee` | The ordinary monomorphic callee. |
| `parameter` | The callee's exact `borrow R` parameter identity. |
| `args` | Exactly the whole-Vec place and the row-index place, in order. |

Validation replays every descriptor against its direct call slot, the
innermost traversal region reconstructed by the freeze proof, the region's
owner place, element, bound and row index, the callee's parameter identity,
ownership and type, and the helper proof. An unbound, forged, stale, displaced
or nested descriptor fails closed. Resolution produces descriptors only for
argument slots; only an ordinary call binds them.

## Loans, evaluation and failure

The traversal freeze keeps the complete Vec owner and generation frozen for
the whole loop region. The receiving call additionally holds a
`BorrowedCall` loan of the whole Vec origin from before the call to after it,
covering later argument evaluation, the helper body, its contracts and
canonical failure cleanup. The call loan ends after that call. The traversal
freeze remains through the whole loop region, including its latches and subsequent iterations, and ends
only when the traversal exits; call completion does not authorize Vec mutation. Arguments evaluate left to right; the Vec place is
inspected before the index. The loan owns no storage, status source or cleanup
transition; canonical cleanup vectors are unchanged and never reordered.

## Backend carriers

No backend materializes, clones, moves or drops a row, record or leaf.

* Interpreter: once, while staging the argument, the Vec carrier identity,
  generation, bounds and row authentication are checked; the helper parameter
  then aliases the exact frozen row by reference. Field reads inside the
  helper are ordinary record reads with no per-read generation check; their
  soundness rests on the validated traversal freeze and call loan, which
  statically exclude every Vec mutation for the complete call.
* Native C11 (`semaprax.native.borrowed-record-carrier.v1`): a lent parameter
  receives `const spx_row_loan_v1 *` in place of the ordinary record pointer
  and hidden leaf pointers. Tag 1 names a resident record (every ordinary
  caller adapts), tag 2 names the frozen Vec carrier, row index and captured
  generation. Each read checks the carrier and generation; tag-2 reads use the
  authenticated `spx_leaf_read_field` operation, and any refusal is a runtime
  invariant failure.
* Core Wasm (`semaprax.wasm.borrowed-record-carrier.v1`): the parameter stays
  one i32 pointer, so ordinary callers and external wrappers keep their exact
  ABI. A row loan passes a caller-frame `{owner: i64, index: i64}` block with
  bit 31 set; reads of that tag call the existing private Vec field-read
  import, which re-authenticates the owner, generation, descriptor and bounds,
  and trap on refusal. The caller frame reserves only that 16-byte block and
  one i32 local per loan, never record-sized storage for the nominal `R`
  argument; each lent read reserves one 8-byte import scratch.

Only parameters that receive at least one row loan change carrier; every other
parameter keeps its ordinary ABI.

## Projection and cache

Programs with row loans select Graph v82, which retains every inner v81 fact
and projects each descriptor as `{"kind":"row_loan", ...}` with all descriptor
identities, plus a non-authoritative `row_loans` marker. Older programs keep
their schema and bytes. The checked-module HIR cache encodes the descriptor as
a new tag and its compatibility is `semaprax.project-checked-module-hir.v8`;
decoded HIR has no authority until complete validation replays it. Workspace
replay binds every authored `helper(row)` slot to exactly one retained
descriptor whose callee and parameter come from current local or provider
source; retained primitive bodies hash every descriptor identity.

## Qualification definitions

Owning definitions are authored in `tests/language/record_vec_traversal.rs`
(source, HIR, graph, forged descriptors and refusals),
`tests/owned_data/owned_leaf_vec/record_traversal.rs` (interpreter, native and
Wasm parity, including the same helper called with a resident record and a
loan, and a canonical contract failure on all three engines),
`src/hir/row_loan/tests.rs`, `src/loan_plan/traversal/tests.rs`,
`src/wasm/aggregate/row_loan/tests.rs` (caller and helper frame layout),
`tests/project/semantic_cache.rs` (warm checked-module cache re-proves the
current provider helper and refuses provider drift exactly as a cold build),
`src/cli/help/authoring.rs`, and
`src/workspace_graph/source_schema/static_text/tests/retained_lexical_composition.rs`
(imported helper success, proof and drift refusals). Definitions are not
execution evidence. Allocation-fault sweeps, maximum-capacity traversal,
Catalog/ShiftSim adoption and matched agent campaigns remain unqualified.
