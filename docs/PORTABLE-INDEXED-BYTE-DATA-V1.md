# Portable Indexed Byte Data v1

Audience: language users, tool authors, and compiler contributors.

Status: partially implemented with local internal compiler and bounded public
Project/npm adapter evidence.
Checked target-independent `usize`, fixed `[u8; N]`, uniquely owned `Bytes`,
non-escaping `Slice<u8>`, all six compiler-owned byte operations, lexical
owner borrowing, target-neutral capacity analysis, Graph v17, canonical
CleanupPlan settlement, and interpreter/native O0/O2/internal Core-Wasm Node
execution have focused local evidence. Project Manifest v3 now selects the
closed `useful-data.v1` profile and locally proves strict `Uint8Array`
JavaScript/TypeScript inputs, exact six-file npm carrier replay, and an offline
installed multi-module binary-frame consumer. Unix publication is
handle-relative, create-new, and no-clobber; Windows v2 publication remains
fail-closed. Complete hostile cross-platform boundary evidence, exact-head
hosted evidence, registry publication, and promotion remain open. This is not
a complete Useful Data capability claim.

Portable Indexed Byte Data v1 is the first bounded Useful Data profile. It
adds one target-independent index scalar, fixed byte arrays, uniquely owned
byte buffers, non-escaping borrowed byte slices, and total indexed reads. It
extends the existing checked scalar, ownership, explicit-mutation, bounded
`while`, owned-`string`, and borrowed-`str` semantics; it does not create a
parallel evaluator, ownership system, cleanup authority, or project builder.

## Source contract

The exact new source types are:

```text
usize
[u8; N]
Bytes
Slice<u8>
```

`usize` is a checked unsigned 64-bit semantic integer on every target. It is
not Rust `usize`, C `size_t`, a pointer-width integer, or a Wasm memory offset.
Its range is exactly `0..=18446744073709551615`. Decimal literals use the
mandatory `usize` suffix, for example `0usize` and `65536usize`; a negative
literal is invalid. Canonical formatting removes redundant leading zeroes and
retains the suffix. Checked `+`, `-`, `*`, `/`, and `%`, equality, ordering,
immutable bindings, `let mut`, and assignment follow the existing scalar
evaluation and sticky-arithmetic-failure rules. Native lowers the value to
`uint64_t`; Wasm lowers it to `i64` whose bits are interpreted unsigned for
arithmetic and comparisons. Conversion to a physical address or Wasm `i32`
offset is never implicit and occurs only after a checked length/range proof.

`[u8; N]` is a fixed-size, inline, Copy array. `N` is a canonical unsigned
decimal source constant in `0..=65536`; it is part of the type identity.
Array literals are either an explicit byte inventory such as
`[1u8, 2u8, 0u8]` or a repeat form such as `[0u8; 32]`. The explicit inventory
determines `N`, including `[]` as `[u8; 0]`; the repeat count must satisfy the
same bound. Elements must be exactly `u8`; there is no integer coercion. The
sum of inline array payload bytes in one function's parameters, locals,
temporaries, and result staging must not exceed 65,536 bytes. Arrays may use
the existing checked Copy aggregate machinery, including record fields, only
when that cumulative bound and every existing aggregate bound hold. They are
not admitted as generic arguments, variant payloads, imports, callbacks, or
public ABI values in v1.

Inline storage accounting is over canonical HIR storage slots, not backend
allocations. Parameters, bindings, required expression/call staging, and the
provisional result are distinct slots. A direct array literal initializer is
materialized into its destination and is not also counted as a separate
temporary. Every executable root also has a 65,536-byte **active-array
call-path budget**. This deterministic semantic over-approximation
checked-sums sequential calls, takes the maximum of alternatives, and counts
caller call staging and callee parameters separately. Any call-graph cycle
from which nonzero array storage is reachable is rejected. The rule is
target-independent so a backend stack model cannot silently narrow the
admitted language.

`Bytes` is a uniquely owned, immutable byte buffer with an exact length in
`0..=65536`. It is non-Copy and NUL-safe. Moving it invalidates the source;
dropping it releases its payload exactly once. It may be passed or returned by
ordinary internal monomorphic functions, but v1 forbids storing it in records,
variants, generic instances, imported signatures, callbacks, async state, or
public target signatures. There is no capacity distinct from length, mutation,
append, reserve, reallocation, implicit UTF-8 interpretation, or implicit
conversion to `string`.

`Slice<u8>` is an immutable borrowed view carrying a root identity, checked
offset, and checked length. It is admitted as an explicit `borrow Slice<u8>`
parameter, as a local compiler-proven view of a live `Bytes`, fixed array,
`str`, or another byte slice, and nowhere else. It is non-owning and
cleanup-inert. It cannot be returned by an ordinary function, stored in an
aggregate, imported, exported as an owned value, moved into an owning
parameter, retained by a callback, cross an async suspension, or be
constructed from integer or pointer values. Compiler-owned view intrinsics
are the only apparent slice-returning declarations and must attach the
authenticated origin in HIR rather than behave as ordinary arbitrary-return
calls.

The exact compiler-owned operations and reserved identities are:

| Source operation | Stable identity | Semantic signature |
| --- | --- | --- |
| `byte_len` | `core.bytes.len` | `(value: borrow Slice<u8>) -> usize` |
| `byte_get` | `core.bytes.get` | `(value: borrow Slice<u8>, index: usize) -> Option<u8>` |
| `bytes_copy` | `core.bytes.copy` | `(value: borrow Slice<u8>) -> Bytes` |
| `bytes_as_slice` | `core.bytes.as-slice` | `(value: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `core.array-u8.as-slice` | `(value: borrow [u8; N]) -> Slice<u8>` |
| `str_as_bytes` | `core.str.as-bytes` | `(value: borrow str) -> Slice<u8>` |

`array_as_slice` is compiler-polymorphic only in the authenticated constant
`N`; it does not introduce user-visible const generics. The operation table is
closed, names are reserved, and calls project as compiler-owned calls plus the
authenticated view facts described below. There are no methods or subscript
syntax in v1.

`byte_get` is total. It returns `Some(value[index])` exactly when
`index < byte_len(value)` and `None` otherwise, including for `usize::MAX`.
It never emits an unchecked C load, a Wasm out-of-bounds access, a trap, a
sentinel byte, an arithmetic failure, or a new bounds-status domain. The
compiler-owned `Option` profile is widened only to the exact Copy payload
`u8` needed by this operation; `Option<usize>`, non-Copy options, and general
generic widening are not implied.

Indexed Byte Loop v2 permits this exact compiler-owned `Option<u8>` result to
be matched inside bounded `while` conditions and bodies. The match must be
exhaustive and guard-free with exact `Some { value: u8 }` and `None {}` cases,
and every arm result remains a Copy scalar. This is an immutable read-only
widening: `byte_len` and `byte_get` are the only byte operations admitted in a
loop, a slice view must already exist, and `bytes_copy`, view construction,
owned values, general variants, effects, imports, and cleanup-bearing work stay
rejected. A dynamic index at or beyond the slice length selects `None` through
the same target-independent semantics as a straight-line read.

`bytes_copy` copies the exact byte sequence, including embedded NUL and bytes
that are not valid UTF-8. It never aliases its input. `str_as_bytes` preserves
the already validated UTF-8 byte sequence and the original invocation root;
it does not allocate or authorize a reverse byte-to-text conversion.

## Ownership, provenance, and borrowing

Each admitted slice has a compiler-authenticated provenance fact:

```text
root identity + root kind + root length + offset + length
```

Static HIR represents an ordinary slice parameter as the symbolic root kind
`function_parameter`; an internal call substitutes the argument's existing
root rather than minting or recharging a root. Only a concrete host entry
materializes such a symbolic root as external invocation input. Derived views
use the concrete root kinds owned `Bytes`, fixed array storage, or borrowed
`str`. A subview must retain the same root.
The verifier and hostile-HIR validator independently prove
`offset <= root_length` and `length <= root_length - offset`; they never prove
range validity by unchecked `offset + length`. Zero-length views are valid at
the end of a root and are normalized without dereferencing a pointer.

HIR records view creation as an explicit, non-consuming borrow of an exact
named storage place. `bytes_as_slice` and `array_as_slice` cannot borrow a
temporary in v1; an owned buffer or array must first be bound to a name so its
cleanup lifetime is unambiguous. Slice-producing expressions and value aliases
have separate authenticated provenance entries. The hostile-HIR validator
independently reconstructs both tables rather than trusting attached facts.

External roots are charged once when admitted. Forwarding, local aliases,
array views, `bytes_as_slice`, `str_as_bytes`, and future subviews of an
already admitted root do not recharge their bytes. Distinct external
parameter positions are distinct roots even if a host passes the same address
or JavaScript object twice. Borrowed `str` and `Slice<u8>` parameters share
one counter: one invocation admits at most 65,536 cumulative external root
bytes across both carrier kinds.

Borrowing is shared and immutable. A live `Slice<u8>` derived from `Bytes`
prevents that owner from being moved or dropped. V1 uses conservative lexical
lifetimes: the restriction lasts until the end of the block that owns the
slice binding, rather than relying on inferred last use. A view derived from
an array similarly prevents replacement of its storage for the lexical
borrow. Branch joins retain the most restrictive live-borrow state. No
mutable borrow, shared mutation, reference identity comparison, reborrowing
across tasks, callback retention, or async lifetime is admitted.

`Bytes` is one owned cleanup leaf. Evaluation remains left-to-right. A call
stages owned arguments in caller-owned epochs and transfers them only at the
existing atomic call commit. Failure is sticky; cleanup may not replace it.
An owned byte result is staged, checked against postconditions, and published
only after non-result cleanup, exactly like every existing owned result.
Its reserved lifecycle identity is `core.bytes.drop`. Type facts remain
`copy=false`, `contains_resource=false`, and `needs_drop=true`; `Bytes` is not
misclassified as a user resource merely to obtain cleanup behavior.

## Allocation and capacity contract

Every byte length and budget calculation uses checked arithmetic. The exact
v1 bounds are:

- 65,536 cumulative external root bytes per invocation;
- 65,536 bytes per fixed array or owned `Bytes` payload;
- 65,536 cumulative inline fixed-array bytes per function frame;
- 16 allocation-producing `bytes_copy` sites on any executable call path;
- 1,048,576 maximum source-derived owned-byte payload bytes on any such path.

The same checked analysis authenticates the active-array call-path budget
described above. Its per-function and per-root summaries are target-neutral
compiler facts and Graph v17 serializes them.

The executable closure containing `bytes_copy` must be acyclic. An
allocation-producing operation is forbidden in a `while` body or condition,
and direct or mutual recursion reaching one is rejected before emission. The
compiler computes the conservative maximum across branches and the sum across
calls; it does not trust a backend counter. This ensures the semantic bound
cannot be exhausted dynamically by a loop and avoids inventing a recoverable
result for physical allocator exhaustion.

After all semantic bounds have passed, physical allocation failure is an
invariant fail-stop, matching the existing owned-string policy; it is not
reported as `None`, an arithmetic error, or partial success. No backend may
weaken a semantic capacity bound because more host memory is available.

## HIR, Graph, layout, and cleanup authority

HIR has distinct type variants for `usize`, `[u8; N]`, `Bytes`, and
`Slice<u8>`. A slice-producing expression records its exact origin and range;
a generic pointer/length pair is not sufficient HIR evidence. Type facts mark
`usize` and `[u8; N]` Copy, `Bytes` uniquely owned/non-Copy, and `Slice<u8>`
borrowed/non-storable/non-droppable.

Graph v17 is mandatory if the reachable resolved program contains any of the
four new types, an array literal, a byte-data operation, or a byte-slice
provenance fact. V17 serializes `N`, root kind and identity, offset/length
expression identities, operation identity, ownership facts, and all exact
limits. V17 has precedence over the existing v16 refutable-match and v15
bounded-while schemas. Programs without byte-data facts select the same Graph
v10-v16 version as before and retain byte-identical Graph JSON and revision
digests.

Native64 and Wasm32 internal fixed-array layouts both have size `N` and
alignment 1. Their layout identity and digest include the element identity
and `N`. `usize` has size/alignment 8/8 on Native64 and is an `i64` Wasm value;
its semantic range is identical on both. `Bytes` and `Slice<u8>` are not
admitted as ordinary aggregate fields, so this profile makes no stable
aggregate layout claim for them.

CleanupPlan v2/v3 may remain selected only if their existing generic
initialize, transfer, call-commit, finalize, stage-result, and publish
transitions express `Bytes` without changing the meaning of any transition.
The canonical builder and independent replay must derive the same byte-buffer
slot, liveness, transfer, and reverse-order drop facts from core HIR. If any
new transition, edge condition, failure producer, or reinterpretation is
needed, the implementation must add CleanupPlan v4 and select it only for the
new profile. Backend-local shadow cleanup or a trusted attached plan is never
an alternative.

## Native representation

The target-independent native carriers are conceptually:

```c
struct spx_slice_u8_v1 {
    const uint8_t *ptr;
    uint64_t len;
};

struct spx_bytes_v1 {
    uint8_t *ptr;
    uint64_t len;
};
```

An empty value is normalized to `ptr == NULL && len == 0`. A non-empty value
requires a non-null pointer. Root admission checks the exact cumulative bound
before the target function begins; derived views check subtraction-based
ranges before pointer arithmetic. C cannot authenticate an arbitrary non-null
pointer, so the host remains responsible for providing readable storage for
the complete admitted external range. The compiler never calls `strlen`,
`strcmp`, or the owned-string runtime for bytes.

`Bytes` allocation reserves exactly `len` bytes, with no hidden terminator or
spare capacity. Drop accepts the normalized empty representation and frees
each non-empty allocation exactly once. V1 public native exports accept
borrowed slice carriers and return only scalar/`usize` results; owned-byte
return ABI, cross-allocator transfer, and public destroy functions remain
closed.

## Wasm and JavaScript representation

Raw public Wasm slice parameters are `(i32 offset, i32 length)` and internal
calls may use one packed `i64` view only after both halves have been validated.
`usize` remains a semantic `i64`; conversion to memory offsets checks the
value before narrowing. The public adapter checks every range against the
declared public scratch region and enforces the 65,536-byte cumulative root
bound before calling the target. Zero-length ranges are accepted only at a
valid boundary. Memory is fixed for the profile; guest code and generated
runtime do not call `memory.grow`.

Owned byte buffers use a compiler-owned, quota-bound handle arena supplied by
the deterministic generated JavaScript runtime. `bytes_copy` snapshots the
validated guest bytes into a fresh `Uint8Array`; handles are never addresses,
zero is invalid, stale generations reject, and cleanup removes the exact live
entry. The arena enforces the same statically authenticated allocation-site
and payload bounds. This runtime is an explicit required compiler import, not
ambient host or network authority. Raw Core-Wasm consumers must provide the
same contract; only the generated package is claimed by v1.

An internal slice carrier is packed exactly as
`((root_word as u64) << 32) | length_u32`. A clear root-word high bit denotes
an authenticated fixed-memory offset; a set high bit denotes an arena token
whose remaining 31 bits are nonzero. Tokens are issued monotonically, are
never reused within one runtime instance, and fail-stop before `0x80000000`
would be exhausted. Every arena import authenticates the exact `(token,
length)` pair and rejects stale or already-dropped tokens. Even empty `Bytes`
owns a nonzero token and arena entry; only a zero-length fixed array normalizes
to root word zero and length zero.

The additive project profile is named `useful-data.v1`. Its selected exports
may accept `borrow Slice<u8>` and return `i64`, `bool`, or `usize`; owned bytes,
arrays, aggregates, effects, imports, callbacks, async, and contracts remain
outside the public profile. Generated TypeScript uses `Uint8Array` inputs and
`bigint` for `usize` results.

The JavaScript facade accepts exactly an ordinary, attached, fixed-length
`Uint8Array`. It rejects `SharedArrayBuffer` backing, resizable backing,
detached buffers, other typed-array element types, `DataView`, and implicit
array-like coercion. It snapshots each argument before writing public scratch,
then packs non-overlapping ranges, checks their cumulative length, and invokes
Wasm synchronously. Mutation of the caller's array after the snapshot cannot
change the call. Separate parameter positions remain separate charged roots.
The wrapper authenticates the exact Wasm digest before instantiation and uses
the existing context-bound Project carrier verification; context-free carrier
inspection remains consistency-only.

## Diagnostics

The source-facing byte-data family is closed:

| Code | Meaning |
| --- | --- |
| `SPX-T260` | malformed, negative, or out-of-range `usize` literal |
| `SPX-T261` | fixed-array length or cumulative inline storage is outside the exact bounds |
| `SPX-T262` | fixed-array literal element/count disagrees with `[u8; N]` |
| `SPX-T263` | byte-data operation has the wrong arity, type, or ownership mode |
| `SPX-T264` | `Slice<u8>` escapes, is stored, or crosses a closed boundary |
| `SPX-T265` | an owner or array storage is moved, dropped, or replaced while lexically borrowed |
| `SPX-T266` | a byte view lacks an admitted source-level provenance origin or has an invalid range |
| `SPX-T267` | the allocation closure is cyclic, reaches a loop, exceeds 16 sites, or exceeds 1,048,576 bytes |
| `SPX-T268` | a new data type is used in an unsupported generic, variant, import, callback, async, or public ABI position |

Ordinary unknown-name, reserved-name, arity, move, and hostile-HIR failures
retain their established diagnostic when it is more specific. `SPX-T266` is
source-facing only for a malformed derivation; forged resolved HIR must fail
closed under the existing HIR-invariant diagnostic rather than being accepted
because a source diagnostic cannot be reproduced. Diagnostics are selected
before backend emission and are stable across targets and optimization levels.

## Legacy preservation

### Checked multiplication correction

The ordinary Core-Wasm emitter and aggregate/status emitter previously lowered
the unsigned multiplication overflow predicate with an eager integer `and`.
Although it included `right != 0`, the `UINT64_MAX / right` operand executed
first, so multiplication by a zero right operand trapped. This contradicted
the existing checked-u64 source contract, the interpreter's `checked_mul`, and
the native helper's short-circuit guard. Zero is a valid multiplier, including
when the other operand is `usize` maximum.

Both Wasm emitters now put the division-based overflow check inside a real
`if right != 0` block. Already evaluated operands are retained and multiplication
still executes afterward. This does not skip operand evaluation: a failure
while computing the left operand must still win even when the multiplier would
be zero. The aggregate emitter includes the new block in its failure-branch
depth, so genuine nonzero overflow still selects multiplication status 3 and
reaches the ordinary cleanup/status epilogue rather than continuing with a
wrapped product. The ordinary Core-Wasm lane retains its existing trap channel
for genuine overflow.

This is an explicit correctness exception to prior blanket artifact-byte
preservation statements: modules that emit `usize` multiplication intentionally
change Wasm bytes, including earlier Project profiles, and their derived npm
and target-evidence integrity bindings change accordingly. It is not a
profile-only workaround. Source meaning, admission, canonical source/HIR/Graph,
CleanupPlan, native code, descriptor schemas and public signatures are unchanged.
Modules that do not emit this operation are outside the correction. No existing
known-answer values are rewritten in this batch.

The authored regression gates cover both Wasm routes; zero on either side,
maximum and exact/overflowing products; nested failure branches; and the same
owned-result source through the interpreter, native O0/O2 and generated npm.
The owned fixture stages real Bytes before multiplication, observes success
consumption or failure cleanup, checks preserved failure output and same-instance
reuse, and checks earlier-addition failure before multiplication by zero.
Native provider failures retain their existing normalized semantic-failure
status; this does not claim a newly exposed native arithmetic-status ABI.
The owned-result cross-target fixture (`tests/usize_mul_owned_v1.rs`) passed
locally on macOS arm64 with Rust 1.98, Apple Clang 21 and Node 24.3. This is
evidence for that fixture's interpreter/native O0/O2/npm cases, not execution
of every authored arithmetic regression or hosted validation. Formatting and
static review alone do not prove target execution or promote byte-data or
checked-arithmetic support.

The feature is additive and must be reachability-gated. When no new type or
operation is reachable:

- canonical source, HIR identities, Graph version/JSON/digest, CleanupPlan
  schema/bytes, native C11, Core-Wasm, Component, Web, npm, Rust SDK, status
  dictionary, and project-carrier bytes are unchanged;
- no byte runtime helper, import, memory, handle arena, wrapper branch, or
  type declaration is emitted;
- Project v1 and Project v2 manifest parsing, diagnostics, artifact inventory,
  and publication behavior remain frozen.

Project Manifest v3 may select `useful-data.v1`; earlier schemas must reject
that profile rather than silently reinterpret it. Disconnected stable-ID
export-root linking and the retained trusted Project binding are reused, not
reimplemented.

## Completion gates

Completion requires all of the following at one exact head:

1. Lexer/parser/canonical formatter round trips for `usize`, both array literal
   forms, all four types, and every operation, plus exact legacy byte KATs.
2. Source checker, iterative HIR resolver, recursive oracle, and hostile-HIR
   validator agreement for every diagnostic and ownership/provenance rule.
3. Graph v17, type-fact, Native64/Wasm32 layout, builder-budget, and legacy
   Graph v10-v16 preservation KATs.
4. Canonical cleanup builder/replay agreement for creation, move, borrowed
   read, call commit, branch, early failure, owned return, postcondition,
   reverse-order drop, and use-after-move rejection.
5. Exact capacity-minus-one, capacity, and capacity-plus-one tests for root,
   array, frame, allocation-site, and cumulative payload bounds.
6. `usize` zero/maximum arithmetic, overflow/underflow, unsigned ordering,
   division/remainder-by-zero, and checked physical-offset conversion tests.
7. Empty, one-byte, 65,536-byte, embedded-NUL, `0xff`, invalid-UTF-8,
   aliased-view, nested-forwarding, index `len - 1`, index `len`, and
   `usize::MAX` execution cases.
8. Interpreter, native C11 O0/O2, and Node/Core-Wasm agreement for every
   success, `None`, arithmetic failure, branch, loop, move, and cleanup case.
9. Hostile native null/length and Wasm offset/length, overflow, public-scratch,
   fixed-memory, stale-handle, double-drop, and quota tests.
10. JavaScript/TypeScript tests for exact input types, detached/shared/
    resizable rejection, snapshot isolation, multiple arguments, cumulative
    charging, digest mismatch, offline pack/install, and compiler-free use.
11. A multi-module binary-frame validator/checksum Project that uses a magic
    fixed array, borrowed slices, total indexed reads, `usize`, existing
    `while`/mutation with an exact `Option<u8>` match per dynamic index, an
    owned copy/move/drop path, explicit stable-ID exports,
    and stable-ID display rename; it must execute equivalently through the
    interpreter, native O0/O2, Wasm, generated JS, and installed npm package.
12. Repository fmt, check, clippy with `-D warnings`, workspace tests,
    rustdoc, package verification, example checks, exact-head hosted Linux,
    macOS, Windows, and Rust 1.85 jobs, followed by an adversarial review with
    no unresolved P0/P1/P2 finding.

Local gates may justify only a local partial claim. Public completion and
completion-matrix promotion require the hosted jobs above at the same commit.

## Nonclaims

Portable Indexed Byte Data v1 does not claim general `Vec<T>`, arbitrary
element arrays or slices, const generics, mutable arrays/slices/bytes, append
or capacity management, unchecked indexing, slice-range syntax, UTF-8
decoding, byte-to-string conversion, owned-byte public return ABI, allocator
interoperability, aggregate/resource/generic data exports, imported byte
input or owned-byte output, files, stdin/stdout, WASI, callbacks, async,
threads, atomics, shared memory, memory growth, SIMD, components, registry
publication, signing, provenance, general aggregate matching in loops, or
general-purpose data processing.
