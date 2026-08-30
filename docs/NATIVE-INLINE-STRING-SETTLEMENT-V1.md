# Native Inline String Settlement v1

Status: corrective implementation and regression fixtures authored but unrun;
no new backend, package, or production promotion.

Audience: compiler contributors and native-runtime reviewers.

## Scope

This correction applies to ordinary C11 generation (`emit_c` and `emit_hir_c`)
and the bounded stdout-transcript lane. It implements the existing inline
String cleanup convention for already admitted source. It adds no syntax,
types, intrinsic operations, public ABI, schema, import, or ambient authority.

The resource cleanup inventory deliberately excludes `String`; its physical
allocation ownership is separate from resource CleanupPlan slots. This
correction reuses the private owner-cell machinery introduced for the
[v10 owned UTF-8 provider](PUBLIC-OWNED-UTF8-API-V1.md), without changing
resource inventory, canonical plan ordering, or independent plan replay.

## Exact ownership

Each emitted function is checked for String parameters, result, body
expressions, preconditions, and postconditions. Only a String-bearing ordinary
or stdout-transcript function activates the ledger and bounded staged output.
All owner flags and temporary pointer cells are initialized at function entry,
before any recoverable failure branch; block-local addresses are not retained.

- Literal, clone, and successful call results establish one live owner.
- Place reads retain the existing clone behavior; a temporary handoff moves
  its value and clears its source ownership.
- Binding, branch, match, and provisional-result transfers require a live
  source and dead destination. A live cell must not be overwritten on reuse.
- All arguments evaluate left to right under caller ownership. After staging
  and ownership preflight, the complete String argument group transfers to the
  callee. Plain transport aliases are not additional cleanup owners.
- Normal scope exit and explicit operand consumption clear ownership before
  freeing, including before loop-cell reuse.
- Recoverable failure uses the common epilogue to settle every remaining
  owner, including parameters and provisional results. Cleanup does not
  replace the selected status or write caller result storage.
- Success settles non-result owners before publication. Result ownership is
  relinquished only after the caller store.

Allocation exhaustion, runtime invariant failure, foreign unwinding, signals,
and `longjmp` do not gain recoverable settlement guarantees. The existing
status/context/out-slot ABI and caller storage preconditions remain unchanged.

## Runtime discovery

Ordinary generation emits monomorphic functions and materialized generic
instances. String runtime discovery must inspect both inventories, including
contracts. A generic body can contain String locals or intrinsic calls even
when its caller's signature and expressions contain no String values.

The same discovery selects the existing base, first-wave intrinsic, and
breadth-v2 intrinsic helper groups. No helper is added merely because an
uninstantiated template mentions Strings. String-free ordinary functions keep
the direct output sink and its budget accounting.

## Compatibility

String-bearing ordinary and stdout-transcript C intentionally changes, as do
native byte counts, digests, and integrity facts that bind those exact current
compiler outputs. Target Evidence continues to bind the production emitter;
it must not use a stale alternate emitter to preserve a digest. This includes
dependent Semantic Patch Evidence v2 native bindings when their subject uses
Strings. Source, HIR, Graph, CleanupPlan, status, report, and manifest schemas
do not change. Existing String-free known answers remain unchanged.

The dedicated v8/v9 owned-data provider and existing v10 provider retain their
previous output and budget selection. The three versioned command profiles
(Useful Data Command, Language Command I/O, and Line Command I/O) remain
unchanged, including emitted but unselected functions. Their selected closures
do not admit owned Strings. The scalar Native Rust SDK uses its separate
admitted-closure renderer and is not redirected through this correction.

## Authored evidence and remaining gaps

Loop fixtures retain existing Copy-only loop admission. Ordinary native
condition/body cases use scalar-signature helpers that allocate and settle
one String inside each call; direct String storage in a loop remains
`SPX-T252`. These fixture corrections are authored and unrun, not a language
or backend admission extension.

`tests/native_string_settlement_v1.rs` generates ordinary production C and
observes its actual allocations/frees with the existing fixed-table test
allocator. It checks normalized statuses, poisoned out-slot preservation,
scope and call transfers, contract/provisional failures, loop reuse, String
operations, branches/matches, mixed Bytes, and reuse after failure at O0/O2.
The same context survives 32 ordinary rounds with exactly 13 appended failure
statuses per round. A separate generic-instance-only fixture exercises helper
discovery and cleanup; stdout-transcript fixtures check failure after output
and empty successful output without retaining String allocations.
The separately selected sanitizer case adds ASan/UBSan observations; neither
it nor the ordinary fixture has been executed in this batch.

Focused emitter units cover String presence, generic-instance helper
discovery, bounded output, String-free emission, and frozen profile selection.
Existing String-operation diagnostics and value-conformance fixtures remain
required; physical native evidence does not replace them.

Focused execution commands for a subsequently provisioned environment (not run
for this batch):

```sh
cargo test --test native_string_settlement_v1
cargo test --test semantic_target_evidence_v1 string_cleanup_evidence_binds_current_production_c_and_rejects_foreign_binding
```

The ordinary physical cases require `CLANG` or `clang`; absence is a failure,
not a skip. To select the ignored sanitizer case, first set
`SEMAPRAX_STRING_SANITIZER_CLANG` to an absolute existing Clang executable with
ASan/UBSan runtimes provisioned, then run:

```sh
cargo test --test native_string_settlement_v1 provisioned_ordinary_native_string_asan_ubsan -- --ignored --exact
```

This cleanup correction does not itself determine String representation. The
subsequent [native String contents correction](NATIVE-STRING-CONTENTS-V1.md)
selects the existing length-header runtime for ordinary/stdout generation and
adds authored, unrun embedded-NUL value evidence. Ordinary Wasm's String host API
still lacks physical drop settlement, and the ordinary reference interpreter still
rejects user functions with String-valued signatures. Native allocation
evidence therefore is not full cross-backend String settlement evidence.
The distinct [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md)
is an authored, unrun opt-in conformance route, not an implicit change to that
ordinary profile or a target-allocation proof.
Frozen provider/command projections retain their separate unselected-String
cleanup limitation. These gaps and executed platform/sanitizer evidence remain
necessary before broad production-readiness claims.
