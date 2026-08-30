# Roadmap

Status: living future-sequencing document; not implementation evidence.

Audience: contributors, maintainers, and project evaluators.

The roadmap orders future outcomes. It is not implementation status and does
not repeat completed milestone history. Use the [completion
matrix](COMPLETION-MATRIX.md) for current claims and the
[changelog](../CHANGELOG.md) for what changed.

SEMAPRAX follows risk rather than feature count. Stable semantic identity,
sound ownership, replayable change authority, and honest target boundaries take
priority over broad syntax or generated artifact volume.

## Current priority: close exact-head promotion gates

The current codebase has a bounded multi-file calculator, Project agent
workflow, stable-ID JavaScript/TypeScript and unpublished Rust consumers, and a
multi-file line-filter product. The upstream baseline blocking matrix is green
at its exact commit `4cc03820c86e70527cb65c4b10ee3841c7af167d` in
[run 33259787886](https://github.com/wavect/semaprax/actions/runs/33259787886).
That historical exact-head evidence predates and does not promote the later
WP-01–WP-15, Project v8, Agent Transport v5, Project v9, or Project v10 work.
The release exit remains open on line-filter browser/runtime breadth,
intentional Rust publication, final tagged-artifact execution, and release
notes.

Exit outcomes:

1. Confirm the line-filter product on hosted native and WebAssembly lanes and
   add the browser/runtime breadth claimed by the release.
2. Publish the Rust builder only through an intentionally supported entry
   point, or keep the release claim explicitly unpublished.
3. Publish release notes that cite the exact commit and preserve all bounded
   non-claims.
4. Complete WP-04 only after the tagged release artifacts, checksums, and smoke
   paths pass at that same exact release head.

The [v0.2 audit](COMPLETION-MATRIX.md#v02-product-exit-audit) is the acceptance
checklist.

## Developer preview: promote the authored Project v8 slice

Implementation has moved ahead of the intended promotion sequence: the
additive [Public Owned Data API v1](PUBLIC-OWNED-DATA-API-V1.md) Project v8
profile, npm/Wasm route, safe Rust package route, reference-interpreter lane,
frame-payload validation product, and read-only Project Agent Transport v5 are
now authored in the current source tree. This roadmap does not treat authored
tests as executed evidence. Exact-head hosted promotion remains open, and
v1–v7 compatibility is still a mandatory gate.

The next outcomes are therefore validation and promotion, not another semantic
widening:

1. execute the descriptor, HIR-link, carrier, settlement, hostile-input, and
   v1–v7 known-answer gates at one integration head;
2. execute the identical frame-payload corpus through interpreter, native
   C11 O0/O2, Core Wasm/Node, installed npm, and compiler-free Rust consumers;
3. run strict TypeScript, real-browser, sanitizer, MSRV, and Linux/macOS/Windows
   jobs without skips, retries, masks, or allowed failures;
4. preserve the baseline/display-renamed stable-ID proof across both generated
   consumer packages; and
5. record the exact commit and hosted run before describing the bounded API as
   promoted, supported, or released.

The authored additive read-only [Project Agent Transport
v5](PROJECT-AGENT-TRANSPORT-V5.md) exposes only the canonical API descriptor and
bounded inline npm carrier. Promotion must still prove exact revision and typed
descriptor replay, zero write/process/publication authority, and byte-frozen
v2–v4 behavior before any broader agent workflow is considered.

Records, authored variants, nested algebraic data, owned UTF-8 strings,
allocator transfer, callbacks, async work, and general public aggregate ABIs
remain outside the Project v8 preview. Project v9 flat owned records and
Project v10 owned UTF-8 are additive implementation tranches, not promotions
of the Project v8 preview; internal record/variant support is not a public
aggregate ABI.
The versioned specification owns exact identifiers, admission, lifetime,
compatibility, and promotion gates.

The first controlled widening after that promotion is the additive
[Public Flat Owned Record API v1](PUBLIC-FLAT-OWNED-RECORD-API-V1.md): one
monomorphic result record with exactly one direct `Bytes` field and only direct
`i64`/`bool`/`usize` siblings. Its descriptor and host projections must remain
layout-independent. Its descriptor-bound npm/Core-Wasm and
native-provider/safe-Rust routes are now wired while preserving the v8 target
routes. The implementation and executable evidence are authored but unrun;
the generated packages are unpublished, and hosted promotion remains
outstanding.

The authority-neutral [Project Profile Admission
v1](PROJECT-PROFILE-ADMISSION-V1.md) dispatcher is now authored as the sole
ordinary Phase-A profile gate. It routes the existing v9 descriptor and Wasm
adapter through normal Project construction and Revision Store replay while
preserving v1-v8 and v10 schemas. Its focused evidence is unrun and does not
promote or publish any Project profile.

The next additive string tranche is specified by
[Public Owned UTF-8 API v1](PUBLIC-OWNED-UTF8-API-V1.md). Project v10 is gated
on promoted Project v9 and keeps raw `Bytes` distinct from length-delimited,
strictly validated host strings. Its implementation and executable evidence
are likewise authored but unrun and unpublished. Neither tranche is promoted,
and authored generator/provider evidence is not a hosted result or a
completion-matrix status change.

## 0.3: ownership and fast development

Goal: make the language safer and faster to iterate on without widening public
ABIs prematurely.

### Language and ownership outcomes

- generalize unique ownership beyond the current bounded Copy, string, byte,
  resource, flat owned-byte record, and flat owned-byte variant slices;
- use the bounded [Shared Loan Plan v1](SHARED-LOAN-PLAN-V1.md) as the
  independently replayed proof foundation; the authored-but-unrun
  [Projected Owned-Byte Field Shared Borrow v1](PROJECTED-OWNED-BYTE-FIELD-BORROW-V1.md)
  admits one direct `Bytes` field while deeper/general nested borrowing remains
  ahead of general lifetime inference, mutable borrowing, and escape analysis;
- make cleanup plans cover general control flow, nested aggregates, calls, and
  FFI with independently replayed exactly-once behavior;
- integrate regions/arenas and opt-in shared immutable ARC only after their
  proof models have executable language and runtime counterparts;
- define restricted raw-memory operations and an auditable `unsafe` policy;
- extend aggregate, `Option`, `Result`, and matching beyond the exact
  [Owned Byte Variant Algebra v1](OWNED-BYTE-VARIANT-ALGEBRA-V1.md) profiles,
  including nesting, authored generics, non-Copy propagation, and public ABIs;
- complete mutation and generic interactions across interpreter, native, and
  Wasm lanes.

### Development-loop outcomes

- execute and promote the authored retained
  [Prepared Project Interpreter and Source Trace v1](PROJECT-PREPARED-INTERPRETER-V1.md),
  then evolve it into incremental refresh without weakening revision binding;
- extend its expression-origin trace into source-level debugger and diagnostic
  mapping across target runtimes;
- execute and mature the authored
  [Project Revision Store v1](PROJECT-REVISION-STORE-V1.md), which persists
  exact authenticated Project inputs only through one injected trusted,
  current-euid-owned `0700` held root under an explicit host-exclusive
  same-principal, ancestor, and Darwin-ACL mutation precondition; its authored
  hardening keeps persistence fail-closed while allowing unrelated reads past
  one untraversed inert stage identity and exposes only an authority-neutral
  locator for full-replay ambiguity resolution. An additive authored-unrun
  [Windows-entry-v1 authority](PROJECT-REVISION-STORE-WINDOWS-V1.md) preserves
  ordinary Unix-v1 bytes through a separate schema and explicit APIs. It
  accepts only fixed local NTFS under exact effective-SID and
  protected-DACL admission, relative held handles, a validated identity mutex,
  and one non-replacing handle-relative publication pivot. It deliberately remains
  neither an ambient cache nor a verifier bypass;
- broaden context and impact edges beyond the current bounded call and
  workspace families;
- measure semantic-context cost and usefulness on representative repositories
  and actual model tokenizers.

Exit condition: representative owned applications pass the same success,
failure, cleanup, and contract corpus through the development, native, and
WebAssembly lanes, with stable source/graph migrations.

## 0.4: components, packages, and interoperability

The first bounded offline lock is now authored as a read-only graph over an
explicit finite set of integrity-bound Package Report subjects. It establishes
canonical coordinates, dependency-first order, graph rejection, exact target
intersection, and declared-capability closure without a registry, fetch,
resolver, scripts, compilation, or publication authority. Its local gates are
unexecuted and it is not yet the production package manager described by this
milestone.

An additive Semantic Package Report v2 implementation is also authored but
unrun. It makes the report subject self-contained and source-authenticated and
projects stable type, ownership, effect, structural-contract, reachable-type,
and ternary target facts. Compatibility classification remains a subsequent
stage and no milestone status is promoted.

An additive source-authenticated Lock v2 and stable-ID-only Compatibility
Evidence v1 are authored above Report v2. Unknown semantic closure or lock
context drift remains indeterminate; evidence is unrun and the milestone is
not promoted.

An additive Offline Deterministic Package Resolver v1 is authored above those
exact V2 subjects. It selects a first-feasible, bounded, source-replayed graph
under strict semantic-version, target, and declared-capability policy and emits
one independently replayed Lock-v2 result. Its focused public evidence is
unrun. It is not acquisition, a registry/cache, a published lock workflow, a
build sandbox, target execution, trusted provenance, or runtime capability
enforcement, so the milestone remains unpromoted.

An additive Offline Published Semantic Lock Snapshot v1 now captures exact
Resolver-v1 input, unchanged resolution evidence, and unchanged Lock-v2 bytes,
then optionally publishes that fixed three-file inventory into one fresh local
directory through the existing safe lower authority state machine. Its hostile
replay, bound, and publication evidence is authored but unrun. This is not an
updateable package lock workflow, registry/cache, trusted provenance, build,
target execution, or sandbox, so 0.4 remains unpromoted.

Additive Subject/Lock v3 and Resolver v2 are authored as the bounded
package-authenticated dependency-range prerequisite. They add exact, tilde,
and caret constraint intersection and bind every selected version back to its
authenticated requirement. Their evidence is locally unrun and unpromoted;
general compatibility negotiation, acquisition, registry/cache, supported
publication, and trusted provenance remain later work.

An additive Offline Effect-Free Scalar Core-Wasm Package Build v1 is authored
above exact Resolver-v1 replay. Its intentionally narrow first slice accepts
one dependency-free selected Subject v2, replays the embedded canonical source,
emits the unchanged scalar Core-Wasm profile, authenticates the exact runtime
import/export inventory, and returns canonical manifest/evidence bytes. A
separate safe crate provides create-new exact-inventory local publication after
independent replay. The hostile wire, association, cross-pairing, bound, and
publication evidence is authored but unrun. This is not multi-package source
linking, acquisition, a registry/cache, trusted provenance, runtime execution,
capability enforcement, or a hermetic sandbox, so 0.4 remains unpromoted.

An additive Offline Multi-Package Source Capsule v1 is authored above exact
Resolver-v1 replay. It admits two through four effect-free scalar packages,
requires the source-derived import graph to equal the selected Subject-v2
dependency graph, exact-compares normalized Report-v2 interfaces, binds an
explicit root and only its explicit exports, and retains the ordinary linked
HIR behind a crate-private replay seam. Its focused evidence is unrun. It is
not a package build, acquisition, publication, provenance, target execution,
runtime enforcement, or hermetic sandbox, so 0.4 remains unpromoted.

An additive Linked Scalar Core-Wasm Package Build v2 now consumes only that
capsule's exact replay receipt and retained linked HIR. It binds the selected
package closure, explicit root and root-owned exports, source-set/link facts,
and distinct canonical v2 manifest/evidence around the unchanged scalar Wasm
emitter. The safe publisher reuses the v1 held-authority state machine rather
than adding platform authority. Two-package, hostile cross-pair/mutation,
fixed-point/boundary, and publication-settlement evidence is authored but
unrun; no target conformance, acquisition, trusted provenance, or hermetic
sandbox is claimed, so 0.4 remains unpromoted.

Goal: turn bounded reports and private host evidence into a supported,
versioned ecosystem surface.

### Package outcomes

- interface-first manifests that carry the bounded resolver into a published
  lockfile workflow with target matrices, capability closure, provenance,
  licenses, and reproducible artifact records;
- execute the authored source-capsule, linked-build-v2, shared-publication, and
  build-v1 preservation evidence on the exact candidate head;
- compatibility analysis over types, effects, contracts, ownership, and target
  availability;
- a package registry and offline cache model with explicit least authority;
- stable migration rules for language, graph, patch, package, and ABI schemas.

### ABI and host outcomes

- stable canonical and native ABIs for aggregates, resources, borrowed views,
  strings, errors, callbacks, and async work;
- supported C/C++, Rust, Java/Kotlin, Swift/Objective-C, JavaScript/TypeScript,
  and WIT consumers with conformance suites;
- WebAssembly Component Model publication and multi-runtime execution;
- replacement of private loader/host fixtures with intentionally public,
  reviewed APIs where appropriate;
- capability-limited plugin loading and hostile-plugin tests.

Exit condition: one versioned package is consumed from every supported host
language and target lane with reproducible builds, compatibility checks, and
no undocumented ambient authority.

## 0.5: concurrency and applications

Goal: demonstrate that verified shared meaning can support real applications
without pretending every platform is identical.

### Concurrency and services

- structured tasks, cancellation, cleanup, `Sendable`, and `Shareable` checks;
- deterministic effect handlers and test schedule replay;
- general command, filesystem, network, clock, and service I/O through explicit
  capabilities;
- server/edge packaging, observability, deployment diagnostics, and load tests.

### Application model

- typed state, actions, update functions, semantic view trees, navigation,
  localization, assets, accessibility, and lifecycle;
- accessible DOM/CSS and hydration for the web;
- supported Swift/Apple, Kotlin/Android, Windows, Linux, and desktop adapters;
- explicit platform blocks and custom accelerated rendering escape hatches;
- distributable artifacts with permissions, entitlements, manifests, and
  signing metadata while credentials remain outside compiler authority.

Exit condition: one shared application has maintained web, iOS, Android,
macOS, Windows, and Linux clients with declared platform differences and
representative hosted or device evidence.

## 1.0: validate the complete programming system

The 1.0 gate is the final product in the
[completion matrix](COMPLETION-MATRIX.md#final-validation-product), not a
version-number aspiration.

It requires one maintained offline-first product with:

- all six client platforms from the shared SEMAPRAX program;
- native notifications, secure storage, local databases, authentication, and
  background synchronization;
- native or WASI server execution;
- a custom accelerated visual;
- one C library, one JavaScript package, and one WebAssembly component;
- reproducible builds, compatibility and migration evidence, and representative
  CI/simulator/device execution;
- complete language safety, debugger/diagnostic, package, capability, and
  operations gates for the features the product uses.

No narrow report, generated fixture, or private platform adapter substitutes
for this maintained end-to-end proof.

## Research profiles after the core product

Economic-agent work remains optional and subordinate to the language's
authority model. The current injected-host policy and evidence core grants no
built-in provider transport, wallet, key, mainnet, or signing authority. Any
future profile must preserve explicit capabilities, approvals, custody
separation, idempotent settlement, private-data boundaries, and complete audit
traces without weakening the core product gates.
