# Owned Bounded Byte Buffer v2

Audience: language users, tool authors, and compiler contributors.

Status: implementation in progress; execution evidence remains required before
this profile is accepted. This additive internal profile extends
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) with a fixed
five-byte write and internal tagged one-or-five and one-or-six-or-forty-eight source reads. It changes no grammar, public ABI, allocation rule, capacity
ceiling, or authority. Its exact same-owner assignment participates in the
additive cleanup/graph profile described below.

## Source contract

`bytes_set5` is a compiler-owned reserved operation:

| Function | Signature |
| --- | --- |
| `bytes_set5` | `(buffer: own Bytes, index: usize, first: u8, second: u8, third: u8, fourth: u8, fifth: u8) -> Bytes` |
| `bytes_set1_or5_from_slice` | `(buffer: own Bytes, index: usize, one: u8, source: borrow Slice<u8>, selector: usize) -> Bytes` |
| `bytes_set1_or6_or48_from_slice` | `(buffer: own Bytes, index: usize, one: u8, source: borrow Slice<u8>, selector: usize) -> Bytes` |

It is admitted wherever the v1 `bytes_set` chain/reopen form is admitted. In a
bounded `while`, the only mutable shape is the exact same-owner replacement:

```semaprax
buffer = bytes_set5(buffer, index, first, second, third, fourth, fifth);
// or: buffer = bytes_set1_or6_or48_from_slice(buffer, index, one, source, selector);
```

The allocation remains outside the loop. The buffer operand is the complete
owned mutable binding and no borrowed view may cross the replacement. A
write-once chain counts `bytes_set5` and `bytes_set1_or5_from_slice` as five fill elements and `bytes_set1_or6_or48_from_slice` as forty-eight against the existing
256-element static fill ceiling.

## Semantics

Operands evaluate left to right: buffer, index, then `first` through `fifth`.
After every operand succeeds, the runtime preflights the complete interval using
`index <= length` and `length - index >= 5`. A failed preflight selects the
existing `semaprax.byte-buffer.v1/1` adapter failure before owner transfer or a
store. A successful call transfers the one owner and stores the five bytes in
increasing index order. No partial prefix is published by a failed call.

The tagged source operations evaluate buffer, index, `one`, source, and
`selector` in that order. The selector's high bit chooses source-copy mode.
With that bit clear, each operation preflights one destination element, writes
`one`, and never reads the source. The one-or-five operation uses the lower 63
bits as its source offset and copies five bytes when tagged. The
one-or-six-or-forty-eight operation uses bit 62 to choose forty-eight versus
six bytes when tagged; its lower 62 bits are the source offset. Each selected
width is preflighted in full before owner transfer or a store. Reads and
stores proceed in increasing order. A position outside the borrowed source
contributes `0u8`; it does not select a source-range failure. The source is an
authenticated borrowed slice binding whose root is distinct from the moved
buffer, so no borrowed view can alias the owner across its commit boundary.

The source verifier and hostile-HIR validator re-derive the whole-binding,
capacity, static interval, and ownership facts. Cleanup replay represents one
ordinary propagated-call status source and one canonical argument transfer;
there is no new cleanup leaf. An exact same-owner assignment selects the
additive CleanupPlan v17 and Graph v70 contract owned by
[Same-Owner Byte-Buffer Renewal v1](BYTE-BUFFER-RENEWAL-V1.md).

## Targets and limits

The reference interpreter clones the owner once and writes the selected width
after a single preflight. Native C11 emits checked `spx_bytes_set5`,
`spx_bytes_set1_or5`, and `spx_bytes_set1_or6_or48` helpers. Internal Core-Wasm emits
the matching selected preflight before sealed corresponding host imports,
whose host-side validation independently rejects forged carriers, intervals, or
byte values. The public Wasm byte adapter remains rejected.

These operations neither allocate nor grow a buffer. The v1 `131072` byte capacity
ceiling, allocation-site accounting, fixed Core-Wasm memory, and no-ambient-
authority rules remain in force.

## Required evidence

The owning byte-buffer harness must cover canonical source/graph projection,
source and hostile-HIR admission, the `SPX-T272` literal interval diagnostic,
runtime out-of-range status before owner commit, cleanup replay, interpreter,
native C11 O0/O2, and internal Core-Wasm execution. The frozen catalog oracle
must then run unchanged under its original 100M fuel envelope. These operations
remain private compiler support until that catalog gate passes.

The cleanup-replay skeleton preflight now charges the actual transition and
edge work for each reachable block visit instead of multiplying every visit by
the program's widest transition. A hostile widened call still fails before
materialization. With the candidate tagged catalog conversion this cleared
`SPX-H006`, but the unchanged catalog selector returned `FuelExhausted` under
the original 100M envelope. The current candidate groups eight adjacent
validated canonical lowercase `\u00hh` controls into one 48-byte append and
uses six-byte appends for shorter runs. A bounded scalar scan classifies the
run; unused predecessor whole-response length walkers are removed from the
application graph. Scalar selection, width, byte, and
cursor-advance helpers keep those branches outside the owning response loop;
otherwise independent branch choices multiply its cleanup paths beyond the
unchanged 65,536 ceiling. Advancing a copied run immediately classifies its
successor, allowing consecutive 48-byte writes. The owning 100M gate remains
unverified until the complete original selector passes.
