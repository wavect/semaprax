# Owned Bounded Byte Buffer v2

Audience: language users, tool authors, and compiler contributors.

Status: implementation in progress; execution evidence remains required before
this profile is accepted. This additive internal profile extends
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) with a fixed
five-byte write and an internal one-or-five source read write. It changes no grammar, public ABI, graph schema, allocation
rule, capacity ceiling, or authority.

## Source contract

`bytes_set5` is a compiler-owned reserved operation:

| Function | Signature |
| --- | --- |
| `bytes_set5` | `(buffer: own Bytes, index: usize, first: u8, second: u8, third: u8, fourth: u8, fifth: u8) -> Bytes` |
| `bytes_set1_or5_from_slice` | `(buffer: own Bytes, index: usize, wide: bool, one: u8, source: borrow Slice<u8>, source_start: usize) -> Bytes` |

It is admitted wherever the v1 `bytes_set` chain/reopen form is admitted. In a
bounded `while`, the only mutable shape is the exact same-owner replacement:

```semaprax
buffer = bytes_set5(buffer, index, first, second, third, fourth, fifth);
// or: buffer = bytes_set1_or5_from_slice(buffer, index, wide, one, source, source_start);
```

The allocation remains outside the loop. The buffer operand is the complete
owned mutable binding and no borrowed view may cross the replacement. A
write-once chain counts `bytes_set5` and `bytes_set1_or5_from_slice` as five fill elements against the existing
256-element static fill ceiling.

## Semantics

Operands evaluate left to right: buffer, index, then `first` through `fifth`.
After every operand succeeds, the runtime preflights the complete interval using
`index <= length` and `length - index >= 5`. A failed preflight selects the
existing `semaprax.byte-buffer.v1/1` adapter failure before owner transfer or a
store. A successful call transfers the one owner and stores the five bytes in
increasing index order. No partial prefix is published by a failed call.

`bytes_set1_or5_from_slice` evaluates buffer, index, `wide`, `one`, source,
and `source_start` in that order. It preflights one destination element when
`wide` is false, writes `one`, and never reads the source. When `wide` is true,
it preflights five destination elements, then reads `source_start` through
`source_start + 4` in increasing order. A position outside the borrowed source
contributes `0u8`; it does not select a source-range failure. The source is an
authenticated borrowed slice binding whose root is distinct from the moved
buffer, so no borrowed view can alias the owner across its commit boundary.

The source verifier and hostile-HIR validator re-derive the whole-binding,
capacity, static interval, and ownership facts. Cleanup replay represents one
ordinary propagated-call status source and one canonical argument transfer;
there is no new cleanup leaf or graph schema version.

## Targets and limits

The reference interpreter clones the owner once and writes five bytes after the
single preflight. Native C11 emits checked `spx_bytes_set5` and
`spx_bytes_set1_or5` helpers. Internal Core-Wasm emits the matching selected
preflight before sealed `spx_bytes_set5` and `spx_bytes_set1_or5` host imports,
whose host-side validation independently rejects forged carriers, intervals, or
byte values. The public Wasm byte adapter remains rejected.

`bytes_set5` and `bytes_set1_or5_from_slice` neither allocate nor grow a buffer. The v1 `131072` byte capacity
ceiling, allocation-site accounting, fixed Core-Wasm memory, and no-ambient-
authority rules remain in force.

## Required evidence

The owning byte-buffer harness must cover canonical source/graph projection,
source and hostile-HIR admission, the `SPX-T272` literal interval diagnostic,
runtime out-of-range status before owner commit, cleanup replay, interpreter,
native C11 O0/O2, and internal Core-Wasm execution. The frozen catalog oracle
must then run unchanged under its original 100M fuel envelope. The intrinsic is
private compiler support until that catalog gate passes; no catalog source
conversion is currently admitted.

The cleanup-replay skeleton preflight now charges the actual transition and
edge work for each reachable block visit instead of multiplying every visit by
the program's widest transition. A hostile widened call still fails before
materialization. With the candidate tagged catalog conversion this cleared
`SPX-H006`, but the unchanged catalog selector returned `FuelExhausted` under
the original 100M envelope. The conversion remains out of the branch pending
a source/runtime change that meets that envelope.
