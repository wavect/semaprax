# Standalone Wasm Internal String Settlement v1

Audience: compiler contributors and runtime implementers.

Status: authored implementation; all new executable evidence is unrun.
This opt-in compiler/runtime profile does not promote a completion-matrix row.

## Purpose and unchanged boundaries

This profile supplies compiler-directed String ownership settlement for a
selected, bounded standalone Wasm call closure. Ordinary Wasm's value-only
String imports do not establish physical settlement. A host that clears its
entire table on return would also leave loop and nested-call lifetimes unproven.
The new profile therefore lowers ownership at expression, scope and call exits,
including language failures and recognized runtime-capacity refusals.

It is additive: ordinary Wasm, Target Evidence, Project manifests v1-v10,
existing npm/Rust packages, descriptors, imports and emitted bytes remain on
their existing paths. No CLI, daemon, Project or package route selects this
profile implicitly. Its private String cells do not change resource liveness,
CleanupPlan schemas, or the resource ABI's prohibition on payload sentinels.
Zero is reserved solely as an empty private String cell, never a live String
handle (not even an empty String).

The explicit source-only `build --target web --profile internal-strings-v1`
package entry is specified separately in [Standalone internal String Web
package v1](WASM-INTERNAL-STRINGS-WEB-V1.md). It reuses these exact compiler and
runtime outputs; it does not silently select this profile for legacy builds.

Affected long-term matrix requirements are cross-backend semantic equivalence,
unique ownership/move safety, Wasm core/components and JavaScript/TypeScript.
All remain Partial, including the separate ordinary-Wasm settlement gap.

## Compiler entry and admission

`wasm::internal_strings::emit_module(&Program, &[String], InternalStringOptions)`
returns an immutable `InternalStringModule` or an existing stable compiler
diagnostic. Accessors provide module bytes, canonical descriptor JSON and the
bound JavaScript runtime source. This is compilation only, not a proof or
replay-authority API. HIR validation precedes profile admission and lowering;
malformed HIR remains `SPX-H006`, unsupported lowering remains `SPX-W111`.

Selection names 1..=32 distinct explicit stable function identities. The
descriptor and raw-wrapper ordinals use canonical stable-ID order independent
of request order. Public functions have at most eight value parameters, each
`i64` or `bool`, and an `i64` or `bool` result. The selected transitive closure
contains at most 256 monomorphic, acyclic functions. Internal signatures also
admit value `char` and owned-by-value String. The latter has canonical HIR
ownership `Own`, not `Value`; profile admission rejects forged ownership modes
without widening the public scalar boundary. Ordinary verified literals,
cloning reads, all seven String intrinsics, String equality/inequality,
branches, lazy Boolean flow, scalar matching/guards, mutable scalar bindings,
loops, internal calls, requires and ensures are covered. Loop bodies and
conditions retain the existing Copy-only source/HIR admission (`SPX-T252`
for direct String storage or non-scalar call signatures). Repeated String
settlement is exercised by scalar-signature helpers that allocate and release
Strings internally on each iteration; this does not add owned loop storage
or CleanupPlan back-edges.
String assignment remains the existing source-level `SPX-U105` rejection:
[Explicit Mutation v1](EXPLICIT-MUTATION-V1.md) admits only Copy scalar targets.
This profile does not widen source mutation or weaken verifier admission.

Resources, Bytes, nominal aggregates/variants, generics, foreign imports,
effects and unsafe blocks are outside this profile, not silently approximated.
Selected cycles and excessive derived stack/owner requirements are rejected
before module emission. This is not general ordinary-language admission.
These exclusions apply to the selected closure: unrelated valid declarations
do not alter its Wasm, descriptor or runtime bytes. The complete original HIR
is still validated first; invalid unselected source is not ignored. Only the
proved nominal-free selected profile skips whole-program layout discovery and
unrelated command/range scratch planning.

## Exact artifact and memory contract

Descriptor identity is `semaprax.wasm-internal-strings.v1`; runtime identity is
`semaprax.wasm-internal-strings.runtime.v1`. The descriptor binds selected
export signatures, the exact module SHA-256/length, fixed memory geometry,
derived stack/owner requirements and effective quota policy. Canonical JSON
uses the repository's deterministic rendering conventions. A digest binds
bytes, not source provenance, trusted execution or successful settlement.

The module uses one private unshared memory with initial and maximum four
64-KiB pages, no start function, table or imported memory. Frames occupy
`[0,65536)`; the mutable shadow-stack global starts at 65536. A fixed scalar
result word starts at 65536. UTF-8 literal data occupies `[196608,262144)`.
The compiler derives complete selected call-path frame usage and rejects
anything above 65536 bytes. Engine stack exhaustion remains an unexpected
trap, not a recoverable language failure.

Generated bytes pass the existing Wasm structural validator before the
compiler returns any artifact. This is not execution or host-conformance
evidence. Before emission, selected bodies and contract expressions are limited to
65536 expression nodes and expression nesting depth 256. Static String-drop
work is the sum of owner cells visited by lexical scope sweeps plus twice the
function owner count for normal/failure epilogues, capped at 262144. All
arithmetic in these inventories is checked. These limits bound selected
lowering work, not source parsing/HIR validation or total heap usage. The
16-MiB final module limit is checked after emission and is an output-size cap,
not a preallocation or peak-memory guarantee.

Raw wrappers `__spx_call_<ordinal>` accept only the scalar arguments and
return an `i32` status; they never accept a caller-chosen result pointer.
Raw memory and the mutable stack-pointer global are confined to the generated
trusted runtime and independent raw-ABI tests, never its safe facade. The raw
ABI itself is not a read-only or safe public boundary.
Every invocation starts with a zeroed result word; failure leaves it zero,
and successful result publication is after all non-result cleanup. The
trusted runtime verifies stack restoration and emptiness before exposing any
result and clears the scalar result scratch before returning.

## Closed private import ABI

All imports use `semaprax.internal-strings.v1`, in this exact order:

| Name | Wasm signature | Ownership |
| --- | --- | --- |
| `literal` | `(i32 pointer, i32 length) -> i64` | Mints from the immutable literal segment |
| `clone` | `(i64 handle) -> i64` | Borrows input; mints independent owner |
| `concat` | `(i64 left, i64 right) -> i64` | Borrows both; mints independent owner |
| `from_char` | `(i32 scalar) -> i64` | Mints valid UTF-8 for one Unicode scalar |
| `byte_len` | `(i64 handle) -> i64` | Borrows |
| `char_len` | `(i64 handle) -> i64` | Borrows |
| `eq` | `(i64 left, i64 right) -> i32` | Borrows; canonical Boolean |
| `starts_with` | `(i64 value, i64 prefix) -> i32` | Borrows; canonical Boolean |
| `contains` | `(i64 value, i64 needle) -> i32` | Borrows; canonical Boolean |
| `drop` | `(i64 handle) -> ()` | Authenticates and removes exactly one owner |

All mint operations are NONCONSUMING at this private boundary. Staged operands
remain in compiler-owned cells until mint success and explicit generated
drops, or the common failure sweep. This preserves source concat's consuming
semantics without a second, host-side ownership-commit rule. Mint returns zero
only for an explicitly checked quota refusal and records its first closed
capacity cause. The compiler checks zero immediately and selects private
adapter status 11, branching to the common String cleanup path. It must not
invent a CleanupPlan status source for a source-infallible allocation.

Status 0 is success; arithmetic statuses 1..8 and requires/ensures statuses
9/10 retain their existing compiler-domain meanings. Capacity 11 is a profile
policy outcome, not an additional language arithmetic or contract failure.
All other statuses, invalid handles, invalid Unicode/carriers, unexpected
host errors, throwing drops or Wasm traps cause absorbing poison. Generic
JavaScript exceptions are never converted into a recoverable capacity result.

## Runtime bounds and publication

Generation-time `InternalStringOptions` supplies integer byte limits with
these defaults and hard maxima; JavaScript cannot override them:

| Limit | Default | Maximum |
| --- | ---: | ---: |
| Per-value UTF-8 bytes | 65536 | 65536 |
| Live UTF-8 bytes | 1048576 | 16777216 |
| Cumulative allocated UTF-8 bytes per invocation | 16777216 | 67108864 |

Byte limits may be zero (only zero-byte allocations fit). The conservative
owner bound is derived from actual selected function owner cells along the
maximum call path plus one transient mint, and must not exceed 65536. Optional
`max_live_owners` may lower the effective capacity to any value in
`1..=derived_capacity`; values above the derived bound are rejected, not
silently clamped. Empty Strings consume slots. Handles are monotonic nonzero
instance-local tokens, never reused; token-space exhaustion is another closed
capacity cause and is checked before allocation.

Before allocating a UTF-8 array, the runtime checks owner count, per-value
size, projected live bytes, cumulative bytes and token capacity in that order.
Projected live bytes include concat inputs AND the new output simultaneously.
Counters increase only for completed allocations; authenticated drop reduces
only live slots/bytes. Cumulative bytes never refund within an invocation.
No decode/encode round trip is needed for comparison, concatenation or search;
counting validates/scans scalar UTF-8 and substring search uses bounded space.
These are owned-buffer/slot bounds, not bounds on total JavaScript heap,
garbage collection, wall time or engine memory.

The generated ES module exports async `instantiate(bytes)`, authenticates a
bounded independent snapshot against its embedded exact module digest before
instantiation, and admits only an ordinary nonshared, nonresizable Uint8Array
carrier. It exposes a frozen `{ call(exportStableId, ...args) }` object, not
the imports, instance, memory, arena or raw handles. Inputs must be primitive
BigInt `i64` or Boolean values without coercion. The synchronous call returns
one frozen tagged outcome: scalar success; normalized language failure; or
capacity with a closed cause. Wrong argument shape is a pre-execution error;
unexpected runtime failures are terminal errors.

Busy/poison checks precede invocation work. Reentry sets absorbing poison even
if a nested exception is caught. Before outward success, language failure or
capacity publication, the arena must ALREADY be empty, live bytes zero, stack
restored, result/scratch facts consistent, and status/capacity cause coherent.
No `finally { entries.clear() }` may conceal missing generated drops. The next
invocation resets cumulative accounting only after the previous invocation has
proven settlement. Unexpected exceptions/traps do not claim cleanup or permit
reuse. Removing owned references is not a promise of synchronous garbage
collection or release of physical operating-system pages.

## Required evidence and nonclaims

Author success and stable diagnostic regressions with canonical source/Graph
witnesses. Compare scalar values and normalized language failures with native
C11 O0/O2 and the opt-in internal-String interpreter. Separate raw-module
accounting fixtures must observe exact mint/drop identities, zero survivors,
untouched failure output, repeated loop/call reuse, scalar assignment and the
unchanged String-assignment diagnostic,
late-argument failures, requires/ensures, guarded-match fallthrough, lazy
branches and embedded-NUL/multibyte String operations.

Hostile evidence covers exact/+1 owner/live/cumulative limits, empty owners,
concat overlap, each mint's capacity refusal, recovery after checked capacity,
wrong/stale/double drops, unexpected host errors, traps, reentry, forged module
bytes and input carriers. Compatibility witnesses cover unchanged ordinary and
earlier Project artifact paths. No local execution is authorized in this work;
format/diff inspection cannot satisfy these executable gates.

No fuel, deadline, cancellation, general sandbox, recursive-call guarantee,
trap recovery, Component Model runtime, public owned String boundary, package
promotion or overall production-readiness claim follows from this profile.

### Authored gate inventory

`tests/wasm_internal_strings_v1.rs` contains the canonical/Graph, normalized
interpreter/native O0/O2, raw Wasm all-reached-mint refusal, safe-facade,
generation-time quota and stable admission regressions. Its private
`raw.mjs` oracle is independent of the production arena. The `host.mjs`
engine-fault wrappers are test-only simulation: they exercise absorbing
poison after initialized ownership and caught reentry, not recovery from a
real engine crash. Boolean-return mint sites are also fault-injected.

Compiler units in `src/wasm/internal_strings/tests.rs` cover the fixed module
shape, unsupported profile boundaries, guarded matching and selection facts.
Private host units in `runtime/tests.rs` and `runtime/tests/arena.mjs` cover
byte/slot accounting, UTF-8/carrier rejection and terminal token exhaustion.
The token boundary uses a uniquely asserted test-only initial-counter splice,
not a public runtime counter override.

Earlier ordinary/native-v10/interpreter loop fixtures are corrected to obey
source admission; v10 retains one String across a Copy-only loop instead of
claiming per-iteration allocation. Existing legacy artifact known-answer
tests remain required and unrun. Calling an old emitter before/after the new
one is only state-isolation evidence, not a substitute for revision-to-revision
byte preservation. No tests, builds, runtime probes or hosted workflows were
executed as part of this implementation.
