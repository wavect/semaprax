# Public Generic Metadata Consumers v1

In plain terms: this defines what each generated consumer must independently check.

Audience: generated-consumer integrators, ABI reviewers, and promotion
reviewers.

Status: the four metadata consumers are an implemented bounded generator,
hosted green on Linux, macOS, and Windows with all four toolchains exercised.
This closes the *grammar* half of gates PG-5 and PG-6 of the
[Public Generic Ownership milestone](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md).
The *calling* half now has a first, local-only implementation for Rust (see
[Rust calling consumer (issue #156)](#rust-calling-consumer-issue-156)
below), for C11 (see
[C11 calling consumer (issue #158)](#c11-calling-consumer-issue-158) below),
for TypeScript/Wasm (see [TypeScript/Wasm calling consumer (issue
#157)](#typescriptwasm-calling-consumer-issue-157) below), and for C++17
(see [C++17 calling consumer (issue
#159)](#c17-calling-consumer-issue-159) below), built on the versioned
descriptor ([issue #152](PUBLIC-GENERIC-DESCRIPTOR-V1.md)) and the native
and Core Wasm physical carrier adapters
([issue #154](PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154),
[issue #155](PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155))
that did not exist when the metadata half closed; both gates stay open for
hosted evidence of all four languages. Public generic ownership remains
unsupported and unpublished.

The TypeScript package emits descriptor-bound Logical Carrier v1 frames and
drives the compiler-owned zero-import Wasm provider through its full lifecycle.
Earlier stand-in and flat-frame text describes the separately selected reference
corpus lane, not a fallback. Shared-corpus migration, broader endpoints,
browser/hosted evidence, and publication remain open.

The four calling-consumer generators retain their byte-oriented configuration
API: generation embeds the supplied trusted descriptor bytes without granting
them authority. At **open**, Rust, C11, and TypeScript/Wasm independently check
that the submitted descriptor is exactly twelve bounded UTF-8 Descriptor-v1
frames with the frozen descriptor, boundary-profile, and type-grammar versions,
then independently parse the embedded trusted descriptor and compare all twelve
contracted fields in order before any provider call or Wasm instantiation.
C++17 uses the C11 check. Because fixed-width framing and every field are
covered, this structured replay preserves byte-exact calling-layer admission
without treating the descriptor as one opaque blob. A generated
consumer configured with malformed trusted bytes therefore cannot open, even
when a caller submits those same bytes. This calling-layer pairing is stricter
than the reference descriptor identity replay: a presentation-name-only
change can preserve reference identity but fails byte-exact calling pairing.
These checks validate wire framing and versions; they do not recompute
ProgramRoot, source, export, or instance digests from checked semantics.
The provider ABI's existing byte-pairing semantics are unchanged.

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

The widened calling-consumer corpus also passed at the job level on all three
hosted operating systems in [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593),
at prior commit `7def8fb1…` (with the follow-up run at `3548dc9a…` recorded in
the release-candidate evidence). Neither run is an exact-current-head run;
fresh hosted evidence at the current checkout remains pending.

## Nonclaims (metadata consumers)

The four metadata consumers described above define no calling convention,
descriptor, carrier, package, layout, allocation, or ownership transfer, and
no value crosses any boundary through them. Generation does not admit a
public generic signature: the public projections still reject generic
surfaces, and the milestone's separation gate continues to prove it. The
hosted run recorded above covers this corpus and nothing else: it is not a
support decision and not a publication. Calling a public generic export over
a versioned descriptor and carrier is a separate generator, described next
for Rust, C11, TypeScript/Wasm, and C++17; hosted evidence now exists for all
four (corrected 2026-09-19: run 35433295593 at `7def8fb1…`, not yet a #164
frozen candidate — see each section's own Status line), and
none of them admits a public generic signature either, which is why PG-5
and PG-6 stay open despite the sections below.

## Rust calling consumer (issue #156)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for Rust.

Status: unsupported and unpublished. **Revised 2026-09-19** (issue #164
audit): hosted evidence now exists for this section's own harness — [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, all three
`public-generic-ownership-milestone` OS jobs `success` (see
PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md's PG-8 note) — not yet a #164
frozen candidate. Extends the existing generator
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
src/descriptor.rs   -- embeds TRUSTED_DESCRIPTOR_BYTES/TRUSTED_BINDING_BYTES; bounded twelve-field parse/replay
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
(descriptor fields independently parsed and compared; binding byte-exact)
*before* calling the native adapter at all — a
consumer that only trusted the native side's own answer would defeat the
point of an independently verified descriptor
([issue #152](PUBLIC-GENERIC-DESCRIPTOR-V1.md)). `Provider::transform`
consumes `Input` by value (Rust's own move semantics make reuse through the
safe API impossible), transfers it exactly once, and releases the native
result handle explicitly before returning any decoded output. `ResultGuard`
retains a Drop fallback. `Error::ReleaseFailed` suppresses success when explicit
release fails; `last_release_status()` preserves that status without replacing
an earlier export/decode error. `close()` reports refusal and retains a live
owner for retry. Destructor diagnostics are best-effort writes, not panic-prone
printing. The [consumer settlement continuation](PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#native-calling-consumer-continuation-issue-162)
owns the new bounded-codec and execution gates; its Rust execution is a
separate required check, not implied by the historical round-trip evidence.

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

**Known limitations, stated once.** (Corrected 2026-09-19: hosted evidence
now exists, see the Status line above.) This harness assumes a Unix-like host
with `clang`, `ar`, and `cargo` on `PATH`; hosted CI resolves `clang` via LLVM
on `windows-latest` too (confirmed in run 35433295593), so "untried on
Windows" no longer holds, though the toolchain there is clang, not MSVC. The trusted
descriptor bytes are a canonical encoded Descriptor-v1 test fixture, not
derived from a real checked generic export. The generated
`rust-version = "1.88"` field states the minimum-toolchain claim; the
execution harness builds and runs the generated crate with whatever
`rustc`/`cargo` the host provides (1.98 locally), not a provisioned 1.88
toolchain, so it proves the crate is real, external, and executes against
the real provider, not that 1.88 itself builds it. The type model covers
flat owned-`Bytes` leaves only (see above); nested records and Copy scalars
are not yet generated. The dedicated
`max_bounds_saturation.rs` harness now drives this generated Rust consumer
through exactly 256 leaves × 64 KiB = 16 MiB against the real compiled native
provider, then re-runs the same shape with only the generated consumer's
total budget reduced by one byte. The latter refuses before provider
allocation; this is separate from, and deliberately not duplicated by, this
consumer's historical per-leaf round-trip case.

## C11 calling consumer (issue #158)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for C11, and the C++17 consumer ([C++17 calling
consumer (issue #159)](#c17-calling-consumer-issue-159)) that wraps it.

Status: unsupported and unpublished. **Revised 2026-09-19** (issue #164
audit): hosted evidence now exists for this section's own harness — [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, all three
`public-generic-ownership-milestone` OS jobs `success` (see
PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md's PG-8 note) — not yet a #164
frozen candidate. `semaprax::public_generic_consumer::c_calling`
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
(descriptor fields independently parsed and compared; binding byte-exact)
*before* calling the native adapter at all — the same
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
its caller. The additive `transform_with_settlement` reports primary, native
and explicit-release status separately. A release failure suppresses a decoded
success with status 9 (`RELEASE_FAILED`), while an earlier failure stays primary.
`close_checked` preserves a live owner on a retriable refusal; legacy void close
still delegates with a best-effort note. Length preflight, complete decode
validation before allocation, reverse cleanup and exact source-pointer
preconditions are specified in the [consumer continuation](PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#native-calling-consumer-continuation-issue-162).

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

**Known limitations, stated once.** (Corrected 2026-09-19: hosted evidence
now exists, see the Status line above.) This harness assumes a Unix-like host
with `clang` (or `$CLANG`) and `ar` on `PATH`; hosted CI resolves `clang` via
LLVM on `windows-latest` too (confirmed in run 35433295593), so "untried on
Windows" no longer holds, though the toolchain there is clang, not MSVC. The
trusted descriptor bytes are a canonical encoded Descriptor-v1 test fixture,
not derived from a real checked generic export. The type model covers flat
owned-`Bytes` leaves only (see above); nested records and Copy scalars are
not yet generated. The dedicated `max_bounds_saturation.rs` harness drives
this C11 consumer through the exact 16 MiB aggregate against the real compiled
native provider at both `-O0` and `-O2`, and proves pre-dispatch refusal by
changing only the generated total budget to one byte below that aggregate. The
historical per-leaf case remains a distinct, narrower codec control. The
provisioned ASan/UBSan variant is `#[ignore]`d by default and was not run in
this round; only the plain `-O0`/`-O2` build and run is recorded as executed
evidence here.

## TypeScript/Wasm calling consumer (issue #157)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for TypeScript/Wasm.

Status: unsupported and unpublished. **Revised 2026-09-19** (issue #164
audit): hosted evidence now exists for this section's own harness — [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, all three
`public-generic-ownership-milestone` OS jobs `success` (see
PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md's PG-8 note) — not yet a #164
frozen candidate. Extends the existing metadata
generator (`semaprax::public_generic_consumer::typescript_calling`, a new
sibling of `rust_calling`) rather than a parallel framework: it reuses
`rust_calling`'s own `RecordShape`/`OwnedByteField`/`ShapeError` types and
the metadata consumer's identifier scheme (lowercase hex of identity bytes),
and it targets — never reimplements or modifies — the Core Wasm physical
adapter ([issue #155](PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)).

**Two routes, one generated file, updated for #229/#287.** Issue #155's own
Core Wasm physical adapter (`public_generic_abi::wasm::provider::WasmProvider`)
is still exactly what it always was: a Rust struct exercised only in-process
by Rust test code (`wasm/provider/tests.rs`), never compiled to a `.wasm`
binary. That specific struct is not the compiled provider described below,
and this document does not claim otherwise. Separately, issues #229/#287
shipped a genuinely different, genuinely *compiled* closed Core Wasm
artifact (`src/wasm/public_generic_provider`, emitted by
`emit_public_generic_wasm_provider_v1`) that does expose a real
open/input_prepare/call/result_export/value_release/result_release/
provider_close ABI with zero ambient imports, and this SAME generated
`wasm-provider.ts` already has a second, internal (not exported from the
generated package's public surface) class, `CompiledProvider`, that
delegates its entire lifecycle to those exports — canonical Logical Carrier
v1 frame encode/decode included — rather than keeping allocation, handles or
dispatch host-side. `Provider.open()` returns whichever route the binding
selects; only `Provider`'s own public shape (`transform`/`close`/
`settlement`) is exposed either way.

The host-owned `Provider` class this section otherwise describes remains
exactly as before: its allocator, handle registry, and call-lifecycle state
stay **host-side, in generated TypeScript** — a bounded exact-LIFO bump
allocator and a private, `#`-branded handle registry, mirroring
`reverse_probe.mjs`'s own host-owned bookkeeping over real
`WebAssembly.Memory` rather than sharing any code with `WasmProvider` — and
it calls into real, genuinely compiled Wasm bytecode only for the one
already-named fixture endpoint export, operating in place on real linear
memory this wrapper allocates, writes, and later zeroes on release. It never
touches `src/public_generic_abi/wasm/**`, and it remains the explicitly
selected route for the reference/fixture and settlement corpora described
below — `WasmProviderBindingV1::exported_endpoint_export_name` selects
between the two classes; there is no fallback between them. See
`src/public_generic_consumer/typescript_calling.rs`'s own module
documentation, and
[PUBLIC-GENERIC-WASM-PROVIDER-TARGET-V1.md](PUBLIC-GENERIC-WASM-PROVIDER-TARGET-V1.md),
for the compiled route's own accounting; `tests/public_generic_wasm_adapter_v1/compiler_provider_artifact.rs`
is the compiled route's real-execution and hostile-lifecycle evidence, not
this fixture-only harness.

**What is generated versus hand-written.** Every file below is produced by
`generate_typescript_calling_consumer(descriptor_bytes, binding, input,
output)`, a pure function from already-trusted descriptor/`WasmProviderBindingV1`
bytes and a `RecordShape` to source text:

```text
package.json          -- fixed template; one devDependency, typescript 5.8.3 (repo-pinned)
package-lock.json     -- fixed template; the same pinned integrity hash this repo already uses
tsconfig.json         -- fixed template; strict mode, ES2023+DOM lib (for WebAssembly/Web Crypto types)
src/errors.ts         -- fixed template; closed discriminated error union and retained secondary cleanup statuses
src/descriptor.ts     -- embeds trusted descriptor/binding bytes; bounded twelve-field descriptor replay plus Web-Crypto module-artifact verification
src/types.ts          -- Input/Output interfaces, one readonly Uint8Array field per leaf, field order preserved
src/carrier.ts         -- Logical Carrier v1 codec for exactly FIELD_COUNT leaves, BigInt-exact leaf-count/length handling
src/wasm-provider.ts   -- fixed template; the host-owned allocator/registry/lifecycle wrapper (`Provider`) plus the compiled-provider ABI wrapper (`CompiledProvider`, #229/#287), selected by the binding's endpoint export name
src/index.ts           -- fixed template; re-exports Provider, Input/Output, errors and settlement report types
test/round-trip.mjs    -- sampleInput/assertReversed/per-leaf-bound/failure-matrix tests, generated per shape
```

The type model is scoped identically to the Rust calling consumer's own
scope note: a flat, descriptor-ordered sequence of owned `Uint8Array`
leaves (the boundary profile admits exactly one owned input parameter and
one owned result in v1), so this generator emits exactly two concrete
interfaces, `Input` and `Output`. A Copy-scalar or nested-record leaf remains
future public-generic generator work; #119 closed an internal owned-record
execution profile and does not itself widen this public calling surface.

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
scalar leaf yet, so this is the exact-integer discipline the current wire
format has to prove; range-checking those scalar domains is future
public-generic generator work, not a gap invented here.

**Ownership and settlement (issue #162 continuation).** `Provider.open`
requires module bytes: a precompiled `WebAssembly.Module` receives the closed
`module-bytes-required` refusal, since its original bytes cannot be recovered
for digest verification. The exact supplied BufferSource range is copied before
the first `await`, then hashed, compiled, checked for the exact two-export/no-import
surface, and instantiated. Shared, resizable and detached ingress buffers are
refused. Exported descriptor/binding byte arrays are informational copies;
private captured bytes remain the replay authority even if those public arrays
are mutated. Module bytes are bounded to 16 MiB, including custom sections.

The reference shape admits 1–256 owned byte leaves. A leaf is at most 65,536
bytes, total payload at most 16,777,216 bytes, and the complete carrier at most
16,779,272 bytes. The host arena therefore allows **257 pages**, not 256: the
extra page is necessary for count/length framing at the exact payload maximum.
The original `reference_wasm_module::build()` bytes remain unchanged at 256
pages; the settlement fixture explicitly selects `build_with_max_pages(257)`.
Module maximum-memory refusal is tested separately with a one-page fixture.

Exactly one call may be in flight per provider. A competing prepare/transform
is refused before allocation and cannot release the existing owner's frame.
Handles use intrinsic private-field branding, a private provider identity,
input/result kind and a monotonic generation. Generation `0xffffffff` is the
last admitted value; reuse/exhaustion refuses without resurrecting an old handle.
Close with a live input/result refuses while preserving its owner for explicit
settlement and retry. No cancellation, concurrent calls, or endpoint retry is
introduced. Non-consuming result export may be retried without re-execution.

Input records require exact own data fields; getters are not invoked. Byte
views are validated through typed-array intrinsics rather than user-overridden
methods or properties. All result framing, including the last leaf, is checked
before copying any decoded payload. The entire result remains private until
commit. `transform` explicitly releases the result before returning success:
a release-only error is primary, while a prior execution/export/decode error
remains primary and retains release errors in `secondaryCleanupStatuses` and
`Provider.settlement()`. Injected release failure is reported only **after**
physical zeroing and invalidation, so it cannot create a deliberate leak.

Reports include observed endpoint calls, live/peak frame bytes and allocations,
active input/result handles, zeroed bytes, retained memory pages, host event
sequence and frame release order. These are **host-owned frame observations**,
not a fabricated compiler leaf cleanup trace. JavaScript heap reclamation and
provider object GC are not measured; zero live spans does not claim zero retained
Wasm pages. At most 4,096 trace rows are admitted, with two reserved for cleanup;
ordinary operations refuse at 4,094 rather than silently truncating evidence.

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
devDependency. A separate generated-consumer execution pair saturates the
aggregate bound with 256 owned 64 KiB leaves (exactly 16 MiB) through input
encoding, the test-only Wasm endpoint, result decoding, and copy-out; mutating
only the generated total budget to one byte less refuses that same input before
endpoint dispatch or a live allocation. This is consumer evidence over the
fixture endpoint and the host-owned `Provider` route, not evidence for the
`CompiledProvider` route's own ABI (see the two-routes note above). The
TypeScript aggregate pair was added after the recorded hosted milestone run,
so it is current local evidence rather than a backdated hosted claim.

**Known limitations, stated once.** (Corrected 2026-09-19: hosted evidence
now exists, see the Status line above.) No browser/Chromium fixture is
exercised —
"at minimum exercise the current Node/Wasm route" is met; the browser route
is deferred, not claimed. The older package harness skips when its toolchains are missing. The new
[settlement gate](PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#typescript-host-owned-caller-continuation-issue-162)
requires Node 22 and the pinned TypeScript 5.8.3 compiler and fails on absence;
its Unix Cargo bridge compares all ten files from the actual Rust generator
before executing them. That bridge must be run before claiming generator proof. The
trusted descriptor bytes are a canonical encoded Descriptor-v1 test fixture,
not derived from a real checked generic export. Most importantly: this proves the generated consumer's own
real execution, exact-copy-out, exact-integer carrier decoding, and exact
settlement against real Wasm bytecode; this specific harness targets only
the fixture endpoint and the host-owned `Provider` route, so it does not by
itself prove the compiled open/input_prepare/call/result_export/release Wasm
provider ABI issues #229/#287 shipped separately (see the two-routes note
above and `compiler_provider_artifact.rs` for that route's own evidence).
Issue #155's own `WasmProvider` struct remains uncompiled, as stated above.
The `reference_wasm_module` fixture this harness compiles is, and remains, a
test-only stand-in used deliberately by this fixture/settlement lane, not a
claim that #155's protocol has been compiled to Wasm.

## C++17 calling consumer (issue #159)

Audience: generated-consumer integrators and ABI reviewers evaluating the
calling half of PG-5/PG-6 for C++17, and anyone checking that the C++
wrapper genuinely wraps rather than reimplements the C11 client.

Status: unsupported and unpublished. **Revised 2026-09-19** (issue #164
audit): hosted evidence now exists for this section's own harness — [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, all three
`public-generic-ownership-milestone` OS jobs `success` (see
PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md's PG-8 note) — not yet a #164
frozen candidate.
`semaprax::public_generic_consumer::cxx_calling` is a thin generator on top
of [`c_calling`](#c11-calling-consumer-issue-158): it calls
`c_calling::generate_c_calling_consumer` for the exact same arguments and
reuses every one of its four files byte-for-byte (a dedicated test,
`reuses_the_c_calling_consumer_files_byte_for_byte`, proves this), adding
exactly two new files. It never restates the carrier codec, the
open/transform/close lifecycle, or the descriptor/binding pairing check —
the generated C++17 header `#include`s the generated C11 header and calls
only its declared functions.

**What is generated versus a fixed template.** Every file below is produced
by `generate_cxx_calling_consumer(descriptor_bytes, binding, input, output)`,
a pure function from already-trusted descriptor/binding bytes and a
`RecordShape` to source text:

```text
spx_pg_v1.h                              -- verbatim, from c_calling
spx_pg_calling_consumer.h                -- verbatim, from c_calling
spx_pg_calling_consumer.c                -- verbatim, from c_calling
round_trip.c                              -- verbatim, from c_calling (unused by this consumer's own harness, kept for parity with the reused file set)
include/semaprax_public_generic_v1.hpp   -- fixed template + descriptor-derived Input/Output fields and per-field transform plumbing
test/round_trip.cpp                       -- fixed template + descriptor-derived sample-input/assert-reversed/per-leaf-bound code
```

The wrapper header is header-only (every function is `inline` or defined
in-class): it declares no separate translation unit of its own, so it can
be `#include`d from more than one C++ file, or twice in one file, without an
One Definition Rule violation — a dedicated harness test,
`wrapper_header_compiles_standalone_as_cxx17_and_tolerates_double_inclusion`,
compiles it alone, twice, as C++17.

**Move-only is the crux, not decoration.** `Provider` (owns one opened
`spx_pg_calling_consumer` handle) and `Output` (owns one populated
`spx_pg_output` value) both:

- delete their copy constructor and copy assignment operator, proven by
  `static_assert(!std::is_copy_constructible_v<T>)` /
  `static_assert(!std::is_copy_assignable_v<T>)` inside the generated header
  itself (not merely in this generator's own test suite) and restated again
  in the generated `test/round_trip.cpp`;
- are `noexcept` move-constructible and move-assignable
  (`static_assert(std::is_nothrow_move_constructible_v<T>)` /
  `..._assignable_v<T>`), leaving the moved-from object in a valid,
  releasable, non-owning state (a null `spx_pg_calling_consumer*` for
  `Provider`, a zero-valued `spx_pg_output{}` for `Output`);
- release their sole owned resource at most once, via a private idempotent
  `reset()` (`Provider`) or an unconditional `spx_pg_output_free(&raw_)`
  (`Output`, itself idempotent on an already-zeroed value), called from a
  `noexcept` destructor, from move-assignment before taking the new handle,
  and from an explicit public `close()` a caller may call before the
  destructor runs;
- guard self-move-assignment explicitly (`if (this != &other)`), and keep
  their raw member private with no public constructor accepting it.

`generated_cxx_calling_consumer_executes_against_the_real_native_provider`
in `tests/public_generic_native_adapter_v1/cxx_calling_consumer.rs` runs the
generated `test/round_trip.cpp` against the real native provider and
exercises: a full success round trip; moving a `Provider` and continuing to
use the move target; moving an `Output`; move-assigning over a live
`Output`; a moved-from `Provider` and a moved-from `Output` each destructing
cleanly (the latter releasing nothing, since its leaves already transferred
to the move target); self-move-assignment on both types (through a pointer
alias, so `-Wself-move` never fires on a genuine runtime self-assignment);
an explicit `close()` followed by the destructor; early return from a helper
after a live `Output` was already constructed; relocating a
`std::vector<Output>` past its capacity; an explicit independent copy of a
view's bytes that survives the source `Output`'s later mutation (never
sharing storage); zero-length and embedded-zero-byte leaves; the exact
per-leaf byte bound (64 KiB) accepted and rejected one byte over; hostile
pairing (a mutated descriptor, a mutated binding, and a well-formed
descriptor extended to name a different document), each rejected by
`Provider::open` before any native allocation with the exact typed
`ErrorKind` the generated C11 status maps to; and the full ordinal `0..=13`
failure-injection matrix.

**Zero-leak evidence, and whose counters it is.** Every terminal case above
asserts `spx_pg_consumer_test_live_allocations() == 0` (and, inside the
matrix, `spx_pg_consumer_test_live_handles`) — the *native provider's own*
test-only counters, reached only through the wrapped C11 consumer's own
`spx_pg_consumer_test_live_allocations`/`_live_handles` accessors this C++
header calls verbatim. This wrapper keeps no allocation or handle count of
its own: it proves nothing about itself that the underlying, already-proven
C11/native layers do not already guarantee, and the RAII correctness above
is never inferred merely because one round trip succeeded — the compile-time
static assertions and the explicit move/self-move/moved-from/early-return
tests are required, independent evidence.

**Error model.** `ErrorKind` restates `spx_pg_consumer_status`'s closed
vocabulary exhaustively (`map_error_kind` is a total function over the C11
enum, defaulting unreached values to `ExecutionFailed` rather than undefined
behavior) and extends it with `AllocationFailure`/`NullArgument` so every
non-OK C11 status has an exact typed home; `Error` carries both the mapped
`ErrorKind` and the raw native status an `ExecutionFailed` restates, never a
parsed string. `Result<T>` is this generator's own closed, exception-free
sum type (C++17 has no `std::expected`): a `std::variant<T, Error>` wrapped
behind `has_value()`/`value()`/`error()`, never exposing the discriminant as
anything else. `ReleaseFailed` is now reachable through the additive C
settlement and checked-close APIs. `Error::release_status()` retains secondary
explicit-release evidence; `Provider::close_checked()` returns a typed status.
Move assignment retains both owners when closing the target is refused. No C++ exception crosses the C11 boundary in either direction: every
wrapper method is `noexcept`, and every call into the C11 surface is a plain
C function call.

**Input/output model.** `Input` is an ordinary aggregate of
`std::vector<std::uint8_t>` leaves (one per descriptor field, in descriptor
order), consumed by value so the ownership transfer is visible at the call
site; it is not itself RAII-critical, since a `Provider` that never receives
it (a moved-from or default-constructed `Provider`) never touches its
vectors at all and lets them destruct normally. `Output` exposes one
`BytesView` accessor per leaf — a small pointer/length view (no
`std::span`, a C++20 facility) valid only while the `Output` remains live
and unmoved; `to_owned(BytesView)` returns an explicit, independent copy
when a caller needs the bytes to outlive the `Output`. Scope, restating
[`c_calling`](#c11-calling-consumer-issue-158)'s own: the bound native
provider implements only a flat, descriptor-ordered sequence of owned-bytes
leaves, so `Input`/`Output` carry no scalar or nested-record member yet. The
dedicated `max_bounds_saturation.rs` harness drives this C++17 facade through
the exact 16 MiB aggregate against the real compiled native provider, and a
one-byte-smaller generated total budget refuses that identical input before
provider allocation. Its historical per-leaf (64 KiB) case remains an
independent, narrower codec control.

**Generated name safety.** Every field name is `field_<hex-identity>` —
the exact scheme [`c_calling`](#c11-calling-consumer-issue-158) and
[`rust_calling`](#rust-calling-consumer-issue-156) already use, reused
rather than reinvented — so a field name can never collide with a C++
keyword (the fixed `field_` prefix is never itself a keyword) and is
injective in the field's declaration-identity bytes, never its display
text. The wrapper types themselves (`Provider`, `Output`, `Input`, `Error`,
`ErrorKind`, `Result`) are fixed names inside an explicit, versioned
namespace (`semaprax::public_generic::v1`), and the include guard
(`SEMAPRAX_PUBLIC_GENERIC_CONSUMER_V1_HPP`) is a stable literal tied to that
same contract identity, not derived from any one descriptor.

**Known limitations, stated once.** (Corrected 2026-09-19: hosted evidence
now exists, see the Status line above.) This harness assumes a Unix-like host
with `clang`/`clang++` (or `$CLANG`/`$CLANGXX`) on `PATH`; hosted CI resolves
`clang`/`clang++` via LLVM on `windows-latest` too (confirmed in run
35433295593), so "untried on Windows" no longer holds, matching every other
native-adapter harness in this document — the toolchain there is clang, not
MSVC. The
trusted descriptor bytes are a canonical encoded Descriptor-v1 test fixture,
not derived from a real checked generic export. The type model covers
flat owned-`Bytes` leaves only (see above); nested records and Copy scalars
are not yet generated (#119). The original round-trip harness did not cover
16 MiB; the new [consumer continuation](PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#native-calling-consumer-continuation-issue-162)
adds that exact bound and explicit `ReleaseFailed`/checked-close propagation.
The provisioned
ASan/UBSan variant
(`provisioned_cxx_calling_consumer_asan_ubsan`) is `#[ignore]`d by default
and was not run in this round; only the plain `-O0`/`-O2` build and run is
recorded as executed evidence here.

## Shared hostile corpus (issue #160)

Audience: reviewers checking whether "all four consumers reject the same
malformed input" is genuinely cross-checked, or merely four independent
hand-written approximations that happen to look similar.

Status: unsupported and unpublished — the same standing as every consumer
section above, corrected 2026-09-19 the same way: hosted evidence now exists
(run 35433295593 at `7def8fb1…`, all three OS jobs `success`), not yet a
#164 frozen candidate.

**Why this section exists.** Each consumer section above already generates
and runs its own hostile-pairing tests: a mutated descriptor, a mutated
binding, a well-formed binding naming a different descriptor, and the
per-leaf byte bound (exact and one-over), one test per outcome, written once
per language against that language's own per-issue fixture bytes. Before
this section, nothing compared their actual outcomes to each other — a
consumer that quietly started accepting what the other three reject would
not have failed anything, because every existing test only asserts against
its own author's expectation. `tests/support/public_generic_hostile_corpus.rs`
is the one shared manifest (one on-disk file, `#[path]`-included, unmodified,
into both native and Wasm harnesses) naming 27 cases and their one expected
outcome; `tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs`
and `tests/public_generic_wasm_adapter_v1/shared_hostile_corpus.rs` generate
all four consumers from the SAME canonical descriptor baseline, capture each
one's REAL observed outcome (never assert-and-swallow), and check every one
of them against that one manifest. The composed corpus is identified as
`semaprax.public-generic-hostile-corpus.v1` with outcome-manifest digest
`sha256:79ae641c8b3274fcb87e8cd314f028052f6cdcd5ccdea66f0ae524ccca89008f`;
the separate malformed-trusted descriptor manifest contains six additional
cases. The digest binds the 27 shared case ids and expected outcomes, the
exact bytes of the seven malformed and three valid result-carrier cases, and the existing
descriptor material; it does not bind native/Wasm harness-local mutation
recipes, which remain execution evidence.

**Coverage audit — what already existed per consumer before this issue** (test
names are exact, from the generator's own `ROUND_TRIP_BODY`/equivalent
template, exercised by the harness named in parentheses):

| Case | Rust (#156) | C11 (#158) | C++17 (#159) | TypeScript/Wasm (#157) |
| --- | --- | --- | --- | --- |
| Mutated descriptor rejected pre-allocation | `open_rejects_a_mutated_descriptor_before_any_native_allocation` | `test_open_rejects_a_mutated_descriptor_before_any_native_allocation` | `test_open_rejects_a_mutated_descriptor_before_any_native_allocation` | `"open rejects a mutated descriptor before any Wasm allocation"` |
| Mutated binding rejected pre-allocation | `open_rejects_a_mutated_binding_before_any_native_allocation` | `test_open_rejects_a_mutated_binding_before_any_native_allocation` | `test_open_rejects_a_mutated_binding_before_any_native_allocation` | `"open rejects a mutated provider binding before any Wasm allocation"` |
| Valid binding names a different well-formed descriptor | `open_rejects_a_valid_binding_that_names_a_different_descriptor` | `test_open_rejects_a_valid_binding_that_names_a_different_descriptor` | `test_open_rejects_a_valid_binding_that_names_a_different_descriptor` | `"open rejects a well-formed descriptor naming a different document"` |
| Exact per-leaf bound (64 KiB) accepted | `exactly_the_per_leaf_byte_bound_is_accepted` | `test_exactly_the_per_leaf_byte_bound_is_accepted` | `test_exactly_the_per_leaf_byte_bound_is_accepted` | `"exactly the per-leaf byte bound is accepted"` |
| One byte over the per-leaf bound rejected | `one_byte_over_the_per_leaf_bound_is_rejected_locally` | `test_one_byte_over_the_per_leaf_bound_is_rejected_locally` | `test_one_byte_over_the_per_leaf_bound_is_rejected_locally` | `"one byte over the per-leaf bound is rejected locally before any Wasm allocation"` |
| Full ordinal failure-injection matrix | `failure_injection_matrix_settles_every_ordinal_with_zero_live_resources` (`0..=13`) | `test_failure_injection_matrix_settles_every_ordinal_with_zero_live_resources` (`0..=13`) | `test_failure_injection_matrix_settles_every_ordinal_with_zero_live_resources` (`0..=13`) | `"the failure-injection matrix settles every ordinal with zero live resources"` (`0..=7`) |
| Provider-artifact digest mismatch (loaded module bytes changed) | not applicable — native providers are compiled in, not loaded as a runtime artifact | not applicable, same reason | not applicable, same reason | `"open rejects a module whose bytes do not replay the trusted provider-artifact digest"` |
| Stale/foreign/reused/post-close handle | covered by each native harness's OWN lifecycle matrix elsewhere (`probe.c`'s own C-hosted matrix; not this generator's own hostile-pairing set) | same | same | `"a stale/foreign handle is rejected rather than reused"`, `"a released input handle cannot be reused..."`, `"call after close is rejected"` |

Every cell above already passed before this issue; none of it is
duplicated by the shared corpus. What none of it did is compare outcomes
ACROSS languages — that is this section's actual contribution.

**What the shared corpus adds, and how agreement is enforced.** Fifteen cases
originally (issue #160), then seventeen with the cross-binding cases below,
generated from ONE canonical descriptor baseline
(`BASELINE_DESCRIPTOR_BYTES`) fed identically to all four
`generate_*_calling_consumer` calls: `success_baseline`,
`descriptor_first_byte_flipped`, `binding_last_byte_flipped`,
`descriptor_names_different_document`, `descriptor_unknown_schema`,
`descriptor_invalid_utf8_export_id`, `descriptor_stale_program_root`,
`descriptor_reordered_context`, `descriptor_duplicate_context`,
`descriptor_truncated_final_frame`, `descriptor_extra_frame`,
`descriptor_overlong_schema_claim`, `descriptor_presentation_rename`,
`exactly_per_leaf_bound_accepted`, `one_byte_over_per_leaf_bound_rejected`,
`binding_wrong_target_profile`, `binding_valid_for_different_artifact`.
The result-decoder family brings the total to 27: truncated width, wrong
leaf count, `u64::MAX` width, trailing byte, a complete 65,537-byte leaf,
declared length greater or smaller than the available payload, and accepted
empty, binary and exact-65,536-byte leaves. All ten use the generated
consumer's result-validator entry point. The positive controls prevent an
always-rejecting decoder from satisfying the hostile corpus; the complete
max-plus-one payload makes its capacity refusal independent of truncation.
The native C11 codec and its C++17 facade mutation-prove capacity, leaf-count,
and trailing-byte refusal branches. The capacity temporary artifact removes
only the per-leaf comparison and admits the complete max-plus-one document;
the two framing artifacts change only their reached refusal return to `OK`.
TypeScript independently changes the exact-width branch's closed result class.
The normal corpus requires every document to be refused, so these are
executable counterfactuals rather than source-text coverage. They do not claim
mutation proof for Rust, the other result-framing branches, or Logical Carrier
v1's separate field-path, kind, handle, and cleanup-plan rules.
Each generated consumer's own
`consumer.files()` output is left byte-for-byte untouched (the "byte for
byte" claim above still holds); the harness instead splices one additional,
hand-written test into the already-generated round-trip file at write time
(Rust: appended as a new `#[test]`; C11/C++17: inserted before the fixed
`main`, called immediately before its settlement line; TypeScript: inserted
before the fixed pass/fail tail of the generated `run()`), built ONLY from
that file's own already-generated helpers (`sample_input`/
`input_with_first_field`/`assert_reversed`, the embedded
`TRUSTED_DESCRIPTOR_BYTES`/`TRUSTED_BINDING_BYTES`, the provider's own
test-only diagnostics) — it never re-derives a generated field name itself
(`identifier`/`field_name` are `pub(crate)`-only inside
`semaprax::public_generic_consumer`, unreachable from an external
integration-test crate by construction). Every spliced test PRINTS its
observed outcome as `SHARED_CORPUS <case_id> <STATUS>` rather than only
asserting it locally; the harness parses all four real outputs and asserts
every one of them equals the SAME expected status in
`tests/support/public_generic_hostile_corpus.rs::EXPECTED` — so a consumer
that silently starts disagreeing with the other three fails with a message
naming the exact case and the exact wrong status observed, not a generic
"assertion failed." This was verified directly (not merely by inspection):
deliberately corrupting one status in the C11 driver during development
made `shared_hostile_corpus_agrees_across_rust_c11_and_cxx17_consumers` fail
with `binding_last_byte_flipped: c_calling_consumer reported
DESCRIPTOR_REJECTED, expected PROVIDER_MISMATCH`, then the change was
reverted and the test passed again.

**Native vs. Wasm: one manifest, two harnesses, not one process.** The
native (`tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs`,
covering Rust/C11/C++17) and Wasm
(`tests/public_generic_wasm_adapter_v1/shared_hostile_corpus.rs`, covering
TypeScript) shared-corpus tests are separate test binaries with disjoint
toolchain preconditions (clang/cargo vs. node/tsc) and cannot compare
outcomes inside one process. Agreement across all four is enforced
transitively rather than in one literal assertion: both harnesses check
their own consumers' real outcomes against the identical
`tests/support/public_generic_hostile_corpus.rs::EXPECTED` table, so if any
one of the four disagrees with what the shared manifest says the other
three must produce, that harness's own test fails.

**Zero-leak evidence, and whose counters it is.** Exactly the standing rule
every section above already states: every terminal native case asserts
`spx_pg_consumer_test_live_allocations`/`_live_handles` — the native
provider's OWN test-only counters, reached through each consumer's own
accessor, never a consumer's own bookkeeping. The TypeScript route's
"provider" here is `Provider.diagnostics.liveAllocations` — this is the
generated `wasm-provider.ts`'s own host-side bookkeeping for its
host-owned `Provider` route, deliberately selected by this corpus (see
below); the same file's separate `CompiledProvider` route delegates to
issue #229/#287's own compiled provider artifact instead, but this shared
corpus does not exercise that route.

**Known gaps, not fixed here, not duplicated:**

- **#229/#287**: a genuinely compiled provider ABI now exists
  (`src/wasm/public_generic_provider`) and the generated
  `CompiledProvider` route in this same TypeScript generator proves it
  (`tests/public_generic_wasm_adapter_v1/compiler_provider_artifact.rs`).
  This Wasm shared-corpus test intentionally continues to run against the
  SAME hand-assembled, clearly test-only `reference_wasm_module` the
  sibling TypeScript harness uses — one real fixture endpoint export over
  real `WebAssembly.Memory`, never the compiled provider — so the
  TypeScript route in THIS shared corpus specifically still exercises only
  the generated consumer's own codec/lifecycle logic against the fixture,
  not the compiled provider ABI; that is by design (explicit reference
  selection, matching the native trio's own untouched fixtures), not a
  remaining gap in #229/#287 itself.
- **#119**: flat owned-`Bytes` leaves only; the shared corpus's
  single-field `RecordShape` carries the same limitation every consumer
  section above already states, not a new one.
- **Aggregate-bound scope**: the shared hostile corpus intentionally remains
  small and does not duplicate saturation. Exact 16 MiB admission plus a
  one-byte-short pre-dispatch refusal are exercised in the dedicated native
  Rust/C11/C++17 `max_bounds_saturation.rs` harness and the generated
  TypeScript/Wasm `typescript_calling_consumer.rs` harness. Those are separate
  executable consumer tests, not a claim that the small hostile documents
  themselves materialize a 16 MiB payload. The native saturation harness is
  within the recorded hosted milestone scope; the later TypeScript aggregate
  pair is local-only until a hosted run executes that exact revision.
- The full ordinal failure-injection matrix is deliberately NOT included in
  the shared corpus: native's protocol has 14 injectable ordinals
  (`0..=13`) and Wasm's has 8 (`0..=7`) — different phase counts for
  different carrier protocols, so "ordinal N" does not name the same
  logical phase across native and Wasm and a literal per-ordinal
  cross-language comparison would compare different things under the same
  label. Each route's own existing, full, local matrix (see the audit table
  above) is unmodified and still runs.
- Sanitizer variants remain `#[ignore]`d in every harness this section
  touches, for the same reason stated in every section above: no sanitizer
  toolchain is provisioned here.
- Hosted job-level evidence for this widened section exists at the prior
  commits recorded above; no exact-current-head hosted run is claimed or
  implied.

### Cross-runtime and cross-artifact additions (issue #173)

Issue #173 asked, across the same four consumers, for hostility against
stale target/artifact associations and "independent recomputation rather
than trusting embedded digest fields." Two cases were added to
`tests/support/public_generic_hostile_corpus.rs::EXPECTED` (now seventeen, not
fifteen) and to both harnesses: `binding_wrong_target_profile` and
`binding_valid_for_different_artifact`. Both submit a FULLY well-formed
alternate `NativeProviderBindingV1`/`WasmProviderBindingV1` — never a
corrupted byte string, but a value a real decoder would happily accept as
*some* legitimate binding, just not this one's — so they exercise a
different failure mode than `binding_last_byte_flipped`'s arbitrary bit
flip: `binding_wrong_target_profile` names the OTHER route's
`TargetProfile` (`CoreWasm` submitted to the native route, `NativeC11`
submitted to the Wasm route); `binding_valid_for_different_artifact` names
the correct target profile but a different `provider_artifact_digest` and
endpoint symbol/export name, as if minted for a different deployed provider.
Both expect `PROVIDER_MISMATCH` and were verified, like the #160 cases
before them, with a real negative control: deliberately mis-declaring
either expected status in `EXPECTED` failed both the native and the
TypeScript harness with the exact case and the exact (correct) status
observed, then was reverted.

**Descriptor branch reachability.** The generated Rust, C11, and TypeScript
verifiers now parse both submitted and embedded Descriptor-v1 bytes into twelve
bounded UTF-8 fields, enforce the three frozen version literals, and compare
each field in canonical order; C++17 executes that C11 verifier. They no longer
authorize a descriptor through one opaque whole-buffer equality check. The nine
structured descriptor cases above pass through all four generated consumers,
while the separate six-case malformed-trusted
manifest therefore embeds each hostile document as the consumer's own
trusted bytes, then submits identical bytes. It exercises truncation,
descriptor schema, boundary-profile version, type-grammar version, UTF-8,
and trailing bytes. Executable mutation controls weaken one specific
generated check at a time and require its corresponding document to reach
the provider. C11, Rust and TypeScript have their own decoder mutations;
C++17 executes its generated facade over the same mutated C decoder. C++
reachability is checked by executing its facade, including provider cleanup,
rather than inferred from delegation. These mutations occur only in temporary
test artifacts and never weaken the committed generators.

**Carrier categories still outside this shared wire.** A generated flat
result carrier consists of a leaf count and length-prefixed byte payloads.
It has no generation, ownership flag, field path, variant tag or cleanup-plan
field to substitute. Generation belongs to the provider handle registry;
ownership and cleanup ordering belong to the call state machine and
settlement plan; leaf paths and the closed leaf-kind tag belong to
`LogicalCarrierFrame`. Their existing reference-codec, provider lifecycle and
settlement tests remain separate evidence. The 27-case consumer corpus does
not claim four-language equivalence for those categories. Separate focused
gates now connect canonical frames to the compiler-owned Wasm provider and to
the additive authenticated native identity profile; those gates do not
retroactively widen this shared corpus or establish hosted/public support.

**Two reference-codec gaps #173's audit found and closed directly.**
Auditing every hostile case already listed above against the five wire
codecs that actually exist (`descriptor.rs`, `carrier.rs`,
`carrier/frame.rs`, `native/binding.rs`, `wasm/binding.rs`) found two
classes named in #173's own scope with zero asserting test anywhere in the
repository: invalid UTF-8 inside a decoded string field (every one of the
five codecs decodes a field with `String::from_utf8`/`str::from_utf8` and
maps a failure to that codec's own malformed-input code, but no fixture
ever exercised the failing branch) and an unrecognized `LeafKind` wire tag
in `carrier/frame.rs` (`LeafKind` is a closed one-variant enum this round —
`Bytes`, tag `0` — so no public constructor can produce a second legal
variant to substitute, but the decoder's rejection of an unrecognized tag
byte, the real "variant tag" hostile case, was itself never exercised).
`tests/projections/public_generic_descriptor_carrier_hostile_replay.rs`
closes both, against the real public codec API only
(`decode`/`parse_bounded`/`decode_binding`/`decode_native_provider_binding`/
`decode_wasm_provider_binding`, never a private helper): six cases, one per
codec for invalid UTF-8 plus one for the carrier frame's leaf-kind tag,
each first asserting the unmutated fixture is ACCEPTED (the required
positive control), then asserting the SPECIFIC refusal code and message
substring on the mutated bytes — never merely that decoding failed. It
deliberately does not repeat reorder/duplicate/extra-field cases (already
the same "trailing bytes" branch under every codec's own existing test, per
the paragraph above — a byte-identical repetition under a new name would
prove nothing new) or cleanup-plan/release substitution (already covered by
`carrier/tests.rs`'s release-order tests and the cross-engine settlement
corpus, issue #162).

## Aggregate execution entry point (issue #172)

Audience: anyone who wants "all four generated calling consumers execute"
to be a single command they can run, rather than a claim reconstructed by
reading four separate test files.

Status: unsupported and unpublished, same standing as every section above
(corrected 2026-09-19: hosted evidence now exists, not yet a #164 frozen
candidate) — this script runs the same four `cargo test`-selected tests the
sections above already describe; it adds no new generator, no new provider,
and no new claim.

`tests/public_generic_native_adapter_v1/run_all_four_callers.sh` runs, as
four `cargo test` invocations against the checked-out tree: the Rust
(#156), C11 (#158), and C++17 (#159) headline execution tests in one
`cargo test --test public_generic_native_adapter_v1` invocation, then the
TypeScript/Wasm (#157) headline execution test in a separate
`cargo test --test public_generic_wasm_adapter_v1` invocation (a distinct
test binary with a distinct toolchain precondition — `node`/`tsc` versus
`clang`/`clang++`/`cargo` — exactly like every other harness pairing in
this document). It prints one `AGGREGATE <test> PASS|FAIL|SKIPPED` line per
caller and one summary. Missing `node`/a repository-pinned (5.8.3) `tsc` is
reported as an explicit `SKIPPED` line, never folded into a false pass or
fail, matching the underlying test's own skip behavior; `clang`, `clang++`,
and `cargo` are assumed present unconditionally, matching every native
harness above.

**The aggregate never claims parity across the four.** Its own printed PASS
line for the TypeScript/Wasm caller states, verbatim, that it ran "against a
TEST-ONLY Wasm stand-in module, NOT a real compiled provider ABI — see
issue #229," and its final summary states plainly that three of the four
callers execute against a genuinely compiled provider artifact today (Rust,
C11, C++17) while the fourth executes only against
`tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs`'s
hand-assembled stand-in. That fourth caller's own selected headline test
predates #229/#287's now-shipped compiled provider and this aggregate script
was not updated to add a fifth, compiled-provider selection, so its printed
claim about the fourth caller remains accurate for what it runs, not a claim
that #229/#287 is unimplemented: the compiled provider now exists and is
proven separately, outside this aggregate, by
`tests/public_generic_wasm_adapter_v1/compiler_provider_artifact.rs`. Run it
with:

```sh
sh tests/public_generic_native_adapter_v1/run_all_four_callers.sh
```

Exit code 0 means the three native callers passed and, when `node`/`tsc`
were available, the TypeScript/Wasm caller also passed against its stand-in
module — never that this specific script exercised the compiled Wasm
provider ABI.

## Cross-engine settlement corpus (issue #162)

Audience: PG-7 reviewers checking whether "equal checked behavior on
interpreter, native C11, and Core Wasm" is one comparison or three unrelated
claims that happen to agree.

Status: in-process, proof-only evidence — a narrower standing than every
section above in what it exercises (see below), though no longer narrower in
whether it is hosted: `cargo test --locked -p semaprax --lib
public_generic_abi` (which includes this module) is part of the
"Public generic ABI carrier, settlement corpus and WIT projection" step, and
that step is `success` in [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
at `7def8fb1…` (corrected 2026-09-19; not yet a #164 frozen candidate). This
is still **not** a fifth generated calling consumer: it does not compile a
foreign language, does not link a real native provider binary, and does not
instantiate a compiled `.wasm` module. It compares two in-process Rust
adapters directly.

`src/public_generic_abi/carrier/settlement_corpus.rs`
(`semaprax.public-generic-settlement-corpus.v1`) is one shared case table —
success, empty/zero-length leaves, two-leaf structural order, owned-value
abandonment, sticky failure after an earlier failure, and repeated
invocation/provider recreation — run against both
`public_generic_abi::interpreter::InterpreterProvider` and
`public_generic_abi::wasm::provider::WasmProvider`
([issue #155](PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)),
comparing each engine's observed status, its released
[`TraceEvent`](../src/public_generic_abi/carrier/trace.rs) sequence, and its
final live-allocation/handle count against one independently pinned
expectation and against each other. `should_panic` negative controls
(a truncated trace, a wrong status, a perturbed result, a nonzero final
live-allocation count) prove the comparison itself can fail rather than
vacuously agreeing.

**What this does not cover, stated once.** Native C11 is explicitly out of
scope for this corpus: `crate::public_generic_abi::native` only renders a
compilable C translation unit, and has no in-process Rust adapter analogous
to `InterpreterProvider`/`WasmProvider` to run this same case table against
without inventing a fourth artifact outside this file's own lease. Real
compiled-and-executed native settlement (allocation, the full `0..=13`
failure-injection matrix, zero-live-allocation assertions) is proven
separately by `tests/public_generic_native_adapter_v1/**` (`probe.c`,
`fixture.rs`, and the three native calling consumers above) — but not through
this corpus's own cross-engine comparison mechanism, so "equal checked
behavior on interpreter, native C11, and Core Wasm" is evidenced today as two
engines directly compared plus a third proven independently, not as one
three-way comparison. `WasmProvider` itself is the same in-process Rust
adapter the TypeScript/Wasm calling consumer above does **not** use — that
consumer's fixture/settlement route targets real compiled Wasm bytecode via
a hand-assembled stand-in module, distinct from the compiled provider
artifact issues #229/#287 shipped separately (see the two-routes note in
the TypeScript/Wasm calling consumer section above) — while this corpus
never leaves the Rust process — so the "Wasm" claims across this document
are evidence of three different things (`WasmProvider` in-process, the
fixture stand-in, and the #229/#287 compiled provider) and should not be
conflated. Peak allocation/handle counts are not compared, only final
(post-terminal) counts, since neither adapter tracks a peak.

**Execution evidence.** Verified directly for this update:
`cargo test --locked -p semaprax --lib
public_generic_abi::carrier::settlement_corpus` — 13 passed, 0 failed (6
cross-engine agreement cases, 5 `should_panic` negative controls on the checker
itself, and 2 settlement-manifest cases).

**Not yet recorded as hosted evidence.** The
`public-generic-ownership-milestone` job now continues beyond the grammar-only
`projections public_generic_consumers` gate: it invokes the native adapter,
Core-Wasm adapter, and descriptor/carrier hostile-replay harnesses. No hosted
run for that expanded corpus is recorded here yet; issue #163 owns the fresh
three-host run and its exact-commit evidence record. This module's local
settlement evidence therefore remains local evidence, and the cross-engine
limitations described above remain open under issue #162.

This section documents execution evidence only. The obligations this corpus
executes against are derived and specified in
[Public Generic Settlement Obligations v1](PUBLIC-GENERIC-SETTLEMENT-V1.md),
which remains that document's own lease.

The [native calling-consumer continuation](PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#native-calling-consumer-continuation-issue-162)
adds 112 shared C/C++ fixture cases, independently replayed evidence, caller-side
allocation/cleanup observations and Rust explicit-settlement implementation and
regressions. Production-template execution, actual generator execution and Rust
execution are labelled separately. This does not change PG-7 or PG-9 status.
