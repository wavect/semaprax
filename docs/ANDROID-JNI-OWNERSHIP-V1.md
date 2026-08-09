# Private Android JNI ownership adapter v1

Status: private implementation and CI configuration exist; local Rust/C and
source-lock evidence is green, and the packaging contract is source-locked. The
first hosted API-35 x86_64 APK build, install, and Emulator execution is
pending, so this is not yet hosted Android runtime evidence or a public Android
application boundary.

This document freezes the first bounded Kotlin/JNI projection of SEMAPRAX
ownership. It connects one exact generated `token.discard-two` callable-v3
provider to the unpublished native host and authenticated receipt ledger. The
adapter is evidence for the ownership boundary described by
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md), not a public resource ABI and
not permission to open `SPX-B104`.

## Scope

The first fixture owns one pair of direct-trivial resource payloads. Explicit
`OwnedSession.consume()` runs the exact generated provider, which finalizes
ordinal 1 before ordinal 0 and publishes scalar zero. The non-throwing
`AutoCloseable.close()` wrapper and Cleaner-compatible fallback both dispatch
the same native consume operation asynchronously; they are not SEMAPRAX's
fallible explicit `close` import. No JVM callback is permitted after native
`CallCommit`.

The adapter must prove:

- one exact Android/Bionic/ELF descriptor and provider image;
- a thread-bound native host and receipt ledger;
- opaque generation-tagged Kotlin handles rather than pointer casts;
- all-or-none transfer of the pair into the native call;
- exactly-once physical finalization and no retry after uncertainty;
- deterministic status normalization before a Kotlin exception is created;
- non-throwing automatic cleanup;
- stale, forged, cross-runtime, duplicate, reentrant, and wrong-thread
  rejection before ownership mutation;
- exact O0/O2 behavior in an installed API-35 x86_64 Emulator APK once the
  configured hosted job is observed green; and
- arm64 Android JNI/provider compilation and ELF inspection without claiming
  arm64 device execution.

## Thread and lifetime model

`PrivateSettlementHostV3` and its exact loader lease remain `!Send` and
`!Sync`. Kotlin owns one dedicated `HandlerThread` per native runtime. Provider
admission, handle adoption, call execution, receipt commit, drain, and runtime
drop all execute on that thread. The native runtime is stored only in that
thread's local storage.

The Cleaner-compatible daemon never enters the native host. It atomically
claims the wrapper's handle and enqueues one cleanup command on the owning
`HandlerThread`. Its cleanup action retains the dispatcher but never the
wrapper object. Runtime shutdown is one-way and succeeds only after the handle
table is empty and no call is active or quarantined. Process termination is
outside the cleanup guarantee.

`JNIEnv` and JNI local references are call-local. They are never retained or
shared between threads. `JNI_OnLoad` registers a closed native-method table and
performs no provider admission. `JNI_OnUnload` does not attempt to tear down a
thread-affine runtime.

## Opaque handle encoding

`SPXAJH01` is a positive, nonzero `u64` with this exact layout:

| Bits | Field | Rule |
| --- | --- | --- |
| 63 | sign/reserved | zero |
| 48..62 | runtime tag | nonzero 15-bit process-lifetime tag |
| 24..47 | generation | nonzero 24-bit generation |
| 0..23 | slot | nonzero 24-bit slot |

The known answer for runtime tag 1, generation 1, slot 1 is
`0x0001000001000001`. Rust and Kotlin must independently encode and decode this
value.

A table entry is one of `Vacant`, `Live`, `Claimed`, `Consumed`,
`Quarantined`, or `Retired`. A consume operation validates the complete token
and atomically changes `Live` to `Claimed` before touching the receipt ledger.
A genuine precommit rejection restores the same entry to `Live`. Executed
success, executed failure, or authenticated abort changes it to `Consumed`.
Uncertain postcommit state changes it to `Quarantined` and poisons/drains the
runtime. A reusable terminal slot increments its generation before returning
to `Live`; generation exhaustion permanently retires the slot. Runtime tags
are not reused during the process lifetime.

Payload equality never establishes ownership identity. Payload zero and
`u64::MAX` remain valid. Handles do not expose addresses, Rust owner values,
loader identities, or capability bytes.

## Settlement handoff

The outer table is the JVM wrapper authority before a call. It is not a second
copy of a live settlement-ledger owner. At consume time the adapter claims the
outer entry, asks the native host to adopt both exact owners as one preflighted
group, and transfers them at the existing `CallCommit` boundary. Partial owner
registration is forbidden. Only a defined rejection before native host
execution begins leaves the receipt ledger unchanged and restores the outer
entry; any host-execution error is conservatively terminal or uncertain. Once
`CallCommit` succeeds, the outer entry can never become live again.

All parsing, buffer reservation, status storage, quarantine capacity, and
receipt storage required by the native provider are acquired before
`CallCommit`. The existing callable-v3 evidence and zero-Rust-allocation gate
remain authoritative for the irreversible interval. Kotlin/JNI allocation
before or after that interval is not evidence of native postcommit allocation.

## Status projection

JNI does not expose a context-local SEMAPRAX status token. `SPXAJS01` is a
private fixed `u64` projection:

| Bits | Field |
| --- | --- |
| 0..31 | nonzero status code |
| 32..34 | status class |
| 35..36 | retryability |
| 37..52 | nonzero adapter-manifest domain ordinal |
| 53..63 | zero |

Class tags are `1 Contract`, `2 Arithmetic`, `3 Import`, `4 ExplicitClose`,
and `5 Adapter`. Retryability tags are `0 Unknown`, `1 false`, and `2 true`.
The known answer for domain ordinal 1, class Adapter, retryability false, and
code 1 is `0x0000002d00000001`. Zero is the only success word.

The private manifest defines at least these stable domains:

- `semaprax.android.jni.v1` for boundary and handle errors;
- `fixture.jvm.v1`, code 7, class Import, retryable false, for the one declared
  throwing callback fixture; and
- `semaprax.adapter.unexpected.v1`, code 1, class Adapter, retryability
  Unknown, for every undeclared JVM throwable.

Exception class names, messages, stack traces, and objects are nonsemantic
sidecars. A JNI callback probe is precommit-only. The shim detects a pending
exception, classifies it using the closed manifest, clears it, finishes native
cleanup, and returns a status word with no pending JNI exception. Kotlin may
construct and throw `SemapraxException` only after JNI returns. Cleaner cleanup
never throws or reports a recoverable error.

## Kotlin ownership wrapper

The API-28-compatible cleanup layer uses `PhantomReference` and
`ReferenceQueue`; it does not rely on `java.lang.ref.Cleaner`, which is newer
than the native minimum API. Explicit `consume()`, `AutoCloseable.close()`, and
fallback cleanup share one atomic handle cell. Only the winner may perform or
enqueue native consumption. A second wrapper `close()` is an idempotent
non-throwing no-op, while a copied raw token remains stale at the native
boundary and cannot finalize again.

Correctness tests invoke `cleanable.clean()` through `cleanForTest()`, thereby
running the identical registered `PhantomReference` cleanup action, and then
cross the dispatcher's drain barrier. Nondeterministic GC/enqueue observation,
collection of a wrapper, and process-exit cleanup are not evidence. The wrapper
uses a reachability fence or equivalent around an explicit native operation so
fallback cleanup cannot race a still-live call.

## Required executable evidence

Rust tests cover handle known answers, every reserved/zero/high-bit field,
same-payload distinct identity, capacity, generation reuse and exhaustion,
stale/copy/replay/cross-runtime rejection, all-or-none claim/restore, terminal
execution, quarantine, drain, out-slot preservation, and panic containment.

The JNI shim is compiled with the pinned NDK for x86_64 and arm64 using strict
C warnings. Its exported-symbol allowlist, target architecture, dynamic
dependencies, absence of workspace search paths, method registration table,
pending-exception discipline, and status known answers are inspected.

The configured no-UI instrumentation APK is required to exercise both O0 and O2
providers and publish one exact app-private result. Its assertions cover explicit `consume()`,
deterministic fallback cleanup, a consume-versus-Cleaner race, copied/stale/forged/
cross-runtime/wrong-thread rejection, declared and unexpected exception
normalization, exact finalizer order and payload, nonzero receipt/candidate/
identity evidence, a changed ledger digest, a healthy host, zero measured Rust
postcommit allocations, and an empty handle table after the drain barrier.
The exact file is `files/semaprax-android-jni-v1.txt`, read with `run-as`, and
its canonical success line includes API 35, x86_64, the O0 explicit-consume and
O2 Cleaner paths, all known answers, `finalizers=1:13,0:11`,
`publication=no-owned`, `allocations=0`, and `handles=0`. This paragraph
describes the implemented assertion contract; it becomes hosted execution
evidence only after the dedicated job is green.

The APK and every native library are inspected before installation. The build
uses no AndroidX, JUnit, Compose, UI resource, dynamic dependency version, or
network repository. The Gradle packaging task runs offline and uses only the
checked project plus preinstalled, version-checked Kotlin and Android build
tools.

## Deliberate nonclaims

This tranche does not implement general resource or imported-finalizer
execution, SEMAPRAX's explicit fallible `close` import, a stable public native
handle ABI, postcommit JVM callbacks, reentrant or nested calls, async,
cancellation, cross-thread resource transfer, hot reload, general quiescence,
process-kill cleanup, malicious-code containment, signed artifact provenance,
arm64 device execution, UI/Compose/View/accessibility behavior, AAR
publication, the broader callable corpus, or public compiler admission.

The Java/Kotlin and Android completion rows remain incomplete. Ordinary
resource-bearing native builds continue to fail with `SPX-B104`.
