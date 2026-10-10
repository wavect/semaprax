# Scoped owned-vector field reads v1

Status: partial source batch for OPT-730; owning executable gates are authored. No owning execution,
application qualification or agent-efficiency result is claimed.

## Checked source shape

`vec_field<Row>(values, index, "field")` inspects one explicit record field of
an already admitted flat owned-leaf `Vec<Row>`. The type argument is the exact
monomorphic nominal record; the selector is an exact compile-time String
literal naming one of its explicitly identified fields. It is resolved into the
field's stable identity, not evaluated, allocated, encoded as a runtime selector,
or accepted from a variable. No record/Vec element family, capacity or scalar
set is added. Primitive Vec elements retain their existing operations.

The vector must be a named live own/borrow carrier, or an independently admitted
named collection-record path to that carrier. Temporary constructors, call-result
roots, generic function templates, arbitrary callbacks, dynamic selectors,
foreign/implicit field identities and owned element extraction remain closed.
Ordinary record invariants retain their existing construction and replay rules.

A Copy scalar field returns that scalar. A String field returns the existing
nonescaping `borrow str`; a Bytes field returns the existing `borrow Slice<u8>`.
The exact composition `str_as_bytes(vec_field<Row>(values,index,"text"))` borrows
the same String bytes. These operations never deep-clone or materialize an owned
leaf, transfer an element, drop a leaf, allocate a result owner, or renew a vector.
`vec_clone_at` keeps its distinct allocating owning-result contract.

## Evaluation, loans and failure

The vector source is inspected before the index, left to right. The index is
evaluated exactly once as usize. Bounds are checked against the current length;
an invalid index selects the existing `semaprax.vec.v1` code2. Authentication
precedes storage access. Stale/private-forged carriers violate the runtime
contract; they cannot be normalized into a valid source read or repaired.

A scalar read has a synchronous loan through its read operation, including index
evaluation. A String/Bytes view retains the exact named vector carrier path
through its last use. The loan protects the entire vector generation, so any
movement, replacement, push, reserve, clear, sort or consuming iteration of that
vector is forbidden while the view is live, including through a parent-record
move. Distinct parent-record siblings remain independent. Changing the source
index variable after a read does not retarget its already selected view.

Views may be locally aliased or passed to existing synchronous borrow parameters.
The borrowed views cannot escape via returns, aggregates, storage, captures or
tasks, or be reclassified as owners. Named byte-range rules and existing per-function loan/work limits
remain unchanged. Releasing a loan creates no runtime finalizer and no authority.
Failure selection and cleanup order remain the existing canonical sticky rules.

## Independent representation and replay

`VecFieldRead` HIR contains the exact element type, stable field identity, a fused
String-byte-view flag and exactly two children (Vec source and index). The source
selector literal is static metadata; child expression paths are `.arg.0` and
`.arg.1`, including their original nesting when fused. The flag may only convert
a String field's Str result into Slice<u8>. Each use independently replays the
record identity, every field's owner/index/name/type, the live root and complete
carrier path, child types and ownership, and result type. Cached result types or
layout positions are not authority.

SharedLoanPlan retains the complete vector origin and derives its canonical
lifetime from this node and ordinary uses. The node retains the index expression
and selected field identity; the loan conservatively protects the full vector,
not a fabricated integer element handle. Source, HIR, cache and graph replay must
agree. ByteSliceProvenance adds an optional boxed `VectorFieldProvenance` with
element, stable field and index-child identity; its root and projections still
name the carrier. Named Str aliases preserve this metadata through byte views
and ranges. Checked cache compatibility v7 independently resolves the retained
canonical synthetic source, so a valid same-type field substitution cannot gain
source authority. Malformed/stale fields, roots, selectors, flags and attached loan plans
fail closed. Source identity guards reserve `vec_field` and `core.vec.field`.

Native C11 reads the selected carrier directly from authenticated compact or
legacy storage. Scalar bit patterns remain exact, including NaNs and signed zero.
String views use the ordinary length-aware representation; Bytes views reference
the original payload. Core Wasm adds an independently versioned private read
operation over its existing descriptors and tag10/tag11 carriers; old imports and
encodings stay exact. The host returns existing leaf identity/scalar bits without
allocating, cloning or renewing authority. The interpreter reads by reference and
must not call its logical owning clone/materialization path.

The internal v30/v31/v32 command selections already admit these private element
carriers and retain their exact scalar entry/public command ABI and effects. This
read introduces no public owning ABI or ambient capability. V27/v29 and public
export adapters retain their closed operation/carrier admission. No new Project
schema is justified merely by an internal synchronous read; any contrary frozen
adapter inventory must be explicitly preserved before qualification.

## Executable obligations

The owning existing harnesses must cover canonical source/graph/cache replay,
scalar and String/Bytes reads, embedded NUL, signed/float bits, invalid selector
and exact/+1 index, left-to-right index failure, live-loan moves/renewals, permitted
renewal after last use, parent paths, stale/forged field/element/root/provenance,
unused old-profile helpers, and sticky failure cleanup. Repeated full-vector scans
must run under unchanged logical materialization limits and native allocator-
aborting read witnesses; strict Wasm host witnesses must reject any read-time
clone, allocation, drop or renewal. Native O0/O2, interpreter and strict Wasm must
agree on the same source. Typed ShiftSim and independent catalog/order adoption
retain the complete original application acceptance and strong TS comparisons.


The scoped semantic gates are `hir::vec_field::tests` (6),
`interpreter::vec_field::tests` (1), and
`project::incremental::snapshot::vec_field::tests` (1). Backend, graph and
source-index gates live in their existing owning harnesses. None of these new
source-authored gates is execution evidence until the grouped check succeeds.
