# Native capability tokens v1

Status: connected private protocol mechanics. The publishable compiler exposes
no authority, but the unpublished callable-v2 host uses this audited
codec/authority with the exact loader lease and synchronized ownership ledger.
This is not a public C ABI and does not open `SPX-B104`.

The codec defines authenticated bearer bytes for two staged capability kinds:

- `1`: a function-independent resource owner;
- `2`: a provisional owned result bound to the exact function-template
  fingerprint that produced it.

An owner remains eligible for different compatible functions because its token
does not contain function scope. Converting a provisional result into a general
owner will require a future synchronized registry transition and generation
rotation; the codec does not implement that state machine.

## Canonical 64-byte envelope

| Offset | Size | Field |
| --- | --- | --- |
| 0 | 4 | magic `SPXC` |
| 4 | 1 | version `1` |
| 5 | 1 | closed capability-kind tag |
| 6 | 2 | zero reserved bytes |
| 8 | 8 | nonzero binding epoch, little-endian `u64` |
| 16 | 8 | nonzero registry slot, little-endian `u64` |
| 24 | 8 | nonzero generation, little-endian `u64` |
| 32 | 32 | complete HMAC-SHA256 tag |

No payload, pointer, status token, ownership flag, secret, raw thread ID, or
library address appears in the token. Decoding uses exact slices and explicit
little-endian conversion; it does not reinterpret a Rust or C struct layout.
Wrong length, magic, version, kind, reserved bytes, or zero required integers
cannot produce claims.

## Authentication transcript

The implementation uses the exactly pinned RustCrypto `hmac` crate with
`Hmac<Sha256>`. Verification passes the complete 32-byte tag to
`Mac::verify_slice`; only that tag verification has the library's constant-time
guarantee. Parsing and the API as a whole are not claimed constant-time.

The canonical message contains these labeled fields in order:

1. domain `semaprax.native-capability-token.v1\0`;
2. raw physical-module fingerprint;
3. adapter binding identity;
4. binding epoch;
5. capability kind;
6. absent function scope for owners, or the raw function-template fingerprint
   for provisional owned results;
7. resource and lifecycle identities;
8. static thread-policy identity;
9. runtime-observed thread-binding identity, which is not a raw thread ID;
10. the exact 32-byte canonical token body.

Each field is encoded as `u64be(label_length) || label ||
u64be(value_length) || value`. The physical-module fingerprint transitively
binds the descriptor schema, physical target, semantic module ABI, and module
identity. Zero physical-module and function-template fingerprints are reserved
as uninitialized sentinels and rejected.

The checked provisional-result and owner vectors were independently reproduced
with Node's standard HMAC implementation:

```text
body  5350584301020000080706050403020118171615141312112827262524232221
tag   360d7ab6a2dc56f85c20af120455d368a9c6fd8b4cb683fee42e0a209096b0f0
token 5350584301020000080706050403020118171615141312112827262524232221360d7ab6a2dc56f85c20af120455d368a9c6fd8b4cb683fee42e0a209096b0f0

owner 535058430101000008070605040302011d000000000000001f00000000000000d7cda640f93588c8bf207a6a1c9bbbecd68dfc95f60c73dcc136adac4e9606fb
```

## Private native authority

The codec's production caller is the private Rust authority used by the
unpublished physical host.
Construction requires a module lease, derives the physical-module fingerprint
from it, and validates the immutable adapter, resource, lifecycle, and
thread-policy identities before requesting entropy. It then asks
the exactly pinned `getrandom` 0.4.3 `fill` API for one 72-byte seed:

- 32 bytes for the HMAC secret;
- 8 bytes interpreted as the little-endian binding epoch;
- 32 bytes for an opaque binding nonce, encoded as fixed lowercase hexadecimal
  before it enters the authentication transcript.

The [upstream target table](https://docs.rs/getrandom/0.4.3/getrandom/#supported-targets)
selects native sources for Linux/Android, Windows, macOS, and iOS without a
SEMAPRAX feature or custom backend configuration. The dependency's declared
MSRV is Rust 1.85, exactly matching this crate. Browser/WASI entropy is not
enabled by this native layer and needs a separate capability contract.

One fill error—including a partially modified destination—returns a stable
`EntropyUnavailable` category after best-effort temporary-buffer clearing. An
all-zero secret, zero epoch, or all-zero binding nonce returns
`InvalidEntropy`. There is no retry and no fallback to time, PID, counters,
environment, descriptor hashes, or compiled bytes. A successful fill is not a
mathematical uniqueness proof; exact entropy and context repetition produces
the same authority and is tested as an explicit nonclaim.

The authority captures `std::thread::current().id()` itself. Every owner/result
mint and authentication checks the actual current thread before codec parsing
or claim access. The raw thread ID never enters a token, transcript, error, or
trace. The authority deliberately remains `Send + Sync` so safe routing code
can receive a stable `WrongThread` rejection; tests pin that auto-trait policy,
exercise all four methods on another real thread, prove wrong-thread precedence
even for malformed credentials, then prove the original thread still works.
The authority, secret, and credential wrapper are not public. The authority and
secret are non-`Clone` and non-formatting; the credential wrapper is also
non-copying and non-formatting. These API properties reduce accidental logging
but do not make readable bearer bytes linear.

Test-only entropy injection cannot compile into a normal build. Its fixed seed
has independently reproduced complete authority vectors:

```text
owner  535058430101000008070605040302011d000000000000001f000000000000000bf252ff0712e1ddbed99617c0de27c8489806ea5af84f08aca9b8e7077fc480
result 53505843010200000807060504030201250000000000000029000000000000000f8fbb7b2da85b73c36e9fdbb402ea60db4d61acf0f5db0dfc67dabb9d247bc5
```

## Retained-module lease topology

Exact module-instance identity is the private `Arc` allocation, not a path,
fingerprint, descriptor, or bearer-token byte string. Compiler-side topology
tests substitute a fake retained pin, while the unpublished host supplies the
exact callable-v2 loader pin. The authority owns one lease; every minted owner or provisional-result
credential wrapper explicitly retains the same allocation. Raw 64-byte token
bytes retain nothing. Authentication checks exact lease-instance identity
before accepting the wrapper.

The authority retention path checks the current process ID against the
injected loader origin while preserving that origin's incarnation, then passes
a one-way open-to-draining gate. The lower-level test seam separately proves
that a mismatched process ID or incarnation is rejected before state access.
Draining rejects new retention, minting, and authentication without revoking
existing lifetime pins. The fake pin releases exactly once after the final
strong reference, including concurrent final drops. Its leaf allocation
contains no authority, credential, registry, callback, finalizer, or other
retention backedge.

Two fake loads may deliberately have identical physical fingerprints and,
under repeated test entropy and context, identical authenticated bytes. Their
credential wrappers still fail cross-instance authentication. This proves why
the wrapper's allocation identity is necessary and why copied bearer bytes are
not a module-lifetime capability.

The authority suite additionally covers entropy error after a partial test
fill, every all-zero structural component, invalid binding before any entropy
request, exact one-fill behavior, secret/epoch/nonce/module/adapter/resource/
lifecycle/thread-policy/function changes, zero slot/generation mapping, stable
error redaction, native OS smoke, and catastrophic full-entropy repetition.
Separate lifetime tests cover invalid fake identities, equal-fingerprint
instance nonconflation, draining, process/incarnation mismatch, exact-instance
authority and credential retention across drop orders, concurrent last drops,
cross-instance rejection despite equal token bytes, deliberate traits, and
absence of retention cycles.

The suite also requires RFC 4231 HMAC-SHA256 test case 1 exactly, mutates all
512 token bits, runs a deterministic arbitrary-byte corpus across lengths zero
through 128, covers every short length plus an overlong token, both reserved
bytes, every sealed context dimension, owner/result scope separation, stale
expected generations, and maximum `u64` fields.

## Security boundary and nonclaims

The secret and authority types are private, non-`Clone`, and absent from
`Debug`. Compiler-private topology tests still use a fake pin. The unpublished
physical host connects the same protocol to strict descriptor, dictionary, and
trace-certificate admission, a real exact callable lease, generated provider
execution, and ledger owner generations. It still has no independent
code-provenance authentication, general callback/finalizer quiescence, proven
Windows dependency-collision runtime, fork recovery/reseeding, locked memory,
or audited zeroization. Authentic origin and fork integration remain future
boundaries.
Best-effort filling of temporary/key buffers is not a memory-erasure guarantee,
and HMAC implementation internals may copy key material.

HMAC authenticates bytes; it does not make a copyable bearer token linear or
prevent replay. Slot liveness, generation retirement, atomic duplicate checks,
executed-failure consumption, and owned-result reminting belong to the
synchronized host-ownership ledger. The current authentication transcript also
uses a bounded trusted-context allocation; an allocation-free callable
preflight must stream into the MAC or use preallocated storage.

The compiler only registers these private modules. Resource preflight never
constructs a secret, authority, binding, or token, and no C symbol exposes
minting or authentication. The unpublished host's unsafe adoption and generated
callable executor are private physical plumbing evidence, not the public native
adapter. General postcommit fallback cleanup, physical finalization and
quiesced unload, code identity, fork handling, hostile concurrency, confirmed
Windows runtime, mobile profiles, a green public dynamic-provider sanitizer
run, Rust-host sanitizer instrumentation, and public compiler emission must
land before `SPX-B104` can change.
