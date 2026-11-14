# Compiled source-audit transport V1

Audience: compiler contributors and reviewers.

Status: the three existing audit owners passed at `80099513c` on Rust 1.97.1.
Compiler peak reduction remains open. [OPT #798](https://github.com/wavect/semaprax/issues/798)

The HIR boundary, workspace projection and native interop layout audits retain
all live Rust snapshots as compile-time include_str! inputs. Their complete
original include occurrence order, repetitions, separators, forbidden predicates,
role classifier and static negatives are unchanged. No runtime filesystem read
of Rust source or authority is added.

A small local audit_source_join macro holds immutable &[&str] pieces and joins
at the same existing unmeasured audit invocation. Nested joins borrow temporary
Strings until the enclosing join finishes. Owning locals keep those strings
alive across the existing borrowed tuple/facade calls. No const/static consumer
requires a flattened literal. No new test binary, generic runtime cache or
framework exists. Original full source before-images and exact transport
substitutions are externally replayed; no flattened old corpus is embedded as
a binary oracle. Tiny cross-piece UTF8/NUL/CRLF equality assertions live in the
same original three owners.

Authority metadata generation normalizes only the exact declared test macro
for its existing literal-inventory grammar. Actual current source bytes remain
unmodified in the metadata and at runtime. The original classifier, production
forbidden predicates and six negative controls remain; chunk paths/literals,
multiplicity and production/test roles must equal the complete prior manifest.
Every production file length/hash remains exact. Existing source inverse domains
require a complete next->component view before prior-source assertions.

The direct include occurrence sums (before macro alias repetition) were
10,425,492 HIR bytes, 8,078,363 workspace bytes and 15,244,999 interop bytes at
b5eca55b96890e1a815f0d8d9e7b42c78d743b07. They belong to integration/interop
harness compilations, not the library unit artifact. These sums explain no
previous unit RSS refusal. New runtime String allocations happen only in the
standalone source/layout audit owners, outside ACTIVE and allocation observers;
this is no production cost or savings claim. Actual build memory comparison,
resource policy, all benchmark gates and full help remain separate and unchanged.

## Production library bootstrap

The measurements use the same source at `80099513c`, Rust 1.97.1
`8bab26f4f`, and macOS 26.5.1 on an 18 GiB arm64 host. Cargo used one worker,
offline locked dependencies, private SSD targets, optimization level 0,
debug info disabled, and incremental compilation disabled. The sampler
preserved Cargo's jobserver descriptors. Peak RSS is the operating system's
high-water reading for the primary library `rustc` process.

| Build scope | Setting | Peak RSS (bytes) | rustc seconds | Cargo seconds |
| --- | --- | ---: | ---: | ---: |
| Fresh private target | 256 codegen units | 4,666,556,416 | 102.46 | 234.21 |
| Fresh private target | 64 codegen units | 5,367,201,792 | 101.64 | 225.38 |
| Fresh library, shared dependencies | 256 codegen units | 4,990,140,416 | 101.72 | 102.62 |
| Fresh library, shared dependencies | 512 codegen units | 4,957,962,240 | 102.42 | 103.14 |
| Fresh library, shared dependencies | 256 units, default page decay | 5,098,864,640 | 102.12 | 107.48 |
| Fresh library, shared dependencies | 256 units, immediate page decay | 5,211,455,488 | 109.96 | 115.96 |

The fresh-target builds used `cargo build --locked --offline --lib -p semaprax
--jobs 1` with `CARGO_PROFILE_DEV_CODEGEN_UNITS` set per arm. The shared-dependency
builds used `cargo rustc` with an explicit final codegen-unit argument. Both
allocator runs passed identical compiler arguments and used the existing
jemalloc allocator. The second set `dirty_decay_ms:0,muzzy_decay_ms:0` for the
compiler process only. The two allocator builds produced byte-identical
library archives. [jemalloc options](https://jemalloc.net/jemalloc.3.html)

The 256-unit fresh-target compiler was in MIR borrow checking at its first
4 GiB RSS observation. In the shared-dependency default-allocator build, a
one-second stack sample started at 43.00 seconds with 4,991,778,816 bytes RSS.
The sampled RSS reached 5,057,740,800 bytes at 43.23 seconds. That stack showed
crate metadata encoding. A later sample also showed monomorphization
collection under metadata encoding. The 512-unit result differs from its
matched baseline by less than the variation between the 256-unit builds.

The 64- and 256-unit fresh-target builds produced byte-identical calculator
source, graph JSON, C11, Core Wasm, and unknown-call diagnostics. Their emitted
C11 ran at `-O0` and `-O2`, and Core Wasm ran in Node, all returning 42.
The HIR, workspace, and native interop audit binaries discovered 11, 505, and
249 tests respectively. Each original audit selector discovered and passed
exactly one test. The source-authority unit tests, current source-pin producer
check, and scoped production Clippy passed.

## Full library-test link attempt

The legacy and v0 symbol-mangling builds at
`435894a09f95f12046cc51e1df2d71b1a750eff2` requested the complete library
test binary on the same 18 GiB macOS arm64 host and Rust 1.97.1. Both used
`cargo test --locked --offline --jobs 1 -p semaprax --lib --no-run` with fresh
private SSD targets, 256 codegen units, optimization level 0, no debug info
or incremental compilation, and serial rustc front end and LLVM backend.
The measurement wrapper preserved Cargo's jobserver descriptors. The
captured primary rustc commands differed only in their target paths and
mangling mode.

| Mangling | Cargo wall seconds | rustc seconds to exit | Peak sampled rustc RSS before exit | Last completed rustc pass reported |
| --- | ---: | ---: | ---: | --- |
| legacy | 194.56 | 14.12 | 2,128,035,840 bytes | `crate_injection` |
| v0 | 185.10 | 19.60 | 3,009,216,512 bytes | `finalize_macro_resolutions` |

The host guard terminated each rustc after three consecutive samples of
macOS memory pressure level 2. The v0 arm completed macro expansion in
17.73 seconds, at 2,859,286,528 bytes of RSS. Both stops preceded
monomorphization and symbol mangling. These sampled peaks describe
different incomplete phases. Neither arm linked a new executable, so this
pair cannot establish a memory improvement or test inventory parity.
The 14 source-authority tests and current source-pin check passed separately.

## Current full library-test codegen-unit comparison

At `f0bae5b682351de082219c2053d59c34e855d2ba`, the same 18 GiB host
and Rust 1.97.1 targeted the complete library-test artifact sequentially.
The builds reused cached dependency artifacts and forced fresh primary
compilations through `cargo rustc --locked --offline --jobs 1 -p semaprax
--lib --profile test -- -Ccodegen-units=N`. Both used optimization level 0,
zero debug info, no incremental compilation and a measurement wrapper that
passed through Cargo's jobserver descriptors. The captured primary rustc
arguments differed only in the artifact suffix and codegen-unit count.

| Codegen units | Cargo wall seconds | rustc seconds | Kernel rustc peak RSS (bytes) | Sampled peak elapsed seconds |
| --- | ---: | ---: | ---: | ---: |
| 256 | 91.81 | 90.68 | 6,489,948,160 | 30.88 |
| 64 | 87.18 | 86.52 | 7,026,163,712 | 28.57 |

The host guard stopped both compilers after three consecutive one-second
samples at macOS memory pressure level 2. The one-second stack capture
starting at 30.44 seconds in the 256-unit arm placed its sampled peak
inside HIR type checking. That phase precedes code generation. The two
incomplete runs cannot establish a causal codegen-unit saving or linked
test inventory parity.

The previously linked five-worker library-test artifact for the same
source tree listed 11,533 tests and zero benchmarks. Its complete test-list
SHA-256 was
`9973fc3af0904d041236fe417d0141677279c46e2a5d333dd1f5d73b76f41109`.
All ten byte-traversal receiver tests passed on that artifact, including the
complete immediate predecessor and its inverse. The 14 source-authority
tests passed independently. The remaining bootstrap comparison needs two
successful one-worker builds with exact executable inventories and an
intervention that targets the measured high-water phase.

## All-features link attempt at `1fe13fc79`

Two fresh private targets requested the whole all-features library test
artifact on the same 18 GiB macOS arm64 host and Rust 1.97.1. Both builds ran
with one Cargo worker, optimization level 0, 256 codegen units, no debug
info, no incremental compilation, and a serial rustc front end and LLVM
backend. The primary compiler commands differed only in target paths and
symbol mangling. The sampler checked system memory pressure about once
per second and retained phase events and sampled process-group RSS.

| Mangling | rustc seconds to exit | Peak sampled group RSS | Last completed rustc pass |
| --- | ---: | ---: | --- |
| legacy | 37.72 | 4,092,428,288 bytes | `emit_ast_lowering_delayed_lints` |
| v0 | 26.46 | 3,419,619,328 bytes | `complete_gated_feature_checking` |

The host guard stopped both compilers after three consecutive checks at
macOS memory pressure level 2. Both stopped before monomorphization and
symbol mangling. These peaks cover different incomplete front-end
intervals. Executable inventory parity and a completed memory comparison
remain unverified.
