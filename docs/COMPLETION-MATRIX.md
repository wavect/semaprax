# Full-goal completion matrix

This document is the authoritative audit checklist for the complete SEMAPRAX objective. A row is complete only when the linked implementation and automated evidence prove the stated gate. Design text, generated placeholders, or a successful build on a narrower target do not satisfy a broader gate.
The dashboard is refreshed at meaningful executable-evidence milestones, not
for each internal refactor, so progress remains visible without inflating
status from configuration or design alone.

Agent Runtime note: [Bounded Native Agent Runtime v1](AGENT-RUNTIME-V1.md) adds
deterministic fake-host parser/router/tool-loop and Trace/Evidence coverage plus
a narrow injected-host C1 Rust API. It does not change any completion row and
makes no public provider, language/backend,
durable-memory, wallet, payment, signing, or economic-authority claim.
Public Agent Runtime v1 is hosted GREEN at 8cf29aff8d1be3ccf74c36bc8c837f0c666ca067 (run 31591039261, 12/12 jobs, private and public deterministic fake-host gates on Ubuntu, macOS, and Windows). Private Economic Agent v1 A+B is exact-head hosted green at fe75c38d898b71e3ed5c57411fb46d0dbd4fc34b in run 31611748969, including both Economic gates on Ubuntu, macOS, and Windows. Public Economic Agent v1 C is exact-head hosted green at 03f1f2736de23d03b298f265f93409de89a6be95 in run 31616168124 (12/12 jobs), including the private, process-termination, and public Economic gates on Ubuntu, macOS, and Windows.
Public Wasm Scalar Exports v1 moves only the JavaScript and TypeScript row from
Missing to Partial. Current totals are 39 Partial/17 Missing.
[Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) and [Project Manifest
v2](PROJECT-MANIFEST-V2.md) deepen existing Partial language, Project,
WebAssembly, JavaScript/TypeScript, Web, and package rows with local evidence
only; they do not change dashboard totals. Exact-head hosted promotion remains
pending, and no npm registry publication is claimed.
[Project Manifest v3](PROJECT-MANIFEST-V3.md) and the public
`useful-data.v1` adapter deepen the same existing Partial Project,
WebAssembly, JavaScript/TypeScript, Web, and package rows with local evidence
only. Unix v2 publication is safely handle-relative; Windows v2 publication
remains fail-closed. Hosted promotion, registry publication, and release
promotion remain open, and dashboard totals do not change.
[Typed Hygienic Generation v1](HYGIENIC-GEN-V1.md) is locally evidenced
(`cargo test --locked -p semaprax --test hygienic_gen_v1` plus unit tests) and
moves only the Typed hygienic generation row from Missing to Partial; it makes
 no hosted-execution claim. Current totals are 40 Partial/16 Missing.
[OpenAPI Schema Generation v1](OPENAPI-V1.md) and [C Header Emission
v1](C-HEADER-V1.md) are locally evidenced read-only projection tranches that
move the OpenAPI, Protobuf/gRPC, GraphQL, and SQL row and the C and
Objective-C row from Missing to Partial; neither claims schema import, live
conformance fixtures, compiled header evidence, or hosted promotion. Together
with Typed Hygienic Generation v1 this batch moves three rows Missing to
Partial. Current totals are 42 Partial/14 Missing.
[Canonical ABI Report v1](ABI-REPORT-V1.md) is a locally evidenced read-only
report/descriptor tranche that moves only the Portable canonical ABI and
native fast ABI row from Missing to Partial: `semaprax abi-report` emits one
deterministic digest-authenticated envelope describing both the Native64 fast
ABI (verbatim production prototypes, checked sizes/alignments, by-value copy,
status/out contract) and the portable Core-Wasm scalar mapping (`i64`/`i32`,
canonical bool boundary, copy-only) for selected monomorphic by-value
`i64`/`bool` functions, cross-consistent with both real backend projections.
It claims no interface semantics beyond the selected scalar exports, no
borrowing, no cross-language conformance suites, no target execution, and no
hosted promotion.
[Build Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) is a locally
evidenced read-only projection tranche (`semaprax capability-manifest`,
`semaprax.capability-manifest.v1`) that moves only the Sandboxed builds and
dependencies row from Missing to Partial: it declares one verified module's
exact per-function and import effect inventories plus its module permit list
inside a closed five-domain capability vocabulary, asserts an explicit
empty-by-default ambient authority section over filesystem/home/network/
process/secrets, fails closed (`SPX-K202`-`SPX-K204`) on out-of-vocabulary
capabilities, injection, tampering, and source drift, and pins golden KATs in
`tests/capability_manifest_v1.rs`. It performs no sandbox enforcement at
build time, resolves no dependencies, writes no lockfile, hosts no registry,
and provides no enforcement machinery; Project Manifest v1 still has no
resolver, lockfile, dependency graph, package registry, or capability
sandbox.
[C++ Shim Projection v1](CXX-SHIM-V1.md) is locally evidenced
(`cargo test --locked -p semaprax --test cxx_shim_projection_v1` plus unit
tests) and moves only the C++ row from Missing to Partial: the read-only
`semaprax cxx-shim` command projects explicitly selected explicit-ID
monomorphic by-value `i64`/`bool` functions of one verified module into a
deterministic C++17-compatible `extern "C"` header fragment whose declaration
lines are extracted verbatim from the production native C11 projection, under
a digest-authenticated canonical envelope with independent replay
verification. It claims no C++ compilation or conformance, no header import
or parsing, no exception/ownership policy beyond the bounded slice, no
adapters, and no hosted promotion. Current totals are 45 Partial/11 Missing.
[Interface Package Report v1](PACKAGE-REPORT-V1.md) is a locally evidenced
read-only projection tranche (`semaprax package-report <file>`,
`semaprax.package-report.v1`) that moves only the Interface-first packages
and target matrices row from Missing to Partial: one verified module is
described by one digest-authenticated canonical envelope carrying its sorted
admitted export inventory (explicit-ID monomorphic effect-free by-value
`i64`/`bool` functions with interface parameter/result types, canonically
rendered contracts/effects, persistent stable IDs, and exact Native64
prototype lines extracted verbatim from the production native projection
under per-export domain-separated digests), a closed target availability
matrix marking exactly `native64` and `wasm32` available for this profile,
and an explicit closed unavailable-capability inventory. Independent replay
re-derives counts, both closed sections, exclusion vocabulary, export order,
and every signature digest; pinned KATs, tamper rejection including
forged-but-re-signed envelopes, CLI exit codes, and cross-consistency with
`semaprax abi-report` and `semaprax openapi` are green locally in
`tests/package_report_v1.rs`. It provides no resolver, lockfile, dependency
model, registry, compatibility engine, conformance tests, provenance,
signatures, licenses, or SBOM; Project Manifest v1 boundaries remain.
Current totals are 46 Partial/10 Missing.
[UI Dialect Schema Projection v1](UI-SCHEMA-V1.md) is a locally evidenced
read-only projection tranche (`semaprax ui-schema`,
`semaprax.ui-dialect-schema.v1`) that moves only the First-class
application/state/UI dialect row from Missing to Partial: one verified module
is projected into one deterministic digest-authenticated envelope whose
state-shape descriptors carry checked Native64 record layouts (field names,
`i64`/`bool` types, offsets/sizes/alignments from `aggregate_layout`) for
public non-generic scalar records, whose action descriptors mirror the
Canonical ABI Report v1 admission profile with parameter/result types, and
whose controls/accessibility/navigation section is explicitly empty by
default. Pinned KATs, every exclusion reason, per-field tamper rejection,
and cross-consistency against checked layouts and abi-report signatures are
green locally in `tests/ui_schema_v1.rs`. It claims no typed update/view
language constructs, no semantic controls, accessibility, navigation,
localization, assets, platform blocks, or custom rendering, no rendering,
runtime, or DOM, and no target execution. Current totals are 47 Partial/9
Missing.
[Freestanding Object Profile v1](FREESTANDING-V1.md) is a locally evidenced
read-only projection tranche (`semaprax freestanding-object`,
`semaprax.freestanding.v1`) that moves only the Embedded and real-time row
from Missing to Partial: for one verified effect-free scalar module it emits
one deterministic digest-authenticated envelope containing the complete
freestanding C11 translation unit derived from the production native C11
projection with the host entry wrapper, stdio/stdlib includes, and
public-failure reporter excluded, two recorded substitutions
(invariant-failstop, external function linkage), explicit no-runtime/
no-allocation/no-blocking/no-libc-dependency assertions replayed during
independent verification, and declared allowed undefined symbols; tests
compile the real emitted bytes with `-ffreestanding -nostdlib -c` into
relocatable objects and pin the symbol surface against the declared set. It
claims no MMIO/volatile/atomics, linker-script control, hardware/emulator
execution, interrupt/RTOS model, or board targets, invokes no toolchain from
the command itself, and claims no completion beyond this bounded slice.
Current totals are 48 Partial/8 Missing.
[Explicit Mutation v1](EXPLICIT-MUTATION-V1.md) is a locally evidenced
language slice that moves only the Immutable-by-default values and explicit
mutation row from Missing to Partial: local bindings may declare `let mut`,
and simple locals admit statement-only assignment `<binding> = <expr>;` with
exact type matching, left-to-right evaluation, initializer-equivalent checked
arithmetic statuses on both native C11 O0/O2 and Node/Wasm, additive Graph
serialization (`"mutable":true`, `"kind":"assign"`; non-mutation graphs stay
byte-identical), and CleanupPlan v2 shapes unchanged for straight-line
mutation. Diagnostics `SPX-U101`-`SPX-U106` reject assignment to immutable
bindings, exact type mismatches, `mut` outside local lets, duplicate
modifiers, non-scalar/non-Copy targets or values, and assignments inside
contract expressions. It claims no field, aggregate, reference/borrow, or
collection mutation, no concurrency or memory-model rules, and no cross-task
rules. Current totals are 49 Partial/7 Missing.
[Reference Interpreter v1](INTERPRETER-V1.md) is a locally evidenced
evaluation tranche (`semaprax interpret <file> --function <name|stable-id>
[--arg <i64|bool literal>]... [--max-bytes N]`, `semaprax.interpret.v1`)
that moves only the Fast development lane row from Missing to Partial: one
explicit-ID monomorphic effect-free scalar function of one verified module
is evaluated directly from verified HIR with the admitted scalar surface —
`let mut`/assignment, blocks, `if`, lazy operators, left-to-right evaluation
with sticky failure selection, checked `i64`/`i32`/`u8` arithmetic reusing
the compiler's exact status table, total IEEE-754 floats, contracts, and
admitted calls including recursion — and one digest-authenticated canonical
JSON envelope reports the returned value or the exact normalized failure
status plus fuel accounting (steps used versus budget), argument echo, and
source digest. A 28-row corpus proves byte-identical result/status
transcripts against native C11 O0/O2 and Node/Wasm; pinned KATs, fuel
exhaustion fail-closed, determinism, per-field tamper rejection, drift
binding, and CLI exit codes are green locally in `tests/interpreter_v1.rs`.
It makes no JIT/AOT/Cranelift, incremental persistence, hot reload, or
debugger mapping claim, executes no target, and changes no source. Current
totals are 50 Partial/6 Missing.
[Unsafe Boundary Mechanics v1](UNSAFE-BOUNDARIES-V1.md) is a locally
evidenced language slice that moves only the Restricted unsafe and raw memory
row from Missing to Partial: an `unsafe { .. }` statement wraps ordinary safe
checked statements (no raw pointers or memory operations exist and none are
added), each block requires a verbatim `@audit("...")` summary following the
`@id` attribute pattern, the module must declare `permit { unsafe }` through
the existing permit mechanism (`SPX-N101`-`SPX-N105` fail closed), Graph JSON
gains one explicit `"kind":"unsafe"` node per boundary with unchanged schema
selection and byte-identical output for non-boundary programs, backends lower
the body transparently on native C11 O0/O2 and Node/Wasm, and CleanupPlan v2
shapes are unchanged. It claims no raw pointers or memory operations, no lint
or platform conformance, boundary mechanics only, and no safety claims about
block contents. Current totals are 51 Partial/5 Missing.
[Plugin Manifest Projection v1](PLUGIN-MANIFEST-V1.md) is a locally
evidenced read-only projection tranche (`semaprax plugin-manifest <file>`,
`semaprax.plugin-manifest.v1`) that moves only the Plugins row from Missing
to Partial: one verified module is described by one digest-authenticated
canonical envelope carrying its sorted provided-export inventory
(explicit-ID monomorphic effect-free by-value `i64`/`bool` functions with
persistent stable IDs, interface types, rendered contracts, and exact
Native64 prototype lines extracted verbatim from the production native
projection under per-export domain-separated digests), six closed exclusion
reasons covering every non-admitted function, plugin identity fields sourced
from module metadata conventions (module name plus a build-hash-style
version derived from the domain-separated stable source digest; no version
metadata exists in the language today), required host capabilities derived
exactly like Build Capability Manifest v1 inside the same closed five-domain
vocabulary with fail-closed `SPX-Q102` rejection of every out-of-vocabulary
token, an explicit empty-by-default canonical resource-limits section, and a
closed unavailable-sections inventory. Independent replay re-derives
counts, all closed sections, exclusion vocabulary, export order, identity/
version consistency, and every signature digest; pinned KATs, tamper
rejection including forged-but-re-signed envelopes, determinism, budget
fail-closed behavior, CLI exit codes, and cross-consistency with `semaprax
capability-manifest` (equal capability sections) and `semaprax abi-report`
(byte-equal symbols/signatures) are green locally in
`tests/plugin_manifest_v1.rs`. It performs no Component Model runtime or
packaging, no host loading or lifecycle management, no versioning
negotiation, no resource-limit enforcement, and no hostile-plugin execution
testing. Current totals are 52 Partial/4 Missing.
[Region Structure Report v1](REGION-REPORT-V1.md) is a locally evidenced
read-only projection tranche (`semaprax region-report <file>`,
`semaprax.region-report.v1`) that moves only the Regions/arenas row from
Missing to Partial: for every admitted explicit-ID monomorphic effect-free
scalar function of one verified module it reports the binding lifetime
partition derived from existing borrow/move facts (parameters, `let`/`let
mut` locals, and match pattern bindings with real resolved-HIR value ids,
ownership modes, type keys, definition offsets, effective live-range ends,
and use counts), canonical region clusters under the rule that overlapping
live ranges can never share one region, escape facts naming `SPX-O104` as the
check that makes today's borrows provably non-escaping, resolved-call-graph
own-consumption move facts, and maximal bulk-release grouping candidates of
co-dying bindings, all inside one digest-authenticated canonical envelope
whose independent replay re-derives the clustering, escape, move, and
grouping sections exactly. Pinned KATs, determinism, every exclusion reason,
resolved-HIR cross-consistency, tamper rejection including forged-but-re-signed
envelopes, budget fail-closed behavior, and CLI exit codes are green locally in
`tests/region_report_v1.rs`. It implements no region inference, adds no region
annotation syntax, introduces no arena runtime behavior, changes no destructor
behavior, executes nothing, and changes no source. Current totals are 53
[Deterministic Scoped Task Model v1](SCOPED-TASKS-V1.md) is a locally evidenced
hidden proof-model tranche (`src/scoped_tasks.rs`,
`tests/scoped_tasks_model_v1.rs`) in the exact `native_settlement` style: a
deterministic target-neutral model of structured scoped concurrency whose
bounded task DAG inside one strict scope tree, canonical stable-id sequential
scheduling, sticky cancellation propagation (descendants cancel before any
sibling starts new work while started work drains), children-before-parents
cleanup in reverse completion order, first-failure stickiness with sibling
draining, closed per-task `Sendable`/`Shareable` annotations, fail-closed
structural hostility (escaping dependencies, double/orphan/missing joins,
cycles, bounds, work budget), input-permutation determinism, and
domain-separated canonical JSON trace digests are pinned by four KAT digests
plus hostile, determinism, and serialization evidence. It adds no language
syntax, no runtime threads or scheduler, no compiler/backend change, no real
concurrency execution, and no `Sendable` checking of real programs; actors,
reducers, synchronization, and verified schedulers remain open. It moves only
the Structured concurrency row from Missing to Partial. Current totals are 54
Partial/2 Missing.
[Deterministic ARC Zone Model v1](ARC-ZONES-V1.md) is a locally evidenced
hidden proof model (`src/arc_zones.rs`, `cargo test -p semaprax --test
arc_zones_model_v1` plus seven module units) that moves only the Shared
immutable ARC and opt-in managed zones row from Missing to Partial: one
deterministic target-neutral model of retain/release reference counting inside
explicit opt-in managed zones fixes bounded per-zone object graphs, exact
reverse-construction finalization order with canonical-order payload-link
cascades, closed cycle-participation deferral whose zone exits reject retained
cycles fail-closed with one smallest-member witness diagnostic instead of
leaking silently, escape demotion as a deterministic rewrite rule for proven
zone-local shared handles, and closed concurrency annotations under which zones
are single-threaded by declaration and cross-zone/cross-thread sharing requires
an explicit `Shareable` mark; four canonical known-answer trace digests,
hostile rejections (foreign-zone release, double release, unbalanced exit),
inventory-permutation determinism, and byte-pinned domain-separated canonical
JSON are pinned in `tests/arc_zones_model_v1.rs`. It performs no runtime RC
integration, adds no language syntax or compiler/backend change, executes no
target, and claims no real allocation behavior. Current totals are 55
Partial/1 Missing.
[Portable SIMD Eligibility Report v1](SIMD-REPORT-V1.md) is a locally
evidenced read-only analysis tranche (`semaprax simd-report <file>
[--max-bytes N]`, `semaprax.simd-report.v1`) that moves only the SIMD and GPU
row from Missing to Partial: per admitted explicit-ID monomorphic effect-free
scalar function of one verified module it reports every maximal pure
straight-line arithmetic sub-expression over `i64`/`i32`/`u8`/`f32`/`f64`
derived from the real resolved HIR nodes with its proposed portable lane
width (2/4/8 under a fixed 128-bit lane model with a documented deterministic
largest-feasible-first rule), the closed portable lane-operation mapping
table, effect-freedom justification facts, and an explicit closed
ineligibility reason for every non-covered expression (calls, contracts,
division/remainder, bool mixing, char ops, mutation targets, computed
operands, control flow, aggregate operations, scalar leaves); independent
digest replay re-derives counts, vocabularies, ordering, per-region digests,
and width feasibility, so forged-but-re-signed envelopes fail closed
(`SPX-V101`-`SPX-V103`). Pinned KATs, determinism, budget fail-closed
behavior, CLI exit codes, and cross-consistency against real HIR nodes are
green locally in `tests/simd_report_v1.rs`. It emits no SIMD codegen or
intrinsics, no SPIR-V/WebGPU/GPU kernels, makes no autovectorization claim,
executes no target, and changes no source. Current totals are 56
Partial/0 Missing.
[Batch integration 2026-08-24] Seven locally evidenced tranches deepen
existing Partial rows without moving any status: String operations v1 adds
reserved `core.string.*` intrinsics (`string_len`, `string_concat`,
`string_is_empty`) through the ordinary monomorphic call path with
native/Wasm/interpreter equivalence (`tests/string_ops_v1.rs`);
Field mutation v1 admits direct scalar Copy-field assignment on `let mut`
record/class locals (`SPX-U107`-`SPX-U112`, unchanged CleanupPlan shapes,
`tests/field_mutation_v1.rs`); Bounded while-loop v1 admits `while <bool>`
under a Copy-scalar profile with program-level Graph v15 above v14,
linearized cleanup-plan iteration under fail-closed liveness guards, and
native/Wasm/interpreter lowering (`SPX-T251`-`SPX-T253`, `tests/
while_loops_v1.rs`); Reference Interpreter admission widens to the full
Copy-scalar surface (`tests/interpreter_scalar_widen_v1.rs`);
`abi-report`/`c-header`/`cxx-shim`, `openapi`/`package-report`/`ui-schema`
admit the same widened surface with verbatim-native digests, bound-aware
compat classification, and layout cross-consistency
(`tests/interop_scalar_widen_v1.rs`, `tests/schema_scalar_widen_v1.rs`);
Class single inheritance v1 adds ancestor-prefix layouts, static nearest-
first method resolution, exact-signature overrides, `super.m(...)` calls,
and typed-let upcasts under a cleanup-inert suffix rule
(`SPX-T227`-`SPX-T234`, `tests/class_inheritance_v1.rs`). All claims remain
bounded by their doc files' nonclaims; no target execution, hosted
promotion, or status change is claimed, and frozen KATs were re-pinned only
for honest builder-bytes growth from the new syntax. Public Project Developer
Loop v1 additionally exposes bounded in-process `run` and exact-manifest-module
`test` over authenticated linked HIR, with revision/stable-ID/fuel/outcome-bound
canonical evidence and no artifact, process, discovery, or target-execution
claim. This adds evidence inside existing Partial rows and makes no status
transition. Current totals remain 56 Partial/0 Missing.

[Indexed Byte Loop v2](WHILE-LOOPS-V1.md#indexed-byte-loop-v2) locally deepens
the Portable Indexed Byte Data slice and now admits true dynamic immutable
traversal inside bounded loops: only
compiler-owned `byte_len`/`byte_get` reads and an exhaustive, guard-free match
over the exact compiler-prelude `Option<u8>` result are accepted. Source and
hostile-HIR validation keep view construction, allocation, owned data, general
aggregate matching, imports, and effects closed. CleanupPlan schemas and
liveness meaning remain unchanged, and the binary-frame Project exercises
dynamic in-range plus `None` reads through interpreter, native O0/O2, and
Core-Wasm/Node. This deepens existing Partial control-flow and Useful Data rows
without a status change; current totals remain 56 Partial/0 Missing.

Status values:

- **Implemented** — the gate is covered by executable evidence.
- **Partial** — useful implementation exists, but the full gate is not proven.
- **Missing** — no qualifying implementation exists yet.

## Milestone dashboard

This compact view is the release-truth summary; the detailed rows below remain
the completion contract. Current totals are **56 Partial / 0 Missing**; every
row remains Partial because its full evidence boundary is still open.

[OpenAPI Schema Generation v1](OPENAPI-V1.md) adds only the read-only
`semaprax openapi` projection of admitted monomorphic scalar signatures into a
deterministic canonical OpenAPI 3.1 document and the `semaprax openapi-compat`
exact-authentication plus classification lane over two such envelopes, with
pinned payload/digest knowledge-AT evidence in
`tests/openapi_generation_v1.rs`. It makes no Protobuf/gRPC, GraphQL, or SQL
claim, imports no schemas, runs no live conformance fixtures, hosts no
registry or server, executes no target, and remains scalar-only. The OpenAPI,
Protobuf/gRPC, GraphQL, and SQL row therefore moves from Missing to Partial.
At that milestone, totals were 40 Partial/16 Missing.

[Public Wasm Scalar Exports v1](WASM-SCALAR-EXPORTS-V1.md) exposes only
explicitly selected stable-ID monomorphic `i64`/`bool` functions from a
completely effect-free scalar program. It emits stable-ID-derived Core-Wasm
adapters, a canonical `semaprax.web.v4` digest manifest, generated frozen
JavaScript bindings and TypeScript declarations, and a calculator consumer.
Admission, deterministic package, Node execution, status normalization, and
stable-ID rename evidence qualify a bounded public JS/TS slice. Exact
TypeScript 5.8.3 consumer compilation and one Chromium loopback interaction are
hosted green at `d883ace579bfd86f723cdc6819224fde51f0677d` in [run 32523952912,
job 96901973072](https://github.com/wavect/semaprax/actions/runs/32523952912/job/96901973072). The JavaScript and TypeScript row
therefore becomes Partial, while Web, WebAssembly, Functions, and the milestone
dashboard remained Partial. At that milestone, totals were 39 Partial/17 Missing.

Semantic Workspace Operations v1 deepens the existing Partial human/agent
projection and atomic-agent-change rows without changing their status. It
authenticates one managed base AST/HIR occurrence sidecar and one candidate
graph to compile bounded explicit stable-ID declaration/direct import-alias
renames into one exact existing Change-v1 replacements proposal and canonical
derivation wrapper. Additive outer Evidence now binds the exact Operations
intent to unchanged Change-v1 Evidence and gates one exclusive immutable
publication route; it adds no Operations-native Context/Impact/Review or
reusable Evidence/receipt authority, path-set change, target/test execution,
provenance, or automatic identity selection. Exact replay inside apply mints
  only one invocation-local publication authority. The exact
  `dfc04278c6ba9a7dd247d4cc4add3af91f55b936` matrix is hosted green in [run
  31570834457](https://github.com/wavect/semaprax/actions/runs/31570834457);
  all 12 jobs passed, including the Operations process-termination gate on
  Ubuntu, macOS, and Windows. Current totals remain 39 Partial/17 Missing.

| Milestone | Status | Evidence boundary |
| --- | --- | --- |
| Persistent Project agent developer loop | Partial | Project Agent Transport v2 retains one authenticated multi-file Project graph/context/test session. Additive opt-in v3/v4 locally prove the bounded scalar rename/workflow route, while Project Manifest v2 adds the retained-authority Useful Text carrier. Project Manifest v3 now locally preserves one authenticated useful-data snapshot across profile-exact entry/test/export linking and an exact `semaprax.project-npm-build.v2` carrier; the installed binary-frame fixture executes without compiler access. Context-free carrier inspection proves compiler consistency only and grants no publication authority. Project Native Rust SDK v1 remains locally evidenced for its narrower profile. General or multi-file change, managed-Workspace publication, Windows v2 npm publication, exact-head hosted promotion, registry publication, recovery, and exactly-once delivery remain open. |
| Human and agent semantic projections | Partial | Canonical `.spx`, validated stable-ID HIR, hardened atomic single-file renames, and the program-wide Graph v10/v11/v12/v13/v14 lattice are executable. Additive Agent Context v2 preserves exact v1 default behavior and bytes while adding deterministic forward/reverse/both call traversal, global per-depth stable-ID order, minimum-depth direction provenance, and separate traversal/reference frontiers; local v2 and legacy-v1 gates are 8/8 and 8/8, and the full hosted matrix is green in [run 31397881268, Ubuntu job 93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327). V2 remains call-graph-only and does not itself claim general reverse semantic edges or impact analysis. Separate Semantic Impact v1 previews one Patch v1/v2 file read-only with exact source-consumer provenance and bounded reverse-call impact for exact generic-call instance changes; its canonical report KAT is `94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9`, and the exact `1b3731a` matrix is hosted green in [run 31408654657 attempt 2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2), including [Ubuntu job 93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404), while repository-wide/non-call impact remains open. Bounded Semantic Patch v2 adds persistent record/case-member and variant-case renames plus exact direct-scalar generic-call argument replacement under one pre-state transaction and a selective post-HIR semantic-delta gate; its exact `f95d243` full matrix is hosted green in [run 31401200449 attempt 2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2), including [Ubuntu job 93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044). It remains single-file with trusted patch provenance and no general repair/impact claim. Semantic patch A0 uses a canonical regular source, a cooperating create-new sibling lock, bounded create-new staging, exact source/stage byte and identity rechecks, and identity-aware cleanup; its full matrix is hosted green in [run 31396483313, including Windows job 93481068538](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068538). Unix device/inode identity is exact; Windows compares held same-file handles by volume plus the available 64-bit file index and does not claim ReFS 128-bit or hostile non-unique-index uniqueness. Any authenticated generic function declaration selects v14 above explicit record-pattern v13, generic-record v12, Option-propagation v11, and legacy/Result v10. V14 binds exact function templates, concrete instances, and call instances; unused templates select v14 without fabricating an executable instance. Recursive record/member/binding identity, immutable record update, generic Copy record/variant construction, exact structured concrete arguments, authenticated ordinary prelude types, and typed propagation meaning remain included. Predictable sibling collision/stale-lock DoS, crash-left locks, the final trusted-directory path window, power-loss durability, and multi-file typed repair/impact remain open |
| Ownership and cleanup meaning | Partial | Move/partial-place checks plus independently rebuilt and replayed CleanupPlan v2 plans, and feature-minimal v3 plans for bounded Option propagation, are executable, including exact body/Result-residual/Option-None Copy-result staging and shared postcondition/publication joins; general lifetimes, aliases, concurrency, FFI, and public physical cleanup remain open |
| Aggregate records v1 bounded execution | Partial | Construction/projection/update, stable IDs, checked Native64/Wasm32 layouts, frozen one-byte/alignment-one empty records, and cleanup are executable; nested public scalar records and exact-instance generic Copy records with ordered direct `i64`/`bool` arguments run through native C11 O0/O2 and Node/Wasm, with the generic-record gate hosted green in [run 31365363898, Ubuntu job 93383304995](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304995). Bounded irrefutable Copy-record matches now destructure exact nested/generic instances with scalar or whole-record bindings, ignored fields, one evaluation, scalar arms, program-wide Graph v13, unchanged straight-line CleanupPlan v2/v3, Native O0/O2, and 4,096-entry Node/Wasm; the Ubuntu gate is hosted green in [run 31373317800, job 93406925130](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406925130), and independent security review is green. One private shared-plan resource harness separately proves an exact cross-backend cleanup trace and zero liveness. Stable public aggregate ABIs, public resource-record execution, nested/resource/non-Copy generic breadth, refutable or ownership-aware matching, and general aggregate execution remain open |
| Copy variants + bounded generics/prelude/`?` | Partial | Nominal variants with explicit direct `i64`/`bool` arguments, ordinary compiler-owned `Option<T>`/`Result<T,E>`, exhaustive copy match, exact-instance layouts, and Native O0/O2 plus Node/Wasm are hosted green in [run 31347109201](https://github.com/wavect/semaprax/actions/runs/31347109201). Bounded postfix `?` for direct-scalar Copy `Result<T,E>` is hosted green in [run 31353051690](https://github.com/wavect/semaprax/actions/runs/31353051690), and the analogous Option tranche is hosted green in [run 31360176398, job 93367728277](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728277). Bounded explicitly instantiated effect-free generic Copy functions now have exact source/HIR/Graph-v14/native/Wasm evidence, including unused-template validation, concrete-instance separation, failure order, poison, and 4,096-entry Node re-entry; independent security review is green and the hosted matrix is green in [run 31385406865, Ubuntu job 93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). Inference, constraints, aggregate/resource/non-Copy generic signatures or arguments, generic-function `?`, non-copy propagation/matching, residual conversion, stable public aggregate ABI, callable/component signatures, and public resource admission remain open |
| Native code and interop | Partial | Scalar C11/Clang and bounded private callable/resource evidence exist. Private Native Rust Interoperability v1 A+B has a frozen scalar current-host static-link implementation with direct held compiler authority, a pre-effect 12-use process arena, exact Windows tool-root/linker handling without ambient child `PATH`, bounded preparation/replay, exact-inventory publication, and fail-stop settlement. Its Ubuntu/macOS/Windows, Rust 1.85, Windows runtime/capacity, and Linux sanitizer promotion gate is green at `50b96dccabe3b3dcbcdf38bab380f3eb8699184c` in [run 32402944574](https://github.com/wavect/semaprax/actions/runs/32402944574). Additive Public Native Rust SDK v1 Phase C exposes a narrow API from the still-unpublished builder and emits an exact nine-file, dependency-free local Cargo package with stable-ID methods, a safe scalar export/import facade, deterministic held archiving, an independently replayed outer manifest, and calculator/callback consumers. Project Native Rust SDK v1 locally extends that same package to one authenticated Project: a target-neutral exact subject binds canonical manifest, Project/workspace/graph, complete source, and exact export-origin facts; the already linked entry HIR reaches the builder without flatten/reparse; distinct Project descriptor/bundle/outer schemas bind target ABI and artifacts; and Web/Node plus Rust consumers preserve stable-ID behavior across the daemon rename. The Project and direct local gates are implemented, but exact-head Ubuntu/macOS/Windows promotion remains pending; no registry, root CLI, general Project SDK, import, capability, aggregate, or resource claim follows. Compiler sysroot/dynamic-library descendant provenance, independent linker-index semantic reconstruction, and exact linker-descendant image execution under a same-path race are not claimed. Resources, aggregates, borrowing, strings, async, general native execution, and C/Objective-C/Swift/Kotlin ecosystem import remain open. See [NATIVE-RUST-INTEROP-V1](NATIVE-RUST-INTEROP-V1.md) |
| Web and portable components | Partial | Existing scalar/Copy/resource Core-Wasm and private Component evidence remains bounded, and the Project-v1 Web/Chromium lane retains its hosted evidence and exact bytes. Project v2 locally proves the bounded borrowed-text package. Additive Project v3 now locally proves profile-exact `Slice<u8>` export linking, fixed-memory Core Wasm, strict snapshotting `Uint8Array` JS/TS, exact metadata/digest checks, and the installed six-file binary-frame npm package. Unix publication is safely handle-relative; Windows v2 publication remains fail-closed. Exact-head hosted promotion, registry publication, Components, and multi-engine conformance remain open. | General source/algebraic Component selection, resource/non-Copy public returns, imports/capabilities, callbacks/async, callable/FFI aggregate signatures, browser/multi-engine conformance, safe Windows v2 publication, and stable public Component API/ABI remain open |
| Desktop and mobile applications | Partial | Private macOS engine/AppKit ([job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230)), Windows engine/Win32 UI ([job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480)), Swift/iOS XCFramework/app ([job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228)), and Android JNI/Kotlin app ([job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206)) gates are green. Public SDKs, UI language, lifecycle breadth, signing/distribution, and device breadth remain open |
| Full SEMAPRAX product objective | Partial | No single lane proves native mobile + desktop + web + broad interop + full ownership/lifetime safety together; the global goal is not complete |

Project Agent Transport v2 deepens the existing Partial human/agent projection
row without changing its status. `semapraxd --stdio` binds one Project v1
manifest at startup, retains entry/test HIR plus one complete Project-specific
graph and typed context index from the same Phase-A build, and serves closed
revision-bound snapshot/check/graph/context/test requests. Pre/post request
held-input reauthentication is absorbing, strict raw framing and response caps
are executable, and Transport v1 remains byte-preserved. Additive opt-in
[Project Rename Transaction v1](PROJECT-RENAME-TRANSACTION-V1.md) reports
Transport v3 and locally proves one preview-digest-bound display rename of one
explicit-ID scalar function selected by `web_exports`: one complete candidate
Project validation, overlapping Project/A0 authentication, unchanged
single-file A0 commit, response preflight, exact reload, refreshed
graph/context/test, and unchanged stable-ID Web/Node consumption. Default v2
remains read-only. General/multi-file change, import-alias or identity rename,
client path/patch/evidence, build/network/disk-persistence authority,
cross-process revision indexing, recovery/exactly-once delivery, and exact-head
hosted promotion remain open. Local evidence is six black-box transport tests,
four injected session-boundary tests, and sealed planner/A0 units. Totals remain
56 Partial/0 Missing.

Additive [Project Agent Workflow v1](PROJECT-AGENT-WORKFLOW-V1.md) reports
Transport v4 without changing the Partial status or totals. It binds one
server-derived exported-function display rename to complete base/candidate
Project-specific typed Impact, fixed-section Review, the existing A0/reload
apply boundary, and one refreshed pathless Web-only inline build. The exact
seven-artifact carrier is independently replayed and locally materialized for
stable-ID Node execution. General patch/change authority, multi-file edits,
native/Rust daemon builds, request-selected outputs, persistence, recovery,
exactly-once delivery, and exact-head hosted promotion remain open. Current
totals remain 56 Partial/0 Missing.

Project Manifest v1 is a locally evidenced bounded build-input slice, not a
status transition. It authenticates one canonical explicit 2–16-source
`semaprax.toml` set, reuses the existing Semantic Workspace Phase-A build once
in memory without creating a managed workspace, and links real scalar provider
bodies by stable ID into one entry and one test closure. The full named set is
permit-, type-, interface-declaration-, interface/native-import-, generic-, and
effect-free, and only admits by-value `i64`/`bool` functions; explicit stable-ID
`use function` provider edges are the sole cross-file composition mechanism,
while reverse consumers, synthetic mains, and stubs are excluded. Native and
Web consume the same linked HIR, with the Web package binding
`semaprax.web-project.v1` project/Phase-A/artifact facts. Local
manifest-hostility, held-input drift, closure, native O0/O2, Web/Node, and
stable-ID rename gates exist. Project CLI publishes the digest-bound Web
package (default) and, additively, explicit create-new native executables
through Public Project Native Publication v1: one linked entry closure,
the unchanged shared Clang C11 lane, pre/post-publication held-input rechecks,
existing-destination rejection, deterministic entry C projections, and
stable-ID display rename preservation of published native behavior. Public
Project Developer Loop v1 runs the authenticated entry or exact sole manifest
test closure in process with bounded, digest-bound execution evidence; it
claims neither target execution nor test discovery. Exact-head hosted
promotion of the native-publication and developer-loop lanes remains held.
Post-publication drift is
`SPX-J103`: the complete retained output remains for caller reconciliation
and is never deleted automatically. It claims no general packages,
dependencies, registries, capabilities, interface/native imports or `use type`
edges, aggregates/resources, generics, test discovery, repository authority,
hostile-window no-clobber publication, cross-build executable byte determinism,
or production packaging. Totals remain 39 Partial/17 Missing.

Project Manifest v2 is an additive locally evidenced profile over the same
held Project authority. It binds canonical version/profile metadata, admits
the Useful Text Consumer borrowed-`str` boundary, links every selected
stable-ID export root, and emits exact Web/JS/TS plus a six-file offline npm
package and independently replayed pathless carrier. Stable-ID display rename
is preserved and Project-v1 bytes are unchanged. Exact-head hosted promotion,
general text or collection semantics, dependencies, and npm registry
publication remain open; dashboard totals do not change.

Project Manifest v3 additively selects the closed `useful-data.v1` profile and
preserves v1/v2 canonical bytes. One authenticated snapshot links profile-exact
entry, test, and selected stable-ID export closures, reconstructing exact byte
operation, provenance, capacity, and cleanup facts. Local evidence covers
fixed-memory Core Wasm, a strict snapshotting `Uint8Array` JavaScript/TypeScript
facade, the exact six-file `semaprax.project-npm-build.v2` carrier, tamper
replay, offline pack/install, and compiler-free installed use of the
multi-module binary-frame fixture. Unix publication is handle-relative,
create-new, and no-clobber; Windows v2 publication remains fail-closed. Hosted
promotion, safe Windows publication, npm registry publication, and release
promotion remain open; dashboard totals do not change.

The Human and agent semantic projections dashboard also includes bounded
Semantic Workspace Transaction v1. It authenticates 2–16 canonical sources,
preflights unchanged admitted per-file patches, publishes one complete
immutable generation, and pivots only `ACTIVE` for cooperating locked readers.
Integration 12/12, hostile wire/CLI 5/5, workspace units 37/37, library
482/482, full local gates, preservation, and security are green. The exact
`afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted green in [run
31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
including [Ubuntu job
93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
and [Windows job
93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
strict Clippy and is not green evidence. Raw source/Git/editor atomicity, cross-file semantics, repository
Graph/analysis, create/delete/move, recovery/GC, and power-loss durability
remain open, so the dashboard stays Partial and current totals remain 39
Partial/17 Missing.

The same dashboard now includes additive Semantic Workspace Patch Evidence v1.
It binds one exact Workspace Patch and preview to sorted per-file Review and
child Patch-Evidence-v1 facts, independently replays a canonical outer capsule,
and gates an opt-in Workspace apply route before candidate/staging creation.
Local public 6/6, apply 5/5, hostile 2/2, units 8/8, shared Workspace 39/39,
root library 496/496, and preservation 107/107 are green. The exact
`cda4892ee74100fd11c5161ad857d469ec5e5421` matrix is hosted green in [run
31491573287](https://github.com/wavect/semaprax/actions/runs/31491573287), with
all 12 jobs passing. The artifact grants no authority, performs no cross-file
semantic reasoning or target/test execution, and adds no Target Evidence v1 or
Evidence v2 aggregation. General repository Graph/analysis, semantic
resolution, provenance/approval, consumer compatibility, raw-tree publication,
and recovery/durability remain open. No status changes: totals remain exactly
39 Partial/17 Missing.

The dashboard also includes the additive, locally evidenced Semantic Workspace
v1, Workspace Semantic Graph v1, Workspace Analysis v1, and Semantic Workspace
Change v1 contracts. They deepen—but do not complete—the existing Human and
agent semantic projections, Atomic agent changes, Token-budgeted semantic
context, Impact analysis before modification, Proof-carrying patches, and
Semantic human review rows. The bounded lane authenticates 2–16 managed files,
resolves explicit direct function/type imports in one unified build, projects
six typed edge families, emits read-only Context/Impact/Review, and binds a
2–16-file replacements-only proposal to full managed-graph delta Evidence.
Verification receipts grant no authority. Apply freshly replays proposal and
Evidence under one exclusive lock before no-clobber candidate publication and
the sole ACTIVE pivot. Local public C3 is 10/10 and private authority evidence
is 11/11; exact-head hosted Ubuntu/macOS/Windows promotion is pending.

This bounded result does not satisfy the general completion gates: no
create/delete/move, raw-tree/Git/editor atomic publication, target/project test
execution, signing/provenance/approval, general package/reexport or
resource/interface/ownership composition, persistence/incrementality,
automatic recovery/GC, external compatibility, or power-loss durability is
proved. Process termination proves only tested OS lock release and
authenticated old/new managed state. Status cells are unchanged and totals
remain exactly 39 Partial and 17 Missing.

The human/agent projection dashboard now also includes the bounded, hosted
Diagnostic Repair v1 milestone described in the detailed repair row below:
canonical query/preview JSON, one `breaking_identity_rebase`, and the isolated
single-operation Semantic Patch v3 apply path through unchanged A0. Phase A is
13/13 integration; the Phase B semantic integration corpus is 7/7; v3 A0 hook
units are 4/4; aggregate v3 integration-plus-hook evidence is 9/9; and library
404/404, full-preservation, and security gates are green. The exact `dae957a`
full matrix is hosted green in [run 31418476217 attempt
1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1),
including [Ubuntu job
93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265);
all 12 jobs passed. This does not change the dashboard
status or its general/multi-file repair nonclaims.

The same Partial dashboard now also includes locally implemented Bounded
Semantic Review v1. Patch v1/v2 reports embed complete, nontruncated Impact v1
evidence; the sole canonical Patch v3 report embeds the exact shared identity
rebase and no Impact object. Seven fixed evidence-linked sections map the RFC
behavior/API/security/memory/target/migration/unsafe concepts without renaming
the wire keys. Exact Patch v1/v2/v3 report KATs are
`054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`,
`37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and
`081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`.
Review integration 10/10, hook/limit units 4/4, library 408/408, full
preservation, and security gates are green. The exact
`2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in [run
31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
including [Ubuntu job
93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
all 12 jobs passed. This
adds no Context, target/test execution, verifier/proof artifact, authenticated
patch provenance, human approval, or A0 authority, and it does not change the
dashboard status.

Semantic Patch Evidence v1 adds one bounded proof carrier and moves only the
Proof-carrying patches row from Missing to Partial. Patch v1/v2 capsules bind
complete nontruncated Impact-v1-backed Review evidence; the sole canonical
Patch v3 capsule binds the shared identity rebase. Independent verification
requires exact typed and byte replay. The separate `patch-with-evidence` route
acquires the unchanged A0 lock and requires replay before staging; ordinary
`patch` remains evidence-optional. A+B is 11/11 integration plus 5/5 internal
units, Phase C is 16/16 integration plus 11/11 hook/limit units, and library
420/420 plus doctest 37/37 are locally green. The exact
`34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green
in [run
31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
including [Ubuntu job
93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97 lint
and is not green evidence. Review itself remains
byte-identical, read-only, without `review::verify`, and non-proof; its KATs
and nonclaims do not change. The current matrix is 39 Partial and 17 Missing.

[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) phases 1–2 are
implemented. Every resolved function carries an independently rebuilt
target-neutral cleanup CFG with storage/leaf liveness, regions, atomic call
commits, sticky failures, guarded finalization, and result publication; Graph
v10 serializes CleanupPlan v2, while authenticated bounded Option propagation
uses program-level Graph v11 and per-function CleanupPlan v3 without migrating
legacy/Result-only output; any authenticated generic record declaration takes
program-level Graph v12 precedence, and any authenticated explicit record
pattern takes Graph v13 precedence, and any authenticated generic function
declaration takes Graph v14 precedence above the whole lower lattice, while
retaining the appropriate per-function cleanup schema. Generic function
instances keep canonical CleanupPlan v2 template-ID-only; exact instance
authentication belongs to HIR and Graph v14. Wildcard-only record matches do
not select v13. Phase-3 evidence now includes status/trace types, independent
plan replay, a reference executor, native scalar status/out execution,
deterministic host templates and [descriptor-only
v1](NATIVE-ADAPTER-DESCRIPTOR-V1.md), authenticated capability mechanics, and a
real unpublished native host that connects the v1 descriptor, exact loader
lease, same-thread OS-seeded authority, ownership ledger, and opaque owners in
Linux/macOS fixtures. Private [callable descriptor
v2](NATIVE-CALLABLE-ABI-V2.md) now binds compiler-derived execution/cleanup,
event-dictionary, and trace-path-certificate fingerprints plus exact symbols,
capacities, signature, and result. The compiler emits the complete guarded C11
provider; the ownership host independently parses and authenticates the
descriptor, dictionary, and trie-DFA certificate, then invokes the exact
instance-bound callable through its authority and atomic ledger.

WebAssembly separately implements the narrow [owned ABI
v1](WASM-OWNED-ABI-V1.md) for one direct trivial-resource identity with real
Node execution. Generated native C and Wasm now emit deterministic
dictionary-authenticated semantic ordinals for the same authoritative 14-case
corpus. Real generated native shared libraries execute through the physical
ownership host at O0/O2, and native/Node-Wasm both match the exact reference
trace, outcome, publication, and final logical liveness. Every excluded Wasm
shape remains `SPX-W111`. Public native resource execution remains blocked by
fallback cleanup/quiescence generalization, production Android/iOS profiles,
and execution/admission; `SPX-B104` remains closed. The bounded Linux Rust-host
ASan requirement is green in [public run 31259216533, job
93107277065](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277065).
That run also proves the build-only callable bundle on
[Ubuntu](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277094),
[macOS](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277081),
and [Windows](https://github.com/wavect/semaprax/actions/runs/31259216533/job/93107277085),
without implying application-platform or public-execution support.
The build-only public callable-v2 API/CLI emits one deterministic hashed bundle
for a selected direct-trivial owned function. The generated corpus and hardened
dependency isolation passed on Windows in [run 31257545008, job
93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756).

The proposed [RFC 0004 native call recovery and settlement
contract](RFC-0004-NATIVE-CALL-SETTLEMENT.md) defines a bounded callable-v3
frame/checkpoint/settlement/receipt foundation for the physical-failure and
quiescence gap. Its hidden target-neutral model and private compiler derivation
from validated cleanup HIR are implemented for the current direct-trivial
owned slice. A separately versioned private `SPXNPRF1` envelope now binds the
exact v2 call contract and trace certificate to a bounded binary graph, which
the host parses independently without loading or executing it. The separate
private `SPXNABI3` contract now fixes descriptor/hash/graph/capacity and
linkage metadata plus seven independently encoded/parsed bounded runtime
codecs: a six-argument execute ABI, payload-bearing frame, exact tags/digests,
candidate, and host-only HMAC receipt. `CertifyOutcome` binds an embedded
ordinal/outcome witness to
the trace-certificate fingerprint through a nonzero host-recomputed digest;
this is not independent host acceptance of the trace-path DFA certificate, and
resealed witness/digest mutations are rejected. The ordinary machine-code
emitter is build-target-bound with no public/general cross-target configuration;
a hidden closed selector emits complete target-bound iOS evidence providers for
five enumerated targets and closed arm64/x86_64 Android dynamic providers with
exact Bionic/ELF guards. Graph-derived private
providers now execute all 14 authoritative normal scenarios through exact
dynamic-image admission and the receipt ledger at `-O0`/`-O2`. That joint path
proves exact descriptor/instance binding, pre-settle copied-evidence validation,
replay, generation refresh, finalizer order, pin lifetime, and zero measured
Rust heap growth from immediately before `CallCommit` through `ReceiptCommit`.
Injected decode-reserve failure quarantines exact evidence/pins, and seven
joint failure/interruption fixtures cover physical return, malformed wires,
durable boundaries, replay, and decision conflict. Canonical pre-execute unwind
skips provider execute and reaches authenticated abort receipt commit. A
bounded static-registration model feeds the same ledger in non-Apple
fake-function evidence. A mandatory macOS gate now requires this static-only
composition to type-check for five distinct iOS device, simulator, and Catalyst
Rust targets, with dynamic loader and desktop v1/v2 surfaces absent. The same
mandatory job is configured to link and execute one exact arm64 iOS Simulator
`token.discard-two` provider through static registration and the authenticated
ledger at `-O0`/`-O2`; [run 31318280135, job
93257002836](https://github.com/wavect/semaprax/actions/runs/31318280135/job/93257002836)
proved that bounded path. The standalone-process slice does not prove device or
app lifecycle execution, the remaining corpus on iOS, exhaustive crash/fatal-
allocator failure injection, Android device execution, quiescence,
malicious-code containment, physical-finalizer generality, or public admission.
It is not the Android APK/JNI gate; that separate green hosted evidence is
recorded below. `SPX-B104` remains closed.
The mandatory Android job compiles the loader/host and exact
providers for x86_64 and arm64, then runs one x86_64 `token.discard-two`
dynamic provider through the same receipt ledger at O0/O2 in an API-35
emulator. [Run 31320436726, job
93262427248](https://github.com/wavect/semaprax/actions/runs/31320436726/job/93262427248)
is green. This bounded native-process evidence does not by itself satisfy the
Android application row's JNI/Kotlin/APK/lifecycle/UI gate.

A separate private [Android JNI ownership adapter
v1](ANDROID-JNI-OWNERSHIP-V1.md) is now implemented and CI-configured. Local
Rust/C tests and source locks cover the closed `RegisterNatives` table,
`SPXAJH01` handle ownership, `SPXAJS01` status/exception normalization,
HandlerThread confinement, deterministic `PhantomReference` cleanup action,
poison-preserving outputs, exact finalizer evidence, and the plugin-free offline
APK packaging contract. The same-package no-UI Instrumentation APK is configured
to install on API 35 x86_64 and exact-match one app-private result after O0
explicit `consume()` and O2 Cleaner paths; arm64 is compile/ELF inspection only.
The exact APK build/install/Instrumentation path is green in [run 31338834586,
job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206).
This moves only the Java/Kotlin and Android rows to **Partial**; it proves no GC collection,
process-exit cleanup, AAR, UI/accessibility, general lifecycle, device or arm64
runtime, public ABI/admission, or `SPX-B104` change.

The private [Apple Swift ownership adapter
v1](APPLE-SWIFT-OWNERSHIP-V1.md) and [WIT boundary
v1](WIT-COMPONENT-BOUNDARY-V1.md) are implemented with local Rust/Node and
source-lock evidence. Swift/iOS is **Partial** for the closed same-thread
wrapper and green bounded XCFramework/Simulator-app gate in [run 31338834586,
job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228).
WIT is **Partial** for deterministic schema/adapter output, a separate
independently parsed scalar Component Model fixture, checked v2 composition,
and private Portable Result Component v3. V3 binds the exact generated scalar
core to `result<s64, status>`, passes independent and upstream validation, and
has local typed Wasmtime evidence for success, addition overflow, division by
zero, false precondition, and false postcondition with zero imports, an empty
linker, and no WASI. Poison preservation and sticky status selection remain
separately frozen at the generated-core boundary. Its isolated runtime graph
cannot widen the public compiler graph or MSRV. The current prelude-bound KAT
migration and standalone runner are hosted green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212).
The separate private Source-Result Component v4 connects one exact
effect-free source closure using compiler-owned `Result<i64, bool>`, postfix
`?`, and `Result<bool, bool>` to WIT 0.2
`result<result<bool, bool>, status>`. Its source/core/profile/component and
layout-v2 bindings have frozen KATs; independent and upstream validators,
every-byte/cross-profile hostile tests, generated-core Node outcomes, default-
consumer hiding, and CI source locks are green. The isolated runner executes
typed inner `Ok`/`Err`, outer status, residual short-circuit, sticky failure,
shared postconditions, re-entry, fresh instances, and fuel failure in [run
31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).
Private Scalar Algebraic Component v5 separately freezes the six direct-copy
`Option`/complete `Result`-matrix exports, exact layouts and identity mapping,
hostile profile closure, and zero-import runner source/KATs; pinned Rust 1.97.1
typed execution is hosted green in [run 31360176398, job
93367728269](https://github.com/wavect/semaprax/actions/runs/31360176398/job/93367728269).
Private Nested Record Component v6 separately freezes WIT package
`semaprax:private@0.4.0`, interface `nested-records`, world
`semaprax-private-v6`, and one fixed nested scalar-record `transform` export.
Exact source/core/Inner-layout/Outer-layout/profile/component/DAG KATs,
independent/upstream validation, hostile closure, local generated-core
execution, default-consumer hiding, source locks, and security review are
green. Its pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in [run
31365363898, job
93383304974](https://github.com/wavect/semaprax/actions/runs/31365363898/job/93383304974).
Private Generic Record Component v7 separately freezes WIT package
`semaprax:private@0.5.0`, interface `generic-records`, world
`semaprax-private-v7`, and four exact exports over ordered `Duo<i64, bool>`,
`Duo<bool, i64>`, `Phantom<i64>`, and `Phantom<bool>` instances. Exact source/
core/layout/Graph-v12/plan/profile/component identity, same-layout Phantom
separation, independent/upstream validation, hostile closure, local generated-
core Node execution, default-consumer hiding, source locks, and security review
are green. Its pinned Rust 1.97.1/Wasmtime 47 typed runtime is hosted green in
[run 31373317800, job
93406924922](https://github.com/wavect/semaprax/actions/runs/31373317800/job/93406924922).
Private Record-Pattern Projection Component v8 separately freezes WIT package
`semaprax:private@0.6.0`, interface `record-pattern-projections`, world
`semaprax-private-v8`, and four ordered monomorphic preserve/invert exports over
the distinct same-layout `Phantom<i64>` and `Phantom<bool>` instances. Exact
source/core/two-layout/Graph-v13/plan/profile/component/DAG KATs,
independent/upstream validation, all-pair identity-swap rejection, generated-
core Node execution, poison/invalid-value closure, default-consumer hiding,
source locks, strict gates, and security review are green. Its pinned Rust
1.97.1/Wasmtime 47 typed runtime is hosted green in [run 31385406865, job
93445428268](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428268).
Private Generic-Function Instance Component v9 separately freezes WIT package
`semaprax:private@0.7.0`, interface `generic-function-instances`, world
`semaprax-private-v9`, three phantom Copy templates, and six exact ordered
Graph-v14 `FunctionInstanceId` exports with identical scalar WIT signatures and
no record/layout roots. Source/Graph/core/plan/profile/raw/DAG KATs,
independent/upstream and every-byte validation, all 15 pair-swap rejections,
local core 5/5, component 4/4, CI-lock 4/4, full gates, and security review are
green. Its pinned Rust 1.97.1/Wasmtime 47 zero-import typed runtime is hosted
green in [run 31392541096, job
93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492).
Private Source-Option Propagation Component v10 separately freezes WIT package
`semaprax:private@0.8.0`, interface `option-propagation`, world
`semaprax-private-v10`, and one exact compiler-owned `Option<i64>` through
postfix-`?` to `Option<bool>` export. Source/Graph-v11/prelude/two-layout/
CleanupPlan-v3/core/profile/raw/DAG KATs, typed/raw `Some`/`None`, contracts,
arithmetic, sticky failure, poison/tag/status hostility, repeated/fresh
instances, CI locks, full gates, and security review are green. Its pinned Rust
1.97.1/Wasmtime 47 zero-import v3-v10 runtime is hosted green in [run
31396483313, job
93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502).
V1-v9 bytes remain unchanged. General source
`Result`/`Option`/`?`
component mapping, general source selection, general/empty/nested/resource/non-
Copy record mapping, general generic-function components, imports/capabilities,
callable/FFI aggregate signatures, public component API/ABI, and `SPX-B104`
remain absent.
The hidden linear phase model now starts from the sole authenticated
post-`CallCommit` state and exercises exact `SettlementDecisionCommit`,
provider-candidate, model-`ReceiptCommitted`, and absorbing `Quarantined`
evidence. Its 29 focused tests cover phase-aware unwind,
every-finalizer interruption, exact candidate/committed replay, hostile
cross-binding and state mutation, and preserved evidence. It deliberately
allocates and grants no exact-instance reservation, host authentication, ledger
publication, provider/FFI, loader retention, or physical finalizer authority.
Those physical gates remain required; this adds no native runtime evidence to a
completion row and leaves `SPX-B104` closed.

The dedicated Linux
[callable-host sanitizer job](https://github.com/wavect/semaprax/actions/runs/31256134955/job/93099637801)
is green for all 14 O0/O2 dynamically loaded ASan/UBSan provider cases. It did
not instrument the Rust host code. The dependency-policy job was also green,
but unrelated Clippy/GCC failures kept that historical overall workflow red; it
is not the later Windows evidence linked above.

## Defining product contract

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Agent-native semantic program | Partial | Graph v10 serializes validated legacy/Result HIR plus complete CleanupPlan v2 plans; v11 authenticates bounded Option propagation, v12 authenticated generic records, v13 explicit record patterns, and highest-precedence v14 any authenticated generic function declaration. V14 carries exact function-template, function-instance, and call-instance identities and ordered arguments. A same-schema v14 correction adds missing array delimiters around function-template parameters; two-parameter templates previously produced invalid JSON in module and context projections. Unused templates select v14 without materialization, all contexts report the program schema, and lower v10-v13 bytes remain unchanged without a generic function. Bounded Semantic Patch v2 now admits exact persistent record/case-member and variant-case renames plus addressed scalar generic-call argument replacement, with pre-state atomic selection and a post-HIR semantic-delta gate; it is still single-file and patch-file provenance is trusted. Its focused suite is 9/9 and the exact `f95d243` matrix is hosted green in [run 31401200449 attempt 2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2), including [Ubuntu job 93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044). Semantic Impact v1 adds a canonical read-only single-file preview with processed-patch digest, source-consumer facts, exact call-change seeds, and bounded reverse callers; its KAT is `94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9`, and the exact `1b3731a` matrix is hosted green in [run 31408654657 attempt 2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2), including [Ubuntu job 93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404). The migrated exact v14 module/Agent Context/bounded-context SHA-256 KATs are `7a61fa6229f2db7aca6a035fd961720e8a401c138cc66c9cd71c64d45bed5efd`, `2841401e7ba85fa8e47b3c35a15ae401b4a271d2500d70bbf3627f1453869eb6`, and `d7bda2be1fc366195ffb00a9e20b2b03204b4dd6f46e8019842dd84f70b54ab8`. Corrected JSON parse/KAT evidence is locally and hosted green in [run 31390043736, Ubuntu job 93459346296](https://github.com/wavect/semaprax/actions/runs/31390043736/job/93459346296). Separate generic execution/backend evidence was hosted in [run 31385406865, Ubuntu job 93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338), before this serializer correction | Versioned multi-file graph API covers callers, targets, tests, packages, generated artifacts, typed repairs, impact, semantic review, and authenticated patch-file provenance |
| Human-readable program | Partial | Canonical `.spx` source and formatter | Complete language round-trips deterministically; graph-aware merge/diff, debugger source mapping, and normal Git/editor workflows are verified |
| Meaning in, verified machine code out | Partial | Typed scalar core, owned `string`, checked integers/floats/chars, effects, native status/out contracts, and poison-preserving publication are executable. [Useful Text Consumer v1](USEFUL-TEXT-CONSUMER-V1.md) adds locally evidenced non-escaping `borrow str` inputs, four compiler-owned read operations, interpreter invocation-root provenance, cleanup-inert HIR/Graph, native O0/O2 pointer-plus-length execution over host-provided readable storage, and fixed-scratch range- and UTF-8-validated Wasm/JS behavior with embedded NUL. Legacy scalar/Web bytes are preserved. Exact-head hosted promotion, `usize`, arrays/slices, indexing, iteration, mutable/general text processing, and general borrowed/heap-backed aggregates remain open. | All safe-language guarantees survive every backend; native artifacts and portable components pass conformance suites on every supported target |
| Meaning in, verified machine code out | Partial | The locally evidenced `u8` scalar retains checked 0..=255 arithmetic, one-byte Native64 layout, Graph, native O0/O2, and Node/Wasm equivalence. [Portable Indexed Byte Data v1](PORTABLE-INDEXED-BYTE-DATA-V1.md) adds target-independent semantic-u64 `usize`, fixed `[u8; N]`, uniquely owned immutable `Bytes`, non-escaping `Slice<u8>`, six closed byte operations, exact capacity/provenance/cleanup facts, Graph v17, interpreter, native O0/O2, and internal Core-Wasm/Node execution. Indexed Byte Loop v2 locally adds exact compiler-owned `byte_get -> Option<u8>` matching for true dynamic cleanup-inert traversal. Project v3 locally adds the bounded public `useful-data.v1` Project/npm adapter, strict `Uint8Array` facade, and installed multi-module binary-frame acceptance. Complete hostile cross-platform evidence, safe Windows v2 publication, exact-head hosted promotion, registry publication, and release promotion remain open, so this is not a complete Useful Data claim. | All safe-language guarantees survive every backend; native artifacts and portable components pass conformance suites on every supported target |
| Atomic agent changes | Partial | Single-file stable-ID function/resource renames update calls and ownership type boundaries with domain-separated SHA-256 stale/legacy revision rejection. Bounded Semantic Patch v2 additionally authenticates exact persistent owner/member/case identities and revision-scoped generic-call tuples against one pre-edit HIR, preserves shorthand binding/Place identity, groups multi-index call changes into one derived instance, and rejects semantic deltas outside the selected identities. Its focused suite is 9/9 and its exact full matrix is hosted green in [run 31401200449 attempt 2](https://github.com/wavect/semaprax/actions/runs/31401200449/attempts/2), including [Ubuntu job 93505622044](https://github.com/wavect/semaprax/actions/runs/31401200449/job/93505622044). It changes neither Graph v10-v14 nor CleanupPlan v2/v3 and still trusts patch-file provenance. A0 authenticates a canonical regular source, serializes cooperating writers through a create-new sibling lock, uses bounded create-new staging, preserves permissions and syncs staged bytes, rechecks exact source identity/bytes/revision and stage path/handle identity/bytes at both final boundaries, and never cleans a foreign replacement. Internal race/failure/path-swap tests are 5/5, integration patch tests are 17/17, and the full matrix is hosted green in [run 31396483313, including Windows job 93481068538](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068538). Unix device/inode identity is exact; Windows held same-file volume plus 64-bit file-index comparison does not claim ReFS 128-bit or hostile non-unique-index uniqueness. Predictable-name collision/stale-lock DoS, crash-left locks, the trusted final path window, power-loss durability, general typed repair/impact, and multi-file commits remain nonclaims | Typed, transactional multi-file edits support every public semantic operation and either commit fully or leave all source/graph state unchanged |

Semantic Workspace Transaction v1 adds bounded evidence to both the
Agent-native semantic program and Atomic agent changes rows without changing
either Partial status. The public path-set/manifest/snapshot/preview protocols
and live initialize/apply route authenticate 2–16 canonical pre-existing
sources, preserve existing per-file Patch v1/v2/v3 semantics, publish one
complete immutable generation, and pivot only `ACTIVE` for cooperating locked
readers. Exact KATs and local integration 12/12, hostile 5/5, workspace units
37/37, library 482/482, full preservation, and security are green. The exact
`afde3b3302e0f88fd8af3278efaf0ddd72e6dfe7` matrix is hosted green in [run
31472847068](https://github.com/wavect/semaprax/actions/runs/31472847068),
including [Ubuntu job
93719800613](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800613)
and [Windows job
93719800611](https://github.com/wavect/semaprax/actions/runs/31472847068/job/93719800611);
all 12 jobs passed. Earlier run 31471716036 on `4daa407` failed only Windows
strict Clippy and is not green evidence. The Agent-native gate remains open for a versioned multi-file Graph
covering callers, targets, tests, packages, artifacts, repairs, review, and
provenance. The Atomic gate remains open for every public semantic operation,
cross-file resolution, raw source/Graph publication, create/delete/move,
repository integration, recovery/GC, and power-loss durability. Original
sources are not rewritten; managed readers see old or new through `ACTIVE`.

Diagnostic Repair v1 adds a bounded agent-native operation without changing
the two Partial statuses above. Its canonical Patch v3 authenticates one exact
automatic-function identity assignment, replays the complete
`breaking_identity_rebase`, and commits through the same A0 path. It adds no
other public semantic operation, multi-file transaction, authenticated patch
provenance, Graph or CleanupPlan schema/version or semantic-shape widening,
Graph v11-v14 repair admission, or backend/runtime semantic change. The
admitted Graph-v10 revision/identity/callee/derived-ID content changes and
identity-bearing CleanupPlan content may rebase. The exact `dae957a` full
matrix is hosted green in [run 31418476217 attempt
1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1),
including [Ubuntu job
93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265);
all 12 jobs passed.

Bounded Semantic Review v1 adds a read-only agent projection without changing
the Agent-native row's Partial status. It accepts Patch v1/v2 through complete,
nontruncated embedded Impact v1 evidence and the sole canonical Patch v3
through the shared identity-rebase evidence. It adds no semantic operation,
Context, target execution, verifier/proof artifact, provenance authentication,
or commit authority. Local 10/10 integration, 4/4 hook/limit units, library
408/408, full preservation, and security gates are green. The exact
`2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in [run
31423743369 attempt
1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1),
including [Ubuntu job
93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170);
all 12 jobs passed.

Semantic Patch Evidence v1 adds a bounded agent proof carrier and an opt-in
evidence-gated A0 route without changing the Agent-native or Atomic agent
changes rows from Partial. It adds no Patch operation, Graph/CleanupPlan
schema or semantics, backend/runtime meaning, repository graph, multi-file
transaction, or authenticated provenance. Ordinary `patch` remains unchanged.
Local A+B is 11/11 plus 5/5; Phase C is 16/16 plus 11/11; library 420/420,
doctest 37/37, full preservation, and security are green. The exact
`34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted
green in [run
31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632),
including [Ubuntu job
93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949);
all 12 jobs passed. The earlier `e04c2c9` run failed only the Rust 1.97 lint
and is not green evidence.

Semantic Workspace Patch Evidence v1 additively aggregates exact per-file
Evidence-v1 bindings over the already admitted managed Workspace transaction.
Its outer capsule and receipt freeze homogeneous-v1/v2/v3 and mixed KATs; the
evidence-gated route acquires the exclusive permanent lock first and requires
exact replay before candidate generation or staging, then enters the unchanged
Workspace commit core. Local public generation/verification is 6/6, apply 5/5,
hostile 2/2, module units 8/8, shared Workspace 39/39, root library 496/496,
and preservation 107/107. Full local gates/security are green and hosted exact-
head evidence is pending. It grants no authority, aggregates neither Target
Evidence nor Evidence v2, and proves no cross-file semantics, repository
analysis, target/test execution, provenance/approval, safety, compatibility,
raw-file atomicity, recovery, or durability. It does not change the existing
Proof-carrying patches, Agent-native, or Atomic agent changes statuses.

Semantic Target Evidence v1 and additive Patch Evidence v2 do not move another
row or widen the existing Partial status. They bind exact base/candidate Graph,
typed zero capability delta, production C11 source, and structurally validated
Wasm core projections; Evidence v2 independently replays that report before
the same lock-first A0 staging boundary. Target 9/9, target units 4/4,
Evidence-v2 8/8, library 439/439, full local gates, and security are green. The
exact `fcdf3861d79faea27c526a8dc5105b92c6738213` matrix is hosted green in [run
31440359793](https://github.com/wavect/semaprax/actions/runs/31440359793),
including [Ubuntu job
93624123631](https://github.com/wavect/semaprax/actions/runs/31440359793/job/93624123631);
all 12 jobs passed. Reports/capsules execute no target or project test,
grant no authority, and prove neither safety nor compatibility. Those
artifacts still have no repository or multi-file scope. The separate Workspace
Transaction v1 tranche supplies only bounded managed-generation publication;
general repository/multi-file Graph semantics remain open. Totals remain
exactly 39 Partial and 17 Missing.

## Language and safety

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Records and algebraic variants | Partial | Canonical records/variants, bounded explicit direct-scalar generic instances, ordinary `Option`/`Result`, exhaustive Copy-variant matching, irrefutable Copy-record destructuring, direct-scalar postfix `?`, exact instance/layout/symbol identity, ownership facts, independently replayed cleanup, and the Graph v10-v13 aggregate lattice execute through their documented native/Wasm gates | Public resource-bearing record execution/admission, nested or resource generic arguments/fields, resource- or record-bearing variant payloads, refutable/literal/guard/or/rest/nested-variant patterns, non-Copy ownership-aware matching/propagation, residual conversion, generic-function use of aggregate syntax or `?`, callable/component aggregate signatures, stable public aggregate ABIs, and general native/Wasm aggregate execution verified; ordinary `SPX-B104`/`SPX-W111` gates remain closed |
| Functions, closures, interfaces, implementations, generics | Partial | Monomorphic named functions plus bounded explicitly instantiated generic Copy functions with one or two owner/index-stable parameters, direct scalar/own-parameter by-value signatures, explicit `i64`/`bool` arguments, exhaustive unused-template validation without materialization, exact concrete HIR/Graph-v14/native/Wasm identities, local C11 O0/O2 and 4,096-entry Node evidence, and clean security review; the hosted matrix is green in [run 31385406865, Ubuntu job 93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). Public Wasm Scalar Exports v1 makes an explicitly selected monomorphic scalar subset callable by persistent identity through generated JS/TS bindings while rejecting every generic, aggregate, resource, import, or effectful profile. Generic records/variants and declaration-only resource interface/import contracts remain separate evidence | Inference, constraints, aggregate/resource/non-Copy signatures, generic-to-generic calls, recursion, effects, generic entrypoints, callable imports, closures, coherent implementations, specialization boundaries, and separate compilation verified |
| `Option` and `Result`; no null or unchecked exceptions | Partial | Ordinary compiler-owned `semaprax.prelude.v1` variants with explicit direct `i64`/`bool` arguments, exhaustive copy matching, and native C11 O0/O2 plus Node/Wasm execution. Bounded Result and Option postfix `?` preserve one evaluation, shared postconditions/publication, status separation, and poison. Option uses CleanupPlan v3 and program-bound Graph v11, raised to v12 by a generic record, v13 by an explicit record pattern, or v14 by any generic function declaration. A separate exact private Component v10 maps one `Option<i64>` through postfix `?` to `Option<bool>` and is hosted green in [run 31396483313, job 93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502) | Nested/resource arguments, general/non-copy `?`, generic-function `?`, residual conversion, general/public FFI or Component mappings, non-copy ownership modes, and a stable public aggregate ABI verified |
| Immutable-by-default values and explicit mutation | Partial | [Explicit Mutation v1](EXPLICIT-MUTATION-V1.md): `let mut` locals plus statement-only `<binding> = <expr>;` with exact-type checking, immutable-by-default diagnostics `SPX-U101`-`SPX-U106`, canonical round-trip, deterministic additive Graph serialization with pinned non-mutation bytes, CleanupPlan v2 unchanged for straight-line mutation, and native C11 O0/O2 plus Node/Wasm execution evidence in `tests/explicit_mutation_v1.rs` | Field, collection, reference/mutable-borrow, and cross-task mutation rules verified |
| Unique ownership and move safety | Partial | Explicit trivial/imported lifecycles; move/partial-place analysis; a pinned control-flow regression battery (tests/ownership_control_flow_v1.rs) covering lazy-boolean operands, match scrutinees, refutable-match guards, branch joins, while-loop admission, mutation boundaries, and `?` with live resources; replay-validated cleanup plans; hostile-HIR parity; and a private exact-instance native callable host with its separately bounded evidence. The internal byte-data tranche adds a one-leaf uniquely owned immutable `Bytes` lifecycle with canonical `core.bytes.drop`, plan replay, exact move/failure settlement, and local interpreter/native/Wasm execution. Existing callable evidence includes exact reference/native-host-O0/O2/Wasm outcomes, traces, publication, and final logical liveness for all 14 cases, plus its hosted sanitizer gates. | Open the public native gate only after general physical/malformed-response fallback cleanup and quiescence and mobile evidence, expose the public byte-data adapters, then extend exactly-once/double-free proof through loops, closures, concurrency, and FFI ownership |
| Borrowed views and lifetime safety | Partial | Non-consuming `borrow` boundaries and move-after-borrow behavior, plus locally evidenced immutable byte views with exact named-root provenance and conservative lexical owner protection | Mutable/shared aliasing, escaping borrows, general reborrows and slices, and public zero-copy FFI pass positive and compile-fail suites |
| Regions/arenas | Partial | [Region Structure Report v1](REGION-REPORT-V1.md): read-only `semaprax region-report <file> [--max-bytes N]` emits one digest-authenticated canonical `semaprax.region-report.v1` envelope per verified module reporting, per admitted explicit-ID monomorphic effect-free scalar function, the binding lifetime partition from existing borrow/move facts (real resolved-HIR binding ids, kinds, ownership modes, type keys, definition offsets, effective live-range ends, use counts), canonical region clusters where overlapping live ranges can never share a region, escape facts naming enforcing check `SPX-O104` for today's provably non-escaping borrows, resolved-call-graph own-consumption move facts, and maximal bulk-release grouping candidates of co-dying bindings; independent replay re-derives every derived section exactly (`SPX-L101`-`SPX-L103`; evidence in `tests/region_report_v1.rs`) | Region inference and annotations prevent escape; bulk release and destructor behavior are verified |
| Shared immutable ARC and opt-in managed zones | Partial | [Deterministic ARC Zone Model v1](ARC-ZONES-V1.md): locally evidenced hidden proof data (`src/arc_zones.rs`) fixing bounded per-zone object graphs, accounted retain/release over base-reference plus explicit-handle plus live-payload-link strong counts, exact reverse-construction zone-exit drain order with canonical-order depth-first payload cascades, cycle-participation deferral whose zone exits reject retained cycles (SCCs plus self-loops) fail-closed with one canonical smallest-member witness instead of leaking silently, escape demotion as a deterministic shared-to-unique rewrite rule for sole-held zone-local objects, and closed `Shareable` annotations enforcing single-threaded-by-declaration zones with fail-closed cross-zone/cross-thread sharing; four KAT digests (`b4d9a893…`, `c25ca301…`, `a9da55d2…`, `f04b2180…`), hostile foreign-zone/double-release/unbalanced-exit rejection, permutation determinism, and domain-separated byte-pinned canonical JSON are green locally in `tests/arc_zones_model_v1.rs`. No runtime RC integration, language syntax, compiler/backend change, real allocation behavior, weak references, cycle collection, or cross-backend evidence exists | Retain/release correctness, cycle policy, escape optimization, and concurrency constraints verified for a real compiler/runtime implementation with allocation semantics, diagnostics, and native/Wasm equivalence |
| Restricted `unsafe` and raw memory | Partial | [Unsafe Boundary Mechanics v1](UNSAFE-BOUNDARIES-V1.md): explicit `unsafe { .. }` boundary statements around ordinary safe checked code with a mandatory verbatim `@audit("...")` summary, a required module-level `permit { unsafe }` capability declaration mirroring effect permits, compile-time diagnostics `SPX-N101`-`SPX-N105`, additive Graph nodes (`"kind":"unsafe"`; non-boundary graphs byte-identical to the pinned pre-feature digest), transparent native C11 O0/O2 and Node/Wasm execution, and unchanged CleanupPlan v2 shapes in `tests/unsafe_boundaries_v1.rs`. No raw pointers or memory operations exist or are added; no lint/platform conformance and no safety claims about block contents are made | Unsafe boundaries are explicit graph nodes with capability, audit summary, lint, and platform conformance coverage for real raw-memory features (pointers/volatile/atomics) verified |
| Checked/wrapping/saturating arithmetic | Partial | Checked `i64` arithmetic in the C/Clang lane returns exact `semaprax.arithmetic.v1` statuses without internal process termination | Full numeric family, explicit alternative modes, SIMD behavior, and backend equivalence verified |
| Effects and capabilities | Partial | Declared function effects, module permits, and call-edge propagation | Inference, parameterized capabilities, no ambient authority, handlers, dependency summaries, and platform manifests verified |
| Contracts and progressive verification | Partial | Contract type checking and runtime guards. The additive read-only Property-Test Generation v1 tranche (`semaprax.properties`) now generates deterministic boundary-lattice plus seeded candidates from admitted scalar signatures, filters them through `requires`, evaluates bodies and interprocedural callees with checked semantics under one step budget, and reports exact `ensures` counterexamples in canonical digest-bound `semaprax.property-tests.v1` JSON; it performs no symbolic execution, static discharge, shrinking, or target execution and changes no status | Static discharge, bounded symbolic/SMT checks, counterexamples, invariants/state machines, property tests, and proof obligations verified |
| Structured concurrency | Partial | [Deterministic Scoped Task Model v1](SCOPED-TASKS-V1.md): locally evidenced hidden target-neutral proof model (`src/scoped_tasks.rs`, seven module units plus `tests/scoped_tasks_model_v1.rs`) with a bounded task DAG inside one strict scope tree, exactly-one-parent-join structural containment rejecting escape/double/orphan joins at construction, deterministic sequential scheduling in canonical stable-id order, sticky cancellation propagation before any sibling starts new work with running work draining, children-finalize-before-parents scope exit in reverse completion order, sticky first-failure selection with sibling draining and abandoned dependents, closed per-task `Sendable`/`Shareable` annotations, four pinned KAT trace digests (`c2c1ac40…`, `98a5bf2f…`, `b51cf73d…`, `051da660…`), permutation determinism, byte-pinned canonical JSON traces under domain-separated SHA-256 digests, and 4,096/65,536-count plus 1,000,000-work-unit fail-closed bounds; no runtime threads, scheduler, syntax, backend change, execution, or real-program `Sendable` checking is claimed | Scoped tasks, cancellation, cleanup, `Sendable`/`Shareable`, deterministic scheduler, actors/reducers, and synchronization verified |
| Typed hygienic generation | Partial | The locally evidenced Typed Hygienic Generation v1 tranche (`semaprax hygienic-gen`, `semaprax.hygienic-gen.v1`): deterministic typed AST-to-AST synthesis of default constructors and scalar field accessors from admitted non-generic scalar records, real-verifier admission of the combined program, Graph-visible resolved identities for every generated declaration, stable-ID-derived `__gen_` names that survive rename-with-same-id and code movement, fail-closed hygiene (`SPX-Y102`/`SPX-Y103`) and envelope budgeting (`SPX-Y105`), digest-bound canonical JSON with fixed nonclaims; no textual rewriting, macros, cross-file scope, persistence, or target execution | Sandboxed generation with richer template families (variants, resources, generic records), cross-file generation through workspace transactions, generated-code provenance in patches/evidence, and platform-hosted execution evidence |

## Compiler and output targets

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Fast development lane | Partial | [Reference Interpreter v1](INTERPRETER-V1.md) provides the read-only `semaprax interpret` evaluation lane with deterministic digest-authenticated outcomes, bounded fuel, and its established scalar cross-backend corpus. The internal byte-data tranche locally extends direct verified-HIR evaluation to fixed arrays, owned `Bytes`, borrowed byte views, the six compiler-owned byte operations, exact external-root accounting, and cleanup settlement; this does not widen the public interpreter command profile. No JIT/AOT/Cranelift, incremental persistence, hot reload, debugger mapping, or target execution is claimed. | Cranelift JIT/AOT, incremental affected-node builds, hot reload, and debugger mapping verified |
| Optimizing native lane | Partial | Validated stable-ID HIR lowers to sequenced C11/Clang AOT, including bounded explicitly instantiated generic Copy functions with exact template-plus-ordered-argument symbols, public nested scalar records, exact-instance generic Copy records, bounded irrefutable Copy-record patterns, and bounded copy variants/matches. Local generic-function O0/O2 execution proves exact instance separation, contracts, failure order, and poison; the hosted matrix is green in [run 31385406865, Ubuntu job 93445428338](https://github.com/wavect/semaprax/actions/runs/31385406865/job/93445428338). The internal byte-data tranche additionally executes fixed arrays, owned `Bytes`, borrowed views, exact allocation failure settlement, and plan-driven drops at O0/O2 locally. Existing callable/resource hosted evidence remains separately bounded | Public resource execution/admission, public byte-data adapters, stable public aggregate ABI, generic inference/constraints/richer signatures, refutable/non-Copy matching, general fallback cleanup/quiescence, Android/iOS profiles, LLVM/MLIR lowering, LTO/PGO, cross-compilation, CPU specialization, debug/release parity, and reproducibility verified |
| WebAssembly core/components | Partial | Existing scalar, bounded Copy/generic/variant, owned-Wasm, and private Component evidence remains bounded. Public Wasm Scalar Exports v1 retains hosted scalar evidence; Useful Text v1 remains local. The byte-data tranche proves fixed-memory array/view and guarded owned-`Bytes` internals. Project v3 now locally exposes the bounded public `Slice<u8>` input profile through checked offset/length Core Wasm and strict snapshotting `Uint8Array` JS/TS with fixed memory and authenticated export metadata. Exact-head hosted promotion remains pending. | WASI, owned-byte/public aggregate or resource returns, imports, async/capabilities, additional browser engines, general Component mapping and stable public Component API/ABI verified |
| Embedded and real-time | Partial | [Freestanding Object Profile v1](FREESTANDING-V1.md) adds the read-only `semaprax freestanding-object <file>` projection: one verified effect-free scalar module per invocation, one deterministic canonical `semaprax.freestanding.v1` envelope whose translation-unit bytes derive from the production native C11 projection with the host entry wrapper, stdio/stdlib includes, and public-failure reporter excluded and with recorded invariant-failstop and external-linkage substitutions; explicit no-runtime/no-allocation/no-blocking/no-libc-dependency assertions are recomputed from textual checks and replayed during independent digest verification; closed `SPX-A101`-`SPX-A104` fail-closed diagnostics; pinned envelope/unit KATs, determinism, per-field tamper rejection including forged-but-re-signed payloads, admission rejections, CLI exit codes, and a real toolchain gate compiling the emitted bytes into `-ffreestanding -nostdlib` relocatable objects with `nm`-verified symbol surface bounded by the declared allowed set (`memcpy`, `strcmp`) green locally in `tests/freestanding_object_v1.rs`. No MMIO/volatile/atomics support, linker-script control, hardware/emulator execution, interrupt/RTOS model, or board targets are claimed | Bare-metal artifacts beyond one relocatable-object scalar profile, MMIO/volatile/atomics, linker control, and hardware/emulator tests verified |
| SIMD and GPU | Partial | [Portable SIMD Eligibility Report v1](SIMD-REPORT-V1.md) adds the read-only `semaprax simd-report <file> [--max-bytes N]` analysis: per admitted explicit-ID monomorphic effect-free scalar function of one verified module, one deterministic canonical `semaprax.simd-report.v1` envelope lists every maximal pure straight-line arithmetic sub-expression over `i64`/`i32`/`u8`/`f32`/`f64` (derived from the real resolved HIR nodes, with element type, operator/leaf counts, closed portable operation sequence in post-order evaluation order, and a proposed portable lane width of 2/4/8 selected by the documented deterministic largest-feasible-first rule under the fixed 128-bit lane model ceilings `i64`/`f64`→2, `i32`/`f32`→4, `u8`→8), effect-freedom justification facts with exact call/assignment counts, an explicit closed ineligibility reason for every non-covered expression (`call`, `contract`, `division_remainder`, `bool_mixing`, `char_operation`, `mutation_target`, `computed_operand`, `control_flow`, `aggregate_operation`, `scalar_leaf`), five closed function-admission exclusion reasons, domain-separated SHA-256 digests over payload/source/every region root with independent replay verification, pinned KATs, determinism, budget fail-closed behavior, tamper rejection including forged-but-re-signed envelopes, CLI exit codes, and cross-consistency against real HIR nodes green locally in `tests/simd_report_v1.rs`. No SIMD codegen or intrinsics are emitted, no SPIR-V/WebGPU/GPU kernels exist, no autovectorization is claimed, no target is executed, and hosted promotion is not claimed | Portable SIMD lowering plus SPIR-V/WebGPU/platform kernels and memory/effect rules verified |

## Ecosystem interoperability

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Interface-first packages and target matrices | Partial | Interface Package Report v1 remains the bounded read-only scalar report. Project v2 retains its exact Useful Text package and `semaprax.project-npm-build.v1` carrier. Project v3 locally adds the exact six-file Useful Data package and context-bound `semaprax.project-npm-build.v2` carrier, with independent replay/tamper rejection and no-network pack/install plus compiler-free consumption. Opaque prepared builds retain trusted Project facts; context-free inspection proves consistency only. Unix publication is handle-relative/create-new; Windows v2 publication remains fail-closed. Registry publication and hosted promotion are not claimed. | Resolver, lockfile, compatibility, implementations, capabilities, safe Windows v2 publication, conformance matrices, registry publication, provenance, signatures, licenses, SBOM, and reproducibility verified |
| Portable canonical ABI and native fast ABI | Partial | [Canonical ABI Report v1](ABI-REPORT-V1.md) adds the read-only `semaprax abi-report <file> --function ...` projection: one deterministic canonical `semaprax.abi-report.v1` envelope that reports, per explicitly selected explicit-ID monomorphic by-value `i64`/`bool` function, both the Native64 fast ABI (verbatim production C11 prototype, checked compiler sizes/alignments with `i64` 8/8 and `bool` 1/1, by-value copy semantics, and the status/out contract) and the portable canonical mapping (Core-Wasm `i64`/`i32` signatures, raw export symbols, canonical bool boundary normalization exactly as the web-v4 scalar-export adapters emit it, and fixed copy behavior), under domain-separated digests with independent replay verification, closed `SPX-A201`-`SPX-A204` fail-closed diagnostics, pinned envelope KATs, byte-level cross-consistency against both real backend projections, every exclusion reason, tamper rejection per digest field, and CLI exit codes green locally in `tests/abi_report_v1.rs`; no interface semantics beyond selected scalar exports, no borrowing (copy-only slice), no cross-language conformance suites, no target execution, and hosted promotion are claimed | Equivalent interface semantics with documented copy/borrow behavior and cross-language conformance verified |
| C and Objective-C | Partial | [C Header Emission v1](C-HEADER-V1.md) adds the read-only `semaprax c-header <file> --function ...` projection: deterministic C11 headers for explicitly selected explicit-ID monomorphic by-value `i64`/`bool` functions whose declaration lines are extracted verbatim from the production native C11 projection, with typed stable-ID/contract/effect/status-contract/ownership annotations under a fail-closed hygiene guard, identity-derived include guards that are stable under formatting-only drift, digest-authenticated canonical envelopes with independent replay verification, closed `SPX-D101`-`SPX-D105` fail-closed diagnostics, pinned golden envelope/header KATs, native cross-consistency, every exclusion reason exercised, guard stability rules, tamper rejection, and CLI exit-code evidence green locally in `tests/c_header_emission_v1.rs`; hosted promotion remains pending. Header import, raw bindings, safe wrappers, error/string/buffer mappings, Objective-C anything, and compiled conformance remain unclaimed | Header import, raw bindings, ownership annotations, safe wrappers, error/string/buffer mappings, and tests verified |
| C++ | Partial | [C++ Shim Projection v1](CXX-SHIM-V1.md) adds the read-only `semaprax cxx-shim <file> --function ...` projection: deterministic C++17-compatible `extern "C"` header fragments for explicitly selected explicit-ID monomorphic by-value `i64`/`bool` functions whose declaration lines are extracted verbatim from the production native C11 projection, with typed stable-ID/contract/effect/status-contract/ownership annotations under a fail-closed hygiene guard, identity-derived include guards stable under formatting-only drift, digest-authenticated canonical envelopes with independent replay verification, closed `SPX-X101`-`SPX-X105` fail-closed diagnostics, pinned golden envelope/fragment KATs, native cross-consistency, every exclusion reason exercised, per-field tamper rejection including forged-but-re-signed inner digests caught by replay, and CLI exit-code evidence green locally in `tests/cxx_shim_projection_v1.rs`. No C++ compilation, conformance, adapters, or execution is claimed; hosted promotion remains pending | Stable shim workflow, exception/ownership policy, maintained adapters, and unsafe classification verified |
| Java and Kotlin | Partial | Private generated JNI shim plus minSdk-28 Kotlin ownership wrapper: closed `RegisterNatives`, HandlerThread confinement, generation-tagged handles, fixed status/exception normalization, deterministic identical Cleaner action, explicit `consume()` ownership transfer, and green API-35 x86_64 APK/Instrumentation evidence in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206) | JVM metadata import, public JNI generation, general Android lifecycle/ownership integration, bidirectional calls, and representative hosted conformance verified |
| Swift and Apple frameworks | Partial | Private Swift 6 ownership wrapper, stable-thread static host, generation-tagged handles, target-bound device/simulator fixtures, and bounded XCFramework/installed-Simulator execution are green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228) | Public Swift/Objective-C bindings, async/result/ownership breadth, framework metadata import, distributable XCFramework output, and representative tests verified |
| JavaScript and TypeScript | Partial | Public Wasm Scalar Exports v1 retains its hosted stable-ID `bigint`/`boolean` facade, and Useful Text v1 locally adds bounded `string` inputs. Project v3 locally adds exact `Uint8Array` input declarations and a strict runtime facade that rejects shared/resizable/detached/differently typed/coercible inputs, snapshots accepted bytes, enforces cumulative bounds, and authenticates Wasm/export metadata. Offline installed compiler-free consumption is locally evidenced. The package is unpublished and exact-head hosted promotion is pending. | General strings/typed arrays and owned-byte results, promises/errors, callbacks/resources, additional browsers, package/version compatibility, registry publication, and Component transpilation verified |
| WIT and WebAssembly Components | Partial | Deterministic private profiles v1-v7 retain their frozen bytes and hosted evidence. Private Record-Pattern Projection Component v8 fixes package `semaprax:private@0.6.0`, interface `record-pattern-projections`, world `semaprax-private-v8`, and four exact monomorphic exports over distinct same-layout `Phantom<i64>`/`Phantom<bool>` instances; its exact local and hosted gates are green. Private Generic-Function Instance Component v9 fixes package `semaprax:private@0.7.0`, interface `generic-function-instances`, world `semaprax-private-v9`, and six exact ordered Graph-v14 `FunctionInstanceId` exports; its exact local gates and pinned hosted execution are green in [run 31392541096, job 93467490492](https://github.com/wavect/semaprax/actions/runs/31392541096/job/93467490492). Private Source-Option Propagation Component v10 fixes package `semaprax:private@0.8.0`, interface `option-propagation`, world `semaprax-private-v10`, and the exact compiler-owned `Option<i64>` through postfix-`?` to `Option<bool>` export. Source/Graph-v11/prelude/two-layout/CleanupPlan-v3/core/profile/raw/DAG KATs, independent/upstream validation, every-byte/cross-version hostility, typed/raw Some/None/contracts/arithmetic/sticky-failure/poison/invalid-tag-bool-status evidence, source/runner/CI locks, full gates, and security review are green; pinned Rust 1.97.1/Wasmtime 47 hosted execution is green in [run 31396483313, job 93481068502](https://github.com/wavect/semaprax/actions/runs/31396483313/job/93481068502). V1-v9 bytes remain unchanged | General source selection/export, general/empty/nested/resource/non-Copy record mapping or algebraic carriers, general generic-function components, imports/capabilities, futures/streams, callbacks/reentrancy, browser and multi-engine conformance, package/version negotiation, callable/FFI aggregate signatures, public API/ABI, and `SPX-B104`/`SPX-W111` admission verified |
| OpenAPI, Protobuf/gRPC, GraphQL, and SQL | Partial | The read-only `semaprax openapi` command projects admitted monomorphic scalar signatures of one verified module into a deterministic canonical OpenAPI 3.1 document under a `semaprax.openapi.v1` envelope with domain-separated digests, and `semaprax openapi-compat` authenticates two such envelopes exactly before classifying their difference into closed breaking/non-breaking/informational finding families with a pinned verdict; `tests/openapi_generation_v1.rs` pins the exact document payload/digest, all exclusion reasons, every finding family, determinism, budget fail-closed behavior, and tamper/foreign rejection | Protobuf/gRPC, GraphQL, and SQL projections; schema import parsing; live conformance fixtures; richer type profiles (floats, records, variants, resources); registry/server hosting verified |

## Application platforms

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| First-class application/state/UI dialect | Partial | [UI Dialect Schema Projection v1](UI-SCHEMA-V1.md) adds the read-only `semaprax ui-schema <file> [--max-bytes N]` projection: one deterministic canonical `semaprax.ui-dialect-schema.v1` envelope that describes one verified module's typed application schema — every public non-generic scalar-field record as a state-shape descriptor with field names, `i64`/`bool` types, and offsets/sizes/alignments taken exclusively from the checked Native64 compiler layouts, every explicit-ID monomorphic by-value effect-free scalar function as a typed action descriptor mirroring the abi-report admission profile, dedicated closed exclusion reasons for automatic-identity/generic/resource/variant/mixed records and the six shared function reasons, and an explicit empty-by-default controls/accessibility/navigation nonclaim section — under domain-separated digests with independent replay verification, pinned envelope KATs over three examples, layout cross-consistency against `aggregate_layout`, action cross-consistency against Canonical ABI Report v1 signatures, determinism, budget fail-closed behavior, per-field tamper rejection including re-minted forgeries, and CLI exit codes green locally in `tests/ui_schema_v1.rs`. No typed update/view language constructs, no semantic controls, accessibility, navigation, localization, assets, platform blocks, custom rendering, rendering/runtime/DOM, or target execution are claimed; hosted promotion remains pending | Typed state/actions/update/view, semantic controls, accessibility, navigation, localization, assets, platform blocks, and custom rendering verified |
| Web | Partial | Existing scalar HTML/ES/Wasm and Project-v1 hosted evidence remains unchanged. Project v2 locally adds the config-validator Useful Text package. Project v3 locally adds profile-exact `Slice<u8>` exports, fixed-memory Wasm, strict snapshotting `Uint8Array` JS/TS, exact v2 npm carrier replay, and offline installed binary-frame execution while preserving v1/v2 bytes. It makes no registry or general-browser claim; exact-head hosted promotion is pending. | DOM/CSS application output, accessible UI, SSR/hydration, general collection and Wasm resource/Component support, capabilities, additional browser engines, safe Windows v2 publication, Canvas/WebGPU escape hatch, registry publication, and deployable sample verified |
| iOS | Partial | Existing private static callable runtime plus a Swift 6 same-thread host, device/universal-Simulator XCFramework construction, and two installed arm64-Simulator app paths are green in [run 31338834586, job 93309086228](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086228) | Public native/Swift host, distributable framework/app project, UIKit/SwiftUI adapter, lifecycle, accessibility, signing metadata, and physical-device plus representative simulator samples verified |
| Android | Partial | Private same-package no-UI Instrumentation APK executes on an API-35 x86_64 Emulator with offline plugin-free packaging, exact JNI/O0/O2 inventory and ownership assertions in [run 31338834586, job 93309086206](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086206); arm64 remains compile/ELF inspection only | Public native code and Kotlin/JNI host, AAR/app project, Compose/View adapter, lifecycle, accessibility, manifests/packaging, and representative emulator plus device samples verified |
| macOS | Partial | A private headless `APPL` engine and AppKit frontend with one visible window/button, native accessibility label, pre-launch engine digest, bounded terminate/kill path, and ordered control/close/terminate evidence are green in [run 31338834586, job 93309086230](https://github.com/wavect/semaprax/actions/runs/31338834586/job/93309086230) | Public/general AppKit or SwiftUI host, signed engine provenance, menus/navigation, comprehensive accessibility/lifecycle, signing/notarization metadata, and representative sample verified |
| Windows | Partial | A private portable PE engine package and separate Win32 GUI-subsystem frontend with one visible window/button, `IAccessible` name, pre-launch engine digest, exact imported-DLL/no-export-directory contract, and ordered control/destroy/quit path are green in [run 31343897595, job 93322134480](https://github.com/wavect/semaprax/actions/runs/31343897595/job/93322134480). Earlier hosted evidence also confirms the callable corpus and dependency isolation in [run 31257545008, job 93103151756](https://github.com/wavect/semaprax/actions/runs/31257545008/job/93103151756) | Public/general Win32 or WinUI host, signed engine provenance, comprehensive accessibility/lifecycle, installer/MSIX/signing metadata, and representative application sample verified |
| Linux | Partial | Host compilation exercised in CI | Native application, selected UI adapter, accessibility, AppImage/deb/rpm metadata, and sample verified |
| Edge and server | Partial | Host-native scalar CLI only | Server runtime, async I/O, HTTP/data adapters, native/WASI output, observability, deployment, and load/conformance tests verified |
| Plugins | Partial | [Plugin Manifest Projection v1](PLUGIN-MANIFEST-V1.md) adds the read-only `semaprax plugin-manifest <file> [--max-bytes N]` projection: one deterministic canonical `semaprax.plugin-manifest.v1` envelope per verified module with the sorted provided-export inventory (explicit-ID monomorphic effect-free by-value `i64`/`bool` functions under per-export digest-authenticated verbatim Native64 signatures), six closed exclusion reasons, module-name plus digest-derived build-hash version identity fields, required host capabilities derived exactly like Build Capability Manifest v1 over the closed five-domain vocabulary with fail-closed `SPX-Q101`-`SPX-Q104` diagnostics, an explicit empty-by-default resource-limits section, and a closed unavailable-sections inventory; independent replay re-derives counts, sections, vocabulary, ordering, identity/version consistency, and digests, so forged-but-re-signed mutations still fail closed. Pinned KATs, determinism, budget exhaustion, CLI exit codes, tamper rejection per digest field, and cross-consistency with `semaprax capability-manifest` and `semaprax abi-report` are green locally in `tests/plugin_manifest_v1.rs`. No Component Model runtime or packaging, host loading/lifecycle, versioning negotiation, resource-limit enforcement, hostile-plugin execution tests, or target execution is claimed | Capability-limited Component Model plugins, lifecycle, versioning, resource limits, and hostile-plugin tests verified |

## Agent economics, review, and operations

| Requirement | Status | Current evidence | Completion gate |
| --- | --- | --- | --- |
| Token-budgeted semantic context | Partial | Versioned deterministic `semaprax.agent-context.v1` adds exact whole-JSON byte and function-node budgets, used/omitted/deferred accounting, closed truncation reasons, query-bound stable-ID progress frontiers with non-dangling emitted call edges, aggregate pagination plus individual-page oversize rejection, strict CLI options, and selectable compact contracts, parameter/result ownership, effects, and reference-closed types. Explicit-direction `semaprax.agent-context.v2` retains those limits and the exact v1 default while adding forward/reverse/both call traversal, independently built caller edges, global per-depth stable-ID order, minimum-depth direction provenance, and disjoint traversal/reference frontiers with direction-bound replay. Its forward/reverse/both SHA-256 KATs are `922404133444942ab86607772362098e0f5656add6bea607a890be2bcfe5b7c9`, `9a2ebfe569926e67f436379cf2b5c96d510daadd11d0a295ed54903cb612627b`, and `4ec8a62a17551e87dc301d08f0a09c6159445757bca6dd9920a7db4e3790ce17`; the full matrix is hosted green in [run 31397881268, Ubuntu job 93485198327](https://github.com/wavect/semaprax/actions/runs/31397881268/job/93485198327). Offline `semaprax.agent-context-economics.v1` freezes four maintenance questions, two exact context goldens, canonical exact-case/separator-normal/Windows-forbidden-and-reserved-safe non-symlink source containment, exact manifest/context digests and label IDs, supported-facet-only source/context byte and non-model lexical-unit economics, reviewed relevance/evidence recall, mutations, and conservative explicit-or-unique-target-merge-base plus dirty-Git-reconciled quick/changed/full routing with an exact ordered executable gate plan. The small corpus records context larger than source; v2 remains call-graph-only, cleanup/lifecycle/import and target/diagnostic/test facets remain unavailable, and Graphify remains deferred under ADR 0001 | Exact model-token budgets, real target/diagnostic/test graph edges, cleanup/lifecycle/import facets, answer-quality/relevance guarantees, persistent indexing, representative large-repository/model benchmarks, and measured savings verified |
| Impact analysis before modification | Partial | `semaprax.semantic-impact.v1` deterministically previews one Semantic Patch v1/v2 file without writing source. It binds exact source base/candidate revisions, Graph v10-v14 schema, patch schema and processed-byte digest; preserves operation/change/source-consumer provenance; and computes byte/node/depth-bounded reverse callers only for exact generic-call instance changes over explicit persistent functions/templates. The canonical report SHA-256 KAT is `94bbe5dcfe02f4b80b12ba5c8faf0889ddf11a96598072e539490c71a09518e9`; the exact `1b3731a` full matrix is hosted green in [run 31408654657 attempt 2](https://github.com/wavect/semaprax/actions/runs/31408654657/attempts/2), including [Ubuntu job 93530141404](https://github.com/wavect/semaprax/actions/runs/31408654657/job/93530141404). Rename consumers are source projection, automatic behavioral callers fail closed as `SPX-G110`, existing `SPX-T226` closure remains, and patch provenance, non-call semantics, repository/multi-file scope, persistence/incrementality, ranking, repair, and commit authority remain nonclaims. Impact itself emits no review sections; the separate Review v1 layer embeds its complete nontruncated report | Call/type/contract/test/schema/migration/target/capability consumers are computed incrementally and verified on real repositories |
| Typed holes and compiler-generated repairs | Partial | Bounded Diagnostic Repair v1 exposes one exact `SPX-S103` automatic-function identity repair through canonical query/instantiation JSON, classifies it `breaking_identity_rebase`, independently proves the one-annotation HIR/normalized-Graph rebase, and emits the sole canonical three-line Semantic Patch v3 `assign-function-id` operation. V3 revalidates the complete reduced repair domain and applies through unchanged single-file A0. Impact v1 rejects every syntactically valid, canonical v3 as `SPX-G110` before semantic selector interpretation; malformed or noncanonical v3 remains `SPX-G101`. The breaking operation changes Graph-v10 revision/identity/callee/derived-ID content and may rebase identity-bearing CleanupPlan content, but widens no Graph or CleanupPlan schema/version or semantic shape, admits no Graph v11-v14 repair, and changes no backend/runtime semantics. Frozen query/preview/independently authored Graph SHA-256 KATs are `ef689fed2c742dea6cedb0b8ec3d449e5facd8748dd00cb8a8f2e6115be82075`, `ae779749b252e5d9661172dfebcd3317211b97310eed57a0a6b7a692be1053e4`, and `d255c0e88ff497436ca0737ffd139cf47c2c142cf1b4f2da071514c0515ad2b3`. Local Phase A integration is 13/13; the Phase B semantic integration corpus is 7/7; v3 A0 hook units are 4/4; aggregate v3 integration-plus-hook evidence is 9/9; and the library suite is 404/404. Full preservation and security review are green. The exact `dae957a` full matrix is hosted green in [run 31418476217 attempt 1](https://github.com/wavect/semaprax/actions/runs/31418476217/attempts/1), including [Ubuntu job 93553147265](https://github.com/wavect/semaprax/actions/runs/31418476217/job/93553147265); all 12 jobs passed. Typed holes, other diagnostics/declaration kinds, ranking/composition/automatic application, general or multi-file repair, authenticated patch provenance, and other v3 operations remain open | Obligations and valid repair operations are machine-readable and proven sound by compile-fail/repair tests |
| Proof-carrying patches | Partial | `semaprax.semantic-patch-evidence.v1` binds exact source/Graph/base/candidate/Patch/Review/supporting-evidence facts and work accounting for every admitted Patch v1/v2 operation and the sole canonical Patch v3 operation. `verify-patch-evidence` independently rebuilds and byte-compares the capsule; the separate `patch-with-evidence` route acquires the unchanged A0 lock and requires exact replay before staging and final commit. Capsule SHA-256 KATs are `03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa`, `23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba`, and `d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c`; receipt KATs are `1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1`, `6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48`, and `13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b`. A+B is 11/11 plus 5/5, Phase C 16/16 plus 11/11, library 420/420, and doctest 37/37 locally. The exact `34a8ed82e9ae96277aa51e7994c19644331f5e78` replacement matrix is hosted green in [run 31431768632](https://github.com/wavect/semaprax/actions/runs/31431768632), including [Ubuntu job 93596706949](https://github.com/wavect/semaprax/actions/runs/31431768632/job/93596706949); all 12 jobs passed. Additive `semaprax.semantic-workspace-patch-evidence.v1` binds sorted per-file child Evidence-v1 facts over one exact managed Workspace preview and gates apply on replay before candidate/staging creation. Its local public 6/6, apply 5/5, hostile 2/2, units 8/8, shared Workspace 39/39, root library 496/496, and preservation 107/107 are green. The exact `cda4892ee74100fd11c5161ad857d469ec5e5421` matrix is hosted green in [run 31491573287](https://github.com/wavect/semaprax/actions/runs/31491573287), including [Ubuntu job 93779117078](https://github.com/wavect/semaprax/actions/runs/31491573287/job/93779117078); all 12 jobs passed. Ordinary `patch` and Workspace apply remain unchanged. Neither capsule is signature/provenance, approval, target/test execution, general formal proof, reusable authorization, repository/cross-file semantic analysis, external-consumer compatibility, or new Patch/Graph/Cleanup/runtime semantics | General patch claims, tests, capability deltas, target expectations, authenticated provenance/approval, repository and multi-file scope, consumer compatibility, and proof artifacts are independently verified before commit across supported targets |
| Semantic human review | Partial | Bounded `semaprax.semantic-review.v1` emits a deterministic read-only single-file report for Patch v1/v2 and the sole canonical Patch v3 `assign-function-id`. V1/v2 embed complete nontruncated Impact v1 evidence; v3 embeds the exact shared `semaprax.identity-rebase.v1` object and no Impact report. Every operation has one evidence-linked finding in each fixed wire section: `behavior`, `api_identity`, `security_authority`, `memory_ownership`, `target_artifact`, `migration`, and `unsafe`. Exact v1/v2/v3 report KATs are `054c12822e9984b3f9cab06056f311f35af3b06a438af7ade0b452a823443946`, `37fe056f519366fcaf6c13586e3b78afd64d51483490a1120e3e0fdc1b04c421`, and `081bcb20aca2e74f724f5bc0cd2cf03770a499e11aa090d92b59650209165544`. Local Review integration is 10/10, hook/limit units are 4/4, and library 408/408, full preservation, and security gates are green. The exact `2634011f3d205077d4533701e412bec8fdcff7c8` full matrix is hosted green in [run 31423743369 attempt 1](https://github.com/wavect/semaprax/actions/runs/31423743369/attempts/1), including [Ubuntu job 93570423170](https://github.com/wavect/semaprax/actions/runs/31423743369/job/93570423170); all 12 jobs passed. The report has no flags, Context, target/test execution, public verifier/proof artifact, authenticated provenance, approval UI/policy, or A0 authority and is not general repository/multi-file/security/memory/unsafe/target/migration analysis | Behavioral/API/security/memory/target/migration/unsafe-code summaries are deterministic and checked against known changes |
| Sandboxed builds and dependencies | Partial | [Build Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) adds the read-only `semaprax capability-manifest <file>` projection: one verified module's exact build capabilities — sorted module permit inventory, per-function declared effect sets, per-interface-import effect sets — inside a closed five-domain vocabulary (filesystem, home, network, process, secrets) with an explicit empty-by-default ambient authority assertion, domain-separated digest-authenticated canonical JSON envelopes with independent replay verification that re-derives the ambient section and re-checks the vocabulary, fail-closed `SPX-K201`-`SPX-K204` diagnostics for options, out-of-vocabulary capabilities, budget exhaustion, and injection/tamper/drift, plus pinned golden KATs, determinism, and CLI exit-code evidence green locally in `tests/capability_manifest_v1.rs`; hosted promotion remains pending. No sandbox is enforced at build time, no network/home/secrets/filesystem/process enforcement machinery exists, and Project Manifest v1 still has no resolver, lockfile, dependency graph, or package registry | Resolver, lockfile, compatibility, dependency graph, package registry, actual capability enforcement against the declared manifest, hostile dynamic package tests, and reproducibility verified |
| Debugger, profiler, diagnostics, and operations | Partial | Stable diagnostics for the scalar seed | Source-level debugging/profiling, crash/trace mapping, observability, deployment diagnostics, and every backend verified |

## Final validation product

Completion requires one maintained offline-first product built from a shared SEMAPRAX codebase with web, iOS, Android, macOS, Windows, and Linux clients; native notifications and secure storage; local databases; native/WASI backend; authentication; background synchronization; a custom accelerated visual; one C library; one JavaScript package; and one WebAssembly component. Every artifact must be built and exercised in CI or on representative simulators/devices, with platform-specific implementations declared rather than hidden behind false portability.
