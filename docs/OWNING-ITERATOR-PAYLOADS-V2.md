# Owning Iterator Payloads v2

Status: implemented private payload profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
The broader iterator and public-library goals remain incomplete.

Audience: language, ownership, cleanup, backend, and standard-library
contributors.

This profile extends [Owning Iterators v1](OWNING-ITERATORS-V1.md) to consuming
iteration of `Vec<Bytes>`. It defines the ownership boundary for `Iter<Bytes>`
and `IterStep<Bytes>`, and for `for own` traversal, without widening borrowed
vector access or the public ABI. The implementation selects Prelude
v8, CleanupPlan v13, and Graph v45. Each is selected only when the exact
retained `Iter<Bytes>`/`IterStep<Bytes>` types, including the local `Done` case
and signatures, are present; scalar Prelude v7, CleanupPlan v10-v12, and
earlier graphs remain unchanged. The dedicated Wasm owned-iterator imports
are specified below and exercised by the focused runtime corpus.

## Payload transfer

`vec_into_iter<Bytes>` transfers the existing vector allocation into an owning
iterator. `iter_next<Bytes>` consumes one iterator generation and either
settles exhaustion or yields one `Bytes` item and one successor `Iter<Bytes>`.
The item and the rest move exactly once through a detached-prefix carrier. The
iterator authority owns the initialized range `[cursor, len)`; the detached
prefix is transferred as the yielded `Bytes` and is never cleaned up by the
iterator. No vector operation accepts that carrier. No byte buffer is cloned,
and no iterator backing allocation is introduced.

Before the commit boundary, `iter_next<Bytes>` validates the cursor and live
Bytes range. It then atomically detaches the item and advances the successor.
Canonical `Yield` owner order is the item `Bytes` followed by the rest
`Iter<Bytes>`. The successor retains the remaining initialized payloads in
their original order. Existing vector validity, initialized-slot tracking, and
drop order remain authoritative: the iterator does not weaken or reinterpret
the original Vec validity facts. The initialized suffix remains iterator-owned
while the detached prefix is removed from that ownership window.
Exhaustion settles the vector store exactly once. Early iterator drop settles
only unyielded payloads; a yielded item is governed by its receiving owner.

Arguments stage left to right. Read, capacity, and contract failures select a
sticky status before publication and clean up every still-staged owner. A
failed call publishes no item or successor. `for own` uses this same consuming
protocol and preserves the same transfer and failure rules. Its item binding is
`Own` and its condition is `Borrow`; it does not admit a borrowed `VecGet` or
an implicit clone.

## Closed boundaries and replay

`vec_get<Bytes>` remains closed because borrowed access cannot return an owning
payload. The additive payload path must preserve the existing scalar Vec slots,
validity facts, cleanup plans, and public descriptors. Exact source, HIR, graph,
ProgramRoot, native, and Core Wasm projections must replay the same selected
payload carrier and ownership transitions. Forged or stale carrier, type,
case, cleanup, or source bindings fail closed and confer no authority.

This profile adds no ambient allocator, host capability, transport, or public
generic iterator ABI. Generic authored iterator implementations, lazy closure
adapters and broader payload types remain separate scope. The scalar callback
and closure profiles are implemented but do not widen this Bytes payload ABI.

## Graph and cache binding

The additive profile selects the exact Graph v45 binding for retained
`Iter<Bytes>` and `IterStep<Bytes>` declarations and their local `Done` and
`Yield` signatures. Its graph binds Prelude v8, while CleanupPlan v13 records
the detached-prefix ownership and successor settlement order. The cache
codec round-trips that v13 cleanup plan; decoded plans remain untrusted until
independent HIR replay rejects stale or substituted bindings. Scalar Prelude
v7, CleanupPlan v10-v12, and earlier graph and cache bytes remain unchanged.

## Focused local evidence

The original local corpus remains a historical witness; the implemented
release corpus is now hosted green. The executable `iterator` library filter,
`owned_iterator` workspace filter, and `owned_iterator_payloads` owned-data
filter cover empty and exhausted `Vec<Bytes>`, ordered yields,
multi-byte payloads, early drop, complete `for own` traversal, item and rest
failure, sticky cleanup, repeated exact settlement, and cross-layer replay on
the interpreter, native C11 `-O0`/`-O2`, and Core Wasm. Separate scalar-iterator
and owned-Vec preservation cases pass. Borrowed
`VecGet<Bytes>` remains rejected. Wasm tests additionally reject no-write
success, invalid step tags, and borrowed payload carriers before compiler
commit. Malformed-provider tests inspect the retained host state and perform
explicit host cleanup; they do not claim automatic settlement after a trap.
The existing Linux iterator selector includes these cases.

## Core Wasm host boundary

Only this owned-iterator profile selects these imports from `env`:

```text
spx_iter_bytes_into_v2(vec: i64, out_iter: i32) -> i32
spx_iter_bytes_next_v2(iter: i64, cursor: i64, out_step: i32) -> i32
spx_iter_bytes_drop_v2(iter: i64, cursor: i64) -> void
```

The 16-byte iterator frame contains a distinct opaque iterator handle followed
by a 64-bit cursor. The 32-byte step contains a 32-bit tag, a zero reserved
word, a Bytes carrier, the successor iterator handle, and successor cursor in
that order. `into` writes a cursor of zero. `next` writes a complete `Yield`
with tag 1 and the detached existing byte owner, or an all-zero `Done` after
settling the exhausted backing store. The compiler pre-fills the output frame
with `0xA5`. Zero and `0xA5A5A5A5A5A5A5A5` are reserved handle values;
unchanged poison handles, an invalid tag, nonzero reserved word, or malformed
`Done` fields reject before compiler ownership commits. The host must write
the complete frame on success.

The status-returning imports retain existing Vec status codes: zero succeeds;
nonzero codes 1 through 3 commit neither output nor ownership. Complete handle,
window, payload, and output-range validation precedes mutation. The
`(handle, cursor)` pair is the authority and the old pair is invalidated
when `next` succeeds. The host may rotate the numeric handle or retain it with
the updated cursor, but the successor handle must be nonzero and nonpoison and
the successor cursor must be `cursor + 1`; no fresh backing allocation is
required. It retypes the vector's backing-store authority without adding a
second backing allocation, and its distinct iterator
registry prevents old Vec operations from accepting iterator handles. Drop
settles only the initialized suffix in index order, then the backing store.
Scalar iterator imports, layouts, and module bytes remain unchanged.
