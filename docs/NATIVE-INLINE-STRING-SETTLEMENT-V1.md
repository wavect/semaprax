# Native Inline String Settlement v1

Status: implemented bounded profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md). Historical local,
authoring-time, ignored, or separately provisioned observations below retain
their narrower scope; public promotion and broader product completion remain
separately gated.

Audience: compiler contributors and native-runtime reviewers.

## Scope

This correction applies to ordinary C11 generation (`emit_c` and `emit_hir_c`)
and bounded stdout transcripts. It applies the existing inline String cleanup
rule to admitted source. It adds no syntax, type, intrinsic, public ABI, schema,
import, or ambient authority.

The original correction used the private owner-cell machinery introduced for
the [v10 owned UTF-8 provider](PUBLIC-OWNED-UTF8-API-V1.md). The additive
[owned String variant profile](OWNED-STRING-VARIANTS-V1.md) migrates ordinary
String ownership into canonical CleanupPlan slots under `core.string.drop`,
including independent replay. Plan-owned Strings use those exact transitions
and exits; the legacy owner ledger is disabled for those functions.

## Exact ownership

Each emitted function is checked for String parameters, result, body
expressions, preconditions, and postconditions. Only a String-bearing ordinary
or stdout-transcript function activates the ledger and bounded staged output.
All owner flags and temporary pointer cells are initialized at function entry,
before any recoverable failure branch; block-local addresses are not retained.

- Literal, clone, and successful call results establish one live owner.
- Owning place reads retain the existing clone behavior, with the additive
  [named length condition](STRING-LENGTH-CONDITIONS-V1.md) inspection exception;
  borrowed reads alias the existing carrier. A temporary handoff moves
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

The subsequent [owned-data internal String correction](NATIVE-OWNED-DATA-STRING-SETTLEMENT-V1.md)
explicitly extends this ledger and instance discovery to the v8/v9 provider,
including emitted but unselected String functions; its String-bearing output
intentionally changes. String-free output/budgets and the existing v10 provider
selection remain unchanged. The three versioned command profiles
(Useful Data Command, Language Command I/O, and Line Command I/O) remain
unchanged, including emitted but unselected functions. Their selected closures
do not admit owned Strings. The scalar Native Rust SDK uses its separate
admitted-closure renderer and is not redirected through this correction.

## Authored evidence and remaining gaps

The String branch-result regression corpus also covers a statementful function
body ending in a String `if`, an `if` inside a variant-match arm, and nested
`if` values inside scalar-match arms. Native emission moves the selected `if`
result into its enclosing variant-match join through the exact canonical
handoff; it does not replay the already completed branch transfers. The
`tests/language/string_branch_results.rs` gate checks both selected paths,
interpreter results, source/graph round trips, CLI execution, and balanced
native allocations at O0/O2. Its separate Wasm cases retain their admitted
scalar boundary and stable variant-profile refusal.

Native block completion also settles String operands nested in scalar
statements and tails, including empty `if` branch blocks and nested blocks.
The selector uses only slots of that block's canonical lexical region and
emits its normal exit's `finalize_in_order` after any owned tail handoff.
Nested blocks and match arms settle their own regions; no additional `if`
finalizer duplicates their exits. This corrects repeated loop branches such
as `if flag { string_find(text, "a", 0) } else { 0 }`, whose operand owners
previously remained live on the next iteration.

`tests/language/native_if_string_temporaries.rs` covers both selected outcomes,
nested `if` and statementful blocks, owned String results, false guards and
checked guard failure. Native O0/O2 probes repeat calls with zero live
allocations and poisoned failure result storage. Len-based controls also run
on the admitted String-settling Wasm profile; Find retains its existing Wasm
refusal. These are focused authored regressions, pending current-head execution.
Loop admission is owned by [Owned String Loops v2](OWNED-STRING-LOOPS-V2.md).

`tests/native/string_settlement.rs` generates ordinary production C and
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

Custom C fixture entry points bypass the generated entry wrapper's Windows
stdout setup. The shared test-only `tests/support/native_fixture_stdio.c`
therefore selects checked binary stdout before any fixture output or semantic
call. It is included before allocator instrumentation, so CRT headers and
transport setup are not instrumented provider code. Ordinary, generic,
stdout-transcript, contents, v8/v9/v10 provider, and internal interpreter/Wasm
native parity fixtures retain their exact LF byte assertions. No captured
output is normalized, no allocation oracle changes, and the helper is not part
of a generated artifact or production runtime. This portability correction is
implemented on all hosts.

Focused emitter units cover String presence, generic-instance helper
discovery, bounded output, String-free emission, and frozen profile selection.
Existing String-operation diagnostics and value-conformance fixtures remain
required; physical native evidence does not replace them.

Focused execution commands for the specified provisioned environment:

```sh
cargo test --test native string_settlement::
cargo test --test semantic target_evidence::string_cleanup_evidence_binds_current_production_c_and_rejects_foreign_binding
```

The ordinary physical cases require `CLANG` or `clang`; absence is a failure,
not a skip. To select the ignored sanitizer case, first set
`SEMAPRAX_STRING_SANITIZER_CLANG` to an absolute existing Clang executable with
ASan/UBSan runtimes provisioned, then run:

```sh
cargo test --test native string_settlement::provisioned_ordinary_native_string_asan_ubsan -- --ignored --exact
```

This cleanup correction does not itself determine String representation. The
subsequent [native String contents correction](NATIVE-STRING-CONTENTS-V1.md)
selects the existing length-header runtime for ordinary/stdout generation and
adds release-tested embedded-NUL value evidence. Ordinary Wasm's String host API
still lacks physical drop settlement, and the ordinary reference interpreter still
rejects user functions with String-valued signatures. Native allocation
evidence therefore is not full cross-backend String settlement evidence.
The distinct [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md)
is an implemented opt-in conformance route, not an implicit change to that
ordinary profile or a target-allocation proof.
Frozen command/callable projections retain their separate unselected-String
limitations. The owned-data provider correction is separately release-tested. These gaps and executed platform/sanitizer evidence remain
necessary before broad production-readiness claims.
