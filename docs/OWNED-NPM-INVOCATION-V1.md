# Owned npm invocation failure state v1

Status: correction with scoped local real-package evidence; no hosted or
production-readiness promotion.

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

Unknown export identity consistently reports `RangeError`; v9 previously used
`TypeError` for that rejection. Argument count/type rejection remains
`TypeError`. These preflight errors remain reusable and do not enter Wasm.

Export selection admits primitive strings only. A non-string identity is
rejected with `RangeError("SEMAPRAX export identity must be a string")` before
lookup or diagnostic formatting, without reading properties or invoking
conversion hooks. Previously, interpolation of an unknown object could run
caller code while busy and poison an otherwise reusable instance; a Symbol
could instead throw `TypeError` during formatting. Unknown primitive strings
retain their exact existing diagnostic. Poison and busy checks still precede
identity admission: a genuinely nested call remains fail-stop regardless of
the nested identity's type. This changes only selected runtime JavaScript and
its dependent integrity bindings, not the string-keyed public API.

The shared runtime state belongs in small private generator-owned JavaScript
helpers, with explicit v8/v9/v10 selection in the existing owned-data and flat
record renderers. Do not widen an earlier profile or edit generated package
artifacts by hand. Existing bounded-renderer known answers intentionally need
reviewed replacement values for this bug fix; historical false/unselected
fragment pins remain exact. Static literal-template expansion and hashing is
not target execution and must not be described as behavioral evidence.

## Required authored evidence

The authored entry point is
[`tests/project_owned_failure_fsm_v1.rs`](../tests/project_owned_failure_fsm_v1.rs).
Its companion `baseline.rs` freezes complete TypeScript, package JSON and API
metadata formats independently of the current renderers. Supplied descriptor
bytes are authenticated fixture inputs, not a historical descriptor known
answer; comparison with direct current Wasm emission is consistency evidence,
not historical byte-preservation proof. Unchanged compiler/descriptor paths and
their existing known-answer gates remain separately required.

Real generated six-artifact packages cover v8 direct Bytes, variants and mixed
scalars; v9 flat records; and v10 direct, variant and mixed UTF-8 results.
Test-only engine/import wrappers inject failures without runtime source
rewriting or production fault hooks. The matrix includes:

- pre-entry rejection followed by successful reuse;
- non-string and hostile export identities without property reads, coercion,
  scratch mutation or engine entry, followed by same-instance reuse;
- actual checked-status cleanup followed by successful reuse;
- post-entry `TypeError`/`RangeError`, before mint and after initialized owners;
- actual consume followed by invalid UTF-8 decoding failure;
- exact leading-BOM output and imported scalar UTF-8 boundaries, distinguishing
  malformed text from carrier, memory and unexpected host failures;
- forged semantic markers and negative, fractional, NaN or unknown statuses;
- every representative falsy thrown value;
- caught reentry followed by attempted later import/consume/publication;
- a cleanup failure after a primary failure, preserving exact error identity;
- one engine entry only, with later invocations rejected after poison; and
- exact unchanged non-runtime artifacts and historical renderer known answers.

The existing v10 lifetime probe separately requires exact healthy arena
capacity, settlement and nonreused tokens; exhaustion and unsettled entry use
separate instances and must reject every later import, consume or settlement.
No successful cleanup is inferred from their poisoned state.

### Authentic malformed-result observations

The companion `tests/project_owned_failure_fsm_v1/result.mjs` extends those
same seven generated packages. It runs the real selected Wasm function and
mutates its returned result storage at the test engine boundary, then lets the
unchanged generated facade decode it. It does not replace the facade, copy its
decoder, edit runtime source, or use a second arena implementation. Existing
lower-level carrier fixtures remain separate evidence, not proof that the
public wrapper reaches their checks.

Healthy nonempty and empty owned results calibrate engine-entry, real mint,
payload-read and successful owner-consumption observations. Negative cases
cover untagged/zero-token, unissued and same-arena previously consumed tokens,
wrong lengths, over-capacity lengths, and a carrier dropped through the real
import before return. A second real minted owner tests that valid copy-out
alone cannot establish complete arena settlement. Each case requires its
specific production diagnostic, one observed fault injection, no outward
value, and rejection of a later call before engine reentry.

Variant cases send invalid Option and Result tags through the actual decoder
and require zero payload reads or owner consumption. A genuine inactive None
with garbage in unused payload storage must instead remain successful and
reusable, without reading or consuming that storage. Changing a real active
Ok to an inactive Err while retaining its owner must fail arena settlement.
Scalar and record bool corruption must reject before publication; a real
checked semantic failure with modified output must not take the recoverable
status path. Scoped, pass-through DataView/Map observers are restored after
each observation and never manufacture a decoder failure.

These are JavaScript boundary regressions. Test-only engine
substitution and trusted-realm observation do not prove an adversarial-JavaScript
sandbox, compiler generation of malformed results, native behavior, or
physical deallocation of owners retained by a poisoned instance. Production
artifacts, package subjects, descriptors and existing known answers are unchanged.

### Authentic owned-result finalization failures

The companion `tests/project_owned_failure_fsm_v1/finalization.mjs` observes
the real owned `Bytes` result of `case.copy` in all seven packages. After a
successful real Wasm return, it binds observations to the exact returned token,
arena Map and owned byte array. Scoped intrinsic hooks inject failures at
copy allocation, before and after actual owner deletion, input scratch clearing,
result-slot poisoning, and variant/record wrapper freezing. Healthy pass-through
calibration requires copy, consumption, empty-arena settlement and reusable
success before fault cases are accepted.

Each fault preserves the exact thrown Error or falsy value, publishes no value,
and rejects a later call without another engine entry or owned import. A real
owned-result copy failure followed by a distinct scratch-cleanup failure must
retain the primary error. Successful deletion followed by a throw is not counted
as successful settlement. Observer assertions escape the capture oracle, and
all patched intrinsics are restored before reuse checks. These trusted-realm
faults do not demonstrate physical out-of-memory behavior, deallocation or GC,
or a hostile-JavaScript sandbox. UTF-8 decoder evidence remains a separate case.

### Successful inactive results after initialized ownership

`tests/project_owned_inactive_cleanup_v1.rs` adds a separate authenticated v8
Project whose two exports copy their borrowed input before selecting `Some`
versus `None`, or `Ok` versus `Err`. It materializes the six verified inline
artifacts for Node consumption; it is not a production filesystem-publication
or installed-package test. Canonical source/Graph round-trips and descriptor
bindings are checked, and fixture inputs and artifacts remain unchanged.

The consumer executes the actual Wasm and passes through the actual arena
imports. It binds observations to the minted token, arena Map and byte array.
Every call must mint once, including empty bytes. An inactive success must
drop that same owner during compiler execution, delete it exactly once and
perform no result consumption. Active controls must instead consume once
after engine return, without a compiler drop. Both paths require the bound
arena to be empty before publication, cleared scratch and healthy reuse.
`None` must not read inactive payload storage; `Err` reads its scalar error,
not an owned payload.

The fixed corpus covers empty, binary/invalid-UTF8, 65,535 and 65,536-byte
inputs, repeated active/inactive/recovery sequences, and retained independent
host copies. All 96 invocations use one real instance. Scoped intrinsic
observers are restored in `finally`; no carrier, generated source or arena is
replaced. These observations establish logical ownership settlement, not
physical deallocation, native behavior, browser coverage or a hostile-realm
sandbox. Temporary fixture trees are retained, and the selected gate needs an
external process deadline/resource bound.

The subsequent [native and safe-Rust companion](PUBLIC-OWNED-DATA-API-V1.md#initialized-owner-inactive-results)
shares the exact Project source without changing this JavaScript observer. The
combined root gate now also requires Clang and checks physical native cleanup
at O0/O2; a separately selected private gate publishes and consumes the real Rust
SDK. Those observations, not the npm Map counters, own native allocation and
safe-Rust publication evidence. Browser coverage remains separate.

### Scoped local execution

On macOS AArch64 with Rust 1.98 and Node 24.3, both tests in
`project_owned_failure_fsm_v1` pass across the seven generated packages,
including the malformed-result and finalization companions above. Production
runtime templates and existing artifact known answers are unchanged by the
finalization-test batch. This is local evidence for this selected gate, not a
full quality-profile, hosted release, package publication, support promotion
or overall production-readiness claim.

The later descriptor/inactive-cleanup test-only batch passes the complete
`public_api_descriptor_v1` suite (16 tests), `project_owned_bytes_npm_v1`
(7), `project_owned_failure_fsm_v1` (2), `project_owned_tuple_npm_v1` (1),
and `project_owned_inactive_cleanup_v1` (1) together on macOS AArch64/Rust
1.98 and offline Linux AArch64/Rust 1.88, both using Node 24.3. The new fixture
observes 64 active consumes and 32 inactive compiler drops. Two documentation
checks pass on both hosts; strict compiler-library and changed-integration
Clippy passes on macOS. Linux uses the provisioned resource-bounded,
network-disabled container. Production files and artifact known answers are
unchanged. Windows, the pinned three-engine browser gate, native allocation
evidence, full-profile verification and exact-head hosted promotion remain
separate; these local results do not certify the concurrently changing main.
