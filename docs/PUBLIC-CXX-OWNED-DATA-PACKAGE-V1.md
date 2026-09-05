# Public C++ Owned-Data Package v1

Status: additive library-first contract; not a promoted Project target or CLI.
Audience: compiler, native-provider, C ABI, and C++ SDK maintainers.

## Subject and artifact

The package is available only for an already authenticated exact
`semaprax.project.v8` / `owned-data-api.v1` Project. The compiler reuses and
replays `semaprax.public-owned-data-api.v1`, then emits the existing native
provider and two new projections. It does not rediscover admission from source.

`semaprax.project-cxx-owned-data-package.v1` is compact canonical JSON in this
order: `schema`, Project schema/revision/workspace revision/graph digest, then
`descriptor`, `c_header`, `cxx_header`, `provider_c`, `limits`, and
`settlement`. Each artifact contains exact UTF-8 text, byte length, and SHA-256.
The outer digest is SHA-256 over the domain
`semaprax.project-cxx-owned-data-package.digest.v1\0`, the little-endian u64
canonical byte length, and the exact canonical bytes. Verification regenerates
all bytes from the held Project and compares them exactly; hashes cannot remint
an artifact.

The complete canonical package is at most 4,194,304 bytes. Semantic-provider
and canonical-provider logical output bytes are pre-bounded; an explicit
structural reserve covers the fixed runtime and maximum 32 export adapters.
This is not a claim about allocator capacity or total resident memory.
Borrowed input and owned output remain cumulatively bounded to 65,536 bytes.
Project v1-v7 and v9-v11 artifacts are unchanged.

## C boundary

`semaprax_owned_data.h` is a directly consumable C11 boundary and contains only fixed-width integers,
byte pointers, lengths, status values, and opaque context/handle types. No STL
type, exception, C++ object, allocator, or callback crosses `extern "C"`.
Booleans are exactly `uint8_t` values 0 or 1. Text is pointer-plus-length UTF-8;
no sentinel scanning is permitted.

The provider's closed statuses are success, semantic failure, adapter failure,
invalid handle, copy failure, and settlement failure. Output slots are poisoned
before calls and must remain unchanged on failure. Unknown status/tag,
non-canonical bool, invalid liveness, invalid length, or settlement uncertainty
fails stop.

## C++17 ownership

`semaprax_owned_data.hpp` is a header-only C++17 wrapper. `Client` privately
owns one aligned opaque context allocation and is neither copyable nor movable.
Every invocation initializes one context. Borrowed argument lengths are read
from plain `string_view`/`ByteView` values and preflighted left-to-right before
that initialization; the wrapper invokes no user conversion callback or proxy.
Their backing storage must remain valid and unmodified for the complete
synchronous call. C++ cannot enforce that caller-side alias obligation.

An active result handle is private and never returned. Its exact length is
queried, storage is allocated, bytes are copied, and the handle is dropped
before context close and result publication. Allocation exceptions trigger the
same drop-then-close rollback. A known semantic/adapter failure becomes
`Failure` only after a certain close. Copy failure is reported only after a
certain handle drop and close. Invalid handles, unknown tags/statuses, failed
drop, failed close, and cleanup uncertainty call `std::terminate`; continuing
could duplicate ownership or use an ambiguous context. No exception crosses C.

## Evidence and nonclaims

Local executable evidence separately compiles the generated provider and a
header-only C11 consumer at O0 and O2, links them with the C compiler, and
executes invalid-bool poison preservation followed by owned-byte call, length,
copy, single drop, stale-handle rejection, and context closure. The C consumer
uses only declarations from `semaprax_owned_data.h`; it does not include the
provider source or use the C++ wrapper.

Owning tests must cover exact replay and digest remint rejection, exact/+1 input
and output bounds, poisoned failure slots, stale/duplicate/wrong-context
handles, injected copy/drop/close failures, repeated success, and independent
C11/C++17 compilation at O0 and O2. This contract does not activate a manifest
target, filesystem publisher, CLI, package manager, dynamic loader, Windows
MSVC ABI, or hosted support claim.
