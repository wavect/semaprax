# Bulk UTF-8 String v1

Status: source implementation and focused regressions authored; execution and
application qualification pending. OPT-731 remains open. Source-derived copy
counts do not establish runtime, agent-token, or accepted-task cost savings.

## Contract

```text
string_from_utf8(input: borrow Slice<u8>) -> string
```

The exact compiler identity is `core.string.from_utf8`. This pure, monomorphic
intrinsic borrows one authenticated byte slice and returns a fresh owned String.
It accepts exactly canonical UTF-8: reject overlong encodings, isolated or wrong
continuations, truncated sequences, surrogate code points and values above
U+10FFFF. Empty input is valid. Preserve exact bytes, including NUL, BOM and
Unicode noncharacters; do not normalize, replace or strip them.

A malformed input selects the existing nonretryable adapter status
`semaprax.convert.v1`, code `1`. The aggregate Wasm encoding is status `21`.
Validate the complete selected slice before allocating or logically charging a
result. Do not inspect bytes outside a checked range. Failure leaves the result
unpublished and does not consume the source. The existing allocator/capacity
policy remains authoritative; this operation adds no recoverable allocator
status or per-input raw byte cap.

Success performs one final payload allocation/copy. The result owns independent
bytes, so its source owner may settle after the call. The call cannot extend a
borrow, export a borrowed result, erase a streaming epoch or make a stale range
valid. Operand evaluation and range validation precede the ordinary zero-owned
call commit. Enclosing owned calls still stage left to right and transfer their
arguments together; a conversion failure releases the staged prefix through the
canonical cleanup plan without replacing the selected status. Lazy operands
remain lazy.

## Admission and authority

This is an additive ordinary source-language intrinsic, outside the frozen
five-operation `StringOp::CONVERSIONS` catalog. The parser and canonical formatter
use the existing call syntax; source and HIR independently replay the exact
callee identity, one borrowed Slice operand, owned String result, no type
arguments or function instance, and ordinary loan/cleanup facts. Authored
functions, types, fields, templates and instances gain no authority from using
the reserved name or identity. Graph and retained cache use existing Call
encoding, and source-bound graph replay rejects changed bytes or identities.

Named borrowed slices may be copied inside ordinary String loops. Their complete
provenance and lifetime are independently replayed; no byte allocation or new
view-construction class is admitted in loops. Already admitted byte-range and
String-view expressions retain their existing loop rules.

[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md#admitted-shapes) admits the
ordinary compiler String-operation class (its admitted-shapes item 2).
[Stream Text Command v1](STREAM-TEXT-COMMAND-V1.md) admits private owned String
results alongside borrowed byte-slice parameters; [v27](STREAM-DATA-COMMAND-V1.md)
inherits that runtime/signature class, and [v29](STREAM-DATA-COMMAND-V2.md) adds
independently authenticated nominal carriers. Those ordinary private closures
can use this checked intrinsic. Their external roots, capabilities, carrier
layouts, allocation policies and work limits do not change. Frozen owned
collection operation sets do not acquire new collection operations.

Explicitly closed adapters remain closed: Project v10/public owned UTF-8
rejects compiler String intrinsics; standalone internal-String arena selectors
reject this byte-slice constructor; scalar/public Wasm adapters retain their
conversion refusal, including when an existing Map selects aggregate lowering.
Core aggregate Wasm support is not a new streaming host or public package ABI.
Terminated native String representations also refuse this operation.

## Runtime ownership and accounting

- The interpreter validates with strict UTF-8, then uses its ordinary
  `materialize_utf8_copy` seam once. Where a Fixed budget is selected, charge
  one logical materialization and exactly the selected byte length atomically.
  Empty success charges one and zero bytes. The evaluator-wide counters remain
  cumulative across private calls; failure does not debit or refund them.
  Existing UnlimitedLegacy application selections remain unchanged. The fixed
  4096/65536 policy is not attributed to v30/v31 command evaluation.
- Native C11 reads the authenticated slice carrier, validates without copying,
  then uses the existing length-delimited String header/payload constructor
  once. NUL never becomes a terminator. Cleanup publishes only a successful
  String and uses the existing allocator/failure policy.
- Core aggregate Wasm conditionally imports
  `spx_string_from_utf8_v1(i64 input, i32 output) -> i32`. The guest authenticates
  the borrowed/range carrier before the synchronous host call. The host resolves
  the exact source range without copying, validates UTF-8, then creates one
  ordinary owned payload and writes the result only on success. Status 21 is
  the only checked refusal; unknown host statuses retain invariant traps.
  Existing payload/owner limits and descriptor authority checks are unchanged.

## Owning gates

All following gates are authored, not executed by this source batch:

- `--lib string_ops::bulk_utf8_tests::` (3): canonical/graph/cache round trip,
  stale source/identity, forged signatures/instances/ownership, source loans,
  type/arity/generic/reserved-name diagnostics and frozen adapter refusal.
- `--lib interpreter::string_operations::bulk_utf8_tests::` (2): exact one-copy
  logical accounting, count/byte boundaries, malformed-before-charge, shared
  private-call meter and empty input.
- `--test owned_data bulk_utf8::` (3): interpreter, native C11 O0/O2, strict
  private Wasm host and production browser adapter. Valid Unicode/NUL/BOM/
  noncharacters, invalid bytes outside selected ranges, detached results,
  empty/131072-byte internal inputs and repeated loop calls; thirteen malformed
  classes; lazy paths and both orders of conversion/arithmetic failure with
  staged String/Bytes cleanup. Native payload allocation counts and strict-host
  payload-copy counts pin one per success and zero for malformed input.
- Existing `hir::validation::string_intrinsic::tests` and
  `hir::validation::call_parameters::tests` enumerate all 37 StringOps,
  including the new exact reserved identity and borrowed parameter mode.

Regenerate only affected authentic artifact pins in the grouped qualification
flow. The production browser runtime source changes, but ordinary Call graph
and cache formats and old intrinsic/import ordering do not. Original application
acceptance and matched agent benchmarks remain separate obligations.
