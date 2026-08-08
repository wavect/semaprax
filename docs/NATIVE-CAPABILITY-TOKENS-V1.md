# Native capability tokens v1

Status: private, disconnected mechanics for a future retained native runtime.
This is not a public C ABI and does not enable resource execution.

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

The suite also requires RFC 4231 HMAC-SHA256 test case 1 exactly, mutates all
512 token bits, runs a deterministic arbitrary-byte corpus across lengths zero
through 128, covers every short length plus an overlong token, both reserved
bytes, every sealed context dimension, owner/result scope separation, stale
expected generations, and maximum `u64` fields.

## Security boundary and nonclaims

The secret type is private, non-`Clone`, and absent from `Debug`. Its trusted
constructor rejects an all-zero key but cannot measure entropy. Deterministic
test keys prove only canonical HMAC mechanics. There is no OS CSPRNG
integration, entropy-failure path, retained library capability, unique-epoch
allocator, module pin/unload protocol, fork reseeding, locked memory, or audited
zeroization. Best-effort key filling on drop is not a memory-erasure guarantee.

HMAC authenticates bytes; it does not make a copyable bearer token linear or
prevent replay. Slot liveness, generation retirement, atomic duplicate checks,
executed-failure consumption, and owned-result reminting belong to the
synchronized host-ownership ledger. The current authentication transcript also
uses a bounded trusted-context allocation; an allocation-free callable
preflight must stream into the MAC or use preallocated storage.

The compiler only registers this private module. Resource preflight never
constructs a secret, binding, or token, and no C symbol exposes minting or
authentication. OS entropy, safe owner acquisition, runtime-owned outcome
storage, physical finalization, module lifetime, and hostile concurrency must
all land before a callable adapter can use this codec or `SPX-B104` can change.
