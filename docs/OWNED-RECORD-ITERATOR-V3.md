# Owned Record Iterator v3

Status: implemented private payload profile.

Audience: language, ownership, cleanup, native, and Core Wasm contributors.

V3 extends [Owning Iterator Payloads v2](OWNING-ITERATOR-PAYLOADS-V2.md)
from `Bytes` to the exact monomorphic record shape in
[Owned Record Collection Element v1](OWNED-RECORD-COLLECTION-ELEMENT-V1.md):
two direct `Bytes` fields and one direct Copy scalar. The scalar may be `i64`,
`i32`, `u8`, `usize`, `char`, `f32`, `f64`, or `bool`. The record declaration
and all three field identities must be explicit. Generic, nested, extra-field,
missing-field, `String`, class, variant, and resource shapes remain refused.

The admitted operations are:

```spx
vec_into_iter<Record>(value: own Vec<Record>) -> own Iter<Record>
iter_next<Record>(value: own Iter<Record>) -> own IterStep<Record>
for own item in iterator { body }
```

`vec_get<Record>`, record iterator borrowing, a public generic iterator ABI,
and broader authored payloads remain outside this profile. Signatures remain
monomorphic at the one exact authored record shape.

## Ownership and cleanup

`vec_into_iter` retypes the existing vector authority as an iterator without
copying an element or allocating another backing store. Its initialized
window is `[cursor, length)`. `iter_next` consumes one iterator generation.
`Done` settles the exhausted carrier. `Yield` atomically transfers the whole
record at `cursor` and a successor iterator whose cursor is `cursor + 1`.
The record's two byte owners stay distinct and its scalar bits keep their
declared type. The detached prefix is no longer owned by the iterator.

Validation precedes the call commit. A malformed carrier, wrong record or
field identity, stale cursor, aliased byte leaf, incomplete output frame, or
wrong successor traps or selects the existing operation failure without
publishing an item or successor. Failure selection is sticky. Dropping the
iterator visits only the unyielded suffix in element order, drops each
element's byte fields in declaration order, and then releases the backing
store. The yielded record is cleaned by its receiving owner. `for own` uses
the same `iter_next` boundary; body failure cleans the current item and the
unvisited remainder exactly once.

CleanupPlan v13 already represents the relevant meaning: the yielded owned
item and the remainder are independent owners. V3 therefore retains v13.
The exact item type and field identities remain in checked HIR and additive
Graph v67; frozen Graph v45 remains the `Bytes` payload contract.
Prelude v11 binds the added admission, layout, and host protocol. Existing
scalar and `Bytes` iterator programs keep their prior prelude, cleanup,
Graph, and generated-byte contracts.

Combining record iteration with ordinary scalar Vec renewal selects CleanupPlan
v15 and Graph v67, retaining both independently authenticated fact groups.

## Target layout

Native C11 stores the record in the existing `spx_vec_record_v1` slot. The
record iterator has its own authority tag, so vector and iterator operations
cannot accept each other's handles. `IterStep<Record>` uses the canonical
variant layout with the canonical aggregate layout embedded in `Yield.item`;
field placement is derived from stable identities, never source names.

Core Wasm keeps the 16-byte `(handle, cursor)` iterator frame. This profile
selects only these additional imports:

```text
spx_iter_record_into_v3(vec: i64, out_iter: i32) -> i32
spx_iter_record_next_v3(iter: i64, cursor: i64, out_step: i32,
                        byte0_offset: i32, byte1_offset: i32,
                        scalar_offset: i32, rest_offset: i32) -> i32
spx_iter_record_drop_v3(iter: i64, cursor: i64) -> void
```

It also selects the existing byte-buffer imports and private byte-memory
export, even for empty traversal without explicit byte operations, because
step validation authenticates both byte carriers through that contract.

Offsets are relative to `out_step` and are derived from the independently
validated canonical variant and record layouts. `next` writes tag 0 and zero
payload fields for `Done`, or tag 1, two live byte carriers, the exact scalar
bits, and a nonzero successor frame for `Yield`. The compiler poisons the
whole output first and validates the tag, both byte carriers, the scalar's
canonical wire form, successor, and cursor before committing the consumed
iterator. This includes sign-extended `i32`, zero-extended `f32`, bounded
`u8`/`bool`, and Unicode-scalar `char` carriers. Hostile no-write, invalid-tag,
borrowed-carrier, noncanonical-scalar, and stale-successor frames fail before
owner commit.

## Required evidence

The owner harness executes manual `iter_next` and `for own` on the interpreter,
native C11 at `-O0` and `-O2`, and Core Wasm for all eight Copy scalar shapes.
It observes both byte lengths and scalar values, checks repeated settlement,
forces failure after the first detached item to prove suffix cleanup, and
injects hostile Wasm frames. Source/HIR tests freeze Prelude v11, CleanupPlan
v13, stable identity reconstruction, canonical formatting, and refusal of
near-miss record shapes. The scalar and `Bytes` iterator suites remain the
compatibility gates.
