# Owned Bounded Byte Buffer v1

Audience: language users, tool authors, and compiler contributors.

Status: partially implemented with local internal compiler evidence. The
allocate-fill-freeze-read cycle, the compile-time capacity rule, the
compile-time and run-time element-index rules, the canonical CleanupPlan
settlement, the semantic graph projection, and execution on the reference
interpreter and the native C11 backend at O0/O2 have focused local evidence. The internal Core-Wasm backend executes the same exact
profile through bounded host-arena imports with focused local Node evidence. A
growable vector, elements wider than one byte, a public FFI or Project layout,
a hosted or browser support claim, and a `std.*` interface are all open and are
not claimed here.

Owned Bounded Byte Buffer v1 is the first owned bounded collection in the
language. It adds two compiler-owned operations to
[Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) so a program
can build an owned `Bytes` buffer from computed element values instead of only
copying one that already exists. It introduces no new type, no new cleanup leaf
kind, no graph schema version, and no new mutation syntax.

## What this deliberately is not

Capacity *growth* in a loop is not buildable today, and this document does not
pretend otherwise. A loop-carried *fill* of an already allocated buffer is
admitted; see [Source contract](#source-contract). Two independent rules block
growth, and both must be decided before a growable byte vector is designed:

- `src/byte_data_capacity.rs` rejects an owned byte allocation reachable from a
  `while` condition or body, and `MAX_BYTES_COPY_SITES` counts *static* sites,
  not loop iterations.
- Core-Wasm linear memory cannot grow. `FIXED_MEMORY_PAGES` has `min == max`,
  and the exact owned buffer profile instead uses opaque host-arena tokens
  reached through the frozen `env.spx_bytes_zeroed` and `env.spx_bytes_set`
  imports. This is not a general allocator or mutable collection ABI.

This tranche therefore fixes the capacity at the allocation site, which needs
neither loop-reachable allocation nor growth. The *element index* is not fixed:
it is any admitted `usize` expression, so an offset a scan discovers can be
written, including from inside a bounded `while`. Only the allocation is static.

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
5. **Atomic failure.** A capacity that is not known at the allocation site, a
   capacity above the admitted owned byte payload extent, a literal element
   index at or above the capacity, and any index into an empty buffer are
   compile-time diagnostics. A *computed* element index outside the buffer is
   the one run-time failure. It is selected before the owner transfer commits
   and before any byte is written, so no partially filled buffer is ever
   produced or observable, and the buffer is destroyed by the canonical
   CleanupPlan exit that the failed store never consumed.
6. **Exactly one destruction path.** The allocation temporary, each call
   argument, and each intermediate result are separate canonical CleanupPlan
   slots, but on the success path every one of them is *transferred* into the
   next chain link, and the frozen binding is the only slot the success exit
   finalizes. Each `bytes_set` additionally owns one element-bound failure
   exit, which finalizes exactly the call-argument slot that store did not
   consume. No exit ever destroys more than one owner, and every destruction
   goes through the existing `core.bytes.drop` lifecycle.

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
  a frozen buffer, with exactly one exception: the *same-owner replacement*
  `buffer = bytes_set(buffer, index, value)`, whose assignment target and whose
  `buffer` operand are the same `let mut` binding. The call moves the single
  owner out of the binding and the assignment publishes the returned owner back
  into it, so exactly one generation is live at every point and the buffer is
  never nameable half-filled. Any other named binding in the operand — a second
  owner, or a `let` that does not republish the operand — stays `SPX-T271`, and
  a borrowed view that is live across the replacement is `SPX-T265`.
- `bytes_set`'s `index` operand is any `usize` expression. A *literal* index at
  or above the chain's capacity is `SPX-T272`, and so is any index into a
  zero-capacity buffer, because neither can ever name an element. Every other
  index is admitted and checked at run time; see [Element bound](#element-bound).
- A chain holds at most `256` `bytes_set` links.
- `bytes_zeroed` is not admitted in a `while` condition or body: the allocation
  stays outside the loop. The byte-family rule reports `SPX-T252` and the owned
  byte allocation rule reports `SPX-T267` independently. `bytes_set` *is*
  admitted there, but only through the same-owner replacement above — the
  loop-carried fill. A `bytes_set` in a `while` that is not that assignment's
  right-hand side is still `SPX-T252` or `SPX-T271`.

Reading a frozen buffer uses the existing operations unchanged: `bytes_as_slice`
for the borrowed view, `byte_len` for the length, `byte_get` for the checked
`Option<u8>` lookup, and `byte_range` for a sub-view. Deterministic iteration is
the existing Indexed Byte Loop v2 shape over the frozen buffer's view.

## Element bound

An index the compiler cannot bound is checked on every backend, with one
normalized status and one selection rule.

| Field | Value |
| --- | --- |
| Domain | `semaprax.byte-buffer.v1` |
| Code | `1` (`index_out_of_bounds`) |
| Class | `adapter` |
| Retryable | `false` |

The rule is the same three sentences on every route:

1. The operands are evaluated left to right: buffer, index, value.
2. The store fails when `index >= length`, where `length` is the buffer that
   was staged as the operand. Capacity is a literal at the allocation site, so
   the transferred buffer's length and the chain capacity are the same number.
3. The failure is selected **before** the owner transfer commits, so the store
   writes nothing and the buffer stays in its canonical call-argument slot for
   that exit's single finalizer.

Because failure precedes the commit, no backend invents a destruction of its
own; the canonical CleanupPlan owns it, and the `bytes_set` call carries an
ordinary `PropagatedCall` status source that independent replay re-derives.

| Route | Where the bound is enforced |
| --- | --- |
| Reference interpreter | `src/interpreter/owned_buffer.rs` compares the index against the transferred owner's length and returns the normalized status. |
| Native C11 | `spx_bytes_set_check_v1` records the adapter status; generated code branches to the epilogue before calling `spx_bytes_set`. |
| Internal Core-Wasm | Generated code compares the index against the carrier's byte length and selects internal status value `16`, which the ordinary Web wrapper maps back to `semaprax.byte-buffer.v1` code `1`. |

The Core-Wasm host import keeps its own independent gate on the carrier, index,
and value. Admitted programs can no longer reach it, because generated code
fails first; it remains so that a compiler or host defect cannot become a silent
truncation or a store into an unauthenticated arena entry.

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

The native and Core-Wasm host runtimes additionally refuse an out-of-range
store, as a runtime invariant failure and a host rejection respectively.
Core-Wasm also authenticates the opaque carrier and the exact `usize`/`u8`
import arguments before mutating the same arena entry. Those paths are
unreachable from admitted source, which selects the element-bound failure
first, and exist only so a compiler or host defect can never become silent
truncation or new authority.
The public byte-export adapter rejects any program using this internal-only
profile with `SPX-W115`; the host imports do not widen a public descriptor.

## Target support

| Route | Support |
| --- | --- |
| Reference interpreter | Executes the full cycle. |
| Native C11 (O0 and O2) | Executes the full cycle through `spx_bytes_zeroed` and `spx_bytes_set`. |
| Internal Core-Wasm | Executes the exact cycle through frozen host-arena imports. Focused local Node evidence covers three in-place writes and reads, computed in-range offsets, a computed out-of-range offset selecting `semaprax.byte-buffer.v1` code 1, repeated success, element-bound-failure and contract-failure re-entry at one live arena entry, deterministic valid modules, and absence of `memory.copy` and `memory.grow`. |
| Public Wasm byte adapter | Rejected with `SPX-W115`; no descriptor or public owned-buffer ABI is admitted. |

This is local internal target evidence, not hosted, browser, cross-platform, or
production support. Interpreter and native behavior remain covered by their
existing focused gates.

## Open gates

- Element types wider than one byte, which need either an `Option<i64>`
  compiler-owned return or a stride-aware read family.
- A computed *capacity*. `SPX-T271` still requires a literal at the allocation
  site, because the target-neutral owned byte capacity analysis and the
  Core-Wasm arena both size from it.
- Capacity growth. Neither the exact host-arena protocol nor fixed Core-Wasm
  linear memory admits it. A loop-driven *fill* at a fixed capacity is admitted
  and is no longer an open gate.
- A public FFI or project-boundary layout. The single admitted owned parameter
  shape crossing a project boundary is unchanged by this document.
- A `std.*` interface, once the compiler-owned host surface moves behind one.
