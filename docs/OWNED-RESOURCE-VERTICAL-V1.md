# Owned resource vertical slice v1

Status: implementation contract. This document defines the first public
resource-execution slice; it does not make the slice implemented. Native
`SPX-B104` and WebAssembly `SPX-W111` remain mandatory until every admission
and conformance gate below is executable.

Current private evidence now connects feature-gated compiler emission to the
real callable-v2 loader/authority/ledger host. It executes the complete 14-case
corpus from generated O0/O2 shared libraries and exactly matches the reference
and Node/Wasm outcomes and semantic traces. That closes the former
former callable-composition gap, but not this public gate.

## Purpose

The first slice converts the existing validated ownership and cleanup meaning
into real host behavior on both native and WebAssembly targets. It must consume
the exact attached `semaprax.cleanup-plan.v1`; a backend or host may not infer,
repair, or independently choose ownership behavior after HIR validation.

The slice admits only monomorphic functions whose complete reachable type and
control-flow shape is:

- one exact direct `drop trivial` resource/lifecycle identity, with every
  resource parameter received as `own`;
- `i64` and `bool` value parameters;
- an `i64` or one exact owned input resource as the result;
- the already classified `requires`, `ensures`, comparison, checked-add,
  and cleanup-plan operations exercised by the native
  conformance corpus; and
- no records, projections, imported finalizers, generic types, loops,
  nested/direct calls, callbacks, async work, foreign calls, or resource
  allocation inside SEMAPRAX code.

Every excluded shape continues to fail before artifact emission with its
stable backend diagnostic. Admission is additive and structural, never based
on a function name or source spelling.

## Shared ownership transaction

A host invocation has five ordered phases:

1. **Preflight** borrows every caller owner without consuming its safe wrapper,
   then validates the HIR, attached cleanup plan, target contract, module
   instance, function template, all handles and generations, result capacity,
   status arena, and trace capacity without changing ownership.
2. **Commit ingress** performs one indivisible ledger state change whose owner
   vector is ordered by parameter ordinal. It consumes all owned arguments or
   none; "left-to-right" defines semantic event order, never sequential partial
   consumption. Rejection before this point consumes nothing; no rejection is
   possible afterward.
3. **Execute** follows the validated cleanup CFG and records the first sticky
   normalized failure.
4. **Finalize** clears liveness before each reverse-order finalizer. On
   success, every live non-result owner is finalized exactly once while the
   provisional owned result remains live. On failure, the provisional result
   is finalized too.
5. **Publish** atomically transfers the still-live provisional owned result,
   or commits a scalar result, only after successful postconditions and
   non-result finalization. Every failure leaves caller result storage
   logically uninitialized.

The canonical status and semantic trace must equal the target-neutral
reference executor byte-for-byte. Physical addresses, platform handles,
secrets, loader paths, and target-specific identities never enter that trace.
Every fallible allocation, generation advance, result credential or handle
reservation, status/trace capacity check, and serialization bound check occurs
before commit. A host exception after commit is an executed adapter failure,
not a rejection. The current direct-trivial host retires its committed logical
ledger state, but a general canonical fallback cleanup/finalizer trace and
physical quiescence protocol remains required before this contract is complete.

## Native host

The native host owns, without exposing:

- one exact retained platform-loader instance and admitted immutable
  descriptor;
- one same-thread capability authority bound to that exact instance;
- one synchronized owner ledger with nonzero generation counters;
- runtime-owned resource payload storage;
- active-call and finalizer quiescence state; and
- one typed callable entrypoint for each admitted function template.

Native admission is an explicit `unsafe` trusted-code boundary. The caller of
`unsafe open_admitted` must guarantee the exact root image, every selected
dependency, same-root provenance and exact ABI of every callable symbol, and
stable loader namespace for the complete lifetime of the host, every retained
lease, and every call. All callable symbols and dependencies are eagerly
resolved during admission; later dynamic or delay resolution is prohibited.
Descriptor equality alone is insufficient. Once those lifetime safety
preconditions and structural decoder checks succeed, the returned
thread-confined host object may expose safe calls; SEMAPRAX v1 does not
independently authenticate the caller's provenance claim.

Callable providers use descriptor v2 and a strict call ABI; descriptor v1
remains descriptor-only and continues promising no callable owner API. The v2
wire fixes the exact symbol allowlist, parameter/result layouts,
readable/writable lifetimes, normalized status behavior, unwind prohibition,
and semantic-versus-physical fingerprints. It independently binds the event
dictionary and compiler-owned trace-path trie-DFA. Generated C compile-guards
the authenticated architecture, OS, environment, object format, pointer width,
and endian profile. Descriptor mutation, strict request/response codec, target
mismatch, and hostile trace-path tests now fail closed. Generated C,
descriptor-v2 bytes, fingerprints, certificates, and export symbols remain
deterministic across repeated derivation.

Initial native owners enter through a separately audited `unsafe` adoption
boundary. Its caller proves unique ownership, valid type/lifecycle identity,
payload validity, and absence of foreign aliases. Admission allocates a fresh
nonzero ledger slot and generation before returning an opaque owner; failure
leaves ownership with the caller. Safe code has no raw-payload adoption path.

Safe calls borrow mutable owner wrappers during preflight and mark them
consumed only at atomic ingress commit. A rejected call returns with the exact
wrappers still live and reusable; tests must successfully invoke or drop them
after rejection. Dropping any live owner or owned-result wrapper outside a call
retires its ledger generation, runs its finalizer exactly once, and only then
releases its module pin. Dropping an already consumed wrapper performs no
second finalization.

The physical lease, authority, owner credentials, result credentials, calls,
and finalizers are thread-confined in v1. They are neither `Send` nor `Sync`.
A future bound-thread executor may expose sendable request handles, but it may
not move the loader lease or cause the final platform release on another
thread.

Draining is one-way. It rejects new owner creation, retention, and calls while
preserving every existing lifetime pin. The platform handle becomes eligible
for release only after owners, credentials, calls, callbacks, and finalizers
are quiescent. Eligibility does not claim immediate physical unmapping.

The publishable compiler crate remains `unsafe_code = "forbid"`. Platform
loading stays isolated in the unpublished loader crate; production host code
belongs in a separate unpublished workspace crate so `cargo package -p
semaprax` remains independently verifiable.

## WebAssembly host

Each instantiated module owns a private resource table. The core-visible v1
handle is one nonzero `i32` packing an 11-bit runtime tag, 10-bit nonzero
generation, and 10-bit nonzero slot; it is not a linear-memory pointer. A slot
may be reused only with a strictly advanced generation; generation exhaustion
permanently retires the slot, and tag or slot exhaustion returns a stable
non-mutating host error. Runtime tags are not reused within one loaded host
module. The instance also owns its status arena, trace buffer, call context,
and shadow stack. Handles and contexts from another instance are rejected
before ownership changes.

Ingress validates every handle first, then commits all owned arguments
atomically. Successful owned publication rotates the generation and returns a
fresh handle. Consumed, copied, stale, zero, out-of-range, wrong-instance, and
replayed handles fail closed. Linear-memory ranges use checked arithmetic and
must be completely in bounds before any read or write. Host exceptions are
normalized into a stable adapter status and cannot unwind through the Wasm
boundary.

The first host ingress operation inserts an already host-owned payload into
one instance table. It must consume a unique, noncopyable host ownership
identity and allocate a fresh nonzero slot and generation. Reusing an adoption
identity is rejected, while distinct ownership identities with equal opaque
payload bytes are valid. This is a trusted host boundary, not a SEMAPRAX
allocation primitive. The returned handle is the only ownership capability
visible to the Wasm module.

Dropping a live host handle retires its exact packed tag/slot/generation and
finalizes its payload once. Rejected calls leave the original handle value live
and reusable; successful ingress invalidates it before execution continues.

The Wasm implementation must execute the same cleanup plan and emit the same
semantic trace as native and the reference executor; a JavaScript-only shadow
model is not backend conformance evidence.

## Required corpus

The public native artifact, public Wasm artifact, and reference executor must
all cover:

- discard with zero and maximum opaque payloads;
- two owned inputs with reverse exact-once cleanup;
- true and false preconditions;
- checked addition success, overflow, and failed precondition;
- scalar result publication;
- owned identity publication;
- selection of the second of two same-typed owned inputs, including equal
  opaque payloads;
- failed precondition before owned selection; and
- failed owned-result postcondition with finalization and no publication.

For every case, compare the normalized status, publication state, complete
event sequence, storage/lifecycle identities, and final liveness. Native O0
and O2, ASan, and UBSan executions must agree exactly.

The private native corpus now exercises non-`cfg(test)` feature-gated compiler
and host surfaces: the compiler emits the artifact, the host loads it, and the
safe call API runs the corpus through its ledger. This is stronger than a
generated-source harness, but the ordinary public compiler build/preflight path
still rejects with `SPX-B104`; therefore the production-reachability gate is
not complete.

## Hostile boundary gates

Tests must prove non-mutating rejection of malformed, copied, stale,
cross-registry, cross-instance, wrong-thread, and draining credentials;
descriptor, target, function-template, and physical-instance mismatches;
undersized result/status/trace storage; invalid Wasm handles, generations,
contexts, and memory ranges; replay and double consumption; and hostile HIR or
cleanup-plan mutations.

Drop-order tests must retain the exact physical module through the authority,
every live owner/result credential, active call, and finalizer. Separate opens
of equal paths and equal descriptors remain different instances. The safe API
exposes no raw pointer, callable symbol, loader handle, manual close, secret,
or copyable owner credential.

Rejection tests must reuse the exact native owner wrappers and Wasm handle
values successfully afterward. Ordinary owner drop, owner drop after rejection,
owned-result drop, consumed-wrapper drop, and every drain/drop ordering must
prove one ledger retirement, one finalizer, and correct last-pin release.

## Quality gates

The slice cannot be admitted until all of these pass:

- exact native/reference/Wasm conformance for the complete corpus;
- real runtime-loaded and callable native fixtures on Linux, macOS, and
  Windows, including exact cleanup traces;
- real Wasm execution through Node in CI;
- a green public run of the Linux dynamic-callable job, completed by
  [run 31256134955, job 93099637801](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801),
  with all 14 O0/O2 cases executing from ASan/UBSan-instrumented generated
  providers loaded through the host;
- sanitizer instrumentation of the Rust host itself, not merely linkage of the
  sanitizer runtimes required by the generated providers;
- compile-fail thread-confinement and non-copying/non-formatting API tests;
- malformed-input and hostile-HIR suites;
- exhaustion and wraparound suites for module instances, owner slots,
  generations, invocation IDs, status arenas, trace/event counts, and encoded
  lengths, all proving non-mutating pre-commit failure;
- deterministic double-build equality for generated C, descriptor v2, Wasm,
  and stable export symbols;
- an unsafe-boundary audit proving that platform loading, callable FFI, and
  raw-payload adoption are confined to named quarantine crates/modules while
  the compiler crate remains unsafe-free;
- Rust 1.85, strict workspace Clippy, docs with warnings denied, root package
  verification, and the complete cargo-deny policy, always with every workspace
  feature enabled; and
- the existing native, web, and example matrix without weakened tests.

## Explicit nonclaims

This v1 slice does not implement imported finalizers, records, variants,
`Option`, `Result`, borrowing across FFI, allocation APIs, callbacks, async,
concurrency, fork recovery, hot reload, independent same-root native symbol
provenance authentication, Windows dependency-collision runtime evidence,
signed code admission, WebAssembly
Components, packages, UI, or mobile/desktop application hosts. Those remain
subsequent completion-matrix gates; this slice is the ownership prerequisite,
not a substitute for them.

It also does not yet prove general physical/malformed-response fallback cleanup
and quiescence, sanitizer instrumentation of the Rust host itself, a green
public Windows callable/dependency-collision run, Android device admission, an
iOS static-link profile, or public compiler emission. Those are blockers for
`SPX-B104`, not implications of the private 14-case success.

The sanitizer job above was green even though unrelated Clippy/GCC failures
kept its overall workflow run red. It is not evidence for Rust-host
instrumentation, Windows runtime behavior, or a green overall platform matrix.
