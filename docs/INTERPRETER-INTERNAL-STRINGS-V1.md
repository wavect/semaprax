# Internal String Interpreter v1

Status: additive implementation with bounded, partial local validation;
the [local validation record](WASM-INTERNAL-STRINGS-V1.md#local-validation-record)
owns the executed scope. Cross-platform, full-profile, and hosted gates remain;
no production or support promotion.

Audience: language users, compiler contributors, and conformance reviewers.

## Explicit entry point

```sh
semaprax interpret-strings <file> --function <name|stable-id> [--arg literal]... [--max-bytes N]
```

The additive library module is `semaprax::interpreter::internal_strings`, with
`interpret`, `verify_envelope`, and `verify_envelope_against_source` functions
using the same signatures as their [ordinary interpreter](INTERPRETER-V1.md)
counterparts. `InterpreterOptions` and `Interpretation` are reused unchanged.
The command shares the existing option parser and exit-code convention:
returned value is 0, language/capacity failure is 1, and usage failure is 2.
The API returns no terminal LF; the CLI appends one.

The existing `interpret` command/API, source-report verifier, Project entry and
test execution, prepared Project interpreter, source traces, stdout, command,
and owned-data evaluators do not opt in. Their admission, wire bytes, and
diagnostics remain unchanged. In particular, ordinary interpretation still
rejects an internal String-signature callee with `SPX-F102`.

## Admission and evaluation

The selected external function retains the existing explicit-ID,
monomorphic, effect-free scalar-result boundary. Arguments remain the existing
direct scalar or invocation-borrowed text/byte literals. No owned String
argument or result is added to the command or report grammar.

Only the transitive internal function inventory is widened: alongside its
previously admitted signatures, it admits direct `string` parameters with
the ordinary by-value declaration mode and direct `string` results. Explicit
identities, effect exclusion, monomorphism, and every existing expression
admission check remain required. This is not generic, aggregate-String,
mutable-borrow, import, unsafe, or effectful-call support.

The ordinary verifier and validated HIR remain the source of meaning. The
existing evaluator already represents String values as Rust-owned UTF-8;
there is no second String execution engine. Place reads clone, arguments
evaluate left to right into an invocation-owned vector, and the complete
vector enters the callee frame. Rust ownership releases frames and temporaries
on normal return, late-argument failure, failed pre/postconditions, fuel
exhaustion, and depth refusal. Failed postconditions do not publish a
provisional result. No CleanupPlan transition, target runtime, or allocator
transfer is added by this admission policy.

String lengths include embedded NUL, and scalar counts count U+0000 as one
scalar. Arithmetic and contract failures retain the compiler's normalized
status and first-failure selection. Existing step charges, recursion ceiling,
and fixed-stack evaluation thread are unchanged.

## Distinct report identity

Both the outer object and its payload use exactly:

```text
semaprax.interpret.internal-strings.v1
```

Payload digest domain:

```text
semaprax.interpret.internal-strings.payload.v1\0
```

The digest framing remains `SHA-256(domain || little_endian_u64(length) ||
exact_payload_bytes)` with `sha256:` and 64 lowercase hex digits. Source
digests retain the existing `semaprax.interpret.source.v1\0` domain. This is
a directly rendered report for the selected profile, not a legacy execution
envelope wrapped or relabeled after evaluation.

Outer member order is `schema, digest, bytes, payload`. Payload member order
is `schema, source, function, arguments, limits, fuel, outcome, nonclaims`.
The nested member order, external value grammar, normalized-status grammar,
outcome kinds, and six ordered nonclaims are inherited from Interpreter v1.
The distinct schema and digest domain select the new profile; there is no
implicit fallback between the two verifiers.

## Bounds and replay

The new route validates options before source evaluation. Source acquisition
and final drift checks are bounded to 16 MiB through the existing authenticated
source-snapshot helpers. Source-bound verification also bounds its source read.
Envelope input is capped at 16 MiB before JSON parsing, and must additionally
fit its embedded `limits.max_bytes`. The existing option range is 1 KiB through
16 MiB, default 64 KiB. The library step-budget range remains 1 through
100,000,000, default 1,000,000; the call-depth ceiling remains 256.

Generation has a separately bounded serialization-work allowance and checks
the exact final envelope length against `max_bytes`; exact capacity succeeds
and one byte less fails without truncation. The legacy renderer's accounting
is unchanged.

The new verifier requires canonical encoding, exact member sets and order,
and exact schema/domain bindings. Duplicate members, alternative escaping,
whitespace variants, unknown keys, re-signed malformed values/statuses, and
contradictory fuel/outcome facts reject. Fuel exhaustion is true exactly when
the outcome kind is `fuel_exhausted`. Source binding detects drift, but does
not independently re-execute the program, authenticate provenance, or prove
that a submitted outcome was obtained by execution.

The existing diagnostic families are reused: `SPX-F101` options, `SPX-F102`
admission, `SPX-F103` arguments, `SPX-F104` generation bounds, `SPX-F105`
evaluation guards, and `SPX-F106` replay/source-binding rejection. Shared
source I/O and final-drift diagnostics remain available on generation.

## Evidence and nonclaims

Authored evidence covers String forwarding and return, clones, empty/NUL and
multibyte values, nested/mixed arguments, contracts, sticky failure, late
argument failure, depth/fuel limits, subsequent calls, and scalar legacy
fuel/output preservation. Native O0/O2 and ordinary Core-Wasm comparisons use
the actual source and raw target emitters, not the frozen public scalar-export
package profile, which still excludes String closures.

The report tests cover deterministic generation, source drift, exact output
capacity, input bounds, canonical and re-signed hostile envelopes, profile
cross-pair rejection, unchanged external String rejection, frozen
effect/import/generic/unsafe rejection, and CLI behavior.
Executed selections and their limits are recorded in the
[local validation record](WASM-INTERNAL-STRINGS-V1.md#local-validation-record).
This is partial local evidence, not completion of the cross-platform,
full-profile, or hosted gates, and does not imply interpreter sanitizer evidence.

The focused gates are:

```sh
cargo test --lib interpreter::internal_strings::tests
cargo test --test interpreter_internal_strings_v1
cargo test --test interpreter_v1
cargo test --test native_string_settlement_v1
```

The new integration fixture is split between
`tests/interpreter_internal_strings_v1.rs` and its `source.spx`, `protocol.rs`,
`support.rs`, and `probe.mjs` children. Its parity gate requires installed
Clang and Node; neither is downloaded or silently skipped. Existing legacy
and native String gates remain necessary preservation evidence. These
commands document required checks; the linked record, not this list, identifies
which selections have executed.

Fuel bounds evaluated work, not String byte growth or peak heap allocation.
Source/output caps are not an execution-memory sandbox. Allocator failure,
foreign unwind, cancellation, target finalizers, and signal recovery do not
gain new guarantees. Wasm String value parity is not physical Wasm settlement.
No Project/Transport widening, debugger, persistent cache, JIT, native code
execution, filesystem mutation, network, or publication authority is added.
