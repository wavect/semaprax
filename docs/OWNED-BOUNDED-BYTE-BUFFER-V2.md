# Owned Bounded Byte Buffer v2

Audience: language users, tool authors, and compiler contributors.

Status: implementation in progress; execution evidence remains required before
this profile is accepted. This additive internal profile extends
[Owned Bounded Byte Buffer v1](OWNED-BOUNDED-BYTE-BUFFER-V1.md) with one fixed
five-byte write. It changes no grammar, public ABI, graph schema, allocation
rule, capacity ceiling, or authority.

## Source contract

`bytes_set5` is a compiler-owned reserved operation:

| Function | Signature |
| --- | --- |
| `bytes_set5` | `(buffer: own Bytes, index: usize, first: u8, second: u8, third: u8, fourth: u8, fifth: u8) -> Bytes` |

It is admitted wherever the v1 `bytes_set` chain/reopen form is admitted. In a
bounded `while`, the only mutable shape is the exact same-owner replacement:

```semaprax
buffer = bytes_set5(buffer, index, first, second, third, fourth, fifth);
```

The allocation remains outside the loop. The buffer operand is the complete
owned mutable binding and no borrowed view may cross the replacement. A
write-once chain counts `bytes_set5` as five fill elements against the existing
256-element static fill ceiling.

## Semantics

Operands evaluate left to right: buffer, index, then `first` through `fifth`.
After every operand succeeds, the runtime preflights the complete interval using
`index <= length` and `length - index >= 5`. A failed preflight selects the
existing `semaprax.byte-buffer.v1/1` adapter failure before owner transfer or a
store. A successful call transfers the one owner and stores the five bytes in
increasing index order. No partial prefix is published by a failed call.

The source verifier and hostile-HIR validator re-derive the whole-binding,
capacity, static interval, and ownership facts. Cleanup replay represents one
ordinary propagated-call status source and one canonical argument transfer;
there is no new cleanup leaf or graph schema version.

## Targets and limits

The reference interpreter clones the owner once and writes five bytes after the
single preflight. Native C11 emits a checked `spx_bytes_set5` helper. Internal
Core-Wasm emits the same preflight before the sealed `spx_bytes_set5` host
import, whose host-side validation independently rejects forged carriers,
intervals, or byte values. The public Wasm byte adapter remains rejected.

`bytes_set5` neither allocates nor grows a buffer. The v1 `131072` byte capacity
ceiling, allocation-site accounting, fixed Core-Wasm memory, and no-ambient-
authority rules remain in force.

## Required evidence

The owning byte-buffer harness must cover canonical source/graph projection,
source and hostile-HIR admission, the `SPX-T272` literal interval diagnostic,
runtime out-of-range status before owner commit, cleanup replay, interpreter,
native C11 O0/O2, and internal Core-Wasm execution. The frozen catalog oracle
must then run unchanged under its original 100M fuel envelope.
