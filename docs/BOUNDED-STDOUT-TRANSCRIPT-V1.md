# Bounded Stdout Transcript v1

Status: locally evidenced. This justifies only a Partial claim; exact-head
hosted promotion remains separate.

## Objective

This tranche adds the first checked-language output operation without granting
ambient stdio authority or exposing partial host effects during semantic
execution. Source uses the existing capability syntax:

```semaprax
permit { process.stdout.write }

@id("example.emit")
fn emit(value: borrow Slice<u8>) -> usize
    uses { process.stdout.write }
{
    stdout_write(value)
}
```

`stdout_write` is compiler-owned with stable identity
`core.host.stdout-write` and exact signature
`(value: borrow Slice<u8>) -> usize`. The name and identity are reserved. The
call requires the exact `process.stdout.write` effect in the containing
function and module permit; ordinary `SPX-E101`/`SPX-E102` propagation remains
authoritative.

## Semantic transcript

`stdout_write` does not call libc stdio, WASI, JavaScript console APIs, an
arbitrary callback, or an operating-system handle. It atomically appends the
exact bytes of its authenticated slice to a fresh invocation-owned semantic
transcript and returns the exact semantic `usize` length.

The transcript is staged during evaluation. It becomes observable only when
the root invocation reaches terminal success after contracts, cleanup, and
result publication. Any semantic failure, capacity failure, interpreter guard,
or target invariant discards the staged transcript. This success-only seal
keeps interpreter, native, and Wasm behavior equivalent and prevents a later
checked failure from leaving externally visible partial language output.

A separate fixed-purpose application adapter may physically flush one sealed
transcript after successful invocation. Adapter flush failure is an adapter
failure; it cannot retroactively become checked-language success or a semantic
status.

## Static admission and bounds

The complete executable closure is analyzed target-independently:

- at most one `stdout_write` executes on any call path;
- a loop condition/body may not reach `stdout_write`;
- a direct or mutual call cycle that can reach `stdout_write` is rejected;
- the slice must be an existing unprojected `Slice<u8>` place with authenticated
  provenance;
- the transcript is at most 65,536 bytes, inherited from the existing exact
  slice/root bound;
- contracts, generic templates, imports, callbacks, async state, and ordinary
  resource/aggregate boundaries remain closed.

Sequential calls sum, alternatives take their maximum, and calls include the
callee summary. Because every admitted invocation starts with an empty
65,536-byte transcript and can execute at most one write of an already bounded
slice, the operation is semantically infallible and never truncates. Invalid
source shapes or an excessive/cyclic/loop-reachable output plan use `SPX-T269`;
forged HIR fails `SPX-H006`.

## HIR, cleanup, and Graph

HIR represents the intrinsic as an ordinary call with the exact compiler-owned
callee identity, one borrowed slice argument, and a value `usize` result. The
source resolver and hostile-HIR validator independently authenticate the
signature, provenance, effect, and capacity summary.

The operation introduces no owned storage slot, finalizer, imported status,
or failure producer. Existing call argument evaluation and Copy-result staging
remain left-to-right. The transcript is invocation runtime state, not a
resource leaf, callable-settlement action, or cleanup-plan authority.

Reachable stdout transcript meaning selects additive Graph v18 above v17 and
serializes the exact operation, effect, capacity summary, transcript bound, and
success-only publication policy. Programs without the operation retain their
prior Graph schema, HIR, cleanup, native, Wasm, and package bytes.

## Target behavior

- The hosted reference interpreter has a separate effect-admitting API and
  envelope carrying the sealed transcript. Existing `semaprax.interpret.v1`
  remains effect-free and byte-compatible.
- Native code receives invocation-owned fixed-capacity transcript storage
  explicitly. Generated code copies bytes after slice validation and exposes
  the transcript only on root success; it never writes a process descriptor.
- The public Wasm command profile admits only a slice rooted in one selected
  external input parameter. `stdout_write` records its authenticated scratch
  pointer and length in private guest globals; it copies into the exported
  transcript range only after target status and the canonical result carrier
  both succeed. A throwing consumer-supplied import therefore leaves the range
  zero even under raw instantiation. The generated facade additionally exposes
  bytes only after arena settlement and wipes the complete range on every
  primary or settlement failure. No stdout, WASI, console, or mutable host-sink
  import is introduced. Raw general stdout-profile Wasm emitters remain
  crate-private.

## Required evidence

Evidence must cover source/resolver/hostile-HIR parity, missing permits/effects,
forged identities/types/provenance, contract and loop rejection, direct and
mutual cycle rejection, zero/one/65,536-byte writes, sequential and branch
path accounting, failure-after-write transcript discard, cleanup replay with
live owned data, fresh and repeated invocation isolation, interpreter/native
O0/O2/Wasm-Node transcript equivalence, strict import inspection, and exact
legacy Graph/backend/package preservation.

## Nonclaims

This tranche does not add stdin, arguments, files, directories, environment,
network, stderr, arbitrary sinks, callbacks, streaming, multiple writes,
partial transcript observation, flushing during semantic execution, async,
threads, WASI, Component Model I/O, terminal behavior, encoding, line
buffering, locale, process inheritance, or physical write durability.
