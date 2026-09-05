# Public Flat Owned Record C++ Adapter v1

Status: local C++17 integration evidence; unpublished and unpromoted.

Audience: compiler contributors, generated-package integrators, and promotion reviewers.

This additive Project-v9 projection is derived from an authenticated
`semaprax.public-flat-owned-record-api.v1` descriptor. It layers a safe C++17
value API over the low-level C provider boundary described by
[Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md). It does
not change Project-v9 admission, descriptor bytes, provider symbols, or the
provider carrier.

## Generated surface

`render_flat_owned_record_cpp_header` emits one aggregate per authenticated
record identity and one method per selected export in
`semaprax::flat_owned_record_v1`. The fixed mappings are:

| SEMAPRAX | C++17 |
| --- | --- |
| `i64` | `std::int64_t` |
| `bool` | `bool` |
| `usize` | `std::uint64_t` |
| `Bytes` | `std::vector<std::uint8_t>` |
| `borrow str` | invocation-only `std::string_view` |
| `borrow Slice<u8>` | invocation-only checked `ByteView` |

Record and field spellings remain the descriptor's injective stable-ID-derived
host names. Public records contain only host-owned values: no provider handle,
context, pointer, offset, alignment, padding, or native SEMAPRAX record layout
is exposed.

The generated `Client` is noncopyable, nonmovable, and bound to its creating
thread. Its context storage is aligned dynamically from the provider's checked
size and alignment. Cumulative borrowed input is limited to 65,536 bytes and is
checked before context entry.

## Ownership and failure protocol

Every result carrier slot is poisoned with `UINT64_MAX` before the provider
call. A recoverable semantic or adapter failure is accepted only if all slots
remain poisoned; the context closes before `Failure` is thrown. Unknown status,
carrier mutation on failure, malformed successful booleans, invalid handles,
copy/drop failure, or uncertain context closure fail-stop.

On success the adapter authenticates copied scalar encodings, copies the sole
owned byte handle into a fresh `std::vector`, drops that handle exactly once,
and closes the provider context. Only after those obligations settle does it
construct and return the public record. Allocation failure rolls back the
provider handle and closes the context before propagating the exception.

The included C boundary uses `SPX_FLAT_RECORD_STATIC(N)` so its array parameter
retains C11's `static N` minimum in C and is legal `N` syntax in C++. This is an
intentional generated-header byte correction, not a provider ABI change.

## Local evidence and limits

The focused Project test derives the C and C++ headers and real native provider,
compiles the provider as a separate C11 translation unit and the consumer as a
separate C++17 translation unit, links, and executes at `-O0` and `-O2`. It
checks repeated calls, descriptor-derived members, scalar values, byte contents,
and implicit settlement through the safe value-only API. Existing C11 evidence
continues to exercise invalid-input poison, copy, single-drop, stale-handle
rejection, and context closure.

This is not a package, registry artifact, supported ABI, MSVC or cross-platform
claim. It does not cover nested records, variants, resources, multiple owners,
owned parameters, callbacks, async work, or borrowed values that escape an
invocation. Those remain closed until separately specified and evidenced.
