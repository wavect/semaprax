# Owned npm invocation failure state v1

Status: authored correction in progress; executable evidence remains unrun.

Audience: compiler/runtime contributors, SDK integrators and reviewers.

## Existing contract, corrected implementation

This is a shared private implementation contract for the existing Project
v8/v9/v10 generated JavaScript runtimes, not a new Project, descriptor, carrier,
Wasm ABI or public API profile. It implements the publication and fail-stop
requirements in [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md),
[Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md), and
[Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md).

The previous facades incorrectly treated every `TypeError` or `RangeError`
after execution began as recoverable and trusted an exception's public
`semapraxSemantic` property. A fatal UTF-8 conversion after owner consumption
could therefore leave an empty arena and permit reuse despite runtime
uncertainty. A caught reentry error did not latch poison. View creation outside
the guarded body and unguarded v9 scratch cleanup also left gaps in failure
selection and busy-state release. Truthiness-based error publication could
swallow a thrown `null`, `undefined`, `false`, zero or empty string.

## State and admission

The generated instance has one private, synchronous invocation state shared
by its facade and owned arena. It exposes no raw instance, imports, handles,
memory, callback, reset, recovery or poison-clear operation.

| State | Permitted next action |
| --- | --- |
| Idle, previously settled | Reserve the busy guard for one invocation |
| Preflight, busy but not entered | Validate the requested export and complete input tuple |
| Admitted and active | Execute once, authenticate result, copy and settle, then publish |
| Active with checked semantic failure | Verify untouched output, settle and report that same failure |
| Poisoned | Reject every later invocation/import/consume/publication |

Input type/arity/identity/UTF-8/whole-tuple capacity validation and snapshots
remain before scratch writes and arena entry. Rejection before the invocation
transition owns no target cleanup obligation and leaves an idle instance
reusable. Existing captured-intrinsic input checks and limits remain unchanged.

A busy check and guarded busy reservation precede preflight work; the separate
entered flag is set only after input validation/snapshots and before target
scratch/view operations. Any nested call while busy poisons
both the invocation state and arena before throwing, even if the exception is
caught below the facade. Poison is absorbing; it is checked before later host
imports, result consumption and outward publication. This does not add safe
callbacks or reentrancy. The runtime assumes trusted realm intrinsics; it is
not a sandbox against hostile co-resident JavaScript.

Every operation after setting busy, including typed-view construction, is
inside the guarded execution/settlement scope. Busy is released through a
finally path even if execution, settlement or scratch cleanup throws. Releasing
busy never clears poison and cannot authorize a later invocation.

## Failure identity and publication

Raw status must be an integer in `0..=10`. Zero is success. A status in
`1..=10` is an ordinary checked-language failure only after the complete output
slot is proven unchanged. The facade constructs and retains one private error
object for that observed status; only that exact object can take the
recoverable path. Its construction must finish before it becomes the selected
recoverable error. Status classification never relies on exception class,
truthiness, arbitrary properties or a duck-typed `semapraxSemantic` marker.
The existing outward error fields may remain for consumers, but grant no
authority to an incoming exception.

Every other post-entry exception poisons the instance, including
`TypeError`, `RangeError`, unexpected allocation/decoder errors, malformed
status/carrier/output, throwing finalizers and raw Wasm traps. This holds even
when no owner was minted or when every owner has already been consumed. An
empty arena alone is not proof that interrupted execution is reusable.

The first selected thrown value is preserved exactly, including falsy values.
Selection uses an explicit presence flag or unique internal sentinel. An
initial `null` cannot accidentally authenticate a thrown `null` as a semantic
failure. Later settlement/scratch failures latch poison but do not replace an
earlier selected error. With no earlier failure, the first cleanup failure is
selected. Neither an error nor a successful host value is silently discarded.

Success and recoverable semantic failure require successful complete arena
settlement, scratch cleanup and an unpoisoned state. Successful language
`Result::Err` and `Option::None` remain values, not execution failures. Scalar
and record result authentication precedes outward publication. Owned results
are copied into fresh host storage and consumed exactly once; v10 decodes
only owned UTF-8 after consumption. Invalid UTF-8 poisons even after that
consumption succeeds. Decoding preserves all Unicode scalar content, including
an initial U+FEFF (BOM) as data; it does not silently remove a source character.
Raw `Bytes` are never decoded as strings.

No bulk arena clearing may conceal a missing compiler drop. Poisoned state
does not promise physical cleanup, rollback or retry. Private references may
remain retained by an unusable instance; no garbage-collection or heap-bound
claim follows from poisoning.

## Imported UTF-8 validation

The existing `spx_owned_utf8_validate_v1` signature and valid/invalid UTF-8
meaning remain unchanged. It returns zero only for authenticated bytes that
are not Unicode-scalar UTF-8. A bad carrier, bad memory extent, poison, or an
unexpected host exception is not malformed user text and must not be hidden
by a blanket decoder catch. A bounded byte validator can distinguish those
cases without allocating or invoking a text decoder.

## Compatibility and file ownership

Only v8/v9/v10 `semaprax.js` bytes and dependent artifact/carrier byte lengths
and integrity bindings intentionally change. Wasm bytes, canonical semantic
descriptors, API metadata whose facts are unchanged, public parameter/result
mapping, TypeScript declarations, binding re-exports, package metadata, Rust
SDKs and native providers do not change. Existing Project v1-v7 and historical
unselected renderer fragments remain byte-identical. The fixed v8/v9
16-owner policy and v10 selected-owner capacity are unchanged.

The shared runtime state belongs in small private generator-owned JavaScript
helpers, with explicit v8/v9/v10 selection in the existing owned-data and flat
record renderers. Do not widen an earlier profile or edit generated package
artifacts by hand. Existing bounded-renderer known answers intentionally need
reviewed replacement values for this bug fix; historical false/unselected
fragment pins remain exact. Static literal-template expansion and hashing is
not target execution and must not be described as behavioral evidence.

## Required authored evidence

Real generated six-artifact packages cover v8 direct Bytes, variants and mixed
scalars; v9 flat records; and v10 direct, variant and mixed UTF-8 results.
Test-only engine/import wrappers inject failures without runtime source
rewriting or production fault hooks. The matrix includes:

- pre-entry rejection followed by successful reuse;
- actual checked-status cleanup followed by successful reuse;
- post-entry `TypeError`/`RangeError`, before mint and after initialized owners;
- actual consume followed by invalid UTF-8 decoding failure;
- forged semantic markers and negative, fractional, NaN or unknown statuses;
- every representative falsy thrown value;
- caught reentry followed by attempted later import/consume/publication;
- a cleanup failure after a primary failure, preserving exact error identity;
- one engine entry only, with later invocations rejected after poison; and
- exact unchanged non-runtime artifacts and historical renderer known answers.

These fixtures, compiler checks and hosted release gates are unrun. No tests,
builds, target probes or hosted workflows are authorized in this batch. No
package publication, support promotion or overall production-readiness claim
is made.
