# Public Generic Metadata Consumers v1

Audience: generated-consumer integrators, ABI reviewers, and promotion
reviewers.

Status: the four metadata consumers below are an implemented bounded
generator, hosted green on Linux, macOS, and Windows with all four consumer
toolchains exercised on each. This closes the *grammar* half of gates PG-5
and PG-6 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
The *calling* half now has a first, local-only implementation for Rust (see
[Rust calling consumer (issue #156)](#rust-calling-consumer-issue-156)
below), for C11 (see
[C11 calling consumer (issue #158)](#c11-calling-consumer-issue-158) below),
and for TypeScript/Wasm (see [TypeScript/Wasm calling consumer (issue
#157)](#typescriptwasm-calling-consumer-issue-157) below), built on the
versioned descriptor ([issue #152](PUBLIC-GENERIC-DESCRIPTOR-V1.md)) and the
native and Core Wasm physical carrier adapters
([issue #154](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154),
[issue #155](PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155))
that did not exist when the metadata half closed; both gates stay open for
the remaining language's calling consumer (C++ - issue #159) and for hosted
evidence of the Rust, C11 and TypeScript ones. Public generic ownership
remains unsupported and unpublished.

## Why metadata consumers come first

Before a foreign toolchain can *call* a public generic export, several
toolchains have to agree on what its types are — and refuse everything else.
That agreement is falsifiable on its own, without a calling convention: give
four languages the same canonical description, and require all four to accept
exactly those bytes and to refuse each hostile variant with the same closed
reason.

A generated consumer here therefore receives no SEMAPRAX value, allocates
none, frees none, instantiates no Wasm module, and links against nothing. Its
generated type declarations show the substituted field tree in the target
language; they define no layout, no ownership transfer, and no ABI. Every
generated file carries that statement in its own banner, and the executable
gate asserts the banner is present.

## Identifiers

| Layer | Identifier |
| --- | --- |
| Metadata format | `semaprax.public-generic-consumer-metadata.v1` |
| Magic prefix | `spxpgcm1;` |
| Languages | `rust`, `typescript`, `c`, `cxx` |
| Type spelling | [Public Generic Type Grammar v1](PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md) |
| Described surface | [Public Generic Compatibility v1](PUBLIC-GENERIC-COMPATIBILITY-V1.md) |

## The metadata format

```
document := "spxpgcm1;" record*
record   := "|" <field count> ";" field+
field    := <byte length> ":" <bytes> ";"
```

Counts are decimal without a leading zero. Every field is framed by its byte
length, so a declaration identity may contain the format's own punctuation — or
a newline — without a parser ever guessing where a field ends. The first field
of a record is its kind:

| Kind | Fields |
| --- | --- |
| `E` | export identity, parameter count |
| `P` | export identity, index, position kind, ownership mode, type |
| `R` | export identity, position kind, type |
| `I` | instance term, template declaration identity, declared arity |
| `A` | instance term, argument index, argument term |
| `F` | instance term, field index, field identity, field term |
| `L` | instance term, leaf index, owned-leaf path |
| `D` | surface digest |

Record order is the surface's own byte order — exports by declaration
identity, instances by canonical term, then by index — and the `D` record
closes the document, so a stale description differs from a fresh one even when
every other fact agrees.

## What every consumer does

1. Parse strictly. Exact byte lengths, no leading zero in a count, a `;` after
   every field, a `|` before every record, no NUL inside a field, and nothing
   after the last record.
2. Validate every type field as a canonical term of the type grammar, with the
   grammar's own 64-level and 4,096-node bounds, requiring the term to consume
   its field exactly.
3. Compare the whole document against the metadata the consumer embeds.

Refusals are a closed three-value vocabulary, identical in all four languages:

| Reason | Meaning |
| --- | --- |
| `malformed` | not a well-formed document of the format |
| `term` | a type field is not a canonical grammar term |
| `mismatch` | well formed, but not the expected document |

There is deliberately no fourth "non-canonical" reason. The format admits no
optional whitespace, no alternate spelling, and no leading zero, so a strict
parse that consumes the document already implies the bytes are their own
canonical rendering; a reason no input could produce would be decoration, not
a closed vocabulary.

A consumer run with no argument verifies its embedded bytes; run with a file
it verifies that file. It prints `ok` and exits 0, or prints
`refused:<reason>` and exits 3.

## Generated declarations

Type names are derived from bytes, not from display names: the lowercase hex of
the exact instance term or field identity. A rename therefore changes no
generated identifier, and two distinct instances can never collide. Nested
instances are declared before the instances that hold them, because a struct
member of an incomplete type is an error in C and C++.

The scalar spellings are declaration types for reading metadata, not an ABI
mapping:

| Grammar | Rust | TypeScript | C | C++ |
| --- | --- | --- | --- | --- |
| `i64` | `i64` | `bigint` | `int64_t` | `std::int64_t` |
| `i32` | `i32` | `number` | `int32_t` | `std::int32_t` |
| `u8` | `u8` | `number` | `uint8_t` | `std::uint8_t` |
| `usize` | `u64` | `bigint` | `uint64_t` | `std::uint64_t` |
| `char` | `char` | `string` | `uint32_t` | `char32_t` |
| `f32` | `f32` | `number` | `float` | `float` |
| `f64` | `f64` | `number` | `double` | `double` |
| `bool` | `bool` | `boolean` | `bool` | `bool` |
| `bytes` | `Vec<u8>` | `Uint8Array` | `struct spx_pg_bytes` | `std::vector<std::uint8_t>` |

The TypeScript consumer is two files: the ES module a Wasm host would load and
the ambient declarations a TypeScript author compiles against.

## Bounds and diagnostics

| Bound | Value |
| --- | --- |
| Canonical metadata bytes | 1,048,576 |
| Records per document | 8,192 |
| Fields per record | 8 |

Reaching a bound is a refusal, never a truncated or repaired document.
`SPX-PG401` reports a refusal on the Rust side and `SPX-PG402` a bound.
Generation is deterministic — regenerating produces byte-identical files — and
authority-free: it reads checked facts, returns source text, and touches no
file, process, or network.

## Evidence

The `public_generic_consumers` gate in the projections harness generates all
four consumers, compiles them warning-free (`-Wall -Wextra -Werror` for C and
C++, `-D warnings` for Rust), and runs each one on its embedded metadata, on
the same bytes from a file, and on nine hostile documents: empty, wrong magic,
truncated, a leading-zero count, a field count reaching past the record, a
forged term length, reordered records, an appended record, and a stale surface.
Each language must report the same closed reason as the Rust reference reader
and as every other exercised language, and the gate fails if no toolchain was
available rather than passing silently. A language whose toolchain is absent is
skipped; that is a narrower run, not a passing one.

## Hosted evidence

Hosted evidence: the milestone corpus passed on `ubuntu-latest`, `macos-latest`,
and `windows-latest` for implementation commit `2ef043ba1b989f49b256e456f71fb6e89068bf33` in
[run 34594793245](https://github.com/wavect/semaprax/actions/runs/34594793245). That is evidence for the corpus this document owns, not for the
milestone's remaining gates.

## Nonclaims (metadata consumers)

The four metadata consumers described above define no calling convention,
descriptor, carrier, package, layout, allocation, or ownership transfer, and
no value crosses any boundary through them. Generation does not admit a
public generic signature: the public projections still reject generic
surfaces, and the milestone's separation gate continues to prove it. The
hosted run recorded above covers this corpus and nothing else: it is not a
support decision and not a publication. Calling a public generic export over
a versioned descriptor and carrier is a separate generator, described next
for Rust, C11 and TypeScript/Wasm; the C++ calling consumer remains untouched
(issue #159), which is why PG-5 and PG-6 stay open for every language but
Rust, C11 and TypeScript despite the sections below.

## Rust calling consumer (issue #156)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for Rust.

Status: local, proof-only evidence only (no hosted CI run recorded for this
section), unsupported and unpublished. Extends the existing generator
(`semaprax::public_generic_consumer::rust_calling`) rather than a parallel
framework: it reuses the metadata consumer's own identifier scheme
(lowercase hex of identity bytes, never display text) and its
`.txt`-template/LF-normalization convention, and it links against — never
reimplements or modifies — the native C11 physical adapter
([issue #154](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154)).

**What is generated versus hand-written.** Every file below is produced by
`generate_rust_calling_consumer(descriptor_bytes, binding, input, output)`,
a pure function from already-trusted descriptor/binding bytes and a
`RecordShape` to source text — no file is hand-copied into a fixture. Two
honest qualifications:

- `src/provider.rs`'s FFI declarations (the `extern "C"` block, status
  constants, and the safe `Provider`/`ResultGuard` wrapper logic) are a
  **fixed template** baked into the generator, not derived per-descriptor —
  they restate `spx_pg_v1.h` (a frozen, versioned ABI), so nothing about a
  specific descriptor changes them. This mirrors
  `native/template.rs`'s own `HEADER_V1`/`BODY_V1` being fixed text pasted
  around descriptor-dependent byte constants.
- The concrete type model is scoped to what the bound native provider
  actually implements today: a flat, descriptor-ordered sequence of owned
  `Bytes` leaves (see
  [Native C11 physical adapter](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154)'s
  own "existing owned-Bytes shapes" scope note). A Copy-scalar or
  nested-record leaf is future generator work tracked by the same #119
  prerequisite that scope note names, not a limitation invented here.
  [Public Generic Boundary Profile v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md)
  admits exactly one owned input parameter and one owned result in v1, so
  the generator emits exactly two concrete structs, `Input` and `Output`.

Generated layout (deterministic, LF-only, every file ends with a trailing
newline):

```text
Cargo.toml         -- fixed template; zero dependencies; [lints.rust] unsafe_code = "deny"
build.rs           -- fixed template; links SPX_PG_PROVIDER_LIB_DIR/_NAME, compiles nothing itself
src/lib.rs          -- fixed template; #![deny(unsafe_code)]; re-exports the safe API
src/error.rs        -- fixed template; closed Error enum, sticky-failure secondary-release note
src/descriptor.rs   -- embeds TRUSTED_DESCRIPTOR_BYTES/TRUSTED_BINDING_BYTES; independent byte-exact verify()
src/types.rs        -- Input/Output structs, one Vec<u8> field per leaf, field order preserved
src/carrier.rs       -- encode/decode for exactly FIELD_COUNT leaves, independently validated
src/provider.rs      -- the FFI shim (unsafe confined here) and the safe Provider/diagnostics API
tests/round_trip.rs -- sample_input/assert_reversed/per-leaf-bound tests, generated per shape
```

`Cargo.lock` is not hand-rendered by the generator: the crate has zero
dependencies, so `cargo generate-lockfile` against the generated `Cargo.toml`
is itself deterministic, and the execution harness below runs it once and
then always builds `--locked`.

**Safety.** All `unsafe` is confined to `src/provider.rs`'s private `ffi`
submodule; every other generated file is denied `unsafe_code` twice over
(`lib.rs`'s own `#![deny(unsafe_code)]` and the package-wide
`[lints.rust] unsafe_code = "deny"`, which also covers `tests/round_trip.rs`).
Every `unsafe` block carries a `// SAFETY` comment stating pointer
provenance, length validity, the native contract relied upon, ownership
before/after the call, the null/output-init expectation, why the handle is
live, and who releases on failure. A generator test
(`unsafe_is_confined_to_the_provider_module`) mechanically rejects an
`unsafe` keyword appearing anywhere else, and a second test
(`every_unsafe_block_in_the_provider_module_is_preceded_by_a_safety_comment`)
requires every block to carry one. `Input`/`Output`/`Provider` derive neither
`Copy` nor `Clone` (`owned_types_never_derive_copy_or_clone`); `Provider`
derives only `Debug`.

**Ownership and settlement.** `Provider::open` independently replays
submitted descriptor/binding bytes against the embedded trusted values
(byte-exact equality) *before* calling the native adapter at all — a
consumer that only trusted the native side's own answer would defeat the
point of an independently verified descriptor
([issue #152](PUBLIC-GENERIC-DESCRIPTOR-V1.md)). `Provider::transform`
consumes `Input` by value (Rust's own move semantics make reuse through the
safe API impossible), transfers it exactly once, and releases the native
result handle in every case — success or an early `?` out of decode — via a
`Drop`-based guard. A release/close failure observed only during `Drop` is
printed as secondary evidence and never overwrites the primary `Err` already
selected: sticky failure, restated for this consumer.

**Execution evidence.** `tests/public_generic_native_adapter_v1/rust_calling_consumer.rs`
generates the crate, compiles the *same* rendered reference provider
`fixture.rs` exercises (issue #154's provider, never a second
implementation) into a static library, writes the generated crate to a
temporary directory outside this repository's own workspace, runs
`cargo generate-lockfile`, `cargo clippy --locked --all-targets -- -D
warnings`, and `cargo test --locked -- --test-threads=1` (single-threaded:
the linked provider's own allocator, handle registry, and failure-injection
state are process-global — `spx_pg_v1.h` states "no concurrency claim is
made anywhere in this file"). The generated crate's own seven tests all pass
against the real provider: a full success round trip with exact
reversed-byte assertions, a mutated descriptor, a mutated binding, and a
well-formed descriptor extended to name a different (still well-formed)
document, each rejected by `Provider::open` before any native allocation; the
per-leaf byte bound accepted exactly at 64 KiB and rejected one byte over;
and the full ordinal `0..=13` failure-injection matrix — the same matrix
`probe.c` drives from C — each asserting zero live native
allocations/handles afterward via the provider's own test-only counters. A
second test confirms the generated `Cargo.toml` declares no dependency at
all, so the crate never depends on this workspace's own `semaprax` crate.

**Known limitations, stated once.** Local evidence only: no hosted CI run is
recorded for this section, and this harness assumes a Unix-like host with
`clang`, `ar`, and `cargo` on `PATH` — Windows/MSVC is untried. The trusted
descriptor bytes are the same fixture placeholder `fixture.rs` uses (#119
still blocks deriving one from a real checked generic export). The generated
`rust-version = "1.88"` field states the minimum-toolchain claim; the
execution harness builds and runs the generated crate with whatever
`rustc`/`cargo` the host provides (1.98 locally), not a provisioned 1.88
toolchain, so it proves the crate is real, external, and executes against
the real provider, not that 1.88 itself builds it. The type model covers
flat owned-`Bytes` leaves only (see above); nested records and Copy scalars
are not yet generated. No maximum-total-payload (16 MiB) case is exercised,
only the per-leaf (64 KiB) bound — a narrower but still first-over-bound
proof.

## C11 calling consumer (issue #158)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for C11, and the future C++17 consumer
([issue #159](#nonclaims-metadata-consumers)) that wraps it.

Status: local, proof-only evidence only (no hosted CI run recorded for this
section), unsupported and unpublished. `semaprax::public_generic_consumer::c_calling`
is a separate generator from [`rust_calling`](#rust-calling-consumer-issue-156)
— it links against the native ABI's own real C symbols directly, never
through a Rust FFI restatement of them — but shares that generator's shape
types, field-naming scheme (lowercase hex of identity bytes), leaf-count
validation, and byte-array-literal convention, and links against — never
reimplements or modifies — the same native C11 physical adapter
([issue #154](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154)).

**What is generated versus a fixed template.** Every file below is produced
by `generate_c_calling_consumer(descriptor_bytes, binding, input, output)`, a
pure function from already-trusted descriptor/binding bytes and a
`RecordShape` to source text. Two honest qualifications, restating
[`rust_calling`]'s own:

- `spx_pg_calling_consumer.c`'s little-endian carrier codec, status-mapping
  functions, and the `spx_pg_consumer_open`/`_transform`/`_close` lifecycle
  logic are a **fixed template** baked into the generator, not derived
  per-descriptor — they restate `spx_pg_v1.h` (a frozen, versioned ABI), so
  nothing about a specific descriptor changes them.
- The concrete type model is scoped to exactly what the bound native
  provider implements today: a flat, descriptor-ordered sequence of owned
  `Bytes` leaves (see
  [Native C11 physical adapter](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154)'s
  own scope note). [Public Generic Boundary Profile
  v1](PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md) admits exactly one owned input
  parameter and one owned result in v1, so the generator emits exactly two
  concrete structs, `spx_pg_input` and `spx_pg_output`.

Generated layout (deterministic, LF-only, every file ends with a trailing
newline):

```text
spx_pg_v1.h                -- verbatim copy of the frozen native ABI header (native/template.rs::HEADER_V1)
spx_pg_calling_consumer.h  -- fixed template; the clean public surface; names no native ABI type
spx_pg_calling_consumer.c  -- fixed template + embedded trusted bytes/FIELD_COUNT/per-field statements
round_trip.c                -- fixed template + per-shape sample-input/assert-reversed/bound-target code
```

**Clean surface for #159.** `spx_pg_calling_consumer.h` depends on nothing
but `<stddef.h>`/`<stdint.h>` and never mentions a native ABI handle type
(`spx_pg_provider_v1` and friends stay entirely inside
`spx_pg_calling_consumer.c`, behind the opaque `spx_pg_calling_consumer`
handle) — a generator test
(`consumer_header_names_no_native_abi_type_and_only_two_includes`)
mechanically enforces both, and an execution-harness test
(`consumer_header_compiles_standalone_as_c11`) compiles the header alone as
C11. Issue #159's C++17 consumer is expected to `extern "C"`-include this
exact header and link against the compiled `spx_pg_calling_consumer.c`
rather than invent a second ABI.

**Ownership and settlement.** `spx_pg_consumer_open` independently replays
submitted descriptor/binding bytes against the embedded trusted values
(byte-exact equality) *before* calling the native adapter at all — the same
independent-verification requirement [`rust_calling`] documents (issue
#152). `spx_pg_consumer_transform` consumes `*input` by value: every leaf's
bytes are copied into an independent carrier buffer before any native
transfer ("input bytes are copied before native ownership transfer"), and
every leaf of `*input` is freed and zeroed before the call returns, success
or failure alike, so the caller cannot reuse or double-free it. `*out_output`
is zeroed before anything else happens and is populated only on
`SPX_PG_CONSUMER_OK`; every field stays exactly `{NULL,0}` on any
failure — "output parameters have deterministic failure values." The native
result handle this call opens internally is always released, by this call,
before it returns, whichever way it returns — "result remains opaque/owning
and is released explicitly," entirely inside the consumer, never exposed to
its caller. A release/close failure observed only as secondary cleanup is
printed to `stderr` and never changes the already-selected primary outcome:
sticky failure, restated for this consumer.

**Failure injection across one opaque call.** The native ABI arms
one-shot failure injection for the *next* native call only, and its trace
ordinals 0–4 fire inside `spx_pg_input_prepare_v1` while ordinals 5–13 fire
inside `spx_pg_call_v1` — two separate native calls
`spx_pg_consumer_transform` makes internally. `spx_pg_consumer_test_inject_failure`
therefore records the pending ordinal, and `spx_pg_consumer_transform`
re-arms it before the second native call exactly when the first one did not
already consume it (`status == SPX_PG_STATUS_OK`), restating
`tests/public_generic_native_adapter_v1/probe.c`'s own C-hosted pattern for
the same provider, hidden behind the single opaque `spx_pg_consumer_transform`
call an external caller actually sees.

**Execution evidence.** `tests/public_generic_native_adapter_v1/c_calling_consumer.rs`
generates the consumer, compiles the *same* rendered reference provider
`fixture.rs` exercises (issue #154's provider, never a second implementation)
into an object file, writes the generated files to a temporary directory
outside this repository's own workspace, and builds
`spx_pg_calling_consumer.c` + `round_trip.c` + the provider object into one
executable at both `-O0` and `-O2` with `-std=c11 -Wall -Wextra -Werror`,
asserting a warning-free build each time (unlike the Rust harness's separate
`cargo clippy` step, C's own `-Wall -Wextra -Werror` is the whole lint gate).
Each built executable is run and must print exactly
`c-calling-consumer-settled` with empty `stderr`. The generated
`round_trip.c` exercises: a full success round trip with exact
reversed-byte assertions using the native provider's own live-allocation
counter; a mutated descriptor, a mutated binding, and a well-formed
descriptor extended to name a different (still well-formed) document, each
rejected by `spx_pg_consumer_open` before any native allocation; the
per-leaf byte bound accepted exactly at 64 KiB and rejected one byte over;
and the full ordinal `0..=13` failure-injection matrix — the same matrix
`probe.c` drives from C for the provider itself — each asserting zero live
native allocations/handles afterward via the provider's own test-only
counters (`spx_pg_test_live_allocations_v1`/`spx_pg_test_live_handles_v1`,
reached only through this consumer's own `spx_pg_consumer_test_live_allocations`/
`_live_handles` wrappers) and a deterministically all-zeroed output. A
separate `#[ignore]`d test (`provisioned_c_calling_consumer_asan_ubsan`,
matching `fixture.rs`'s own convention) runs the same matrix under
`-fsanitize=address,undefined` when `SEMAPRAX_STRING_SANITIZER_CLANG` is
provisioned.

**Known limitations, stated once.** Local evidence only: no hosted CI run is
recorded for this section, and this harness assumes a Unix-like host with
`clang` (or `$CLANG`) and `ar` on `PATH` — Windows/MSVC is untried. The
trusted descriptor bytes are the same kind of fixture placeholder
`fixture.rs` and the Rust calling consumer use (#119 still blocks deriving
one from a real checked generic export). The type model covers flat
owned-`Bytes` leaves only (see above); nested records and Copy scalars are
not yet generated. No maximum-total-payload (16 MiB) case is exercised, only
the per-leaf (64 KiB) bound — a narrower but still first-over-bound proof,
matching the Rust calling consumer's own stated limitation. The provisioned
ASan/UBSan variant is `#[ignore]`d by default and was not run in this round;
only the plain `-O0`/`-O2` build and run is recorded as executed evidence
here.

## TypeScript/Wasm calling consumer (issue #157)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for TypeScript/Wasm.

Status: local, proof-only evidence only (no hosted CI run recorded for this
section), unsupported and unpublished. Extends the existing metadata
generator (`semaprax::public_generic_consumer::typescript_calling`, a new
sibling of `rust_calling`) rather than a parallel framework: it reuses
`rust_calling`'s own `RecordShape`/`OwnedByteField`/`ShapeError` types and
the metadata consumer's identifier scheme (lowercase hex of identity bytes),
and it targets — never reimplements or modifies — the Core Wasm physical
adapter ([issue #155](PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)).

**A load-bearing honest limitation, stated once here.** Unlike the Rust
calling consumer above, which links against a genuinely *compiled* native
provider artifact (`native/provider_body.c`, built into a real static
library), issue #155's Core Wasm physical adapter
(`public_generic_abi::wasm::provider::WasmProvider`) has never been compiled
to an actual `.wasm` binary exposing an
open/input_prepare/call/result_export/release ABI a JS host could
`WebAssembly.instantiate` and call — it is a Rust struct exercised only
in-process by Rust test code (`wasm/provider/tests.rs`). No
`#[no_mangle] extern "C"` export and no compiled Wasm artifact for this
protocol exists anywhere in this repository. The one genuinely real, named
Wasm export issue #155 *does* define is
`FIXTURE_ENDPOINT_EXPORT_NAME` (`"spx_pg_wasm_endpoint_reverse_bytes_v1"`),
previously proven only by `reverse_probe.mjs`'s narrower, handle-free
memory-primitive script (real `WebAssembly.Memory`, with the reversal itself
done in JavaScript, not in compiled Wasm bytecode).

This generator's `wasm-provider.ts` therefore keeps the allocator, handle
registry, and call-lifecycle state **host-side, in generated TypeScript** —
a bounded exact-LIFO bump allocator and a private, `#`-branded handle
registry, mirroring `reverse_probe.mjs`'s own host-owned bookkeeping over
real `WebAssembly.Memory` rather than sharing any code with
`WasmProvider` — and calls into real, genuinely compiled Wasm bytecode only
for that one already-named endpoint export, operating in place on real
linear memory this wrapper allocates, writes, and later zeroes on release.
It never touches `src/public_generic_abi/wasm/**`. This is real Wasm
execution with owned copy-out, but it does not (and cannot yet) prove a
full compiled-provider open/call/close ABI, because no such compiled
artifact exists. See `src/public_generic_consumer/typescript_calling.rs`'s
own module documentation for the identical accounting in the generator's
own doc comments.

**What is generated versus hand-written.** Every file below is produced by
`generate_typescript_calling_consumer(descriptor_bytes, binding, input,
output)`, a pure function from already-trusted descriptor/`WasmProviderBindingV1`
bytes and a `RecordShape` to source text:

```text
package.json          -- fixed template; one devDependency, typescript 5.8.3 (repo-pinned)
package-lock.json     -- fixed template; the same pinned integrity hash this repo already uses
tsconfig.json         -- fixed template; strict mode, ES2023+DOM lib (for WebAssembly/Web Crypto types)
src/errors.ts         -- fixed template; closed discriminated error union, sticky-failure secondary note
src/descriptor.ts     -- embeds TRUSTED_DESCRIPTOR_BYTES/TRUSTED_BINDING_BYTES/endpoint name/artifact digest; independent byte-exact verify() and Web-Crypto module-artifact-digest verification
src/types.ts          -- Input/Output interfaces, one readonly Uint8Array field per leaf, field order preserved
src/carrier.ts         -- Logical Carrier v1 codec for exactly FIELD_COUNT leaves, BigInt-exact leaf-count/length handling
src/wasm-provider.ts   -- fixed template; the host-owned allocator/registry/lifecycle wrapper and the safe Provider API
src/index.ts           -- fixed template; re-exports Provider, Input/Output, and the error model only
test/round-trip.mjs    -- sampleInput/assertReversed/per-leaf-bound/failure-matrix tests, generated per shape
```

The type model is scoped identically to the Rust calling consumer's own
scope note: a flat, descriptor-ordered sequence of owned `Uint8Array`
leaves (the boundary profile admits exactly one owned input parameter and
one owned result in v1), so this generator emits exactly two concrete
interfaces, `Input` and `Output`. A Copy-scalar or nested-record leaf is
future generator work tracked by the same #119 prerequisite.

**Exact integers.** `leaf_count` and every per-leaf `len` in Logical Carrier
v1's wire framing are `u64` fields. JavaScript `number` silently rounds
anything past `Number.MAX_SAFE_INTEGER` (2^53), so `src/carrier.ts` reads
and compares every wire integer as `bigint` before it is ever narrowed to a
`Number` for a length or an allocation — a hostile carrier claiming a length
at `u64::MAX` is rejected by an exact `bigint` comparison against the
64 KiB per-leaf bound, proven by a dedicated generated test
(`"exact-integer carrier decoding rejects a hostile u64::MAX leaf length"`)
and by one at exactly one byte over the bound. The concrete generic record
shape this round admits carries no user-facing `i64`/`i32`/`u8`/`char`/`f32`
scalar leaf yet (the same #119-deferred boundary the Rust consumer states),
so this is the exact-integer discipline the current wire format has to
prove; range-checking those scalar domains is future generator work once
#119 unblocks them, not a gap invented here.

**Ownership and settlement.** `Provider.open` independently replays
submitted descriptor/binding bytes against the embedded trusted values
(byte-exact equality), and independently recomputes a domain-separated
SHA-256 digest of the supplied `.wasm` module bytes against the embedded
trusted `provider_artifact_digest` (via `globalThis.crypto.subtle`, no
`node:crypto`/`@types/node` dependency) — both *before* any Wasm memory
allocation. This digest check is a real, content-based module-identity
check, not merely export-name or successful-instantiation trust: a
byte-mutated (but still export-name-identical) module is rejected. Raw
numeric handles are never exported: `OpaqueHandle` has a private
constructor and a private `#tag` field, so a cast through `unknown` cannot
forge or reuse one (`instanceof` still requires a real instance of the
class). `Provider.transform` stages input, calls, exports, and releases in
one method with `try`/`finally` cleanup; a `Provider.diagnostics` test-only
static surface exposes the same private steps to `test/round-trip.mjs` for
stale/foreign-handle and failure-injection exercises, without a second,
parallel implementation.

**Execution evidence.**
`tests/public_generic_wasm_adapter_v1/typescript_calling_consumer.rs`
generates the package, writes it to a temporary directory outside this
repository's own workspace, type-checks it with the repository-pinned `tsc`
(5.8.3, the same pin `platform-tests/wasm-scalar-browser-v1/package.json`
already uses), and runs its own `test/round-trip.mjs` under real Node
against a real, genuinely compiled `.wasm` module
(`tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs`) that hand-
assembles the WebAssembly binary format directly (no `wat2wasm`/`wasm-tools`
tool or `wat` crate is available in this environment) exporting real linear
memory and a real compiled byte-reversal function under exactly
`FIXTURE_ENDPOINT_EXPORT_NAME`. All 14 of the generated package's own tests
pass against that real module: a full success round trip with exact
reversed-byte assertions and proof the result is a fresh copy (mutating the
original input buffer after the call does not change the already-returned
result); a mutated descriptor, a mutated binding, a well-formed descriptor
naming a different document, and a module whose bytes do not replay the
trusted artifact digest, each rejected before or without any Wasm
allocation; a stale/foreign handle and a released-then-reused handle (via a
cast through `unknown`), each rejected; call-after-close; memory growth
across a real page boundary that does not invalidate result decoding; the
two exact-integer hostile-length tests; the per-leaf byte bound accepted
exactly at 64 KiB and rejected one byte over; and the full failure-injection
ordinal `0..=7` matrix (this wrapper's own host-owned pipeline stages) —
each asserting zero live allocations/handles afterward via
`Provider.diagnostics`. A second test confirms the generated `package.json`
declares no runtime dependency at all beyond the pinned `typescript`
devDependency.

**Known limitations, stated once.** Local evidence only: no hosted CI run is
recorded for this section, and no browser/Chromium fixture is exercised —
"at minimum exercise the current Node/Wasm route" is met; the browser route
is deferred, not claimed. This harness is gated on `node` and a
repository-pinned (5.8.3) `tsc` being present on `PATH` or at a known pnpm
install location; it skips (never fails) on a host without either. The
trusted descriptor bytes are the same kind of fixture placeholder the Rust
section's harness uses (#119 still blocks deriving one from a real checked
generic export). Most importantly: this proves the generated consumer's own
real execution, exact-copy-out, exact-integer carrier decoding, and exact
settlement against real Wasm bytecode; it does not prove a compiled
open/input_prepare/call/result_export/release Wasm provider ABI, because
issue #155 has not shipped one — see the load-bearing limitation above. The
`reference_wasm_module` fixture this harness compiles is a test-only stand-in
for that missing artifact, not a claim that #155's protocol has been
compiled to Wasm.
