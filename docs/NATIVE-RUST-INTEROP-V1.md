# Native Rust Interoperability v1

Status: private A+B design and implementation are locally green. Public C and
hosted promotion remain held. The six output artifacts have frozen
whole-byte known-answer identities after independent exact replay and exhaustive
byte-edit rejection; this wire freeze is not an A+B runtime or platform GO.

Native Rust Interoperability v1 is an additive, current-host, scalar bridge. It
does not change callable v2/v3, the native loader or host, Graph schemas, Wasm,
`SPX-B104`, or any existing wire/KAT. Its admitted round trip is safe generated
Rust caller → selected SEMAPRAX export → selected Rust-import callback → scalar
result. It never detours through Wasm or a dynamic library.

## Source and semantic admission

The only new source form is an explicitly identified Rust import:

```spx
@id("host.add")
import rust fn add(left: i64, right: i64) -> i64
    effects { host.math }
    failure status "host.math.v1";
```

Every native Rust import must end with an explicit `failure status "domain";`
or `failure infallible;` clause; omission rejects rather than silently choosing
a failure model. Parameters are 0–8 value-mode `i64`/`bool`; results are unit,
`i64`, or `bool`. IDs are explicit, effects are sorted and selected, failure
domains are closed, and calls retain the distinct HIR kind
`NativeRustImportCall`. Selected exports are 1–32 explicit-ID,
non-entry, monomorphic scalar functions whose result is `i64` or `bool`; `unit`
is admitted only as a Rust-import result. Their acyclic transitive closure is at
most 256 functions and may reach only selected Rust imports. Calls from a
contract, including through a helper, are rejected. Graph-derived routes reject
`SPX-G218`; Wasm rejects `SPX-W114`; ordinary callable routes remain closed by
`SPX-B104`.

## Canonical documents and digests

Spec schema is `semaprax.native-rust-interop-spec.v1`, compact JSON plus one LF,
with ordered keys `schema,module,source_revision,target,exports,imports,
capabilities,limits,nonclaims`. Descriptor schema is
`semaprax.native-rust-interop-descriptor.v1`, with ordered keys `schema,module,
source_revision,hir_digest,target,status_domains,abi,exports,imports,limits,
nonclaims`. Bundle schema is `semaprax.native-rust-interop-bundle.v1`, with
ordered keys `schema,descriptor,files,toolchain,limits,nonclaims`.

Digest domains are:

- `semaprax.native-rust-interop.source-revision.v1\0`
- `semaprax.native-rust-interop.hir-digest.v1\0`
- `semaprax.native-rust-interop.spec-digest.v1\0`
- `semaprax.native-rust-interop.descriptor-digest.v1\0`
- `semaprax.native-rust-interop.call-contract.v1\0`
- `semaprax.native-rust-interop.capabilities.v1\0`
- `semaprax.native-rust-interop.bundle-digest.v1\0`

The target row is `triple,pointer_width,endian,panic_strategy,thread_policy` and
admits only the exact current host, 64-bit little-endian, unwind, same-thread
profile. Call contracts use u64-BE length framing and bind direction, persistent
ID, source parameter names and scalar types, result, sorted effects and
capabilities, exact status domains/ordinals including semantic 65533, host
65534, and adapter 65535 where required, complete ABI row, and target.

Limits are fixed: exports 32, imports 32, parameters 8, closure functions 256,
status domains 64, effects 64, identifier bytes 128, source bytes 16,777,216,
spec bytes 1,048,576, descriptor bytes 1,048,576, generated C bytes 4,194,304,
generated header bytes 1,048,576, combined generated Rust bytes 4,194,304,
manifest bytes 1,048,576, cumulative builder bytes 33,554,432, JSON depth 8,
semantic expression depth 512, call depth 32, bridge crossings 4,096, and
unexpected inventory entries 0.

## ABI and status

The generated C ABI is version 1, calling convention C. `spxnr_status_v1` is a
u64: code bits 0–31, class 32–39, retry bit 40, reserved zero bits 41–47, and
domain ordinal 48–63. Zero is success. Ordinal 65533 is
`semaprax.native-rust-semantics.v1`, 65534 is host, and 65535 is adapter;
selected status domains occupy sorted ordinals 1..N. Semantic codes are neg/add/
sub/mul/div/rem = 1..6. Contract pre/post codes are 1/2. Results are caller-owned,
uninitialized, and written only after complete success.

Context `SPXNRCTX1` stores ABI version, size, userdata, imports-table pointer,
capability digest, call depth, and zero reserved word. `SPXNRIMP1` stores version,
size, and callbacks in descriptor order. C validates pointer alignment,
versions, sizes, bool 0/1, callback presence, capability digest, depth, result
pointer, and status canonicality. The safe Rust wrapper enforces the call budget,
same-thread ownership, and non-reentrant use before effects. The generated bridge never formats, returns, or
stores a caught panic payload and forgets it before the FFI return; no unwind
crosses FFI. Output from a caller-installed process-global panic hook is outside
the bridge's authority and is neither suppressed nor claimed. No allocator
crosses the boundary.

Generated safe Rust defines `NativeRustImports`, `NativeRustImportResult`,
`NativeRustStatusClass`, `NativeRustCapabilities`, `NativeRustBridge`, and the
closed call errors. The bridge is opaque, non-Clone/non-Debug, !Send/!Sync,
same-thread, and has no host/raw-context escape. A private sibling FFI module is
the only generated unsafe quarantine.

## Build and publication authority

Private A is pure:
`prepare_native_rust_interop(&Program,&[u8]) -> PreparedNativeRustInterop`.
Its cumulative authority is reserved before phase entry. Pre-resolution HIR,
cleanup inventory/plan, TypeFacts, post-HIR fact construction, the five
renderers, Descriptor replay, and the independent C-expression replay have
named retained-versus-scratch envelopes, iterative depth-bounded traversals,
observed high-water gates, and exact/minus-one entry tests. Persistent facts
and final artifact sinks are charged separately from sequential scratch; the
Spec allocation is transferred rather than charged twice. These are local
bounded-memory facts for this private preparation path, not a general compiler
allocation or no-allocation claim.

Private B calls A once. `RUSTC` must name an explicit absolute discovery
executable; that executable may only run the frozen bounded sysroot query and
produces no accepted artifact. B independently opens the reported sysroot and
its exact `bin/rustc`/`bin/rustc.exe`, rejects path indirection, requires that
direct compiler to reproduce the same held sysroot, validates its version, and
admits Rust artifacts only through the distinct held-direct-rustc authority.
Clang is independently held. One exact pre-effect process arena is consumed by
the four discovery/version operations and eight build/link/run operations. On
Windows its attribute-list size is queried once before effects, capped,
aligned, reserved before allocation, and rechecked on every use. Windows also
requires a verified absolute `SEMAPRAX_LINKER`, embeds that exact path as one
prepared `--ld-path=<absolute>` argument, and holds and rechecks the linker
around both Clang links without adding `PATH` to the child environment. Private B
exactly replays Descriptor and Manifest bytes, and generates
header/C/safe-Rust/private-FFI artifacts with independent ordered exact-byte
consumers. Prepared invocations bind the admitted current-host target spelling,
including underscore-bearing target components, while rejecting other
punctuation. The four required `rustc -vV` fields share one preallocated
65,536-byte fixed-capacity store; parsing is no-growth and its retained capacity
is transferred exactly into Phase B rather than reserving four independent
maximum strings. The canonical Spec input and all six outputs reject every-byte
substitution, deletion, insertion, and truncation. One fixed-target fixture pins
the byte length and independently recomputed raw SHA-256 of Descriptor, Manifest,
header, C, safe Rust, and private FFI; it additionally pins the existing protocol
domain digests for Descriptor and Manifest. It compiles strict C and Rust,
statically links the object with the frozen Linux native-static library tail
when applicable, executes the round trip, then publishes a create-new exact
inventory.
There is no dylib, loader, symbol lookup, network, CLI, or public execution
surface.

This direct-image policy closes ordinary rustup-launcher indirection. It does
not claim provenance for the selected compiler sysroot, dynamically loaded
libraries or backends, or arbitrary descendants. The configured MSVC linker is
path-bound and drift-checked, but the current share mode does not prove the
exact descendant image under a same-path replacement race. The explicitly
configured discovery executable is trusted only to nominate the direct
compiler that is then independently held and exercised.

Every owned build stage is continuously represented by the directory authority
returned when it was created. Settlement uses only the opaque exact-inventory
discard operation. Identity, reparse/symlink, or inventory disagreement stops
deletion, preserves any foreign sentinel, and leaves inert residue for external
recovery. Exact success/failure-path settlement evidence remains a promotion
gate. The safe facade and system quarantine expose no generic or recursive
path-delete operation.

Windows promotion additionally requires executable tests for zero and small
stdout at normal EOF, silent deadline expiry, descendant-held stdout without
overflow, one-character/reserved-DOS/case-folded names, and injected image,
Job assignment, resume, terminate, wait/query, pipe-peek, and pipe-read
failures. Every ordinary error must retain its sticky code only after proven
leader-and-Job quiescence. An unprovable settlement must fail-stop before any
later tool or publication action. Source inspection and non-Windows cfg-off
compilation do not satisfy this hosted gate.

The six manifest file rows are `descriptor.json`, `module.c`,
`semaprax_native_rust_interop.h`, `semaprax_native_rust_interop.rs`,
`semaprax_native_rust_interop_ffi.rs`, and `module.o`/`module.obj`, sorted. The
directory additionally contains `semaprax.native-rust-interop.json`. The
manifest never hashes itself.

## Diagnostics and nonclaims

The exact owned diagnostics are B106 noncanonical spec; B107 closed declaration
reason; B108 descriptor disagreement; B109 limit; B110 target/toolchain; B111
generated replay; I230 Clang; I231 Rust link/run; I232 publication; G218 Graph;
and W114 Wasm. Diagnostics never echo source, paths, tool output, secrets, panic
payloads, or pointers.

The ordered nonclaims in every document deny resource/aggregate/pointer ABI,
cross-boundary allocation, Wasm detours, dynamic loading, public execution,
changes to callable/Graph/Agent/Economic/Workspace/Patch wires, sandboxing,
same-UID process signaling or task-port isolation,
cross-target reuse, unwind, abort/OOM/signal/process recovery, power-loss
durability, async/reentrant/cross-thread use, provenance or ambient authority,
error text/payload evidence, exactly-once effects, other ecosystem bindings,
dynamic dependency identity or filesystem-race isolation,
stable Rust ABI, public CLI/registry/network, general interop readiness, and a
completion-matrix promotion.

Public C remains held until this private A+B surface is committed and its exact
head is green on Ubuntu, macOS, and Windows, including the required Windows
runtime/capacity settlement and Linux sanitizer lanes. Local runs qualify only
when `RUSTC` and `CLANG` explicitly select the admitted absolute tools; ambient
launcher or proxy discovery is intentionally not equivalent evidence.
