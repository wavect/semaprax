# Borrowed Record-Field Uniqueness v1

Status: authored source batch (#786); executable qualification remains separately gated.
Audience: coding agents, compiler contributors and reviewers.

`vec_unique_by<Row>(rows, "id")` returns `true` when no two rows of an already
admitted flat Copy-record or directly owned String/Bytes-record Vec have equal
values in one selected field. It borrows the Vec for the call only. Row order,
row storage, payload owners, length, capacity and generation are unchanged, and
the Vec remains usable afterwards. It is a distinct operation: it is not
`vec_sort_by`, does not consume or renew its owner, and grants no ordering.

## Closed source profile

The type argument is one explicit, monomorphic record selected by the existing
Copy-record or owned-leaf classifier, with explicit persistent identities on the
record and every field, exactly as [Explicit Record Sort Keys
v1](EXPLICIT-RECORD-SORT-KEYS-V1.md) admits. Generic, nested, recursive,
implicit-ID records and primitive Vecs are refused. The legacy two-Bytes plus
one-scalar owned-record carrier (tag 10) is also refused in v1: its private
Wasm layout is bound lazily by owning operations, and a borrowed query may not
bind authority.

The first argument is a named, available `own` or `borrow` binding of exactly
`Vec<Row>` with no moved or partial places. Temporaries, call results, record
paths, generic `T` parameters and function templates are refused. The second
argument is one string literal naming one field with ordinary ASCII identifier
spelling, at most 256 bytes. The v1 key profile is closed to:

* `string` fields; and
* non-floating Copy scalar fields: `i64`, `i32`, `u8`, `usize`, `bool`, `char`.

`f32`/`f64` keys are refused, so no numeric-versus-bit equality ambiguity
exists. `Bytes` keys are refused in v1; no raw-Bytes widening is implied.
Computed names, field lists, `field:asc` selectors, callbacks and wrong arity
are refused. Every refusal is `SPX-T319`. The operation is not admitted in
`while` bodies (it is not a while-admitted Vec operation), so loop bodies keep
their existing `SPX-T283` refusal.

## Descriptor and authority

The literal is compiler metadata, never an evaluated or allocated String. The
runtime Call has exactly one runtime child, the borrowed Vec at `.arg.0`, one
type argument, no instance, `bool` result and value ownership. Its callee begins
`core.vec.unique-by.v1:` and carries the canonical length-delimited descriptor
`element`, complete `layout`, persistent `field` identity (each `len:bytes`),
bounded to 16384 bytes. The element and complete layout (family, nominal
identity, exact capacity, every field ID/type in declaration order) are derived
by the explicit sort-key layout derivation of the one-field ascending selector;
only that layout/field metadata is reused. A sort-by descriptor never decodes as
a uniqueness descriptor and vice versa.

Source admission (`source_verify/field_unique.rs`, both the iterative verifier
and the independent oracle) derives the descriptor from source declarations.
The HIR resolver (iterative and recursive reference) derives it from resolved
declarations. HIR validation, the interpreter, native C and private Core Wasm
each replay the complete descriptor from current declarations before reading
storage, and each re-checks the exact root shape. Workspace source
reconstruction (`workspace_graph/expected_projection/field_unique.rs`)
independently derives the callee at each source expression identity and
compares it with retained HIR (`SPX-G173`). A reserved prefix grants nothing:
authored declarations or imports of the prefix and forged, stale, truncated or
trailing descriptor data fail closed. A same-typed valid descriptor for a
different field is valid HIR but fails current-source replay.

## Equality, result and evaluation

Equality is full selected-key equality under the existing Vec total comparator:
String compares unsigned exact bytes and then length, so NUL, UTF-8 prefixes and
non-ASCII bytes are ordinary data; integer-like scalars compare exact values.
Unselected fields never matter. Empty and one-row Vecs return `true`. No hash,
fingerprint or digest grants equality.

## Loans, status and cleanup

The call borrows the whole named Vec root from call entry through call exit
(the `field_unique` receiver in `loan_plan/call_arguments.rs`), including
aliases. No new ownership exemption is introduced: the borrowed argument is
never transferred, renewed or finalized. Move, renewal, push, clear, sort or
consuming iteration of that root cannot overlap the call; other reads and field
views coexist.

The operation is fallible only through scratch acquisition. A refused ordinal
scratch allocation selects the existing `semaprax.vec.v1` code 3 through the
ordinary operation-failure status lane of the canonical cleanup plan. It is
not in the infallible Vec operation set. First-status selection is sticky,
cleanup runs in canonical order and never touches the borrowed owner, and the
Bool is published only after scratch release. Selected-replay renewal profiles
refuse this operation as requiring separately authenticated execution.

## Runtime scratch and payers

Each engine allocates one ordinal array of `len` `u32` values (`4 * len` bytes,
`len <= 8192`, so the product cannot overflow), fills `0..len`, performs a
deterministic in-place heapsort of ordinals comparing borrowed keys in the
original rows, then compares adjacent sorted keys for equality. No key, row or
String is copied, and no stable sort or per-comparison allocation exists.
Vecs with fewer than two rows allocate nothing.

* Interpreter: one fallible `try_reserve_exact(len)`, fuel charged per row
  authentication and per comparison; the carrier `Arc` is only read.
* Native C11: `spx_unique_ordinals_v1` performs one `malloc(4 * len)`, freed
  before return on both results; `NULL` selects code 3 with the owner intact.
* Private Core Wasm: `env.spx_vec_unique_by_v1(owner, identity, shape,
  position) -> i32` returns 0 (duplicate), 1 (unique) or 3 (scratch refused);
  any other result is a fail-stop trap. The host authenticates the exact bound
  identity/shape, generation and field position and must not bind, renew,
  copy, drop or insert authority. Copy-record constructors bind their layout
  through the existing `spx_vec_sort_by_v1` zero policy whenever this operation
  is present. The strict test host's typed-array scratch is fixture memory, not
  paid native backing.

The identity writer reserves its output buffer on the active bounded-output
budget before allocation, and the ascending selector uses the budgeted formatter.
Workspace declaration cost retains the conservative explicit-sort transient
prebound. These local reservations do not establish complete descriptor replay
payment: decode copies element/layout/field text, and decimal parsing in the
shared reader allocates before the identity writer's reservation. Prelude
source-use discovery reuses the existing AST visitor without a heap frontier
inside its fixed-depth contour; its deeper overflow route remains unchanged. Exact coverage
of those allocations remains an accounting qualification obligation. Runtime
scratch has no compiler payer: native scratch is an exact allocator request
with no context ledger, and the interpreter has no general heap ledger. These
are disclosed limits, not allowances.

## Prelude, graph and cache

New programs using the operation select prelude v18, which extends the v17
contract with this operation. Programs without it keep their prior prelude
selection and bytes. The operation adds no graph node kind: graphs carry the
descriptor callee of an ordinary Call, and graph verification recomputes it.
Checked-HIR cache entries carry the same Call; restored entries pass ordinary
validation and current-source replay.

## Unsupported surfaces

No public Rust or Wasm ABI support is added. The shared-owner private host
(`wasm/shared_owner`) does not join the new import and stays closed. Older
command profiles and public adapters keep their carrier refusals.

## Owning gates

* `vec_ops::field_unique::tests` (5): canonical/graph/cache/prelude, independent
  source/resolved derivation and closed key profile, source negatives, forged
  HIR/cache descriptors, descriptor bounds.
* `workspace_graph::expected_projection::field_unique::tests` (3).
* CLI help `author:unique-by` card test.
* `tests/owned_data/owned_leaf_vec/field_unique.rs`: interpreter, native O0/O2
  and strict Wasm; empty/one/maximum; first/middle/last duplicates;
  NUL/UTF-8/prefix; scalar bounds; unselected-field differences; owner reuse;
  scratch refusal; hostile host controls; independent clone/sort oracle.
* `tests/project/standard_library/application_json/owned/shiftsim_id_validation.rs`
  and the complete ShiftSim corpus owner.

The focused gates cover the interpreter, native C11 at O0 and O2, and the
strict private Core-Wasm host. The complete ShiftSim corpus and its saved
pairwise and clone/sort oracles pass at the ordinary limits.

The native allocation probe observes 65,668 requested bytes across nine
uniqueness scratch requests in one corpus invocation, with a peak of 16,384
live scratch bytes and zero scratch remaining at reentry. Native and strict
private Wasm boundary controls accept the same 8-byte request under an
8-byte limit and refuse it under a 7-byte limit with zero scratch allocated.
These figures cover
the ordinal scratch only. The surrounding row and string allocations and the
compiler's cumulative workspace reservations have separate accounting.

## ShiftSim adoption

`shiftsim.schedule.unique_patient_ids` keeps its persistent identity and
signature and its body becomes `vec_unique_by<Patient>(values, "id")`. The
former temporary `Vec<string>` of cloned identifiers and
`shiftsim.schedule.unique_identifiers` are removed from the application and
retained byte-for-byte as an independent saved test oracle. Patient
validation, duplicate rejection, scheduling, serialization, acceptance inputs,
default limits and the TypeScript baseline are unchanged.
The adoption changes the two authored uniqueness helpers from 1,015 bytes
across 30 lines to one 148-byte helper across five lines. This comparison
counts the declarations and bodies and excludes their surrounding blank lines.
