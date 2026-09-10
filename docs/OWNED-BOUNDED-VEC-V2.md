# Owned Bounded Vec v2

Audience: language users and compiler contributors.

Status: implemented additive owned-payload profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
This extends [scalar Vec v1](OWNED-BOUNDED-VEC-V1.md) with owned Bytes payloads.
It does not redefine that frozen scalar contract. Consuming payload traversal
is implemented separately in [Owning Iterator Payloads v2](OWNING-ITERATOR-PAYLOADS-V2.md).

## Owned operations

The compiler nominal `core.vec` admits `Vec<Bytes>` for these explicit calls:

| Operation | Parameters | Result |
| --- | --- | --- |
| `vec_with_capacity<Bytes>` | `usize` capacity | own `Vec<Bytes>` |
| `vec_push<Bytes>` | own vector, own Bytes | own next vector |
| `vec_len<Bytes>` | borrowed vector | `usize` |
| `vec_capacity<Bytes>` | borrowed vector | `usize` |
| `vec_reserve_exact<Bytes>` | own vector, `usize` additional | own next vector |
| `vec_set<Bytes>` | own vector, `usize` index, own Bytes | own next vector |
| `vec_clear<Bytes>` | own vector | own next vector |

All existing operation identities and `semaprax.vec.v1` status codes are
retained. `vec_get<Bytes>` is rejected: returning an owning payload from a
borrowed vector would require a clone or a separately specified borrowed view.
Traversal through the scalar `for` profile remains closed for these payloads.

Arguments evaluate left to right. Push and set stage both the vector and the
new Bytes owner. A full push, out-of-range set, or failed reserve selects its
existing status before the canonical owner commit. Ordinary failure cleanup
therefore settles every still-staged owner, including a newly produced Bytes
argument. Successful operations transfer the declared owners together and
publish one next vector generation.

Set drops the replaced Bytes exactly once. Clear drops initialized payloads in
index order, then sets length to zero while retaining capacity. Lexical vector
cleanup drops all initialized payloads in index order, then releases backing
storage. Reserve moves initialized Bytes carriers into replacement storage
without copying the owned byte buffers. Empty and uninitialized slots carry no
payload finalizer.

Capacity remains bounded by 8,192 elements. The additive profile charges 16
bytes per payload carrier, at most 131,072 bytes, independently of the bounded
byte buffers those carriers own. The scalar profile keeps its existing
8-byte charge. Push has no implicit growth; reserve uses
`max(old_capacity, length + additional)` with overflow and bounds checked.

## Binding and hosts

Prelude v6 extends the frozen v5 contract. Its exact bytes live in
`tests/fixtures/prelude-v6.contract`, with SHA-256
`924f67b773e3dc4d26b3891fe2415b6e567842a2c07b02c8eae55ae5467c56c4`.
Canonical source, graph and semantic workspace bindings select that contract
before lowering. A later source using an earlier prelude cannot downgrade it.

Core Wasm selects explicit `spx_vec_*_v2` imports for all vector operations in
a program using this profile. Tag 9 denotes an owned Bytes payload. The v2
host must retain the existing scalar tag meanings, transfer payload handles
only after success, reject copying tag 9, and recursively drop initialized
Bytes handles. A host providing only unversioned imports fails to link.

## Evidence and remaining scope

The focused selectors are `owned_bytes_vec` in the library harness,
`owned_vec_bytes` in the owned-data harness, and `owned_vec_bytes_workspace`
in the workspace harness. Their assertions cover exact ownership, canonical
round-trip, frozen contract bytes, wrong-prelude graph rejection, ProgramRoot
replay, repeated mutation, contract and allocation failure, and balanced
native allocations and Wasm handles. Runtime cases also cover private function
composition, mixed scalar and owned vectors, and mutable same-owner replacement.
The implemented corpus has hosted-green v0.4.0 evidence; earlier local results
remain historical witnesses.

The scalar `std.collections` aliases and public descriptors remain frozen.
This tranche does not supply a public generic ABI, broader generic function
substitutions, other owned payload types, pop/removal, regions, arenas or shared
ownership. Consuming Bytes traversal is the implemented Iterator Payloads v2
extension; callbacks, closures and adapters likewise retain their separate
owning profiles. Their existence does not widen the operations in this document
or complete the full language and library goal.
