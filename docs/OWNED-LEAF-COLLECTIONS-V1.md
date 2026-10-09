# Owned leaf collections v1

Status: implementation in progress for the remaining runtime portion of #723.
This document records the proposed additive contract; no executable completion
or agent-efficiency result is claimed. The earlier Copy-record implementation
and declaration-only logical JSON schemas do not complete #723.

## Element and storage boundary

The new element is either primitive `string` or an explicitly identified,
monomorphic flat record with one through eight fields, of which one or two are
direct owned `string`/`Bytes` leaves and the rest are direct Copy scalars. Every
record field has an explicit identity.
SEMAPRAX source spells the text type `string`; `String` below names the
owned runtime representation, not source syntax.
The scalar set is `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, and `bool`.
Field names and declaration order do not determine admission. Nested, recursive,
generic, resource, class, third-owned-leaf and borrowed-field elements are
outside this tranche. Existing record invariants remain checked at ordinary
construction. Copy-out duplicates an already checked value exactly.

## Authoring forms and current limits

Write the source type as lowercase `string`. A by-value String parameter is
written `text: string`; it already transfers ownership, so `text: own string`
is refused (`SPX-O002`). Give user-declared records and every field explicit
stable IDs. For example:

```semaprax
@id("example.user")
record User {
    @id("example.user.name") name: string,
    @id("example.user.visits") visits: i64,
}
```

`Vec<string>` is an owning collection. `vec_push<string>(words, text)` consumes
`text`, and `vec_into_iter<string>(words)` transfers its elements to `for own`.
Use the v30 Project profile and `vec_clone_at` when a fresh deep copy is
intended. A `Vec<string>` declaration inside a JSON request schema remains a
schema description unless it is also an admitted executable source carrier.

Bytes construction or copying inside a bounded `while` remains refused with
`SPX-T267`; this collection tranche does not relax loop allocation rules.
Construct or copy the Bytes value before the loop and pass or move the existing
owner through the admitted operation. In particular, do not put
`bytes_zeroed` or `bytes_copy` inside the loop.

Source and HIR reconstruct this shape independently from authenticated
declarations. String, the record, and their Vec/iterator carriers remain affine;
no owned leaf becomes Copy. The old two-Bytes-plus-one-scalar record collection,
Copy-record collection, scalar Vec, and public ABI profiles stay unchanged.

Each owned leaf charges the existing 16-byte owned-carrier rate against the
existing 131072-byte carrier envelope. Each Copy field charges one eight-byte
word against the existing 8192-word scalar envelope. Logical capacity is the
minimum of 8192, `floor(8192 / scalar_count)` when nonzero, and
`floor(131072 / (16 * owned_count))`. This gives 8192 for `Vec<string>`, 2048 for
a String-plus-four-scalar record, and 4096 for two owned leaves plus one scalar.
These are carrier-storage charges, not a new bound on the sum of leaf contents.
Ordinary String/Bytes payload, live-owner, runtime allocation and builder limits
remain authoritative. Capacity counts logical elements. No existing envelope
is raised or specialized to a benchmark. Primitive `Vec<Bytes>` stays unchanged.

The four new source operations have distinct stable identities:
`core.vec.clone-at`, `core.vec.replace`, `core.vec.reserve-owned`, and
`core.vec.sort-owned`. Their result/argument contracts below use the existing
`Vec<T>` nominal carrier. Old `vec_get`, `vec_set`, `vec_reserve_exact` and
`vec_sort` keep their old element admission, including legacy owned-record
refusals. New operations may act on the legacy two-Bytes-plus-scalar shape using
its existing physical carrier; this does not rewrite its old operation answers.
Project v27/v29 refuse the new runtime operation/element closure. No second
collection nominal or source-generated privilege is introduced.

## Reads, replacement, and iteration

The existing typed constructors, push, len, capacity and clear admit the new
elements. Push consumes its element. The additive `vec_clone_at<T>` operation
borrows the vector only for that call and returns a fresh owned element: a
clone of each owned leaf in declaration order and unchanged scalar fields.
This is an explicit copy-out contract,
like an owning String map read. It does not return a borrowed record or a view
into a vector generation. Reads therefore cannot introduce escaping element
loans. Existing borrowed-vector parameter loans and overlapping mutation/move
refusals remain enforced by ordinary source/HIR loan analysis.

The additive `vec_replace` stages vector, index, and replacement left to right. Invalid index or
preflight allocation failure leaves the staged owners available to ordinary
failure cleanup. The ordinary grouped call commit transfers the vector and
replacement together. Successful replacement settles the old element exactly
once and publishes one renewed vector. No observer can access a half-replaced
element. Failed deep copy-out settles every partial result without mutating the
source vector. Failure selection remains sticky.

The additive `vec_reserve_owned` uses `max(capacity, len + additional)` semantics, checked
addition and the element-specific capacity bound. Clear drops initialized leaves
in canonical element/field order and retains capacity. Length/capacity are
nonallocating borrowed queries. `vec_into_iter`, `iter_next`, and consuming
`for own` transfer each original element once; early exit/failure settles the
remaining inventory through the existing iterator plan. Indexed nonconsuming
reads use the owned copy-out rule above. Same-owner mutations follow the existing
renewal protocol; ownership cannot be recovered by rebinding a moved value.

## Deterministic ordering

The additive `vec_sort_owned` consumes and returns the same vector, stably ordered by declaration
field order. Primitive String vectors use unsigned UTF-8 byte order, with proper
prefix order and ordinary embedded-NUL behavior. Record String fields use that
same rule. Bytes fields use unsigned byte lexicographic order without UTF-8
validation. Scalars retain existing integer/character/bool and floating total
order, including signed zero and NaN payloads. A sort moves complete owning
carriers and never clones, drops, calls user comparison code, or obtains effects
while comparing. The private representation must keep the leaf's exact owner
identity and current generation. No untyped scalar handle is an owning value.

A scheduler can declare priority, arrival, id, service, deadline in that order
and sort its actual patients. The independently checked codec still binds JSON
wire field names. A report can declare negative count followed by path for
descending counts and ascending UTF-8 ties. Unrelated catalog records must work
under the same structural rule.

## Representations and authority

Native code stores real String/Bytes carriers and scalar slots. Core Wasm uses
ordinary authenticated owning leaf representations and a versioned, bounded
collection carrier; any provider descriptor change needs its own exact replay
and host fixture. Interpreter values retain owned Strings/Bytes and named records.
Every backend uses the same logical capacity, ordered comparison, failure
selection, call commit and cleanup contract. Target allocation layout is not
source or cache authority.

New Project v30 (`language-command-io.owned-data.v1`) may transport owned/borrowed vectors and
owned/borrowed record values through private helpers. Public command roots remain
`fn() -> i64`; effects, grants, argv/stdin/stdout provider limits and old-profile
refusals remain unchanged. Logical schemas become runtime values only after
their independently authenticated runtime shape is admitted. Generated codec
names and synthetic origin grant no permission.

## Executable qualification required

- Independent source/HIR classifiers, source round trips, graph and cache replay;
  hostile nominal/field/origin/type-fact drift; stable refusal of unsupported
  shapes and unchanged old profiles.
- Same-source interpreter, native C11 O0/O2 and strict Core-Wasm success for
  String vectors and heterogeneous one-/two-owned-leaf records, all scalar widths,
  NUL/Unicode/prefix and multi-key ties, exact NaN bits, empty/full vectors,
  exact/+1 capacities, reserve overflow, get/set bounds and owner renewal.
- Partial construction, get clone, reserve, replacement and iteration failures;
  exact old/new leaf lifetime, zero leaked/double-settled owners, sticky status,
  no partial result publication, active-borrow mutation/move refusal.
- Actual native command composition, complete ShiftSim 15, unchanged LogLens49
  and a structurally unrelated catalog application. Existing meaningful-data,
  raw-whitespace, UTF-8, decimal, diagnostic and public capability requirements
  may not be weakened. TeamDesk912 remains unchanged if included.
- Source reduction is a manual proxy only. Efficiency requires at least five
  matched fresh agent trials per arm per before/after condition, same model,
  prompts and strong idiomatic TS baseline. Report acceptance, turns, net input,
  authored tokens, fixed harness context, conditional accepted-task cost and wall
  time separately; unavailable billed cost is null.
