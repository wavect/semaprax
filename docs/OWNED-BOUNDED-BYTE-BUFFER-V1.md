# Owned Bounded Byte Buffer v1

Audience: language users, tool authors, and compiler contributors.

Status: partially implemented with local internal compiler evidence. The
allocate-fill-freeze-read cycle, the compile-time capacity and element-index
rules, the canonical CleanupPlan settlement, the semantic graph projection, and
execution on the reference interpreter and the native C11 backend at O0/O2 have
focused local evidence. The WebAssembly backend rejects the feature with an
exact diagnostic. A growable vector, elements wider than one byte, a public FFI
layout, and a `std.*` interface are all open and are not claimed here.

Owned Bounded Byte Buffer v1 is the first owned bounded collection in the
language. It adds two compiler-owned operations to
[Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) so a program
can build an owned `Bytes` buffer from computed element values instead of only
copying one that already exists. It introduces no new type, no new cleanup leaf
kind, no graph schema version, and no new mutation syntax.

## What this deliberately is not

`push` in a loop is not buildable today, and this document does not pretend
otherwise. Two independent rules block it, and both must be decided before a
growable vector is designed:

- `src/byte_data_capacity.rs` rejects an owned byte allocation reachable from a
  `while` condition or body, and `MAX_BYTES_COPY_SITES` counts *static* sites,
  not loop iterations.
- The WebAssembly backend cannot allocate at all. `FIXED_MEMORY_PAGES` has
  `min == max`, no `memory.grow` is emitted anywhere, and owned bytes there are
  opaque host-arena tokens reached through a frozen `env` import set.

This tranche therefore fixes the capacity at the allocation site and writes
every element at a literal index, which needs neither loop-reachable allocation
nor growth.

## Ownership and borrowing model

The model is stated before the syntax, because the syntax exists to make it
checkable.

1. **One owner.** A buffer has exactly one owner at every point in its life.
2. **Never Copy.** A buffer is `Bytes`. It is uniquely owned and needs drop; no
   assignment, argument, or result duplicates it.
3. **Write-once, then frozen.** Filling is a single expression. The partially
   filled buffer is never bound, never borrowed, and never observable. Binding
   the expression's result is the freeze; from that point the buffer is one
   immutable owned value reached only through the established borrowed reads.
4. **No stale view across the freeze.** Because no intermediate state is
   nameable, a borrowed view cannot exist during the fill. After the freeze,
   moving the owner while a lexical view is live remains the established
   `SPX-T265` rejection.
5. **Atomic failure.** Capacity exhaustion, an out-of-range element index, a
   capacity that is not known at the allocation site, and a capacity above the
   admitted owned byte payload extent are all compile-time diagnostics. No
   partially constructed buffer is ever produced, so there is nothing to unwind.
6. **Exactly one destruction path.** The allocation temporary, each call
   argument, and each intermediate result are separate canonical CleanupPlan
   slots, but every one of them is *transferred* into the next chain link. The
   frozen binding is the only slot any exit finalizes, through the existing
   `core.bytes.drop` lifecycle.

## Source contract

Two compiler-owned operations join the existing byte family. Their names are
reserved; declaring one is `SPX-S113`.

| Function | Signature |
| --- | --- |
| `bytes_zeroed` | `(count: usize) -> Bytes` |
| `bytes_set` | `(buffer: own Bytes, index: usize, value: u8) -> Bytes` |

One buffer is exactly one *write-once chain*: a `bytes_zeroed` call, optionally
wrapped in `bytes_set` links.

```semaprax
module app.buffer;

@id("app.main")
fn main() -> i64
{
    let buffer = bytes_set(bytes_set(bytes_zeroed(2usize), 0usize, 65u8), 1usize, 66u8);
    let view = bytes_as_slice(buffer);
    if byte_len(view) == 2usize { 0 } else { 1 }
}
```

The admission rules are:

- `bytes_zeroed`'s `count` operand is a `usize` literal of at most `65536`
  (`SPX-T271`). The capacity is therefore known at the allocation site, which
  is what the target-neutral capacity analysis and both backends require.
- `bytes_set`'s `buffer` operand is syntactically the enclosing chain's
  previous `bytes_zeroed` or `bytes_set` call (`SPX-T271`). A named binding is
  a frozen buffer and can never be re-opened.
- `bytes_set`'s `index` operand is a `usize` literal strictly below the chain's
  capacity (`SPX-T272`).
- A chain holds at most `256` `bytes_set` links.
- Neither operation is admitted in a `while` condition or body. The byte-family
  rule reports `SPX-T252` and the owned byte allocation rule reports
  `SPX-T267`.

Reading a frozen buffer uses the existing operations unchanged: `bytes_as_slice`
for the borrowed view, `byte_len` for the length, `byte_get` for the checked
`Option<u8>` lookup, and `byte_range` for a sub-view. Deterministic iteration is
the existing Indexed Byte Loop v2 shape over the frozen buffer's view.

## Capacity

`bytes_zeroed` joins the established owned byte allocation family rather than
introducing a second accounting rule. One call is one allocation site, its
literal capacity is its payload contribution, and the existing
`MAX_BYTES_COPY_SITES` site count, `MAX_OWNED_BYTE_PAYLOAD_BYTES` payload sum,
and loop-reachability rejection all apply unchanged. `bytes_set` allocates
nothing: it transfers the single live owner in, stores one byte, and hands the
same owner back.

## Trust boundary

The source verifier admits the chain from source text. HIR validation
re-derives the identical fact from resolved HIR alone, so a graph, patch, or
transaction that never passed through source cannot forge a buffer with an
unknown capacity, an out-of-range element index, or a second owner; every such
forgery is `SPX-H006`.

The native runtime additionally refuses an out-of-range store as a runtime
invariant failure. That path is unreachable from an admitted program and exists
only so a defect can never become a silent truncation.

## Target support

| Route | Support |
| --- | --- |
| Reference interpreter | Executes the full cycle. |
| Native C11 (O0 and O2) | Executes the full cycle through `spx_bytes_zeroed` and `spx_bytes_set`. |
| WebAssembly | Rejected with `SPX-W110`, naming the operation and the missing host-arena protocol. |

The WebAssembly rejection is deliberate. Admitting the feature there needs new
`env` host imports and a host protocol for allocating and mutating an arena
token; lowering a partial buffer instead would break the equal-behavior
invariant.

## Open gates

- Decide the WebAssembly allocation story (`memory.grow` versus a host-arena
  protocol) and then admit the feature on that backend.
- Element types wider than one byte, which need either an `Option<i64>`
  compiler-owned return or a stride-aware read family.
- A loop-driven fill, which needs the two blockers above resolved.
- A public FFI or project-boundary layout. The single admitted owned parameter
  shape crossing a project boundary is unchanged by this document.
- A `std.*` interface, once the compiler-owned host surface moves behind one.
