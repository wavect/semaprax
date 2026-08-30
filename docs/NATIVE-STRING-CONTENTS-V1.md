# Native String Contents v1

Status: corrective implementation and regression evidence authored but unrun;
no production, package, or cross-platform promotion.

Audience: compiler contributors and native-runtime reviewers.

## Contract and defect

Owned `string` values contain exact UTF-8 bytes. U+0000 is data, not an end
marker. Byte length, scalar count, equality, cloning, concatenation, prefix
and substring operations must observe the whole value, including content
after a NUL byte. Empty text and a single NUL scalar are distinct values.

The source lexer, canonical formatter, HIR literals, and native literal
escaping already preserve those bytes. The defective ordinary native runtime
copied literal bytes but later used C terminator searches to determine value
length. Changing escaping or rejecting NUL would not correct that defect.

## Representation selection

Ordinary C11 (`emit_c` and `emit_hir_c`) and stdout-transcript generation reuse
the existing length-header String runtime introduced by the v10 provider.
Construction, clone, equality, both intrinsic helper groups, and drop all use
the same representation within a generated translation unit. The trailing
terminator remains a convenience; it never determines semantic length.

The header and helper names retain their historical `v10` spelling to reuse
the exact runtime bytes. This does not select Project v10, widen its public
closure, or confer provider-handle authority. Ordinary String selection is
separate from the v10 provider's borrowed-status and byte-carrier runtime
selection. This correction does not add either carrier runtime to String-free
ordinary output; existing source-driven carrier selection remains unchanged.

The existing [inline owner ledger](NATIVE-INLINE-STRING-SETTLEMENT-V1.md)
continues to govern allocation ownership, argument commit, scope exit, failure
settlement, and result publication. No cleanup transition or resource
CleanupPlan interpretation changes. Nonempty concatenation in the reused
runtime uses one temporary allocation in addition to the result allocation;
physical fixtures count and settle both rather than hiding this cost.

## Compatibility and authority

String-bearing ordinary and stdout-transcript C bytes intentionally change.
Dependent Target Evidence and Patch Evidence v2 bind those current production
bytes and therefore change their corresponding digests and lengths. They do
not use an older emitter to retain stale artifact bindings.

The existing v10 provider runtime constants and outputs remain unchanged.
Earlier owned-data providers, all three versioned command profiles, and the
private callable prelude remain on their existing paths. String-free native
output and its budget accounting remain unchanged. Source, Graph, HIR,
CleanupPlan, diagnostic, manifest, descriptor, and evidence schemas do not
change. Wasm output, host imports, and interpreter admission do not change.

Project v1-v4 native routes use ordinary emission, but their Phase-A profiles
reject owned Strings throughout the retained compiled function inventory.
Project v5-v7 use the unchanged command profiles. This correction therefore
does not change admitted Project v1-v7 native artifacts.

Generated internal String `char *` values now point into a header allocation.
They must originate from that generated runtime and be released through its
`spx_string_drop`. Foreign C literals, `malloc`/`strdup` pointers, or values
from a different runtime/profile are not valid String inputs. Raw `free` of a
String result is invalid. These internal signatures are not a supported
public String ABI; the existing public C/C++ projections exclude them.

No new filesystem, process, network, callback, or publication authority is
introduced. Allocation failure and runtime invariant failure remain fail-stop;
no signal, unwind, or `longjmp` recovery guarantee is added.

## Evidence and remaining gates

Authored regressions cover NUL positions, unequal suffixes after NUL, exact
byte and scalar lengths, Unicode beside NUL, cloning, consuming calls and
results, concatenation, prefix/substring matches, and `string_from_char` for
U+0000. Native physical fixtures check explicit lengths and `memcmp`, exact
allocation/free accounting, failure-slot poison, and reuse after failure.
Terminator-based C assertions cannot establish the contents contract.

The value corpus compares interpreter, native O0/O2, and Core-Wasm/Node only
where those existing profiles admit it. Owned user String signatures remain
outside the interpreter profile. Node value equality is not physical Wasm
String settlement; its ordinary host API still lacks a drop operation.
The separate [Internal String Interpreter v1](INTERPRETER-INTERNAL-STRINGS-V1.md)
adds an authored, unrun opt-in route for String helpers, without changing this
corpus's ordinary-interpreter rejection or adding external String values.
Frozen earlier-profile emitted-but-unselected String functions retain their
separate representation and cleanup limitations.

The focused gates (not run for this batch) are:

```sh
cargo test --locked -p semaprax --test native_string_settlement_v1
cargo test --locked -p semaprax --test string_ops_v1 --test string_ops_v2
cargo test --locked -p semaprax --test semantic_target_evidence_v1 string_cleanup_evidence_binds_current_production_c_and_rejects_foreign_binding
```

Physical native cases require `CLANG` or `clang`; the cross-backend contents
case additionally requires Node. These new cases fail when a required tool is
absent rather than skipping conformance. For the explicitly ignored sanitizer
gate, set `SEMAPRAX_STRING_SANITIZER_CLANG` to an absolute existing Clang
executable with ASan/UBSan runtimes provisioned, then run:

```sh
cargo test --locked -p semaprax --test native_string_settlement_v1 provisioned_ordinary_native_string_asan_ubsan -- --ignored --exact
cargo test --locked -p semaprax --test native_string_settlement_v1 contents::provisioned_embedded_nul_native_values_asan_ubsan -- --ignored --exact
```

All new evidence remains unrun. Required target and sanitizer execution,
ordinary Wasm settlement, full interpreter admission, and exact-head package
promotion remain open; this correction alone is not production readiness.
