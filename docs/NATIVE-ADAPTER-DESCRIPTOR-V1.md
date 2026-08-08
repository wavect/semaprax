# Native adapter descriptor v1

Status: private, descriptor-only phase-3 evidence. This is not a public or
stable ecosystem ABI.

The native resource preflight derives one descriptor per admitted function
from its sealed authority-free host template. That template already came from
the exact cleanup and value plans accepted for the function. Descriptor
derivation therefore cannot reclassify HIR, re-plan ownership, or accept
caller-supplied signature metadata.

## Canonical wire

All integers are unsigned 32-bit little-endian values. Every string is a
32-bit byte length followed by non-empty, NUL-free UTF-8. Fingerprints are raw
32-byte SHA-256 values, not hexadecimal strings. Ordered vectors remain in
semantic signature order.

| Field | Encoding |
| --- | --- |
| Magic | eight bytes: `SPXNABI1` |
| Version | `1` |
| Header size | `20` bytes |
| Total size | exact byte length of the complete descriptor |
| Physical target | framed target tag |
| Schema fingerprint | raw, independently domain-separated SHA-256 |
| Target fingerprint | raw hash of the complete target tag |
| Physical-module fingerprint | raw hash binding schema, target, semantic module ABI, and module identity |
| Function-template fingerprint | raw admitted-template fingerprint |
| Module and function | framed persistent identities |
| Parameters | count plus complete ordered tagged scalar or owned-resource entries |
| Result | tagged scalar `i64` or exact owned-input parameter/value/owner mapping |

The target tag binds architecture, operating system, environment, object
format, pointer width, endianness, and descriptor-getter calling convention.
The v1 provider is intentionally host-only. Its generated source checks C
compiler macros and exact pointer width before materializing the blob, so a
provider target that disagrees with any encoded property fails compilation.
Unknown or unprovable environments and object formats fail closed. Parameter indices and
owned-resource ordinals must be dense and canonical. Unknown tags, malformed
UTF-8, NUL, noncanonical indices, truncation, trailing data, fingerprint
inconsistency, or an inexact owned-result mapping are rejected by the strict
test decoder.

The schema, target, physical-module, and getter-symbol hashes use distinct
domains. The getter symbol binds both the physical module and function
template, so two admitted functions in one module cannot silently share a
descriptor symbol.

## Staged C surface

The generated header is consumable as C11 and C++ and declares one function:
an `extern "C"`-compatible getter returning a pointer to immutable,
library-owned descriptor bytes. The total length is in the fixed wire prefix.
The header asserts 8-bit bytes and exact integer widths, uses explicit
`__cdecl` on Windows, and supports hidden-default builds with one annotated
export.

No C struct layout, enum, `bool`, `size_t`, context, status, token, owner,
payload, output slot, allocator, finalizer, or callable SEMAPRAX function is
exposed. The blob itself is pointer-free. Strict tests compile the provider,
C consumer, and C++ consumer as separate translation units under hostile pack
scopes, link and execute them. A second test builds a real `.so`, `.dylib`, or
`.dll`, compiles a separate dynamic consumer (`dllimport` on Windows), runs it,
and requires the getter to be the sole dynamic export when the platform export
inspection tool is available.

## Trust and lifetime boundary

Compiler resource preflight derives the descriptor, header, and source and
then discards all of them. It does not bind the host ledger or create runtime
authority. Production retention, dynamic loading, module pinning/unloading,
runtime-owned storage, allocation-failure behavior, authenticated ownership
tokens, status/output lifetimes, unwind containment, imported finalizers, and
cross-thread execution remain future gates.

The separate test-only fake-backed Rust lease is never constructed from, and
does not retain, the shared-library provider built by this descriptor suite.
It proves strong-reference topology only; physical provider lifetime remains a
production-loader gate.

Consequently, public native resource emission still returns the exact generic
`SPX-B104` diagnostic. A future callable adapter must consume this compatibility
evidence and independently prove the host-ownership transaction and cleanup
protocols; the descriptor alone grants no right to execute or transfer an
owner.
