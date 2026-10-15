# Owned record scalar field update v1

Audience: language users, compiler contributors and reviewers.

Status: source implementation batch for #836; all owning executable gates below
are pending. No implementation completion, application acceptance or efficiency
result is claimed.

## Source and admission

`vec_update_field<Row>(rows, index, value, "field") -> Vec<Row>` consumes and
renews an existing owning vector. The selector is last, consistent with lowering
a contiguous left-to-right evaluated operand list followed by literal metadata.
It is one literal ASCII field identifier of at most 256 bytes. It is not a
runtime String, allocation, evaluated fourth operand or dynamic selector.

`Row` is an explicitly identified, monomorphic, invariant-free flat record
already admitted by [Owned Leaf Collections v1](OWNED-LEAF-COLLECTIONS-V1.md):
one through eight explicitly identified fields, one or two direct String/Bytes
leaves, all remaining fields from `i64`, `i32`, `u8`, `usize`, `char`, `f32`,
`f64`, `bool`. The selected field must be one of those Copy scalars, and `value`
must have its exact type; there is no coercion. `index` has type `usize`.
Primitive, Copy-only, nested, recursive, generic, resource, class,
borrowed-field and invariant-bearing records do not acquire this operation.
The legacy two-Bytes record may use its existing carrier when it independently
meets this same predicate; its old operations and physical layout stay exact.

Missing, computed, non-Copy or unsupported selectors/shapes refuse with
`SPX-T320`; ordinary wrong operand types retain `SPX-T205`. An own vector is
required; ordinary move/availability and whole-root loan rules apply. The
operation is allowed in existing bounded loops under their unchanged work and
allocation limits. Loop admission counts four source arguments; lowering removes
only the literal selector, leaving three evaluated runtime children. It creates
no general mutable reference or new authority.

## Evaluation and commit

Evaluate vector, index and value exactly once from left to right. The selector
is resolved statically. Finish RHS evaluation before checking the index; an RHS
failure therefore remains selected even if the index would be out of range.
Authenticate the current owner/generation, complete nominal layout and scalar
slot before storage access. Invalid index selects existing `semaprax.vec.v1`
code 2 before transferring the staged owner. Preflight or RHS failure leaves
staged owners to ordinary canonical failure cleanup.

Success performs one Copy scalar slot store, then publishes the same vector at
its next generation, with one ordinary grouped call commit. Length and capacity
are unchanged. Every unrelated scalar bit and every String/Bytes owner and
payload identity stays unchanged. The store does not clone, allocate, drop a
leaf, rebuild a row, settle another field, call user code or acquire an effect.
Checked preflight generation exhaustion/stale or forged private carriers fail
closed; no stale handle is repaired. Failure selection is sticky and cleanup
plan order is consumed as authored, never sorted or repaired.

A field view protects the whole vector generation through its last use.
Movement/update while that loan is live is refused, including inside a scoped
row traversal. End the loan first; copying an index does not bypass it.
[Scoped Vec Field Reads v1](SCOPED-VEC-FIELD-READS-V1.md) retains its existing
read-only behavior. No view survives mutation.

## HIR, source and cache authority

HIR uses the ordinary owned `Call` form with exactly three runtime children
`.arg.0`, `.arg.1`, `.arg.2`, one exact element type and no function instance.
Its compiler-owned callee is `core.vec.update-field.v1:<descriptor>`. The
canonical length-delimited descriptor retains the exact nominal identity,
complete declaration-order field/type/capacity layout and persistent selected
field ID, bounded to 16384 bytes. Replay independently derives the layout,
field name, type, owner and admission from current declarations. A reserved
prefix alone grants no authority; declarations/imports cannot impersonate it.

The source verifier independently derives the profile from source declarations;
HIR validation checks the exact RHS signature again. Canonical formatting uses
the existing ordinary generic-call grammar and preserves the literal selector.
Graph prelude v19 is selected only by this operation; prior source keeps its
prior prelude schema/bytes. The ordinary graph Call schema is unchanged.
Workspace replay compares the source-derived descriptor at its exact expression
identity. Checked module cache compatibility v12 independently resolves the
retained canonical synthetic source. A coherent same-type field substitution
cannot gain source/cache authority even after rebuilding all ownership proofs.

The existing cleanup, renewal, status and cache Call machinery retains the
operation's fallible preflight/one-owner boundary. No new plan vector is sorted
or repaired. Old public command entry ABI, carrier capacities, profiles v27/v29
and public owning adapters retain their existing admission. Private owned-leaf
profiles alone admit this operation; schema declarations or synthetic origins
cannot widen them.

## Runtime representations

The interpreter uses unique existing vector/record storage and mutates the
selected scalar slot without materializing owned values. Native C11 authenticates
its existing carrier descriptor and index and writes the selected scalar word;
all eight scalar encodings retain exact bits, including signed zero and NaNs.
The success owner transfer follows the ordinary cleanup epoch protocol.

The private Core-Wasm host adds only `spx_vec_update_field_v1`, six i64 arguments
`(handle, nominal_identity, shape, index, ordinal, scalar_bits) -> i64`. Existing
imports, layouts and tags remain unchanged. The operation authenticates the
complete descriptor, current generation, index, ordinal and scalar encoding.
A null failure result consumes nothing; success renews the existing authority
slot once and returns its fresh generation. No row/payload clone, allocation or
Drop occurs at the store. Legacy tag10 descriptor binding is selected only when
this update is reachable. This is a private host protocol, not a public ABI.

## Adoption and exact owning gates (pending)

The actual typed ShiftSim scheduler carries `Worker { id, free_at }` and updates
`free_at` after the assignment's borrowed ID is copied into the output report.
Its runtime parallel worker-state Vec and complete worker-row reconstruction
are removed. `PatientState` and independent sorted arrival indices remain:
they serve different work. Prior WorkerState helper bodies, old scheduler,
independent acceptance/oracles and all original ShiftSim15 tasks stay intact.
The unrelated [inventory example](../examples/inventory-field-update/main.spx)
updates `count` and `active` while preserving `sku` and the other row.

Minimum next grouped selection, owned by the integration parent:

- `cargo test --locked -p semaprax --lib vec_ops::field_update::tests -- --nocapture`
- `cargo test --locked -p semaprax --lib interpreter::owned_vec::field_update::tests -- --nocapture`
  authors checked-source construction-through-Drop controls for unique stores,
  vector/row Arc alias refusals and generation overflow before commit. The
  unique-store measurement pays the same actual descriptor checks in its
  independent no-store control; raw unrelated f32/f64 bits, String/Bytes owner
  identity and materialization counters remain observable. Overflow retains its
  exact invariant guard through ordinary source-frame failure and owner Drop;
  it is not converted into a language-visible status.
- `cargo test --locked -p semaprax --lib project::incremental::snapshot::field_update::tests -- --nocapture`
- `cargo test --locked -p semaprax --bin semaprax scalar_field_update_card -- --nocapture`
- Existing owned-data backend harness's `field_update` module: same-source
  interpreter/native C11 O0/O2/strict Wasm, exact/+1 index, every scalar lane and
  floating bit patterns, RHS first failure, physical leaf identity and zero
  clone/allocation/drop at store, whole construction-through-Drop settlement,
  stale generation and malformed descriptor controls.
- Existing typed ShiftSim application harness: unchanged complete ShiftSim15,
  original invalid-input controls and independent scheduler comparison, plus
  unrelated inventory/count acceptance; full original fuel/bounds retained.
- Scoped Clippy and required current pin regeneration follow the parent's batch;
  no full suite is selected by this source-only work.

No Cargo command, build, test, Clippy or benchmark was executed by this authoring
lane. Static source is separate from executable evidence. Immediate prior source
archives/inverses and direct pending selectors accompany this batch. Historical
global provenance composition remains deliberately uncomposed here; the parent
will compose it once after integrating disjoint lanes. No measured token, turn,
latency, billed-cost or efficiency savings follows from a source reduction.
