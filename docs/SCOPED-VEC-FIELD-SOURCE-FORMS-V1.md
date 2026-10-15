# Scoped Vec Field Source Forms v1

Audience: compiler contributors, tool authors, and reviewers.

Status: source-only successor for the newly reached Catalog traversal refusal;
all qualification is pending. This extends representation and independent
replay of [Scoped Vec Field Reads v1](SCOPED-VEC-FIELD-READS-V1.md) and
[Scoped Record Vec Traversal v2](SCOPED-RECORD-VEC-TRAVERSAL-V2.md).

The ordinary `vec_field<Row>(rows,index,"field")` operation already admits any
checked Value `usize` index. Its existing current-length bounds check,
left-to-right evaluation, exact nominal field, nonescaping result view,
whole-carrier shared loan, sticky failure and cleanup do not change. It can
read an earlier row while a `for borrow` traversal freezes that same immutable
whole Vec. No source syntax, ABI, capability, runtime operation or limit changes.

Previously, the freeze receiver classified every `VecFieldRead` as the current
row and required its index to equal the immutable iteration snapshot. The
Catalog's unchanged `vec_field(items,index-1usize,"id")` therefore failed that
receiver regardless of whether its scalar identifier helper used an indexed
byte loop or a byte `for`. This is a newly reachable earlier composition defect;
it is not evidence that the byte helper itself changed Catalog behavior.

## Checked representation and replay

`VecFieldRead` now retains a `VecFieldReadOrigin`: `Explicit` for an authored
`vec_field` call, `Traversal` for a synthesized `row.field` read. Exact fused
String byte views preserve that form. Canonical AST syntax is unchanged.

The form grants no authority. Both forms independently authenticate the live
named Vec place, ownership, exact nominal declaration/field, result mode and
Value usize index. The complete freeze receiver still walks every child,
including the ordinary index expression, and still refuses mutation, transfer,
rebinding, aggregate storage and unauthorized root occurrences. A Traversal
read additionally requires the exact enclosing traversal, whole carrier and
immutable row index; the independent row census checks this even without a
whole-row loan. The existing RowLoan and Copy-row GET checks remain exact.

A retagged Traversal read with an independently valid ordinary index describes
a different HIR operation; a tag is not a proof of the authored source. Source
replay therefore binds both forms to their exact authored sites and refuses
form laundering in either direction. Cache replay re-resolves the canonical
synthetic AST and compares every retained body and contract. Workspace replay
independently enumerates explicit calls and row-derived projections in the
complete record-traversal body. A same-typed field, root or index substitution
retains all prior refusal obligations.

## Versions and backends

Graph v85 is an outer selective layer over v84/v83 and every existing inner
fact. Scoped reads project `source_form:"explicit"` or
`source_form:"traversal"`. Graphs without scoped field reads keep their prior
schema and bytes. Legacy and frozen evidence routes refuse v85.

Checked module cache v11 encodes the source form as Explicit tag0 or Traversal
tag1; every other tag is refused. Old v10 wire is not reinterpreted under v11.
The source frontend AST wire remains unchanged. No generated pin is edited by
this source batch; actual owning producers determine moved pins later.

Interpreter, native C11 O0/O2 and Core Wasm retain the same authenticated field
read operation, bounds/status handling, no-copy view and cleanup sequence. The
source form is compile-time proof metadata, not a new public or host carrier.
Backend admission still requires complete current HIR and loan validation.

## Authored owning controls

The existing owning modules add controls for the real previous-row lookup,
byte predicate, early return, break/continue, owner reentry, invalid-index
sticky failure and strict native/Wasm read witnesses; exact/short/zero backing
and work; original receiver source inverse; row-index/field/root/type/freeze
forgeries; canonical source, graph and current cache; unknown wire tags and
source/cache/workspace form laundering. No top-level harness binary is added.

The original paid receiver retains its strict old classification and complete
body. Its only compiled-source compatibility change ignores the added enum
field; a complete inverse restores the archived original text. The Catalog
helper, all three adoption controls, full23 owner, manual reference, corpus,
SPEC, profiles, acceptance and resource budgets remain unchanged.

Source preparation and reviews are not execution evidence. The complete
Catalog owner and the owning implementation controls, scoped Clippy and any
genuinely moved producer/pin still require actual qualification in the next
implementation batch. No application fit, completed issue or performance/token
saving is claimed.
